# Changelog

All notable changes to PortalDoctor are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Added bounded, passive PipeWire (`pw-dump`) and WirePlumber (`wpctl status`)
  collection with privacy-safe normalized video topology.
- Added deterministic Phase 5 findings `PW001`–`PW003` and `SC001`–`SC002`,
  keeping portal routing evidence separate from media-stack evidence.
- Added snapshot schema documentation for the new `pipewire` and `wireplumber`
  sections; the top-level schema remains v1 because the fields are additive.
- Added opt-in bounded current-boot/user-session journal evidence for
  allowlisted portal, PipeWire, and WirePlumber units.
- Added structured journal parsing, stable error classification, message
  sanitization, and `journal_excerpt` correlation on existing media findings.
- Added fixture coverage for empty, unavailable, timed-out, noisy, malformed,
  and representative journal input.
- Added the explicit `portaldoctor report` command with terminal, JSON and
  Markdown output formats plus report/schema version metadata.
- Added report-level redaction: environment allowlist enforcement, `$HOME`
  normalization, secret-pattern masking, and optional hostname suppression.
- Marked raw journal and raw PipeWire dumps as excluded in shareable report
  metadata; only bounded normalized evidence is emitted.
- Added shareable-report redaction tests and a stable Markdown golden fixture.
- Reworked the README into a product-focused quick-start and reference guide.
- Re-recorded the Terminalizer demo with a larger canvas, slower pacing,
  readable color accents, and no renderer title bar.
- Added the checked-in `docs/demo/terminalizer.yml` recording configuration.

## [0.1.0] — 2026-08-29

This is the first public PortalDoctor release.

- Published the `portaldoctor` package to crates.io.
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

## Remaining after Phase 7

- Active portal probes.
- KDE, wlroots/Sway, Hyprland and Niri compatibility claims.
- Automatic fixes and GUI.

[Unreleased]: https://github.com/SetraTheXX/Portal-Doctor/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/SetraTheXX/Portal-Doctor/releases/tag/v0.1.0
