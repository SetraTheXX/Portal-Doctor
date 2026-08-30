# Compatibility & Known Limitations (v0.2)

## Supported environment

PortalDoctor v0.2 is developed and validated against exactly one target:

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
they are untested in v0.2 and no support is claimed.

## What v0.2 does NOT cover

- **Active probes** — v0.2 never calls portal interfaces, so end-to-end
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
4. The v0.2 collector caps each `pw-dump`/`wpctl` stream at 16 MiB and treats
   overflow as unavailable. Normal desktop graphs are much smaller; no raw
   graph is retained in the snapshot.
5. The v0.2 journal collector is available only for sessions
   where `journalctl --user` can read a current-boot user journal. Missing or
   restricted journals are reported as a section status; they do not make the
   normal passive check fail. Journal excerpts are limited and sanitized.
6. Shareable reports intentionally exclude raw journal and PipeWire streams.
   They expose normalized, bounded evidence and can suppress the current
   hostname, but local paths and allowlisted session values should still be
   reviewed before public attachment.

## Reporting gaps

If you hit an unsupported setup, generate the shareable report and review it
before attaching it to an issue:

```sh
portaldoctor report --format markdown --suppress-hostname > portaldoctor-report.md
```

Use `portaldoctor report --json` when a machine-readable attachment is more
useful. The older `portaldoctor check --json` form remains the compatibility
snapshot output and should be reviewed manually before public sharing.
