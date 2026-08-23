pub mod dbus;
pub mod engine;
pub mod environment;
pub mod portal;

/// Shared fixture-test helper enforcing the PRD §8/G5 finding contract.
#[cfg(test)]
pub(crate) mod contract {
    use crate::model::finding::Finding;

    /// Every finding must carry all structured fields with non-empty content.
    pub(crate) fn assert_contract(findings: &[Finding]) {
        for finding in findings {
            assert!(!finding.id.is_empty());
            assert!(!finding.title.is_empty(), "{}: empty title", finding.id);
            assert!(!finding.summary.is_empty(), "{}: empty summary", finding.id);
            assert!(
                !finding.explanation.is_empty(),
                "{}: empty explanation",
                finding.id
            );
            assert!(!finding.evidence.is_empty(), "{}: no evidence", finding.id);
            assert!(
                finding
                    .impact
                    .as_deref()
                    .is_none_or(|impact| !impact.is_empty()),
                "{}: empty impact string",
                finding.id
            );
            assert!(
                !finding.recommendation.is_empty(),
                "{}: no recommendation",
                finding.id
            );
            assert!(
                finding.recommendation.iter().all(|step| !step.is_empty()),
                "{}: empty recommendation step",
                finding.id
            );
            assert!(
                !finding.source_component.is_empty(),
                "{}: empty source_component",
                finding.id
            );
        }
    }
}
