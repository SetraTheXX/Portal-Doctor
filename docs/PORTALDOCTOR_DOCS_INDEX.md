# PortalDoctor — Documentation Index

**Baseline date:** 2026-08-31

This folder contains the project-definition and release documentation for
PortalDoctor.

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
Active probes:  ASHPD later
PipeWire:       bounded normalized `pw-dump` evidence included in v0.2
Journal:        bounded user-journal evidence included in v0.2; opt-in
Reports:        terminal/JSON/Markdown shareable reports included in v0.2
v0.2:           no automatic fixes, no GUI, no broad desktop support claim
```

## Current status

Phase 4 is complete and v0.1.0 is published on both GitHub Releases and
crates.io. Phases 5–7 (PipeWire/WirePlumber, opt-in bounded journal evidence
and shareable reports/privacy) shipped in v0.2.0; v0.2.1 is now published with
the stabilization and release gates. The validated support target remains
Ubuntu 26.04 + GNOME + Wayland + systemd; active probes and wider desktop
validation are the next roadmap steps.
