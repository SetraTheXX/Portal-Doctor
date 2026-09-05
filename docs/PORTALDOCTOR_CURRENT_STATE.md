# PortalDoctor — Current State and Handoff

**Last verified:** 2026-09-05
**Current public release:** `v0.2.1`
**Current development phase:** Phase 8 / `v0.3.0`
**Primary next issue:** [#3 — Active FileChooser, Screenshot and ScreenCast probes](https://github.com/SetraTheXX/Portal-Doctor/issues/3)

This is the canonical handoff document for a new maintainer or coding agent.
Read it before changing code, documentation or GitHub planning metadata.

## One-minute project summary

PortalDoctor is a passive, read-only Rust CLI that explains Linux desktop
portal failures by correlating session/environment state, XDG portal routing,
D-Bus and systemd user services, PipeWire/WirePlumber health, optional bounded
journal evidence and shareable reports.

The published `v0.2.1` line is a stabilized passive diagnostic product. The
next product step is not another stabilization pass: it is the explicitly
invoked, bounded active-probe work in Phase 8.

## Release and repository state

- `v0.2.1` is published on [crates.io](https://crates.io/crates/portaldoctor)
  and in the [GitHub release](https://github.com/SetraTheXX/Portal-Doctor/releases/tag/v0.2.1).
- The release includes the documented exit-code contract, locked package and
  install smoke coverage, the readable four-scene demo, a Linux x86_64 binary
  and a SHA256 checksum asset.
- `main` is the active branch. Before starting work, verify
  `git status --short --branch`, `git log -1 --oneline --decorate` and
  `git diff --check`.
- The default product path remains passive, read-only, rootless and bounded.

## Completed roadmap scope

- Phases 0–4: project foundation, environment/session discovery, portal routing,
  D-Bus/systemd verification and the v0.1 diagnostic engine.
- Phases 5–7: bounded PipeWire/WirePlumber evidence, opt-in journal evidence,
  privacy-aware terminal/JSON/Markdown reports and the v0.2.0 release.
- v0.2.1: exit-code semantics, locked package/install gates, release assets,
  demo/release alignment and final passive-path stabilization.

Do not repeat the historical Phase 0–7 bootstrap work unless a regression is
demonstrated by a test or a supported user report.

## Validated scope and boundaries

The documented and validated baseline is:

- Ubuntu 26.04
- GNOME, including the `ubuntu:GNOME` identifier
- Wayland
- systemd user session
- `org.freedesktop.portal.Desktop`

Other distributions and desktops may work, but they are not v0.2 support
claims. The following remain outside the published v0.2.1 boundary:

- active portal method/dialog probes,
- validated KDE, wlroots/Sway, Hyprland or Niri behavior,
- automatic fixes,
- GUI workflows.

The docs.rs build warning is expected for this binary-only package; local binary
documentation generation succeeds and PortalDoctor does not promise a library
API.

## Current next step: Phase 8 / v0.3.0

The detailed executable checklist is [GitHub Issue #3](https://github.com/SetraTheXX/Portal-Doctor/issues/3).
The ASHPD decision checkpoint is recorded in
[`PORTALDOCTOR_ASHPD_DECISION.md`](PORTALDOCTOR_ASHPD_DECISION.md).
Implement it in this order:

1. [x] Evaluate and record the ASHPD integration strategy and its compatibility
   implications. The accepted boundary is PortalDoctor-owned lifecycle control
   with ASHPD used only where its public API preserves the required request and
   cleanup observability.
2. Define a stable machine-readable `ProbeResult` contract before adding
   user-facing findings.
3. Implement the first bounded FileChooser probe only.
4. Add success, user-cancellation, timeout, unavailable-backend, malformed
   response and transport-failure coverage, including cleanup assertions.
5. Validate one real supported desktop session before starting Screenshot or
   ScreenCast probe implementation.

Active probes must never run from `portaldoctor` or `portaldoctor check` by
default. They must clearly warn about possible dialogs, remain rootless and
read-only, use the central timeout policy and clean up every request/session
resource on success, cancellation and failure.

### First bounded task acceptance criteria

The first task is complete only when all of the following are true:

- the ASHPD strategy and its trade-offs are recorded in the implementation
  documentation,
- the probe result states and JSON shape are stable enough to test,
- `portaldoctor probe filechooser` is explicit and does not affect passive
  commands,
- success, cancellation, timeout, unavailable service and malformed response
  are distinguishable and covered by fixtures or mocks,
- no selected file is read, copied or modified,
- request/session resources are cleaned up on every path,
- the supported real-session validation path is documented,
- `cargo fmt --check`, strict locked Clippy, locked tests, locked release build,
  locked package and clean-root install smoke all pass.

Do not implement all three probe families in the first slice and do not begin
desktop expansion or remediation as part of it.

The current change stops after item 1. Do not start `ProbeResult` or any probe
implementation until this decision record has been accepted as the design
boundary for the next implementation slice.

## Quality gates

Run the relevant gates from the repository root before reporting completion:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --release
cargo package --locked
install_root="$(mktemp -d -t portaldoctor-smoke.XXXXXX)"
cargo install --path . --locked --root "$install_root"
"$install_root/bin/portaldoctor" --version
PORTALDOCTOR_BIN=target/release/portaldoctor \
  python3 scripts/validate-v0.1-faults.py
```

For release-facing changes, also verify the GitHub Actions run, release asset
checksum and crates.io installation. Keep `cargo audit` clean when dependency
changes are introduced.

## Documentation and planning rules

- Update the README only when user-facing behavior, supported scope or release
  state changes; do not use it as the task tracker.
- Use Issue #3 for the v0.3.0 implementation checklist and its acceptance gate.
- Use the roadmap for phase boundaries and release mapping.
- Keep issue #7 as the high-level sequence tracker; its current baseline must
  mention v0.2.1 before v0.3.0.
- Preserve the checked-in demo’s role as a v0.2.1 passive diagnostic showcase;
  do not imply that it demonstrates active probes.
