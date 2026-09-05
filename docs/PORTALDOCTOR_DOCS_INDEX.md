# PortalDoctor — Documentation Index

**Baseline date:** 2026-09-05

This folder contains the project-definition and release documentation for
PortalDoctor.

## Start here for current work

### `PORTALDOCTOR_CURRENT_STATE.md`

Canonical current-state and handoff document. It records the published
release, validated scope, completed phases, the first bounded v0.3.0 task,
acceptance criteria and quality gates. Read it before using the roadmap or
starting implementation.

## Public release documents

### `findings.md`

The v0.2 finding catalog: IDs, severity, confidence and trigger conditions.

### `json-schema.md`

The publishable schema-v1 reference for `--json` output.

### `privacy.md`

The allowlist, read-only guarantees and report-sharing guidance.

### `compatibility.md`

The validated platform target and known limitations.

### `release-notes-v0.2.0.md`

Release notes for the passive ScreenCast-readiness and shareable-report
release.

### `release-notes-v0.2.1.md`

Release notes for the exit-code, package/install and demo stabilization
release.

### `release-notes-v0.1.0.md`

Release notes for the first public release.

## Project-definition documents

### `PORTALDOCTOR_RESEARCH.md`

Use this when asking:

- Why should this project exist?
- What does the upstream XDG portal architecture look like?
- What adjacent tools already exist?
- What is PortalDoctor's real differentiation?
- What current Linux problems validate the idea?

It includes the competitive analysis and primary upstream research sources.

### `PORTALDOCTOR_PRD.md`

Use this as the product contract.

It defines:

- product goals,
- non-goals,
- target users,
- commands,
- v0.1/v0.2/v0.3 scope,
- privacy/reliability/determinism requirements,
- finding contract,
- v1.0 definition.

### `PORTALDOCTOR_ARCHITECTURE.md`

Use this for implementation design.

It defines:

- collectors,
- normalized snapshot,
- portal route resolver,
- diagnostic rule engine,
- D-Bus/systemd/PipeWire strategy,
- privacy/redaction strategy,
- active probe architecture,
- JSON versioning,
- test architecture,
- packaging decisions.

### `PORTALDOCTOR_ROADMAP.md`

Use this as the execution plan.

It defines Phase 0 through Phase 13, phase exit criteria, release mapping and
the recommended first implementation vertical slice.

### `PORTALDOCTOR_ASHPD_DECISION.md`

Use this for the accepted Phase 8 ASHPD/lifecycle boundary, runtime and
compatibility assumptions, timeout cleanup obligations and error/fallback
taxonomy. It is a decision record, not an implementation guide for the probe
commands.

---

## Project Baseline Decisions

The current plan intentionally locks these decisions:

```text
Project:        PortalDoctor
Language:       Rust
License target: MIT
Default mode:   read-only / passive
Root required:  no
AI required:    no
Initial target: Ubuntu 26.04 + GNOME + Wayland + systemd user session
Core design:    collectors -> snapshot -> resolver -> rules -> findings -> report
D-Bus:          zbus
Active probes:  PortalDoctor-owned zbus lifecycle; selective ASHPD use
PipeWire:       bounded normalized `pw-dump` evidence included in v0.2
Journal:        bounded user-journal evidence included in v0.2; opt-in
Reports:        terminal/JSON/Markdown shareable reports included in v0.2
v0.2:           no automatic fixes, no GUI, no broad desktop support claim
```

## Current status

Phase 4 is complete and v0.1.0 is published on both GitHub Releases and
crates.io. Phases 5–7 (PipeWire/WirePlumber, opt-in bounded journal evidence
and shareable reports/privacy) shipped in v0.2.0; v0.2.1 was published with
the stabilization and release gates. The validated support target remains
Ubuntu 26.04 + GNOME + Wayland + systemd. The next implementation gate is
Phase 8 / v0.3.0, beginning with the bounded FileChooser probe sequence in
[GitHub Issue #3](https://github.com/SetraTheXX/Portal-Doctor/issues/3).

For a direct handoff, use
[`PORTALDOCTOR_CURRENT_STATE.md`](PORTALDOCTOR_CURRENT_STATE.md), not the
historical bootstrap section at the bottom of the roadmap.
