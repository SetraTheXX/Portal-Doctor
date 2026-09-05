# ASHPD Integration Decision for v0.3.0

**Status:** Accepted for Phase 8 planning
**Decision date:** 2026-09-05
**Scope:** active FileChooser, Screenshot and ScreenCast probes
**Implementation status:** first bounded FileChooser lifecycle implemented and
audited on `main`; Screenshot and ScreenCast remain future slices

## Decision

PortalDoctor will use a **PortalDoctor-owned D-Bus lifecycle adapter**, with
ASHPD treated as a compatible high-level reference and an optional helper only
where its public API preserves the lifecycle information we need.

ASHPD will not be the sole orchestration layer for v0.3.0. The adapter will own
the request/session state machine through the existing `zbus` major line so it
can:

- subscribe to the `Request::Response` signal before making the portal call,
- provide a unique `handle_token`, derive the expected request path before the
  call and retain/validate the returned request handle,
- apply a timeout to each lifecycle stage,
- call `Request.Close` on timeout or explicit cancellation,
- call `Session.Close` for ScreenCast sessions, and
- report cleanup failure separately instead of silently treating a cancelled
  task as a completed probe.

This is a deliberate **hybrid boundary**, not a rejection of ASHPD. ASHPD is
the preferred reference for portal method names, option/result models and
portal-specific error semantics. Direct `zbus` is required where the public
high-level wrapper would hide a live request before its response arrives.

## Why ASHPD is useful

The current ASHPD `0.13.13` release covers the three planned portal families
(`file_chooser`, `screenshot` and `screencast`), is Rust-native, uses zbus and
offers a Tokio runtime feature plus per-portal feature flags. A compatibility
spike confirmed that `ashpd 0.13.13`, the repository's locked `zbus 5.19.0`
line and a Tokio current-thread runtime compile together.

Using its concepts as the reference avoids inventing a second interpretation
of the XDG portal API. Its typed error surface also gives the future adapter a
clear starting taxonomy for portal rejection, user cancellation, missing
interfaces and transport failures.

## Why ASHPD cannot own the entire lifecycle

The XDG portal protocol is signal-based: a portal method returns a Request
object, then the result arrives through `Request::Response`; the caller may
abort the interaction with `Request.Close`. The caller must also subscribe
before the method call to avoid a response race.

ASHPD's public high-level builders follow the convenient shape
`builder.send().await?.response()?`. In ASHPD `0.13.13`, the internal request
waits for the response before `send()` returns its `Request` object. Therefore,
wrapping that call in a timeout does not leave PortalDoctor with the request
handle needed to issue an explicit `Request.Close` after the timeout. Dropping
the future is not accepted as a cleanup proof for a diagnostic tool whose
contract requires bounded, observable cleanup.

The same concern is more important for ScreenCast, where a live Session and a
PipeWire file descriptor must be closed on every success, cancellation, timeout
and failure path. The adapter therefore owns these handles and may use ASHPD
helpers only after the cleanup boundary is demonstrably preserved.

### FileChooser late-reply boundary

The first FileChooser adapter sends a per-request `handle_token` and subscribes
to `Request::Response` before `OpenFile`. For modern portals this makes the
Request path predictable as
`/org/freedesktop/portal/desktop/request/<sender>/<token>`; the returned path is
still validated against the caller's sender so older portal implementations
that generate a different token remain usable. The method future is retained
through a bounded recovery window after a request-stage timeout or Ctrl-C.

If a late reply arrives, its returned handle is closed immediately. If no
reply arrives, the predicted handle is closed once. A successful `Close` is
verified; an unknown-object response on a path whose method reply never
arrived is intentionally `unverified`, because a transport that never
returned cannot prove whether the portal created the request after the close
race. This is a bounded, observable non-clean result rather than an implicit
fallback or a false cleanup claim. A handle returned by `OpenFile` treats an
already-gone object as completed, because the portal has already completed or
removed that known request.

## Planned dependency and runtime boundary

The first FileChooser slice intentionally does not add ASHPD. It enables the
Tokio runtime feature on the existing `zbus` line and adds the small
`futures-lite`/Tokio runtime boundary required to own the response stream and
cleanup calls. The passive blocking collectors continue to use their existing
path. An ASHPD dependency remains deferred until a later probe can demonstrate
that its public API preserves the same lifecycle observability.

The initial implementation experiment used:

```toml
ashpd = { version = "0.13.13", default-features = false, features = [
  "tokio",
  "file_chooser",
] }
```

The later Screenshot and ScreenCast slices may add their feature flags only
when their own implementation begins. No GTK, Wayland window-handle or
PipeWire FFI feature is required for the first CLI FileChooser slice.

The async boundary will be a short-lived, current-thread Tokio runtime owned by
the explicit probe command. It will not be introduced into the passive default
path, and it will not replace the existing blocking collectors. The adapter
will use `tokio::time::timeout` (or the equivalent central policy) around each
portal stage rather than one unbounded timeout around the whole process.

The ASHPD crate currently declares Rust 1.87 as its minimum toolchain. The
project's future implementation must either document that MSRV or select a
compatible ASHPD version before adding the dependency. The current repository
does not yet declare an MSRV, so this decision does not silently change that
public contract.

