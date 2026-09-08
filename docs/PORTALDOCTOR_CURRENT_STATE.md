# PortalDoctor — Current State and Handoff

**Last verified:** 2026-09-09
**Current public release:** `v0.2.1`
**Current development phase:** Phase 8 / `v0.3.0`
**Primary next issue:** [#3 — Active FileChooser, Screenshot and ScreenCast probes](https://github.com/SetraTheXX/Portal-Doctor/issues/3)

This is the canonical handoff document for a new maintainer or coding agent.
Read it before changing code, documentation or GitHub planning metadata.

## One-minute project summary

PortalDoctor is a passive, read-only Rust CLI that explains Linux desktop
portal failures by correlating session/environment state, XDG portal routing,
D-Bus and systemd user services, PipeWire/WirePlumber health, optional bounded
journal evidence and shareable reports.

The published `v0.2.1` line is a stabilized passive diagnostic product. The
development branch now contains explicit, bounded FileChooser and Screenshot
active probes; neither is part of the published `v0.2.1` release. Screenshot
has capability-negotiated v3 Window and trusted-GNOME v2 interactive paths;
both are controlled/fake-tested, but the real-session release gate remains
open because of the external GNOME provider blocker. ScreenCast now has
internal, non-public `CreateSession`, `SelectSources`, `Start`,
`StreamsReturned` and `OpenPipeWireRemote` adapters with controlled ownership,
cleanup, typed-payload and direct-FD evidence, including fail-closed cleanup
for malformed success payloads. The five slices also pass one aggregate
controlled lifecycle gate covering cross-stage failure, cleanup ordering,
privacy and open-resource assertions. Its real-session gate is separately
**BLOCKED** because the live frontend reports `AvailableSourceTypes=0` and
does not advertise the required Window bit `2`; no ScreenCast UI E2E has been
run in that state. PipeWire handshake/media and the public command remain
outside the implemented boundary.

## Release and repository state

- `v0.2.1` is published on [crates.io](https://crates.io/crates/portaldoctor)
  and in the [GitHub release](https://github.com/SetraTheXX/Portal-Doctor/releases/tag/v0.2.1).
- The release includes the documented exit-code contract, locked package and
  install smoke coverage, the readable four-scene demo, a Linux x86_64 binary
  and a SHA256 checksum asset.
- `main` is the active branch. Before starting work, verify
  `git status --short --branch`, `git log -1 --oneline --decorate` and
  `git diff --check`.
- The default product path remains passive, read-only, rootless and bounded.

## Completed roadmap scope

- Phases 0–4: project foundation, environment/session discovery, portal routing,
  D-Bus/systemd verification and the v0.1 diagnostic engine.
- Phases 5–7: bounded PipeWire/WirePlumber evidence, opt-in journal evidence,
  privacy-aware terminal/JSON/Markdown reports and the v0.2.0 release.
- v0.2.1: exit-code semantics, locked package/install gates, release assets,
  demo/release alignment and final passive-path stabilization.

Do not repeat the historical Phase 0–7 bootstrap work unless a regression is
demonstrated by a test or a supported user report.

## Validated scope and boundaries

The documented and validated baseline is:

- Ubuntu 26.04
- GNOME, including the `ubuntu:GNOME` identifier
- Wayland
- systemd user session
- `org.freedesktop.portal.Desktop`

Other distributions and desktops may work, but they are not v0.2 support
claims. The following remain outside the published v0.2.1 boundary:

- the unreleased development FileChooser and Screenshot probes,
- the unreleased ScreenCast active probe (only internal bounded lifecycle
  slices exist; no public command or release approval),
- validated KDE, wlroots/Sway, Hyprland or Niri behavior,
- automatic fixes,
- GUI workflows.

The docs.rs build warning is expected for this binary-only package; local binary
documentation generation succeeds and PortalDoctor does not promise a library
API.

## Current next step: Phase 8 / v0.3.0

The detailed executable checklist is [GitHub Issue #3](https://github.com/SetraTheXX/Portal-Doctor/issues/3).
The ASHPD decision checkpoint is recorded in
[`PORTALDOCTOR_ASHPD_DECISION.md`](PORTALDOCTOR_ASHPD_DECISION.md).
The implementation and release gates are intentionally separate. FileChooser
is complete, Screenshot controlled implementation is complete, and Screenshot
real GNOME success/cancellation remains an external release blocker. ScreenCast
controlled/internal implementation is also complete, but its real success and
portal-native cancellation gate is **BLOCKED** by the live provider capability
absence (`AvailableSourceTypes=0`, Window bit `2` missing). Both real-session
gates block v0.3.0 release approval. Neither blocker reopens completed
internal work or freezes Phase 8 indefinitely: independent bounded work, and a
next development phase when it does not depend on these real gates, may
proceed. Public active-probe commands and release approval remain blocked.
Do not retry either provider state until its exact capability/stability trigger
is met, and do not treat a ScreenCast implementation or fake result as
real-session evidence. See
[`PORTALDOCTOR_SCREENCAST_DECISION.md`](PORTALDOCTOR_SCREENCAST_DECISION.md)
for the re-evaluation trigger.

The first independent Phase 9 development slice now has deterministic KDE
Plasma `kde-portals.conf` and `kde.portal` fixtures covering desktop-specific
candidate precedence, metadata parsing, default route selection, `UseIn`
exclusion and the desktop-specific configuration rule. This is static parser
and resolver coverage plus controlled runtime-correlation tests for the
generic backend-unit mapping, systemd state parsing and D-Bus ownership/finding
semantics for `org.freedesktop.impl.portal.desktop.kde`. It does not add a KDE
runtime/support claim, active probe, or release approval; controlled
KDE/Plasma Wayland session identity, activation-environment mismatch and
missing-`WAYLAND_DISPLAY` coverage is also fixture-only, and live Plasma
validation remains a later gate. A production-pipeline-style aggregate passive
snapshot now composes the KDE routing, environment, D-Bus and systemd fixtures:
healthy state is finding-free, while missing or failed KDE runtime state emits
only the existing generic `DBUS002` finding.
An additional isolated `dbus-run-session` gate now calls the production D-Bus
collector directly, proving the KDE well-known name `HasOwner`/`NoOwner` paths
and sorted/deduplicated selected-name behavior without starting a portal.
An additional explicit ignored gate places a guarded fake `systemctl` in a
temporary PATH prefix and calls the production systemd-user collector directly.
It proves the exact KDE unit invocation and active/failed/not-found/timeout
mapping, including reaping a timed-out child, without contacting the real user
systemd manager.
The production D-Bus and systemd collectors are also exercised together under
one isolated `dbus-run-session` plus guarded fake-systemctl wrapper and fed into
the existing KDE routing/rule pipeline. Healthy owned/active state is finding-
free; absent/not-found and absent/failed states produce only `DBUS002`.
This remains controlled passive coverage and does not claim a live Plasma
session or KDE support.

### Phase 9 live Plasma preflight checkpoint — 2026-09-08

The current host is Ubuntu 26.04.1 on Wayland with `XDG_CURRENT_DESKTOP`
`ubuntu:GNOME`, `XDG_SESSION_DESKTOP` `ubuntu` and `gnome-shell`; it is not a
Plasma session. The KDE backend package is not installed,
`xdg-desktop-portal-kde.service` is `not-found`/inactive, the canonical KDE
D-Bus name has no owner, and the passive route selects GNOME for FileChooser,
Screenshot and ScreenCast. Live Plasma validation is therefore
**BLOCKED / NOT AVAILABLE**. The host must not be repeatedly forced into this
gate, and no active probe or UI validation was attempted.

Re-evaluate this gate only after a real Plasma Wayland session is available,
the KDE backend/package and `xdg-desktop-portal-kde.service` are installed and
healthy, and `org.freedesktop.impl.portal.desktop.kde` has a D-Bus owner. This
checkpoint does not change the Phase 8 Screenshot or ScreenCast release gates.

### Phase 10 first bounded static slice

Controlled Sway/wlroots fixtures now cover a desktop-specific
`sway-portals.conf`, `xdg-desktop-portal-wlr` and GTK descriptors, mixed
Screenshot/ScreenCast → `wlr` routing, FileChooser/Settings → GTK fallback,
desktop-config precedence and quiet provider/duplicate rule evaluation. This
is parser/resolver/rule coverage only; no live Sway session, active probe,
provider installation or wlroots support claim follows from it. A controlled
passive aggregate now also combines the Sway Wayland environment fixture with
the mixed route table: a complete `WAYLAND_DISPLAY` run is finding-free, while
its absence produces only the existing `ENV003` finding. This remains fixture
coverage, not live Sway validation or a support claim.

### Phase 10 controlled runtime-correlation slice

The generic runtime correlation path is covered against the same mixed Sway
fixture: `wlr` resolves to
`org.freedesktop.impl.portal.desktop.wlr` and
`xdg-desktop-portal-wlr.service`, while `gtk` resolves to
`org.freedesktop.impl.portal.desktop.gtk` and
`xdg-desktop-portal-gtk.service`. Healthy owner/unit states are silent;
missing or failed `wlr`, and missing GTK fallback runtime, produce only the
existing generic `DBUS002` finding. This is controlled snapshot/rule coverage,
not a live Sway runtime, provider or active-probe validation.

The production collectors are also covered together by an explicit ignored
`dbus-run-session` plus guarded fake-`systemctl` aggregate gate. It feeds the
descriptor-derived WLR/GTK D-Bus names and the three exact portal unit names
through `dbus::collect()` and `systemd_user::collect()` before evaluating the
existing passive rule pipeline. Healthy, WLR-missing, GTK-fallback-missing and
WLR-failed states are covered; no real user systemd manager, portal provider or
active probe is contacted.

This controlled Phase 10 slice exercises the production
`activation_environment::collect()` path with the same Sway mixed-routing
snapshot. Equal Sway/Wayland values are clean; stale desktop, stale Wayland
and missing activation `WAYLAND_DISPLAY` produce only generic `ENV004`, while
unavailable and timed-out activation collection suppresses mismatch findings
and preserves the normal environment note/status behavior. The integration
harness bounds and reaps the fake `systemctl` child and does not validate a
live Sway session or create a support claim.

### Phase 10 live Sway readiness and active-probe decision checkpoint — 2026-09-09

The read-only preflight found Ubuntu 26.04.1 on Wayland with
`XDG_CURRENT_DESKTOP=ubuntu:GNOME`, `XDG_SESSION_DESKTOP=ubuntu` and
`WAYLAND_DISPLAY=wayland-0`; no Sway compositor/session was present. The
`xdg-desktop-portal-wlr` package/binary and user unit were absent/not-found,
and `org.freedesktop.impl.portal.desktop.wlr` had no D-Bus owner. Passive
portal routing selected GNOME for Screenshot and ScreenCast; the portal
frontend, GNOME/GTK backends, PipeWire and WirePlumber were running. The
frontend-only read-only capability values were ScreenCast version `5` with
`AvailableSourceTypes=0` and Screenshot version `2` without `AvailableTargets`,
matching the existing GNOME capability blockers. No active probe or portal UI
was opened.

The decision is **LIVE SWAY ENVIRONMENT BLOCKED / NOT AVAILABLE** and
**ACTIVE PROBE READINESS BLOCKED**. This is an environment/provider absence,
not a newly observed PortalDoctor production bug. The active-probe audit found
no GNOME assumption leaking into the WLR route: Screenshot uses the public
frontend, keeps the v2 compatibility path GNOME-only, and requires the v3
Window bit; the internal ScreenCast lifecycle also uses the public frontend
and fails closed when `AvailableSourceTypes` lacks Window bit `2`. There is no
public ScreenCast command, backend-direct call or WLR support claim.

Re-open live WLR active validation only after a real Sway Wayland session is
available, the WLR backend/package and `xdg-desktop-portal-wlr.service` are
installed and healthy, the canonical WLR D-Bus name has an owner, passive
routing selects `wlr` for Screenshot/ScreenCast, and the frontend advertises
the required Window capabilities (`AvailableTargets & 2` for Screenshot v3
and `AvailableSourceTypes & 2` for ScreenCast) with PipeWire/WirePlumber
ready. Do not repeatedly force the current GNOME host. This checkpoint does
not change the Phase 8 GNOME Screenshot or ScreenCast blockers.

Implement and release-gate it in this order:

1. [x] Evaluate and record the ASHPD integration strategy and its compatibility
   implications. The accepted boundary is PortalDoctor-owned lifecycle control
   with ASHPD used only where its public API preserves the required request and
   cleanup observability.
2. [x] Define a stable machine-readable `ProbeResult` contract before adding
   user-facing findings. The standalone v1 shape is documented in
   [`probe-result-schema.md`](probe-result-schema.md) and implemented at
   `src/model/probe.rs`; constructor and serde validation reject schema-version
   mismatches, contradictory cleanup states and probe/stage/resource
   combinations outside the v1 matrix. It is not embedded in the passive
   report yet.
3. [x] Implement the first bounded FileChooser probe only. The command is
   `portaldoctor probe filechooser`; it uses a direct zbus lifecycle adapter,
   a per-request `handle_token`, central request/recovery/response/cleanup
   timeouts and standalone `ProbeResult` JSON.
4. [x] Add protocol fixture coverage for success, user cancellation, request
   and response timeout, unavailable/unsupported, malformed response and
   infrastructure failure, including response-terminal no-close and
   no-response `Request.Close` assertions. The
   response match is installed before `OpenFile`; a late method reply is
   recovered within a bounded grace period, and the token-derived path is
   closed when the reply never arrives. A matching `Response` ends the request
   and is never followed by `Request.Close`. An unresolved transport race is
   reported as `unverified`, never claimed clean, and no implicit fallback is
   performed. The reproducible controlled portal is
   [`scripts/validate-filechooser-fake.py`](../scripts/validate-filechooser-fake.py).
5. [x] Validate both cancellation and successful selection in one real
   supported desktop session before expanding the active-probe implementation
   sequence.
6. [x] Make the controlled FileChooser lifecycle harness a permanent CI gate.
   CI runs every fake-portal mode and asserts the machine result, process exit,
   URI redaction, response-terminal no-close behavior and observed
   no-response `Request.Close` count, including the explicit no-request
   cleanup boundary.
7. [x] Require OS-provided entropy for every `handle_token`. Entropy failure
   fails closed as `infrastructure_failure` before `OpenFile`; no PID/time
   fallback is allowed.
8. [x] Define the Screenshot probe's lifecycle, target policy, privacy
   boundary and release/validation gates in
   [`PORTALDOCTOR_SCREENSHOT_DECISION.md`](PORTALDOCTOR_SCREENSHOT_DECISION.md).
9. [x] Implement the explicit Screenshot lifecycle using the shared
   PortalDoctor-owned zbus request mechanics. The command negotiates v3+
   Window bit `2` or a trusted GNOME v2 interactive path; v2 sends no target
   and makes no Window-only claim.
10. [x] Add the permanent controlled Screenshot matrix for both capability
    shapes: success, cancellation, request/response timeout, late reply,
    malformed response, portal/transport failure, unsupported target/version,
    untrusted v2, unavailable service and cleanup failure. It asserts exact
    options, response-terminal no-close/no-response close calls,
    open-request state, exit code, result shape and privacy redaction.
11. [ ] Validate Screenshot success and cancellation in a real supported
    Ubuntu 26.04 + GNOME + Wayland session using the negotiated path. The
    installed provider exposes v2 without `AvailableTargets`; a forced
    cancellation produced a clean PortalDoctor result but was followed by a
    GNOME backend `SIGSEGV`, so this remains an external blocker and no real
    E2E is being rerun in the offline hardening task.
12. [x] Record the bounded ScreenCast design and acceptance checkpoint in
    [`PORTALDOCTOR_SCREENCAST_DECISION.md`](PORTALDOCTOR_SCREENCAST_DECISION.md).
    This permits a separate development slice while keeping the Screenshot
    real-session gate open; it does not add a public ScreenCast command or
    approve the v0.3.0 release.
13. [x] Implement the internal `CreateSession` adapter and Session ownership
    boundary only. Controlled fake coverage proves response-match-before-call,
    OS-entropy tokens, terminal-response zero `Request.Close`, bounded
    no-response close, exactly-once `Session.Close`, expected-path cleanup for
    malformed success payloads, fail-closed ownership and privacy.
    `SelectSources`, PipeWire and real ScreenCast E2E remain out of scope for
    this slice.
14. [x] Implement the internal `SelectSources` adapter only. It negotiates
    the advertised Window source bit, sends exactly `types=2`,
    `multiple=false` and a fresh entropy-backed request token without
    persistence/restore options, then closes every acquired Session exactly
    once. Controlled coverage includes terminal-response zero `Request.Close`,
    bounded no-response cleanup, capability rejection, malformed/transport
    outcomes, request/session cleanup failures, aggregate cleanup resources
    and privacy. `StreamsReturned`, PipeWire and real ScreenCast E2E remain out
    of scope for this slice.
15. [x] Implement the internal `Start` adapter only after successful
    `CreateSession` and `SelectSources`. It reuses the preinstalled response
    match, sends the same owned Session handle, a fresh entropy-backed request
    token and explicit `parent_window=""`, and never decodes the Start
    `streams` payload. Controlled coverage proves terminal-response zero
    `Request.Close`, bounded no-response cleanup, exact arguments, aggregate
    Request/Session cleanup failures, exactly-once Session cleanup, raw-stream
    privacy and no open resources. `StreamsReturned`, PipeWire and real
    ScreenCast E2E remain out of scope.
16. [x] Implement the internal `StreamsReturned` boundary only after a
    successful Start Response. Accept only the XDG `a(ua{sv})` stream
    container with exactly one Window stream for the existing `multiple=false`
    policy; keep node IDs and properties opaque and out of results, logs and
    state. Missing/wrong-type/empty/multiple/malformed or contradictory
    `source_type` payloads are `malformed_response`; terminal Start responses
    never trigger `Request.Close`, and the owned Session is still closed
    exactly once. Controlled coverage includes cleanup failure and privacy.
    PipeWire handshake/media and real ScreenCast E2E remain out of scope.
17. [x] Implement the internal `OpenPipeWireRemote` direct-FD boundary only
    after a valid Window stream result. Send empty options with no
    `handle_token`, deserialize only into a typed owned FD, close the FD before
    exactly-once Session cleanup, and keep late/ambiguous ownership fail-closed
    as `unverified/pipe_wire_remote`. Controlled coverage proves direct-method
    timeout/cancellation recovery, late FD release, cleanup ordering, FD and
    Session failure aggregation, zero Request.Close, no open resources and
    privacy. PipeWire handshake/media, a public command and real E2E remain out
    of scope.
18. [x] Run the aggregate internal ScreenCast lifecycle gate across all five
    slices. The controlled matrix covers full success, representative
    cross-stage failures, Request/Session/FD cleanup failures and aggregation,
    terminal-response zero-close behavior, late/ambiguous direct-FD ownership,
    FD-before-Session ordering, exact call counts, privacy and no-open-resource
    assertions. The gate also confirms the public ScreenCast command remains
    hidden.
19. [ ] Validate the complete internal ScreenCast lifecycle in a disposable
    supported Ubuntu 26.04 + GNOME + Wayland session, then make a separate
    release/public-command decision. The current read-only preflight reports
    interface `version=5` but `AvailableSourceTypes=0` (Window bit `2` absent),
    so this item is currently **BLOCKED** by genuine provider capability
    absence and no real UI E2E is run. Re-evaluate only after the public bit is
    advertised, the provider/frontend is stable, and one success plus one
    portal-native cancellation E2E can be performed. Do not count fake portal
    evidence as complete or expose a public command before it passes.

Real-session validation on 2026-09-06 used the release binary in the current
Ubuntu 26.04 + GNOME + Wayland + systemd user session. An explicit
`probe filechooser --json` run was cancelled with `Ctrl-C` while the portal
interaction was active and produced `user_cancelled`, `stage: complete`,
`cleanup.status: completed` and process exit `1` under the earlier lifecycle
implementation. A second run selected an existing file through the GNOME
chooser and produced `success`, `stage: complete`, `cleanup.status: completed`
and process exit `0` under that same earlier implementation. For FileChooser
and Screenshot, the current XDG invariant reports the Request portion as
`cleanup.status: not_required` for a terminal `Response`; only no-response
abort paths require Request cleanup. ScreenCast additionally tracks any
independently possible Session ownership, so malformed terminal
`CreateSession` success is never silently reported as `not_required`.
Neither run emitted a URI, filename or file content. A separate no-session-bus
run produced `unavailable` within the bounded setup window.

The same session reports Screenshot interface version `2` and no
`AvailableTargets` property. The implementation can use its v2 interactive
path only when the GNOME route/provider evidence is independently verified;
that path makes no Window-only claim. The observed forced cancellation is not
release evidence because the provider crashed after the clean PortalDoctor
result.

The provider limitation was rechecked on 2026-09-06, rather than inferred
from the application result: the host has Ubuntu packages
`xdg-desktop-portal 1.21.1+ds-1ubuntu3` and
`xdg-desktop-portal-gnome 50.0-0ubuntu1`, while the upstream XDG portal
changelog records Screenshot target selection in frontend `1.21.2` and the
current GNOME backend source still advertises implementation version `2`.
The provider crash is an external stability blocker, not a PortalDoctor
serialization or controlled-lifecycle test failure. A controlled fake portal
or a wlroots backend would not be evidence for the required GNOME session, and
no real GNOME E2E is rerun in the offline hardening task.

The crash evidence is specific: after a forced cancellation, the provider
logged `InteractiveScreenshot didn't return a file` and exited with `SIGSEGV`
from `/usr/libexec/xdg-desktop-portal-gnome`. The controlled v2 success,
known-handle cancellation, unknown-handle cancellation, timeout, malformed,
transport and cleanup-failure matrix passes without this provider failure.

On 2026-09-07 the provider was read-only healthy before one success-path
attempt: Ubuntu 26.04.1, GNOME/Wayland, GNOME Screenshot routing and the live
backend owner were all present. The portal UI did not complete a selection;
PortalDoctor returned `timed_out`, `stage: response`,
`cleanup.status: completed`, exit `1`, and emitted no artifact data. The
backend then logged `InteractiveScreenshot didn't return a file` and crashed
with `SIGSEGV`/core-dump. The provider service is currently failed and was not
restarted. This attempt is neither Screenshot success nor portal-native
cancellation evidence, and the second E2E was deliberately not attempted.

`ProbeResult` v1 has no `provider_crashed` or `externally_blocked` status. The
observed result therefore remains the truthful operation result
`timed_out/response` with verified request cleanup; PortalDoctor must not infer
a provider crash from timeout alone. A future bounded post-abort health check
may add a sanitized human-facing diagnostic only when independent backend
owner/service evidence confirms the loss. A machine-readable provider-crash
field belongs in a future schema version, not an enum addition hidden inside
v1. Until then, the provider incident is an external release blocker, not a
new success/cancellation state.

References: [XDG portal changelog](https://github.com/flatpak/xdg-desktop-portal/blob/main/NEWS.md),
[GNOME Screenshot backend](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/screenshot.c#L349).

Active probes must never run from `portaldoctor` or `portaldoctor check` by
default. They must clearly warn about possible dialogs, remain rootless, use
the central timeout policy, honor a terminal `Response` without a later
`Request.Close`, and abort only no-response request/session resources on
cancellation or failure. FileChooser never reads selected content; Screenshot
never reads the image, but the portal may create a portal-managed artifact that
`Request.Close` does not delete.

### First bounded task acceptance criteria

The first task is complete only when all of the following are true:

- the ASHPD strategy and its trade-offs are recorded in the implementation
  documentation,
- the probe result states and JSON shape are stable enough to test,
- `portaldoctor probe filechooser` is explicit and does not affect passive
  commands,
- success, cancellation, timeout, unavailable service and malformed response
  are distinguishable and covered by fixtures or mocks,
- no selected file is read, copied or modified,
- no-response request resources are closed on every abort path; a matching
  `Response` ends the request without `Request.Close`; token-derived cleanup is
  attempted on a late/unanswered request and ambiguous transport outcomes are
  explicitly `unverified`,
- the supported real-session validation path is documented,
- `cargo fmt --check`, strict locked Clippy, locked tests, locked release build,
  locked package and clean-root install smoke all pass.

### Screenshot acceptance checkpoint

The Screenshot slice is implementation-complete in the controlled environment
only. Its release gate additionally requires:

- capability negotiation: v3+ introspection and AvailableTargets Window-bit
  enforcement with no Screen/Area/Active-Window fallback, or trusted GNOME v2
  interactive mode with no `target` and no Window-only claim,
- a warning before the portal call and no URI/path/image output or persistence,
- bounded success, cancellation, request/response timeout, late-reply,
  malformed, transport, unsupported and cleanup-failure coverage,
- a real Ubuntu 26.04 + GNOME + Wayland session proving success and
  cancellation for the negotiated path without opening or inspecting the
  generated image, and
- the full locked package/install, passive regression, audit and CI gates.

This is a conditional provider gate, not a request to retry the same broken
stack indefinitely. Re-evaluate it only after the GNOME/XDG provider or
frontend version changes, or in a disposable session that supplies a fixed
provider. Keep the v3 Window-only implementation intact and keep v2
unreleased. The gate blocks v0.3.0 release approval, but it does not block the
separate ScreenCast development checkpoint below.

### ScreenCast external capability checkpoint

The current ScreenCast preflight is a genuine provider capability absence at
the public frontend boundary, not an inferred PortalDoctor failure: the live
interface is version `5`, but `AvailableSourceTypes=0`, so Window bit `2` is
missing. Routing selects the GNOME provider, the provider/frontend services
and PipeWire/WirePlumber are running, and the sanitized read-only evidence did
not show a ScreenCast registration or initialization error. The separate
Screenshot provider crash remains an independent blocker. The Window-only
policy therefore stays fail-closed; `MONITOR` fallback is not allowed.

This gate must not be retried in the same provider state. Re-open it only when
`AvailableSourceTypes & 2 != 0`, the selected provider and frontend are stable,
and a supported disposable session is ready for exactly one real success and
one portal-native cancellation E2E. Both runs, followed by the release and
regression gates, are required before a public ScreenCast command or v0.3.0
approval can be considered.

Do not implement all three probe families in the first slice and do not begin
desktop expansion or remediation as part of it.

The current change includes the Screenshot implementation and its controlled
fake audit. ScreenCast remains a separate bounded change and has completed all
five design-approved internal slices plus the aggregate controlled audit;
desktop compatibility work is still out of scope. Screenshot and ScreenCast
remain unreleased until their own real-session success/cancellation gates and
the final release gates pass. The observed GNOME Screenshot backend crash and
the current ScreenCast Window-capability absence keep those gates open.

### ScreenCast design and internal lifecycle checkpoints

The design boundary is recorded in
[`PORTALDOCTOR_SCREENCAST_DECISION.md`](PORTALDOCTOR_SCREENCAST_DECISION.md).
It fixes the lifecycle as `CreateSession -> SelectSources -> Start ->
StreamsReturned -> OpenPipeWireRemote`, reuses the audited Request/Response
invariants, assigns ownership to Request, Session and the returned PipeWire
FD independently, and requires reverse-order cleanup with fail-closed v1
`ProbeResult` mapping. The five internal slices now stop after the
`OpenPipeWireRemote` FD is released:
they validate Session ownership, negotiate the advertised Window capability,
send only `types=2`, `multiple=false`, a fresh request token and explicit
`parent_window=""`, and enforce exactly-once `Session.Close` without exposing
a public command. Start drops its `streams` result at the Start-only boundary;
`StreamsReturned` accepts only the typed XDG `a(ua{sv})` container with one
Window stream and does not retain its node ID or properties. The
`OpenPipeWireRemote` slice sends an empty options map, creates no Request,
closes a typed owned FD exactly once before Session.Close, and does not connect
to PipeWire or inspect media.
Malformed terminal success payloads use only the expected token-derived Session
path for best-effort cleanup and cannot end as silent `not_required`; foreign paths are
never closed. Controlled CreateSession, SelectSources and Start coverage proves
terminal-response zero Request.Close, bounded no-response cleanup, aggregate
Request/Session cleanup failures, exact arguments, raw-stream privacy and
exactly-once Session.Close. The StreamsReturned controlled matrix also proves
valid/minimal/opaque-property streams, fail-closed malformed and Window-policy
combinations, terminal-response zero Request.Close, Session cleanup failure
aggregation and sanitized state. The OpenPipeWireRemote matrix additionally
proves direct-method errors, timeout/cancellation/late-FD recovery, FD-before-
Session ordering, exactly-once release, cleanup aggregation and no raw FD
privacy. No PipeWire media access or real ScreenCast E2E was run. The internal
implementation gate is complete; the real-session gate is currently blocked
before UI execution by `AvailableSourceTypes=0` and the missing Window bit `2`.
After the capability/stability trigger, the next gate is exactly one real
success plus one portal-native cancellation, followed by the
release/public-command decision. The aggregate
controlled gate composes these boundaries from CreateSession through FD
release and Session.Close, covering representative cross-stage failures,
cleanup aggregation, terminal zero-close behavior and no-open-resource/privacy
assertions.

## Quality gates

Run the relevant gates from the repository root before reporting completion:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --release
cargo package --locked
install_root="$(mktemp -d -t portaldoctor-smoke.XXXXXX)"
cargo install --path . --locked --root "$install_root"
"$install_root/bin/portaldoctor" --version
PORTALDOCTOR_BIN=target/release/portaldoctor \
  python3 scripts/validate-v0.1-faults.py
./scripts/validate-screencast-create-session-ci.sh
./scripts/validate-screencast-select-sources-ci.sh
./scripts/validate-screencast-start-ci.sh
./scripts/validate-screencast-streams-returned-ci.sh
./scripts/validate-screencast-open-pipe-wire-remote-ci.sh
./scripts/validate-screencast-aggregate-ci.sh
```

For release-facing changes, also verify the GitHub Actions run, release asset
checksum and crates.io installation. Keep `cargo audit` clean when dependency
changes are introduced.

The active lifecycle audit additionally runs the controlled portal harness
inside an isolated session bus. The permanent CI entry point covers the v3
`success`, `cancel`, `malformed`, `response-timeout`, `late-reply`,
`request-timeout`, `transport-failure` and `unsupported` modes, the equivalent
trusted-v2 matrix, an untrusted-v2 fail-closed mode and close-failure fixtures
for the independent cleanup axis:

```sh
PORTALDOCTOR_BIN=target/release/portaldoctor \
  ./scripts/validate-filechooser-fake-ci.sh
PORTALDOCTOR_BIN=target/release/portaldoctor \
  ./scripts/validate-screenshot-fake-ci.sh
./scripts/validate-screencast-create-session-ci.sh
./scripts/validate-screencast-select-sources-ci.sh
./scripts/validate-screencast-start-ci.sh
./scripts/validate-screencast-streams-returned-ci.sh
./scripts/validate-screencast-open-pipe-wire-remote-ci.sh
./scripts/validate-screencast-aggregate-ci.sh
```

For one scenario during local debugging:

```sh
dbus-run-session -- python3 scripts/validate-filechooser-fake.py \
  --mode success -- target/release/portaldoctor probe filechooser --json
```

The CI wrapper repeats the harness for every listed mode. The harness asserts
the standalone JSON status, shell exit code, URI redaction, response-terminal
no-close behavior, and the expected no-response `Request.Close` observation.
Ambiguous real-transport paths remain `cleanup.status: unverified` rather than
claiming cleanup success. The ScreenCast wrappers additionally assert Session
close counts, open-resource state, token validity, exact Window-only and
headless Start options, stage-local Start success, typed StreamsReturned
acceptance/fail-closed rejection, synthetic-stream dropping and privacy without
creating a public command or a real portal session.

## Documentation and planning rules

- Update the README only when user-facing behavior, supported scope or release
  state changes; do not use it as the task tracker.
- Use Issue #3 for the v0.3.0 implementation checklist and its acceptance gate.
- Use the roadmap for phase boundaries and release mapping.
- Keep issue #7 as the high-level sequence tracker; its current baseline must
  mention v0.2.1 before v0.3.0.
- Preserve the checked-in demo’s role as a v0.2.1 passive diagnostic showcase;
  do not imply that it demonstrates active probes.
