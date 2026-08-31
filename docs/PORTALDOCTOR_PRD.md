# PortalDoctor — Product Requirements Document (PRD)

**Status:** Initial product baseline  
**Date:** 2026-08-22  
**Owner:** Project maintainer  
**License target:** MIT  
**Primary implementation language:** Rust

---

## 1. Product Summary

PortalDoctor is a Linux CLI that diagnoses XDG Desktop Portal integration problems by collecting desktop-session, portal configuration, D-Bus, user-service and relevant PipeWire/WirePlumber state into a normalized snapshot and evaluating deterministic diagnostic rules.

### Product statement

> **PortalDoctor is a deterministic diagnostic tool for XDG Desktop Portals, Wayland and PipeWire integration on Linux.**

### Core user question

> Why is screen sharing, a file chooser, a screenshot portal, settings propagation or another desktop portal path broken or suspicious on this Linux system?

---

## 2. Problem Statement

Portal failures are difficult to diagnose because the required evidence is distributed across multiple layers:

- desktop/session environment,
- XDG search paths,
- `portals.conf`,
- `.portal` implementation metadata,
- D-Bus session activation,
- `systemd --user`,
- portal backend services,
- journal logs,
- Wayland session integration,
- PipeWire,
- WirePlumber.

Existing tools expose these layers independently but do not normally reconstruct portal selection and correlate the evidence into a clear diagnosis.

PortalDoctor will provide this missing correlation layer.

---

## 3. Goals

### G1 — Identify the active desktop/session context

PortalDoctor must be able to describe the relevant Linux desktop context without requiring root privileges.

At minimum:

- OS/distribution metadata,
- desktop identity,
- session type,
- Wayland/X11 indicators,
- selected environment variables,
- user activation environment when available.

### G2 — Reconstruct XDG portal routing

PortalDoctor must discover:

- candidate `portals.conf` files,
- effective search precedence,
- installed `.portal` definitions,
- which backend provides which implementation interface,
- effective backend candidates per portal interface.

### G3 — Verify runtime reachability

PortalDoctor should verify:

- session D-Bus availability,
- XDG Desktop Portal frontend reachability,
- selected backend reachability/activation where practical,
- user-service state on systemd systems.

### G4 — Correlate media stack health for ScreenCast

When ScreenCast is relevant, PortalDoctor should evaluate:

- whether a ScreenCast implementation exists,
- whether PipeWire can be reached,
- whether WirePlumber/session management is present,
- whether runtime evidence suggests a broken pipeline.

### G5 — Produce deterministic findings

Every rule must produce stable, structured output with:

- finding ID,
- severity,
- confidence,
- title,
- explanation,
- evidence,
- impact,
- recommendation.

### G6 — Produce shareable reports safely

PortalDoctor should generate human-readable, JSON and Markdown reports with privacy-aware redaction suitable for GitHub issues and support requests.

### G7 — Remain useful without network access or AI

The core diagnostic path must not require:

- internet access,
- cloud APIs,
- LLMs,
- telemetry.

---

## 4. Non-Goals

PortalDoctor v1.x is **not** intended to:

- implement a portal backend,
- replace XDG Desktop Portal,
- replace PipeWire or WirePlumber,
- replace a Wayland compositor,
- become a Flatpak package manager,
- perform generic whole-system health checks,
- automatically edit system configuration by default,
- require sudo for normal diagnostics,
- support every desktop environment in v0.1,
- embed an AI model in the core diagnostic engine,
- provide a GUI in the initial roadmap.

---

## 5. Primary Users

### Persona A — Linux power user

Needs to answer why screen sharing/FileChooser/portal behavior differs between applications or after desktop changes.

### Persona B — Application developer

Needs a reproducible report from a user whose portal integration fails.

### Persona C — Distro / desktop maintainer

Needs consistent evidence about routing, activation environment and runtime state.

### Persona D — Portal/backend developer

