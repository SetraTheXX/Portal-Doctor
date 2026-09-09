use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::collectors::environment::COMPARISON_KEYS;
use crate::model::environment::{EnvironmentInfo, EnvironmentRelation, SessionInfo};
use crate::model::finding::Finding;
use crate::model::snapshot::{SNAPSHOT_SCHEMA_VERSION, Snapshot};
use crate::model::status::CollectorState;

/// Version of the standalone remediation-preview contract.
pub const REMEDIATION_PREVIEW_SCHEMA_VERSION: u32 = 1;
/// Version of the explicit approval binding contract.
pub const REMEDIATION_APPROVAL_SCHEMA_VERSION: u32 = 2;
/// Stable remediation identifier for the first bounded preview.
pub const ENV004_REMEDIATION_ID: &str = "ENV004.activation_environment_import";
const ENV004_FINDING_ID: &str = "ENV004";

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

/// Evidence binding for a preview. A future apply implementation must verify
/// this binding against a freshly collected snapshot before doing anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemediationBinding {
    pub snapshot_schema_version: u32,
    pub collected_at: u64,
    pub finding_id: String,
    pub evidence_digest: String,
    pub proposal_digest: String,
}

/// Typed, dry-run-only remediation proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemediationProposal {
    pub binding: RemediationBinding,
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

/// Typed outcome of verifying a stored ENV004 preview against fresh evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Env004PreviewVerification {
    Valid,
    Tampered { reason: VerificationTamperReason },
    StaleEvidence { reason: StaleEvidenceReason },
    NotApplicable { reason: NotApplicableReason },
    UnsupportedSchema { reason: VerificationSchemaReason },
}

/// Reasons a stored preview cannot be trusted as the same proposal document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationTamperReason {
    StoredProposalDigestMismatch,
    ProposalContractMismatch,
}

/// Reasons a stored proposal is internally valid but no longer matches fresh
/// actionable ENV004 evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleEvidenceReason {
    EvidenceDigestMismatch,
    EnvironmentUpdatesMismatch,
}

/// Schema boundaries checked before a fresh proposal can be compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationSchemaReason {
    PreviewSchemaMismatch,
    SnapshotSchemaMismatch,
}

/// Typed outcome of checking whether a future `ENV004` apply operation
/// produced the expected activation-environment effect. This contract is
/// pure and intentionally remains separate from the not-yet-implemented
/// apply path.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Env004EffectVerification {
    Converged,
    StillMismatched { reason: EffectMismatchReason },
    NoLongerApplicable { reason: EffectApplicabilityReason },
    Tampered { reason: VerificationTamperReason },
    Unavailable { reason: EffectUnavailableReason },
}

/// Why fresh evidence still represents an unresolved mismatch.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMismatchReason {
    RequestedValueNotConverged,
    ExpectedProcessValueChanged,
    AdditionalEnv004Finding,
}

/// Why a stored proposal no longer has an applicable effect to verify.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectApplicabilityReason {
    ExpectedProcessValueChanged,
}

/// Why the fresh snapshot cannot safely establish the post-apply effect.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectUnavailableReason {
    SnapshotSchemaMismatch,
    SnapshotUnavailable,
    ComparisonNotPerformed,
    ProcessValueMissing,
    ComparisonEntryMissing,
    InconsistentComparison,
    InconsistentFindings,
}

/// Explicit user decision carried by an approval record.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationApprovalState {
    Approved,
    NotApproved,
}

/// Approval bound to one exact, integrity-checked ENV004 proposal.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemediationApproval {
    pub approval_contract_version: u32,
    pub remediation_id: String,
    pub proposal_digest: String,
    pub evidence_digest: String,
    pub action: RemediationAction,
    pub target: RemediationTarget,
    pub user_approval: RemediationApprovalState,
    /// Digest over every bounded approval field except this digest itself.
    pub approval_digest: String,
}

/// Opaque capability admitted by the verified ENV004 approval boundary.
///
/// The fields stay private and the type is intentionally not serializable or
/// cloneable. A future apply implementation must consume a permit produced by
/// `create_env004_apply_permit` instead of accepting a raw approval document.
#[allow(dead_code)]
pub struct Env004ApplyPermit {
    proposal_digest: String,
    approval_digest: String,
    fresh_evidence_digest: String,
    environment_updates: Vec<EnvironmentUpdate>,
    prior_activation_values: Vec<PermitActivationState>,
}

impl Env004ApplyPermit {
    #[allow(dead_code)]
    fn binds_to(
        &self,
        proposal: &RemediationProposal,
        approval: &RemediationApproval,
        fresh_evidence_digest: &str,
        environment_updates: &[EnvironmentUpdate],
    ) -> bool {
        self.proposal_digest == proposal.binding.proposal_digest
            && self.approval_digest == approval.approval_digest
            && self.fresh_evidence_digest == fresh_evidence_digest
            && self.environment_updates == environment_updates
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PermitActivationState {
    key: String,
    value: Option<String>,
}

/// The desired value and the exact activation-side pre-state for one bounded
/// ENV004 key. `Absent` is explicit so rollback never has to infer whether an
/// empty string was a real value.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Env004PriorActivation {
    Present(String),
    Absent,
}

/// The inverse operation that a future apply implementation would perform.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Env004RollbackAction {
    Restore(String),
    Unset,
}

/// One deterministic, side-effect-free ENV004 execution step.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env004ExecutionStep {
    pub key: String,
    pub desired_process_value: String,
    pub prior_activation: Env004PriorActivation,
    pub rollback: Env004RollbackAction,
}

/// A deterministic transaction/rollback plan admitted only from an opaque
/// apply permit. It has no execution method and does not invoke any command.
#[allow(dead_code)]
pub struct Env004ExecutionPlan {
    proposal_digest: String,
    approval_digest: String,
    fresh_evidence_digest: String,
    steps: Vec<Env004ExecutionStep>,
}

/// Typed reasons why an approval or its bound proposal cannot be trusted.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalTamperReason {
    ApprovalDigest,
    ApprovalContractVersion,
    ProposalDigest,
    ProposalContract,
    ApprovalBinding,
}

/// Typed reasons why an otherwise valid approval cannot authorize a fresh run.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStaleEvidenceReason {
    FreshSchemaMismatch,
    FreshSnapshotUnavailable,
    FreshComparisonUnavailable,
    InconsistentFindings,
    EvidenceDigestMismatch,
    EnvironmentUpdatesMismatch,
}

