# ScreenCast Probe Design Checkpoint for v0.3.0

**Status:** Bounded internal `CreateSession`, `SelectSources`, `Start`,
`StreamsReturned` and `OpenPipeWireRemote` implementation complete and
aggregate-controlled-audited; real-session validation is currently **BLOCKED**
by an external provider capability; no public command or release approval
**Decision date:** 2026-09-07
**Last verified:** 2026-09-08
**Scope:** `org.freedesktop.portal.ScreenCast` lifecycle design through the
bounded direct-FD `OpenPipeWireRemote` boundary, including explicit FD and
Session ownership/cleanup
**Out of scope:** PipeWire handshake/media access, a public command, real
ScreenCast E2E/release approval, Screenshot real E2E reruns, backend-direct
calls, desktop expansion, automatic remediation and changes to the passive
v0.2.1 contract

This document is the design boundary for the current and next Phase 8
active-probe slices. It does not authorize a production ScreenCast command by
itself.

## Sequencing and release boundary

The Phase 8 status is intentionally split into implementation progress and
release approval:

| Area | Current state | Consequence |
| --- | --- | --- |
| Screenshot controlled implementation | Complete and controlled/fake-tested | Keep the v3 Window-only path; keep v2 compatibility unreleased. |
| Screenshot controlled implementation | Complete and controlled/fake-tested | Keep the v3 Window-only path; keep v2 compatibility unreleased. |
| Screenshot real GNOME success/cancellation | **BLOCKED** by the external GNOME provider hang/crash | The v0.3.0 release approval remains blocked. Do not retry the same provider state. |
| ScreenCast controlled/internal implementation | Complete: all five bounded slices and the aggregate controlled gate pass | Keep this implementation complete; it has no public command and no media/PipeWire handshake. |
| ScreenCast real success/cancellation | **BLOCKED**: the live frontend reports `version=5` but `AvailableSourceTypes=0`, so Window bit `2` is absent | Do not open the real UI or expose the command. Re-evaluate only after the exact provider-capability trigger below. |
| Phase 8 development sequencing | Independent bounded work may continue | This does not close either real-session gate, authorize a public command or approve v0.3.0. |

Both external real-session blockers are release gates, not reasons to reopen
completed internal implementation or spin Phase 8 indefinitely. Independent
bounded development and a next roadmap development phase may proceed only when
the work does not depend on either real-session gate. This sequencing allowance
does not close Phase 8, expose a public ScreenCast command, claim active-probe
release readiness or approve v0.3.0. A fake portal, a backend-direct call or a
different desktop backend cannot close either required GNOME release gate.

The supported validation claim remains Ubuntu 26.04 + GNOME + Wayland + a
systemd user session through `org.freedesktop.portal.Desktop`. No KDE,
wlroots/Sway, Hyprland or Niri claim is created here.

## External provider capability checkpoint

The current read-only preflight is sufficient to classify the ScreenCast gate
as **A — genuine provider capability absence** at the public frontend boundary:

- the live ScreenCast interface reports `version=5`;
- `AvailableSourceTypes=0`, so the required Window bit (`2`) is not advertised;
- routing selects the GNOME provider and its descriptor advertises the
  ScreenCast interface;
- the Ubuntu/GNOME/Wayland session, portal services, PipeWire and WirePlumber
  are running, and the installed frontend/provider versions are known; and
- no ScreenCast registration/initialization error was found in the sanitized
  read-only service evidence. The separate GNOME Screenshot hang/crash remains
  an independent provider blocker and is not treated as proof of a ScreenCast
  initialization failure.

This is an external capability gate, not a PortalDoctor `ProbeResult` bug. The
Window-only policy remains fail-closed: `AvailableSourceTypes=0` cannot be
treated as support, and `MONITOR` or another source fallback is not allowed.
No real ScreenCast E2E is evidence while this capability is absent, and the
same provider/package/version state must not be retried.

Re-evaluate this gate only when all of the following are true:

1. the public frontend reports `AvailableSourceTypes & 2 != 0`;
2. the selected provider and frontend remain stable and healthy without a
   provider hang, crash or capability-registration failure; and
3. a supported disposable Ubuntu/GNOME/Wayland session is available for one
   real success and one portal-native cancellation E2E.

