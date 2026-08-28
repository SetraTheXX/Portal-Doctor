# Changelog

All notable changes to PortalDoctor are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

No changes since v0.1.0.

## [0.1.0] — 2026-08-29

This is the first public PortalDoctor release.

- Reproducible three-scene README demo script and Terminalizer GIF.
- Fault-injection acceptance harness execution in CI.
- Normal diagnostic runs no longer emit informational tracing noise.
- Acceptance validation prefers the binary built by the current checkout.
- Snapshot rules are evaluated once per command run.

### Phase 0 — Project foundation

- Rust CLI binary scaffold (`portaldoctor`).
- `check` command (default command), `--json`, `--version` and `--help`.
- Snapshot schema v1, collection status, finding, evidence, severity and
  confidence models.
- Versioned JSON output contract and terminal/JSON renderers.
- MIT license, README, contributor/security docs and GitHub Actions CI.

### Phase 1 — Environment and desktop discovery

- `/etc/os-release` collection and parsing.
- Allowlisted XDG/session environment collection and effective XDG config/data
  roots.
- Wayland/X11 session discovery and process versus systemd activation comparison.
- Deterministic `ENV001`–`ENV004` rules and
  `portaldoctor check environment [--verbose]`.

### Phase 2 — Portal discovery and routing

- Desktop-specific and generic `portals.conf` discovery across config and data
  roots with upstream precedence and lowercase desktop names.
- `[preferred]` parsing, including `default=`, interface-specific overrides,
  `*`, `none` and source provenance.
- `.portal` backend discovery across effective XDG data roots with duplicate
  provenance.
- Explainable route resolution and deterministic selection.
- `XDP003`–`XDP005` and `CFG001`–`CFG004` rules.
- `portal list`, `portal routes`, `portal explain <interface>` and
  `check portal`.

### Phase 3 — D-Bus and systemd runtime verification

- `zbus` session-bus checks for the portal frontend and selected backend names.
- Classified runtime outcomes: owner, no owner, no session bus, timeout,
  access denied, activation failure and malformed response.
- Bounded `systemctl --user show-environment` and portal-unit state collection.
- Central timeout policy and process-group cleanup for timed-out subprocesses.
- `DBUS001`–`DBUS002` and `XDP001`–`XDP002` rules.

### Phase 4 — Diagnostic engine v1 and release gate

- Finalized and tested the complete 15-rule v0.1 registry.
- Completed structured finding fields: explanation, evidence, impact,
  recommendations and source component.
- Actionable terse terminal output with the first recommended next step.
- Published finding catalog, JSON schema-v1 reference, privacy statement,
  compatibility matrix and known limitations.
- Public-facing README and v0.1.0 release notes.
- Validation on Ubuntu 26.04 / GNOME / Wayland / systemd user session.

## Deferred after v0.1

- PipeWire/WirePlumber state and ScreenCast media-stack correlation (Phase 5).
- Journal evidence engine.
- Active portal probes.
- KDE, wlroots/Sway, Hyprland and Niri compatibility claims.
- Automatic fixes and GUI.

[Unreleased]: https://github.com/SetraTheXX/Portal-Doctor/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/SetraTheXX/Portal-Doctor/releases/tag/v0.1.0
