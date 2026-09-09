use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::collectors::environment::COMPARISON_KEYS;
use crate::model::environment::{EnvironmentRelation, SessionInfo};
use crate::model::finding::Finding;
use crate::model::snapshot::{SNAPSHOT_SCHEMA_VERSION, Snapshot};
use crate::model::status::CollectorState;

/// Version of the standalone remediation-preview contract.
pub const REMEDIATION_PREVIEW_SCHEMA_VERSION: u32 = 1;
/// Stable remediation identifier for the first bounded preview.
pub const ENV004_REMEDIATION_ID: &str = "ENV004.activation_environment_import";

/// A deterministic decision for one requested remediation target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemediationPreview {
    /// Version of this standalone preview document, separate from the passive
    /// diagnostic snapshot schema.
    pub schema_version: u32,
    /// Finding requested by the caller.
    pub finding_id: String,
    /// Whether the current evidence supports a proposal.
    pub applicability: RemediationApplicability,
    /// Absent whenever the target is not applicable.
    pub proposal: Option<RemediationProposal>,
}

/// Machine-readable applicability result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RemediationApplicability {
    Applicable,
    NotApplicable { reason: NotApplicableReason },
}

/// Fail-closed reasons for omitting a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotApplicableReason {
    FindingNotPresent,
    SnapshotSchemaMismatch,
    SnapshotTimestampMissing,
    SnapshotUnavailable,
    ActivationComparisonUnavailable,
    SnapshotInconsistent,
    MissingProcessValue,
    NoActionableMismatch,
}

impl NotApplicableReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::FindingNotPresent => "finding not present",
            Self::SnapshotSchemaMismatch => "snapshot schema is unsupported",
            Self::SnapshotTimestampMissing => "snapshot timestamp is missing",
            Self::SnapshotUnavailable => "required snapshot sections are unavailable",
            Self::ActivationComparisonUnavailable => "activation comparison is unavailable",
            Self::SnapshotInconsistent => "snapshot comparison evidence is inconsistent",
            Self::MissingProcessValue => "a required process-side value is missing",
            Self::NoActionableMismatch => "no actionable environment mismatch is present",
        }
    }
}

/// The only action represented by this slice. It describes intent only; it
/// never invokes `systemctl import-environment` or writes a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationAction {
    ImportActivationEnvironment,
}

/// Explicit destination of the previewed values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationTarget {
    SystemdUserActivationEnvironment,
}

/// One allowlisted process-side value that would be imported by a future
/// apply implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentUpdate {
    pub key: String,
    pub value: String,
}

/// Typed, dry-run-only remediation proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemediationProposal {
    pub remediation_id: String,
    pub action: RemediationAction,
    pub target: RemediationTarget,
    pub dry_run: bool,
    pub environment_updates: Vec<EnvironmentUpdate>,
    pub files_modified: Vec<String>,
    pub service_restarts: Vec<String>,
    pub package_changes: Vec<String>,
    pub configuration_changes: Vec<String>,
    pub apply: ApplyStatus,
}

/// Apply boundary for this preview-only release slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyStatus {
    NotImplemented,
}

/// Build the only currently supported remediation preview from the evaluated
/// finding list and the same in-memory snapshot that produced it.
#[must_use]
pub fn preview_env004(snapshot: &Snapshot, findings: &[Finding]) -> RemediationPreview {
    let not_applicable = |reason| RemediationPreview {
        schema_version: REMEDIATION_PREVIEW_SCHEMA_VERSION,
        finding_id: "ENV004".to_owned(),
        applicability: RemediationApplicability::NotApplicable { reason },
        proposal: None,
    };

    if !findings.iter().any(|finding| finding.id == "ENV004") {
        return not_applicable(NotApplicableReason::FindingNotPresent);
    }
    if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return not_applicable(NotApplicableReason::SnapshotSchemaMismatch);
    }
    if snapshot.collected_at == 0 {
        return not_applicable(NotApplicableReason::SnapshotTimestampMissing);
    }
    if snapshot.session.status != CollectorState::Available
        || snapshot.environment.status != CollectorState::Available
    {
        return not_applicable(NotApplicableReason::SnapshotUnavailable);
    }
    let (Some(session), Some(environment)) = (
        snapshot.session.value.as_ref(),
        snapshot.environment.value.as_ref(),
    ) else {
        return not_applicable(NotApplicableReason::SnapshotUnavailable);
    };
    if !environment.activation_comparison.performed {
        return not_applicable(NotApplicableReason::ActivationComparisonUnavailable);
    }
    if !session_matches_process(session, &environment.process) {
        return not_applicable(NotApplicableReason::SnapshotInconsistent);
    }

    let mut seen_keys = BTreeSet::new();
    let mut updates = Vec::new();
    for entry in &environment.activation_comparison.entries {
        if !COMPARISON_KEYS.contains(&entry.key.as_str()) || !seen_keys.insert(entry.key.as_str()) {
            return not_applicable(NotApplicableReason::SnapshotInconsistent);
        }

        let process_value = environment.process.get(&entry.key).cloned();
        if process_value != entry.process_value
            || expected_relation(
                entry.process_value.as_deref(),
                entry.activation_value.as_deref(),
            ) != Some(entry.relation)
        {
            return not_applicable(NotApplicableReason::SnapshotInconsistent);
        }

        match entry.relation {
            EnvironmentRelation::Equal => {}
            EnvironmentRelation::Different | EnvironmentRelation::MissingActivation => {
                let Some(value) = entry
                    .process_value
                    .as_deref()
                    .filter(|value| !value.is_empty())
                else {
                    return not_applicable(NotApplicableReason::MissingProcessValue);
                };
                updates.push(EnvironmentUpdate {
                    key: entry.key.clone(),
                    value: value.to_owned(),
                });
            }
            EnvironmentRelation::MissingProcess | EnvironmentRelation::NotChecked => {
                return not_applicable(NotApplicableReason::MissingProcessValue);
            }
        }
    }

    if updates.is_empty() {
        return not_applicable(NotApplicableReason::NoActionableMismatch);
    }
    updates.sort_by(|left, right| left.key.cmp(&right.key));

    RemediationPreview {
        schema_version: REMEDIATION_PREVIEW_SCHEMA_VERSION,
        finding_id: "ENV004".to_owned(),
        applicability: RemediationApplicability::Applicable,
        proposal: Some(RemediationProposal {
            remediation_id: ENV004_REMEDIATION_ID.to_owned(),
            action: RemediationAction::ImportActivationEnvironment,
            target: RemediationTarget::SystemdUserActivationEnvironment,
            dry_run: true,
            environment_updates: updates,
            files_modified: Vec::new(),
            service_restarts: Vec::new(),
            package_changes: Vec::new(),
            configuration_changes: Vec::new(),
            apply: ApplyStatus::NotImplemented,
        }),
    }
}