/// Typed outcome of verifying explicit approval against a proposal and fresh
/// evidence. `Valid` is the only outcome that could authorize a future apply
/// path; this slice does not implement that path.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Env004ApprovalVerification {
    Valid,
    Tampered { reason: ApprovalTamperReason },
    StaleEvidence { reason: ApprovalStaleEvidenceReason },
    NotApproved,
    NotApplicable { reason: NotApplicableReason },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionableEvidence {
    key: String,
    process_value: String,
    activation_value: Option<String>,
    relation: EnvironmentRelation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectUpdateStatus {
    Converged,
    ActivationNotConverged,
    ExpectedProcessChanged,
}

/// Build the only currently supported remediation preview from the evaluated
/// finding list and the same in-memory snapshot that produced it.
#[must_use]
pub fn preview_env004(snapshot: &Snapshot, findings: &[Finding]) -> RemediationPreview {
    let not_applicable = |reason| RemediationPreview {
        schema_version: REMEDIATION_PREVIEW_SCHEMA_VERSION,
        finding_id: ENV004_FINDING_ID.to_owned(),
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

    let mut actionable_evidence = match collect_actionable_evidence(environment) {
        Ok(evidence) => evidence,
        Err(reason) => return not_applicable(reason),
    };

    actionable_evidence.sort_by(|left, right| left.key.cmp(&right.key));
    let updates = actionable_evidence
        .iter()
        .map(|evidence| EnvironmentUpdate {
            key: evidence.key.clone(),
            value: evidence.process_value.clone(),
        })
        .collect::<Vec<_>>();
    let evidence_digest = digest_actionable_evidence(&actionable_evidence);
    let mut proposal = RemediationProposal {
        binding: RemediationBinding {
            snapshot_schema_version: snapshot.schema_version,
            collected_at: snapshot.collected_at,
            finding_id: ENV004_FINDING_ID.to_owned(),
            evidence_digest,
            proposal_digest: String::new(),
        },
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
    };
    proposal.binding.proposal_digest = digest_proposal(&proposal);

    RemediationPreview {
        schema_version: REMEDIATION_PREVIEW_SCHEMA_VERSION,
        finding_id: ENV004_FINDING_ID.to_owned(),
        applicability: RemediationApplicability::Applicable,
        proposal: Some(proposal),
    }
}

/// Verify a stored preview without applying it or invoking any write-capable
/// command. The stored digest is checked before the contract and fresh
/// evidence are evaluated; collection timestamps are provenance only and are
/// intentionally not compared between the two previews.
#[must_use]
pub fn verify_env004_preview(
    stored_preview: &RemediationPreview,
    fresh_snapshot: &Snapshot,
    fresh_findings: &[Finding],
) -> Env004PreviewVerification {
    let Some(stored_proposal) = stored_preview.proposal.as_ref() else {
        return match stored_preview.applicability {
            RemediationApplicability::NotApplicable { ref reason } => {
                Env004PreviewVerification::NotApplicable { reason: *reason }
            }
            RemediationApplicability::Applicable => Env004PreviewVerification::Tampered {
                reason: VerificationTamperReason::ProposalContractMismatch,
            },
        };
    };

    if digest_proposal(stored_proposal) != stored_proposal.binding.proposal_digest {
        return Env004PreviewVerification::Tampered {
            reason: VerificationTamperReason::StoredProposalDigestMismatch,
        };
    }

    if stored_preview.schema_version != REMEDIATION_PREVIEW_SCHEMA_VERSION
        || stored_proposal.binding.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION
    {
        return Env004PreviewVerification::UnsupportedSchema {
            reason: if stored_preview.schema_version == REMEDIATION_PREVIEW_SCHEMA_VERSION {
                VerificationSchemaReason::SnapshotSchemaMismatch
            } else {
                VerificationSchemaReason::PreviewSchemaMismatch
            },
        };
    }

    if !stored_preview_contract_is_valid(stored_preview, stored_proposal) {
        return Env004PreviewVerification::Tampered {
            reason: VerificationTamperReason::ProposalContractMismatch,
        };
    }

    if fresh_snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Env004PreviewVerification::UnsupportedSchema {
            reason: VerificationSchemaReason::SnapshotSchemaMismatch,
        };
    }
    let fresh_preview = preview_env004(fresh_snapshot, fresh_findings);
    let RemediationApplicability::Applicable = fresh_preview.applicability else {
        let RemediationApplicability::NotApplicable { reason } = fresh_preview.applicability else {
            unreachable!("applicability was matched above");
        };
        return if reason == NotApplicableReason::SnapshotSchemaMismatch {
            Env004PreviewVerification::UnsupportedSchema {
                reason: VerificationSchemaReason::SnapshotSchemaMismatch,
            }
        } else {
            Env004PreviewVerification::NotApplicable { reason }
        };
    };
    let Some(fresh_proposal) = fresh_preview.proposal.as_ref() else {
        unreachable!("applicable preview must carry a proposal");
    };

    if stored_proposal.binding.evidence_digest != fresh_proposal.binding.evidence_digest {
        return Env004PreviewVerification::StaleEvidence {
            reason: StaleEvidenceReason::EvidenceDigestMismatch,
        };
    }
    if stored_proposal.environment_updates != fresh_proposal.environment_updates {
        return Env004PreviewVerification::StaleEvidence {
            reason: StaleEvidenceReason::EnvironmentUpdatesMismatch,
        };
    }

    Env004PreviewVerification::Valid
}

/// Verify the observable effect that a future `ENV004` apply operation was
/// expected to produce. This function never writes, invokes `systemctl`, or
/// compares unrelated snapshot fields. Proposal integrity and contract checks
/// happen before any fresh snapshot interpretation.
#[allow(dead_code)]
#[must_use]
pub fn verify_env004_effect(
    proposal: &RemediationProposal,
    fresh_snapshot: &Snapshot,
    fresh_findings: &[Finding],
) -> Env004EffectVerification {
    let tamper_reason = if digest_proposal(proposal) != proposal.binding.proposal_digest {
        Some(VerificationTamperReason::StoredProposalDigestMismatch)
    } else if !proposal_contract_is_valid(proposal) {
        Some(VerificationTamperReason::ProposalContractMismatch)
    } else {
        None
    };
    if let Some(reason) = tamper_reason {
        return Env004EffectVerification::Tampered { reason };
    }

    if fresh_snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Env004EffectVerification::Unavailable {
            reason: EffectUnavailableReason::SnapshotSchemaMismatch,
        };
    }
    if fresh_snapshot.environment.status != CollectorState::Available
        || fresh_snapshot.environment.value.is_none()
    {
        return Env004EffectVerification::Unavailable {
            reason: EffectUnavailableReason::SnapshotUnavailable,
        };
    }
    let environment = fresh_snapshot
        .environment
        .value
        .as_ref()
        .expect("available environment section has a value");
    if !environment.activation_comparison.performed {
        return Env004EffectVerification::Unavailable {
            reason: EffectUnavailableReason::ComparisonNotPerformed,
        };
    }

    let has_env004 = match fresh_env004_consistency(environment, fresh_findings) {
        Ok(has_mismatch) => has_mismatch,
        Err(reason) => return Env004EffectVerification::Unavailable { reason },
    };
    let mut all_converged = true;
    for update in &proposal.environment_updates {
        let update_status = match verify_effect_update(environment, update) {
            Ok(status) => status,
            Err(reason) => return Env004EffectVerification::Unavailable { reason },
        };
        if update_status == EffectUpdateStatus::ExpectedProcessChanged {
            return if has_env004 {
                Env004EffectVerification::StillMismatched {
                    reason: EffectMismatchReason::ExpectedProcessValueChanged,
                }
            } else {
                Env004EffectVerification::NoLongerApplicable {
                    reason: EffectApplicabilityReason::ExpectedProcessValueChanged,
                }
            };
        }
        if update_status == EffectUpdateStatus::ActivationNotConverged {
            all_converged = false;
        }
    }

    if has_env004 {
        Env004EffectVerification::StillMismatched {
            reason: if all_converged {
                EffectMismatchReason::AdditionalEnv004Finding
            } else {
                EffectMismatchReason::RequestedValueNotConverged
            },
        }
    } else {
        Env004EffectVerification::Converged
    }
}

/// Create an approval record only from an intact, contract-valid ENV004
/// proposal. The caller supplies the explicit user decision; this function
/// never performs an apply or any other side effect.
#[allow(dead_code)]
#[must_use]
pub fn create_env004_approval(
    proposal: &RemediationProposal,
    user_approval: RemediationApprovalState,
) -> Option<RemediationApproval> {
    if digest_proposal(proposal) != proposal.binding.proposal_digest
        || !proposal_contract_is_valid(proposal)
    {
        return None;
    }
    let mut approval = RemediationApproval {
        approval_contract_version: REMEDIATION_APPROVAL_SCHEMA_VERSION,
        remediation_id: proposal.remediation_id.clone(),
        proposal_digest: proposal.binding.proposal_digest.clone(),
        evidence_digest: proposal.binding.evidence_digest.clone(),
        action: proposal.action,
        target: proposal.target,
        user_approval,
        approval_digest: String::new(),
    };
    approval.approval_digest = digest_approval(&approval);
    Some(approval)
}

/// Verify an explicit approval against one exact proposal and fresh ENV004
/// evidence. `Valid` only means that a future apply boundary may consider the
/// approval authoritative; no write-capable operation exists here.
#[allow(dead_code)]
#[must_use]
pub fn verify_env004_approval(
    approval: &RemediationApproval,
    proposal: &RemediationProposal,
    fresh_snapshot: &Snapshot,
    fresh_findings: &[Finding],
) -> Env004ApprovalVerification {
    if digest_approval(approval) != approval.approval_digest {
        return Env004ApprovalVerification::Tampered {
            reason: ApprovalTamperReason::ApprovalDigest,
        };
    }
    if let Err(reason) = verify_approval_integrity(approval, proposal) {
        return Env004ApprovalVerification::Tampered { reason };
    }
    if approval.user_approval != RemediationApprovalState::Approved {
        return Env004ApprovalVerification::NotApproved;
    }

    verify_approval_fresh_evidence(proposal, fresh_snapshot, fresh_findings)
}

/// Admit a future ENV004 apply operation only after the approval and fresh
/// evidence have passed every existing verification boundary. The returned
/// capability is opaque, non-serializable and non-cloneable; this function
/// never invokes `systemctl`, writes a file or changes the environment.
#[allow(dead_code)]
#[must_use]
pub fn create_env004_apply_permit(
    proposal: &RemediationProposal,
    approval: &RemediationApproval,
    fresh_snapshot: &Snapshot,
    fresh_findings: &[Finding],
) -> Option<Env004ApplyPermit> {
    if verify_env004_approval(approval, proposal, fresh_snapshot, fresh_findings)
        != Env004ApprovalVerification::Valid
    {
        return None;
    }

    let fresh_preview = preview_env004(fresh_snapshot, fresh_findings);
    let fresh_proposal = fresh_preview.proposal?;
    if fresh_proposal.binding.evidence_digest != proposal.binding.evidence_digest
        || fresh_proposal.environment_updates != proposal.environment_updates
    {
        return None;
    }
    let environment = fresh_snapshot.environment.value.as_ref()?;
    let prior_activation_values =
        capture_permit_activation_state(environment, &fresh_proposal.environment_updates)?;

    Some(Env004ApplyPermit {
        proposal_digest: proposal.binding.proposal_digest.clone(),
        approval_digest: approval.approval_digest.clone(),
        fresh_evidence_digest: fresh_proposal.binding.evidence_digest,
        environment_updates: fresh_proposal.environment_updates,
        prior_activation_values,
    })
}

