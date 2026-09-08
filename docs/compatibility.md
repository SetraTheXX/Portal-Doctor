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

## What the published v0.2 line does NOT cover

- **Published active probes** — v0.2.1 never calls portal interfaces. The
  development `main` branch has a separate, unreleased FileChooser probe, but
  it does not extend the v0.2.1 support claim or release contract.
- **Screenshot active lifecycle** — the published v0.2.1 package does not
  call portal interfaces. Development `main` contains the controlled v3+
  Window-only path and the trusted-GNOME v2 interactive compatibility path;
  v2 makes no Window-only claim and neither path is public or release-ready.
  The real GNOME success/cancellation gate is currently **BLOCKED** by the
  observed provider hang/crash. The same broken provider state must not be
  retried; the v3 Window-only path remains intact.
- **ScreenCast active lifecycle** — the published v0.2.1 package contains no
  active ScreenCast probe. Development `main` contains the internal
  `CreateSession -> SelectSources -> Start -> StreamsReturned ->
  OpenPipeWireRemote` lifecycle and its passing aggregate controlled audit,
  but no public `probe screencast` command and no real-session validation.
  The real gate is **BLOCKED** because the frontend reports
  `AvailableSourceTypes=0` and lacks Window bit `2`; no PipeWire
  media/handshake/capture is implemented.
- **KDE / wlroots / Hyprland / Niri** — no support claims; route resolution
  may work but is unvalidated. Phase 9 now has static KDE config/metadata and
  routing fixtures plus controlled KDE runtime-correlation tests for generic
  service/D-Bus finding semantics and KDE/Plasma Wayland environment/session
  fixtures, including a healthy/degraded passive snapshot aggregate and an
  isolated session-bus ownership gate plus a guarded production systemd-user
  collector gate for the exact KDE unit and bounded state mapping, plus a
  combined isolated D-Bus/systemd collector run through the existing passive
  rule pipeline; no Plasma runtime validation or support claim follows from
  them. The 2026-09-08 live Plasma preflight on the current host is
  **BLOCKED / NOT AVAILABLE**: the host is GNOME/Wayland, the KDE backend
  package and service are absent, and the KDE D-Bus name has no owner. Recheck
  only in a real Plasma Wayland session with a healthy KDE backend/service and
  KDE D-Bus ownership; do not repeatedly force the current host. Phase 10 now
  adds controlled static Sway/wlroots mixed-routing fixtures and rule coverage
  (`Screenshot`/`ScreenCast` → `wlr`, `FileChooser`/`Settings` → GTK), plus a
  controlled Sway Wayland passive aggregate where a healthy environment is clean
and a missing `WAYLAND_DISPLAY` yields only `ENV003`; there is still no live
Sway validation or wlroots support claim. Controlled runtime-correlation tests
also cover the generic `wlr`/GTK D-Bus and systemd mappings, with degraded
provider states remaining generic `DBUS002` findings; this is not live runtime
validation or a support claim. An explicit isolated production-collector gate
also covers the descriptor-derived D-Bus names and exact WLR/GTK systemd units
for healthy, missing and failed runtime states; it never contacts real user
systemd or a live portal provider. The production activation-environment
collector is also covered in a bounded isolated gate: equal Sway values are
clean, stale/missing activation values remain generic `ENV004`, and
unavailable/timeout collection does not invent a mismatch. This is controlled
coverage only; live Sway validation and support remain open.

### Phase 10 live Sway readiness checkpoint — 2026-09-09

The current host is Ubuntu 26.04.1 GNOME/Wayland, not a Sway session. A
read-only preflight found no `xdg-desktop-portal-wlr` package, user service or
canonical WLR D-Bus owner; passive Screenshot/ScreenCast routes select GNOME.
PipeWire and WirePlumber are running, but this does not make WLR active-probe
validation possible. The decision is **LIVE SWAY ENVIRONMENT BLOCKED / NOT
AVAILABLE** and **ACTIVE PROBE READINESS BLOCKED**. No active probe, provider
installation or support claim is made.

Recheck only after a real Sway Wayland session exposes a healthy WLR backend and
service, owns `org.freedesktop.impl.portal.desktop.wlr`, selects WLR in passive
routing, and advertises Screenshot v3 Window bit `2` plus ScreenCast
`AvailableSourceTypes` Window bit `2`. The current GNOME host must not be
repeatedly forced into this gate.
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