Only after that trigger may the two real runs be performed. A clean pair is
required before any public command or v0.3.0 release decision.

## Lifecycle decision

The probe owns one bounded state machine with these externally visible stages:

```text
CreateSession -> SelectSources -> Start -> StreamsReturned -> OpenPipeWireRemote
```

The canonical `ProbeStage` JSON values are `create_session`,
`select_sources`, `start`, `streams_returned` and `open_pipe_wire_remote`.
The `StreamsReturned` stage is the successful decoding boundary of the
`Start` response; it is not a second portal method. `OpenPipeWireRemote` is a
separate portal method that returns a Unix file descriptor rather than a
Request object in the XDG contract.

Each Request-producing method follows the already audited FileChooser and
Screenshot adapter rules:

1. Install the `Request::Response` match before sending the method call.
2. Keep the method future alive through a bounded recovery window if its reply
   is late.
3. Treat the first matching `Response` as terminal for that Request, including
   success, user cancellation, portal rejection and malformed response data.
4. Never call `Request.Close` after a matching `Response`.
5. On client cancellation, timeout, an unanswered method reply or another
   no-response path, call `Request.Close` once within the cleanup budget and
   report the verified, failed or unverified outcome independently.

`CreateSession` uses mandatory OS entropy for both the Request `handle_token`
and the Session `session_handle_token`; later Request-producing stages use a
fresh `handle_token`. Entropy failure is an early `infrastructure_failure`,
with no PID, timestamp or other predictable fallback.

The shared PortalDoctor-owned request adapter remains the lifecycle authority.
ASHPD types and helpers may be used only where they preserve the request
handle, response race and cleanup observability recorded in
[`PORTALDOCTOR_ASHPD_DECISION.md`](PORTALDOCTOR_ASHPD_DECISION.md).

## Ownership and cleanup

There are three independent resource classes:

| Resource | Acquired at | Owner | Release rule |
| --- | --- | --- | --- |
| Request object | Every Request-producing method | The current stage adapter | `Request.Close` only before a matching `Response`; terminal responses require no Request close. |
| ScreenCast Session | Successful `CreateSession` response | The probe state machine | `Session.Close` exactly once after all later stage work and the PipeWire remote FD are released, including cancellation and failure. |
| PipeWire remote FD | Successful `OpenPipeWireRemote` reply | The probe process via an owned FD | Close immediately after the bounded readiness check; never hand it to another process, store it, or read media from it. |

Once no Request is pending, cleanup is reverse acquisition order:

```text
PipeWire remote FD -> Session.Close
```

If a stage is still waiting and has no terminal Response, that current Request
is aborted first within the stage cleanup budget; any already-owned PipeWire FD
is then closed, followed by `Session.Close`. This ordering is a state-machine
rule, not permission to close a Request after its `Response`. If the method
reply is ambiguous, the adapter performs bounded late-reply recovery. A late
Session handle is closed as soon as it is recovered; if ownership cannot be
proven, the result is `cleanup.status: unverified`.

`Session.Close` is not a substitute for `Request.Close`, and closing the
PipeWire FD is not evidence that the Session was closed. Each owned resource
must be tracked separately and the aggregate `ProbeResult.cleanup` must expose
any failed or unverified resource through the existing v1 resource values:
`request`, `session` and `pipe_wire_remote`.

### CreateSession malformed-success ownership boundary

A terminal `CreateSession` `Response` with `code=0` does not prove that no
Session was created merely because `session_handle` is missing, has the wrong
type, is malformed or names a foreign/untrusted path. In those cases the
adapter performs one bounded best-effort `Session.Close` only on the
sender-and-token-derived expected Session path. It never closes the path
returned by an untrusted payload. A successful close produces
`malformed_response` with verified `cleanup: completed`; an explicit close
error produces `cleanup: failed` with `session`; and an absent, already-gone or
otherwise ambiguous expected object produces `cleanup: unverified` with
`session`. In every case the terminal Response still forbids
`Request.Close`, so the request close count remains zero. The operation status
does not become `success` merely because the best-effort Session cleanup
succeeds.

The same expected-path-only rule applies when the Response body cannot be
decoded at all: the result remains `malformed_response` and possible Session
ownership is never silently reported as `not_required`.

### SelectSources bounded slice

