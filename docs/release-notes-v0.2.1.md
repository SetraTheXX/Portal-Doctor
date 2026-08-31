# PortalDoctor v0.2.1 — Release Notes

> Stabilization release for the passive Linux desktop portal diagnostic path.

PortalDoctor v0.2.1 keeps the v0.2 passive scope and makes its automation,
release and user-facing contracts explicit. It does not add active portal
probes, automatic fixes or new desktop support claims.

## Highlights

- Finalized diagnostic exit codes for clean, severe-finding, incomplete-context,
  parser and internal-output outcomes.
- Added locked package and clean-root install smoke checks to CI.
- Extended the fault-injection acceptance harness to validate parser and
  runtime exit codes while preserving deterministic JSON findings.
- Refreshed the README demo with a slower, readable four-scene flow:
  health, routing, shareable report and controlled fault.
- Kept the validated support boundary at Ubuntu 26.04, GNOME, Wayland and a
  systemd user session.

## Usage

Install the published crate:

```sh
cargo install portaldoctor --version 0.2.1 --locked
```

Run the default passive check:

```sh
portaldoctor
```

Create a privacy-aware issue attachment:

```sh
portaldoctor report --format markdown --suppress-hostname > portaldoctor-report.md
portaldoctor report --json > portaldoctor-report.json
```

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Completed with no ERROR/CRITICAL finding; INFO/WARNING findings are allowed. |
| `1` | Completed with at least one ERROR/CRITICAL finding. |
| `2` | Invalid CLI usage or arguments. |
| `3` | Minimum graphical session/display or user D-Bus context is unavailable. |
| `4` | Output or internal process error prevented completion. |

## Compatibility boundary

The validated target is Ubuntu 26.04 with GNOME, Wayland and a systemd user
session. Active portal probes, KDE/wlroots validation, automatic fixes and a
GUI remain outside v0.2.1.

## Validation

The release gate covers formatting, strict locked Clippy, the full test suite,
a locked release build, package verification, clean-root install, JSON and
Markdown smoke checks, parser/runtime exit-code checks and the deterministic
fault-injection harness.
