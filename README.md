# PortalDoctor

Read-only diagnostics for the Linux desktop portal stack.

[![CI](https://github.com/SetraTheXX/Portal-Doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/SetraTheXX/Portal-Doctor/actions/workflows/ci.yml) [![Crates.io](https://img.shields.io/crates/v/portaldoctor.svg)](https://crates.io/crates/portaldoctor) [![Release](https://img.shields.io/github/v/release/SetraTheXX/Portal-Doctor?sort=semver)](https://github.com/SetraTheXX/Portal-Doctor/releases) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> **Branch notice:** GitHub `main` is the development branch and contains
> unreleased experimental probe work. For the stable published product, use
> [v0.2.1 on crates.io](https://crates.io/crates/portaldoctor) or the
> [v0.2.1 GitHub release](https://github.com/SetraTheXX/Portal-Doctor/releases/tag/v0.2.1).

<p align="center">
  <img src="./docs/assets/portaldoctor-demo.gif" alt="PortalDoctor diagnosing Linux desktop portal health and routing" width="100%">
</p>

> Find out why screen sharing, file choosers, and screenshots fail — without digging through five different tools.

PortalDoctor is a deterministic Rust CLI that turns scattered XDG Desktop
Portal, Wayland, D-Bus, and systemd user-session state into one
evidence-backed diagnostic report.

> **v0.2.1** — Stabilization release for the passive ScreenCast-readiness path. The validated baseline is Ubuntu
> 26.04 with GNOME, Wayland, and a systemd user session.

The default command remains passive and read-only. The development branch also
contains explicit active probes; Screenshot can ask the portal to create an
artifact and is not part of the published v0.2.1 package.

## Why PortalDoctor?

Desktop portal failures rarely have a single obvious cause. A missing session
variable, an unexpected `portals.conf` preference, an unavailable D-Bus name,
or an unhealthy user service can all surface as the same “screen sharing does
not work” symptom.

PortalDoctor collects those signals in one read-only pass, explains the route
selected for each portal interface, and reports deterministic findings with
evidence and an actionable next step.

| Signal | What it answers |
| --- | --- |
| Session and environment | Is the current Wayland/XDG session coherent? |
| Portal configuration | Which config files and backend descriptors are active? |
| Routing | Which backend is selected for `ScreenCast`, `FileChooser`, and other interfaces — and why? |
| Runtime reachability | Does the portal frontend and selected backend own their D-Bus names? |
| systemd user session | Are discovered portal units present and healthy? |
| ScreenCast media path | Is PipeWire reachable, is WirePlumber answering, and is normalized video topology visible? |
| Findings | What is wrong, how confident is the signal, and what should be checked next? |

## Quick start

### Install from crates.io

```sh
cargo install portaldoctor --version 0.2.1 --locked
portaldoctor
```

For the latest published version, omit the `--version` flag.

### Build from source

```sh
git clone https://github.com/SetraTheXX/Portal-Doctor.git
cd Portal-Doctor
cargo build --locked --release
./target/release/portaldoctor
```

Normal use is read-only and does not require `sudo` or root access.

## Typical workflow

Start with the default passive check:

```sh
portaldoctor
```

Narrow the investigation when you already know which layer is involved:

```sh
portaldoctor check environment
portaldoctor check portal
portaldoctor check pipewire
portaldoctor portal list
portaldoctor portal routes
portaldoctor portal explain ScreenCast
```

Use verbose output for collected values, route evidence, and full finding
explanations. Use JSON when the report needs to be consumed by another tool:

```sh
portaldoctor check environment --verbose
portaldoctor --json > portaldoctor.json
portaldoctor --journal --verbose
```

Generate a report intended for an issue or support request. The explicit
`report` command applies the privacy layer before serialization:

```sh
portaldoctor report
portaldoctor report --format markdown --suppress-hostname > portaldoctor-report.md
portaldoctor report --json > portaldoctor-report.json
```

`--json` is also accepted as the global shorthand for the JSON report format.
The report command normalizes home-directory paths, keeps the existing
environment allowlist, and marks raw journal/PipeWire dumps as excluded.
Review the generated document once before publishing it.

`--journal` is opt-in. It adds a bounded current-boot/user-session journal
check for portal, PipeWire, and WirePlumber units; `--verbose` displays only
the short, sanitized excerpts that match stable error patterns. The same
option can be used with `report` when journal evidence should be included.

### Explicit FileChooser probe (development on `main`)

The first Phase 8 active probe is available on the development branch as an
explicit command. It is not part of the published crates.io `v0.2.1` binary
and is never invoked by `portaldoctor` or `portaldoctor check`:

```sh
portaldoctor probe filechooser
```

This command may open a desktop file chooser. Cancel it when you only want to
validate the cancellation path, or select any file to exercise the success
path. PortalDoctor validates the portal lifecycle, discards the returned URI
after checking its protocol shape, does not read, copy, modify or persist the
selected file, and always reports request cleanup separately. For automation,
use `--json`; the warning stays on `stderr` and `stdout` contains only the
standalone `ProbeResult` document:

```json
{
  "schema_version": 1,
  "probe": "file_chooser",
  "stage": "complete",
  "status": "user_cancelled",
  "cleanup": { "status": "completed", "failed_resources": [] }
}
```

For this explicit command, exit `0` means a successful operation with verified
cleanup; every cancellation, timeout, unavailable/unsupported capability,
malformed response, infrastructure failure or cleanup failure returns `1`.
The passive command exit-code contract below is unchanged.

The request token is internal D-Bus metadata. It is generated with mandatory
operating-system entropy and is never printed or persisted; if entropy is
unavailable, the probe fails before opening a portal request. Maintainers can
run the complete controlled lifecycle gate locally (with `dbus-x11`,
`python3-gi` and GLib introspection installed):

```sh
PORTALDOCTOR_BIN=target/release/portaldoctor \
  ./scripts/validate-filechooser-fake-ci.sh
PORTALDOCTOR_BIN=target/release/portaldoctor \
  ./scripts/validate-screenshot-fake-ci.sh
```

### Explicit Screenshot probe (development on main)

The bounded Screenshot probe is available only on the development branch and
is not part of the published crates.io v0.2.1 binary:

    portaldoctor probe screenshot
    portaldoctor probe screenshot --json

Before the portal call, PortalDoctor warns that the user may be asked to
choose a window and that the portal may create a screenshot artifact. The
first slice requires Screenshot interface version 3 or newer and the
AvailableTargets Window bit (2); it never falls back to Screen, Area or
Active Window. The command never opens or decodes image data and never reads,
copies, logs, persists or deletes the screenshot, URI, path, filename or
metadata. Request.Close ends only the portal request lifecycle and is not image
deletion.

The machine-readable result contains only ProbeResult v1 lifecycle facts. The
controlled gate covers success, cancellation, malformed response, portal
failure, request/response timeout, late reply, transport failure, unsupported
version/target, unavailable service and cleanup failure:

    PORTALDOCTOR_BIN=target/release/portaldoctor \
      ./scripts/validate-screenshot-fake-ci.sh

## What it checks

- operating system, desktop, session type, and allowlisted environment values,
- process environment versus the systemd user activation environment,
- effective XDG configuration and data search paths,
- desktop-specific and generic `portals.conf` files,
- `.portal` backend descriptors, interfaces, and provenance,
- interface-specific, default, wildcard, and disabled route preferences,
- D-Bus name ownership for the portal frontend and selected backends,
- basic systemd user-unit state for the discovered portal services,
- bounded PipeWire/WirePlumber health and privacy-safe video topology,
- optional bounded journal evidence for portal, PipeWire, and WirePlumber
  failures,
- shareable terminal, JSON and Markdown reports with report-level redaction,
- 20 deterministic findings across the `ENV`, `XDP`, `CFG`, `DBUS`, `PW`, and
  `SC` families.

## Example

A healthy GNOME/Wayland session produces a compact report like this:

```text
PortalDoctor 0.2.1
Snapshot schema v1

System: Ubuntu 26.04 LTS (ubuntu)
Session: wayland session · desktop ubuntu:GNOME · session desktop ubuntu
Activation environment: consistent (5 variables compared)
D-Bus: connected · portal frontend reachable
  backend org.freedesktop.impl.portal.desktop.gnome: reachable
  backend org.freedesktop.impl.portal.desktop.gtk: reachable
PipeWire: reachable · 1.6.2 · 81 objects · 10 nodes · 3 links
  video sources: 1 · ScreenCast sources: 1 · portal clients: 1
WirePlumber: reachable · 1.6.2 · 2 client(s)

Findings: none detected.
```

When a problem is detected, the default terminal report includes the finding
ID, a concise explanation, and the first actionable `next:` recommendation.
`--verbose` adds confidence, impact, evidence, and all recommendations.

### Exit codes

Completed diagnostic commands use a stable shell contract:

| Code | Meaning |
| --- | --- |
| `0` | Completed with no ERROR/CRITICAL finding; INFO/WARNING findings are allowed. |
| `1` | Completed with at least one ERROR/CRITICAL finding. |
| `2` | Invalid CLI usage or arguments; `clap` reports the parser error. |
| `3` | Minimum runtime context is unavailable: no recognized graphical display or no reachable user D-Bus. |
| `4` | Output or internal process error prevented completion. |

Code `3` takes precedence when the runtime context is incomplete. Successful
`--help` output exits with `0`.

## Design principles

- **Read-only:** collection does not edit configuration, restart services, or
  open portal dialogs.
- **Deterministic:** the same snapshot and rules produce the same finding
  structure; no AI service or telemetry is involved.
- **Bounded:** D-Bus and subprocess work has time limits so a broken dependency
  cannot hang a diagnostic run.
- **Privacy-aware:** only a small allowlist of diagnostic environment variables
  is collected; arbitrary environment dumps and unrelated journal records are
  not part of the report. Shareable reports normalize `$HOME`, redact obvious
  secret patterns, and can suppress the hostname before serialization.
- **Automation-friendly:** terminal output is concise, while JSON output is
  versioned as snapshot schema v1.

## Scope and limitations

### Product boundary contract

<!-- PORTALDOCTOR_SCOPE_CONTRACT_START -->
```text
default_behavior=passive;read-only;rootless;bounded
default_portal_dialogs=none
default_state_mutation=none
validated_baseline=Ubuntu 26.04;GNOME;Wayland;systemd user session
support_claim=baseline-only;other desktops require dedicated validation
development_active_probes=explicit-only;unreleased;not-default
automatic_fixes=not-default
gui=out-of-scope
```
<!-- PORTALDOCTOR_SCOPE_CONTRACT_END -->

This is the product boundary for the published passive line: the bare
`portaldoctor` command is a bounded diagnostic, not a universal Linux health
check, automatic fixer, or GUI workflow. Development active probes are
explicit and unreleased until their separate acceptance and release gates
pass.

### Validated baseline

| Component | v0.2 baseline |
| --- | --- |
| Distribution | Ubuntu 26.04 |
| Desktop | GNOME, including `ubuntu:GNOME` identifiers |
| Session | Wayland |
| Service manager | systemd user session |
| Portal frontend | `org.freedesktop.portal.Desktop` |

Other distributions and desktops may work, but they are not support claims for
v0.2 until they have a dedicated validation matrix.

### Outside the published v0.2.1 boundary

- validated KDE, wlroots, Sway, Hyprland, or Niri behavior,
- automatic fixes and GUI workflows.

The development `main` branch contains the audited explicit FileChooser
and Screenshot probes described above; both remain outside the published
`v0.2.1` boundary and are unreleased until the v0.3.0 release
decision. Screenshot success still requires a real v3-capable portal session
with the Window target; the current Ubuntu 26.04 session exposes Screenshot
version 2, so it is correctly classified as unsupported without opening a
request.

For the exact compatibility contract and known resolver limitations, see
[compatibility and known limitations](docs/compatibility.md).

## Reproducible demo

The README GIF uses four checked-in scenes: a healthy passive check,
explainable `ScreenCast` routing, a privacy-aware Markdown report, and a
controlled missing-`WAYLAND_DISPLAY` diagnosis with its exit code.
It intentionally remains a passive `v0.2.1` showcase and does not claim to
demonstrate the development FileChooser dialog lifecycle.

```sh
cargo build --locked --release
PORTALDOCTOR_BIN="$PWD/target/release/portaldoctor" ./scripts/demo.sh
```

To recreate the GIF with Terminalizer:

```sh
terminalizer record /tmp/portaldoctor-demo \
  --config docs/demo/terminalizer.yml \
  --command 'bash scripts/demo.sh' \
  --skip-sharing

terminalizer render /tmp/portaldoctor-demo \
  --output docs/assets/portaldoctor-demo.gif \
  --quality 95
```

The checked-in [Terminalizer configuration](docs/demo/terminalizer.yml)
keeps the recording slow enough to read, uses a larger terminal canvas, and
removes the renderer title from the frame.

## Documentation

- [Current state and AI handoff](docs/PORTALDOCTOR_CURRENT_STATE.md)
- [Package page on docs.rs](https://docs.rs/portaldoctor/0.2.1) *(PortalDoctor is a binary-only CLI, so docs.rs does not provide a public library API.)*
- [Finding catalog](docs/findings.md)
- [JSON schema v1](docs/json-schema.md)
- [Active `ProbeResult` schema v1](docs/probe-result-schema.md)
- [Compatibility and known limitations](docs/compatibility.md)
- [Privacy statement](docs/privacy.md)
- [Architecture](docs/PORTALDOCTOR_ARCHITECTURE.md)
- [Development roadmap](docs/PORTALDOCTOR_ROADMAP.md)
- [v0.2.1 release notes](docs/release-notes-v0.2.1.md)
- [v0.2.0 release notes](docs/release-notes-v0.2.0.md)
- [v0.1.0 release notes](docs/release-notes-v0.1.0.md)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## Development

Run the same quality gates used by CI:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --release
cargo package --locked
artifact_dir="$(mktemp -d -t portaldoctor-artifact.XXXXXX)"
./scripts/build-release-artifact.sh "$artifact_dir"
install_root="$(mktemp -d -t portaldoctor-smoke.XXXXXX)"
cargo install --path . --locked --root "$install_root"
"$install_root/bin/portaldoctor" --version
PORTALDOCTOR_BIN=target/release/portaldoctor \
  python3 scripts/validate-v0.1-faults.py
PORTALDOCTOR_BIN=target/release/portaldoctor \
  ./scripts/validate-filechooser-fake-ci.sh
```

The package and install commands verify the artifact before publication. The
release-artifact helper additionally stages one deterministic x86_64 Linux
PortalDoctor binary named `portaldoctor-<version>-<target>` with a SHA-256
sidecar, rechecks the checksum and exact `--version` output, and preserves
executable/installability semantics. This controlled gate does not upload a
GitHub release, create a tag or publish to crates.io; ARM64 remains a separate
future artifact gate. The
fault-injection harness exercises the v0.1-compatible finding contract and
stable parser/runtime exit codes without modifying the host system. See the
[fault-injection harness](scripts/validate-v0.1-faults.py) for the fixture
scenarios. The FileChooser and Screenshot wrappers run controlled success,
cancellation, timeout, malformed-response, transport,
unsupported-capability and cleanup-failure scenarios used by CI; they verify
`Request.Close` calls and privacy redaction, not only JSON shape.

## Roadmap

The v0.2.1 release completes the passive diagnostic stabilization gate; the
next expansion is deliberately layered:

1. finish and publish the bounded FileChooser and Screenshot slices in
   [Issue #3](https://github.com/SetraTheXX/Portal-Doctor/issues/3), then add
   active probes for selected portal interfaces one family at a time,
2. expand validation across KDE and wlroots-based sessions,
3. harden the compatibility matrix and release artifacts,
4. document and ship the next compatible release only after its acceptance
   gate passes.

The full implementation roadmap lives in
[docs/PORTALDOCTOR_ROADMAP.md](docs/PORTALDOCTOR_ROADMAP.md).

## License

[`MIT`](LICENSE) — Copyright (c) 2026 PortalDoctor contributors.
