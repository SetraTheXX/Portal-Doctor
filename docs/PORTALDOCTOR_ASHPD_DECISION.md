# ASHPD Integration Decision for v0.3.0

**Status:** Accepted for Phase 8 implementation and release gating
**Decision date:** 2026-09-05
**Last verified:** 2026-09-08
**Scope:** active FileChooser, Screenshot and ScreenCast probes
**Implementation status:** bounded FileChooser and Screenshot lifecycles are
implemented and audited on development `main`; Screenshot remains unreleased
pending negotiated-path real-session validation. ScreenCast has complete,
aggregate-controlled-audited internal `CreateSession`, `SelectSources`,
`Start`, `StreamsReturned` and `OpenPipeWireRemote` slices, but its real
success/cancellation gate is currently **BLOCKED** because the live frontend
reports `AvailableSourceTypes=0` and lacks Window bit `2`; the public command
and v0.3.0 release approval remain pending

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
- call `Request.Close` only when timeout or explicit cancellation occurs
  before a matching `Response`,
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

A matching `Response` signal is terminal for the Request object, regardless of
whether its status is success, user cancellation, portal failure or its body is
malformed. After that signal the adapter must not call `Request.Close`; the
Request portion of cleanup is therefore `not_required`. The aggregate
`ProbeResult.cleanup` may still report an independently owned ScreenCast
Session or PipeWire resource. In particular, a `CreateSession` success with a
missing, wrong-type, malformed or foreign Session handle triggers only
expected-path Session cleanup and reports `completed`, `failed/session` or
`unverified/session` rather than silently claiming `not_required`.
`Request.Close` is reserved for client cancellation, timeout, late/unanswered
method replies and other paths where no `Response` was received. A failed or
unverified close remains visible on the independent cleanup axis.

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

### Handle-token entropy boundary

The `handle_token` is internal D-Bus request metadata, not user-facing output.
PortalDoctor obtains 128 bits from the operating system's entropy source and
adds a process-local sequence only as a collision supplement. If the entropy
source fails, token creation returns an infrastructure failure before
`OpenFile`; there is no PID, timestamp or other predictable fallback. This
keeps the XDG unique/not-guessable requirement fail-closed instead of trading
request correlation for a guessable cleanup path.

## Planned dependency and runtime boundary

The first FileChooser slice intentionally does not add ASHPD. It enables the
Tokio runtime feature on the existing `zbus` line and adds the small
`futures-lite`/Tokio runtime boundary required to own the response stream and
cleanup calls. The passive blocking collectors continue to use their existing
path. An ASHPD dependency remains deferred until a later probe can demonstrate
that its public API preserves the same lifecycle observability.

The FileChooser token path adds the small `getrandom` OS-entropy dependency;
it is used only by the explicit active probe and is not touched by the
passive diagnostic path.

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
- a returned request path is closed only after cancellation/timeout or another
  no-response path; a terminal `Response` produces
  `cleanup.status: not_required` and never triggers `Request.Close`; a
  token-derived path is used when the method reply is late, and ambiguous
  cleanup remains explicitly `unverified`;
- `handle_token` generation uses mandatory OS entropy; an entropy failure is a
  pre-request `infrastructure_failure`, never a predictable fallback;
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
   budget only when no `Response` was received, including a method-reply
   timeout where the returned handle is not yet available; response success,
   cancellation, failure and malformed-response paths must assert zero close
   calls,
4. ScreenCast sessions and returned file descriptors are always closed, and
5. dropping the async task does not silently claim cleanup; any unresolved
   transport race is bounded and emitted as `unverified`.

The FileChooser slice now satisfies these checks with the controlled portal in
[`scripts/validate-filechooser-fake.py`](../scripts/validate-filechooser-fake.py)
and its permanent CI wrapper
[`scripts/validate-filechooser-fake-ci.sh`](../scripts/validate-filechooser-fake-ci.sh).
The wrapper asserts cleanup calls as well as result shape, and unit tests inject
an entropy failure to verify the fail-closed token boundary. Both cancellation
and successful selection also passed in the supported real Ubuntu/GNOME/Wayland
session. This evidence applies only to FileChooser; it does not pre-approve
Screenshot or ScreenCast.

## Screenshot implementation checkpoint

The bounded Screenshot probe has its implementation and release boundary in
[`PORTALDOCTOR_SCREENSHOT_DECISION.md`](PORTALDOCTOR_SCREENSHOT_DECISION.md).
It reuses the proven FileChooser request, token, timeout and `Request.Close`
semantics, but it is not equivalent in privacy: a successful Screenshot portal
call may create an image and expose it through a URI/Documents portal entry.
PortalDoctor must never read, retain, print, delete or claim cleanup of that
artifact. The implementation now negotiates either the version-3 Window target
(requiring `AvailableTargets` bit `2`) or a narrowly scoped GNOME version-2
interactive compatibility path. The v2 path is enabled only after Wayland,
GNOME route/descriptor and live backend-name evidence agrees; it sends
`interactive=true` without `target` and makes no Window-only claim. Controlled
fake success/cancellation, request/response timeout, malformed-response,
transport, capability and cleanup gates pass for both capability shapes.
The current Ubuntu 26.04 portal advertises Screenshot version 2 without
`AvailableTargets`, so real-session proof must validate the negotiated v2 UI
path. The forced cancellation attempt returned a clean PortalDoctor
`user_cancelled` result but was followed by a GNOME backend `SIGSEGV`; this is
recorded as an external provider blocker, not release approval.

