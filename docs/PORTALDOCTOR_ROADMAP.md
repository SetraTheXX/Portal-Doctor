# PortalDoctor — Development Roadmap

**Status:** Current implementation roadmap; last verified 2026-09-09
**Date:** 2026-09-05
**Strategy:** Narrow vertical slice first, then expand subsystem coverage and desktop compatibility

> **Current handoff:** Read [`PORTALDOCTOR_CURRENT_STATE.md`](PORTALDOCTOR_CURRENT_STATE.md)
> before starting work. Phases 0–7 and the v0.2.1 passive stabilization gate are
> complete. Phase 8 remains the blocked v0.3.0 release/public-command gate;
> its five internal ScreenCast slices and aggregate controlled gate are
> complete. Independent Phase 9–11 compatibility development is allowed to
> proceed through bounded controlled slices, while the remaining real-session
> gates stay tracked in [GitHub Issue #3](https://github.com/SetraTheXX/Portal-Doctor/issues/3).

---

## 1. Roadmap Philosophy

PortalDoctor should not attempt to solve the entire Linux desktop portal ecosystem in its first release.

The implementation sequence is designed to minimize rework:

```text
Foundation
   -> session discovery
      -> portal routing
         -> runtime verification
            -> deterministic doctor v0.1
               -> PipeWire/journal/report v0.2
                  -> active probes v0.3
                     -> desktop expansion
                        -> stable 1.0 contract
```

Each phase must have an exit gate. A phase is not complete merely because code exists.

---

# Phase 0 — Project Foundation

## Objective

Create a clean, testable Rust CLI foundation and lock the product contracts before implementing Linux-specific diagnosis.

## Tasks

### Repository

- initialize Rust binary project,
- MIT license,
- `.gitignore`,
- README skeleton,
- CHANGELOG,
- CONTRIBUTING,
- SECURITY,
- GitHub Actions CI.

### Quality gates

- `cargo fmt --check`,
- strict Clippy,
- `cargo test`,
- CI on pull requests.

### CLI shell

Implement:

```bash
portaldoctor --help
portaldoctor --version
portaldoctor check
```

### Core models

Define:

- collection status,
- snapshot schema v1,
- finding model,
- severity,
- confidence,
- evidence model,
- renderer interface.

### Initial JSON contract

Create a minimal versioned JSON format even before full data exists.

## Exit criteria

- project builds cleanly,
- CI is green,
- no empty architecture scaffolding beyond near-term need,
- `--help` and `--version` work,
- an empty/basic diagnostic snapshot can render to terminal and JSON,
- no Linux system mutation occurs.

## Release

No public feature release required. Optional internal dev tags (e.g.
`v0.0.1-dev`) may be cut; tag names do not have to match the crate version.

Version policy for Phases 0–3:

- the crate version stays `0.1.0` while development targets v0.1.0; no release
  is cut from these phases,
- `portaldoctor_version` in the JSON report always equals the crate version
  (PRD §7.4),
- changes accumulate under `[Unreleased]` in CHANGELOG.md,
- the first public release is **v0.1.0**, cut at the Phase 4 exit gate.

---

# Phase 1 — Environment & Desktop Discovery

## Objective

Reliably answer:

> What desktop/session context is PortalDoctor running inside?

## Collectors

- `/etc/os-release`,
- selected XDG/session environment,
- Wayland/X11 state,
- desktop identifiers,
- XDG config/data search roots,
- systemd user activation environment when available.

## Initial models

```text
SystemInfo
SessionInfo
EnvironmentInfo
EnvironmentComparison
```

## Rules

Implement first environment rules:

- `ENV001`
- `ENV002`
- `ENV003`
- `ENV004`

## CLI

```bash
portaldoctor check environment
portaldoctor check environment --verbose
portaldoctor --json
```

## Tests

Fixtures:

- healthy Ubuntu GNOME Wayland,
- missing `XDG_CURRENT_DESKTOP`,
- Wayland without `WAYLAND_DISPLAY`,
- process environment vs systemd activation mismatch.

## Exit criteria

On the primary Ubuntu/GNOME/Wayland machine, PortalDoctor:

- correctly identifies session type,
- correctly identifies relevant desktop values,
- compares activation environment safely,
- never dumps arbitrary environment values,
- produces deterministic fixture findings.

---

# Phase 2 — XDG Portal Discovery & Routing Resolver

## Objective

Build PortalDoctor's main differentiator: explain which portal backend should serve each interface and why.

## Tasks

### XDG search-path resolver

Implement effective locations based on:

- `XDG_CONFIG_HOME`,
- `XDG_CONFIG_DIRS`,
- `XDG_DATA_HOME`,
- `XDG_DATA_DIRS`,
- standard fallbacks.

### Desktop-specific config resolution

Parse colon-separated `XDG_CURRENT_DESKTOP` values and build desktop-specific config candidate order.

### `portals.conf` parser

Support:

- `[preferred]`,
- `default`,
- explicit implementation-interface selectors,
- ordered backend candidates,
- `none`,
- `*`,
- source provenance.

### `.portal` parser

Collect:

- backend identifier,
- D-Bus name,
- implemented interfaces,
- legacy `UseIn`,
- descriptor path and precedence.

### Route model

Produce route tables with:

- requested candidates,
- available providers,
- selected candidates,
- provenance/evidence,
- route status.

## CLI

```bash
portaldoctor portal list
portaldoctor portal routes
portaldoctor portal explain ScreenCast
```

## Rules

Add:

- `XDP003`
- `XDP004`
- `XDP005`
- `CFG001`
- `CFG002`
- `CFG003`
- initial conservative `CFG004`

## Tests

Required scenarios:

- GNOME default routing,
- explicit FileChooser override,
- configured backend not installed,
- backend installed but missing requested interface,
- malformed config,
- `none`,
- `*`,
- multiple desktop names,
- multiple XDG precedence levels.

## Exit criteria

- routing output is explainable and source-backed,
- no hardcoded GNOME-only route logic,
- resolver fixture tests cover precedence cases,
- same fixture always yields same route/finding result.

---

# Phase 3 — D-Bus & systemd Runtime Verification

## Objective

Move from static configuration analysis to runtime verification.

## Tasks

### zbus integration

- connect to session bus,
- query XDG Desktop Portal frontend safely,
- classify missing name vs timeout vs activation failure,
- capture relevant properties/versions when useful.

### systemd user services

Collect relevant states for:

- XDG Desktop Portal frontend,
- detected portal backends when resolvable,
- user session dependencies relevant to diagnosis.

### Timeouts

Introduce central timeout policy for all runtime operations.

## Rules

Add:

- `DBUS001`
- `DBUS002`
- `XDP001`
- `XDP002`

## Tests

Mock/fixture runtime states:

- no session bus,
- portal bus name absent,
- portal call timeout,
- selected backend unavailable,
- healthy runtime.

## Exit criteria

PortalDoctor can distinguish:

- configuration says provider exists,
- provider descriptor exists,
- runtime frontend/backend cannot actually be reached.

No broken D-Bus service can hang the CLI indefinitely.

---

# Phase 4 — Diagnostic Engine v1 & v0.1 Release

## Objective

Turn collectors and routing into a coherent first usable PortalDoctor release.

## Tasks

### Rule engine

Finalize v0.1 rule registration and evaluation order.

### Evidence

Each error/warning includes structured evidence.

### Terminal reporter

Default output emphasizes actionable findings.

### JSON reporter

Publish schema-v1 documentation.

### Documentation

- README usage,
- initial finding catalog,
- privacy statement,
- compatibility statement,
- known limitations.

### Validation

Primary real environment:

- Ubuntu 26.04,
- GNOME,
- Wayland.

## v0.1 expected capabilities

```text
✓ desktop/session discovery
✓ environment/activation comparison
✓ portal config discovery
✓ backend inventory
✓ route resolver
✓ D-Bus frontend health
✓ basic systemd user runtime state
✓ deterministic findings
✓ terminal output
✓ JSON output
✓ fixture tests
```

## Exit criteria

A real user can install and run:

```bash
portaldoctor
```

and obtain a useful explanation of the basic portal stack without modifying the system.

## Release

**v0.1.0**

---

# Phase 5 — PipeWire & WirePlumber Integration

## Objective

Make ScreenCast readiness meaningful rather than stopping at portal routing.

## Tasks

### PipeWire

Use bounded `pw-dump` execution and JSON parsing.

Capture only portal-relevant normalized state.

### WirePlumber

Check:

- service/reachability,
- `wpctl` connectivity where useful,
- minimal relevant media graph state.

### Correlation

Examples:

```text
ScreenCast provider missing
ScreenCast provider present + PipeWire missing
ScreenCast provider present + PipeWire healthy
PipeWire command unavailable
PipeWire query timeout
```

## Rules

Add/refine:

- `PW001`
- `PW002`
- `PW003`
- `SC001`
- `SC002`

## Exit criteria

PortalDoctor can explain basic ScreenCast stack readiness without starting an actual user capture session.

## Implementation checkpoint — 2026-08-31

The Phase 5 passive collector and correlation rules shipped in v0.2.0:

- bounded `pw-dump --no-colors` and `wpctl status` collection,
- normalized, privacy-safe PipeWire/WirePlumber snapshot sections,
- `PW001`–`PW003` and `SC001`–`SC002`,
- deterministic parser/rule/renderer tests and live GNOME/Wayland validation.

Phase 5 is complete and included in the v0.2.0 release. Its acceptance gate
was closed together with the Phase 6 journal, Phase 7 report, privacy and
compatibility checks.

---

# Phase 6 — Journal Evidence

## Objective

Attach bounded runtime evidence to diagnoses.

## Tasks

- current-boot/user-session journal collector,
- relevant unit allowlist,
- JSON parsing,
- bounded entry/time limits,
- error-pattern classification only where reliable,
- evidence correlation.

## Guardrails

- no full journal dump,
- no unbounded log scan,
- no rules based solely on fragile text if authoritative state exists elsewhere,
- sanitize before report exposure.

## Exit criteria

A finding can show a concise supporting log excerpt without leaking unrelated system history.

## Implementation checkpoint — 2026-08-30

Phase 6 shipped in v0.2.0:

- `--journal` opt-in collection for the current user boot and last 30 minutes,
- allowlisted portal, PipeWire, WirePlumber and dynamic portal backend units,
- 80-record, 512 KiB and normal command-timeout boundaries,
- structured-field parsing, stable classification and privacy sanitization,
- `journal_excerpt` correlation on existing `PW`/`SC` findings,
- fixtures for empty, unavailable, timeout, malformed, noisy and representative
  journal input.

Phase 6 is complete and included in the v0.2.0 release. Journal evidence
remains opt-in and bounded by design.

---

# Phase 7 — Reporting & Privacy

## Objective

Create bug-report-ready, shareable output.

## Commands

```bash
portaldoctor report
portaldoctor report --json
portaldoctor report --format markdown
```

## Tasks

- explicit `report` command with terminal, JSON and Markdown formats,
- redaction engine before shareable serialization,
- `$HOME` normalization and obvious secret-pattern masking,
- optional hostname suppression,
- enforcement of the existing environment allowlist,
- separate report version and snapshot schema version metadata,
- explicit raw-journal/raw-PipeWire exclusion metadata,
- fixtures and golden tests for stable shareable output,
- documentation of collected data, redaction rules and pre-sharing review.

## Exit criteria

Generated report can be attached to a public GitHub issue without exposing obvious secrets or irrelevant personal data by default.

## Implementation checkpoint — 2026-08-30

Phase 7 shipped in v0.2.0:

- `portaldoctor report` emits a redacted terminal report by default,
- `--format json`/`--json` emits a versioned shareable envelope,
- `--format markdown` emits a stable issue-friendly report,
- process environment data is restricted to the existing allowlist and home
  paths normalize to `$HOME`,
- obvious secret labels are masked and `--suppress-hostname` replaces the
  current hostname and host labels,
- raw journal and raw PipeWire dumps remain excluded and are marked as such,
- redaction, format and Markdown golden-fixture tests are checked in.

The v0.2.0 acceptance, privacy and compatibility gates are complete. The
v0.2.1 stabilization gate adds the documented exit-code contract, locked
package/install smoke coverage and the public demo/release alignment. This
patch release is now published; the next implementation phase is Phase 8
active portal probes.

## Release checkpoint — 2026-08-31

v0.2.1 is published to [crates.io](https://crates.io/crates/portaldoctor) and
[GitHub Releases](https://github.com/SetraTheXX/Portal-Doctor/releases/tag/v0.2.1).
It preserves the v0.2.0 passive scope and adds the exit-code contract, locked
package/install smoke, a readable four-scene demo, and Linux x86_64 release
binary/checksum assets. Issue #2 and the v0.2.0 milestone are complete; the
next implementation gate is the explicitly invoked active-probe work in
Phase 8 / v0.3.0.

## Release

**v0.2.1**

---

# Phase 8 — Active Portal Probes

## Objective

Move from “stack looks ready” to explicit portal lifecycle tests.

## Dependency

The Phase 8 integration decision is complete. Read
[`PORTALDOCTOR_ASHPD_DECISION.md`](PORTALDOCTOR_ASHPD_DECISION.md) before
implementing any active request.

- [x] Evaluate ASHPD 0.13.13, its runtime/features and public request/session
  lifecycle.
- [x] Select a PortalDoctor-owned `zbus` lifecycle adapter as the control
  boundary; use ASHPD only where it preserves handle and cleanup observability.
- [x] Define the standalone v1 `ProbeResult` contract for operation status,
  lifecycle stage and independent cleanup outcome, with constructor/serde
  invariant validation, including the probe/stage/resource compatibility
  matrix. See [`probe-result-schema.md`](probe-result-schema.md).
- [x] Implement the first FileChooser slice using that contract. The
  unreleased `main` branch now provides `portaldoctor probe filechooser` with
  a direct `zbus` lifecycle adapter; the published v0.2.1 passive path is
  unchanged.

The first implementation does not add ASHPD to `Cargo.toml`: its high-level
request future would hide the request handle needed for bounded cleanup. It
adds only the Tokio runtime feature on the existing `zbus` line and the small
stream/runtime support needed by the explicit command. The passive
snapshot/report JSON remains unchanged.

## Probes

### Phase 8 sequencing policy

The Phase 8 work is split into controlled implementation state and release
approval state:

- FileChooser controlled and supported-session validation are complete.
- Screenshot controlled implementation is complete, including the v3
  Window-only and trusted-GNOME v2 paths, but its real GNOME
  success/cancellation gate is open because the observed provider hangs and
  crashes. This continues to block v0.3.0 release approval.
- The same broken Screenshot provider version must not be retried for another
  real E2E, and v2 remains unreleased; the v3 Window-only path remains intact.
- ScreenCast controlled/internal implementation is complete: its internal
  `CreateSession`, `SelectSources`, `Start`, `StreamsReturned` and
  `OpenPipeWireRemote` implementations and aggregate cross-stage lifecycle
  gate pass. Its real success/cancellation gate is **BLOCKED** by the live
  provider capability absence (`AvailableSourceTypes=0`, Window bit `2`
  missing). The same provider state must not be retried, and this does not
  close or weaken the Screenshot release gate.
- These external blockers do not reopen completed internal implementation or
  force Phase 8 development to spin forever. Independent bounded work, and a
  next development phase that does not depend on either real-session gate, may
  proceed. Public active-probe commands and v0.3.0 release approval remain
  blocked until their exact real-session gates pass.

The ScreenCast real gate may be re-evaluated only when the public frontend
reports `AvailableSourceTypes & 2 != 0`, the selected provider/frontend is
stable and healthy, and a supported disposable Ubuntu/GNOME/Wayland session is
ready for exactly one real success plus one portal-native cancellation E2E.

The canonical ScreenCast design and acceptance boundary is
[`PORTALDOCTOR_SCREENCAST_DECISION.md`](PORTALDOCTOR_SCREENCAST_DECISION.md).

### FileChooser

```bash
portaldoctor probe filechooser
```

The command is explicit and unreleased. It warns before a desktop dialog,
subscribes to `Request::Response` before `OpenFile`, supplies a unique
`handle_token`, validates the returned or token-derived request handle, bounds
request/recovery/response/cleanup separately, and reports standalone
`ProbeResult` v1 JSON. It never reads, copies, modifies or persists the
selected file; ScreenCast is tracked separately by its design checkpoint.

- [x] Validate the cancellation and cleanup path in the supported Ubuntu 26.04
  + GNOME + Wayland + systemd user session.
- [x] Validate a successful selection path in the same supported session;
  confirm that the result is `success` with a terminal response and no URI,
  filename or file content emitted.
- [x] Validate the request/response race boundary with a controlled fake
  portal: success, user cancellation, malformed response, response timeout,
  late method reply,
  request-stage timeout, transport failure, unsupported capability and
  `Request.Close` observation, including an explicit close-failure result.
- [x] Recover a late method reply within a bounded grace period and use the
  token-derived Request path when the reply never arrives; report unresolved
  transport ambiguity as `cleanup.status: unverified`.
- [x] Make the controlled lifecycle harness a permanent CI gate. Every mode
  asserts JSON status, process exit, URI redaction and the expected
  `Request.Close` count, including explicit no-request scenarios.
- [x] Require OS-provided entropy for every `handle_token`; injected entropy
  failure is tested as a pre-request `infrastructure_failure` with no
  predictable fallback.

### Screenshot

```bash
# Explicit development command; unreleased until the v0.3.0 gate passes.
portaldoctor probe screenshot
```

The implementation boundary is recorded in
[`PORTALDOCTOR_SCREENSHOT_DECISION.md`](PORTALDOCTOR_SCREENSHOT_DECISION.md).
Unlike FileChooser, a successful Screenshot request may create an image and
return a URI for a portal-managed artifact. The first slice is therefore
explicitly side-effectful at the portal data boundary and is not considered
read-only merely because PortalDoctor never opens the image.

- [x] Define a PortalDoctor-owned request lifecycle that reuses the audited
  FileChooser response-race, timeout, cancellation and no-response
  `Request.Close` rules; a terminal `Response` must never be followed by
  `Request.Close`.
- [x] Define capability negotiation: v3 `AvailableTargets` preflight and
  Window-only target policy with no silent Screen/Area/Active-Window fallback,
  plus a trusted-GNOME v2 interactive path that sends no `target` and makes no
  Window-only claim.
- [x] Define the URI/image privacy and artifact-ownership boundary, including
  the fact that Request.Close does not delete a screenshot already created by
  the portal.
- [x] Define controlled-fake, real-session, package, privacy and release gates.
- [x] Implement `org.freedesktop.portal.Screenshot.Screenshot` using the
  shared PortalDoctor-owned request/token/response/cleanup mechanics.
- [x] Validate controlled v2/v3 success, cancellation, request/response
  timeout, late reply, malformed response, portal/transport failure,
  unsupported target/version, untrusted v2, unavailable service and cleanup
  failure.
- [x] Assert exact capability-specific options, ProbeResult v1 semantics,
  response-terminal no-close and no-response Request.Close observations,
  open-request state, exit codes and absence of URI/path/image data from stdout
  and stderr.
- [ ] Run the supported real-session validation in a disposable Ubuntu 26.04
  + GNOME + Wayland context using the path actually negotiated; PortalDoctor
  must not open or inspect the generated image. The current portal exposes v2
  without `AvailableTargets`, so v2 evidence must include the explicit GNOME
  route/provider proof and the broader screen/window/area warning. The
  2026-09-07 success-path attempt hung before selection, returned a bounded
  `timed_out/response/cleanup.status: completed` result, and was followed by
  `InteractiveScreenshot didn't return a file` and an
  `xdg-desktop-portal-gnome` `SIGSEGV`/core-dump. The gate remains externally
  blocked; do not count this as success/cancellation or retry the same provider
  state indefinitely.
  Do not substitute a backend-direct call, controlled fake or wlroots backend
  for the required GNOME success/cancellation evidence.

### ScreenCast

There is no public `portaldoctor probe screencast` command yet.

The public ScreenCast command has not started. The design and bounded
internal `CreateSession`, `SelectSources`, `Start`, `StreamsReturned` and
`OpenPipeWireRemote`
checkpoints in
[`PORTALDOCTOR_SCREENCAST_DECISION.md`](PORTALDOCTOR_SCREENCAST_DECISION.md)
fix the lifecycle as:

```text
CreateSession
SelectSources
Start
StreamsReturned
OpenPipeWireRemote
```

The JSON stages remain `create_session`, `select_sources`, `start`,
`streams_returned` and `open_pipe_wire_remote`, matching the existing v1
`ProbeResult` model. Request, Session and returned PipeWire remote FD ownership
are independent: a terminal Request `Response` means no later
`Request.Close`; no-response cancellation/timeout uses bounded Request cleanup;
the Session is closed exactly once when acquired; and the returned FD is
closed before Session cleanup. No media, stream metadata or raw portal values
may enter output or persistent state.

The internal `CreateSession`, `SelectSources`, `Start`, `StreamsReturned` and
`OpenPipeWireRemote` slices are complete and are not public commands. They own
the Request lifecycle, validate the
returned Session handle, negotiate the advertised Window source bit, send only
`types=2`, `multiple=false` and a fresh request token without persistence or
restore options, and close an acquired Session exactly once. Start additionally
sends the same owned Session handle with explicit `parent_window=""` and drops
the terminal `streams` map at its Start-only boundary. The subsequent
`StreamsReturned` boundary accepts only the XDG `a(ua{sv})` container with one
Window stream, keeps node IDs/properties opaque and rejects malformed or
policy-inconsistent payloads without logging them. OpenPipeWireRemote sends
empty options, creates no Request, acquires only a typed owned FD, closes it
before Session.Close and never connects to PipeWire or reads media. When a terminal
success response has an unusable or foreign Session handle, CreateSession
attempts cleanup only on the expected sender-and-token-derived path;
successful expected cleanup remains `malformed_response`, explicit close
failure is `failed/session`, and absent or ambiguous ownership is
`unverified/session`. SelectSources additionally proves terminal-response
zero Request.Close, bounded no-response cleanup and aggregate Request/Session
cleanup failures. The controlled fake matrices cover success, portal
cancellation/rejection, malformed response bodies, response and request
timeouts, late replies, client cancellation, transport and capability
boundaries, Request/Session close failures, ambiguous ownership,
terminal-response zero-close behavior, exact options and privacy/open-resource
assertions. The Start matrix additionally covers synthetic stream metadata,
request/session cleanup aggregation and its stage-local success boundary. The
StreamsReturned matrix covers valid/minimal/opaque-property streams,
malformed/missing/wrong-type/empty/multiple/policy-inconsistent payloads,
terminal zero-close and Session cleanup/privacy assertions. The aggregate gate
also composes all five slices in one controlled state machine and checks
cross-stage failures, cleanup ordering/aggregation, privacy, open resources and
the hidden public command. The five internal slices remain outside the public
command and release approval. The current real-session gate is **BLOCKED**
before UI execution by the missing Window capability; after the trigger above,
the next gate is one success plus one portal-native cancellation followed by
the release/public-command decision.

- [x] Implement and controlled-test the internal `CreateSession` plus Session
  ownership/cleanup slice. Keep it out of the public CLI and do not count its
  fake evidence as Screenshot real-session evidence.
- [x] Implement and controlled-test the bounded `SelectSources` slice with
  the same Request/Session lifecycle invariants. Keep it out of the public
  CLI and do not count its fake evidence as Screenshot real-session evidence.
- [x] Implement and controlled-test the bounded `Start` slice with the same
  Request/Session lifecycle invariants, exact headless arguments and a dropped
  stream payload. Keep it out of the public CLI and do not count its fake
  evidence as Screenshot real-session evidence.
- [x] Implement and controlled-test the bounded `StreamsReturned` slice with
  the XDG `a(ua{sv})` boundary, exactly-one Window policy, opaque property
  handling, fail-closed malformed cases, terminal zero-close and exactly-once
  Session cleanup. Keep it out of the public CLI and do not count its fake
  evidence as Screenshot real-session evidence.
- [x] Implement and controlled-test the bounded `OpenPipeWireRemote` direct-FD
  slice with empty options, no Request.Close path, typed owned-FD release
  before exactly-once Session.Close, late-FD recovery, cleanup aggregation and
privacy. Keep it out of the public CLI and do not claim ScreenCast readiness.
- [x] Run the aggregate internal lifecycle gate across all five slices. Cover
  full success, representative CreateSession/SelectSources/Start and
  StreamsReturned failures, direct-FD timeout/cancellation/late/ambiguous
  ownership, FD and Session cleanup failures, aggregate cleanup resources,
  terminal-response zero-close behavior, FD-before-Session ordering, exact
  call counts, privacy, no open resources and the hidden public command.
- [ ] Re-evaluate the real-session/release/public-command gate only after the
  public Window capability bit and provider stability trigger are present. Run
  exactly one success and one portal-native cancellation, then all
  release/regression gates. Do not add a public command, PipeWire handshake or
  media access before that decision.

The full ScreenCast implementation exit gate additionally requires the
controlled fake matrix, unit/integration lifecycle tests, disposable supported
Ubuntu/GNOME/Wayland success and user-cancellation E2E, privacy and FD/session
cleanup checks, passive regression, strict fmt/Clippy/tests, release
build/package/install smoke, rustdoc, and cargo audit when dependencies
change. Those gates do not count as Screenshot real-session evidence.

## UX constraints

- active probes never run during default passive check,
- clearly state that a dialog may appear,
- distinguish user cancellation from infrastructure error,
- time out safely,
- clean up sessions on failure.

## Exit criteria

PortalDoctor can identify the exact stage at which a ScreenCast lifecycle fails
or times out, and can prove Request, Session and PipeWire remote cleanup
without consuming media. The internal ScreenCast implementation meets this
controlled criterion, but the ScreenCast real gate is blocked by the absent
Window capability and the Screenshot real gate is independently blocked by the
GNOME provider. Both keep v0.3.0 release approval open. This does not prevent
independent bounded development or a next phase that explicitly does not rely
on either external real-session gate.

## Release

**v0.3.0**

---

## Development sequencing rule after the external gates

Phase 9 development may begin only for independent, bounded compatibility or
read-only work that does not require ScreenCast/Screenshot real-session proof.
This is a development sequencing allowance, not a Phase 8 exit, a public
command approval or a v0.3.0 release decision. Any Phase 9 item requiring an
active-probe real session remains blocked until the corresponding provider
trigger and real success/cancellation gate are satisfied.

# Phase 9 — KDE Plasma Compatibility

## Objective

Validate the architecture beyond GNOME rather than merely allowing it to compile.

## Tasks

- [x] Add deterministic KDE `kde-portals.conf` and `kde.portal` fixtures with
  parser, metadata, candidate-precedence, default-route and `UseIn` coverage.
- [x] Audit the portal resolver/rules for GNOME-only assumptions relevant to
  this slice; no production GNOME-only routing assumption was found. Existing
  GNOME references remain test fixtures, user-facing examples or the separate
  trusted-GNOME Screenshot compatibility boundary.
- [x] Add controlled KDE runtime-correlation coverage for the generic
  `xdg-desktop-portal-kde.service` mapping, active/failed/not-found systemd
  states and `org.freedesktop.impl.portal.desktop.kde` D-Bus ownership/finding
  semantics. This validates deterministic read-only correlation only; it does
  not claim a live Plasma runtime.
- [x] Add controlled KDE/Plasma Wayland environment coverage for `KDE`,
  `KDE:Plasma` and `plasma` session identities, activation-environment
  mismatch semantics and missing `WAYLAND_DISPLAY`. This is fixture-only and
  does not claim a live Plasma session.
- [x] Add one production-pipeline-style KDE passive snapshot aggregate covering
  healthy routing/runtime and degraded KDE owner/service states. Healthy state
  is finding-free; degraded state remains the generic `DBUS002` runtime finding
  without being misclassified as configuration or environment failure.
- [x] Run the production D-Bus collector against an isolated
  `dbus-run-session` with the canonical KDE well-known name owned and absent.
  The gate also proves unsorted duplicate selected names are sorted/deduplicated
  deterministically. This is controlled D-Bus ownership coverage only, not live
  Plasma validation or a KDE support claim.
- [x] Run the production systemd-user collector against a temporary, guarded
  fake `systemctl` through an explicit ignored CI wrapper. The gate proves the
  exact `xdg-desktop-portal-kde.service` invocation and active/failed/not-found/
  timeout mappings, including child reaping, without contacting the real user
  systemd manager. This remains controlled coverage only and creates no live
  Plasma validation or KDE support claim.
- [x] Run the production D-Bus and systemd collectors together under one
  isolated `dbus-run-session` plus guarded fake-systemctl wrapper and feed their
  results into the existing KDE routing/rule pipeline. Owned/active is finding-
  free; absent/not-found and representative absent/failed activation paths emit
  only `DBUS002`. This is controlled passive aggregation only, not live Plasma
  validation or a KDE support claim.
- [ ] Live Plasma Wayland session validation — the 2026-09-08 preflight on the
  current Ubuntu 26.04.1 GNOME/Wayland host is **BLOCKED / NOT AVAILABLE**:
  the KDE backend package and `xdg-desktop-portal-kde.service` are absent, the
  KDE D-Bus name has no owner, and routes select GNOME. Do not force or repeat
  this gate on the same host. Recheck only after a real Plasma Wayland session,
  healthy KDE backend/service, and an owner for
  `org.freedesktop.impl.portal.desktop.kde` are present.
- `xdg-desktop-portal-kde` runtime behavior,
- active probe validation,
- compatibility documentation.

## Rule review

Audit all existing rules for GNOME assumptions.

## Exit criteria

The static KDE fixture and controlled runtime-correlation slices are complete
without changing the v0.2.1 support matrix. KDE/Plasma runtime behavior, active
probes and a documented runtime compatibility claim remain later Phase 9 gates.

## Release

**v0.4.0**

---

# Phase 10 — wlroots / Sway Compatibility

## Objective

Validate mixed-backend routing and activation-environment diagnosis.

## First bounded static slice

- [x] Add controlled `sway-portals.conf`, `wlr.portal` and GTK descriptor
  fixtures with desktop-specific precedence coverage.
- [x] Verify `Screenshot` and `ScreenCast` select `wlr`, while `FileChooser`
  and `Settings` select the appropriate GTK fallback.
- [x] Verify the resolver keeps legacy `UseIn` filtering for no-preference
  routes while configured preferences remain authoritative, and that the
  portal rules do not emit false missing-provider or duplicate/multi-provider
  findings for the pinned mixed configuration.
- [x] Add a controlled Sway Wayland environment fixture and aggregate passive
  snapshot: the healthy mixed route is clean, and a missing `WAYLAND_DISPLAY`
  produces only the existing `ENV003` finding.

This is static parser/resolver/rule coverage only. It does not validate a live
Sway session, install a provider, run an active probe or create a wlroots/Sway
support claim. The aggregate environment coverage is likewise fixture-only and
does not create a live Sway support claim.

## Controlled runtime-correlation slice

- [x] Exercise the generic runtime correlation model for `wlr` and `gtk`:
  descriptor D-Bus names and conventional systemd unit mappings are asserted.
- [x] Verify healthy owner/unit state is silent, while missing or failed `wlr`
  and missing GTK fallback runtime emit only the generic `DBUS002` finding.

This remains controlled snapshot/rule coverage. It does not validate a live
Sway/Wayland session, install or start providers, or run an active probe.

## Controlled production-collector runtime aggregate

- [x] Run the existing Sway mixed-routing snapshot through the production
  `dbus::collect()` and `systemd_user::collect()` paths inside an isolated
  `dbus-run-session` and guarded fake-`systemctl` wrapper.
- [x] Cover healthy WLR/GTK ownership, missing WLR, missing GTK fallback and
  failed WLR; assert exact D-Bus names, exact unit allowlist and generic
  `DBUS002` semantics without contacting real user systemd.

This is an ignored controlled integration gate only. It does not validate a
live Sway session, install/start a provider, run an active probe or create a
wlroots/Sway support claim.

## Controlled activation-environment integration slice

- [x] Feed a Sway Wayland process fixture through production
  `activation_environment::collect()` under an isolated guarded fake
  `systemctl`, while preserving the mixed WLR/GTK routing and runtime checks.
- [x] Cover equal activation values, stale desktop, stale/different or missing
  activation `WAYLAND_DISPLAY`, and unavailable/timeout collection. Healthy
  activation is clean; mismatch cases emit only generic `ENV004`; unavailable
  and timeout cases do not synthesize a mismatch or leak a child process.

This is controlled production-collector coverage only. It does not validate a
live Sway session, install/start a provider, run an active probe or create a
wlroots/Sway support claim.

## Live Sway readiness and active-probe decision checkpoint — 2026-09-09

- [ ] Live Sway preflight is available. The current host is Ubuntu 26.04.1
  GNOME/Wayland, not Sway; `xdg-desktop-portal-wlr`, its user unit and its
  canonical D-Bus owner are absent. The frontend routes Screenshot and
  ScreenCast to GNOME, while PipeWire/WirePlumber are healthy.
- [ ] Active WLR probe validation is authorized. The current decision is
  **ACTIVE PROBE READINESS BLOCKED**: controlled generic lifecycle coverage
  does not substitute for a real WLR provider/session capability check.

The current checkpoint is **LIVE SWAY ENVIRONMENT BLOCKED / NOT AVAILABLE**;
no active probe or portal UI was opened and no support/release claim follows.
Re-evaluate only in a real Sway Wayland session with a healthy WLR package and
`xdg-desktop-portal-wlr.service`, a live
`org.freedesktop.impl.portal.desktop.wlr` owner, WLR-selected passive routes,
Screenshot v3 `AvailableTargets & 2`, ScreenCast `AvailableSourceTypes & 2`,
and ready PipeWire/WirePlumber. This does not reopen or alter the Phase 8
GNOME provider blockers.

## Important cases

`xdg-desktop-portal-wlr` implements a limited subset of portals, so mixed backend usage is expected.

Test:

- Screenshot routing,
- ScreenCast routing,
- GTK fallback for other interfaces,
- `WAYLAND_DISPLAY` propagation,
- `XDG_CURRENT_DESKTOP` propagation,
- desktop-specific `*-portals.conf` behavior.

## Exit criteria

PortalDoctor does not falsely flag intentional mixed-backend configurations as broken.

---

# Phase 11 — Hyprland / Niri Compatibility

## Objective

Handle modern compositor setups where portal integration often combines multiple implementations.

## First bounded Hyprland static/passive slice — 2026-09-09

- [x] Add controlled Hyprland Wayland environment, desktop-specific
  `hyprland-portals.conf`, `hyprland.portal` and GTK fallback fixtures.
- [x] Verify the generic resolver selects Hyprland for Screenshot/ScreenCast
  and GTK for FileChooser/Settings, applies desktop-specific configuration
  before generic fallback, and keeps configured preferences authoritative even
  when a legacy `UseIn` value names another desktop. No-preference routes keep
  the legacy `UseIn` fallback.
- [x] Aggregate the fixture through the existing passive rule pipeline:
  healthy mixed routing is clean, missing `WAYLAND_DISPLAY` emits only
  `ENV003`, and missing Hyprland or GTK owner/unit state emits only the
  corresponding generic `DBUS002` while routes remain unchanged.
- [x] Assert descriptor-derived Hyprland D-Bus ownership naming and the
  generic `ServiceInfo::backend_unit("hyprland")` mapping; intentional
  Hyprland+GTK installation does not create duplicate/config findings.

This slice is controlled static/passive coverage only. It does not validate a
live Hyprland session, install or start providers, run active Screenshot or
ScreenCast probes, create a Hyprland support claim, or change the Phase 8,
KDE or Sway blocker state.

## Controlled production runtime + activation slice — 2026-09-09

- [x] Run the descriptor-derived Hyprland/GTK D-Bus names and exact portal
  units through the production `dbus::collect()` and
  `systemd_user::collect()` paths inside a private `dbus-run-session`.
- [x] Run production `activation_environment::collect()` through the same
  guarded fake-`systemctl` wrapper, including healthy, stale desktop,
  stale/missing activation `WAYLAND_DISPLAY`, unavailable and timeout cases.
- [x] Assert the existing generic rule semantics: healthy mixed runtime is
  clean; missing/failed Hyprland or missing GTK fallback is only `DBUS002`;
  activation mismatches are only `ENV004`; unavailable/timeout activation
  does not create a synthetic mismatch and timed-out children are reaped.
- [x] Keep the wrapper bounded and hermetic: its temporary `systemctl` accepts
  only the exact `--user show` unit contract and `--user show-environment`,
  with no access to the real user systemd manager.

This remains controlled production-collector coverage only. It does not
validate a live Hyprland session, install or start providers, run active
Screenshot or ScreenCast probes, create a Hyprland support claim, or change
the Phase 8, KDE or Sway blocker state.

## Controlled Niri mixed-backend + duplicate-Settings slice — 2026-09-09

- [x] Add an upstream-shaped Niri Wayland fixture with the pure `niri` session
  identity, desktop-specific `niri-portals.conf`, `default=gnome;gtk`, and
  GNOME/GTK descriptor fixtures. The Secret/gnome-keyring entry remains out of
  scope because the current model does not have a Secret-service-specific
  runtime/unit contract.
- [x] Verify the generic resolver's actual capability result: GNOME serves
  ScreenCast, Screenshot, FileChooser and Settings, while explicit
  Access/Notification fallback entries select GTK. A modern configured
  interface/default preference takes precedence over legacy `UseIn`; a
  no-preference route still uses the generic `UseIn` fallback.
- [x] Add passive healthy, missing-Wayland, selected-GNOME-missing and
  selected-GTK-missing aggregates. They remain respectively clean, only
  `ENV003`, or only generic `DBUS002`; mixed installation alone is not a
  duplicate/config finding.
- [x] Add a higher-precedence effective `Settings=gtk` config and retain a
  lower generic `default=gnome;gtk` file only as a candidate. The collector
  selects the first existing file without merging preferences; the resolver
  therefore selects only GTK for Settings, leaves ScreenCast on GNOME and
  emits no `CFG004`.
- [x] Add a typed `portal_frontend` version-evidence section separate from the
  OS `SystemInfo.version_id`. Prefer a bounded frontend `--version` command
  from PATH or a standard installed executable location, otherwise use the
  source-qualified supported `dpkg-query` package metadata path; keep raw
  token, numeric version and provenance together and fail closed for
  malformed/uncomparable output.
- [x] Add the permanent controlled parser/collector matrix for exact `1.22.0`,
  distro revisions, newer versions, malformed/nonzero/missing/timeout and
  oversized output, including child reaping and exact fake-tool argv guards.

This is controlled static/passive compatibility coverage only. It does not
validate a live Niri session, install providers, run active probes, create a
Niri support claim or change the Phase 8, KDE or Sway blocker state.

## Controlled Niri production runtime + activation integration gate — 2026-09-09

- [x] Run the canonical pure-`niri` environment, selected effective
  `niri-portals.conf`, parsed GNOME/GTK descriptors and generic route resolver
  through `selected_backend_dbus_names()`.
- [x] Feed those descriptor-derived names and the exact frontend/GNOME/GTK
  units through production `dbus::collect()` and `systemd_user::collect()` in
  a private `dbus-run-session` with a temporary guarded fake `systemctl`.
- [x] Feed the same process fixture through production
  `activation_environment::collect()`, generic environment comparison and the
  existing passive rule engine.
- [x] Cover healthy GNOME+GTK ownership/active units; GNOME missing/not-found;
  GNOME failed; GTK missing/not-found; stale activation desktop; stale or
  missing activation `WAYLAND_DISPLAY`; and unavailable/timed-out activation.
  Results remain clean, generic `DBUS002`, generic `ENV004`, or no synthetic
  mismatch according to the existing contract.
- [x] Re-run the higher-precedence Settings regression at this integration
  level. The selected Niri file alone is effective, lower generic preferences
  are not merged, Settings selects GTK, capture interfaces select GNOME, the
  runtime set remains GNOME+GTK, and no `CFG004` is emitted.
- [x] Keep the harness fail-closed and bounded: only the exact allowed
  `systemctl --user show`/`show-environment` argv is accepted, no real user
  systemd is reachable, the outer wrapper is time-bounded, and timeout
  children are reaped.

This is controlled production-collector integration coverage, not live Niri
validation, active Screenshot/ScreenCast validation, a Niri support claim or
release approval. It does not change the Phase 8, KDE or Sway blocker state.

## Controlled Niri/XDP #2033 version-evidence gate — 2026-09-09

- [x] Integrate the additive `portal_frontend` section into the production
  `collect_snapshot()` and shareable JSON/Markdown report paths without
  reusing the OS version or portal interface version.
- [x] Use a numeric `SemanticVersion` comparison and preserve source
  provenance; the current Ubuntu host reports `1.21.1` from the installed
  frontend executable, so it is not an affected exact-version match. The
  `dpkg-query` fallback preserves distro revisions in its raw evidence.
- [x] Add `XDP006` only for exact normalized `1.22.0` plus pure Niri, effective
  selected `Settings=gtk`, a valid lower-priority candidate whose effective
  Settings behavior includes GNOME, and both GNOME/GTK Settings-capable
  descriptors. Lower candidates are typed non-effective metadata and are
  never merged into resolver preferences. The finding says known compatibility
  risk and never claims an observed duplicate SettingsChanged conflict.
- [x] Add negative coverage for absent, non-GNOME, malformed and unreadable
  lower candidates; all fail closed without changing generic `CFG004`.
- [x] Cover silent negative cases for missing/uncomparable/non-affected
  versions, other desktops, canonical default routing, single descriptors and
  GNOME+GTK installation without the explicit effective override. Existing
  `CFG004` semantics remain unchanged.

This is controlled version-evidence and compatibility diagnosis only. It does
not validate a live Niri session, observe a real SettingsChanged conflict,
install/update packages, run active probes, create a support claim or approve
a release. Phase 8, KDE and Sway blocker state is unchanged.

## Phase 11 live Hyprland/Niri readiness checkpoint — 2026-09-09

- [x] Perform a read-only host preflight without changing the desktop,
  packages or services. The host is `ubuntu:GNOME` on Wayland with healthy
  PipeWire/WirePlumber and GNOME passive Screenshot/ScreenCast routing.
- [x] Classify Hyprland separately as `BLOCKED / NOT AVAILABLE`: no real
  Hyprland session, backend package/binary/descriptor, active user unit or
  canonical D-Bus owner is present.
- [x] Classify Niri separately as `BLOCKED / NOT AVAILABLE`: no real Niri
  session, backend package/binary/descriptor, active user unit or canonical
  D-Bus owner is present.
- [x] Keep the current GNOME passive route and media-stack evidence separate
  from target capability evidence; do not treat it as Hyprland/Niri validation.
- [x] Leave active Screenshot/ScreenCast validation and support/release claims
  open. Do not retry this checkpoint on the same GNOME host.

Recheck trigger for either target: a real target Wayland session, the matching
backend package/descriptor and healthy user service, canonical D-Bus ownership,
the expected passive Screenshot/ScreenCast route, and the required advertised
portal capability evidence. Controlled Phase 11 development coverage is
complete, but these live/active gates are not release approval and do not alter
the Phase 8, KDE or Sway blocker state.

## Tasks

- Hyprland backend model/fixtures,
- Niri mixed GNOME/GTK fixtures,
- duplicate-provider diagnostics,
- version-aware compatibility knowledge where justified,
- ScreenCast probes on at least one target.

## Important rule principle

Do not automatically label “multiple backends installed” as an error.

Diagnose only evidence-backed conflicts or unusable routing.

## Release

**v0.5.0** target, depending on actual scope.

---

# Phase 12 — Safe Remediation Preview

## Objective

Evaluate whether PortalDoctor should offer controlled fixes without becoming a destructive support script.

This phase is optional and should happen only after diagnosis quality is mature.

## Design

```text
finding
 -> proposed remediation
 -> explanation
 -> dry-run
 -> explicit user approval
 -> apply
 -> verification
```

Example:

```bash
portaldoctor fix ENV004 --dry-run
```

Possible output:

```text
Would import:
  WAYLAND_DISPLAY
  XDG_CURRENT_DESKTOP
into the systemd user activation environment.

Files modified: none
```

## Prohibited behavior

No silent:

- package removal,
- config deletion,
- service restart loops,
- systemwide edits,
- privileged changes.

## Exit criteria

No remediation ships unless it has:

- deterministic applicability,
- dry-run representation,
- tests,
- post-apply verification,
- clear rollback or non-destructive semantics.

---

# Phase 13 — v1.0 Hardening

## Objective

Stabilize PortalDoctor as a dependable open-source Linux diagnostic tool.

## Required v1.0 gates

### Product

- stable default diagnostic UX,
- documented scope and non-goals,
- no misleading “fix all Linux” claims.

### Compatibility

Validated matrix includes at least:

- GNOME,
- KDE Plasma,
- Sway/wlroots,
- Hyprland or Niri.

### Diagnostics

- portal routing,
- environment/activation environment,
- D-Bus runtime,
- systemd user services,
- PipeWire/WirePlumber,
- journal evidence,
- active core probes.

### Contracts

- stable finding semantics,
- versioned JSON schema,
- documented exit codes,
- report privacy contract.

### Engineering

- broad fixture suite,
- release tests,
- dependency/security review,
- x86_64 Linux release artifact,
- ARM64 Linux release artifact,
- checksum generation,
- `.deb` packaging or equivalent documented install path.

## Release

**v1.0.0**

---

# Release Mapping Summary

| Release | Main scope |
|---|---|
| v0.1.0 | Core passive doctor, Ubuntu/GNOME/Wayland, portal routing, D-Bus/runtime basics |
| v0.2.0 | PipeWire/WirePlumber, journal evidence, privacy-aware reports |
| v0.2.1 | Exit-code contract, locked CI/package smoke, readable demo and release alignment |
| v0.3.0 | Active FileChooser/Screenshot/ScreenCast probes |
| v0.4.0 | KDE Plasma validation |
| v0.5.0 | wlroots/Sway + modern compositor expansion, possible safe-fix preview |
| v0.6+ | Compatibility/rule expansion, packaging/hardening |
| v1.0.0 | Stable contracts, documented compatibility matrix, production-quality release |

---

# Recommended Immediate Implementation Slice (historical)

> **Archived bootstrap guidance — do not execute this section as the current
> task list.** It records the original project-start sequence. The active next
> task is Phase 8 / v0.3.0; use `PORTALDOCTOR_CURRENT_STATE.md` and Issue #3.

This section records the original project bootstrap sequence. Phases 0–7 are
complete and shipped across v0.2.0/v0.2.1; the next implementation slice is
Phase 8 active portal probes.

The first Codex implementation goal should be:

```text
Phase 0
+
Phase 1 minimal vertical slice
```

Concretely:

1. initialize the Rust CLI and CI,
2. define snapshot/finding/status models,
3. collect OS/session/environment,
4. collect selected systemd user environment keys,
5. implement `ENV001-ENV004`,
6. render terminal + JSON,
7. add fixtures/tests,
8. run on the actual Ubuntu/GNOME/Wayland machine.

Only after this vertical slice is clean should Phase 2 portal routing begin.

This prevents the project from turning into a large untested parser collection before the diagnostic architecture is proven.
