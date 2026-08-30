# PortalDoctor — Technical Architecture

**Status:** Architecture baseline  
**Date:** 2026-08-30
**Language:** Rust  
**Initial platform:** Linux / Ubuntu 26.04 / GNOME / Wayland / systemd user session

---

## 1. Architecture Objective

PortalDoctor must convert fragmented Linux desktop state into reproducible diagnostic findings without allowing one broken subsystem to break the diagnostic tool itself.

The architecture therefore separates:

1. **collection** — observe raw state,
2. **normalization** — convert it into a stable internal model,
3. **resolution** — reproduce portal backend selection,
4. **rules** — evaluate deterministic conditions,
5. **evidence** — attach facts supporting findings,
6. **reporting** — render terminal/JSON/Markdown output,
7. **probes** — explicitly exercise portals in later releases.

---

## 2. High-Level Architecture

```text
                    +----------------+
                    |      CLI       |
                    +-------+--------+
                            |
                            v
                    +----------------+
                    | Run Coordinator|
                    +-------+--------+
                            |
                +-----------+-----------+
                |                       |
                v                       v
       +------------------+     +------------------+
       | Passive Collectors|     | Active Probes   |
       |   (default)       |     | (explicit only) |
       +---------+---------+     +---------+--------+
                 |                         |
                 +------------+------------+
                              v
                    +--------------------+
                    | Normalized Snapshot|
                    +----------+---------+
                               |
                   +-----------+-----------+
                   |                       |
                   v                       v
          +----------------+       +----------------+
          | Portal Resolver|       | Compatibility  |
          |                |       | Knowledge      |
          +--------+-------+       +--------+-------+
                   |                        |
                   +-----------+------------+
                               v
                    +--------------------+
                    | Diagnostic Engine  |
                    +---------+----------+
                              |
                              v
                    +--------------------+
                    | Findings + Evidence|
                    +---------+----------+
                              |
                +-------------+--------------+
                |             |              |
                v             v              v
             Terminal        JSON          Markdown
```

---

## 3. Core Architectural Principles

### 3.1 Collectors do not diagnose

Bad:

```text
EnvironmentCollector sees missing WAYLAND_DISPLAY
-> directly prints "Screen sharing is broken"
```

Good:

```text
EnvironmentCollector
-> stores activation_env.wayland_display = None

Rule ENV004
-> evaluates that fact in context
-> emits finding if conditions apply
```

This enables fixture testing and reuse.

### 3.2 Reports do not collect

Terminal/JSON/Markdown renderers receive already-sanitized structured results. They should not execute commands or query D-Bus.

### 3.3 Failure is data

A failed collector should produce structured status:

```rust
CollectorState::Available
CollectorState::Unavailable
CollectorState::Unsupported
CollectorState::TimedOut
CollectorState::PermissionDenied
CollectorState::ParseError
```

A timeout against a wedged XDG portal is useful diagnostic evidence.

### 3.4 Passive and active behavior remain separated

Default run must not open dialogs or mutate portal state.

Active probe module is opt-in and versioned independently in behavior.

---

## 4. Suggested Repository Layout

Keep v0.x as one crate unless real reuse pressure appears.

```text
portaldoctor/
├── Cargo.toml
├── Cargo.lock
├── LICENSE
├── README.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
├── docs/
│   ├── findings.md
│   ├── json-schema.md
│   ├── privacy.md
│   └── compatibility.md
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── run.rs
│   ├── error.rs
│   │
│   ├── model/
│   │   ├── mod.rs
│   │   ├── snapshot.rs
│   │   ├── status.rs
│   │   ├── environment.rs
│   │   ├── portal.rs
│   │   ├── dbus.rs
│   │   ├── service.rs
│   │   ├── pipewire.rs
│   │   ├── finding.rs
│   │   └── evidence.rs
│   │
│   ├── collectors/
│   │   ├── mod.rs
│   │   ├── os_release.rs
│   │   ├── environment.rs
│   │   ├── activation_environment.rs
│   │   ├── portal_files.rs
│   │   ├── portal_config.rs
│   │   ├── dbus.rs
│   │   ├── systemd_user.rs
│   │   ├── journal.rs
│   │   ├── pipewire.rs
│   │   └── wireplumber.rs
│   │
│   ├── resolver/
│   │   ├── mod.rs
│   │   ├── search_paths.rs
│   │   └── portal_routes.rs
│   │
│   ├── rules/
│   │   ├── mod.rs
│   │   ├── engine.rs
│   │   ├── environment.rs
│   │   ├── portal.rs
│   │   ├── config.rs
│   │   ├── dbus.rs
│   │   ├── pipewire.rs
│   │   └── screencast.rs
│   │
│   ├── compatibility/
│   │   ├── mod.rs
│   │   └── knowledge.rs
│   │
│   ├── probes/
│   │   ├── mod.rs
│   │   ├── filechooser.rs
│   │   ├── screenshot.rs
│   │   └── screencast.rs
│   │
│   ├── privacy/
│   │   ├── mod.rs
│   │   └── redact.rs
│   │
│   └── report/
│       ├── mod.rs
│       ├── terminal.rs
│       ├── json.rs
│       └── markdown.rs
│
├── tests/
│   ├── fixtures/
│   ├── routing.rs
│   ├── rules.rs
│   ├── json_contract.rs
│   └── cli.rs
└── .github/
    └── workflows/
```

