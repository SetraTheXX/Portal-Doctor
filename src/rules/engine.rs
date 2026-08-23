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
        Box::new(super::dbus::Dbus001),
        Box::new(super::dbus::Dbus002),
        Box::new(super::dbus::Xdp001),
        Box::new(super::dbus::Xdp002),
    ]
}

/// Stable IDs of the complete v0.1 rule registry, in deterministic
/// evaluation order (lexicographic). The finding catalog documentation
/// (`docs/findings.md`) must list exactly these IDs.
// Exercised by the registry tests below; kept public for the docs workflow.
#[allow(dead_code)]
pub fn rule_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = registered().iter().map(|rule| rule.id()).collect();
    ids.sort_unstable();
    ids
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

#[cfg(test)]
mod tests {
    use super::rule_ids;

    /// Phase 4 gate: the v0.1 registry is exactly the documented catalog,
    /// with no duplicates and no undocumented rules.
    #[test]
    fn v01_registry_matches_documented_catalog() {
        let expected = [
            "CFG001", "CFG002", "CFG003", "CFG004", //
            "DBUS001", "DBUS002", //
            "ENV001", "ENV002", "ENV003", "ENV004", //
            "XDP001", "XDP002", "XDP003", "XDP004", "XDP005",
        ];
        assert_eq!(rule_ids(), expected);
    }

    #[test]
    fn registry_has_no_duplicates() {
        let ids = rule_ids();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len());
    }
}
