# PortalDoctor — Current State and Handoff

**Last verified:** 2026-09-09
**Current public release:** `v0.2.1`
**Current development phase:** Phase 13 v1.0 Hardening — finding-semantics, versioned JSON schema, exit-code, shareable-report privacy, default-UX/scope and diagnostic-coverage contract slices COMPLETE
**Release gate under review:** Phase 8 / `v0.3.0` remains blocked
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

## Phase 8 / v0.3.0 release gate and independent development sequencing

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
candidate precedence, metadata parsing, default route selection, and legacy
no-preference `UseIn` behavior. This is static parser
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

### Phase 11 first bounded Hyprland compatibility slice — 2026-09-09

Controlled Hyprland fixtures now cover a desktop-specific
`hyprland-portals.conf`, the canonical
`org.freedesktop.impl.portal.desktop.hyprland` descriptor, GTK fallback and a
Hyprland Wayland environment. The existing generic resolver selects Hyprland
for Screenshot/ScreenCast and GTK for FileChooser/Settings; configured
preferences remain authoritative across desktop identities, while no-preference
fallbacks still apply legacy `UseIn`. The passive aggregate is finding-free when
both backends are healthy, emits only `ENV003` without `WAYLAND_DISPLAY`, and
keeps the routes unchanged while a missing Hyprland or GTK runtime emits only
the corresponding generic `DBUS002`.

This is controlled static/passive coverage only. Production code remains
generic; no live Hyprland session, provider installation, active probe,
support claim or release approval follows from it. Phase 8, KDE and Sway
blockers are unchanged.

### Phase 11 controlled production runtime + activation slice — 2026-09-09

The same Hyprland mixed-routing fixture is now exercised by an explicit,
isolated `dbus-run-session` plus guarded fake-`systemctl` gate. The production
descriptor/resolver output feeds `dbus::collect()`, `systemd_user::collect()`
and `activation_environment::collect()` before the existing environment
comparison and passive rule engine run. The fake tool is temporary, accepts
only the exact `--user show` portal-unit invocations and
`--user show-environment`, and cannot fall through to the real user systemd
manager.

Healthy frontend/Hyprland/GTK ownership and active units are finding-free.
Missing or failed Hyprland runtime produces only generic `DBUS002` with the
canonical Hyprland D-Bus name; missing GTK fallback likewise remains only
`DBUS002`, without changing the selected routes. Stale desktop or activation
Wayland values produce only generic `ENV004`; the process-side
`WAYLAND_DISPLAY` remains present so no synthetic `ENV003` is emitted.
Unavailable or timed-out activation collection does not invent a mismatch,
and the timeout child is reaped. Production collectors remain generic; this is
controlled production-collector coverage, not live Hyprland validation, an
active probe, a support claim or release approval. Phase 8, KDE and Sway
blockers are unchanged.

### Phase 11 controlled Niri mixed-backend + Settings slice — 2026-09-09

Controlled Niri fixtures now cover the upstream Wayland `niri` identity, the
upstream-shaped `default=gnome;gtk` ordering, explicit GTK handling for
Access/Notification, and GNOME/GTK descriptors. The generic resolver treats
the configured default as authoritative: GNOME is selectable for ScreenCast,
Screenshot, FileChooser and Settings even though its legacy `UseIn=gnome` does
not list Niri, while GTK serves the explicit fallback interfaces. With no
interface/default preference, the resolver retains the legacy `UseIn` filter;
this is generic resolver behavior, not a Niri-specific branch. The current
model intentionally omits Niri's Secret/gnome-keyring entry because it has no
Secret-service-specific runtime or unit contract.

The passive aggregate is clean with both selected runtime owners healthy,
emits only `ENV003` when `WAYLAND_DISPLAY` is absent, and emits only generic
`DBUS002` when the selected GNOME or GTK owner is missing. A separate
higher-precedence `Settings=gtk` file is the effective selected config; the
lower generic `default=gnome;gtk` file remains non-effective and is not
merged. Existing lower candidates are recorded separately with parse/read
status and parsed preferences for bounded compatibility evidence. Settings
resolves to GTK while capture remains on GNOME, and intentional multiple
installed descriptors do not produce `CFG004`.

