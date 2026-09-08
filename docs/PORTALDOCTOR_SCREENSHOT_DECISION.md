# Screenshot Probe Design Boundary for v0.3.0

**Status:** Implemented on development `main`; unreleased and not approved
for the v0.3.0 release until negotiated-path real-session success/cancellation
validation passes
**Scope:** `org.freedesktop.portal.Screenshot.Screenshot` only
**Out of scope:** `PickColor`, ScreenCast, image inspection, artifact deletion,
desktop expansion and passive-path changes

This document is the implementation and release boundary for the Phase 8
Screenshot probe. The explicit command is available on development `main`
and does not change the published v0.2.1 passive behavior. It remains
unreleased until every gate in this document passes.

## Decision

Screenshot will reuse the PortalDoctor-owned D-Bus request lifecycle already
proven by FileChooser:

- subscribe to `Request::Response` before calling the portal method,
- provide a per-request unique, not-guessable `handle_token`,
- derive and validate the expected request path while accepting older portals
  that return a different valid path,
- keep the method future alive through the bounded request-recovery window,
- classify the response without retaining its sensitive values, and
- abort only a request that has not emitted `Response`, using the existing
  bounded `Request.Close` policy.

The reusable boundary is lifecycle behavior and invariants, not a copy of the
FileChooser implementation. The shared request/token, response-wait and
Request.Close mechanics now live in `src/probes/portal.rs`; the refactor
preserves the already-tested FileChooser behavior and does not alter the
passive path.

## Portal contract and capability policy

The XDG Screenshot interface exposes a `version` property and, in version 3,
an `AvailableTargets` bitmask. Its `Screenshot` method returns a Request object;
the eventual `Response` result contains a `uri` string on success. The
implementation negotiates one of two explicit capability paths:

1. Preflight `org.freedesktop.portal.Screenshot` introspection and `version`
   through the portal frontend.
2. For version 3 or newer, require `AvailableTargets` and the `Window` target
   bit (`2`). Call `Screenshot` with `parent_window: ""`, `handle_token`,
   `modal: true`, `interactive: true` and `target: 2`.
3. For version 2, enable compatibility only when independent evidence agrees:
   the session is GNOME on Wayland, effective Screenshot routing selects the
   `gnome` descriptor that implements Screenshot and names the canonical GNOME
   backend, and that backend D-Bus name has a live owner. Call the public
   portal with `parent_window: ""`, `handle_token`, `modal: true` and
   `interactive: true`; never send `target`. The GNOME portal UI chooses the
   target and may include a screen, window or area.
4. Treat a missing/old interface, an untrusted v2 provider or an unavailable
   v3 Window target as `unsupported`/`unavailable`; do not issue a request.

The v3 path must not silently fall back to an omitted target, full `Screen`
(`1`), `Area` (`4`) or `Active Window` (`8`). The v2 path is a deliberate
compatibility exception, not a v3 downgrade: it makes no Window-only claim
and its warning discloses the portal-selected target scope. `interactive` is a
portal hint, not a guarantee that every backend will show the same dialog, so
the warning remains mandatory before the request.

`PickColor` is not a Screenshot implementation shortcut. It is a separate
portal operation with a different result contract and remains out of scope.

## Bounded lifecycle

The lifecycle must use the existing central active-probe budgets:

1. Verify a graphical session and user D-Bus before opening any dialog.
2. Inspect the Screenshot interface and negotiate either the trusted GNOME v2
   interactive path or the v3 Window capability with a bounded setup timeout.
3. Print a mode-specific warning on `stderr`: v3 may ask the user to choose a
   window; v2 says the GNOME portal UI may choose a screen, window or area.
   Both warnings disclose that a screenshot artifact may be created.
4. Install the response match, generate the token, derive the expected path,
   and call `Screenshot`.
5. Bound the method reply, retain the future through the existing recovery
   grace period, then bound the response wait.
6. Decode only the response code and the presence/type of `uri`; immediately
   discard the URI value.
7. If no matching `Response` was received, call `Request.Close` on the
   returned or predicted request path and map its result to the independent
   `ProbeResult.cleanup` axis. Never call `Request.Close` after `Response`,
   including a malformed response body.

