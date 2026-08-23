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
  - `portaldoctor check environment [--verbose]`.