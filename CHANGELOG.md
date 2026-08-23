# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 0: project foundation.
  - Rust CLI binary scaffold (`portaldoctor`).
  - `check` subcommand (default command), `--json`, `--version`, `--help`.
  - Core models: snapshot schema v1, collector status, finding, evidence,
    severity and confidence contracts.
  - Versioned JSON output contract (top-level `schema_version` v1).
  - Unit tests and GitHub Actions CI (fmt, clippy, test, release build).
- Phase 1: environment and desktop discovery.
  - Collectors: `/etc/os-release`, allowlisted `XDG`/session variables,
    effective `XDG` search roots, `systemd` user activation environment
    (bounded timeout).
  - Environment rules `ENV001`–`ENV004` with deterministic fixture tests.
  - Environment rules `ENV001`–`ENV004` with deterministic fixture tests.
  - `portaldoctor check environment [--verbose]`.
- Phase 4: diagnostic engine v1 — v0.1 release gate.
  - Finalized the v0.1 rule registry (15 rules) with a catalog test pinning
    registration to the documented set.
  - Every finding now carries the complete structured contract (explanation,
    impact, recommendation) asserted across rule fixture tests.
  - Default terminal output is actionable: each finding shows its first
    recommended step without `--verbose`.
  - Published JSON schema-v1 documentation (`docs/json-schema.md`) and the
    finding catalog (`docs/findings.md`).
- Phase 2: portal discovery and routing resolver.
  - `portals.conf` discovery with desktop-specific names and `XDG` precedence;
    parser supports `[preferred]`, `*`, `none` and provenance.
  - `.portal` backend discovery across effective `XDG` data roots with
    duplicate provenance.
  - Route resolver producing explainable, source-backed route tables.
  - Portal rules `XDP003`–`XDP005` and `CFG001`–`CFG004`.
  - `portaldoctor portal list | routes | explain <interface>`,
    `portaldoctor check portal`.
- Fixed: bounded subprocess helper (`output_bounded`) kills the whole child
  process group on timeout, so shell wrappers or grandchildren cannot survive
  as orphans; systemd user-service collector migrated to it.
- Phase 3: D-Bus and systemd runtime verification.
  - `zbus`-based session bus checks: frontend and selected backend bus names,
    classified outcomes (has owner, no owner, timeout, access denied,
    activation failure), bounded by the central timeout policy.
  - Portal-relevant systemd user unit states via bounded
    `systemctl --user show`.
  - Rules `DBUS001`–`DBUS002` and `XDP001`–`XDP002`; runtime findings included
    in bare `check` output.
- Phase 2 audit fixes: desktop names normalized to lowercase like upstream;
  `org.freedesktop.impl.portal.Default` preference acts as fallback and is
  overridden by interface-specific entries; route selection picks the first
  usable backend (single `Selected`); fixture tests for every portal rule
  (`XDP003`–`XDP005`, `CFG001`–`CFG004`).
- Correctness: `portals.conf` candidates now probe config roots followed by
  data roots per desktop (upstream order) with lowercased desktop names;
  the `default=` key is canonicalized to `org.freedesktop.impl.portal.Default`
  so parser output and resolver fallback lookup match byte-for-byte.