The XDG Desktop Portal 1.22.0 / Issue #2033 behavior is represented as a
controlled regression fixture. The new additive `portal_frontend` snapshot
section keeps `xdg-desktop-portal` software evidence separate from the OS
`system.version_id`: it carries a raw version token, numeric
`normalized_version`, and source provenance. The collector prefers a bounded
frontend `--version` command from PATH or a standard installed executable
location, then falls back to supported `dpkg-query` metadata without using a
shell. The current host's selected frontend executable evidence is `1.21.1`
from `/usr/libexec/xdg-desktop-portal`; the package fallback also retains
distro revisions such as `1.21.1+ds-1ubuntu3` in controlled tests. The exact
`1.22.0` compatibility rule does not fire here. No wider version range is
inferred.

### Phase 11 controlled Niri production runtime + activation integration gate — 2026-09-09

The canonical pure-`niri` fixture now runs through the production collector
chain under a private `dbus-run-session` and a temporary, guarded fake
`systemctl`: selected effective config, parsed GNOME/GTK descriptors, generic
route resolution, `selected_backend_dbus_names()`, `dbus::collect()`,
`systemd_user::collect()`, `activation_environment::collect()`, environment
comparison and the passive rule engine. The fake accepts only the exact
frontend/GNOME/GTK `--user show` calls and `--user show-environment`; it cannot
fall through to the real user systemd manager.

The healthy canonical Niri stack is finding-free. Missing or failed GNOME
runtime yields only generic `DBUS002` with the canonical GNOME D-Bus name;
missing GTK fallback likewise remains only `DBUS002` and does not change the
routes. Stale activation desktop or `WAYLAND_DISPLAY` yields only generic
`ENV004`, while unavailable or timed-out activation produces no synthetic
mismatch and the timed-out child is reaped. The higher-precedence
`Settings=gtk` file is exercised as the complete effective config: Settings
selects GTK, capture interfaces remain on GNOME, the selected runtime set is
still GNOME+GTK, and no `CFG004` is emitted. This is controlled
production-collector coverage only; live Niri validation, active probes,
support claims and release approval remain open, and Phase 8/KDE/Sway
blockers are unchanged.

### Phase 11 xdg-desktop-portal version evidence + XDP #2033 gate — 2026-09-09

The version foundation is now implemented and bounded. Frontend executable
output is preferred from PATH or a standard installed location; on this Ubuntu
host `/usr/libexec/xdg-desktop-portal --version` provides `1.21.1`. The
supported `dpkg-query` source remains the bounded fallback. Exact three-component numeric
versions and conservative distro revisions such as `+ds-1ubuntu3` are
comparable; pre-release, git/date, malformed, oversized, nonzero and timed-out
outputs remain unavailable/uncomparable and never create a compatibility
finding. The OS release field is not reused, and portal interface version
properties are a separate concept.

`XDP006` is deliberately narrow: it requires exact normalized frontend version
`1.22.0`, pure `XDG_CURRENT_DESKTOP=niri`/session identity, an effective
selected `Settings=gtk` preference from the selected config, a valid existing
lower-priority candidate whose effective Settings preference (explicit entry
or fallback default) includes GNOME, and both GNOME and GTK descriptors
advertising Settings. Lower candidates are never merged into route resolution;
missing, malformed, unreadable or non-GNOME lower evidence is ignored. It
reports a known upstream compatibility risk, not an observed duplicate
`SettingsChanged` conflict. Missing or uncomparable version evidence, another
version/desktop, canonical `default=gnome;gtk` without the lower candidate
evidence, a single capable descriptor, or merely having GNOME and GTK
installed is silent. This is controlled evidence/compatibility coverage, not
live Niri validation, active-probe validation, a support claim or release
approval; Phase 8, KDE and Sway blockers are unchanged.