fn capture_permit_activation_state(
    environment: &EnvironmentInfo,
    updates: &[EnvironmentUpdate],
) -> Option<Vec<PermitActivationState>> {
    let mut seen_keys = BTreeSet::new();
    let mut states = Vec::with_capacity(updates.len());
    for update in updates {
        if !COMPARISON_KEYS.contains(&update.key.as_str()) || !seen_keys.insert(&update.key) {
            return None;
        }
        let mut matching_entries = environment
            .activation_comparison
            .entries
            .iter()
            .filter(|entry| entry.key == update.key);
        let entry = matching_entries.next()?;
        if matching_entries.next().is_some()
            || entry.process_value.as_deref() != Some(update.value.as_str())
            || expected_relation(
                entry.process_value.as_deref(),
                entry.activation_value.as_deref(),
            ) != Some(entry.relation)
        {
            return None;
        }
        if !matches!(
            entry.relation,
            EnvironmentRelation::Different | EnvironmentRelation::MissingActivation
        ) {
            return None;
        }
        states.push(PermitActivationState {
            key: update.key.clone(),
            value: entry.activation_value.clone(),
        });
    }
    states.sort_by(|left, right| left.key.cmp(&right.key));
    Some(states)
}

/// Consume an opaque permit into a deterministic transaction/rollback plan.
/// Raw approvals, proposals or snapshots are deliberately not accepted here.
#[allow(dead_code)]
#[must_use]
pub fn create_env004_execution_plan(permit: Env004ApplyPermit) -> Option<Env004ExecutionPlan> {
    let mut seen_update_keys = BTreeSet::new();
    for update in &permit.environment_updates {
        if !COMPARISON_KEYS.contains(&update.key.as_str())
            || update.value.is_empty()
            || !seen_update_keys.insert(&update.key)
        {
            return None;
        }
    }

    let mut prior_by_key = BTreeMap::new();
    for state in &permit.prior_activation_values {
        if !seen_update_keys.contains(&state.key)
            || prior_by_key
                .insert(state.key.as_str(), state.value.clone())
                .is_some()
        {
            return None;
        }
    }
    if prior_by_key.len() != seen_update_keys.len() || permit.environment_updates.is_empty() {
        return None;
    }

    let mut updates = permit.environment_updates.clone();
    updates.sort_by(|left, right| left.key.cmp(&right.key));
    let steps = updates
        .into_iter()
        .map(|update| {
            let prior_value = prior_by_key
                .get(update.key.as_str())
                .expect("validated permit pre-state has every update key");
            let (prior_activation, rollback) = match prior_value {
                Some(value) => (
                    Env004PriorActivation::Present(value.clone()),
                    Env004RollbackAction::Restore(value.clone()),
                ),
                None => (Env004PriorActivation::Absent, Env004RollbackAction::Unset),
            };
            Env004ExecutionStep {
                key: update.key,
                desired_process_value: update.value,
                prior_activation,
                rollback,
            }
        })
        .collect();

    Some(Env004ExecutionPlan {
        proposal_digest: permit.proposal_digest,
        approval_digest: permit.approval_digest,
        fresh_evidence_digest: permit.fresh_evidence_digest,
        steps,
    })
}

fn verify_approval_integrity(
    approval: &RemediationApproval,
    proposal: &RemediationProposal,
) -> Result<(), ApprovalTamperReason> {
    if digest_proposal(proposal) != proposal.binding.proposal_digest {
        return Err(ApprovalTamperReason::ProposalDigest);
    }
    if !proposal_contract_is_valid(proposal) {
        return Err(ApprovalTamperReason::ProposalContract);
    }
    if approval.approval_contract_version != REMEDIATION_APPROVAL_SCHEMA_VERSION {
        return Err(ApprovalTamperReason::ApprovalContractVersion);
    }
    if !approval_binds_to_proposal(approval, proposal) {
        return Err(ApprovalTamperReason::ApprovalBinding);
    }
    Ok(())
}

fn verify_approval_fresh_evidence(
    proposal: &RemediationProposal,
    fresh_snapshot: &Snapshot,
    fresh_findings: &[Finding],
) -> Env004ApprovalVerification {
    if fresh_snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Env004ApprovalVerification::StaleEvidence {
            reason: ApprovalStaleEvidenceReason::FreshSchemaMismatch,
        };
    }
    if fresh_snapshot.environment.status != CollectorState::Available
        || fresh_snapshot.environment.value.is_none()
    {
        return Env004ApprovalVerification::StaleEvidence {
            reason: ApprovalStaleEvidenceReason::FreshSnapshotUnavailable,
        };
    }
    let environment = fresh_snapshot
        .environment
        .value
        .as_ref()
        .expect("available environment section has a value");
    if !environment.activation_comparison.performed {
        return Env004ApprovalVerification::StaleEvidence {
            reason: ApprovalStaleEvidenceReason::FreshComparisonUnavailable,
        };
    }
    let has_env004 = match fresh_env004_consistency(environment, fresh_findings) {
        Ok(has_mismatch) => has_mismatch,
        Err(reason) => {
            return Env004ApprovalVerification::StaleEvidence {
                reason: approval_stale_reason(reason),
            };
        }
    };
    if !has_env004 {
        return Env004ApprovalVerification::NotApplicable {
            reason: NotApplicableReason::FindingNotPresent,
        };
    }

    let stored_preview = RemediationPreview {
        schema_version: REMEDIATION_PREVIEW_SCHEMA_VERSION,
        finding_id: ENV004_FINDING_ID.to_owned(),
        applicability: RemediationApplicability::Applicable,
        proposal: Some(proposal.clone()),
    };
    match verify_env004_preview(&stored_preview, fresh_snapshot, fresh_findings) {
        Env004PreviewVerification::Valid => Env004ApprovalVerification::Valid,
        Env004PreviewVerification::StaleEvidence { reason } => {
            Env004ApprovalVerification::StaleEvidence {
                reason: match reason {
                    StaleEvidenceReason::EvidenceDigestMismatch => {
                        ApprovalStaleEvidenceReason::EvidenceDigestMismatch
                    }
                    StaleEvidenceReason::EnvironmentUpdatesMismatch => {
                        ApprovalStaleEvidenceReason::EnvironmentUpdatesMismatch
                    }
                },
            }
        }
        Env004PreviewVerification::NotApplicable { reason } => {
            Env004ApprovalVerification::NotApplicable { reason }
        }
        Env004PreviewVerification::UnsupportedSchema { reason } => {
            Env004ApprovalVerification::StaleEvidence {
                reason: match reason {
                    VerificationSchemaReason::PreviewSchemaMismatch
                    | VerificationSchemaReason::SnapshotSchemaMismatch => {
                        ApprovalStaleEvidenceReason::FreshSchemaMismatch
                    }
                },
            }
        }
        Env004PreviewVerification::Tampered { reason } => Env004ApprovalVerification::Tampered {
            reason: match reason {
                VerificationTamperReason::StoredProposalDigestMismatch => {
                    ApprovalTamperReason::ProposalDigest
                }
                VerificationTamperReason::ProposalContractMismatch => {
                    ApprovalTamperReason::ProposalContract
                }
            },
        },
    }
}

fn approval_binds_to_proposal(
    approval: &RemediationApproval,
    proposal: &RemediationProposal,
) -> bool {
    approval.remediation_id == proposal.remediation_id
        && approval.proposal_digest == proposal.binding.proposal_digest
        && approval.evidence_digest == proposal.binding.evidence_digest
        && approval.action == proposal.action
        && approval.target == proposal.target
}

fn approval_stale_reason(reason: EffectUnavailableReason) -> ApprovalStaleEvidenceReason {
    match reason {
        EffectUnavailableReason::SnapshotSchemaMismatch => {
            ApprovalStaleEvidenceReason::FreshSchemaMismatch
        }
        EffectUnavailableReason::SnapshotUnavailable => {
            ApprovalStaleEvidenceReason::FreshSnapshotUnavailable
        }
        EffectUnavailableReason::ComparisonNotPerformed
        | EffectUnavailableReason::ProcessValueMissing
        | EffectUnavailableReason::ComparisonEntryMissing
        | EffectUnavailableReason::InconsistentComparison => {
            ApprovalStaleEvidenceReason::FreshComparisonUnavailable
        }
        EffectUnavailableReason::InconsistentFindings => {
            ApprovalStaleEvidenceReason::InconsistentFindings
        }
    }
}

