use crate::model::dbus::{DbusOutcome, PORTAL_FRONTEND_NAME};
use crate::model::evidence::Evidence;
use crate::model::finding::{Confidence, Finding, Severity};
use crate::model::snapshot::Snapshot;
use crate::rules::engine::DiagnosticRule;

/// Fluent builder so portal/D-Bus findings read like their rule intent.
struct DbusFinding {
    finding: Finding,
}

impl DbusFinding {
    fn new(id: &'static str) -> Self {
        Self {
            finding: Finding {
                id: id.to_owned(),
                severity: Severity::Warning,
                confidence: Confidence::High,
                title: String::new(),
                summary: String::new(),
                explanation: String::new(),
                evidence: vec![Evidence::ServiceState],
                impact: Some(String::new()),
                recommendation: Vec::new(),
                source_component: "dbus".to_owned(),
            },
        }
    }

    fn severity(mut self, severity: Severity) -> Self {
        self.finding.severity = severity;
        self
    }

    fn confidence(mut self, confidence: Confidence) -> Self {
        self.finding.confidence = confidence;
        self
    }

    fn title(mut self, title: &'static str) -> Self {
        title.clone_into(&mut self.finding.title);
        self
    }

    fn summary(mut self, summary: String) -> Self {
        self.finding.summary = summary;
        self
    }

    fn explanation(mut self, explanation: &str) -> Self {
        explanation.clone_into(&mut self.finding.explanation);
        self
    }

    fn evidence(mut self, evidence: Evidence) -> Self {
        self.finding.evidence = vec![evidence];
        self
    }

    fn impact(mut self, impact: &str) -> Self {
        self.finding.impact = Some(impact.to_owned());
        self
    }

    fn recommendation(mut self, recommendation: &str) -> Self {
        self.finding.recommendation = vec![recommendation.to_owned()];
        self
    }

    fn build(self) -> Finding {
        self.finding
    }
}

/// Names checked against the session bus, excluding the frontend (the
/// frontend has its own XDP001 rule).
fn backend_checks(snapshot: &Snapshot) -> impl Iterator<Item = (&str, &DbusOutcome)> {
    snapshot
        .dbus
        .value
        .as_ref()
        .map(|info| {
            info.checks
                .iter()
                .filter(|c| c.name != PORTAL_FRONTEND_NAME)
                .map(|c| (c.name.as_str(), &c.outcome))
        })
        .into_iter()
        .flatten()
}

/// DBUS001 — session bus unavailable (PRD §8).
pub struct Dbus001;

impl DiagnosticRule for Dbus001 {
    fn id(&self) -> &'static str {
        "DBUS001"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(info) = &snapshot.dbus.value else {
            return Vec::new();
        };
        if info.connected {
            return Vec::new();
        }
        vec![
            DbusFinding::new(self.id())
                .severity(Severity::Warning)
                .confidence(Confidence::High)
                .title("Session bus unavailable")
                .summary(
                    "No session D-Bus could be reached, so runtime verification was skipped."
                        .to_owned(),
                )
                .evidence(Evidence::MissingProvider)
                .explanation(
                    "Portal backends and the frontend communicate over the session bus; without \
                 it no portal call can succeed.",
                )
                .impact("Every portal-dependent feature is effectively unusable in this session.")
                .recommendation(
                    "Verify DBUS_SESSION_BUS_ADDRESS and that dbus-daemon/dbus-broker is running \
                 for the user session.",
                )
                .build(),
        ]
    }
}

/// DBUS002/// DBUS002 — selected service/backend cannot be reached or activated (PRD §8).
pub struct Dbus002;

impl DiagnosticRule for Dbus002 {
    fn id(&self) -> &'static str {
        "DBUS002"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        if !snapshot.dbus.value.as_ref().is_some_and(|i| i.connected) {
            return Vec::new();
        }
        // NoOwner means the backend exists as a descriptor but nobody owns its
        // bus name right now — exactly "cannot be reached or activated".
        let unreachable: Vec<&str> = backend_checks(snapshot)
            .filter(|(_, outcome)| matches!(outcome, DbusOutcome::NoOwner))
            .map(|(name, _)| name)
            .collect();
        let failed: Vec<&str> = backend_checks(snapshot)
            .filter(|(_, outcome)| {
                matches!(
                    outcome,
                    DbusOutcome::ActivationFailure
                        | DbusOutcome::AccessDenied
                        | DbusOutcome::MalformedResponse
                        | DbusOutcome::Other(_)
                )
            })
            .map(|(name, _)| name)
            .collect();

