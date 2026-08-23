# PortalDoctor

A deterministic, read-only diagnostic CLI for XDG Desktop Portals, Wayland,
PipeWire and Linux desktop integration.

**Status:** Early development — no public release yet.

**Platform scope:** Linux only (initial target: Ubuntu 26.04, GNOME, Wayland,
systemd user session).

## Design principles

- deterministic diagnostics with stable finding IDs,
- read-only by default — no system modification,
- no root/sudo requirement for normal checks,
- no AI dependency in core behavior,
- machine-readable JSON output with a versioned schema,
- privacy-safe, redacted reports.

## Build

```sh
cargo build --release
```

## Usage

```sh
# run the passive diagnostic check (default command)
portaldoctor check

# machine-readable output
portaldoctor check --json
```

Also accepts `--version` and `--help`.

## Documentation

See [docs/PORTALDOCTOR_DOCS_INDEX.md](docs/PORTALDOCTOR_DOCS_INDEX.md) for the
project documentation index (research, PRD, architecture, roadmap).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[`MIT`](LICENSE) — Copyright (c) 2026 PortalDoctor contributors.