/// Render the standalone preview without invoking any write-capable command.
#[must_use]
pub fn render_terminal(preview: &RemediationPreview) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "PortalDoctor remediation preview v{}",
        preview.schema_version
    )
    .expect("String writes cannot fail");
    writeln!(output, "Finding: {}", preview.finding_id).expect("String writes cannot fail");
    match &preview.applicability {
        RemediationApplicability::Applicable => {
            writeln!(output, "Applicability: applicable").expect("String writes cannot fail");
            let proposal = preview
                .proposal
                .as_ref()
                .expect("applicable preview must carry a proposal");
            writeln!(output, "Mode: dry-run").expect("String writes cannot fail");
            writeln!(
                output,
                "Action: import process-side values into the systemd user activation environment"
            )
            .expect("String writes cannot fail");
            writeln!(output, "Would import:").expect("String writes cannot fail");
            for update in &proposal.environment_updates {
                writeln!(output, "  {}={}", update.key, update.value)
                    .expect("String writes cannot fail");
            }
            writeln!(output, "Files modified: none").expect("String writes cannot fail");
            writeln!(output, "Service restarts: none").expect("String writes cannot fail");
            writeln!(output, "Package changes: none").expect("String writes cannot fail");
            writeln!(output, "Configuration changes: none").expect("String writes cannot fail");
            writeln!(output, "Apply: not implemented").expect("String writes cannot fail");
        }
        RemediationApplicability::NotApplicable { reason } => {
            writeln!(output, "Applicability: not applicable").expect("String writes cannot fail");
            writeln!(output, "Proposal: none ({})", reason.as_str())
                .expect("String writes cannot fail");
        }
    }
    output
}

fn expected_relation(
    process_value: Option<&str>,
    activation_value: Option<&str>,
) -> Option<EnvironmentRelation> {
    match (process_value, activation_value) {
        (Some(process), Some(activation)) if process == activation => {
            Some(EnvironmentRelation::Equal)
        }
        (Some(_), Some(_)) => Some(EnvironmentRelation::Different),
        (Some(_), None) => Some(EnvironmentRelation::MissingActivation),
        (None, Some(_)) => Some(EnvironmentRelation::MissingProcess),
        (None, None) => None,
    }
}

fn session_matches_process(
    session: &SessionInfo,
    process: &std::collections::BTreeMap<String, String>,
) -> bool {
    let matches = |key: &str, session_value: Option<&str>| {
        session_value == process.get(key).map(String::as_str)
    };
    matches("XDG_CURRENT_DESKTOP", session.current_desktop.as_deref())
        && matches("XDG_SESSION_DESKTOP", session.session_desktop.as_deref())
        && matches("XDG_SESSION_TYPE", session.session_type_raw.as_deref())
        && matches("WAYLAND_DISPLAY", session.wayland_display.as_deref())
        && matches("DISPLAY", session.display.as_deref())
}

#[cfg(test)]
mod tests {
    use super::{
        ApplyStatus, NotApplicableReason, RemediationAction, RemediationApplicability,
        RemediationTarget, preview_env004, render_terminal,
    };
    use crate::collectors::environment::{environment_info, session_info};
    use crate::model::section::Section;
    use crate::model::snapshot::Snapshot;
    use crate::rules::engine::evaluate;
    use std::collections::BTreeMap;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn snapshot(process: &[(&str, &str)], activation: &[(&str, &str)]) -> Snapshot {
        let process = map(process);
        let activation = map(activation);
        let mut snapshot = Snapshot::new(1);
        snapshot.session = Section::available(session_info(&process));
        snapshot.environment =
            Section::available(environment_info(process, None, Some(&activation)));
        snapshot
    }

