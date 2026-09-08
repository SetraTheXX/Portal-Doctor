#![allow(dead_code)] // The contract is defined before the active probe commands.

use std::fmt;

use serde::de::Error as DeError;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
/// `ScreenCast` uses the explicit session stages so a future result can
/// identify the exact boundary that failed without inventing a second result
/// model.
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

/// Validation failure for a machine-readable probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResultError {
    InvalidSchemaVersion {
        expected: u32,
        actual: u32,
    },
    CleanupResourcesNotAllowed {
        status: CleanupStatus,
    },
    CleanupResourcesRequired,
    DuplicateCleanupResource {
        resource: CleanupResource,
    },
    InvalidStageForProbe {
        probe: ProbeKind,
        stage: ProbeStage,
    },
    InvalidCleanupResourceForProbe {
        probe: ProbeKind,
        resource: CleanupResource,
    },
}

impl fmt::Display for ProbeResultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchemaVersion { expected, actual } => write!(
                f,
                "unsupported probe result schema version {actual}; expected {expected}"
            ),
            Self::CleanupResourcesNotAllowed { status } => write!(
                f,
                "cleanup status {status:?} cannot contain failed resources"
            ),
            Self::CleanupResourcesRequired => {
                f.write_str("failed cleanup must identify at least one resource")
            }
            Self::DuplicateCleanupResource { resource } => {
                write!(f, "cleanup resource {resource:?} is listed more than once")
            }
            Self::InvalidStageForProbe { probe, stage } => {
                write!(f, "probe stage {stage:?} is not valid for probe {probe:?}")
            }
            Self::InvalidCleanupResourceForProbe { probe, resource } => write!(
                f,
                "cleanup resource {resource:?} is not valid for probe {probe:?}"
            ),
        }
    }
}

impl std::error::Error for ProbeResultError {}

/// Cleanup outcome kept independent from the portal operation outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupResult {
    status: CleanupStatus,
    failed_resources: Vec<CleanupResource>,
}

impl CleanupResult {
    /// Construct a result for a probe that acquired no closeable resource.
    #[must_use]
    pub fn not_required() -> Self {
        Self {
            status: CleanupStatus::NotRequired,
            failed_resources: Vec::new(),
        }
    }

    /// Construct a verified cleanup result.
    #[must_use]
    pub fn completed() -> Self {
        Self {
            status: CleanupStatus::Completed,
            failed_resources: Vec::new(),
        }
    }

    /// Construct a cleanup result with known failed resources.
    pub fn failed(failed_resources: Vec<CleanupResource>) -> Result<Self, ProbeResultError> {
        Self::try_new(CleanupStatus::Failed, failed_resources)
    }

    /// Construct a result where cleanup could not be proven.
    #[must_use]
    pub fn unverified() -> Self {
        Self {
            status: CleanupStatus::Unverified,
            failed_resources: Vec::new(),
        }
    }

    /// Construct a result where ownership of the listed resources is known
    /// but their cleanup cannot be proven.
    pub fn unverified_resources(resources: Vec<CleanupResource>) -> Result<Self, ProbeResultError> {
        Self::try_new(CleanupStatus::Unverified, resources)
    }

    /// Construct and validate an arbitrary cleanup state.
    pub fn try_new(
        status: CleanupStatus,
        failed_resources: Vec<CleanupResource>,
    ) -> Result<Self, ProbeResultError> {
        let result = Self {
            status,
            failed_resources,
        };
        result.validate()?;
        Ok(result)
    }