The controlled SelectSources slice starts only after a valid, sender-bound
Session handle has been acquired. It reads the frontend's
`AvailableSourceTypes` property and proceeds only when the Window bit (`2`)
is advertised. A missing property, unavailable property or a value without
that bit is fail-closed `unsupported`; a Session acquired by CreateSession is
still closed exactly once on that path. No Screen, Area or Active Window
fallback is permitted.

The request options are intentionally exact:

```text
handle_token = fresh OS-entropy string
types        = 2 (Window)
multiple     = false
```

No persistence or restore option is sent, and no source information is
retained. The response match is already installed before CreateSession and is
reused for SelectSources. A terminal SelectSources `Response`—success, portal
cancellation, rejection or malformed response—produces zero
`Request.Close` calls. A request-stage timeout, response timeout, client
cancellation, late-reply recovery or transport path performs only the
bounded Request cleanup justified by the XDG lifecycle, then performs the
independent exactly-once Session cleanup. If both cleanup operations fail, the
v1 cleanup resources are aggregated as `request` and `session` rather than
being overwritten by the later cleanup result.

This slice ends after the SelectSources terminal boundary and Session cleanup.
It does not select multiple sources, persist/restore a source selection, call
Start, inspect streams, acquire a PipeWire FD or expose a public command.

### `Start` bounded slice

The controlled Start slice begins only after the same owned Session has
received a successful SelectSources Response. It sends that exact Session
handle, a fresh OS-entropy `handle_token`, and the explicit bounded headless
`parent_window` value `""`. The empty value is intentional: it carries no
desktop, application or window identifier and does not pretend to parent a
portal dialog to a hidden window.

Start reuses the response match installed before CreateSession, so its
Request::Response race boundary is already in place. A terminal Start
Response—success, portal cancellation, rejection or malformed signal—ends the
Start Request and produces zero `Request.Close` calls. A request-stage
timeout, response timeout, client cancellation, late method reply or other
no-response path performs one bounded Request cleanup when a safe request
object is observable. Owned Session cleanup then runs independently and
exactly once; Request and Session cleanup failures are aggregated without
discarding either resource.

The Start `results` map is deliberately not decoded, validated, serialized or
logged. Even a success payload containing synthetic or real stream metadata is
dropped at this boundary. `ProbeStage::Start` with operation status `success`
means only that the bounded Start Response succeeded; it is an internal
milestone, not capture readiness or full ScreenCast success. Stream structure
belongs exclusively to the `StreamsReturned` validation boundary.

The controlled Start gate covers success, portal cancellation/rejection,
malformed envelope/body, response and request-stage timeout, late reply,
client cancellation, transport failure, unanswered request, Request.Close and
Session.Close failures, aggregate cleanup failure, terminal-response zero
Request.Close, exactly-once Session.Close, exact arguments and raw-stream
privacy. The sanitized fake state records only counters and booleans; it never
persists the Session/Request path, token or stream payload.

### `StreamsReturned` bounded slice

`StreamsReturned` is the typed validation boundary of a successful Start
Response; it is not another portal method. The XDG ScreenCast contract defines
the `streams` result as `a(ua{sv})`: an array of `(u, a{sv})` tuples. The first
member is a node ID and the second is a property dictionary. PortalDoctor
checks only the container/tuple shape and the existing Window-only policy:

- exactly one tuple is required because `SelectSources` sent `multiple=false`;
- the node ID must be a D-Bus `u`, but its value is never read into a result,
  log, report or persistent state;
- the property dictionary must be `a{sv}`; unknown properties and their values
  remain opaque and are not interpreted;
- if `source_type` is present, it must be a variant containing `u=2` (Window);
  older providers may omit this optional property; and
- missing/wrong-type/empty/multiple/malformed tuples or a contradictory
  `source_type` are `malformed_response` and fail closed.

Because the Start Response is already terminal, none of these malformed
payloads may call `Request.Close`. The owned Session still receives exactly
one `Session.Close`; a Session cleanup failure moves the result to the existing
`cleanup` stage and preserves the `session` resource. A valid payload reaches
the internal `streams_returned` stage only; it does not decode node IDs,
connect to PipeWire, read media or claim capture readiness.