Needs an external tool that produces a normalized environment and portal-routing snapshot for bug reports.

---

## 6. User Stories

### US-001 — General diagnosis

As a Linux user, I want to run one command and see whether my portal stack is healthy so that I do not need to manually learn every debugging tool first.

### US-002 — Backend routing explanation

As a maintainer, I want to see which backend PortalDoctor expects for `ScreenCast` or `FileChooser` and why so that I can validate the configuration path.

### US-003 — Activation environment mismatch

As a Wayland user, I want PortalDoctor to compare relevant shell/session values with the D-Bus/systemd activation environment so that missing propagated variables are obvious.

### US-004 — ScreenCast readiness

As a user, I want PortalDoctor to tell me whether the ScreenCast backend and PipeWire path are present before I debug a specific application.

### US-005 — Bug report generation

As an upstream developer, I want a user to attach a sanitized PortalDoctor report instead of manually copying many unrelated command outputs.

### US-006 — Machine-readable diagnostics

As a tool author, I want `--json` output with a versioned schema so that I can integrate PortalDoctor into another workflow.

---

## 7. Product Behavior

### 7.1 Default command

```bash
portaldoctor
```

Default behavior:

- passive only,
- read-only,
- no interactive portal dialogs,
- no root requirement,
- bounded timeouts,
- concise terminal output,
- non-sensitive information only.

Equivalent conceptually to:

```bash
portaldoctor check
```

### 7.2 Detailed checks

```bash
portaldoctor check --verbose
portaldoctor check environment
portaldoctor check portal
portaldoctor check pipewire
```

### 7.3 Portal routing commands

```bash
portaldoctor portal list
portaldoctor portal routes
portaldoctor portal explain ScreenCast
```

Expected route output concept:

```text
Interface                         Preferred       Available         Selected
FileChooser                       gnome;gtk       gnome,gtk         gnome
ScreenCast                        gnome           gnome             gnome
Settings                          gnome;gtk       gnome,gtk         gnome
Secret                            gnome-keyring   gnome-keyring     gnome-keyring
```

### 7.4 Machine-readable output

```bash
portaldoctor --json
```

Minimum top-level contract:

```json
{
  "schema_version": 1,
  "portaldoctor_version": "0.2.1",
  "snapshot": {},
  "findings": []
}
```

### 7.5 Report output

v0.2.1 release:

```bash
portaldoctor report
portaldoctor report --json
portaldoctor report --format markdown
```

### 7.6 Active probes

Not enabled by the default command.

Future explicit commands:

```bash
portaldoctor probe filechooser
portaldoctor probe screenshot
portaldoctor probe screencast
```

The user must intentionally invoke these because they may show UI or request user interaction.

---

## 8. Initial Findings Contract

### Finding fields

```text
id
severity
confidence
title
summary
explanation
evidence[]
impact
recommendation[]
source_component
```

### Severity

- `INFO`
- `WARNING`
- `ERROR`
- `CRITICAL`

### Confidence

- `LOW`
- `MEDIUM`
- `HIGH`

Severity indicates impact. Confidence indicates how strongly collected evidence supports the diagnosis.

### Initial rule families

#### Environment

- `ENV001` — XDG desktop identity missing
- `ENV002` — session type unavailable/inconsistent
- `ENV003` — Wayland session without usable `WAYLAND_DISPLAY`
- `ENV004` — relevant shell/session and activation environment mismatch

#### XDG Portal

- `XDP001` — portal frontend cannot be discovered/reached
- `XDP002` — portal frontend runtime appears unhealthy
- `XDP003` — no portal backend definitions discovered
- `XDP004` — requested interface has no usable implementation
- `XDP005` — configuration references an unavailable backend

#### Configuration

- `CFG001` — expected desktop-specific portal configuration not found
- `CFG002` — portal configuration parse error
- `CFG003` — selected backend does not provide requested interface
- `CFG004` — suspicious duplicate/multi-provider resolution

#### D-Bus

