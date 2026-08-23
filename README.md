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
# run all passive diagnostic checks (default command)
portaldoctor check

# restrict to desktop/session/environment discovery
portaldoctor check environment
portaldoctor check environment --verbose

# machine-readable output
portaldoctor check --json

# portal inspection
portaldoctor portal list
portaldoctor portal routes
portaldoctor portal explain ScreenCast
```

Also accepts `--version` and `--help`.

## Documentation

- [Finding catalog](docs/findings.md) — every rule, severity and trigger
  (`ENV001`–`ENV004`, `XDP001`–`XDP005`, `CFG001`–`CFG004`, `DBUS001`–`DBUS002`)
- [JSON schema v1](docs/json-schema.md) — the machine-readable report format
- [Privacy statement](docs/privacy.md) — what is collected, what is contacted
- [Compatibility & known limitations](docs/compatibility.md)
- [Project docs index](docs/PORTALDOCTOR_DOCS_INDEX.md) — research, PRD,
  architecture, roadmap

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Known limitations (v0.1)

- Validated on Ubuntu 26.04 / GNOME / Wayland / systemd only; other setups are
  untested (details in [compatibility](docs/compatibility.md)).
- ScreenCast readiness is judged from configuration and D-Bus reachability;
  PipeWire/WirePlumber state is a later phase.
- No active portal probes, no journal evidence, no automatic fixes.

See [compatibility & known limitations](docs/compatibility.md) for the full
list.

## License

[`MIT`](LICENSE) — Copyright (c) 2026 PortalDoctor contributors.