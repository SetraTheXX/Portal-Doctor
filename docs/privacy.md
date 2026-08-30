# Privacy Statement

PortalDoctor is designed to produce shareable diagnostic reports without
leaking personal data.

## What is collected

Only a fixed allowlist of environment variables is ever read
(architecture §7):

```text
XDG_CURRENT_DESKTOP, XDG_SESSION_DESKTOP, XDG_SESSION_TYPE,
WAYLAND_DISPLAY, DISPLAY, XDG_CONFIG_HOME, XDG_CONFIG_DIRS,
XDG_DATA_HOME, XDG_DATA_DIRS, DBUS_SESSION_BUS_ADDRESS,
XDG_RUNTIME_DIR
```

Anything outside this list — including `PATH`, shell history, credentials
and application state — is never collected or reported.

The single exception to "never read" is `HOME`: it is consulted **only** to
build the `XDG` base-directory default paths (e.g. `$HOME/.config`,
`$HOME/.local/share`) when the corresponding `XDG_CONFIG_HOME` /
`XDG_DATA_HOME` variables are unset. It is never collected as a diagnostic
variable and never reported on its own.

## What is contacted

- The session D-Bus: name-owner lookups for the portal frontend and the
  selected backends only (`NameHasOwner`). No methods are called on portal
  services; no dialogs are triggered.
- systemd user manager (read-only): `systemctl --user show-environment` and
  `systemctl --user show <unit>` for portal-relevant units.
- `journalctl --user` only when `--journal` is supplied: current boot, the
  last 30 minutes, and the allowlisted portal/PipeWire/WirePlumber units. Only
  structured unit, priority, and message fields are inspected.
- Files: `/etc/os-release`, effective `xdg-desktop-portal` config and
  `.portal` descriptor files.

No network access. No telemetry. No AI services. Nothing is written to disk
by default.

## Known value exposure

The allowlisted variables themselves can contain user identifiers (for
example `/run/user/1000` or flatpak export paths under the home directory).
These values can still be useful for diagnosis, so the explicit shareable
report command applies a second privacy boundary before serialization:

```sh
portaldoctor report
portaldoctor report --format markdown --suppress-hostname > portaldoctor-report.md
portaldoctor report --json > portaldoctor-report.json
```

Shareable reports:

- keep only the existing environment allowlist; arbitrary variables such as
  `PATH` are removed even if a future collector accidentally supplies them,
- normalize the current home directory and embedded home paths to `$HOME`,
- mask obvious `token=`, `secret=`, `password=`, credential and authorization
  values,
- replace the current hostname and `host=`/`hostname=` labels when
  `--suppress-hostname` is supplied,
- include explicit `report_version`, `schema_version` and `privacy` metadata,
  including the fact that raw journal/PipeWire dumps are excluded.

There is intentionally no `--include-raw` switch in the shareable command:
the collectors discard raw journal/PipeWire streams after their bounded parse,
which is stricter than making a raw export merely opt-in.

Opt-in journal messages are sanitized before entering the snapshot: absolute
paths, `user=`/`host=` labels, email-like identities, unrelated records, and
overly long messages are not retained. The normalized PipeWire model never
retains the raw `pw-dump` property graph.

The legacy `portaldoctor --json` and `portaldoctor check --json` forms remain
available as the v1 machine-readable compatibility output. Use `portaldoctor
report` when the output is intended for a public issue, and review it once
before attaching it: display values, OS identity, numeric runtime IDs and
diagnostic topology can still be useful system fingerprints.

## Guarantees

- Read-only by default; no system modification, no root requirement.
- All external interactions are bounded by timeouts (2–3 s) and output limits,
  so a wedged dependency cannot hang the tool.
- Shareable reports are designed to exclude obvious secrets: only allowlisted
  variables and their redacted values are serialized, raw process dumps are
  never emitted, and raw journal/PipeWire output is excluded. Review the
  generated attachment before publishing it.
