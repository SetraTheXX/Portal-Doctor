# Contributing to PortalDoctor

Thanks for considering a contribution. PortalDoctor is intentionally narrow:
its PRD, architecture and roadmap are the source of truth.

## Before starting

- Check the roadmap phase and open issues first.
- Keep a PR inside one bounded phase or maintenance scope.
- Do not start the next roadmap phase automatically.
- Do not add a dependency when the standard library or an existing dependency
  is sufficient.
- Discuss changes to the JSON/snapshot contract, finding IDs or supported
  platform claims in an issue before implementation.

## Prerequisites

- Stable Rust toolchain with `rustfmt` and `clippy` components.
- Linux is required for the real-system validation; the validated v0.2 target
  is Ubuntu 26.04 / GNOME / Wayland / systemd user session.

## Development loop

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

CI runs the same Rust quality gates. Do not use `cargo fmt` as a hidden fix in
a PR; format locally, then commit the resulting source intentionally.

For the v0.2 end-to-end acceptance matrix on the validated Linux target:

```sh
cargo build --release
PORTALDOCTOR_BIN="$PWD/target/release/portaldoctor" \
  ./scripts/validate-v0.1-faults.py
```

The harness uses temporary XDG roots and a private D-Bus session. It must not
modify system configuration or stop/disable services.

## Scope and safety rules

- Default commands are passive and read-only; no system mutation is allowed.
- Normal diagnostics must not require root or sudo.
- All external commands and D-Bus operations need an explicit bounded timeout.
- Do not add PipeWire/WirePlumber, journal collection, active portal probes or
  desktop compatibility work outside their roadmap phase.
- Never print or commit secrets, credentials, tokens, private keys or raw
  environment dumps.
- Review JSON output before copying it into an issue.

## Findings and rules

A new or changed finding must include:

- stable ID, severity and confidence,
- non-empty explanation, impact and ordered recommendation,
- structured evidence and non-empty `source_component`,
- positive and negative fixture coverage,
- an update to [docs/findings.md](docs/findings.md),
- JSON schema review when the serialized shape changes.

Routing/configuration changes must cover precedence, lowercase desktop names,
`default`, interface-specific overrides, `*`, `none`, unavailable backends and
`UseIn` behavior where relevant.

## Pull requests

Open a focused branch and PR. A PR should contain:

1. Problem and user-visible behavior.
2. Exact roadmap/phase scope.
3. Implementation summary and deliberate non-goals.
4. Tests and commands run, including real-system validation when applicable.
5. Documentation and CHANGELOG updates for user-facing behavior.
6. Privacy/read-only/timeout impact.

Keep unrelated refactors, formatting-only churn and speculative features out of
the PR. Do not force-push shared branches or rewrite project history.

## Commits

Use one logical change per commit with a descriptive conventional subject such
as `feat:`, `fix:`, `docs:`, `test:` or `chore:`. Breaking JSON, snapshot or
finding-ID changes must be called out in the PR description.

## Bug reports

Use the bug-report template and include:

- `portaldoctor --version`,
- distribution, desktop, session type and systemd user-session details,
- the command that was run,
- expected and actual behavior,
- a reviewed `portaldoctor check --json` report when safe,
- a minimal reproduction or relevant fixture information.

Remove credentials, tokens and unrelated local values before posting output.

## Feature requests

Explain the user problem, proposed behavior, roadmap phase, scope boundary and
how deterministic/read-only behavior would be tested. Features that belong to a
later phase should be proposed there rather than implemented early.
