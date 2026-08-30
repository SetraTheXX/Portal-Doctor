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

- **Published v0.1.0 media contract** — the crates.io package reports
  ScreenCast routing from configuration and D-Bus only. The Phase 5
  PipeWire/WirePlumber collector is implemented on `main` but is not part of
  the published v0.1.0 artifact.
- **Journal evidence in the published artifact** — v0.1.0 does not include
  journal correlation. The unreleased `main` build has an opt-in
  `--journal` collector, which depends on a readable systemd user journal.
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
4. The unreleased Phase 5 collector caps each `pw-dump`/`wpctl` stream at
   16 MiB and treats overflow as unavailable. Normal desktop graphs are much
   smaller; no raw graph is retained in the snapshot.
5. The unreleased Phase 6 journal collector is available only for sessions
   where `journalctl --user` can read a current-boot user journal. Missing or
   restricted journals are reported as a section status; they do not make the
   normal passive check fail. Journal excerpts are limited and sanitized.

## Reporting gaps

If you hit an unsupported setup, please open an issue with the JSON report
(`portaldoctor check --json`) attached — see SECURITY.md/README for where to
report.