### Phase 11 Hyprland/Niri live-readiness checkpoint — 2026-09-09

The current host is `ubuntu:GNOME` on Wayland (`wayland-0`), not a Hyprland or
Niri session. PipeWire and WirePlumber are running, and the existing passive
portal snapshot selects the installed GNOME backend for Screenshot, ScreenCast,
FileChooser and Settings. That GNOME evidence is not target evidence for
Hyprland or Niri.

Hyprland is `BLOCKED / NOT AVAILABLE`: no Hyprland session identity,
`xdg-desktop-portal-hyprland` package/binary/descriptor, active
`xdg-desktop-portal-hyprland.service`, or canonical
`org.freedesktop.impl.portal.desktop.hyprland` owner was found. Niri is
`BLOCKED / NOT AVAILABLE` because no real Niri Wayland session is present;
the upstream-shaped effective `niri-portals.conf`, expected GNOME/GTK mixed
passive routes and target capabilities cannot be validated from this GNOME
host. The live model uses the GNOME/GTK portal backends; a separate Niri
backend artifact is not part of this trigger. No provider was installed or
started, and no active probe was run.

Hyprland may be re-evaluated only after a real Hyprland Wayland session is
available with its corresponding backend package/descriptor and healthy user
unit, canonical D-Bus ownership, the correct target passive route, required
Screenshot/ScreenCast capability evidence, and ready PipeWire/WirePlumber.
Niri may be re-evaluated only after a real Niri Wayland session is available
with an upstream-shaped effective `niri-portals.conf`, healthy GNOME and GTK
portal backends/services, canonical GNOME/GTK D-Bus ownership, expected Niri
mixed passive routes, required Screenshot/ScreenCast capability evidence, and
ready PipeWire/WirePlumber. The same GNOME host must not be forced to stand in
for either target. Phase 11 controlled static, runtime, activation and
version-evidence coverage is complete; live Hyprland/Niri readiness and active
Screenshot/ScreenCast validation remain open. No support or release claim
follows from this checkpoint.

### Phase 12 first bounded Safe Remediation Preview slice — 2026-09-09

The first remediation slice is preview-only and targets `ENV004`. The explicit
command is `portaldoctor fix ENV004 --dry-run`; it evaluates the current
finding and the same in-memory snapshot that produced it, then emits a typed
remediation-preview schema v1 document. When applicable, the proposal lists
only non-empty process-side allowlisted values that differ from or are missing
in the systemd user activation environment, in deterministic key order.

The proposal is strictly non-destructive: `files_modified` is empty, service
restarts, package changes and configuration changes are empty, and `apply` is
`not_implemented`. No file is written and no `systemctl import-environment`
or other write-capable command is invoked. If `ENV004` is absent, the required
process value is missing, the snapshot schema/status/timestamp is unsupported,
or the comparison evidence is inconsistent/unavailable, the preview contains
no proposal and fails closed. This does not add an apply path, change the
passive v0.2.1 contract, or unblock any Phase 8 release gate.

The follow-up provenance-binding slice adds a typed `binding` to every
applicable proposal. It records the source snapshot schema version,
`collected_at`, finding ID, a deterministic `evidence_digest`, and a
`proposal_digest`. The evidence digest is SHA-256 over only sorted actionable
`ENV004` comparison entries (key, process value, activation value and
relation); it excludes paths, secrets and unrelated snapshot sections. The
proposal digest binds that evidence to the preview schema and bounded
proposal shape using explicit labels, lengths and values for binding metadata,
action, target, remediation ID, dry-run/apply state, environment updates and
all side-effect lists. The `proposal_digest` field is excluded from its own
input to avoid a circular hash and must be cleared/recomputed by any future
apply verifier. Reordered entries produce the same evidence digest, while
changed actionable evidence, proposal fields or a different collection
timestamp changes the binding. JSON and terminal output expose the binding,
but `apply` remains `not_implemented` and no write-capable operation is added.

