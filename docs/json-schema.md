# JSON Schema v1

`portaldoctor --json` emits a single, versioned document on stdout. Logs (if
any) go to stderr; stdout is always valid JSON.

## Top-level contract

```json
{
  "schema_version": 1,
  "portaldoctor_version": "0.2.1",
  "snapshot": { "...": "see below" },
  "findings": [ { "...": "see below" } ]
}
```

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | integer | `1` for this format. Bumped only on breaking changes. |
| `portaldoctor_version` | string | Version of the producing binary. |
| `snapshot` | object | Normalized state collected during the run (architecture §6). |
| `findings` | array | Deterministic diagnostic results, sorted by rule ID. |

## Shareable report envelope

The explicit report command emits a privacy-aware document with a separate
document version:

```sh
portaldoctor report
portaldoctor report --json
portaldoctor report --format markdown
```

`--json` selects the JSON format for the report command. Its top-level JSON
shape is:

```json
{
  "report_version": 1,
  "schema_version": 1,
  "portaldoctor_version": "0.2.1",
  "privacy": {
    "redacted": true,
    "home_normalized": true,
    "hostname_suppressed": false,
    "raw_journal": "excluded",
    "raw_pipewire": "excluded"
  },
  "snapshot": { "...": "redacted snapshot v1" },
  "findings": []
}
```

`report_version` versions the shareable envelope; `schema_version` continues
to version the normalized snapshot contract. The report path applies the
environment allowlist, `$HOME` normalization and obvious secret masking
before this document is serialized. Add `--suppress-hostname` when the
hostname should also be replaced. Raw journal and raw PipeWire dumps are
excluded rather than embedded; only bounded normalized evidence can appear.
There is intentionally no raw-export flag in this shareable envelope because
the collectors discard those streams after bounded parsing.

The legacy `portaldoctor --json` and `portaldoctor check --json` output keeps
the original v1 top-level shape for machine compatibility. Treat that form as
diagnostic data to review, not as the public-issue-safe report format.

## Snapshot sections

Every section is an object with:

- `status`: one of `available`, `unavailable`, `unsupported`, `timed_out`,
  `permission_denied`, `parse_error` — collectors never merge "not supported"
  with "failed unexpectedly" (architecture §3.3).
- `value`: the collected data, present only when `status` is `available`.
- `errors`: array of `{ "message": string }` notes explaining failures.

```json
{
  "schema_version": 1,
  "collected_at": 1756029000000,
  "system": {
    "status": "available",
    "value": {
      "id": "ubuntu",
      "name": "Ubuntu",
      "pretty_name": "Ubuntu 26.04 LTS",
      "version_id": "26.04"
    },
    "errors": []
  },
  "session": {
    "status": "available",
    "value": {
      "current_desktop": "ubuntu:GNOME",
      "session_desktop": "ubuntu",
      "session_type": "wayland",
      "session_type_raw": "wayland",
      "wayland_display": "wayland-0",
      "display": ":0"
    },
    "errors": []
  },
  "environment": {
    "status": "available",
    "value": {
      "process": { "XDG_CURRENT_DESKTOP": "ubuntu:GNOME", "...": "..." },
      "search_roots": {
        "config_roots": ["/home/user/.config", "/etc/xdg"],
        "data_roots": ["/home/user/.local/share", "/usr/share"]
      },
      "activation_comparison": {
        "performed": true,
        "entries": [
          {
            "key": "XDG_CURRENT_DESKTOP",
            "process_value": "ubuntu:GNOME",
            "activation_value": "ubuntu:GNOME",
            "relation": "equal"
          }
        ]
      }
    },
    "errors": []
  },
  "portal_config": {
    "status": "available",
    "value": {
      "candidate_files": [".../xdg-desktop-portal/gnome-portals.conf"],
      "selected_file": "/usr/share/xdg-desktop-portal/gnome-portals.conf",
      "preferences": [
        {
          "interface": "org.freedesktop.impl.portal.Default",
          "backends": ["gnome", "gtk"],
          "source_file": "/usr/share/xdg-desktop-portal/gnome-portals.conf",
          "source_priority": 20
        }
      ],
      "parse_errors": []
    },
    "errors": []
  },
  "portal_backends": {
    "status": "available",
    "value": [
      {
        "id": "gnome",
        "descriptor_path": "/usr/share/xdg-desktop-portal/portals/gnome.portal",
        "duplicate_descriptors": [],
        "dbus_name": "org.freedesktop.impl.portal.desktop.gnome",
        "interfaces": ["org.freedesktop.impl.portal.ScreenCast"],
        "legacy_use_in": ["gnome"]
      }
    ],
    "errors": []
  },
  "portal_routes": {
    "status": "available",
    "value": [
      {
        "interface": "org.freedesktop.impl.portal.ScreenCast",
        "requested_candidates": ["gnome", "gtk"],
        "available_candidates": ["gnome"],
        "selected_candidates": ["gnome"],
        "evidence": [{ "message": "preferred entry from ... (priority 20): gnome, gtk" }],
        "status": "selected"
      }
    ],
    "errors": []
  },
  "dbus": {
    "status": "available",
    "value": {
      "connected": true,
      "checks": [
        {
          "name": "org.freedesktop.portal.Desktop",
          "outcome": "has_owner"
        }
      ]
    },
    "errors": []
  },
  "services": {
    "status": "available",
    "value": {
      "units": [
        {
          "unit": "xdg-desktop-portal.service",
          "state": "active",
          "sub_state": "running",
          "unit_file_state": "static"
        }
      ]
    },
    "errors": []
  },
  "pipewire": {
    "status": "available",
    "value": {
      "model_version": 1,
      "version": "1.6.2",
      "object_count": 81,
      "node_count": 10,
      "link_count": 3,
      "portal_client_count": 1,
      "screen_cast_source_count": 1,
      "nodes": [
        {
          "id": 42,
          "media_class": "Stream/Output/Video",
          "state": "running",
          "is_video_source": false,
          "is_screen_cast_source": true
        }
      ],
      "links": [
        {
          "id": 77,
          "output_node_id": 42,
          "input_node_id": 43,
          "media_type": "video",
          "state": "active"
        }
      ]
    },
    "errors": []
  },
  "wireplumber": {
    "status": "available",
    "value": {
      "model_version": 1,
      "pipewire_version": "1.6.2",
      "wireplumber_client_count": 2
    },
    "errors": []
  },
  "journal": {
    "status": "available",
    "value": {
      "model_version": 1,
      "window_minutes": 30,
      "max_entries": 80,
      "scanned_entry_count": 2,
      "ignored_entry_count": 1,
      "match_state": "matched",
      "entries": [
        {
          "unit": "pipewire.service",
          "priority": 3,
          "classification": "pipewire",
          "message": "PipeWire failed for <path> user=<redacted>"
        }
      ]
    },
    "errors": []
  }
}
```

