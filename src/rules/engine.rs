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

/// Canonical public finding catalog. Rule implementations must expose exactly
/// one of these stable IDs; the catalog is also checked against
/// `docs/findings.md` so documentation cannot silently drift from runtime.
pub const FINDING_IDS: &[&str] = &[
    "CFG001", "CFG002", "CFG003", "CFG004", "DBUS001", "DBUS002", "ENV001", "ENV002", "ENV003",
    "ENV004", "PW001", "PW002", "PW003", "SC001", "SC002", "XDP001", "XDP002", "XDP003", "XDP004",
    "XDP005", "XDP006",
];

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
        Box::new(super::compatibility::Xdp006),
        Box::new(super::pipewire::Pw001),
        Box::new(super::pipewire::Pw002),
        Box::new(super::pipewire::Pw003),
        Box::new(super::pipewire::Sc001),
        Box::new(super::pipewire::Sc002),
        Box::new(super::dbus::Dbus001),
        Box::new(super::dbus::Dbus002),
        Box::new(super::dbus::Xdp001),
        Box::new(super::dbus::Xdp002),
    ]
}

/// Stable IDs of the current rule registry, in deterministic evaluation order
/// (lexicographic). The finding catalog documentation (`docs/findings.md`) must
/// list exactly these IDs.
// Exercised by the registry tests below; kept public for the docs workflow.
#[allow(dead_code)]
pub fn rule_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = registered().iter().map(|rule| rule.id()).collect();
    ids.sort_unstable();
    debug_assert_eq!(ids.as_slice(), FINDING_IDS);
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
    use std::collections::BTreeSet;

    use super::{FINDING_IDS, rule_ids};

    /// The registry is exactly the canonical runtime catalog.
    #[test]
    fn registry_matches_canonical_catalog() {
        assert_eq!(rule_ids().as_slice(), FINDING_IDS);
    }

    #[test]
    fn catalog_ids_are_unique_and_well_formed() {
        let unique = FINDING_IDS.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), FINDING_IDS.len());
        assert!(FINDING_IDS.iter().all(|id| {
            let Some((prefix, number)) = id.len().checked_sub(3).map(|split| id.split_at(split))
            else {
                return false;
            };
            (2..=4).contains(&prefix.len())
                && prefix.bytes().all(|byte| byte.is_ascii_uppercase())
                && number.bytes().all(|byte| byte.is_ascii_digit())
        }));
    }

    #[test]
    fn documented_catalog_matches_runtime_catalog_without_duplicates() {
        let docs = include_str!("../../docs/findings.md");
        let documented = docs
            .lines()
            .filter_map(|line| {
                let row = line.trim().strip_prefix("| `")?;
                let (id, _) = row.split_once("` |")?;
                Some(id)
            })
            .collect::<Vec<_>>();
        let unique_documented = documented.iter().copied().collect::<BTreeSet<_>>();
        let expected = FINDING_IDS.iter().copied().collect::<BTreeSet<_>>();

        assert_eq!(documented.len(), unique_documented.len());
        assert_eq!(unique_documented, expected);
    }
}