The third Phase 12 slice adds the pure
`verify_env004_preview(stored_preview, fresh_snapshot, fresh_findings)`
contract. It verifies the stored proposal digest first, then the supported
schema and preview/action/target/dry-run/apply/empty-side-effect contract,
and finally regenerates fresh ENV004 evidence. A changed actionable evidence
digest or proposed update list returns `stale_evidence`; a missing fresh
ENV004 returns `not_applicable`; schema mismatches return
`unsupported_schema`; and any stored integrity/contract mutation returns
`tampered`. `collected_at` remains provenance and is not compared directly,
so an otherwise unchanged fresh snapshot can verify as `valid`. The CLI only
self-checks generated dry-run previews; no apply or write path exists.

The fourth Phase 12 slice adds the pure
`verify_env004_effect(proposal, fresh_snapshot, fresh_findings)` post-apply
verification contract. It checks the stored proposal digest and fixed
preview-only contract first, then requires an available fresh environment
section with a performed comparison. Every proposed key must have one
consistent comparison entry, retain the expected process-side value, and show
that value on the activation side before the result can be `converged`.
Remaining or additional `ENV004` findings produce typed
`still_mismatched`; a changed expected process value with no current `ENV004`
produces `no_longer_applicable`. Unresolved comparison mismatches require
matching `ENV004` evidence and produce `still_mismatched`.
Malformed/unavailable fresh evidence produces `unavailable`; integrity
failures produce `tampered`.
The verifier independently derives mismatch state from the fresh comparison
and requires it to agree exactly with the supplied `ENV004` findings, so
missing or fabricated findings fail closed as inconsistent evidence.
Unrelated snapshot fields and collection timestamps do not affect this
decision. The verifier is not wired to apply, `systemctl` or any write path;
`apply` remains `not_implemented`.

The fifth Phase 12 slice adds the typed `RemediationApproval` contract for
ENV004. `create_env004_approval` can create a record only from an
integrity-checked, contract-valid proposal and binds approval-contract version
2, remediation ID, proposal digest, evidence digest, action, target and
explicit user-approval state. Each record also carries an `approval_digest`
computed with explicit field labels and length-prefixed values over those
bounded fields; the digest excludes itself. `verify_env004_approval(...)`
checks that approval digest first, then proposal digest/contract, approval
binding, explicit `Approved` state and finally fresh evidence. Therefore a
state or binding mutation is `tampered`, not a valid `not_approved` or
authorization result. Its other typed results are `stale_evidence`,
`not_approved` and `not_applicable`. This is an approval and verification
boundary only; no apply authority, `systemctl` invocation or write-capable CLI
path exists.

The seventh Phase 12 slice adds the opaque `Env004ApplyPermit` admission
boundary. Its module-owned factory accepts a permit only after
`verify_env004_approval(...)` returns `valid` and a fresh preview reproduces
the exact proposal-bound evidence digest and environment updates. The permit
privately binds the proposal digest, approval digest, fresh evidence digest and
update list; it is neither serializable nor cloneable, and its private binding
cannot be constructed from a raw approval outside the module. No apply,
`systemctl` or environment-write path exists; `apply` remains
`not_implemented`.

The eighth Phase 12 slice consumes only that permit into a deterministic
`Env004ExecutionPlan`. Every allowlisted ENV004 key carries its desired
process-side value, fresh activation-side pre-state and explicit rollback:
restore a present prior value or unset an absent one. Key order is
deterministic; duplicate, unknown or missing pre-state fails closed. The plan
factory accepts no raw approval, proposal or snapshot and performs no
`systemctl`, file, package, configuration, service or environment write.

