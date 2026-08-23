# Contributing to PortalDoctor

Thanks for considering a contribution.

## Prerequisites

- Rust toolchain (stable) with `rustfmt` and `clippy` components.

## Development loop

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

CI runs exactly these checks; keep them green before opening a pull request.

## Scope

Follow the roadmap and PRD. This project is deliberately narrow:

- default commands are passive, read-only, and must never modify the system,
- no root/sudo requirement for normal diagnostics,
- no AI, network or telemetry dependency in the core diagnostic path.

## Commits

- One logical change per commit, descriptive subject line.
- Changes to the JSON/snapshot contract or rule IDs are breaking; discuss them
  in an issue before opening a PR.

## Reporting issues

Include the output of `portaldoctor check --json` plus a short description of
the environment (distribution, desktop, session type) and what you expected to
see.