Do not create every file on day one. This is the target logical shape, not a mandate to scaffold empty modules.

---

## 5. Dependency Strategy

### Required early dependencies

#### `clap`

CLI argument parsing and subcommands.

#### `serde`, `serde_json`

Normalized snapshot, findings, JSON contract and fixture serialization.

#### `thiserror`

Typed internal errors without leaking implementation details into user-facing findings.

#### `tracing`

Developer/debug logging. Diagnostic findings are not logging messages.

### Runtime dependencies

#### `tokio`

Useful for bounded concurrent collectors and timeouts.

#### `zbus`

Preferred D-Bus access layer. Current `zbus` 5.19 is a stable D-Bus API for Rust and supports session connections.

Suggested design:

- use zbus for D-Bus reachability/properties/introspection where practical,
- avoid parsing `busctl` output for core logic,
- still permit command-based fallback only if necessary and explicitly modeled.

### Later dependency

#### `ashpd`

Use for active portal probes rather than rebuilding all high-level portal request/session mechanics.

### PipeWire strategy

Do not introduce native PipeWire Rust FFI in the first implementation.

Initial collector:

```text
spawn pw-dump
  -> bounded timeout
  -> JSON parse
  -> normalized PipeWire snapshot
```

This is justified because `pw-dump` is explicitly designed to output the current PipeWire state as JSON.

---

## 6. Normalized Snapshot Model

All diagnosis runs should produce a single internal snapshot.

Conceptual shape:

```rust
struct Snapshot {
    schema_version: u32,
    collected_at: Timestamp,
    system: Section<SystemInfo>,
    session: Section<SessionInfo>,
    environment: Section<EnvironmentInfo>,
    portal_config: Section<PortalConfigInfo>,
    portal_backends: Section<Vec<PortalBackend>>,
    portal_routes: Section<Vec<PortalRoute>>,
    dbus: Section<DbusInfo>,
    services: Section<ServiceInfo>,
    pipewire: Section<PipeWireInfo>,
    wireplumber: Section<WirePlumberInfo>,
    journal: Section<JournalInfo>,
    probes: Section<ProbeResults>,
}
```

`Section<T>` should preserve collection status and optional metadata:

```rust
struct Section<T> {
    status: CollectionStatus,
    value: Option<T>,
    errors: Vec<CollectionNote>,
}
```

This avoids treating “not supported” and “failed unexpectedly” as the same condition.

---

## 7. Environment Collector

### Allowlisted variables

Initial set:

```text
XDG_CURRENT_DESKTOP
XDG_SESSION_DESKTOP
XDG_SESSION_TYPE
WAYLAND_DISPLAY
DISPLAY
XDG_CONFIG_HOME
XDG_CONFIG_DIRS
XDG_DATA_HOME
XDG_DATA_DIRS
DBUS_SESSION_BUS_ADDRESS
XDG_RUNTIME_DIR
```

`PATH` may be relevant to activation but is more privacy/noise-sensitive. If collected, report handling must be explicit.

### Two environments

PortalDoctor should distinguish:

1. current process/session environment,
2. systemd user activation environment.

Possible systemd path:

```text
systemctl --user show-environment
```

Only allowlisted keys are retained.

### Comparison model