The ninth Phase 12 slice adds an internal executor state machine with an
injected `Env004ExecutionAdapter` trait. `execute_env004_plan` consumes the
plan by value, applies deterministic steps, and rolls back only completed
steps in reverse order after an apply failure. The tenth bounded slice makes
adapter results explicit as `Applied`, `DefinitelyNotApplied` or
`OutcomeUnknown`: a definitely-not-applied current step preserves the prior
rollback set, while an unknown current step is treated as potentially applied
and is rolled back before earlier completed steps. Rollback failures remain
separately typed. Only a controlled fake adapter exists; there is no
production systemctl/subprocess adapter, environment mutation or CLI
`--apply` path.

### Phase 12 completion checkpoint — 2026-09-09

The Phase 12 controlled contract/design work is **COMPLETE**. Its exit
criteria are covered by the typed implementation and controlled tests:

- [x] deterministic ENV004 applicability and read-only dry-run representation;
- [x] provenance binding and proposal integrity;
- [x] fresh-evidence and post-apply effect verification contracts;
- [x] explicit approval integrity and exact proposal/evidence binding;
- [x] opaque apply-admission capability;
- [x] deterministic pre-state and rollback plan;
- [x] controlled transaction executor with one-shot plan consumption;
- [x] explicit `Applied`, `DefinitelyNotApplied` and `OutcomeUnknown`
  semantics, including fail-closed rollback-failure reporting.

This is a design/contract completion, not production remediation
authorization. The `systemctl` adapter, environment write path and CLI
`--apply` remain unimplemented and unshipped; the only public remediation
entry point remains `portaldoctor fix ENV004 --dry-run`. Phase 12 is **not a
v0.3.0 release blocker**. The release remains blocked by the separate Phase 8
real-session gates: Screenshot success/cancellation is blocked by the external
GNOME provider failure, and ScreenCast success/cancellation is blocked because
the current provider advertises `AvailableSourceTypes=0` without Window bit
`2`.

### Phase 13.1 stable finding-semantics contract checkpoint — 2026-09-09

The first bounded Phase 13 slice is **COMPLETE**. The canonical runtime
catalog now owns the 21 stable finding IDs and debug/test validation checks
that registered rules are deterministic, unique and well formed. The catalog
is checked against `docs/findings.md`, so missing, extra or duplicate public
IDs fail the test suite.

The v1 finding JSON shape also has an exact runtime parity test for field set
and serialized types, cross-checked against `docs/json-schema.md`. Severity
and confidence remain the existing stable enums; `source_component` is the
current category/producer boundary and no separate category field was added.
No finding ID, severity, meaning or top-level schema version changed.

The rule registry contract is unique by semantic rule ID. Individual rules
may still emit multiple interface-scoped instances with the same ID when a
snapshot contains multiple independent affected interfaces; this is
documented behavior, not semantic ID reuse. Phase 8 Screenshot/ScreenCast
real-session gates and the `v0.3.0` blocker state are unchanged, and Phase 12
production apply remains unauthorized.

### Phase 13.2 versioned public JSON schema contract checkpoint — 2026-09-09

The second bounded Phase 13 slice is **COMPLETE**. `PUBLIC_JSON_SCHEMA_VERSION
= 1` is now the canonical runtime value shared by the legacy diagnostic JSON
envelope and normalized snapshot. The docs heading and bounded top-level JSON
example in `docs/json-schema.md` are checked against that constant.

The passive `Report`, `Snapshot` and privacy-aware `ShareableReport` models now
fail closed when a required `schema_version` is missing, has the wrong JSON
type, or is not the current version. Serialization also refuses an invalid
in-memory version. Required fields remain strict, while unknown additive keys
remain accepted according to the existing v0.x compatibility policy. The
shareable `report_version` remains a separate, independently validated
envelope version.

No public JSON meaning, finding ID, remediation, active probe or apply path
was added. Phase 8 Screenshot/ScreenCast real-session gates and the `v0.3.0`
blocker state are unchanged; Phase 12 production apply remains unauthorized.

### Phase 13.3 stable/documented exit-code contract checkpoint — 2026-09-09

