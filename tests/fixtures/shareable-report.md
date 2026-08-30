# PortalDoctor diagnostic report
> Report v1 · Snapshot schema v1 · PortalDoctor `0.1.0`

## Summary
| Field | Value |
| --- | --- |
| Findings | 0 |
| Collected at (Unix ms) | 42 |
| Privacy mode | redacted |
| HOME paths | normalized to `$HOME` |
| Hostname | suppressed |
| Raw journal / PipeWire | excluded; normalized evidence only |

## System and session
### System
| Collection | unsupported: not collected |

### Session
| Collection | unsupported: not collected |

## Environment
| Collection | unavailable: test |

## Portal routing
| Configuration | unsupported: not collected |
| Backends | unsupported: not collected |
| Routes | unsupported: not collected |

## Runtime
| D-Bus collection | unsupported: not collected |
| Services collection | unsupported: not collected |

## Media path
| PipeWire collection | unsupported: not collected |
| WirePlumber collection | unsupported: not collected |

## Journal evidence
| Collection | unsupported: not collected |

## Findings
No findings were produced by the rule engine.

## Collection notes
- `system`: not collected
- `session`: not collected
- `environment`: test
- `portal_config`: not collected
- `portal_backends`: not collected
- `portal_routes`: not collected
- `dbus`: not collected
- `services`: not collected
- `pipewire`: not collected
- `wireplumber`: not collected
- `journal`: not collected

## Sharing checklist
- Environment keys are restricted to the existing allowlist.
- Home-directory paths: normalized to `$HOME`.
- Hostname suppression: enabled.
- Raw journal and raw PipeWire dumps are excluded; only bounded normalized evidence is present.
- Review the report once before attaching it to a public issue.