```rust
struct EnvironmentValue {
    process_value: Option<String>,
    activation_value: Option<String>,
    relation: EnvironmentRelation,
}
```

Where relation can be:

- equal,
- missing_process,
- missing_activation,
- different,
- not_checked.

---

## 8. Portal Configuration Discovery

This is a correctness-sensitive subsystem.

### Inputs

- effective XDG environment,
- desktop names from colon-separated `XDG_CURRENT_DESKTOP`,
- configuration search paths,
- desktop-specific config names,
- generic config fallback behavior.

### Requirements

The resolver must preserve:

- exact candidate files,
- precedence order,
- selected/considered file(s),
- parse errors,
- source location for every resolved preference.

Do not simply load one guessed file from `/usr/share`.

### Parsed model

```rust
struct PortalPreference {
    interface: PortalInterfaceSelector,
    backends: Vec<BackendSelector>,
    source_file: PathBuf,
    source_priority: usize,
}
```

Support special selector semantics such as `none` and `*` as defined by upstream.

---

## 9. `.portal` Backend Discovery

A backend descriptor model should include at least:

```rust
struct PortalBackend {
    id: String,
    descriptor_path: PathBuf,
    dbus_name: String,
    interfaces: BTreeSet<String>,
    legacy_use_in: Vec<String>,
}
```

### Discovery locations

Search through effective XDG data directories plus standard locations according to upstream semantics.

Do not assume `/usr/share/xdg-desktop-portal/portals` is the only location.

### Duplicate handling

If identical backend IDs exist at multiple XDG-data precedence levels, retain provenance so the resolver can explain which descriptor won.

---

## 10. Portal Route Resolver

The route resolver is one of PortalDoctor's primary differentiators.

### Input

- desktop identity,
- parsed portal configs,
- discovered backend descriptors,
- interface capability sets.

### Output

```rust
struct PortalRoute {
    interface: String,
    requested_candidates: Vec<String>,
    available_candidates: Vec<String>,
    selected_candidates: Vec<String>,
    evidence: Vec<RouteEvidence>,
    status: RouteStatus,
}
```

### Important rule

The resolver should reproduce upstream configuration semantics as closely as practical for the detected XDP behavior/version.

When upstream behavior changes between versions, PortalDoctor should:

- model it explicitly where necessary,
- expose the detected version,
- avoid presenting an approximation as certainty.

### Compatibility layer

Known version-specific behavior should not be embedded invisibly in generic rules.

Use a separate compatibility knowledge module:

```rust
CompatibilityFact {
    component,
    version_range,
    condition,
    reference,
    note,
}
```

This makes future updates reviewable.

---

## 11. D-Bus Collector

### v0.1 responsibilities

- connect to session bus,
- verify `org.freedesktop.portal.Desktop` is reachable,
- query selected safe properties with timeouts,
- optionally inspect relevant backend names,
- classify errors.

### Failure taxonomy

Distinguish:

- no session bus,
- name absent,
- activation failure,
- method/property timeout,
- access denied,
- malformed response.

Do not collapse all failures into “D-Bus error.”

### Timeout requirement

Every D-Bus call must be bounded. A portal deadlock is a target diagnostic state.

---

## 12. systemd User-Service Collector

The first supported environment assumes systemd, but the domain model must allow `unsupported` for non-systemd sessions later.

Relevant units may include:

- `xdg-desktop-portal.service`,
- discovered backend units where available,
- `pipewire.service`,
- `wireplumber.service`.

Avoid scraping human-oriented `systemctl status` output for core state if structured properties can be queried.

The initial implementation may use bounded `systemctl --user show ...` subprocesses if it keeps parsing targeted and tested; direct D-Bus can replace/augment this later.

---

## 13. PipeWire Collector

### v0.2 implementation

```text
pw-dump --no-colors
   |
   +-- timeout
   +-- exit status
   +-- JSON parse
   v
PipeWireSnapshot
```

Do not store the entire raw graph in the final public report by default.

Normalize only useful facts, such as:

- connection succeeded,
- PipeWire version if obtainable,
- relevant node/object presence,
- active portal ScreenCast stream evidence when probing,
- errors/timeouts.

### WirePlumber

Initial health evidence can use:

- user service status,
- bounded `wpctl status` or `wpctl` connectivity exit status.

