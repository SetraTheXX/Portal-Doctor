# PortalDoctor — Development Roadmap

**Status:** Implementation roadmap baseline  
**Date:** 2026-08-22  
**Strategy:** Narrow vertical slice first, then expand subsystem coverage and desktop compatibility

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

- redaction engine,
- `$HOME` normalization,
- optional hostname suppression,
- environment allowlist enforcement,
- journal sanitization,
- report schema/version,
- report fixtures.

## Exit criteria

Generated report can be attached to a public GitHub issue without exposing obvious secrets or irrelevant personal data by default.

## Release

**v0.2.0**

---

# Phase 8 — Active Portal Probes

## Objective

Move from “stack looks ready” to explicit portal lifecycle tests.

## Dependency

Evaluate/use ASHPD for active requests.

## Probes

### FileChooser

```bash
portaldoctor probe filechooser
```

### Screenshot

```bash
portaldoctor probe screenshot
```

### ScreenCast

```bash
portaldoctor probe screencast
```

ScreenCast result stages:

```text
CreateSession
SelectSources
Start
StreamsReturned
OpenPipeWireRemote
```

## UX constraints

- active probes never run during default passive check,
- clearly state that a dialog may appear,
- distinguish user cancellation from infrastructure error,
- time out safely,
- clean up sessions on failure.

## Exit criteria

PortalDoctor can identify the exact stage at which a ScreenCast lifecycle fails or times out.

## Release

**v0.3.0**

---

# Phase 9 — KDE Plasma Compatibility

## Objective

Validate the architecture beyond GNOME rather than merely allowing it to compile.

## Tasks

- KDE portal config/metadata fixtures,
- `xdg-desktop-portal-kde` runtime behavior,
- Plasma Wayland environment validation,
- active probe validation,
- compatibility documentation.

## Rule review

Audit all existing rules for GNOME assumptions.

## Exit criteria

KDE/Plasma appears in a documented tested compatibility matrix with known limitations.

## Release

**v0.4.0**

---

# Phase 10 — wlroots / Sway Compatibility

## Objective

Validate mixed-backend routing and activation-environment diagnosis.

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
| v0.3.0 | Active FileChooser/Screenshot/ScreenCast probes |
| v0.4.0 | KDE Plasma validation |
| v0.5.0 | wlroots/Sway + modern compositor expansion, possible safe-fix preview |
| v0.6+ | Compatibility/rule expansion, packaging/hardening |
| v1.0.0 | Stable contracts, documented compatibility matrix, production-quality release |

---

# Recommended Immediate Implementation Slice

Do **not** start by building all phases.

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

