# Active ProbeResult Schema v1

**Status:** Defined for Phase 8 / v0.3.0
**Scope:** Shared result contract only; no active probe or CLI command is implemented

This document defines the machine-readable result that future explicit
FileChooser, Screenshot and ScreenCast probes will emit. It does not change the
published v0.2.1 passive commands or their existing `--json` document.

## Canonical shape

An active probe result is a standalone document with its own schema version:

```json
{
  "schema_version": 1,
  "probe": "file_chooser",
  "stage": "response",
  "status": "user_cancelled",
  "cleanup": {
    "status": "completed",
    "failed_resources": []
  }
}
```

The Rust source of truth is [`src/model/probe.rs`](../src/model/probe.rs).
The model is exported from `src/model` but is not yet embedded in `Snapshot` or
`Report`. That separation prevents a contract change to `portaldoctor --json`
before an active command exists.

## Fields

| Field | Values | Meaning |
|---|---|---|
| `schema_version` | `1` | Version of this standalone result shape, independent of the application version and passive snapshot schema. |
| `probe` | `file_chooser`, `screenshot`, `screen_cast` | Portal family that produced the result. |
| `stage` | lifecycle stage | The last stage reached when the result became terminal. |
| `status` | operation status | Result of the portal operation, independent of cleanup. |
| `cleanup` | cleanup object | Whether owned request/session/media resources were cleaned up and verified. |

## Operation status

`status` is a closed, stable top-level classification for the portal operation:

| Value | Meaning |
|---|---|
| `success` | The portal operation completed successfully. It is a clean result only when cleanup is also complete or not required. |
| `user_cancelled` | The user explicitly cancelled the portal interaction. This is never transport failure. |
| `timed_out` | The bounded stage deadline expired. The adapter must attempt the required close operation and report its result under `cleanup`. |
| `unavailable` | The required portal service/backend was not reachable or available at runtime. |
| `unsupported` | The interface/version/runtime combination is outside the supported capability. |
| `malformed_response` | The portal response could not be decoded or did not match the expected protocol shape. Success must never be inferred. |
| `infrastructure_failure` | D-Bus transport, permission, portal rejection or another runtime infrastructure failure prevented completion. |

`unavailable` and `unsupported` are intentionally separate: a missing service
is not the same compatibility claim as a known interface/version limitation.
The future implementation may add sanitized diagnostic evidence beside this
classification, but v1 does not serialize arbitrary portal error strings,
paths, URIs or raw D-Bus payloads.

## Lifecycle stage

The `stage` value is shared across probe families:

| Value | Use |
|---|---|
| `prepare` | Validate explicit intent and runtime prerequisites before a request. |
| `request` | Create/submit the portal request. |
| `response` | Wait for and decode the request response. |
| `create_session` | ScreenCast session creation. |
| `select_sources` | ScreenCast source-selection request. |
| `start` | ScreenCast start request. |
| `streams_returned` | ScreenCast stream metadata was returned. |
| `open_pipe_wire_remote` | ScreenCast PipeWire remote handoff. |
| `cleanup` | The terminal result was produced while cleanup was still being resolved. |
| `complete` | The operation and cleanup reached a terminal completed state. |

FileChooser and Screenshot use `request`/`response`; ScreenCast uses the
explicit session stages so callers can locate the exact failing boundary.

## Cleanup contract

`cleanup` is a second result axis and must never be discarded when interpreting
`status`:

| `cleanup.status` | Meaning |
|---|---|
| `not_required` | No request/session/resource was acquired, so there was nothing to close. |
| `completed` | Every owned resource was closed and the cleanup outcome was verified. |
| `failed` | Cleanup was attempted but one or more resources could not be closed or verified. `failed_resources` identifies them. |
| `unverified` | The implementation cannot prove cleanup completed; `failed_resources` may identify known unverified resources, but may also be empty when the scope is unknown. Consumers must not treat the result as a clean success. |

`failed_resources` is an array of `request`, `session` and/or
`pipe_wire_remote` with no duplicate entries. It must be empty for
`not_required` and `completed`; `failed` must contain at least one resource.
It is empty for normal success and cancellation when no resource was acquired.
A result is a clean success only when `status == "success"` and cleanup is
`not_required` or `completed` with no failed resources. A portal success paired
with cleanup failure remains visibly `success` on the operation axis but is not
a clean success.

## Versioning and compatibility

- `PROBE_RESULT_SCHEMA_VERSION` is currently `1`.
- It is separate from the passive snapshot/report `schema_version` and from
  `portaldoctor_version`.
- The Rust constructors and `validate()` reject any schema version other than
  `1`; serde deserialization and serialization apply the same check.
- Cleanup constructors and serde also reject contradictory status/resource
  pairs and duplicate failed resources.
- Additive optional fields may be introduced without changing the version;
  changes to required fields, field meaning or enum semantics require a new
  result schema version.
- Consumers must reject unknown required enum values (the Rust model does this)
  or treat the document as an unsupported schema; they must never infer a pass
  from an unknown value. Adding a new enum value therefore requires a new
  result schema version.
- No active result is currently included in the passive `Snapshot`, `Report`,
  shareable report or README demo.
- Active probe shell exit-code mapping is intentionally deferred to the
  FileChooser implementation step; v0.2.1 exit codes remain unchanged.

## Privacy and test boundary

The v1 model contains only typed lifecycle facts and cleanup resources. It does
not contain selected filenames, screenshot data, PipeWire content, raw portal
errors or arbitrary environment values. Future probe-specific evidence must
pass the existing privacy review before it is added.

Serialization, round-trip compatibility, all operation statuses, independent
cleanup failure and passive report non-regression are covered by unit tests in
`src/model/probe.rs` and `src/report/mod.rs`. FileChooser lifecycle tests and
real-session validation remain the next Issue #3 step.
