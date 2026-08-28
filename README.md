# PortalDoctor

Read-only, deterministic Linux CLI for diagnosing XDG Desktop Portal, Wayland
session, D-Bus and systemd user integration issues.

[![CI](https://github.com/SetraTheXX/Portal-Doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/SetraTheXX/Portal-Doctor/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> Find out why Linux screen sharing, file chooser and screenshot portals fail —
> without digging through five different tools.

![PortalDoctor demo](docs/assets/portaldoctor-demo.gif)

The demo shows a healthy passive check, explainable `ScreenCast` routing and a
controlled missing-`WAYLAND_DISPLAY` diagnosis.

**Status:** v0.1.0 released on GitHub. Crates.io publication is pending.

**Validated target:** Ubuntu 26.04 + GNOME + Wayland + systemd user session.

## The problem

Linux desktop integration failures are spread across several layers. Screen
sharing, a file chooser or a screenshot portal can fail because the session
environment is incomplete, `portals.conf` selects the wrong backend, a `.portal`
descriptor is missing an interface, the portal frontend is absent from D-Bus,
or a systemd user service is not healthy. Debugging this by hand means jumping
between environment dumps, XDG paths, portal descriptors, D-Bus tools and
`systemctl` output.

## What PortalDoctor does

PortalDoctor reads those layers without modifying the system, builds one
normalized snapshot, reconstructs portal routing, checks runtime reachability,
and emits deterministic root-cause findings with evidence and a recommended
next step.

It can expose, for example:

- wrong or missing portal backend routing,
- a missing or malformed `portals.conf`,
- a backend descriptor that exists but whose runtime D-Bus name is unreachable,
- an unreachable XDG Desktop Portal frontend,
- an unhealthy or missing systemd user service,
- missing Wayland/session environment values,
- a mismatch between process environment and systemd activation environment,
- a configured backend that does not advertise the requested interface.

**PipeWire and WirePlumber are not implemented in v0.1.** ScreenCast findings
in this version stop at portal configuration and D-Bus/systemd reachability.
PipeWire/WirePlumber correlation is planned for Phase 5.

## Quick start / Installation

### Install from this repository

```sh
cargo install --git https://github.com/SetraTheXX/Portal-Doctor --tag v0.1.0 --locked
```

### Build from source

```sh
git clone https://github.com/SetraTheXX/Portal-Doctor.git
cd Portal-Doctor
cargo build --release
./target/release/portaldoctor
```

To reproduce the README demo locally:

```sh
cargo build --release
PORTALDOCTOR_BIN="$PWD/target/release/portaldoctor" ./scripts/demo.sh
```

With Terminalizer installed, record and render the same flow again:

```sh
terminalizer record /tmp/portaldoctor-demo \
  --command 'bash scripts/demo.sh' --skip-sharing
terminalizer render /tmp/portaldoctor-demo \
  -o docs/assets/portaldoctor-demo.gif -q 90
```

## Usage

Start with the default diagnostic run:

```sh
portaldoctor
```

Then inspect individual areas or machine-readable output:

```sh
portaldoctor check environment
portaldoctor check portal
portaldoctor check --json
portaldoctor portal list
portaldoctor portal routes
portaldoctor portal explain ScreenCast
```

Use `--verbose` for collected values, route evidence and full finding details:

```sh
portaldoctor check environment --verbose
```

## Example

Healthy Ubuntu 26.04 / GNOME / Wayland output:

```text
PortalDoctor 0.1.0
Snapshot schema v1

System: Ubuntu 26.04 LTS (ubuntu)
Session: wayland session · desktop ubuntu:GNOME · session desktop ubuntu
Activation environment: consistent (5 variables compared)
D-Bus: connected · portal frontend reachable
  backend org.freedesktop.impl.portal.desktop.gnome: reachable
  backend org.freedesktop.impl.portal.desktop.gtk: reachable
  backend org.freedesktop.secrets: reachable

Findings: none detected.
```

When a finding exists, the default terminal view includes its first actionable
`next:` recommendation; `--verbose` adds explanation, impact, evidence and all
recommendations.

## v0.1 capabilities

- OS, desktop, session and allowlisted environment discovery.
- Process versus systemd user activation-environment comparison.
- XDG config/data search-path resolution.
- Desktop-specific and generic `portals.conf` discovery and parsing.
- `.portal` backend inventory with duplicate provenance.
- Route resolution for interface-specific preferences, `default`, `*` and
  `none`.
- D-Bus frontend and selected backend name-owner checks with bounded timeouts.
- Basic systemd user unit state collection.
- 15 deterministic findings (`ENV`, `XDP`, `CFG` and `DBUS` families).
- Terminal and versioned JSON output.
- Fixture tests and CI quality gates.

## Safety & privacy

- Read-only by default; normal use does not require sudo or root.
- No network access, telemetry or AI service.
- Only 11 diagnostic environment variables are allowlisted and collected.
- `HOME` is read only to derive XDG default roots such as `$HOME/.config` and
  `$HOME/.local/share`; it is not collected or reported as a diagnostic
  variable.
- No shareable redaction layer is claimed in v0.1. Review JSON output before
  attaching it to a bug report; deeper report redaction is later privacy work.
- External D-Bus and subprocess work is bounded so a broken dependency cannot
  hang the CLI.

## Supported platform / limitations

v0.1 is validated on **Ubuntu 26.04 + GNOME + Wayland + systemd user session**.
No support claim is made for KDE, wlroots/Sway, Hyprland, Niri or other
compositor/distribution combinations.

Known limitations include:

- no PipeWire/WirePlumber graph or media-stack health check,
- no journal evidence correlation,
- no active portal dialogs/probes,
- `NameHasOwner` proves bus ownership, not method-level backend health,
- conventional systemd unit-name mapping is assumed for discovered backends,
- no automatic fixes and no GUI.

See [compatibility and known limitations](docs/compatibility.md) for details.

## Documentation

- [Finding catalog](docs/findings.md)
- [JSON schema v1](docs/json-schema.md)
- [Privacy statement](docs/privacy.md)
- [Compatibility and known limitations](docs/compatibility.md)
- [v0.1.0 release notes](docs/release-notes-v0.1.0.md)
- [Contributing](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security](SECURITY.md)
- [v0.1 fault-injection harness](scripts/validate-v0.1-faults.py)
- [Project documentation index](docs/PORTALDOCTOR_DOCS_INDEX.md)

## License

[`MIT`](LICENSE) — Copyright (c) 2026 PortalDoctor contributors.