The permanent controlled gate is implemented by
[`scripts/validate-screencast-streams-returned-ci.sh`](../scripts/validate-screencast-streams-returned-ci.sh)
and the `streams-*` modes in
[`scripts/validate-screencast-select-sources.py`](../scripts/validate-screencast-select-sources.py).
The 2026-09-08 matrix covers valid/minimal/unknown-property and opaque-value
payloads, missing/wrong-type/empty/multiple/malformed tuples, contradictory
Window properties, Session cleanup failure, terminal Start zero Request.Close,
CreateSession/SelectSources zero Request.Close, exactly-once Session.Close,
closed resources and path/token/node/property/raw-payload privacy.

### `OpenPipeWireRemote` bounded slice

`OpenPipeWireRemote(session_handle, options)` is the XDG direct-FD method. It
does not create a `Request`, so this slice deliberately generates no
`handle_token`, installs no `Response` match for the method and never calls
`Request.Close`. The options value is an empty `a{sv}`; no invented token or
capture option is sent.

The reply is deserialized directly into a typed `zvariant::OwnedFd`. A valid
owned FD is released exactly once before the acquired Session is closed. The
adapter never exposes the descriptor number, transfers it to another process,
opens a PipeWire context, performs a handshake or reads media. A timeout or
client cancellation retains the direct method future for one bounded recovery
window: a late FD is adopted and closed before Session cleanup; an unresolved
reply is `unverified` for `pipe_wire_remote` and is never treated as clean.
Known terminal D-Bus errors that prove no FD was returned are mapped without a
PipeWire cleanup resource; malformed/type replies and transport ambiguity stay
fail-closed as `unverified/pipe_wire_remote`.

The controlled matrix in
[`scripts/validate-screencast-open-pipe-wire-remote-ci.sh`](../scripts/validate-screencast-open-pipe-wire-remote-ci.sh)
and the `open-*` fake modes covers valid and late owned FDs, transport,
unavailable and unsupported errors, malformed replies, direct-method timeout,
client cancellation, ambiguous ownership, FD-close failure, Session-close
failure and aggregate FD+Session failure. It asserts FD release before
`Session.Close`, exactly-once FD and Session cleanup, zero Request.Close calls
for the direct method and all prior terminal requests, no open resources and
sanitized FD/path/token/stream privacy.

This slice reaches only the internal `open_pipe_wire_remote` stage. It is not a
full ScreenCast success claim and does not make the public command available.

## Aggregate internal lifecycle audit

The five bounded slices are now exercised together by the same internal state
machine:

```text
CreateSession -> SelectSources -> Start -> StreamsReturned
              -> OpenPipeWireRemote -> FD release -> Session.Close
```

The permanent aggregate gate is
[`scripts/validate-screencast-aggregate-ci.sh`](../scripts/validate-screencast-aggregate-ci.sh).
It runs the controlled fake portal in an isolated session bus and asserts the
cross-stage ownership rules rather than relying only on the per-slice tests.
Its matrix covers full success; CreateSession, SelectSources and Start
failures; malformed `StreamsReturned`; direct-FD transport, timeout,
cancellation, late/ambiguous-reply and unresolved-ownership paths; FD-close,
Session.Close and combined cleanup failures; representative Request-stage
cancellation; terminal-response zero-close behavior; exact call counts;
FD-before-Session ordering; sanitized state/result privacy; and no open
Request, Session or FD after a clean run. The wrapper also checks that the
public `probe --help` output still does not expose `screencast`.

The 2026-09-08 aggregate audit passed all 16 cross-stage cases, including a
method-reply cancellation before the SelectSources Request path is returned.
This proves
that the five internal adapters compose safely in the controlled fake only; it
does not prove a real GNOME/Wayland provider session, PipeWire/media readiness,
or release readiness.

## Stage behavior and failure policy