- `DBUS001` — session bus unavailable
- `DBUS002` — selected service/backend cannot be reached or activated

#### PipeWire

- `PW001` — PipeWire unavailable
- `PW002` — WirePlumber unavailable
- `PW003` — PipeWire state query failed/timed out

#### ScreenCast

- `SC001` — no usable ScreenCast backend
- `SC002` — ScreenCast route exists but media path is unavailable

---

## 9. v0.1 Scope

### Supported primary environment

- Ubuntu 26.04
- GNOME
- Wayland
- systemd user session
- XDG Desktop Portal frontend
- GNOME/GTK portal backends

### v0.1 required functionality

1. CLI and versioned models.
2. OS/desktop/session discovery.
3. relevant environment collection.
4. systemd user activation-environment comparison.
5. portal configuration discovery.
6. `.portal` backend discovery.
7. portal routing resolver.
8. D-Bus frontend reachability.
9. basic systemd user-service state.
10. deterministic rule engine.
11. terminal reporter.
12. JSON output.
13. fixture-based tests.
14. GitHub CI.

### v0.1 explicitly deferred

- PipeWire deep snapshot,
- journal evidence engine,
- active FileChooser/Screenshot/ScreenCast probes,
- KDE support claim,
- wlroots support claim,
- Hyprland support claim,
- Niri support claim,
- automatic fixes,
- GUI.

---

## 10. v0.2 Scope

Add:

- `pw-dump` JSON collection,
- WirePlumber reachability/state,
- relevant bounded journal collection,
- report generation,
- privacy/redaction engine,
- ScreenCast readiness correlation.

---

## 11. v0.3 Scope

Add explicit active portal probes:

- FileChooser,
- Screenshot,
- ScreenCast.

ScreenCast probe should expose lifecycle stages independently:

```text
CreateSession
SelectSources
Start
stream metadata
OpenPipeWireRemote
```

All active operations need timeouts and clean cancellation.

---

## 12. Desktop Compatibility Roadmap

### v0.4

KDE Plasma / `xdg-desktop-portal-kde`.

### v0.5

Sway / `xdg-desktop-portal-wlr`, then Hyprland and/or Niri compatibility work.

### v1.0 target

Stable enough to claim a tested compatibility matrix covering at least:

- GNOME,
- KDE Plasma,
- one wlroots/Sway setup,
- one modern mixed-backend setup such as Hyprland or Niri.

---

## 13. Privacy Requirements

### PRV-001 — Environment allowlist

PortalDoctor must never serialize the full environment by default.

Allowed values must be explicitly enumerated.

### PRV-002 — Home path normalization

Example:

```text
/home/alice/.config/xdg-desktop-portal/portals.conf
```

should be reportable as:

```text
$HOME/.config/xdg-desktop-portal/portals.conf
```

when the actual username is unnecessary.

### PRV-003 — No credentials/secrets

Do not collect arbitrary values that may contain:

- API keys,
- tokens,
- SSH secrets,
- browser/session data,
- command history.

### PRV-004 — Bounded journal collection

Only collect relevant units and a bounded time/entry window.

### PRV-005 — Sanitization before report output

Raw internal evidence may be richer than shareable report evidence. Redaction occurs before serialization to shareable report formats.

---

## 14. Reliability Requirements

### REL-001 — Timeout all external interactions

D-Bus calls, commands and probes must have explicit time limits.

### REL-002 — Partial snapshot support

One failed collector must not prevent all diagnostics.

Snapshot sections should support states like:

```text
available
unavailable
unsupported
timed_out
permission_denied
parse_error
```

### REL-003 — Never hang behind a broken portal

A broken portal is the exact system under test; PortalDoctor must treat hanging dependencies as evidence rather than inherit the hang.

### REL-004 — No destructive default behavior

`portaldoctor` and `portaldoctor check` must not change state.

---

## 15. Determinism Requirements

### DET-001

Rules operate on a normalized snapshot, not ad-hoc command output.