    /// Validate cleanup status/resource consistency.
    pub fn validate(&self) -> Result<(), ProbeResultError> {
        match self.status {
            CleanupStatus::NotRequired | CleanupStatus::Completed
                if !self.failed_resources.is_empty() =>
            {
                return Err(ProbeResultError::CleanupResourcesNotAllowed {
                    status: self.status,
                });
            }
            CleanupStatus::Failed if self.failed_resources.is_empty() => {
                return Err(ProbeResultError::CleanupResourcesRequired);
            }
            _ => {}
        }

        for (index, resource) in self.failed_resources.iter().enumerate() {
            if self.failed_resources[..index].contains(resource) {
                return Err(ProbeResultError::DuplicateCleanupResource {
                    resource: *resource,
                });
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn status(&self) -> CleanupStatus {
        self.status
    }

    #[must_use]
    pub fn failed_resources(&self) -> &[CleanupResource] {
        &self.failed_resources
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CleanupResultWire {
    status: CleanupStatus,
    failed_resources: Vec<CleanupResource>,
}

impl Serialize for CleanupResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate()
            .map_err(|error| S::Error::custom(error.to_string()))?;
        CleanupResultWire {
            status: self.status,
            failed_resources: self.failed_resources.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CleanupResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CleanupResultWire::deserialize(deserializer)?;
        Self::try_new(wire.status, wire.failed_resources)
            .map_err(|error| D::Error::custom(error.to_string()))
    }
}

impl ProbeStage {
    const fn is_valid_for(self, probe: ProbeKind) -> bool {
        match probe {
            ProbeKind::FileChooser | ProbeKind::Screenshot => matches!(
                self,
                Self::Prepare | Self::Request | Self::Response | Self::Cleanup | Self::Complete
            ),
            ProbeKind::ScreenCast => matches!(
                self,
                Self::Prepare
                    | Self::CreateSession
                    | Self::SelectSources
                    | Self::Start
                    | Self::StreamsReturned
                    | Self::OpenPipeWireRemote
                    | Self::Cleanup
                    | Self::Complete
            ),
        }
    }
}

impl CleanupResource {
    const fn is_valid_for(self, probe: ProbeKind) -> bool {
        match probe {
            ProbeKind::FileChooser | ProbeKind::Screenshot => matches!(self, Self::Request),
            ProbeKind::ScreenCast => true,
        }
    }
}

/// Stable machine-readable result shared by `FileChooser`, `Screenshot` and
/// `ScreenCast` probes.
///
/// This model is intentionally standalone for v0.3.0 planning. It is not yet
/// embedded in the passive `Snapshot` or `Report`, because doing so would
/// change the published v0.2.1 JSON contract before an active command exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    schema_version: u32,
    probe: ProbeKind,
    stage: ProbeStage,
    status: ProbeStatus,
    cleanup: CleanupResult,
}

impl ProbeResult {
    /// Construct a validated v1 result.
    pub fn new(
        probe: ProbeKind,
        stage: ProbeStage,
        status: ProbeStatus,
        cleanup: CleanupResult,
    ) -> Result<Self, ProbeResultError> {
        Self::from_parts(PROBE_RESULT_SCHEMA_VERSION, probe, stage, status, cleanup)
    }

    fn from_parts(
        schema_version: u32,
        probe: ProbeKind,
        stage: ProbeStage,
        status: ProbeStatus,
        cleanup: CleanupResult,
    ) -> Result<Self, ProbeResultError> {
        let result = Self {
            schema_version,
            probe,
            stage,
            status,
            cleanup,
        };
        result.validate()?;
        Ok(result)
    }

    /// Validate schema version and all nested invariants.
    pub fn validate(&self) -> Result<(), ProbeResultError> {
        if self.schema_version != PROBE_RESULT_SCHEMA_VERSION {
            return Err(ProbeResultError::InvalidSchemaVersion {
                expected: PROBE_RESULT_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if !self.stage.is_valid_for(self.probe) {
            return Err(ProbeResultError::InvalidStageForProbe {
                probe: self.probe,
                stage: self.stage,
            });
        }
        self.cleanup.validate()?;
        for resource in self.cleanup.failed_resources() {
            if !resource.is_valid_for(self.probe) {
                return Err(ProbeResultError::InvalidCleanupResourceForProbe {
                    probe: self.probe,
                    resource: *resource,
                });
            }
        }
        Ok(())
    }

    /// A result is a clean success only when the portal operation succeeded
    /// and cleanup was either unnecessary or completed without failures.
    #[must_use]
    pub fn is_clean_success(&self) -> bool {
        self.validate().is_ok()
            && matches!(self.status, ProbeStatus::Success)
            && matches!(
                self.cleanup.status,
                CleanupStatus::NotRequired | CleanupStatus::Completed
            )
            && self.cleanup.failed_resources.is_empty()
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn probe(&self) -> ProbeKind {
        self.probe
    }

    #[must_use]
    pub const fn stage(&self) -> ProbeStage {
        self.stage
    }

    #[must_use]
    pub const fn status(&self) -> ProbeStatus {
        self.status
    }

    #[must_use]
    pub const fn cleanup(&self) -> &CleanupResult {
        &self.cleanup
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ProbeResultWire {
    schema_version: u32,
    probe: ProbeKind,
    stage: ProbeStage,
    status: ProbeStatus,
    cleanup: CleanupResult,
}

impl Serialize for ProbeResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate()
            .map_err(|error| S::Error::custom(error.to_string()))?;
        ProbeResultWire {
            schema_version: self.schema_version,
            probe: self.probe,
            stage: self.stage,
            status: self.status,
            cleanup: self.cleanup.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProbeResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ProbeResultWire::deserialize(deserializer)?;
        Self::from_parts(
            wire.schema_version,
            wire.probe,
            wire.stage,
            wire.status,
            wire.cleanup,
        )
        .map_err(|error| D::Error::custom(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CleanupResource, CleanupResult, CleanupStatus, PROBE_RESULT_SCHEMA_VERSION, ProbeKind,
        ProbeResult, ProbeResultError, ProbeStage, ProbeStatus,
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
        let result = ProbeResult::new(
            ProbeKind::FileChooser,
            ProbeStage::Response,
            ProbeStatus::UserCancelled,
            CleanupResult::completed(),
        )
        .unwrap();

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
        assert_eq!(decoded.schema_version(), 1);
        assert_eq!(decoded.probe(), ProbeKind::FileChooser);
        assert_eq!(decoded.stage(), ProbeStage::Response);
        assert_eq!(decoded.status(), ProbeStatus::UserCancelled);
    }

    #[test]
    fn probe_stage_and_cleanup_resource_matrix_accepts_valid_combinations() {
        let valid_stages = [
            (ProbeKind::FileChooser, ProbeStage::Prepare),
            (ProbeKind::FileChooser, ProbeStage::Request),
            (ProbeKind::FileChooser, ProbeStage::Response),
            (ProbeKind::FileChooser, ProbeStage::Cleanup),
            (ProbeKind::FileChooser, ProbeStage::Complete),
            (ProbeKind::Screenshot, ProbeStage::Prepare),
            (ProbeKind::Screenshot, ProbeStage::Request),
            (ProbeKind::Screenshot, ProbeStage::Response),
            (ProbeKind::Screenshot, ProbeStage::Cleanup),
            (ProbeKind::Screenshot, ProbeStage::Complete),
            (ProbeKind::ScreenCast, ProbeStage::Prepare),
            (ProbeKind::ScreenCast, ProbeStage::CreateSession),
            (ProbeKind::ScreenCast, ProbeStage::SelectSources),
            (ProbeKind::ScreenCast, ProbeStage::Start),
            (ProbeKind::ScreenCast, ProbeStage::StreamsReturned),
            (ProbeKind::ScreenCast, ProbeStage::OpenPipeWireRemote),
            (ProbeKind::ScreenCast, ProbeStage::Cleanup),
            (ProbeKind::ScreenCast, ProbeStage::Complete),
        ];

        for (probe, stage) in valid_stages {
            let result = ProbeResult::new(
                probe,
                stage,
                ProbeStatus::Success,
                CleanupResult::completed(),
            )
            .unwrap();
            let encoded = serde_json::to_value(&result).unwrap();
            assert_eq!(
                serde_json::from_value::<ProbeResult>(encoded).unwrap(),
                result
            );
        }

        let valid_resources = [
            (
                ProbeKind::FileChooser,
                ProbeStage::Response,
                vec![CleanupResource::Request],
            ),
            (
                ProbeKind::Screenshot,
                ProbeStage::Response,
                vec![CleanupResource::Request],
            ),
            (
                ProbeKind::ScreenCast,
                ProbeStage::CreateSession,
                vec![CleanupResource::Request],
            ),
            (
                ProbeKind::ScreenCast,
                ProbeStage::CreateSession,
                vec![CleanupResource::Session],
            ),
            (
                ProbeKind::ScreenCast,
                ProbeStage::OpenPipeWireRemote,
                vec![CleanupResource::PipeWireRemote],
            ),
            (
                ProbeKind::ScreenCast,
                ProbeStage::Cleanup,
                vec![
                    CleanupResource::Request,
                    CleanupResource::Session,
                    CleanupResource::PipeWireRemote,
                ],
            ),
        ];

        for (probe, stage, resources) in valid_resources {
            let cleanup = CleanupResult::failed(resources).unwrap();
            let result = ProbeResult::new(probe, stage, ProbeStatus::Success, cleanup).unwrap();
            let encoded = serde_json::to_value(&result).unwrap();
            assert_eq!(
                serde_json::from_value::<ProbeResult>(encoded).unwrap(),
                result
            );
        }
    }

    #[test]
    fn probe_stage_and_cleanup_resource_matrix_rejects_invalid_constructors() {
        let invalid_stages = [
            (ProbeKind::FileChooser, ProbeStage::CreateSession),
            (ProbeKind::Screenshot, ProbeStage::OpenPipeWireRemote),
            (ProbeKind::ScreenCast, ProbeStage::Request),
            (ProbeKind::ScreenCast, ProbeStage::Response),
        ];

        for (probe, stage) in invalid_stages {
            assert!(matches!(
                ProbeResult::new(
                    probe,
                    stage,
                    ProbeStatus::InfrastructureFailure,
                    CleanupResult::not_required(),
                ),
                Err(ProbeResultError::InvalidStageForProbe { .. })
            ));
        }

        let invalid_resources = [
            (ProbeKind::FileChooser, CleanupResource::Session),
            (ProbeKind::Screenshot, CleanupResource::PipeWireRemote),
        ];

        for (probe, resource) in invalid_resources {
            let cleanup = CleanupResult::failed(vec![resource]).unwrap();
            assert!(matches!(
                ProbeResult::new(
                    probe,
                    ProbeStage::Response,
                    ProbeStatus::InfrastructureFailure,
                    cleanup,
                ),
                Err(ProbeResultError::InvalidCleanupResourceForProbe { .. })
            ));
        }
    }

    #[test]
    fn invalid_probe_stage_and_resource_documents_are_rejected_during_deserialization() {
        let invalid_documents = [
            json!({
                "schema_version": 1,
                "probe": "file_chooser",
                "stage": "create_session",
                "status": "success",
                "cleanup": {
                    "status": "completed",
                    "failed_resources": []
                }
            }),
            json!({
                "schema_version": 1,
                "probe": "screenshot",
                "stage": "open_pipe_wire_remote",
                "status": "success",
                "cleanup": {
                    "status": "completed",
                    "failed_resources": []
                }
            }),
            json!({
                "schema_version": 1,
                "probe": "screen_cast",
                "stage": "response",
                "status": "success",
                "cleanup": {
                    "status": "completed",
                    "failed_resources": []
                }
            }),
            json!({
                "schema_version": 1,
                "probe": "file_chooser",
                "stage": "response",
                "status": "infrastructure_failure",
                "cleanup": {
                    "status": "failed",
                    "failed_resources": ["session"]
                }
            }),
            json!({
                "schema_version": 1,
                "probe": "screenshot",
                "stage": "response",
                "status": "infrastructure_failure",
                "cleanup": {
                    "status": "failed",
                    "failed_resources": ["pipe_wire_remote"]
                }
            }),
        ];

        for document in invalid_documents {
            assert!(serde_json::from_value::<ProbeResult>(document).is_err());
        }
    }

    #[test]
    fn cleanup_failure_is_independent_from_operation_status() {
        let result = ProbeResult::new(
            ProbeKind::ScreenCast,
            ProbeStage::Cleanup,
            ProbeStatus::Success,
            CleanupResult::failed(vec![CleanupResource::Session]).unwrap(),
        )
        .unwrap();

        assert_eq!(result.status(), ProbeStatus::Success);
        assert_eq!(result.cleanup().status(), CleanupStatus::Failed);
        assert_eq!(
            result.cleanup().failed_resources(),
            &[CleanupResource::Session]
        );
        assert!(!result.is_clean_success());
    }

    #[test]
    fn unverified_cleanup_is_not_a_clean_success() {
        let result = ProbeResult::new(
            ProbeKind::Screenshot,
            ProbeStage::Response,
            ProbeStatus::Success,
            CleanupResult::unverified(),
        )
        .unwrap();

        assert!(!result.is_clean_success());
    }

    #[test]
    fn cleanup_status_and_resource_combinations_are_validated() {
        assert!(
            CleanupResult::try_new(CleanupStatus::NotRequired, vec![CleanupResource::Request])
                .is_err()
        );
        assert!(
            CleanupResult::try_new(CleanupStatus::Completed, vec![CleanupResource::Request])
                .is_err()
        );
        assert!(CleanupResult::try_new(CleanupStatus::Failed, Vec::new()).is_err());
        assert!(
            CleanupResult::try_new(
                CleanupStatus::Failed,
                vec![CleanupResource::Request, CleanupResource::Request]
            )
            .is_err()
        );
        assert!(
            CleanupResult::try_new(CleanupStatus::Unverified, vec![CleanupResource::Request])
                .is_ok()
        );
        assert_eq!(
            CleanupResult::unverified_resources(vec![CleanupResource::Session])
                .unwrap()
                .failed_resources(),
            &[CleanupResource::Session]
        );
    }

    #[test]
    fn invalid_cleanup_documents_are_rejected_during_deserialization() {
        let invalid_documents = [
            json!({
                "status": "not_required",
                "failed_resources": ["request"]
            }),
            json!({
                "status": "completed",
                "failed_resources": ["request"]
            }),
            json!({
                "status": "failed",
                "failed_resources": []
            }),
        ];

        for document in invalid_documents {
            assert!(serde_json::from_value::<CleanupResult>(document).is_err());
        }
    }

    #[test]
    fn schema_version_mismatch_is_rejected_during_deserialization() {
        let value = json!({
            "schema_version": 2,
            "probe": "file_chooser",
            "stage": "response",
            "status": "success",
            "cleanup": {
                "status": "completed",
                "failed_resources": []
            }
        });

        assert!(serde_json::from_value::<ProbeResult>(value).is_err());
    }

    #[test]
    fn invalid_internal_values_cannot_be_serialized_or_reported_clean() {
        let invalid_cleanup = CleanupResult {
            status: CleanupStatus::Completed,
            failed_resources: vec![CleanupResource::Request],
        };
        assert!(serde_json::to_value(&invalid_cleanup).is_err());

        let invalid_result = ProbeResult {
            schema_version: 2,
            probe: ProbeKind::FileChooser,
            stage: ProbeStage::Response,
            status: ProbeStatus::Success,
            cleanup: CleanupResult::completed(),
        };
        assert!(serde_json::to_value(&invalid_result).is_err());
        assert!(!invalid_result.is_clean_success());

        let invalid_stage = ProbeResult {
            schema_version: PROBE_RESULT_SCHEMA_VERSION,
            probe: ProbeKind::FileChooser,
            stage: ProbeStage::CreateSession,
            status: ProbeStatus::Success,
            cleanup: CleanupResult::completed(),
        };
        assert!(serde_json::to_value(&invalid_stage).is_err());
        assert!(!invalid_stage.is_clean_success());

        let invalid_resource = ProbeResult {
            schema_version: PROBE_RESULT_SCHEMA_VERSION,
            probe: ProbeKind::Screenshot,
            stage: ProbeStage::Response,
            status: ProbeStatus::Success,
            cleanup: CleanupResult {
                status: CleanupStatus::Failed,
                failed_resources: vec![CleanupResource::PipeWireRemote],
            },
        };
        assert!(serde_json::to_value(&invalid_resource).is_err());
        assert!(!invalid_resource.is_clean_success());
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