Cancellation and timeout must follow the FileChooser policy. A known late
handle is closed immediately; an unanswered request gets one bounded attempt
on the token-derived path. A matching `Response` (success, portal
cancellation, portal failure or malformed body) ends the Request lifecycle and
therefore maps to `cleanup.status: not_required`. An unknown object before a
method reply is `cleanup.status: unverified`, never a clean success. A close
error is `cleanup.status: failed`. There is no passive fallback, shell
fallback or automatic fix.

## ProbeResult mapping

Screenshot uses only the v1 stages `prepare`, `request`, `response`, `cleanup`
and `complete`, and only `CleanupResource::Request`. The public result never
contains the URI.

| Portal condition | `ProbeResult.status` | Cleanup rule |
| --- | --- | --- |
| Response code `0` with a D-Bus string `uri` | `success` | `not_required`: the matching `Response` ended the request. |
| Response code `1` | `user_cancelled` | `not_required`: the matching `Response` ended the request. |
| Response code `2` or a portal rejection | `infrastructure_failure` | `not_required` when the rejection arrives as `Response`; close only if no `Response` arrives. |
| Missing/wrong-type `uri` on response code `0` | `malformed_response` | `not_required`: a malformed matching `Response` is still terminal. |
| Setup/service unavailable | `unavailable` | `not_required` when no request was created. |
| Version/target/interface not supported | `unsupported` | `not_required` when no request was created. |
| Any bounded stage deadline | `timed_out` | Recover and close the returned or predicted request; unresolved transport is `unverified`. |

`cleanup.status: not_required` means that a matching `Response` already ended
the Request lifecycle; it does not mean that no portal-side artifact exists.
`cleanup.status: completed` means that PortalDoctor issued `Request.Close` for
a no-response cancellation/timeout or recovery path and its result was
verified. It does not mean that a screenshot artifact was deleted. The portal
may have already made the image accessible through the Documents portal before
the response was emitted.

## Privacy and side-effect contract

Screenshot is not a passive/read-only operation at the portal data boundary.
On success, the portal may capture pixels from the selected window and make an
image accessible to the application, potentially by creating a Documents
portal entry. The user-facing warning must appear before `Screenshot` is
called and must say that a screenshot may be created by the portal.

PortalDoctor may transiently inspect only the response code and D-Bus value
type needed to classify the protocol response. It must never:

- open, read, decode, hash, thumbnail, copy, upload, modify or delete the
  screenshot image;
- parse, normalize, print, log, serialize, persist or send the returned URI,
  path, filename, document identifier or any equivalent portal handle;
- include image metadata, raw D-Bus results, parent-window identifiers or raw
  portal errors in terminal output, JSON, reports, telemetry or crash context;
- retain the screenshot URI or bytes in a temporary file, cache, test artifact
  or repository asset; or
- claim that `Request.Close` removed an image that the portal may already have
  created.

The machine-readable result contains only the typed lifecycle facts already
defined by `ProbeResult` v1. A successful Screenshot result therefore proves a
request/response lifecycle and verified Request cleanup, not that no image
exists outside PortalDoctor's ownership.

## Compatibility and release boundary

The first real validation matrix remains Ubuntu 26.04 + GNOME + Wayland + a
systemd user session. No KDE, wlroots/Sway, Hyprland or Niri support claim is
created. The probe must call `org.freedesktop.portal.Desktop` and must never
bypass portal routing by invoking a backend directly.

The controlled implementation gates now pass. Before the v0.3.0 release can
be called complete, the repository must have:

- [x] a controlled fake portal covering success, user cancellation, malformed
  response, response timeout, request-stage timeout, late reply, transport
  failure, unsupported target/version, untrusted v2 and Request.Close failure
  for both v2 and v3 capability shapes;
- [x] assertions for exact v2/v3 options, response-terminal no-close behavior,
  no-response cleanup calls and open-request state,
  exit code, `ProbeResult` shape and absence of URI/path/image data from stdout
  and stderr;
- [x] unit tests for capability-bitmask parsing, target policy, URI type checking,
  response mapping and v1 serialization without serializing the URI;
- [ ] real supported-session cancellation and successful validation of the
  negotiated v2 or v3 path using a disposable test context, with no image
  opened or inspected by PortalDoctor; and
