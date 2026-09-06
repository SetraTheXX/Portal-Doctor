# Active ProbeResult Schema v1

**Status:** v1 implemented for the Phase 8 FileChooser and Screenshot slices on development `main`
**Scope:** Standalone active-probe result contract; passive v0.2.1 output remains unchanged

This document defines the machine-readable result that explicit FileChooser,
Screenshot and ScreenCast probes emit. The development branch currently
implements `portaldoctor probe filechooser` and
`portaldoctor probe screenshot`; ScreenCast remains a future slice. It does
not change the published v0.2.1 passive commands or their existing
`--json` document.

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
The model is exported from `src/model` but is not embedded in `Snapshot` or
`Report`. That separation prevents an active-probe contract from changing the
published passive `portaldoctor --json` document.

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

### Probe/stage/resource compatibility

The fields are validated together; a stage or cleanup resource is not valid
merely because it is a known enum value. The v1 compatibility matrix is:

| Probe | Allowed stages | Allowed `failed_resources` values |
|---|---|---|
| `file_chooser` | `prepare`, `request`, `response`, `cleanup`, `complete` | `request` |
| `screenshot` | `prepare`, `request`, `response`, `cleanup`, `complete` | `request` |
| `screen_cast` | `prepare`, `create_session`, `select_sources`, `start`, `streams_returned`, `open_pipe_wire_remote`, `cleanup`, `complete` | `request`, `session`, `pipe_wire_remote` |

`cleanup` and `complete` are shared terminal boundaries, while the
ScreenCast-only session and PipeWire stages cannot be attached to FileChooser
or Screenshot results. Conversely, the generic `request` and `response`
stages are not used for ScreenCast. A cleanup resource listed for a probe must
also be present in that probe's row; otherwise the result is invalid.

The constructor, `validate()`, serialization and deserialization enforce this
matrix. This prevents a producer from emitting, or a consumer from accepting,
an internally well-typed but semantically impossible result.

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

## FileChooser command boundary

The first active command is deliberately separate from passive collection:

```sh
portaldoctor probe filechooser
portaldoctor probe filechooser --json
```

It warns on `stderr` before a desktop dialog may appear. The request is sent
to the portal frontend only after `Request::Response` subscription is active.
Each request carries a unique `handle_token`, so the expected Request object
path can be derived from the caller's D-Bus sender before `OpenFile`; the
returned path is still checked against that sender to support portals that
generate a different token. The returned or predicted path is retained only
for the bounded `Request.Close` cleanup call.

If cancellation or the request-stage deadline wins before `OpenFile` replies,
the method future remains alive for a bounded recovery window. A late valid
handle is closed immediately. If no reply arrives, `Close` is attempted on the
predicted path; an unknown-object result is `unverified`, because an absent
method reply cannot prove whether the portal created the request after the
cleanup race. This is a non-clean, machine-readable result rather than an
implicit fallback. A known returned handle that is already gone is treated as
verified completion. A response is classified by its status and protocol
shape, but the `uris` values are not logged, serialized or used for file I/O.
No selected file is read, copied, modified or persisted.

The request token is internal lifecycle metadata and is never included in the
result document, terminal rendering or persistent state. It is generated from
mandatory operating-system entropy; if entropy is unavailable, the probe
returns `infrastructure_failure` before creating a request instead of using a
predictable fallback. The controlled lifecycle harness is a permanent CI gate:
it checks both result shape and the expected `Request.Close` observation for
each request-created and no-request scenario.

The active command uses the following shell mapping without changing passive
exit codes: `0` means `success` plus verified `completed`/`not_required`
cleanup; `1` means cancellation, timeout, unavailable/unsupported capability,
malformed response, infrastructure failure or cleanup failure. JSON is emitted
even for a result mapped to `1`; process-level runtime/output failures retain
the existing generic error path.

## Screenshot command boundary

The development branch implements the explicit Screenshot command. Its
capability and privacy boundary is documented in
[`PORTALDOCTOR_SCREENSHOT_DECISION.md`](PORTALDOCTOR_SCREENSHOT_DECISION.md).
Screenshot may produce a portal-managed image and a sensitive `uri`; neither
is a `ProbeResult` field. `CleanupResource::Request` describes only the
Request object lifecycle and never promises deletion of the image artifact.
The command is unreleased and does not alter the passive v0.2.1 report.

## Versioning and compatibility

- `PROBE_RESULT_SCHEMA_VERSION` is currently `1`.
- It is separate from the passive snapshot/report `schema_version` and from
  `portaldoctor_version`.
- The Rust constructors and `validate()` reject any schema version other than
  `1`; serde deserialization and serialization apply the same check.
- Cleanup constructors and serde also reject contradictory status/resource
  pairs and duplicate failed resources.
- Probe/stage/resource combinations outside the compatibility matrix above are
  rejected by constructors and both serde directions.
- Additive optional fields may be introduced without changing the version;
  changes to required fields, field meaning or enum semantics require a new
  result schema version.
- Consumers must reject unknown required enum values (the Rust model does this)
  or treat the document as an unsupported schema; they must never infer a pass
  from an unknown value. Adding a new enum value therefore requires a new
  result schema version.
- No active result is included in the passive `Snapshot`, `Report`, shareable
  report or README demo.
- The standalone active command is unreleased until the Phase 8 release gate;
  the published v0.2.1 exit codes and JSON document remain unchanged.

## Privacy and test boundary

The v1 model contains only typed lifecycle facts and cleanup resources. It does
not contain selected filenames, screenshot data, PipeWire content, raw portal
errors or arbitrary environment values. Future probe-specific evidence must
pass the existing privacy review before it is added.

Serialization, round-trip compatibility, all operation statuses, independent
cleanup failure, FileChooser and Screenshot protocol fixtures, privacy
redaction and passive report non-regression are covered by unit tests in
`src/model/probe.rs`, `src/probes/filechooser.rs`,
`src/probes/screenshot.rs` and `src/report/mod.rs`. The controlled
fake-portal matrices are permanent quality gates. A v3-capable real-session
success/cancellation validation is still required before the v0.3.0 release
decision.