| Stage | Required operation | Success boundary | Fail-closed outcomes |
| --- | --- | --- | --- |
| `CreateSession` | Call the portal frontend and decode the returned Session object path. | A valid Session handle is acquired. | Missing interface/provider: `unavailable` or `unsupported`; timeout: `timed_out`; wrong response: `malformed_response`; no implicit fallback. |
| `SelectSources` | Request explicit source selection for the supported bounded probe. | Terminal Response confirms source selection. | User cancellation: `user_cancelled`; timeout/transport/rejection: corresponding v1 failure; no Request.Close after Response. |
| `Start` | Ask the portal to start the selected source session. | Terminal Response is received. | User cancellation, timeout, rejection and malformed response remain distinct. No streams are assumed from a success code alone. |
| `StreamsReturned` | Validate only the typed stream container and required fields. | Stream metadata is structurally valid. | Wrong type, missing required fields or impossible tuples: `malformed_response`; do not print node IDs or metadata. |
| `OpenPipeWireRemote` | Request the PipeWire remote FD from the portal frontend. | A valid owned FD is returned and immediately closed after the readiness boundary. | Unsupported/unavailable/transport failure or invalid FD: `infrastructure_failure`/`unavailable`/`unsupported`; never claim a live capture. |

The operation status and cleanup status remain independent, exactly as in
`ProbeResult` v1:

- A full ScreenCast `success` is emitted only after all required stages
  succeed and every owned Session/FD cleanup is verified; a cleanup failure is
  not a clean success. The internal Start slice may report a stage-local
  `success`, but it is not a full lifecycle or capture-readiness claim.
- `user_cancelled` is reserved for an explicit user or portal cancellation.
- `timed_out` identifies the bounded stage deadline that won; the adapter still
  performs the required no-response Request cleanup and Session/FD cleanup.
- `malformed_response` is used for a terminal Response whose typed payload is
  invalid; the Response is still terminal and must not be followed by
  `Request.Close`.
- `unavailable` and `unsupported` are used before an owned resource exists
  when the frontend/provider/capability cannot satisfy the contract.
- `infrastructure_failure` covers transport, permission, portal rejection or
  PipeWire remote acquisition failures that are not user cancellation.
- Any cleanup failure or unresolved ownership moves the terminal stage to
  `cleanup` and uses `failed` or `unverified`; the operation status is not
  silently rewritten as success.

The current v1 schema has no `provider_crashed` or `externally_blocked`
status. A ScreenCast provider failure must therefore retain the truthful
operation result and cleanup facts; it must not be guessed from a timeout or
silently retried. A future provider-health diagnostic requires a separate
schema decision.

## Privacy and side-effect boundary

ScreenCast is an explicit, side-effectful operation. Before the first portal
call, the command must warn that the portal may display source-selection UI and
may grant access to selected screen/window/area streams.

PortalDoctor must not:

- read, decode, sample, record, encode, upload or persist PipeWire media;
- expose or persist the Session object path, Request path, FD number, stream
  node IDs, application names, window titles, source properties or raw portal
  errors;
- include raw stream tuples, portal payloads or PipeWire metadata in terminal,
  JSON, Markdown, journal, crash or test output; or
- claim that closing the Request, Session or FD deletes a portal-side capture
  grant or artefact.

The probe is a lifecycle/readiness check, not a capture tool. It may validate
the typed existence of returned stream metadata and the ability to acquire a
PipeWire remote FD, then it must release both Session and FD ownership without
consuming media.

## Acceptance checkpoint: internal lifecycle through `OpenPipeWireRemote`

The first implementation task was deliberately bounded to the
`CreateSession` adapter and its Session ownership/cleanup boundary. Its
verified acceptance criteria were:

- request subscription is installed before `CreateSession`;
- success, portal cancellation, malformed response, response timeout, method
  reply timeout, transport failure and explicit client cancellation are
  covered by a controlled fake;
- a terminal Response produces zero `Request.Close` calls;
- a no-response path performs one bounded `Request.Close` attempt and reports
  close failure or ambiguity without claiming clean cleanup;
- a valid Session handle is closed exactly once on every path that acquires it;
- a terminal `code=0` response with a missing, wrong-type, malformed or
  foreign Session handle attempts only expected-path Session cleanup, reports
  `completed`, `failed/session` or `unverified/session` truthfully, and never
  calls `Request.Close`;
- no Session path, token, raw error or payload appears in stdout/stderr or the
  machine result; and
- passive v0.2.1 behavior remains unchanged; and
- the internal adapter is not wired to a public ScreenCast command.

The controlled gate is implemented by
[`scripts/validate-screencast-create-session-ci.sh`](../scripts/validate-screencast-create-session-ci.sh)
and [`scripts/validate-screencast-create-session.py`](../scripts/validate-screencast-create-session.py).
The 2026-09-07 audit passed all 23 cases, including malformed-success
ownership recovery with an existing expected Session, absent/unknown expected
objects, explicit expected-Session close failure, terminal-response zero-close
assertions, no-response close counts, Session cleanup failures and ambiguous
ownership. The result and state files contain only sanitized status, stage,
cleanup, counters and booleans; tokens and object paths are not persisted.