The product is not an audio diagnostics suite; keep only portal-relevant facts.

---

## 14. Journal Collector

Phase 6 is implemented on `main` as an explicit opt-in collector. The default
diagnostic run does not invoke `journalctl`.

### Requirements

- explicit allowlist for `xdg-desktop-portal*.service`, `pipewire*.service`,
  and `wireplumber.service`,
- current boot and a 30-minute window,
- at most 80 records and a 512 KiB output boundary,
- structured JSON output from `journalctl --user`,
- stable error-pattern classification only after the unit/priority checks,
- sanitization before a message enters the normalized snapshot.

Relevant fields can include:

```text
_SYSTEMD_USER_UNIT / _SYSTEMD_UNIT
PRIORITY
MESSAGE
```

The collector discards all other journal fields, including host, process and
boot identifiers. It stores only the normalized unit, priority, classification
and a short sanitized message. Missing fields, unrelated noise, malformed
records and unknown patterns remain `no_relevant_evidence` or
`insufficient_evidence`; they are not turned into guesses.

### Important separation

Journal text can provide supporting evidence but should not become the sole
source of truth for configuration, routing, or PipeWire state. Matching
excerpts attach `Evidence::JournalExcerpt` to an existing media finding.

---

## 15. Diagnostic Engine

### Rule interface concept

```rust
trait DiagnosticRule {
    /// Stable rule identifier, e.g. `"ENV001"`.
    fn id(&self) -> &'static str;
    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding>;
}
```

### Finding model

The field list is owned by the PRD (§8 "Initial Findings Contract"); the Phase 0
implementation (`src/model/finding.rs`) is the reference shape. `impact` remains
optional because some findings carry no distinct consequence beyond severity.

```rust
struct Finding {
    id: String,                     // stable rule identifier, e.g. "ENV001"
    severity: Severity,
    confidence: Confidence,
    title: String,
    summary: String,
    explanation: String,
    evidence: Vec<Evidence>,
    impact: Option<String>,
    recommendation: Vec<String>,    // ordered next steps
    source_component: String,       // collector/rule subsystem that produced the finding
}
```

Dedicated newtypes (`FindingId`, `Recommendation`) are intentionally deferred:
plain strings keep JSON serialization stable. Introduce them only when real
reuse pressure appears.

### Evidence model

Evidence should be structured where possible:

```rust
Evidence::EnvironmentMismatch { ... }
Evidence::ConfigSelection { ... }
Evidence::MissingProvider { ... }
Evidence::DbusTimeout { ... }
Evidence::ServiceState { ... }
Evidence::PipeWireState { ... }
Evidence::WirePlumberState { ... }
Evidence::ScreenCastRoute { ... }
Evidence::JournalExcerpt { ... }
```

Renderers convert structured evidence to text.

### Rule purity

Rules must not:

- execute subprocesses,
- query D-Bus,
- read files,
- modify the system.

They consume a snapshot only.

---

## 16. Active Probe Architecture

Active probes arrive after passive diagnostics are stable.

### Probe contract

```rust
trait Probe {
    async fn run(&self, context: &ProbeContext) -> ProbeResult;
}
```

### ScreenCast result

Represent lifecycle stages explicitly:

```text
CreateSession        pass/fail/timeout/skipped
SelectSources        pass/fail/timeout/skipped
Start                pass/fail/timeout/cancelled
StreamsReturned      pass/fail
OpenPipeWireRemote   pass/fail/timeout
```

User cancellation must not be reported as infrastructure failure.

### Use ASHPD where appropriate

ASHPD already implements high-level Rust wrappers for portals. Prefer it to hand-rolling request/session D-Bus mechanics unless diagnostic needs require lower-level control.

---

## 17. Privacy / Redaction Architecture

Use two representations:

1. internal evidence,
2. shareable evidence.

### Redactor responsibilities

- `$HOME` normalization,
- optional hostname suppression,
- remove environment values outside allowlist,
- strip suspicious secret/token patterns from journal excerpts,
- truncate long messages,
- never serialize arbitrary command environment.

### Implemented shareable boundary — Phase 7

`portaldoctor report` clones the normalized `Report`, serializes it into a
temporary structured tree, applies the redaction policy, reconstructs the
typed report and only then renders terminal/JSON/Markdown output. This keeps
renderers free of collection logic and preserves the legacy
`check --json`/default report shape for compatibility.

