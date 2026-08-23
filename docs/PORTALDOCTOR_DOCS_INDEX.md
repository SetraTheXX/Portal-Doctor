# PortalDoctor — Documentation Index

**Baseline date:** 2026-08-22

This folder contains the initial project-definition package for PortalDoctor.

## Documents

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
- privacy/redaction layer,
- active probe architecture,
- JSON versioning,
- test architecture,
- packaging decisions.

### `PORTALDOCTOR_ROADMAP.md`

Use this as the execution plan.

It defines Phase 0 through Phase 13, phase exit criteria, release mapping and the recommended first implementation vertical slice.

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
PipeWire:       pw-dump JSON first
v0.1:           no automatic fixes, no GUI, no broad desktop support claim
```

## Recommended Next Step

Begin implementation with **Phase 0 + the minimal Phase 1 vertical slice only**.

Do not start PipeWire, active probes or multi-desktop compatibility before the core snapshot/rule/report pipeline is validated on the primary Linux machine.