The second bounded task implements only `SelectSources` on an acquired
Session. Its verified acceptance criteria are:

- `AvailableSourceTypes` is read from the frontend and the operation proceeds
  only when the Window bit (`2`) is advertised; missing capability metadata or
  a value without that bit is `unsupported` and still closes the acquired
  Session exactly once;
- the exact options are a fresh OS-entropy `handle_token`, `types=2` and
  `multiple=false`, with no persistence or restore option and no multiple
  source request;
- success, portal cancellation/rejection, malformed Response, response
  timeout, request-stage timeout, late method reply, client cancellation,
  transport failure and unavailable/unsupported capability are covered by a
  controlled fake;
- terminal SelectSources responses produce zero `Request.Close` calls, while
  every no-response path performs only the bounded Request cleanup justified
  by the returned handle and then performs exactly-once Session cleanup;
- Request.Close and Session.Close failures are preserved independently, and
  simultaneous failures aggregate both `request` and `session` resources;
- no Session/Request path, token, raw portal payload or source information is
  present in the result, logs or harness state; and
- the CreateSession-specific adapter remained disconnected from a public
  ScreenCast command and did not inspect streams or acquire a PipeWire FD.

The permanent SelectSources gate is implemented by
[`scripts/validate-screencast-select-sources-ci.sh`](../scripts/validate-screencast-select-sources-ci.sh)
and [`scripts/validate-screencast-select-sources.py`](../scripts/validate-screencast-select-sources.py).
The 2026-09-07 controlled audit passed 18 cases, including Window capability
acceptance/rejection, exact options, terminal-response zero-close behavior,
request/response timeouts, late replies, client cancellation, transport and
malformed responses, Request.Close and Session.Close failures, combined
cleanup failure, unanswered request ambiguity, exactly-once Session.Close,
open-resource state and privacy.

The third bounded task implements only `Start` on the Session whose Window
source selection succeeded. Its verified acceptance criteria are:

- the existing response subscription is installed before CreateSession and is
  reused for Start;
- Start sends the same sender-bound Session handle, a fresh OS-entropy
  `handle_token`, exactly `parent_window=""` and no other option;
- success, portal cancellation/rejection, malformed response, response and
  request-stage timeout, late method reply, client cancellation, transport
  failure and unanswered request are covered by the controlled fake;
- every terminal Start Response produces zero Start `Request.Close` calls,
  while each no-response path performs only the bounded cleanup justified by a
  known request handle;
- the acquired Session receives exactly one `Session.Close` attempt on every
  Start outcome, including Request cleanup failure, and Request/Session
  failures aggregate as separate v1 resources;
- a success Response containing synthetic stream metadata is accepted only as
  a stage-local Start result; its `streams` map, node IDs and raw values are
  not decoded, validated, serialized, logged or persisted;
- CreateSession and SelectSources terminal requests remain at zero
  `Request.Close`, and no request/session remains open after a clean run; and
- the internal adapter remains disconnected from the public CLI and does not
  implement `StreamsReturned`, `OpenPipeWireRemote`, PipeWire FD use or real
  ScreenCast E2E.

The permanent Start gate is implemented by
[`scripts/validate-screencast-start-ci.sh`](../scripts/validate-screencast-start-ci.sh)
using the controlled fake in
[`scripts/validate-screencast-select-sources.py`](../scripts/validate-screencast-select-sources.py).
The 2026-09-07 controlled audit passed all 15 cases, including exact Start
arguments, synthetic stream-payload dropping, terminal-response zero-close,
no-response cleanup, late replies, cancellation, transport failure,
Request/Session cleanup failures, aggregate resources, exactly-once Session
cleanup, no-open-resource assertions and privacy.

The fourth bounded task implements only the `StreamsReturned` validation
boundary after a successful Start Response. Its verified acceptance criteria
are:

- the fake emits the XDG-shaped `a(ua{sv})` container rather than synthetic
  pseudo-fields;
- exactly one `(u, a{sv})` tuple is accepted for the existing `multiple=false`
  policy, with an optional `source_type` variant accepted only as `u=2`;