fn fresh_env004_consistency(
    environment: &EnvironmentInfo,
    fresh_findings: &[Finding],
) -> Result<bool, EffectUnavailableReason> {
    let mut seen_keys = BTreeSet::new();
    for entry in &environment.activation_comparison.entries {
        if !COMPARISON_KEYS.contains(&entry.key.as_str()) || !seen_keys.insert(entry.key.as_str()) {
            return Err(EffectUnavailableReason::InconsistentComparison);
        }
        let process_value = environment.process.get(&entry.key).map(String::as_str);
        if entry.process_value.as_deref() != process_value
            || expected_relation(
                entry.process_value.as_deref(),
                entry.activation_value.as_deref(),
            ) != Some(entry.relation)
        {
            return Err(EffectUnavailableReason::InconsistentComparison);
        }
    }

    if COMPARISON_KEYS
        .iter()
        .any(|key| environment.process.contains_key(*key) && !seen_keys.contains(key))
    {
        return Err(EffectUnavailableReason::InconsistentComparison);
    }

    let comparison_has_env004 = environment
        .activation_comparison
        .entries
        .iter()
        .any(|entry| entry.relation != EnvironmentRelation::Equal);
    let env004_finding_count = fresh_findings
        .iter()
        .filter(|finding| finding.id == ENV004_FINDING_ID)
        .count();
    let findings_are_consistent = if comparison_has_env004 {
        env004_finding_count == 1
    } else {
        env004_finding_count == 0
    };
    if !findings_are_consistent {
        return Err(EffectUnavailableReason::InconsistentFindings);
    }
    Ok(comparison_has_env004)
}

fn verify_effect_update(
    environment: &EnvironmentInfo,
    update: &EnvironmentUpdate,
) -> Result<EffectUpdateStatus, EffectUnavailableReason> {
    let Some(process_value) = environment.process.get(&update.key) else {
        return Err(EffectUnavailableReason::ProcessValueMissing);
    };
    if process_value.is_empty() {
        return Err(EffectUnavailableReason::ProcessValueMissing);
    }

    let mut matching_entries = environment
        .activation_comparison
        .entries
        .iter()
        .filter(|entry| entry.key == update.key);
    let Some(entry) = matching_entries.next() else {
        return Err(EffectUnavailableReason::ComparisonEntryMissing);
    };
    if matching_entries.next().is_some() {
        return Err(EffectUnavailableReason::InconsistentComparison);
    }
    if entry.process_value.as_deref() != Some(process_value.as_str())
        || expected_relation(
            entry.process_value.as_deref(),
            entry.activation_value.as_deref(),
        ) != Some(entry.relation)
    {
        return Err(EffectUnavailableReason::InconsistentComparison);
    }

    if process_value != &update.value {
        return Ok(EffectUpdateStatus::ExpectedProcessChanged);
    }
    if entry.activation_value.as_deref() != Some(update.value.as_str()) {
        return Ok(EffectUpdateStatus::ActivationNotConverged);
    }
    Ok(EffectUpdateStatus::Converged)
}

fn stored_preview_contract_is_valid(
    stored_preview: &RemediationPreview,
    stored_proposal: &RemediationProposal,
) -> bool {
    stored_preview.finding_id == ENV004_FINDING_ID
        && stored_preview.applicability == RemediationApplicability::Applicable
        && proposal_contract_is_valid(stored_proposal)
}

fn proposal_contract_is_valid(proposal: &RemediationProposal) -> bool {
    proposal.binding.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION
        && proposal.binding.finding_id == ENV004_FINDING_ID
        && proposal.binding.collected_at != 0
        && is_hex_digest(&proposal.binding.evidence_digest)
        && proposal.remediation_id == ENV004_REMEDIATION_ID
        && proposal.action == RemediationAction::ImportActivationEnvironment
        && proposal.target == RemediationTarget::SystemdUserActivationEnvironment
        && proposal.dry_run
        && !proposal.environment_updates.is_empty()
        && proposal.environment_updates.iter().all(|update| {
            COMPARISON_KEYS.contains(&update.key.as_str()) && !update.value.is_empty()
        })
        && proposal
            .environment_updates
            .windows(2)
            .all(|updates| updates[0].key < updates[1].key)
        && proposal.files_modified.is_empty()
        && proposal.service_restarts.is_empty()
        && proposal.package_changes.is_empty()
        && proposal.configuration_changes.is_empty()
        && proposal.apply == ApplyStatus::NotImplemented
}

fn is_hex_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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
            writeln!(output, "Binding:").expect("String writes cannot fail");
            writeln!(
                output,
                "  Snapshot schema: {}",
                proposal.binding.snapshot_schema_version
            )
            .expect("String writes cannot fail");
            writeln!(output, "  Collected at: {}", proposal.binding.collected_at)
                .expect("String writes cannot fail");
            writeln!(output, "  Finding id: {}", proposal.binding.finding_id)
                .expect("String writes cannot fail");
            writeln!(
                output,
                "  Evidence digest: {}",
                proposal.binding.evidence_digest
            )
            .expect("String writes cannot fail");
            writeln!(
                output,
                "  Proposal digest: {}",
                proposal.binding.proposal_digest
            )
            .expect("String writes cannot fail");
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

fn collect_actionable_evidence(
    environment: &EnvironmentInfo,
) -> Result<Vec<ActionableEvidence>, NotApplicableReason> {
    let mut seen_keys = BTreeSet::new();
    let mut actionable_evidence = Vec::new();
    for entry in &environment.activation_comparison.entries {
        if !COMPARISON_KEYS.contains(&entry.key.as_str()) || !seen_keys.insert(entry.key.as_str()) {
            return Err(NotApplicableReason::SnapshotInconsistent);
        }

        let process_value = environment.process.get(&entry.key).cloned();
        if process_value != entry.process_value
            || expected_relation(
                entry.process_value.as_deref(),
                entry.activation_value.as_deref(),
            ) != Some(entry.relation)
        {
            return Err(NotApplicableReason::SnapshotInconsistent);
        }

        match entry.relation {
            EnvironmentRelation::Equal => {}
            EnvironmentRelation::Different | EnvironmentRelation::MissingActivation => {
                let Some(value) = entry
                    .process_value
                    .as_deref()
                    .filter(|value| !value.is_empty())
                else {
                    return Err(NotApplicableReason::MissingProcessValue);
                };
                actionable_evidence.push(ActionableEvidence {
                    key: entry.key.clone(),
                    process_value: value.to_owned(),
                    activation_value: entry.activation_value.clone(),
                    relation: entry.relation,
                });
            }
            EnvironmentRelation::MissingProcess | EnvironmentRelation::NotChecked => {
                return Err(NotApplicableReason::MissingProcessValue);
            }
        }
    }

    if actionable_evidence.is_empty() {
        return Err(NotApplicableReason::NoActionableMismatch);
    }
    Ok(actionable_evidence)
}

fn digest_actionable_evidence(evidence: &[ActionableEvidence]) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "portaldoctor:env004:evidence:v1");
    hash_u64(&mut hasher, evidence.len() as u64);
    for entry in evidence {
        hash_text(&mut hasher, &entry.key);
        hash_text(&mut hasher, &entry.process_value);
        hash_optional_text(&mut hasher, entry.activation_value.as_deref());
        hash_text(&mut hasher, entry.relation.as_str());
    }
    hex_digest(&hasher.finalize())
}

fn digest_proposal(proposal: &RemediationProposal) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "portaldoctor:env004:proposal:v1");
    hash_field_u32(
        &mut hasher,
        "preview_schema_version",
        REMEDIATION_PREVIEW_SCHEMA_VERSION,
    );
    hash_field_u32(
        &mut hasher,
        "snapshot_schema_version",
        proposal.binding.snapshot_schema_version,
    );
    hash_field_u64(&mut hasher, "collected_at", proposal.binding.collected_at);
    hash_field_text(&mut hasher, "finding_id", &proposal.binding.finding_id);
    hash_field_text(
        &mut hasher,
        "evidence_digest",
        &proposal.binding.evidence_digest,
    );
    hash_field_text(&mut hasher, "remediation_id", &proposal.remediation_id);
    hash_field_text(&mut hasher, "action", action_digest_label(proposal.action));
    hash_field_text(&mut hasher, "target", target_digest_label(proposal.target));
    hash_field_bool(&mut hasher, "dry_run", proposal.dry_run);
    hash_field_updates(
        &mut hasher,
        "environment_updates",
        &proposal.environment_updates,
    );
    hash_field_strings(&mut hasher, "files_modified", &proposal.files_modified);
    hash_field_strings(&mut hasher, "service_restarts", &proposal.service_restarts);
    hash_field_strings(&mut hasher, "package_changes", &proposal.package_changes);
    hash_field_strings(
        &mut hasher,
        "configuration_changes",
        &proposal.configuration_changes,
    );
    hash_field_text(&mut hasher, "apply", apply_digest_label(proposal.apply));
    // `proposal_digest` itself is excluded because hashing it would be
    // self-referential. The other binding metadata above is explicit.
    hex_digest(&hasher.finalize())
}

