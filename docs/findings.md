# Finding Catalog

Every diagnostic rule produces a stable, structured finding (PRD §8): `id`,
`severity`, `confidence`, `title`, `summary`, `explanation`, `evidence`,
`impact`, `recommendation[]` and `source_component`. The first 15 IDs are the
published v0.1.0 catalog. The five media-stack IDs below are implemented on
`main` as the unreleased Phase 5 work for v0.2.0; the rule-engine test suite
asserts that the complete current registry is stable and unique. Phase 6 adds
optional journal excerpts as supporting evidence; it does not add a
journal-only diagnosis.

## Environment

| ID | Severity | Confidence | Fires when |
|---|---|---|---|
| `ENV001` | WARNING | HIGH | `XDG_CURRENT_DESKTOP` is absent from the session. |
| `ENV002` | WARNING | MEDIUM | `XDG_SESSION_TYPE` is missing or not one of `wayland`/`x11`. |
| `ENV003` | WARNING | HIGH | The session reports `wayland` but `WAYLAND_DISPLAY` is unset. |
| `ENV004` | WARNING | MEDIUM | Relevant variables differ between the session and the systemd user activation environment (`XDG_CURRENT_DESKTOP`, `XDG_SESSION_DESKTOP`, `XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, `DISPLAY`). |

## XDG Portal

| ID | Severity | Confidence | Fires when |
|---|---|---|---|
| `XDP001` | WARNING | HIGH | The frontend name `org.freedesktop.portal.Desktop` is unreachable on the session bus, or the probe times out (evidence: `dbus_timeout`). |
| `XDP002` | WARNING | MEDIUM | The frontend owns its bus name but its systemd user unit reports `failed` or is inactive — a contradictory runtime state. |
| `XDP003` | WARNING | HIGH | No `.portal` backend descriptors were discovered in any effective `XDG` data root. |
| `XDP004` | WARNING | HIGH | An interface listed in `[preferred]` has no available backend in this desktop context. Explicitly disabled (`none`) interfaces do not fire this rule. |
| `XDP005` | WARNING | HIGH | A `[preferred]` entry names a backend whose descriptor does not exist (`*` and `none` are exempt). |

## Configuration

| ID | Severity | Confidence | Fires when |
|---|---|---|---|
| `CFG001` | INFO/WARNING | HIGH | No `<desktop>-portals.conf` exists for the current desktop. WARNING when no config exists at all; INFO when only the generic `portals.conf` was found. |
| `CFG002` | WARNING | HIGH | The selected `portals.conf` contains malformed lines; the parse errors are quoted verbatim. |
| `CFG003` | WARNING | HIGH | A `[preferred]` entry names backends that exist but do not advertise the interface, while other providers do. |
| `CFG004` | INFO | LOW | An interface has two or more available providers and no `[preferred]` entry pins the choice (conservative by design). |

## D-Bus

| ID | Severity | Confidence | Fires when |
|---|---|---|---|
| `DBUS001` | WARNING | HIGH | No session bus could be reached; runtime verification was skipped. |
| `DBUS002` | WARNING | HIGH | A configured backend bus name has no owner or fails to activate while the session bus itself is reachable. |

## PipeWire and WirePlumber (main, unreleased v0.2.0)

These checks are passive. They run bounded `pw-dump --no-colors` and
`wpctl status` commands, retain only normalized portal-relevant facts, and do
not start a capture session or export the raw media graph.

| ID | Severity | Confidence | Fires when |
|---|---|---|---|
| `PW001` | WARNING | HIGH | `pw-dump` is unavailable or the PipeWire endpoint exits without providing a usable state result. |
| `PW002` | WARNING | HIGH | WirePlumber reachability cannot be verified through the bounded `wpctl status` query. |
| `PW003` | WARNING | MEDIUM | PipeWire was invoked but the query timed out, hit a permission boundary, or returned payload that could not be parsed safely. |

## ScreenCast correlation (main, unreleased v0.2.0)

| ID | Severity | Confidence | Fires when |
|---|---|---|---|
| `SC001` | WARNING | HIGH | The resolved `ScreenCast` route has no provider, or discovered providers do not advertise the `ScreenCast` interface. |
| `SC002` | WARNING | HIGH | A `ScreenCast` provider is selected, but an attempted PipeWire/WirePlumber collection is not available. |

`SC002` keeps route evidence (`screencast_route`) alongside the separate
media-stack evidence (`pipewire_state` and/or `wireplumber_state`). A selected
route alone is never treated as proof that a capture stream can work. A
completely absent backend descriptor remains the responsibility of `XDP003`,
and an explicitly disabled route does not trigger `SC001`.

## Optional journal correlation (main, unreleased Phase 6)

`portaldoctor --journal` reads only the current user boot's last 30 minutes
from an allowlist of portal, PipeWire, and WirePlumber units. The collector
requests structured journal records, limits the query to 80 entries and 512
KiB, keeps only stable portal-relevant error patterns, and sanitizes paths,
identities, host labels, and message length before the snapshot is built.

When a classified excerpt supports an existing `PW001`–`PW003` or `SC001`–`SC002`
finding, that finding receives `journal_excerpt` in addition to its
authoritative state/route evidence. Unknown or insufficient text is retained
as no evidence; it never becomes a diagnosis by itself. Use `--verbose` to
display the sanitized excerpts.

## Notes

- Findings are deterministic: the same snapshot always yields the same
  findings in the same order (sorted by ID).
- Every finding carries at least one structured evidence item
  (`environment_mismatch`, `config_selection`, `missing_provider`,
  `dbus_timeout`, `service_state`, `pipewire_state`, `wireplumber_state`,
  `screencast_route`, `journal_excerpt`) and at least one recommended next
  step.
- `journal_excerpt` is emitted only when `--journal` collected a matching,
  sanitized excerpt for the finding.
