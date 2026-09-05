#![allow(dead_code)] // The contract is defined before the active probe commands.

use serde::{Deserialize, Serialize};

/// Version of the standalone active-probe result contract.
pub const PROBE_RESULT_SCHEMA_VERSION: u32 = 1;

/// Active portal family that produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeKind {
    FileChooser,
    Screenshot,
    ScreenCast,
}

/// Lifecycle stage at which a probe produced its terminal result.
///
/// `FileChooser` and `Screenshot` use the generic request/response stages.
/// `ScreenCast` uses the explicit session stages so a future result can identify
/// the exact boundary that failed without inventing a second result model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStage {
    Prepare,
    Request,
    Response,
    CreateSession,
    SelectSources,
    Start,
    StreamsReturned,
    OpenPipeWireRemote,
    Cleanup,
    Complete,
}

/// Terminal outcome of the portal operation itself.
///
/// Cleanup is deliberately represented separately by [`CleanupResult`]. A
/// portal operation can succeed while cleanup is unverified or failed, and a
/// consumer must not collapse those two facts into one status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    Success,
    UserCancelled,
    TimedOut,
    Unavailable,
    Unsupported,
    MalformedResponse,
    InfrastructureFailure,
}

/// State of request/session/resource cleanup after a probe attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupStatus {
    NotRequired,
    Completed,
    Failed,
    Unverified,
}

/// Resource whose cleanup could not be proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupResource {
    Request,
    Session,
    PipeWireRemote,
}

/// Cleanup outcome kept independent from the portal operation outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupResult {
    pub status: CleanupStatus,
    pub failed_resources: Vec<CleanupResource>,
}

/// Stable machine-readable result shared by `FileChooser`, `Screenshot` and
/// `ScreenCast` probes.
///
/// This model is intentionally standalone for v0.3.0 planning. It is not yet
/// embedded in the passive `Snapshot` or `Report`, because doing so would
/// change the published v0.2.1 JSON contract before an active command exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub schema_version: u32,
    pub probe: ProbeKind,
    pub stage: ProbeStage,
    pub status: ProbeStatus,
    pub cleanup: CleanupResult,
}

impl ProbeResult {
    /// A result is a clean success only when the portal operation succeeded
    /// and cleanup was either unnecessary or completed without failures.
    #[must_use]
    pub const fn is_clean_success(&self) -> bool {
        matches!(self.status, ProbeStatus::Success)
            && matches!(
                self.cleanup.status,
                CleanupStatus::NotRequired | CleanupStatus::Completed
            )
            && self.cleanup.failed_resources.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CleanupResource, CleanupResult, CleanupStatus, PROBE_RESULT_SCHEMA_VERSION, ProbeKind,
        ProbeResult, ProbeStage, ProbeStatus,
    };
    use serde_json::json;

    #[test]
    fn schema_version_is_independent_and_stable() {
        assert_eq!(PROBE_RESULT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn statuses_use_stable_machine_names() {
        let cases = [
            (ProbeStatus::Success, "success"),
            (ProbeStatus::UserCancelled, "user_cancelled"),
            (ProbeStatus::TimedOut, "timed_out"),
            (ProbeStatus::Unavailable, "unavailable"),
            (ProbeStatus::Unsupported, "unsupported"),
            (ProbeStatus::MalformedResponse, "malformed_response"),
            (ProbeStatus::InfrastructureFailure, "infrastructure_failure"),
        ];

        for (status, expected) in cases {
            assert_eq!(serde_json::to_value(status).unwrap(), json!(expected));
        }
    }

    #[test]
    fn probe_result_has_a_versioned_standalone_shape() {
        let result = ProbeResult {
            schema_version: PROBE_RESULT_SCHEMA_VERSION,
            probe: ProbeKind::FileChooser,
            stage: ProbeStage::Response,
            status: ProbeStatus::UserCancelled,
            cleanup: CleanupResult {
                status: CleanupStatus::Completed,
                failed_resources: Vec::new(),
            },
        };

        let value = serde_json::to_value(&result).unwrap();
        assert_eq!(
            value,
            json!({
                "schema_version": 1,
                "probe": "file_chooser",
                "stage": "response",
                "status": "user_cancelled",
                "cleanup": {
                    "status": "completed",
                    "failed_resources": []
                }
            })
        );
        let decoded: ProbeResult = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, result);
    }

    #[test]
    fn cleanup_failure_is_independent_from_operation_status() {
        let result = ProbeResult {
            schema_version: PROBE_RESULT_SCHEMA_VERSION,
            probe: ProbeKind::ScreenCast,
            stage: ProbeStage::Cleanup,
            status: ProbeStatus::Success,
            cleanup: CleanupResult {
                status: CleanupStatus::Failed,
                failed_resources: vec![CleanupResource::Session],
            },
        };

        assert_eq!(result.status, ProbeStatus::Success);
        assert_eq!(result.cleanup.status, CleanupStatus::Failed);
        assert!(!result.is_clean_success());
    }

    #[test]
    fn unverified_cleanup_is_not_a_clean_success() {
        let result = ProbeResult {
            schema_version: PROBE_RESULT_SCHEMA_VERSION,
            probe: ProbeKind::Screenshot,
            stage: ProbeStage::Response,
            status: ProbeStatus::Success,
            cleanup: CleanupResult {
                status: CleanupStatus::Unverified,
                failed_resources: Vec::new(),
            },
        };

        assert!(!result.is_clean_success());
    }

    #[test]
    fn unknown_status_is_rejected_instead_of_inferred_as_success() {
        let value = json!({
            "schema_version": 1,
            "probe": "file_chooser",
            "stage": "response",
            "status": "future_status",
            "cleanup": {
                "status": "completed",
                "failed_resources": []
            }
        });

        assert!(serde_json::from_value::<ProbeResult>(value).is_err());
    }
}