The shareable envelope carries `report_version`, the underlying
`schema_version`, and privacy metadata. The policy filters the process
environment against `ALLOWLISTED_VARIABLES`, normalizes the current home path
to `$HOME`, masks obvious secret labels, optionally masks the current hostname,
and states that raw journal/PipeWire dumps are excluded. Journal and PipeWire
collectors already discard their raw streams, so the shareable layer cannot
accidentally serialize them.

---

## 18. CLI Architecture

Suggested command tree:

```text
portaldoctor
├── check [environment|portal|pipewire]
├── portal
│   ├── list
│   ├── routes
│   └── explain <INTERFACE>
├── report
│   └── --format terminal|json|markdown
└── probe
    ├── filechooser
    ├── screenshot
    └── screencast
```

Global flags:

```text
--json
--verbose
--no-color
--timeout <duration>
--version
```

Do not expose unstable/internal flags in public docs without reason.

---

## 19. Concurrency and Timeouts

Independent passive collectors can run concurrently where doing so does not distort state.

Potential parallel groups:

- OS/session/environment,
- static portal config/metadata,
- D-Bus runtime,
- systemd service state.

Avoid excessive concurrency for subprocess-heavy collection.

All async boundaries require explicit timeout policy.

Recommended concept:

```text
short metadata call: 1–2 s
normal runtime query: 2–3 s
active user-facing portal probe: longer and command-specific
```

Exact defaults should be calibrated by testing rather than treated as fixed architecture law.

---

## 20. JSON Versioning

`schema_version` must be separate from application version.

Example:

```json
{
  "schema_version": 1,
  "portaldoctor_version": "0.2.0"
}
```

Patch/minor application releases may add compatible optional fields while schema-breaking changes require a schema-version change.

Before 1.0, compatibility promises should be conservative but tests should already prevent accidental churn.

---

## 21. Testing Architecture

### Unit tests

- config parser,
- `.portal` parser,
- XDG search-path builder,
- route resolver,
- rule predicates,
- redaction functions.

### Fixture tests

Each scenario provides serialized collector outputs/normalized snapshot inputs and expected findings.

```text
tests/fixtures/<scenario>/
├── metadata.toml
├── snapshot.json
└── expected-findings.json
```

Collector-specific fixtures can be added where raw parsing itself needs validation.

### Golden tests

Useful for:

- CLI terminal output,
- route explanation,
- Markdown reports.

### Live integration tests

Mark separately from normal CI. They should not be required for every contributor to have a full GNOME Wayland session.

---

## 22. CI / Quality Gates

Baseline:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Later:

- dependency audit,
- release artifact smoke tests,
- JSON schema fixture validation,
- supported-architecture builds.

CI should not fake compatibility claims. A successful generic Linux CI run does not equal validated GNOME/KDE/Sway runtime support.

---

## 23. Packaging Strategy

Initial GitHub releases:

- Linux x86_64 tarball/binary,
- checksum.

Then:

- Linux ARM64,
- `.deb`,
- possibly `cargo install` if dependency/system expectations are suitable.

Later package channels can include distro/community packaging, but package-manager proliferation is not an early product goal.

---

## 24. Safe Remediation Architecture — Future Only

Automatic remediation is intentionally deferred.

If introduced later, architecture should require:

```text
Finding
  -> RemediationPlan
       -> Explain
       -> Dry-run
       -> Explicit approval
       -> Apply
       -> Verify
```

Never encode fixes directly inside rendering strings.

Example future command:

```bash
portaldoctor fix ENV004 --dry-run
```

Initial versions should provide copyable recommendations without executing them.

---

## 25. Architecture Decision Summary

| Decision | Choice |
|---|---|
| Language | Rust |
| Initial runtime | Ubuntu 26.04, GNOME, Wayland, systemd-user |
| Core model | collectors -> snapshot -> rules -> findings |
| D-Bus | zbus |
| Active portal API | ASHPD later |
| PipeWire | `pw-dump` JSON first; no early FFI |
| Default behavior | passive, read-only |
| Root requirement | no |
| AI | not part of core |
| Reporting | terminal + JSON first, Markdown later |
| Tests | fixtures + deterministic rules + selective live integration |
| Fixes | deferred, dry-run-first if ever added |