### DET-002

Given identical snapshot data and rule-engine version, findings should be identical.

### DET-003

Finding IDs must not be reused for unrelated semantics.

### DET-004

Compatibility/version heuristics must be explicit and separately testable.

---

## 16. UX Requirements

Default terminal output should prioritize:

1. overall result,
2. errors,
3. warnings,
4. concise environment summary,
5. actionable recommendation.

Example:

```text
PortalDoctor 0.1.0

Session
  Desktop      GNOME
  Type         Wayland
  OS           Ubuntu 26.04

Portal
  Frontend     reachable
  ScreenCast   gnome
  FileChooser  gnome -> gtk

Findings
  ERROR ENV004  WAYLAND_DISPLAY is missing from the activation environment
  WARN  CFG004  Multiple Settings implementations resolved

Summary
  14 passed · 1 warning · 1 error
```

`--verbose` can expose evidence details.

---

## 17. Exit Codes

The completed diagnostic contract is:

- `0` — the diagnostic completed without ERROR/CRITICAL findings; INFO and
  WARNING findings do not fail the run,
- `1` — the diagnostic completed and produced at least one ERROR or CRITICAL
  finding,
- `2` — CLI usage or argument validation failed (handled by `clap`),
- `3` — the diagnostic could not establish the minimum runtime context: a
  recognized graphical session/display and a reachable user session D-Bus,
- `4` — the diagnostic could not complete because of an output or internal
  process error.

Exit code `3` takes precedence over finding severity because an incomplete
runtime context means the result cannot be treated as a complete diagnostic.
Invalid parser input exits `2` through `clap`; successful `--help` output exits
`0`. Warnings alone should not make normal diagnostics fail in scripts.

---

## 18. Performance Expectations

Default passive check should feel immediate and should not invoke slow active portal operations.

Targets:

- most static collectors: tens of milliseconds,
- entire v0.1 passive run: normally well below a few seconds,
- each external interaction: bounded by a documented timeout,
- no unbounded journal traversal.

Performance is secondary to correctness but hangs are unacceptable.

---

## 19. Testing Requirements

Tests must not depend solely on the maintainer's live GNOME desktop.

Required fixture classes:

```text
ubuntu-gnome-healthy
gnome-missing-backend
gnome-missing-activation-env
malformed-portals-conf
screencast-no-provider
missing-pipewire
sway-bad-environment
niri-mixed-backends
```

Each fixture should define expected findings.

Golden/snapshot tests are appropriate for:

- routing tables,
- terminal output,
- JSON schema examples.

---

## 20. Success Criteria

### v0.1 success

PortalDoctor is successful when it can run on the primary Ubuntu/GNOME/Wayland target and reliably answer:

- what desktop/session is active,
- what portal configs were selected,
- what backends are installed,
- which backend is expected for core interfaces,
- whether the frontend/runtime can be reached,
- whether activation environment is suspicious,
- what deterministic findings apply.

### GitHub/public success

- clean README and usage examples,
- reproducible CI,
- documented finding IDs,
- issue template requesting `portaldoctor report` when available,
- no exaggerated claims such as “fixes every Wayland problem.”

---

## 21. v1.0 Definition

PortalDoctor reaches 1.0 when the project has:

- stable documented CLI behavior,
- stable versioned JSON schema,
- stable finding semantics,
- GNOME compatibility validation,
- KDE compatibility validation,
- wlroots/Sway compatibility validation,
- one additional modern compositor/mixed-backend validation,
- portal routing diagnostics,
- D-Bus/runtime diagnostics,
- activation-environment diagnostics,
- PipeWire/WirePlumber correlation,
- bounded journal evidence,
- active FileChooser/Screenshot/ScreenCast probes,
- privacy-aware report generation,
- extensive fixture tests,
- Linux x86_64 and ARM64 release artifacts,
- packaged/reproducible installation path.

Automatic remediation is **not** required for 1.0.