The third bounded Phase 13 slice is **COMPLETE**. Passive exit-code constants
are centralized and tested without changing their public values: `0` means a
complete clean or warning/info-only run, `1` an ERROR/CRITICAL finding, `2`
invalid `clap` usage, `3` unavailable minimum graphical/user-D-Bus context,
and `4` an output/internal incomplete run. Runtime-context `3` remains
authoritative before finding severity, and `main` maps all generic run errors
to `4`.

README and PRD exit-code tables are now exact-parity fixtures. Unit coverage
locks parser error=`2`, `--help`=`0`, the five `RunOutcome` mappings and the
runtime-context precedence rule. `RunOutcome::ActiveProbe` remains separate;
FileChooser/Screenshot standalone exit semantics were not changed or merged
into the passive contract.

No public exit code or active-probe meaning changed. Phase 8 Screenshot/
ScreenCast real-session gates and the `v0.3.0` blocker state are unchanged;
Phase 12 production apply remains unauthorized.

### Phase 13.4 shareable-report privacy contract checkpoint — 2026-09-09

The fourth bounded Phase 13 slice is **COMPLETE**. The shareable constructor
now applies the redaction boundary before creating `ShareableReport`, so raw
reports cannot be mislabeled as safe. A canonical privacy invariant requires
`redacted=true` and `raw_journal=excluded` plus `raw_pipewire=excluded`;
serde and both JSON/Markdown renderers fail closed on mutated or inconsistent
metadata.

Regression coverage verifies the process-environment allowlist, deterministic
`$HOME` normalization, hostname suppression, secret/path redaction across all
finding text fields and normalized journal evidence, and the absence of raw
journal/PipeWire fields. The documented privacy envelope is compared with
runtime serialization. The legacy non-shareable diagnostic output, active
probe behavior and remediation/apply contracts are unchanged.

Phase 8 Screenshot/ScreenCast real-session gates and the `v0.3.0` blocker
state are unchanged; Phase 12 production apply remains unauthorized. The
remaining v0.3.0 external work is still one real Screenshot success plus one
portal-native cancellation and one real ScreenCast success plus one
portal-native cancellation, followed by the locked release/public-command
decision.

### Phase 13.5 default diagnostic UX and scope checkpoint — 2026-09-09

The fifth bounded Phase 13 slice is **COMPLETE**. Bare `portaldoctor` is
locked to the passive `check` path: it performs read-only bounded collection,
normalization, rules and rendering only; no active probe, remediation preview
application, portal dialog or state mutation is dispatched by default. A
terminal invariant test locks the version header and stable high-level
section order.

README and PRD now share a machine-checked scope contract for the validated
Ubuntu 26.04/GNOME/Wayland/systemd-user baseline, explicit/unreleased active
probes, no default fixes, no GUI, and no universal Linux/desktop support
claim. No public CLI behavior or scope claim was broadened. Phase 8/v0.3.0
blockers remain unchanged; Phase 12 apply remains unauthorized.

### Phase 13.6 diagnostic coverage contract checkpoint — 2026-09-10

The sixth bounded Phase 13 slice is **COMPLETE**. The seven roadmap
diagnostic gates now have a canonical inventory tied to the production
collector chain, normalized Snapshot fields, entry points, finding or
`ProbeResult v1` contracts, and controlled coverage. A docs-parity test fails
if a capability row is added, removed or changed without updating the runtime
inventory.

Portal routing, environment/activation, D-Bus, systemd user services,
PipeWire/WirePlumber and opt-in journal evidence are implementation- and
controlled-complete. Their live qualification is limited to the published
Ubuntu 26.04/GNOME/Wayland passive baseline; KDE, wlroots/Sway, Hyprland and
Niri remain controlled-only compatibility coverage. Active core probes are
implementation- and controlled-complete; FileChooser is live-qualified, while
Screenshot and ScreenCast real-session qualification remain externally
blocked. Controlled/fake results are not live support claims.

No collector, public probe command, remediation/apply path or provider E2E was
added. Phase 8 and the `v0.3.0` release blockers remain unchanged.

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
./scripts/validate-portal-version-ci.sh
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