### PipeWire and WirePlumber sections

The `pipewire` section is populated from a bounded `pw-dump --no-colors`
query. `object_count`, `node_count` and `link_count` describe the complete
graph without serializing it. `nodes` and `links` contain only normalized
video-relevant topology: numeric IDs, media class/type, state and boolean
ScreenCast/source flags. Arbitrary node names, application names, host names
and raw properties are intentionally discarded.

The `wireplumber` section is populated from bounded `wpctl status` output and
retains only the PipeWire version and a minimal WirePlumber client count. Both
models carry their own `model_version` so future additive normalization can be
reviewed independently of the top-level schema.

The `journal` section is populated only when `--journal` is supplied. It asks
`journalctl --user` for the current boot's last 30 minutes and only the
allowlisted `xdg-desktop-portal*.service`, `pipewire*.service`, and
`wireplumber.service` units. The query is bounded to 80 records and 512 KiB.
Only `_SYSTEMD_USER_UNIT`/`_SYSTEMD_UNIT`, `PRIORITY`, and `MESSAGE` are
considered; retained messages must match a stable portal-relevant error
pattern and are sanitized before serialization. `match_state` is `matched`,
`no_relevant_evidence`, or `insufficient_evidence`.

Without `--journal`, the section is `unsupported` with a `not requested` note.
If the journal is unavailable, denied, times out, exceeds the output limit, or
returns malformed data, `value` is omitted and the section status explains the
boundary. Journal evidence augments authoritative state and route findings;
it is never the sole source of a diagnosis.

When a command is missing, denied, times out, exceeds the output limit, exits
unsuccessfully or returns malformed data, `value` is omitted and the section's
`status` plus `errors` explain the boundary. The default check remains
passive: no portal dialog, capture session or full media-graph export is
started.

### Route statuses

- `selected` — at least one backend serves the interface.
- `disabled` — explicitly disabled through `none`.
- `no_provider` — no backend can serve it in this desktop context.

### D-Bus outcomes

`has_owner`, `no_owner`, `no_session_bus`, `activation_failure`, `timeout`,
`access_denied`, `malformed_response`, `other` (with message).

## Findings

Each finding follows PRD §8:

```json
{
  "id": "ENV001",
  "severity": "warning",
  "confidence": "high",
  "title": "XDG desktop identity is missing",
  "summary": "XDG_CURRENT_DESKTOP is not set in this session.",
  "explanation": "Applications and portal frontends read XDG_CURRENT_DESKTOP ...",
  "evidence": ["environment_mismatch"],
  "impact": "Portal backend selection may silently misbehave.",
  "recommendation": ["Start the graphical session through the normal desktop launcher."],
  "source_component": "environment"
}
```

- `severity`: `info` | `warning` | `error` | `critical`
- `confidence`: `low` | `medium` | `high`
- `evidence`: one or more of `environment_mismatch`, `config_selection`,
  `missing_provider`, `dbus_timeout`, `service_state`, `pipewire_state`,
  `wireplumber_state`, `screencast_route`, `journal_excerpt`
- `impact` may be `null` when severity already conveys the consequence.
- `recommendation` is ordered; the first entry is the primary next step.

## Versioning policy

- Additive fields do not bump `schema_version` during the v0.x series;
  consumers must ignore unknown keys.
- Breaking changes bump `schema_version` and are documented here before any
  tagged release.