This Screenshot release blocker continues to block v0.3.0 release approval,
but it does not freeze Phase 8 development sequencing. The same broken
provider version must not be retried for another real Screenshot E2E. The v3
Window-only implementation remains intact, the v2 compatibility path remains
unreleased, and a separate bounded ScreenCast design/implementation slice may
proceed without being counted as Screenshot evidence. Its design boundary is
[`PORTALDOCTOR_SCREENCAST_DECISION.md`](PORTALDOCTOR_SCREENCAST_DECISION.md).

## ScreenCast design and bounded lifecycle checkpoints

No public ScreenCast command is started by this decision. The
accepted design boundary is the five-stage lifecycle, and all five internal
bounded adapters plus the aggregate controlled gate are complete. The real
success/cancellation gate is currently **BLOCKED** before UI execution because
the live public frontend reports `AvailableSourceTypes=0`, so Window bit `2`
is absent:

```text
CreateSession -> SelectSources -> Start -> StreamsReturned -> OpenPipeWireRemote
```

The canonical result stages are the existing v1 values
`create_session`, `select_sources`, `start`, `streams_returned` and
`open_pipe_wire_remote`. `StreamsReturned` is the typed interpretation of the
`Start` Response, while `OpenPipeWireRemote` is the subsequent portal method
that returns a Unix FD. No second result schema or generic `request`/`response`
stage is introduced.

The PortalDoctor-owned adapter owns three resource classes independently:

- each Request object is closed only on a no-response cancellation, timeout or
  ambiguous method-reply path; a terminal Response, including cancellation,
  failure or malformed data, forbids a later `Request.Close`;
- a successfully returned ScreenCast Session is closed exactly once with
  `Session.Close`, including later cancellation and failure paths; and
- a terminal `CreateSession` success whose Session payload is not trustworthy
  attempts `Session.Close` only on the expected token-derived path; foreign
  paths are never closed and uncertain ownership is `unverified/session`; and
- a returned PipeWire remote FD is owned by the probe, closed before Session
  cleanup and never read, transferred, persisted or used to consume media.

Cleanup is reverse acquisition order (PipeWire remote FD, Session, then any
still-unanswered Request). Each close has its own bounded budget. An unknown
or ambiguous owner is reported as `failed` or `unverified` through the
existing `CleanupResource::{Request,Session,PipeWireRemote}` values; no
cleanup is silently assumed from dropping a future or closing a different
resource.

The design is fail-closed against the existing `ProbeResult` v1 contract:
full lifecycle success requires all five lifecycle boundaries and verified
cleanup; the internal Start slice may report only a stage-local success and
must not claim capture readiness;
user-cancelled is reserved for explicit user/portal cancellation;
timed-out identifies the winning bounded stage; malformed response is used
for invalid terminal payloads; unavailable/unsupported stop before an owned
resource exists; and transport, permission, portal rejection or remote-FD
failures map to infrastructure failure. A cleanup failure moves the result to
the existing `cleanup` stage and remains visible on the independent cleanup
axis. No `provider_crashed` or `externally_blocked` enum is added to v1.

The internal `CreateSession`, `SelectSources`, `Start`, `StreamsReturned` and
`OpenPipeWireRemote` adapters are not wired to a public command. Their
per-slice and aggregate controlled matrices cover the verified
Request/Session ownership and cleanup boundary. Start uses the same owned
Session, a fresh entropy-backed `handle_token`, exact `parent_window=""` and
does not decode, validate, serialize or log the `streams` result map. The
SelectSources slice is bounded to the advertised Window bit (`types=2`),
`multiple=false`, no persistence/restore options, terminal Response zero
Request.Close and exactly-once Session.Close. The StreamsReturned adapter
accepts only one typed XDG Window stream (`a(ua{sv})`), keeps node IDs and
properties opaque and maps malformed or policy-inconsistent terminal payloads
to `malformed_response` without a second Request.Close. OpenPipeWireRemote is
the direct-FD boundary: it sends empty options, creates no Request, owns a
typed `OwnedFd`, closes it before Session.Close, and reports unresolved late
FD ownership as `unverified/pipe_wire_remote` without connecting to PipeWire.
The aggregate gate
[`validate-screencast-aggregate-ci.sh`](../scripts/validate-screencast-aggregate-ci.sh)
composes the five stages and verifies representative cross-stage failures,
cleanup ordering/aggregation, terminal-response zero-close behavior, privacy,
exact call counts and no-open-resource state. It does not constitute real
GNOME/Wayland evidence or release approval.
The complete design, privacy boundary and acceptance matrix are maintained in
[`PORTALDOCTOR_SCREENCAST_DECISION.md`](PORTALDOCTOR_SCREENCAST_DECISION.md).
The external capability gate must not be retried in the same provider state.
Re-evaluate only when `AvailableSourceTypes & 2 != 0`, the selected
provider/frontend is stable and healthy, and a supported disposable session is
ready for exactly one real success plus one portal-native cancellation E2E.
Both real runs and the final release/regression gates must pass before a public
command or v0.3.0 approval is considered. Until then, this is not a
ScreenCast success or capture-readiness claim.

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
