# PortalDoctor v0.1.0 — Release Notes

> First public release of the read-only PortalDoctor diagnostic CLI.

## What it solves

Linux portal failures are distributed across session environment, XDG search
paths, `portals.conf`, `.portal` backend descriptors, D-Bus and systemd user
services. PortalDoctor collects those layers in one read-only snapshot,
reconstructs backend routing and reports deterministic, evidence-backed
findings.

It helps explain wrong or missing backend routing, malformed portal
configuration, a selected backend that lacks an interface, missing Wayland
session values, activation-environment mismatches, an unreachable portal
frontend and unavailable selected backend names.

## Highlights

- OS, desktop, session and allowlisted environment discovery.
- Process versus systemd user activation-environment comparison.
- Upstream-style config/data root precedence and lowercase desktop-specific
  `portals.conf` lookup.
- `[preferred]` routing including `default=`, interface-specific overrides,
  `*` and `none`.
- `.portal` backend inventory, `UseIn` filtering and duplicate provenance.
- Explainable route tables with requested, available and selected candidates.
- D-Bus frontend/selected-backend name-owner checks with classified failures.
- Bounded systemd user service-state checks.
- 15 deterministic findings across `ENV`, `XDP`, `CFG` and `DBUS` families.
- Actionable terminal output and versioned JSON schema v1.
- 109 fixture/unit tests and CI quality gates.

## Supported environment

The v0.1 validation target is:

- Ubuntu 26.04
- GNOME
- Wayland
- systemd user session
- XDG Desktop Portal frontend
- GNOME/GTK portal backends

Other environments are not claimed as supported by this release.

## Installation

From the repository:

```sh
cargo install --git https://github.com/SetraTheXX/Portal-Doctor --locked
```

Or build from source:

```sh
git clone https://github.com/SetraTheXX/Portal-Doctor.git
cd Portal-Doctor
cargo build --release
```

## Usage

```sh
portaldoctor
portaldoctor check environment
portaldoctor check portal
portaldoctor check --json
portaldoctor portal list
portaldoctor portal routes
portaldoctor portal explain ScreenCast
```

## Safety and privacy

- Read-only by default; normal use does not require root or sudo.
- No network access, telemetry or AI service.
- Only 11 diagnostic environment variables are allowlisted.
- `HOME` is read only to derive XDG default roots when the corresponding XDG
  variables are unset; it is not collected or reported as a diagnostic
  variable.
- JSON output should be reviewed before sharing. v0.1 does **not** claim a
  general-purpose shareable redaction layer.
- D-Bus and subprocess interactions are bounded; timed-out child process groups
  are killed and reaped.

## Limitations

- No PipeWire or WirePlumber graph/state collection.
- ScreenCast readiness stops at portal routing and D-Bus/systemd reachability.
- No journal correlation and no active portal probes/dialogs.
- `NameHasOwner` proves bus ownership, not method-level backend health.
- Backend-to-systemd-unit mapping follows conventional unit names.
- No automatic fixes and no GUI.
- KDE, wlroots/Sway, Hyprland and Niri are not supported claims.

## Explicitly not included

**Phase 5 PipeWire/WirePlumber integration is not part of v0.1.** It remains a
planned next roadmap phase and is intentionally absent from this release.

## Validation

The release-preparation baseline was validated with:

- `cargo fmt --check`
- strict Clippy (`cargo clippy --all-targets --all-features -- -D warnings`)
- `cargo test --all-features`
- `cargo build --release`
- real Ubuntu 26.04 / GNOME / Wayland bare invocation and JSON output checks
