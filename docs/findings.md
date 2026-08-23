# Finding Catalog (v0.1)

Every diagnostic rule produces a stable, structured finding (PRD §8): `id`,
`severity`, `confidence`, `title`, `summary`, `explanation`, `evidence`,
`impact`, `recommendation[]` and `source_component`. This catalog lists the
complete v0.1 registry; the rule engine test suite asserts that exactly these
IDs are registered.

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

## Notes

- Findings are deterministic: the same snapshot always yields the same
  findings in the same order (sorted by ID).
- Every finding carries at least one structured evidence item
  (`environment_mismatch`, `config_selection`, `missing_provider`,
  `dbus_timeout`, `service_state`, `journal_excerpt`) and at least one
  recommended next step.
- `journal_excerpt` evidence is reserved for the Phase 6 journal collector.