        if unreachable.is_empty() && failed.is_empty() {
            return Vec::new();
        }
        let mut names: Vec<String> = unreachable
            .iter()
            .chain(failed.iter())
            .map(|s| (*s).to_owned())
            .collect();
        names.sort_unstable();
        names.dedup();
        vec![
            DbusFinding::new(self.id())
                .severity(Severity::Warning)
                .confidence(Confidence::High)
                .title("Selected service cannot be reached on the bus")
                .summary(format!(
                    "The following configured backend bus name(s) have no owner or failed to \
                 activate: {}.",
                    names.join(", ")
                ))
                .evidence(Evidence::ServiceState)
                .explanation(
                    "Portal calls routed to these backends will fail or time out even though the \
             configuration looks correct.",
                )
                .impact("Those backends are unusable until their bus names come back.")
                .recommendation(
                    "Reinstall or repair the backend package; check `systemctl --user status` for \
             its unit.",
                )
                .build(),
        ]
    }
}

/// Timeout evidence helper for XDP001.
fn frontend_outcome(snapshot: &Snapshot) -> Option<(&str, &DbusOutcome)> {
    snapshot.dbus.value.as_ref().and_then(|info| {
        info.checks
            .iter()
            .find(|c| c.name == PORTAL_FRONTEND_NAME)
            .map(|c| (c.name.as_str(), &c.outcome))
    })
}

/// XDP001 — portal frontend cannot be discovered/reached (PRD §8).
pub struct Xdp001;

impl DiagnosticRule for Xdp001 {
    fn id(&self) -> &'static str {
        "XDP001"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some((name, outcome)) = frontend_outcome(snapshot) else {
            return Vec::new();
        };
        if matches!(outcome, DbusOutcome::HasOwner) {
            return Vec::new();
        }
        let detail = match outcome {
            DbusOutcome::Timeout => {
                "The query timed out; a wedged portal can hang clients.".to_owned()
            }
            DbusOutcome::NoSessionBus => "There is no session bus to talk to.".to_owned(),
            _ => format!("{name} has no owner on the session bus."),
        };
        let mut finding = DbusFinding::new(self.id())
            .severity(Severity::Warning)
            .confidence(Confidence::High)
            .title("Portal frontend cannot be reached")
            .summary(format!(
                "`{name}` is not reachable on the session bus. {detail}"
            ))
            .evidence(Evidence::MissingProvider)
            .explanation(
                "Without the frontend, every portal interface fails regardless of backend \
             health.",
            )
            .impact("Screen sharing, file choosers and settings propagation will not work.")
            .recommendation(
                "Check that xdg-desktop-portal is installed and running (`systemctl --user \
             status xdg-desktop-portal`).",
            )
            .build();
        if matches!(outcome, DbusOutcome::Timeout) {
            finding.evidence = vec![Evidence::DbusTimeout];
        }
        vec![finding]
    }
}

/// XDP002 — portal frontend runtime appears unhealthy (PRD §8).
pub struct Xdp002;

