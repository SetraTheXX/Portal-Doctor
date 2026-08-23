# JSON Schema v1

`portaldoctor --json` emits a single, versioned document on stdout. Logs (if
any) go to stderr; stdout is always valid JSON.

## Top-level contract

```json
{
  "schema_version": 1,
  "portaldoctor_version": "0.1.0",
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
  }
}
```

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
  `missing_provider`, `dbus_timeout`, `service_state`, `journal_excerpt`
- `impact` may be `null` when severity already conveys the consequence.
- `recommendation` is ordered; the first entry is the primary next step.

## Versioning policy

- Additive fields do not bump `schema_version` during the v0.x series;
  consumers must ignore unknown keys.
- Breaking changes bump `schema_version` and are documented here before any
  tagged release.