fn digest_approval(approval: &RemediationApproval) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "portaldoctor:env004:approval:v2");
    hash_field_u32(
        &mut hasher,
        "approval_contract_version",
        approval.approval_contract_version,
    );
    hash_field_text(&mut hasher, "remediation_id", &approval.remediation_id);
    hash_field_text(&mut hasher, "proposal_digest", &approval.proposal_digest);
    hash_field_text(&mut hasher, "evidence_digest", &approval.evidence_digest);
    hash_field_text(&mut hasher, "action", action_digest_label(approval.action));
    hash_field_text(&mut hasher, "target", target_digest_label(approval.target));
    hash_field_text(
        &mut hasher,
        "user_approval",
        approval_state_digest_label(approval.user_approval),
    );
    // `approval_digest` is intentionally excluded to avoid a self-referential
    // hash. Every other bounded approval field is explicitly labelled above.
    hex_digest(&hasher.finalize())
}

fn action_digest_label(action: RemediationAction) -> &'static str {
    match action {
        RemediationAction::ImportActivationEnvironment => "import_activation_environment",
    }
}

fn target_digest_label(target: RemediationTarget) -> &'static str {
    match target {
        RemediationTarget::SystemdUserActivationEnvironment => {
            "systemd_user_activation_environment"
        }
    }
}

fn apply_digest_label(apply: ApplyStatus) -> &'static str {
    match apply {
        ApplyStatus::NotImplemented => "not_implemented",
    }
}

fn approval_state_digest_label(state: RemediationApprovalState) -> &'static str {
    match state {
        RemediationApprovalState::Approved => "approved",
        RemediationApprovalState::NotApproved => "not_approved",
    }
}

fn hash_field_text(hasher: &mut Sha256, label: &str, value: &str) {
    hash_text(hasher, label);
    hash_text(hasher, value);
}

fn hash_field_u32(hasher: &mut Sha256, label: &str, value: u32) {
    hash_text(hasher, label);
    hash_u32(hasher, value);
}

fn hash_field_u64(hasher: &mut Sha256, label: &str, value: u64) {
    hash_text(hasher, label);
    hash_u64(hasher, value);
}

fn hash_field_bool(hasher: &mut Sha256, label: &str, value: bool) {
    hash_text(hasher, label);
    hash_u8(hasher, u8::from(value));
}

fn hash_field_updates(hasher: &mut Sha256, label: &str, updates: &[EnvironmentUpdate]) {
    hash_text(hasher, label);
    hash_u64(hasher, updates.len() as u64);
    for update in updates {
        hash_field_text(hasher, "key", &update.key);
        hash_field_text(hasher, "value", &update.value);
    }
}

fn hash_field_strings(hasher: &mut Sha256, label: &str, values: &[String]) {
    hash_text(hasher, label);
    hash_u64(hasher, values.len() as u64);
    for value in values {
        hash_text(hasher, value);
    }
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hash_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

fn hash_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_u8(hasher, 1);
            hash_text(hasher, value);
        }
        None => hash_u8(hasher, 0),
    }
}

fn hash_u8(hasher: &mut Sha256, value: u8) {
    hasher.update([value]);
}