    fn healthy_process() -> [(&'static str, &'static str); 4] {
        [
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ]
    }

    #[test]
    fn applicable_preview_is_typed_and_deterministic() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let findings = evaluate(&snapshot);
        let preview = preview_env004(&snapshot, &findings);

        assert_eq!(preview.finding_id, "ENV004");
        assert_eq!(preview.applicability, RemediationApplicability::Applicable);
        let proposal = preview.proposal.as_ref().expect("applicable proposal");
        assert_eq!(
            proposal.action,
            RemediationAction::ImportActivationEnvironment
        );
        assert_eq!(
            proposal.target,
            RemediationTarget::SystemdUserActivationEnvironment
        );
        assert!(proposal.dry_run);
        assert_eq!(proposal.apply, ApplyStatus::NotImplemented);
        assert_eq!(
            proposal
                .environment_updates
                .iter()
                .map(|update| (update.key.as_str(), update.value.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("WAYLAND_DISPLAY", "wayland-0"),
                ("XDG_CURRENT_DESKTOP", "GNOME")
            ]
        );
        assert!(proposal.files_modified.is_empty());
        assert!(proposal.service_restarts.is_empty());
        assert!(proposal.package_changes.is_empty());
        assert!(proposal.configuration_changes.is_empty());

        let text = render_terminal(&preview);
        assert!(text.contains("XDG_CURRENT_DESKTOP=GNOME"));
        assert!(text.contains("WAYLAND_DISPLAY=wayland-0"));
        assert!(text.contains("Files modified: none"));
        assert!(text.contains("Apply: not implemented"));
    }

    #[test]
    fn absent_env004_finding_produces_no_proposal() {
        let process = healthy_process();
        let snapshot = snapshot(&process, &process);
        let preview = preview_env004(&snapshot, &evaluate(&snapshot));

        assert_eq!(
            preview.applicability,
            RemediationApplicability::NotApplicable {
                reason: NotApplicableReason::FindingNotPresent
            }
        );
        assert!(preview.proposal.is_none());
    }

    #[test]
    fn missing_process_side_value_fails_closed() {
        let process = [
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let snapshot = snapshot(&process, &activation);
        let findings = evaluate(&snapshot);
        let preview = preview_env004(&snapshot, &findings);

        assert_eq!(
            preview.applicability,
            RemediationApplicability::NotApplicable {
                reason: NotApplicableReason::MissingProcessValue
            }
        );
        assert!(preview.proposal.is_none());
    }

    #[test]
    fn unsupported_or_stale_snapshot_fails_closed() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let mut snapshot_value = snapshot(&process, &activation);
        let findings = evaluate(&snapshot_value);
        snapshot_value.environment = Section::unavailable("test unavailable");
        assert_eq!(
            preview_env004(&snapshot_value, &findings).applicability,
            RemediationApplicability::NotApplicable {
                reason: NotApplicableReason::SnapshotUnavailable
            }
        );

        let mut schema_snapshot = snapshot(&process, &activation);
        schema_snapshot.schema_version += 1;
        assert_eq!(
            preview_env004(&schema_snapshot, &findings).applicability,
            RemediationApplicability::NotApplicable {
                reason: NotApplicableReason::SnapshotSchemaMismatch
            }
        );

        let mut timestamp_snapshot = snapshot(&process, &activation);
        timestamp_snapshot.collected_at = 0;
        assert_eq!(
            preview_env004(&timestamp_snapshot, &findings).applicability,
            RemediationApplicability::NotApplicable {
                reason: NotApplicableReason::SnapshotTimestampMissing
            }
        );
    }

    #[test]
    fn inconsistent_snapshot_evidence_produces_no_proposal() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let mut snapshot_value = snapshot(&process, &activation);
        let findings = evaluate(&snapshot_value);
        snapshot_value
            .environment
            .value
            .as_mut()
            .expect("environment value")
            .activation_comparison
            .entries[0]
            .relation = crate::model::environment::EnvironmentRelation::Equal;

        let preview = preview_env004(&snapshot_value, &findings);
        assert_eq!(
            preview.applicability,
            RemediationApplicability::NotApplicable {
                reason: NotApplicableReason::SnapshotInconsistent
            }
        );
        assert!(preview.proposal.is_none());
    }

    #[test]
    fn preview_serializes_without_an_apply_path() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&snapshot, &evaluate(&snapshot));
        let value = serde_json::to_value(preview).unwrap();
        assert_eq!(value["applicability"]["status"], "applicable");
        assert_eq!(value["proposal"]["dry_run"], true);
        assert_eq!(value["proposal"]["apply"], "not_implemented");
        assert_eq!(value["proposal"]["files_modified"], serde_json::json!([]));
        assert!(value["proposal"].get("command").is_none());
    }
}