## Compatibility assumptions

The first real-session validation remains limited to the existing support
baseline:

- Ubuntu 26.04
- GNOME, including `ubuntu:GNOME`
- Wayland
- systemd user session
- `org.freedesktop.portal.Desktop`

The adapter must query portal interface versions and treat an absent or too-old
interface as `unsupported`/`unavailable`, not as a generic parser failure. It
must not assume that a GNOME backend is present merely because the frontend is
reachable. Existing route evidence remains diagnostic context; active probes
must call the portal frontend and must not bypass routing by invoking a backend
directly.

No KDE, wlroots/Sway, Hyprland or Niri compatibility claim is created by this
decision. Those environments remain Phase 9–11 work.

## Error and fallback policy

The future adapter must preserve these distinctions before translating them to
the public `ProbeResult` contract:

| Condition | Internal meaning | Fallback behavior |
| --- | --- | --- |
| Response success | Portal completed the requested lifecycle stage | Continue to the next stage or return success metadata. |
| Response cancelled | User explicitly cancelled the dialog | Return a first-class cancellation result; never label it transport failure. |
| Response other | Portal interaction ended without success or cancellation detail | Return an explicit portal-interaction outcome. |
| Portal `NotFound`/version mismatch | Frontend or interface is unavailable/unsupported | Stop the probe; do not try an arbitrary backend. |
| Portal rejection (`Failed`, `InvalidArgument`, `NotAllowed`, etc.) | Portal/backend rejected the request | Preserve the portal error category and available evidence. |
| D-Bus transport/name/permission error | Runtime transport or service problem | Return an infrastructure failure with sanitized error context. |
| Response decode/type mismatch | Protocol or wrapper incompatibility | Return malformed-response; never infer success. |
| Stage timeout | No response within the bounded stage budget | Keep the method future for bounded late-reply recovery, close the returned or token-derived Request path, then verify/record cleanup outcome. |
| Close/cleanup failure | The probe cannot prove that the interaction ended | Return cleanup failure as a distinct result and do not claim a clean pass. |
| No graphical context or user bus | Minimum runtime context unavailable | Preserve the existing runtime-context exit semantics; do not open a dialog. |

There is no fallback from an active probe to an implicit passive check, shell
tool, arbitrary backend or automatic fix. A passive snapshot may be collected
separately for context, but its findings cannot be substituted for a failed
active lifecycle result.

The standalone machine-readable `ProbeResult` shape is defined in
[`probe-result-schema.md`](probe-result-schema.md) and implemented in
`src/model/probe.rs`. The first FileChooser command now uses that contract:

- `portaldoctor probe filechooser` is the only active command and is never part
  of the default passive path;
- the `Request::Response` match is installed before `OpenFile` is sent;
- portal introspection, request creation, late-reply recovery, response wait and
  `Request.Close` each have bounded stages;
- success, cancellation, timeout, unavailable/unsupported, malformed response
  and infrastructure failure map to v1 statuses without raw error/URI output;
- a returned request path is closed after response, cancellation or timeout;
  a token-derived path is used when the method reply is late, and ambiguous
  cleanup remains explicitly `unverified`;
- `--json` emits only the standalone result on `stdout`; the dialog/privacy
  warning is on `stderr`.

The active shell mapping is `0` only for a successful operation with verified
cleanup and `1` for every other completed probe result. This does not change
the published v0.2.1 passive contract. The v1 validation also keeps
ScreenCast-only lifecycle stages and cleanup resources out of FileChooser and
Screenshot results.

## Required implementation checks before using ASHPD directly

Before any ASHPD helper is used in a production probe, the implementation must
prove with tests or a controlled fake that:

1. the request handle is observable before waiting for the response,
2. the response subscription cannot race the initial method call,
3. timeout and cancellation call `Request.Close` within the bounded cleanup
   budget, including a method-reply timeout where the returned handle is not
   yet available,
4. ScreenCast sessions and returned file descriptors are always closed, and
5. dropping the async task does not silently claim cleanup; any unresolved
   transport race is bounded and emitted as `unverified`.

The FileChooser slice now satisfies these checks with the controlled portal in
[`scripts/validate-filechooser-fake.py`](../scripts/validate-filechooser-fake.py)
and with both cancellation and successful selection in the supported real
Ubuntu/GNOME/Wayland session. This evidence applies only to FileChooser; it
does not pre-approve Screenshot or ScreenCast.

If a helper fails any of these checks, the adapter uses direct `zbus` for that
stage while retaining ASHPD/specification-compatible types and semantics where
useful.

## Sources reviewed

- [ASHPD repository](https://github.com/bilelmoussaoui/ashpd)
- [ASHPD 0.13.13 API documentation](https://docs.rs/ashpd/0.13.13/)
- [ASHPD 0.13.13 feature metadata](https://docs.rs/crate/ashpd/0.13.13/features)
- [ASHPD Request source](https://docs.rs/ashpd/0.13.13/src/ashpd/desktop/request.rs.html)
- [XDG portal request lifecycle](https://flatpak.github.io/xdg-desktop-portal/docs/requests.html)
- [XDG Request interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Request.html)
- [XDG FileChooser interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html)
- [XDG ScreenCast interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