fn hash_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_be_bytes());
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn hex_digest(digest: &[u8]) -> String {
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("String writes cannot fail");
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
        ApplyStatus, ApprovalStaleEvidenceReason, ApprovalTamperReason, EffectApplicabilityReason,
        EffectMismatchReason, EffectUnavailableReason, Env004ApplyPermit,
        Env004ApprovalVerification, Env004EffectVerification, Env004ExecutionPlan,
        Env004PreviewVerification, Env004PriorActivation, Env004RollbackAction,
        NotApplicableReason, RemediationAction, RemediationApplicability, RemediationApproval,
        RemediationApprovalState, RemediationProposal, RemediationTarget, StaleEvidenceReason,
        VerificationSchemaReason, VerificationTamperReason, create_env004_apply_permit,
        create_env004_approval, create_env004_execution_plan, preview_env004, render_terminal,
        verify_env004_approval, verify_env004_effect, verify_env004_preview,
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

    fn approved_fixture() -> (Snapshot, RemediationProposal, RemediationApproval) {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&snapshot, &evaluate(&snapshot));
        let proposal = preview.proposal.expect("applicable proposal");
        let approval = create_env004_approval(&proposal, RemediationApprovalState::Approved)
            .expect("integrity-checked approval");
        (snapshot, proposal, approval)
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
        assert_eq!(proposal.binding.snapshot_schema_version, 1);
        assert_eq!(proposal.binding.collected_at, 1);
        assert_eq!(proposal.binding.finding_id, "ENV004");
        assert_eq!(proposal.binding.evidence_digest.len(), 64);
        assert_eq!(proposal.binding.proposal_digest.len(), 64);
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
        assert!(text.contains("Evidence digest:"));
        assert!(text.contains("Proposal digest:"));
        assert!(text.contains("Apply: not implemented"));
    }

    #[test]
    fn every_mutable_proposal_field_changes_recomputed_digest() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let base = preview_env004(&snapshot, &evaluate(&snapshot))
            .proposal
            .expect("applicable proposal");
        let base_digest = super::digest_proposal(&base);
        let mut mutations = Vec::new();

        let mut changed = base.clone();
        changed.binding.snapshot_schema_version += 1;
        mutations.push(changed);
        let mut changed = base.clone();
        changed.binding.collected_at += 1;
        mutations.push(changed);
        let mut changed = base.clone();
        changed.binding.finding_id = "ENV004.changed".to_owned();
        mutations.push(changed);
        let mut changed = base.clone();
        changed.binding.evidence_digest = "changed-evidence".to_owned();
        mutations.push(changed);
        let mut changed = base.clone();
        changed.remediation_id = "different-remediation".to_owned();
        mutations.push(changed);
        let mut changed = base.clone();
        changed.dry_run = false;
        mutations.push(changed);
        let mut changed = base.clone();
        changed.environment_updates[0].value = "changed-value".to_owned();
        mutations.push(changed);
        let mut changed = base.clone();
        changed.files_modified.push("a-file".to_owned());
        mutations.push(changed);
        let mut changed = base.clone();
        changed.service_restarts.push("a-service".to_owned());
        mutations.push(changed);
        let mut changed = base.clone();
        changed.package_changes.push("a-package".to_owned());
        mutations.push(changed);
        let mut changed = base.clone();
        changed
            .configuration_changes
            .push("a-configuration".to_owned());
        mutations.push(changed);

        for mutated in mutations {
            assert_ne!(base_digest, super::digest_proposal(&mutated));
        }
    }

    #[test]
    fn untouched_preview_with_fresh_same_evidence_is_valid() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let stored_preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let mut fresh_snapshot = stored_snapshot.clone();
        fresh_snapshot.collected_at = 2;

        assert_eq!(
            verify_env004_preview(&stored_preview, &fresh_snapshot, &evaluate(&fresh_snapshot),),
            Env004PreviewVerification::Valid
        );
    }

    #[test]
    fn mutated_preview_or_digest_is_tampered() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let stored_preview = preview_env004(&snapshot, &evaluate(&snapshot));

        let mut changed_field = stored_preview.clone();
        changed_field
            .proposal
            .as_mut()
            .expect("proposal")
            .configuration_changes
            .push("unexpected-change".to_owned());
        assert_eq!(
            verify_env004_preview(&changed_field, &snapshot, &evaluate(&snapshot)),
            Env004PreviewVerification::Tampered {
                reason: VerificationTamperReason::StoredProposalDigestMismatch,
            }
        );

        let mut changed_digest = stored_preview.clone();
        changed_digest
            .proposal
            .as_mut()
            .expect("proposal")
            .binding
            .proposal_digest = "tampered".to_owned();
        assert_eq!(
            verify_env004_preview(&changed_digest, &snapshot, &evaluate(&snapshot)),
            Env004PreviewVerification::Tampered {
                reason: VerificationTamperReason::StoredProposalDigestMismatch,
            }
        );
    }

    #[test]
    fn recomputed_side_effect_mutation_is_rejected_by_contract() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let mut changed = preview_env004(&snapshot, &evaluate(&snapshot));
        let proposal = changed.proposal.as_mut().expect("proposal");
        proposal
            .configuration_changes
            .push("unexpected-change".to_owned());
        proposal.binding.proposal_digest = super::digest_proposal(proposal);

        assert_eq!(
            verify_env004_preview(&changed, &snapshot, &evaluate(&snapshot)),
            Env004PreviewVerification::Tampered {
                reason: VerificationTamperReason::ProposalContractMismatch,
            }
        );
    }

    #[test]
    fn changed_process_or_activation_evidence_is_stale() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let stored_preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));

        let changed_process = [
            ("XDG_CURRENT_DESKTOP", "Hyprland"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let process_snapshot = snapshot(&changed_process, &activation);
        assert_eq!(
            verify_env004_preview(
                &stored_preview,
                &process_snapshot,
                &evaluate(&process_snapshot),
            ),
            Env004PreviewVerification::StaleEvidence {
                reason: StaleEvidenceReason::EvidenceDigestMismatch,
            }
        );

        let changed_activation = [
            ("XDG_CURRENT_DESKTOP", "Sway"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let activation_snapshot = snapshot(&process, &changed_activation);
        assert_eq!(
            verify_env004_preview(
                &stored_preview,
                &activation_snapshot,
                &evaluate(&activation_snapshot),
            ),
            Env004PreviewVerification::StaleEvidence {
                reason: StaleEvidenceReason::EvidenceDigestMismatch,
            }
        );
    }

    #[test]
    fn missing_env004_is_not_applicable_for_verification() {
        let process = healthy_process();
        let stale_activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &stale_activation);
        let stored_preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let fresh_snapshot = snapshot(&process, &process);

        assert_eq!(
            verify_env004_preview(&stored_preview, &fresh_snapshot, &evaluate(&fresh_snapshot),),
            Env004PreviewVerification::NotApplicable {
                reason: NotApplicableReason::FindingNotPresent,
            }
        );
    }

    #[test]
    fn unrelated_fresh_snapshot_change_remains_valid() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let stored_preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let mut fresh_snapshot = stored_snapshot.clone();
        fresh_snapshot.collected_at = 2;
        fresh_snapshot
            .environment
            .value
            .as_mut()
            .expect("environment value")
            .search_roots
            .config_roots = vec!["/secret/private/config".to_owned()];

        assert_eq!(
            verify_env004_preview(&stored_preview, &fresh_snapshot, &evaluate(&fresh_snapshot),),
            Env004PreviewVerification::Valid
        );
    }

    #[test]
    fn post_apply_converges_when_values_match_and_finding_disappears() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let stored_preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let fresh_snapshot = snapshot(&process, &process);
        let proposal = stored_preview.proposal.as_ref().expect("proposal");

        assert_eq!(
            verify_env004_effect(proposal, &fresh_snapshot, &evaluate(&fresh_snapshot)),
            Env004EffectVerification::Converged
        );
    }

    #[test]
    fn post_apply_stays_mismatched_when_env004_remains() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let stored_preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let proposal = stored_preview.proposal.as_ref().expect("proposal");

        assert_eq!(
            verify_env004_effect(proposal, &stored_snapshot, &evaluate(&stored_snapshot)),
            Env004EffectVerification::StillMismatched {
                reason: EffectMismatchReason::RequestedValueNotConverged,
            }
        );
    }

    #[test]
    fn post_apply_rejects_a_mismatch_with_missing_fresh_findings() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let proposal = preview.proposal.as_ref().expect("proposal");

        assert_eq!(
            verify_env004_effect(proposal, &stored_snapshot, &[]),
            Env004EffectVerification::Unavailable {
                reason: EffectUnavailableReason::InconsistentFindings,
            }
        );
    }

    #[test]
    fn post_apply_rejects_a_fake_finding_after_comparison_converges() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let proposal = preview.proposal.as_ref().expect("proposal");
        let fresh_snapshot = snapshot(&process, &process);
        let fake_findings = evaluate(&stored_snapshot);

        assert_eq!(
            verify_env004_effect(proposal, &fresh_snapshot, &fake_findings),
            Env004EffectVerification::Unavailable {
                reason: EffectUnavailableReason::InconsistentFindings,
            }
        );
    }

    #[test]
    fn post_apply_detects_an_additional_env004_finding() {
        let process = healthy_process();
        let stored_activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &stored_activation);
        let stored_preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let fresh_activation = [
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_DESKTOP", "other"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let fresh_snapshot = snapshot(&process, &fresh_activation);
        let proposal = stored_preview.proposal.as_ref().expect("proposal");

        assert_eq!(
            verify_env004_effect(proposal, &fresh_snapshot, &evaluate(&fresh_snapshot)),
            Env004EffectVerification::StillMismatched {
                reason: EffectMismatchReason::AdditionalEnv004Finding,
            }
        );
    }

    #[test]
    fn post_apply_becomes_no_longer_applicable_when_expected_process_changes() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let stored_preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let changed_process = [
            ("XDG_CURRENT_DESKTOP", "Hyprland"),
            ("XDG_SESSION_DESKTOP", "hyprland"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let fresh_snapshot = snapshot(&changed_process, &changed_process);
        let proposal = stored_preview.proposal.as_ref().expect("proposal");

        assert_eq!(
            verify_env004_effect(proposal, &fresh_snapshot, &evaluate(&fresh_snapshot)),
            Env004EffectVerification::NoLongerApplicable {
                reason: EffectApplicabilityReason::ExpectedProcessValueChanged,
            }
        );
    }

    #[test]
    fn post_apply_tampered_proposal_is_rejected_before_fresh_evidence() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let mut proposal = preview.proposal.expect("proposal");
        proposal
            .configuration_changes
            .push("unexpected-change".to_owned());

        assert_eq!(
            verify_env004_effect(
                &proposal,
                &snapshot(&process, &process),
                &evaluate(&snapshot(&process, &process)),
            ),
            Env004EffectVerification::Tampered {
                reason: VerificationTamperReason::StoredProposalDigestMismatch,
            }
        );
    }

    #[test]
    fn post_apply_unavailable_snapshot_is_typed() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let proposal = preview.proposal.as_ref().expect("proposal");
        let mut fresh_snapshot = snapshot(&process, &process);
        fresh_snapshot.environment = Section::unavailable("activation unavailable");

        assert_eq!(
            verify_env004_effect(proposal, &fresh_snapshot, &[]),
            Env004EffectVerification::Unavailable {
                reason: EffectUnavailableReason::SnapshotUnavailable,
            }
        );
    }

    #[test]
    fn post_apply_requires_a_performed_comparison() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let proposal = preview.proposal.as_ref().expect("proposal");
        let mut fresh_snapshot = snapshot(&process, &process);
        let comparison = &mut fresh_snapshot
            .environment
            .value
            .as_mut()
            .expect("environment value")
            .activation_comparison;
        comparison.performed = false;
        comparison.entries.clear();

        assert_eq!(
            verify_env004_effect(proposal, &fresh_snapshot, &[]),
            Env004EffectVerification::Unavailable {
                reason: EffectUnavailableReason::ComparisonNotPerformed,
            }
        );
    }

    #[test]
    fn post_apply_recomputed_invalid_contract_is_rejected() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let mut proposal = preview.proposal.expect("proposal");
        proposal
            .configuration_changes
            .push("unexpected-change".to_owned());
        proposal.binding.proposal_digest = super::digest_proposal(&proposal);

        assert_eq!(
            verify_env004_effect(&proposal, &snapshot(&process, &process), &[],),
            Env004EffectVerification::Tampered {
                reason: VerificationTamperReason::ProposalContractMismatch,
            }
        );
    }

    #[test]
    fn post_apply_ignores_unrelated_snapshot_fields() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stored_snapshot = snapshot(&process, &activation);
        let preview = preview_env004(&stored_snapshot, &evaluate(&stored_snapshot));
        let proposal = preview.proposal.as_ref().expect("proposal");
        let mut fresh_snapshot = snapshot(&process, &process);
        fresh_snapshot.collected_at = 2;
        fresh_snapshot
            .environment
            .value
            .as_mut()
            .expect("environment value")
            .search_roots
            .config_roots = vec!["/secret/private/config".to_owned()];

        assert_eq!(
            verify_env004_effect(proposal, &fresh_snapshot, &evaluate(&fresh_snapshot)),
            Env004EffectVerification::Converged
        );
    }

    #[test]
    fn approved_proposal_produces_a_bound_approval_record() {
        let (_snapshot, proposal, approval) = approved_fixture();

        assert_eq!(
            approval.approval_contract_version,
            super::REMEDIATION_APPROVAL_SCHEMA_VERSION
        );
        assert_eq!(approval.remediation_id, proposal.remediation_id);
        assert_eq!(approval.proposal_digest, proposal.binding.proposal_digest);
        assert_eq!(approval.evidence_digest, proposal.binding.evidence_digest);
        assert_eq!(approval.action, proposal.action);
        assert_eq!(approval.target, proposal.target);
        assert_eq!(approval.user_approval, RemediationApprovalState::Approved);
        assert_eq!(approval.approval_digest.len(), 64);
        assert_eq!(approval.approval_digest, super::digest_approval(&approval));
    }

    #[test]
    fn invalid_proposal_cannot_create_an_approval() {
        let (_snapshot, mut proposal, _approval) = approved_fixture();
        proposal.binding.proposal_digest = "tampered".to_owned();

        assert!(create_env004_approval(&proposal, RemediationApprovalState::Approved).is_none());
    }

    #[test]
    fn approved_matching_evidence_is_valid() {
        let (snapshot, proposal, approval) = approved_fixture();

        assert_eq!(
            verify_env004_approval(&approval, &proposal, &snapshot, &evaluate(&snapshot)),
            Env004ApprovalVerification::Valid
        );
    }

    #[test]
    fn approved_matching_evidence_admits_an_opaque_apply_permit() {
        let (snapshot, proposal, approval) = approved_fixture();
        let permit: Env004ApplyPermit =
            create_env004_apply_permit(&proposal, &approval, &snapshot, &evaluate(&snapshot))
                .expect("verified approval admits a permit");

        assert!(permit.binds_to(
            &proposal,
            &approval,
            &proposal.binding.evidence_digest,
            &proposal.environment_updates,
        ));
    }

    #[test]
    fn apply_permit_factory_rejects_unapproved_or_tampered_inputs() {
        let (snapshot, proposal, approval) = approved_fixture();
        let not_approved = create_env004_approval(&proposal, RemediationApprovalState::NotApproved)
            .expect("integrity-checked approval");
        assert!(
            create_env004_apply_permit(&proposal, &not_approved, &snapshot, &evaluate(&snapshot),)
                .is_none()
        );

        let mut tampered_approval = approval.clone();
        tampered_approval.user_approval = RemediationApprovalState::NotApproved;
        assert!(
            create_env004_apply_permit(
                &proposal,
                &tampered_approval,
                &snapshot,
                &evaluate(&snapshot),
            )
            .is_none()
        );

        let mut tampered_proposal = proposal.clone();
        tampered_proposal.environment_updates[0].value = "tampered".to_owned();
        assert!(
            create_env004_apply_permit(
                &tampered_proposal,
                &approval,
                &snapshot,
                &evaluate(&snapshot),
            )
            .is_none()
        );
    }

    #[test]
    fn apply_permit_factory_rejects_stale_or_not_applicable_evidence() {
        let (_stored_snapshot, proposal, approval) = approved_fixture();
        let process = healthy_process();
        let changed_activation = [
            ("XDG_CURRENT_DESKTOP", "Sway"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let stale_snapshot = snapshot(&process, &changed_activation);
        assert!(
            create_env004_apply_permit(
                &proposal,
                &approval,
                &stale_snapshot,
                &evaluate(&stale_snapshot),
            )
            .is_none()
        );

        let converged_snapshot = snapshot(&process, &process);
        assert!(
            create_env004_apply_permit(
                &proposal,
                &approval,
                &converged_snapshot,
                &evaluate(&converged_snapshot),
            )
            .is_none()
        );
    }

    #[test]
    fn apply_permit_cannot_match_different_bindings() {
        let (snapshot, proposal, approval) = approved_fixture();
        let permit =
            create_env004_apply_permit(&proposal, &approval, &snapshot, &evaluate(&snapshot))
                .expect("verified approval admits a permit");

        let mut different_proposal = proposal.clone();
        different_proposal.binding.proposal_digest = "other-proposal".to_owned();
        let mut different_approval = approval.clone();
        different_approval.approval_digest = "other-approval".to_owned();
        let mut different_updates = proposal.environment_updates.clone();
        different_updates[0].value = "other-value".to_owned();

        assert!(!permit.binds_to(
            &different_proposal,
            &approval,
            &proposal.binding.evidence_digest,
            &proposal.environment_updates,
        ));
        assert!(!permit.binds_to(
            &proposal,
            &different_approval,
            &proposal.binding.evidence_digest,
            &proposal.environment_updates,
        ));
        assert!(!permit.binds_to(
            &proposal,
            &approval,
            &proposal.binding.evidence_digest,
            &different_updates,
        ));
    }

    #[test]
    fn execution_plan_restores_different_and_unsets_missing_activation() {
        let (snapshot, proposal, approval) = approved_fixture();
        let permit =
            create_env004_apply_permit(&proposal, &approval, &snapshot, &evaluate(&snapshot))
                .expect("verified approval admits a permit");
        let plan: Env004ExecutionPlan = create_env004_execution_plan(permit)
            .expect("well-formed permit admits an execution plan");

        assert_eq!(plan.steps.len(), 2);
        let current_desktop = plan
            .steps
            .iter()
            .find(|step| step.key == "XDG_CURRENT_DESKTOP")
            .expect("desktop step");
        assert_eq!(
            current_desktop.prior_activation,
            Env004PriorActivation::Present("KDE".to_owned())
        );
        assert_eq!(
            current_desktop.rollback,
            Env004RollbackAction::Restore("KDE".to_owned())
        );

        let wayland_display = plan
            .steps
            .iter()
            .find(|step| step.key == "WAYLAND_DISPLAY")
            .expect("Wayland display step");
        assert_eq!(
            wayland_display.prior_activation,
            Env004PriorActivation::Absent
        );
        assert_eq!(wayland_display.rollback, Env004RollbackAction::Unset);
    }

    #[test]
    fn execution_plan_is_deterministically_sorted_for_multiple_keys() {
        let (snapshot, proposal, approval) = approved_fixture();
        let mut permit =
            create_env004_apply_permit(&proposal, &approval, &snapshot, &evaluate(&snapshot))
                .expect("verified approval admits a permit");
        permit.environment_updates.reverse();

        let plan = create_env004_execution_plan(permit).expect("plan remains deterministic");
        let keys = plan
            .steps
            .iter()
            .map(|step| step.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["WAYLAND_DISPLAY", "XDG_CURRENT_DESKTOP"]);
    }

    #[test]
    fn malformed_or_out_of_scope_permit_pre_state_fails_closed() {
        let (snapshot, proposal, approval) = approved_fixture();
        let mut missing_state =
            create_env004_apply_permit(&proposal, &approval, &snapshot, &evaluate(&snapshot))
                .expect("verified approval admits a permit");
        missing_state.prior_activation_values.pop();
        assert!(create_env004_execution_plan(missing_state).is_none());

        let mut unknown_key =
            create_env004_apply_permit(&proposal, &approval, &snapshot, &evaluate(&snapshot))
                .expect("verified approval admits a permit");
        unknown_key.environment_updates[0].key = "UNEXPECTED_KEY".to_owned();
        assert!(create_env004_execution_plan(unknown_key).is_none());
    }

    #[test]
    fn not_approved_state_never_becomes_valid() {
        let (snapshot, proposal, _) = approved_fixture();
        let approval = create_env004_approval(&proposal, RemediationApprovalState::NotApproved)
            .expect("integrity-checked approval");

        assert_eq!(
            verify_env004_approval(&approval, &proposal, &snapshot, &evaluate(&snapshot)),
            Env004ApprovalVerification::NotApproved
        );
    }

    #[test]
    fn not_approved_mutated_to_approved_is_tampered() {
        let (snapshot, proposal, _) = approved_fixture();
        let mut approval = create_env004_approval(&proposal, RemediationApprovalState::NotApproved)
            .expect("integrity-checked approval");
        approval.user_approval = RemediationApprovalState::Approved;

        assert_eq!(
            verify_env004_approval(&approval, &proposal, &snapshot, &evaluate(&snapshot)),
            Env004ApprovalVerification::Tampered {
                reason: ApprovalTamperReason::ApprovalDigest,
            }
        );
    }

    #[test]
    fn approved_mutated_to_not_approved_is_tampered() {
        let (snapshot, proposal, mut approval) = approved_fixture();
        approval.user_approval = RemediationApprovalState::NotApproved;

        assert_eq!(
            verify_env004_approval(&approval, &proposal, &snapshot, &evaluate(&snapshot)),
            Env004ApprovalVerification::Tampered {
                reason: ApprovalTamperReason::ApprovalDigest,
            }
        );
    }

    #[test]
    fn approval_digest_mutation_is_tampered() {
        let (snapshot, proposal, mut approval) = approved_fixture();
        approval.approval_digest = "tampered".to_owned();

        assert_eq!(
            verify_env004_approval(&approval, &proposal, &snapshot, &evaluate(&snapshot)),
            Env004ApprovalVerification::Tampered {
                reason: ApprovalTamperReason::ApprovalDigest,
            }
        );
    }

    #[test]
    fn proposal_or_approval_binding_mutation_is_tampered() {
        let (snapshot, proposal, approval) = approved_fixture();
        let mut changed_proposal = proposal.clone();
        changed_proposal.environment_updates[0].value = "changed".to_owned();
        assert_eq!(
            verify_env004_approval(
                &approval,
                &changed_proposal,
                &snapshot,
                &evaluate(&snapshot),
            ),
            Env004ApprovalVerification::Tampered {
                reason: ApprovalTamperReason::ProposalDigest,
            }
        );

        let mut changed_approval = approval.clone();
        changed_approval.evidence_digest = "different-evidence".to_owned();
        assert_eq!(
            verify_env004_approval(
                &changed_approval,
                &proposal,
                &snapshot,
                &evaluate(&snapshot),
            ),
            Env004ApprovalVerification::Tampered {
                reason: ApprovalTamperReason::ApprovalDigest,
            }
        );

        changed_approval.approval_digest = super::digest_approval(&changed_approval);
        assert_eq!(
            verify_env004_approval(
                &changed_approval,
                &proposal,
                &snapshot,
                &evaluate(&snapshot),
            ),
            Env004ApprovalVerification::Tampered {
                reason: ApprovalTamperReason::ApprovalBinding,
            }
        );
    }

    #[test]
    fn approval_contract_version_mutation_is_tampered() {
        let (snapshot, proposal, mut approval) = approved_fixture();
        approval.approval_contract_version += 1;

        assert_eq!(
            verify_env004_approval(&approval, &proposal, &snapshot, &evaluate(&snapshot)),
            Env004ApprovalVerification::Tampered {
                reason: ApprovalTamperReason::ApprovalDigest,
            }
        );

        approval.approval_digest = super::digest_approval(&approval);
        assert_eq!(
            verify_env004_approval(&approval, &proposal, &snapshot, &evaluate(&snapshot)),
            Env004ApprovalVerification::Tampered {
                reason: ApprovalTamperReason::ApprovalContractVersion,
            }
        );
    }

    #[test]
    fn stale_fresh_evidence_cannot_authorize_approval() {
        let (_stored_snapshot, proposal, approval) = approved_fixture();
        let process = healthy_process();
        let changed_activation = [
            ("XDG_CURRENT_DESKTOP", "Sway"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let fresh_snapshot = snapshot(&process, &changed_activation);

        assert_eq!(
            verify_env004_approval(
                &approval,
                &proposal,
                &fresh_snapshot,
                &evaluate(&fresh_snapshot),
            ),
            Env004ApprovalVerification::StaleEvidence {
                reason: ApprovalStaleEvidenceReason::EvidenceDigestMismatch,
            }
        );
    }

    #[test]
    fn converged_fresh_evidence_is_not_applicable_for_old_approval() {
        let (_snapshot, proposal, approval) = approved_fixture();
        let process = healthy_process();
        let fresh_snapshot = snapshot(&process, &process);

        assert_eq!(
            verify_env004_approval(
                &approval,
                &proposal,
                &fresh_snapshot,
                &evaluate(&fresh_snapshot),
            ),
            Env004ApprovalVerification::NotApplicable {
                reason: NotApplicableReason::FindingNotPresent,
            }
        );
    }

    #[test]
    fn inconsistent_fresh_findings_cannot_authorize_approval() {
        let (stored_snapshot, proposal, approval) = approved_fixture();
        let process = healthy_process();
        let fresh_snapshot = snapshot(&process, &process);
        let fake_findings = evaluate(&stored_snapshot);

        assert_eq!(
            verify_env004_approval(&approval, &proposal, &fresh_snapshot, &fake_findings,),
            Env004ApprovalVerification::StaleEvidence {
                reason: ApprovalStaleEvidenceReason::InconsistentFindings,
            }
        );
    }

    #[test]
    fn unsupported_fresh_schema_is_typed() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let stored_preview = preview_env004(&snapshot, &evaluate(&snapshot));
        let mut fresh_snapshot = snapshot.clone();
        fresh_snapshot.schema_version += 1;

        assert_eq!(
            verify_env004_preview(&stored_preview, &fresh_snapshot, &evaluate(&fresh_snapshot),),
            Env004PreviewVerification::UnsupportedSchema {
                reason: VerificationSchemaReason::SnapshotSchemaMismatch,
            }
        );
    }

    #[test]
    fn identical_evidence_produces_identical_binding() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let findings = evaluate(&snapshot);
        let first = preview_env004(&snapshot, &findings)
            .proposal
            .expect("applicable proposal");
        let second = preview_env004(&snapshot, &findings)
            .proposal
            .expect("applicable proposal");

        assert_eq!(first.binding, second.binding);
    }

    #[test]
    fn comparison_entry_order_does_not_change_binding() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let findings = evaluate(&snapshot);
        let mut reordered_snapshot = snapshot.clone();
        reordered_snapshot
            .environment
            .value
            .as_mut()
            .expect("environment value")
            .activation_comparison
            .entries
            .reverse();

        let original = preview_env004(&snapshot, &findings)
            .proposal
            .expect("applicable proposal");
        let reordered = preview_env004(&reordered_snapshot, &findings)
            .proposal
            .expect("applicable proposal");

        assert_eq!(original.binding, reordered.binding);
    }

    #[test]
    fn actionable_process_value_change_changes_binding() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let original_snapshot = snapshot(&process, &activation);
        let changed_process = [
            ("XDG_CURRENT_DESKTOP", "Hyprland"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let changed_snapshot = snapshot(&changed_process, &activation);

        let original = preview_env004(&original_snapshot, &evaluate(&original_snapshot))
            .proposal
            .expect("applicable proposal");
        let changed = preview_env004(&changed_snapshot, &evaluate(&changed_snapshot))
            .proposal
            .expect("applicable proposal");

        assert_ne!(
            original.binding.evidence_digest,
            changed.binding.evidence_digest
        );
        assert_ne!(
            original.binding.proposal_digest,
            changed.binding.proposal_digest
        );
    }

    #[test]
    fn actionable_relation_change_changes_binding() {
        let process = healthy_process();
        let different_activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let missing_activation = [
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let different_snapshot = snapshot(&process, &different_activation);
        let missing_snapshot = snapshot(&process, &missing_activation);

        let different = preview_env004(&different_snapshot, &evaluate(&different_snapshot))
            .proposal
            .expect("applicable proposal");
        let missing = preview_env004(&missing_snapshot, &evaluate(&missing_snapshot))
            .proposal
            .expect("applicable proposal");

        assert_ne!(
            different.binding.evidence_digest,
            missing.binding.evidence_digest
        );
        assert_ne!(
            different.binding.proposal_digest,
            missing.binding.proposal_digest
        );
    }

    #[test]
    fn collected_at_change_invalidates_proposal_digest() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let mut later_snapshot = snapshot.clone();
        later_snapshot.collected_at = 2;

        let original = preview_env004(&snapshot, &evaluate(&snapshot))
            .proposal
            .expect("applicable proposal");
        let later = preview_env004(&later_snapshot, &evaluate(&later_snapshot))
            .proposal
            .expect("applicable proposal");

        assert_eq!(
            original.binding.evidence_digest,
            later.binding.evidence_digest
        );
        assert_ne!(
            original.binding.proposal_digest,
            later.binding.proposal_digest
        );
    }

    #[test]
    fn unrelated_snapshot_fields_do_not_change_binding() {
        let process = healthy_process();
        let activation = [
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = snapshot(&process, &activation);
        let mut unrelated_snapshot = snapshot.clone();
        unrelated_snapshot
            .environment
            .value
            .as_mut()
            .expect("environment value")
            .search_roots
            .config_roots = vec!["/secret/private/config".to_owned()];

        let original = preview_env004(&snapshot, &evaluate(&snapshot))
            .proposal
            .expect("applicable proposal");
        let unrelated = preview_env004(&unrelated_snapshot, &evaluate(&unrelated_snapshot))
            .proposal
            .expect("applicable proposal");

        assert_eq!(original.binding, unrelated.binding);
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
        assert_eq!(value["proposal"]["binding"]["snapshot_schema_version"], 1);
        assert_eq!(value["proposal"]["binding"]["collected_at"], 1);
        assert_eq!(value["proposal"]["binding"]["finding_id"], "ENV004");
        assert_eq!(
            value["proposal"]["binding"]["evidence_digest"]
                .as_str()
                .expect("evidence digest")
                .len(),
            64
        );
        assert_eq!(
            value["proposal"]["binding"]["proposal_digest"]
                .as_str()
                .expect("proposal digest")
                .len(),
            64
        );
        assert_eq!(value["proposal"]["files_modified"], serde_json::json!([]));
        assert!(value["proposal"].get("command").is_none());
    }
}
