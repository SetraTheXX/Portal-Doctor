use crate::model::finding::Finding;
use crate::model::snapshot::Snapshot;

/// Deterministic diagnostic rule that consumes a snapshot only
/// (architecture §15 rule purity).
pub trait DiagnosticRule {
    /// Stable rule identifier, e.g. `"ENV001"`.
    fn id(&self) -> &'static str;

    /// Evaluate the snapshot and return zero or more findings.
    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding>;
}

/// All currently registered rules in stable evaluation order.
fn registered() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        Box::new(super::environment::Env001),
        Box::new(super::environment::Env002),
        Box::new(super::environment::Env003),
        Box::new(super::environment::Env004),
        Box::new(super::portal::Cfg001),
        Box::new(super::portal::Cfg002),
        Box::new(super::portal::Cfg003),
        Box::new(super::portal::Cfg004),
        Box::new(super::portal::Xdp003),
        Box::new(super::portal::Xdp004),
        Box::new(super::portal::Xdp005),
    ]
}

/// Evaluate every registered rule and return findings sorted by rule ID.
pub fn evaluate(snapshot: &Snapshot) -> Vec<Finding> {
    let mut findings = Vec::new();
    for rule in registered() {
        findings.extend(rule.evaluate(snapshot));
    }
    findings.sort_by(|a, b| a.id.cmp(&b.id));
    findings
}