- missing, wrong-type, empty, multiple, malformed-tuple/container and
  contradictory `source_type` payloads map to `malformed_response`;
- unknown property names and unusual property values remain opaque, and node
  IDs, properties, raw payloads, paths and tokens never enter output, logs,
  reports or harness state;
- every terminal Start Response, including malformed stream payloads, has zero
  Start `Request.Close` calls; CreateSession and SelectSources terminal
  requests also remain at zero;
- every acquired Session receives exactly one `Session.Close`, including valid
  and malformed stream payloads and Session cleanup failure; and
- the internal stage does not connect to PipeWire, use an FD, expose a public
  command or claim capture readiness.

The permanent StreamsReturned gate is implemented by
[`scripts/validate-screencast-streams-returned-ci.sh`](../scripts/validate-screencast-streams-returned-ci.sh).
The 2026-09-08 controlled audit passed all 14 cases and preserved the earlier
CreateSession (23), SelectSources (18) and Start (15) matrices.

The bounded internal lifecycle is not release-ready and the public ScreenCast
command is not accepted until all of the following additional gates pass:

### Controlled fake matrix

- `CreateSession`, `SelectSources` and `Start` success, user cancellation,
  portal rejection, malformed payload, response timeout, request-stage timeout,
  late method reply, transport failure and cleanup failure;
- exact stage mapping and v1 cleanup resource mapping for Request, Session and
  PipeWire remote;
- response-terminal no-close assertions for every Request-producing stage;
- Session.Close ordering and exactly-once assertions on success, cancellation,
  timeout and failure;
- valid/invalid `StreamsReturned` tuples and OpenPipeWireRemote valid/missing/
  invalid FD cases;
- PipeWire remote FD close, simulated close failure/unknown ownership and
  fail-closed `unverified` behavior;
- client cancellation without SIGINT-dependent assumptions, privacy scans and
  exit-code assertions for every result; and
- no open request, session or remote FD at the end of a clean controlled run.

### Unit and integration gates

- `ProbeResult` serialization/deserialization and probe/stage/resource
  invariants, including all ScreenCast-only combinations;
- per-stage mapping tests proving that an incomplete lifecycle cannot become
  `success` and that cleanup failure is independent;
- isolated-session-bus integration tests for response races, late replies,
  cancellation and bounded cleanup; and
- passive regression tests proving no active command runs during `check` or
  the default report path.

### Real-session and release gates

- disposable Ubuntu 26.04 + GNOME + Wayland + systemd-user session;
- real portal-mediated success and user-cancellation E2E for the negotiated
  ScreenCast path, with no media read/recorded and no raw stream data emitted;
- provider service health before and after each run, no unexpected crash, no
  leaked Session/FD and correct exit/result mapping;
- no backend-direct call, fake provider or alternate desktop backend counted
  as real-session evidence;
- `cargo fmt --check`, strict locked Clippy, locked full tests, locked release
  build, locked package/install smoke, passive fault regression, rustdoc and
  `cargo audit` where dependency changes are introduced; and
- documentation, roadmap and release metadata audit with Screenshot’s real
  gate still visibly open until its own external blocker changes.

## Remaining ScreenCast gate

The five internal boundaries and their aggregate controlled lifecycle audit
are complete. The real-session gate is currently **BLOCKED** before UI
execution because the required Window capability is not advertised. Do not
call `SelectSources`, `Start` or `OpenPipeWireRemote`, and do not retry the
same provider state. After the capability trigger above, run exactly one
supported real success and one portal-native cancellation path, then repeat
the release/regression/privacy/cleanup gates before deciding whether to expose
a public command. Until that sequence passes, no ScreenCast success or
capture-readiness claim is made and no v0.3.0 release approval is possible.

## References

- [XDG ScreenCast interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
- [XDG portal request lifecycle](https://flatpak.github.io/xdg-desktop-portal/docs/requests.html)
- [XDG Request interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Request.html)
- [XDG Session interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Session.html)
- [`ProbeResult` v1](probe-result-schema.md)
- [ASHPD integration decision](PORTALDOCTOR_ASHPD_DECISION.md)
- [Screenshot decision boundary](PORTALDOCTOR_SCREENSHOT_DECISION.md)
