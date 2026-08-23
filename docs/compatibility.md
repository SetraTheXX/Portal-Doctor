# Compatibility & Known Limitations (v0.1)

## Supported environment

PortalDoctor v0.1 is developed and validated against exactly one target:

| Component | Supported |
|---|---|
| Distribution | Ubuntu 26.04 |
| Desktop | GNOME (incl. `ubuntu:GNOME` composite identifiers) |
| Session | Wayland |
| Init/service manager | systemd user session |
| Portal frontend | `org.freedesktop.portal.Desktop` |
| Backends | `xdg-desktop-portal-gnome`, `-gtk` and other descriptors discovered through the standard `.portal` mechanism |

Other distributions, desktops and sessions may work — the resolver follows
upstream `xdg-desktop-portal` semantics rather than hard-coding GNOME — but
they are untested in v0.1 and no support is claimed.

## What v0.1 does NOT cover

- **PipeWire / WirePlumber state** — ScreenCast routing is reported from
  configuration and D-Bus only; actual media-graph readiness arrives with the
  Phase 5 collector (`pw-dump`).
- **Journal evidence** — log correlation is a later phase; findings cite
  configuration and runtime reachability only.
- **Active probes** — v0.1 never calls portal interfaces, so end-to-end
  behavior of FileChooser/Screenshot/ScreenCast dialogs is not exercised.
- **KDE / wlroots / Hyprland / Niri** — no support claims; route resolution
  may work but is unvalidated.
- **Automatic fixes** — PortalDoctor diagnoses; it never edits configuration.
- **GUI** — CLI only.

## Known limitations

1. `NameHasOwner` proves a bus name has an owner; it does not verify the
   backend responds to method calls. A hung-but-registered backend can pass.
2. Backend-to-systemd-unit mapping assumes the conventional
   `xdg-desktop-portal-<backend>.service` naming. Units following other
   conventions are reported as `not found` without any finding.
3. `UseIn` matching is ASCII case-insensitive. Upstream comparisons are
   historically case-sensitive; practical desktop/descriptor pairs differ in
   case, so strict matching would misreport common setups.
4. Shell-wrapper scripts used as `systemctl` replacements leave grandchildren
   running for at most one timeout window after detection; direct binaries are
   reaped immediately on timeout (process-group kill).
5. Subprocess stdout larger than the pipe buffer (~64 KiB) causes that probe
   to be treated as timed out instead of captured.

## Reporting gaps

If you hit an unsupported setup, please open an issue with the JSON report
(`portaldoctor check --json`) attached — see SECURITY.md/README for where to
report.