impl DiagnosticRule for Xdp002 {
    fn id(&self) -> &'static str {
        "XDP002"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(outcome) = frontend_outcome(snapshot).map(|(_, outcome)| outcome.clone()) else {
            return Vec::new();
        };
        if outcome != DbusOutcome::HasOwner {
            // Unreachable frontend is XDP001's domain.
            return Vec::new();
        }
        let Some(services) = &snapshot.services.value else {
            return Vec::new();
        };
        let Some(unit) = services.unit(crate::model::service::ServiceInfo::frontend_unit()) else {
            return Vec::new();
        };
        let unhealthy = matches!(unit.state, crate::model::service::UnitState::Failed);
        let mut mismatched_owner = false;
        if unit.state == crate::model::service::UnitState::Inactive {
            // Frontend owns the bus but reports inactive: contradictory state.
            mismatched_owner = true;
        }
        if !unhealthy && !mismatched_owner {
            return Vec::new();
        }
        let detail = if unhealthy {
            "unit reported failed".to_owned()
        } else {
            "unit inactive while owning the bus".to_owned()
        };
        vec![
            DbusFinding::new(self.id())
                .severity(Severity::Warning)
                .confidence(Confidence::Medium)
                .title("Portal frontend runtime appears unhealthy")
                .summary(format!(
                    "The frontend owns `{PORTAL_FRONTEND_NAME}` but its runtime appears \
                     unhealthy ({detail})."
                ))
                .evidence(Evidence::ServiceState)
                .explanation(
                    "A half-alive frontend accepts connections but may misbehave or crash under \
             load.",
                )
                .impact("Portal calls may intermittently fail or hang until the frontend restarts.")
                .recommendation(
                    "Restart the unit: systemctl --user restart xdg-desktop-portal.service",
                )
                .build(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{Dbus001, Dbus002, Xdp001, Xdp002};
    use crate::model::dbus::{DbusCheck, DbusInfo, DbusOutcome};
    use crate::model::finding::Finding;
    use crate::model::section::Section;
    use crate::model::service::{ServiceInfo, UnitState, UnitStatus};
    use crate::model::snapshot::Snapshot;
    use crate::rules::engine::DiagnosticRule;

    const FRONTEND: &str = "org.freedesktop.portal.Desktop";
    const BACKEND: &str = "org.freedesktop.impl.portal.desktop.gnome";

    fn check(name: &str, outcome: DbusOutcome) -> DbusCheck {
        DbusCheck {
            name: name.to_owned(),
            outcome,
        }
    }

    fn dbus_info(connected: bool, checks: Vec<DbusCheck>) -> Section<DbusInfo> {
        Section::available(DbusInfo { connected, checks })
    }

    fn services(frontend_state: UnitState) -> Section<ServiceInfo> {
        Section::available(ServiceInfo {
            units: vec![UnitStatus {
                unit: ServiceInfo::frontend_unit().to_owned(),
                state: frontend_state,
                sub_state: None,
                unit_file_state: Some("static".to_owned()),
            }],
        })
    }

    fn snapshot(dbus: Section<DbusInfo>, services: Section<ServiceInfo>) -> Snapshot {
        let mut s = Snapshot::new(0);
        s.session = Section::unsupported("fixture");
        s.dbus = dbus;
        s.services = services;
        s
    }

    fn ids(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.id.as_str()).collect()
    }

    #[test]
    fn no_session_bus_fires_dbus001_and_xdp001() {
        let s = snapshot(
            dbus_info(false, vec![check(FRONTEND, DbusOutcome::NoSessionBus)]),
            Section::unsupported("n/a"),
        );
        assert_eq!(ids(&Dbus001.evaluate(&s)), ["DBUS001"]);
        assert_eq!(ids(&Xdp001.evaluate(&s)), ["XDP001"]);
    }

    #[test]
    fn frontend_absent_fires_xdp001_only() {
        let s = snapshot(
            dbus_info(
                true,
                vec![
                    check(FRONTEND, DbusOutcome::NoOwner),
                    check(BACKEND, DbusOutcome::HasOwner),
                ],
            ),
            services(UnitState::Active),
        );
        assert!(Dbus001.evaluate(&s).is_empty());
        assert_eq!(ids(&Xdp001.evaluate(&s)), ["XDP001"]);
        // Backend reachable -> DBUS002 silent.
        assert!(Dbus002.evaluate(&s).is_empty());
    }

    #[test]
    fn frontend_timeout_fires_xdp001_with_timeout_evidence() {
        let s = snapshot(
            dbus_info(true, vec![check(FRONTEND, DbusOutcome::Timeout)]),
            Section::unsupported("n/a"),
        );
        let findings = Xdp001.evaluate(&s);
        assert_eq!(ids(&findings), ["XDP001"]);
        assert_eq!(
            findings[0].evidence[0],
            crate::model::evidence::Evidence::DbusTimeout
        );
    }

    #[test]
    fn selected_backend_unreachable_fires_dbus002() {
        let s = snapshot(
            dbus_info(
                true,
                vec![
                    check(FRONTEND, DbusOutcome::HasOwner),
                    check(BACKEND, DbusOutcome::NoOwner),
                ],
            ),
            services(UnitState::Active),
        );
        assert_eq!(ids(&Dbus002.evaluate(&s)), ["DBUS002"]);
        // Frontend healthy -> XDP001 silent.
        assert!(Xdp001.evaluate(&s).is_empty());
    }

    #[test]
    fn activation_failure_fires_dbus002() {
        let s = snapshot(
            dbus_info(
                true,
                vec![
                    check(FRONTEND, DbusOutcome::HasOwner),
                    check(BACKEND, DbusOutcome::ActivationFailure),
                ],
            ),
            services(UnitState::Failed),
        );
        assert_eq!(ids(&Dbus002.evaluate(&s)), ["DBUS002"]);
    }

    #[test]
    fn healthy_runtime_is_silent() {
        let s = snapshot(
            dbus_info(
                true,
                vec![
                    check(FRONTEND, DbusOutcome::HasOwner),
                    check(BACKEND, DbusOutcome::HasOwner),
                ],
            ),
            services(UnitState::Active),
        );
        assert!(Dbus001.evaluate(&s).is_empty());
        assert!(Dbus002.evaluate(&s).is_empty());
        assert!(Xdp001.evaluate(&s).is_empty());
        assert!(Xdp002.evaluate(&s).is_empty());
    }

    #[test]
    fn failed_frontend_unit_with_owner_fires_xdp002() {
        let s = snapshot(
            dbus_info(true, vec![check(FRONTEND, DbusOutcome::HasOwner)]),
            services(UnitState::Failed),
        );
        assert_eq!(ids(&Xdp002.evaluate(&s)), ["XDP002"]);
    }

    #[test]
    fn xdp002_silent_when_frontend_unreachable_or_unit_active() {
        // Unreachable frontend is XDP001's domain.
        let s1 = snapshot(
            dbus_info(true, vec![check(FRONTEND, DbusOutcome::NoOwner)]),
            services(UnitState::Failed),
        );
        assert!(Xdp002.evaluate(&s1).is_empty());

        // Active unit with owner is healthy.
        let s2 = snapshot(
            dbus_info(true, vec![check(FRONTEND, DbusOutcome::HasOwner)]),
            services(UnitState::Active),
        );
        assert!(Xdp002.evaluate(&s2).is_empty());
    }
}