- [x] fmt, strict locked Clippy, locked tests, release build, locked package,
  clean-root install, passive regression, audit/docs and GitHub Actions gates.

The current Ubuntu 26.04 session exposes Screenshot version 2 and no
`AvailableTargets` property. The implementation can select the v2 path only
after the GNOME route/provider evidence is independently verified; it does not
claim Window-only behavior for that path. The local controlled evidence is
complete, but it does not substitute for real-session success/cancellation.

The real GNOME success/cancellation gate remains a v0.3.0 release approval
blocker because the observed provider hangs and crashes. It is not a reason to
retry the same provider version indefinitely: re-evaluate only after the
provider/frontend changes or a disposable fixed provider is available. This
release gate does not freeze Phase 8 development sequencing. ScreenCast is a
separate design-only bounded slice documented in
[`PORTALDOCTOR_SCREENCAST_DECISION.md`](PORTALDOCTOR_SCREENCAST_DECISION.md);
that work cannot close this Screenshot gate, and this document does not
approve ScreenCast implementation or release.

The 2026-09-06 provider audit identified the reason this gate cannot currently
be closed on the supported host: Ubuntu supplies `xdg-desktop-portal 1.21.1`
and `xdg-desktop-portal-gnome 50.0`, the XDG frontend records Screenshot target
selection beginning with `1.21.2`, and the current GNOME backend still exports
Screenshot implementation version `2`. This does not authorize a
backend-direct call, backend fork, wlroots session or controlled fake to count
as GNOME evidence. The gate remains open until a supported GNOME session
proves the path it actually negotiates.

During the 2026-09-06 forced real-session cancellation attempt, PortalDoctor
returned `user_cancelled` with `cleanup.status: completed`, but the GNOME
backend subsequently logged `InteractiveScreenshot didn't return a file` and
exited with `SIGSEGV`. That path had no matching `Response`, so issuing
`Request.Close` was the correct XDG abort action; the new controlled harness
also fails if a close is attempted after a terminal `Response`. The provider
crash therefore remains a provider-side failure while its interactive
Screenshot operation unwinds after client cancellation, not evidence that
PortalDoctor should close a completed request. No real GNOME E2E is rerun as
part of the offline hardening task, and the release gate remains open.

On 2026-09-07 the same provider was healthy during a read-only preflight and
then failed during one success-path attempt: the GNOME UI did not complete a
selection, PortalDoctor reached its bounded response timeout and returned
`timed_out / response / cleanup.status: completed / exit 1`, and the backend
again logged `InteractiveScreenshot didn't return a file` before
`SIGSEGV`/core-dump. No URI, path, image bytes or metadata was emitted. This
is not Screenshot success or portal-native cancellation evidence; the second
real E2E was not run. The v2 compatibility path is therefore not
release-ready, while the v3 Window-only implementation remains preserved.

### Provider-crash reporting boundary

The v1 `ProbeResult` contract intentionally has no `provider_crashed` or
`externally_blocked` enum. The timeout result above is therefore kept as
`timed_out` rather than being relabeled from a single timeout observation.
Adding a new status would change the v1 enum semantics and require a schema
version decision. If future work adds provider diagnostics, it should first
perform a bounded, independent post-abort owner/service health check, emit
only a sanitized user-facing note when that evidence is positive, and keep
the v1 JSON unchanged. A dedicated machine-readable blocker field belongs in
a later schema version. Do not treat a provider crash as a PortalDoctor
success, cancellation or cleanup failure, and do not silently retry it.

The v0.3.0 release must not advertise Screenshot as ready until the warning,
side-effect boundary, disposable validation context and artifact ownership
language are reviewed together. The v2 path remains unreleased and the v3
Window-only policy remains unchanged. Any ScreenCast work must stay in its
separate bounded design/implementation sequence and must not be counted as
Screenshot release evidence.

## Sources

- [XDG Screenshot interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html)
- [XDG portal request lifecycle](https://flatpak.github.io/xdg-desktop-portal/docs/requests.html)
- [XDG Request interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Request.html)
- [`ProbeResult` v1](probe-result-schema.md)
- [ASHPD integration decision](PORTALDOCTOR_ASHPD_DECISION.md)
