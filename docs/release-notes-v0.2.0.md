# PortalDoctor v0.2.0 — Release Notes

> Passive ScreenCast readiness, bounded runtime evidence and privacy-aware
> shareable reports.

PortalDoctor v0.2.0 completes the passive diagnostic path from session state
through portal routing and runtime evidence to an issue-friendly report. It
does not start portal dialogs, modify configuration or claim support for
desktop environments outside the validated target.

## Highlights

- Added bounded PipeWire (`pw-dump`) and WirePlumber (`wpctl status`) health
  collection with normalized, portal-relevant video topology.
- Added deterministic `PW001`–`PW003` and `SC001`–`SC002` findings so a
  ScreenCast route is not mistaken for a working media path.
- Added optional, bounded current-boot/user-session journal evidence with
  stable classification and privacy sanitization.
- Added `portaldoctor report` with terminal, JSON and Markdown formats.
- Added report-level redaction for environment values, home paths, obvious
  secret labels and optionally the current hostname.
- Raw journal and PipeWire streams are excluded from shareable reports and
  explicitly marked as excluded in their metadata.
- Reworked the README and added a slower, readable Terminalizer demo.

## Usage

Install the published crate:

```sh
cargo install portaldoctor --version 0.2.0 --locked
```

Run the default passive check:

```sh
portaldoctor
```

Create an issue-friendly attachment:

```sh
portaldoctor report --format markdown --suppress-hostname > portaldoctor-report.md
portaldoctor report --json > portaldoctor-report.json
```

Opt into bounded journal evidence when investigating a runtime failure:

```sh
portaldoctor --journal --verbose
```

## Compatibility boundary

The validated target is Ubuntu 26.04 with GNOME, Wayland and a systemd user
session. Active portal probes, KDE/wlroots validation, automatic fixes and a
GUI remain outside v0.2.0.

## Validation

The release gate passed formatting, strict Clippy, the full test suite, a
release build, the v0.1-compatible fault-injection harness, package
verification, live report checks and the final CI workflow.

## Privacy note

Review generated attachments before publishing them. The `report` command is
the shareable boundary; the legacy `portaldoctor --json` and
`portaldoctor check --json` outputs retain the compatibility snapshot shape
and require manual review.
