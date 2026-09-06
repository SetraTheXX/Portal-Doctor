# Screenshot Probe Design Boundary for v0.3.0

**Status:** Implemented on development `main`; unreleased and not approved
for the v0.3.0 release until a v3-capable real-session success/cancellation
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
- close the known or token-derived request with the existing bounded
  `Request.Close` policy.

The reusable boundary is lifecycle behavior and invariants, not a copy of the
FileChooser implementation. The shared request/token, response-wait and
Request.Close mechanics now live in `src/probes/portal.rs`; the refactor
preserves the already-tested FileChooser behavior and does not alter the
passive path.

## Portal contract and first target policy

The XDG Screenshot interface exposes a `version` property and, in version 3,
an `AvailableTargets` bitmask. Its `Screenshot` method returns a Request object;
the eventual `Response` result contains a `uri` string on success. The first
slice deliberately narrows that contract:

1. Preflight `org.freedesktop.portal.Screenshot` introspection, `version` and
   `AvailableTargets` through the portal frontend.
2. Require interface version 3 or newer and the `Window` target bit (`2`).
3. Call `Screenshot` with `parent_window: ""` and these options:
   `handle_token`, `modal: true`, `interactive: true`, and `target: 2`.
4. Treat a missing/old interface or an unavailable Window target as
   `unsupported`/`unavailable`; do not issue a request.

The first slice must not silently fall back to an omitted target, full `Screen`
(`1`), `Area` (`4`) or `Active Window` (`8`). This keeps the initial privacy
and validation claim narrow and prevents a capability downgrade from silently
capturing the whole display. `interactive` is a portal hint, not a guarantee
that every backend will show the same dialog, so the warning remains mandatory
before the request.

`PickColor` is not a Screenshot implementation shortcut. It is a separate
portal operation with a different result contract and remains out of scope.

## Bounded lifecycle

The lifecycle must use the existing central active-probe budgets:

1. Verify a graphical session and user D-Bus before opening any dialog.
2. Inspect the Screenshot interface and target capability with a bounded setup
   timeout.
3. Print a clear warning on `stderr` explaining that the portal may ask the
   user to choose a window and may create a screenshot artifact.
4. Install the response match, generate the token, derive the expected path,
   and call `Screenshot`.
5. Bound the method reply, retain the future through the existing recovery
   grace period, then bound the response wait.
6. Decode only the response code and the presence/type of `uri`; immediately
   discard the URI value.
7. Call `Request.Close` on the returned or predicted request path and map its
   result to the independent `ProbeResult.cleanup` axis.

Cancellation and timeout must follow the FileChooser policy. A known late
handle is closed immediately; an unanswered request gets one bounded attempt
on the token-derived path. An unknown object before a method reply is
`cleanup.status: unverified`, never a clean success. A close error is
`cleanup.status: failed`. There is no passive fallback, shell fallback or
automatic fix.

## ProbeResult mapping

Screenshot uses only the v1 stages `prepare`, `request`, `response`, `cleanup`
and `complete`, and only `CleanupResource::Request`. The public result never
contains the URI.

| Portal condition | `ProbeResult.status` | Cleanup rule |
| --- | --- | --- |
| Response code `0` with a D-Bus string `uri` | `success` | Close the request; verified close is required for a clean result. |
| Response code `1` | `user_cancelled` | Close/verify the request lifecycle. |
| Response code `2` or a portal rejection | `infrastructure_failure` | Preserve only the sanitized classification; close if a request may exist. |
| Missing/wrong-type `uri` on response code `0` | `malformed_response` | Close the request. |
| Setup/service unavailable | `unavailable` | `not_required` when no request was created. |
| Version/target/interface not supported | `unsupported` | `not_required` when no request was created. |
| Any bounded stage deadline | `timed_out` | Recover and close the returned or predicted request; unresolved transport is `unverified`. |

`cleanup.status: completed` means only that the portal Request lifecycle was
closed or was verified gone. It does not mean that a screenshot artifact was
deleted. The portal may have already made the image accessible through the
Documents portal before the response was emitted.

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
  failure, unsupported target/version and Request.Close failure;
- [x] assertions for cleanup calls, exit code, `ProbeResult` shape and absence of
  URI/path/image data from stdout and stderr;
- [x] unit tests for capability-bitmask parsing, target policy, URI type checking,
  response mapping and v1 serialization without serializing the URI;
- [ ] real supported-session cancellation and successful Window-target validation
  using a disposable test context, with no image opened or inspected by
  PortalDoctor; and
- [ ] fmt, strict locked Clippy, locked tests, release build, locked package,
  clean-root install, passive regression, audit/docs and GitHub Actions gates.

The current Ubuntu 26.04 session exposes Screenshot version 2 and no
`AvailableTargets` property, so the real-session item is intentionally still
open; the binary returns `unsupported` before issuing a request. The final
quality-gate item is marked only after the release build, package/install,
passive regression and remote CI run are rechecked for this implementation.

The v0.3.0 release must not advertise Screenshot as ready until the warning,
side-effect boundary, disposable validation context and artifact ownership
language are reviewed together. Screenshot implementation must not begin in
the same change as ScreenCast or desktop compatibility expansion.

## Sources

- [XDG Screenshot interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html)
- [XDG portal request lifecycle](https://flatpak.github.io/xdg-desktop-portal/docs/requests.html)
- [XDG Request interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Request.html)
- [`ProbeResult` v1](probe-result-schema.md)
- [ASHPD integration decision](PORTALDOCTOR_ASHPD_DECISION.md)
