use crate::model::evidence::Evidence;
use crate::model::finding::{Confidence, Finding, Severity};
use crate::model::portal::{PortalRoute, RouteStatus};
use crate::model::section::Section;
use crate::model::snapshot::Snapshot;
use crate::model::status::CollectorState;
use crate::rules::engine::DiagnosticRule;

const SCREENCAST_INTERFACE: &str = "org.freedesktop.impl.portal.ScreenCast";

/// Fluent builder shared by the `PipeWire` and `ScreenCast` findings.
struct MediaFinding {
    finding: Finding,
}

impl MediaFinding {
    fn new(id: &'static str, source_component: &'static str) -> Self {
        Self {
            finding: Finding {
                id: id.to_owned(),
                severity: Severity::Warning,
                confidence: Confidence::High,
                title: String::new(),
                summary: String::new(),
                explanation: String::new(),
                evidence: Vec::new(),
                impact: Some(String::new()),
                recommendation: Vec::new(),
                source_component: source_component.to_owned(),
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

    fn evidence(mut self, evidence: Vec<Evidence>) -> Self {
        self.finding.evidence = evidence;
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

/// PW001 — the `pw-dump` command or `PipeWire` endpoint is unavailable.
pub struct Pw001;

impl DiagnosticRule for Pw001 {
    fn id(&self) -> &'static str {
        "PW001"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let section = &snapshot.pipewire;
        if is_not_collected(section) {
            return Vec::new();
        }
        if !matches!(
            section.status,
            CollectorState::Unsupported | CollectorState::Unavailable
        ) {
            return Vec::new();
        }
        let detail = section
            .errors
            .first()
            .map_or("PipeWire state is unavailable.", |note| {
                note.message.as_str()
            });
        vec![
            MediaFinding::new(self.id(), "pipewire")
                .title("PipeWire is unavailable")
                .summary(format!("PipeWire state could not be collected: {detail}"))
                .explanation(
                    "ScreenCast needs a reachable PipeWire session after the portal backend is selected. A missing `pw-dump` tool or an unavailable PipeWire endpoint prevents the media path from being verified.",
                )
                .evidence(vec![Evidence::PipeWireState])
                .impact("ScreenCast readiness cannot be confirmed and capture may fail before a stream is created.")
                .recommendation(
                    "Install the package that provides `pw-dump`, then verify that the user PipeWire session is running.",
                )
                .build(),
        ]
    }
}

/// PW002 — `wpctl status` cannot verify the WirePlumber/session-manager side.
pub struct Pw002;

impl DiagnosticRule for Pw002 {
    fn id(&self) -> &'static str {
        "PW002"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let section = &snapshot.wireplumber;
        if is_not_collected(section) {
            return Vec::new();
        }
        if section.status == CollectorState::Available {
            return Vec::new();
        }
        let detail = section
            .errors
            .first()
            .map_or("WirePlumber state is unavailable.", |note| {
                note.message.as_str()
            });
        vec![
            MediaFinding::new(self.id(), "pipewire")
                .title("WirePlumber is unavailable")
                .summary(format!("WirePlumber state could not be verified: {detail}"))
                .explanation(
                    "WirePlumber provides the session-manager side of the PipeWire desktop media stack. Portal routing may be correct while the session manager is missing, unreachable or unable to answer a bounded status query.",
                )
                .evidence(vec![Evidence::WirePlumberState])
                .impact("The media graph may not be prepared for a ScreenCast stream.")
                .recommendation(
                    "Check the user WirePlumber service and rerun `wpctl status` from the same graphical session.",
                )
                .build(),
        ]
    }
}

/// PW003 — `PipeWire` was invoked but its state query could not be completed.
pub struct Pw003;

impl DiagnosticRule for Pw003 {
    fn id(&self) -> &'static str {
        "PW003"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let section = &snapshot.pipewire;
        if !matches!(
            section.status,
            CollectorState::TimedOut
                | CollectorState::PermissionDenied
                | CollectorState::ParseError
        ) {
            return Vec::new();
        }
        let detail = section
            .errors
            .first()
            .map_or("the state query did not complete", |note| {
                note.message.as_str()
            });
        vec![
            MediaFinding::new(self.id(), "pipewire")
                .severity(Severity::Warning)
                .confidence(Confidence::Medium)
                .title("PipeWire state query failed")
                .summary(format!("The bounded PipeWire state query failed: {detail}"))
                .explanation(
                    "PortalDoctor separates a failed state query from a confirmed missing PipeWire endpoint. The result may be caused by a timeout, permission boundary or an output format that could not be parsed safely.",
                )
                .evidence(vec![Evidence::PipeWireState])
                .impact("The media path is unknown; a ScreenCast failure cannot be localized yet.")
                .recommendation(
                    "Run `pw-dump --no-colors` manually, check the user PipeWire session and review the command boundary before retrying.",
                )
                .build(),
        ]
    }
}

/// SC001 — no usable `ScreenCast` provider is selected for the current desktop.
pub struct Sc001;

impl DiagnosticRule for Sc001 {
    fn id(&self) -> &'static str {
        "SC001"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        if !no_usable_screencast_route(snapshot) {
            return Vec::new();
        }
        vec![
            MediaFinding::new(self.id(), "screencast")
                .title("No usable ScreenCast backend")
                .summary(
                    "No ScreenCast backend is selected and available for the current desktop."
                        .to_owned(),
                )
                .explanation(
                    "ScreenCast requests need a backend that advertises the ScreenCast interface and survives the desktop-specific routing rules. Installed backends for other interfaces do not satisfy this requirement.",
                )
                .evidence(vec![Evidence::ScreenCastRoute])
                .impact("Applications cannot create a ScreenCast session through the portal stack.")
                .recommendation(
                    "Install or select a backend that implements `org.freedesktop.impl.portal.ScreenCast`, then rerun `portaldoctor portal explain ScreenCast`.",
                )
                .build(),
        ]
    }
}

/// SC002 — routing exists, but the passive media-path checks are unavailable.
pub struct Sc002;

impl DiagnosticRule for Sc002 {
    fn id(&self) -> &'static str {
        "SC002"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(route) = screencast_route(snapshot) else {
            return Vec::new();
        };
        if route.status != RouteStatus::Selected {
            return Vec::new();
        }

        // A hand-built/pre-Phase-5 snapshot has not attempted media
        // collection yet. It must not be diagnosed as a broken media path.
        if is_not_collected(&snapshot.pipewire) || is_not_collected(&snapshot.wireplumber) {
            return Vec::new();
        }

        let mut unavailable = Vec::new();
        let mut evidence = vec![Evidence::ScreenCastRoute];
        if snapshot.pipewire.status != CollectorState::Available {
            unavailable.push(format!("PipeWire {}", snapshot.pipewire.status));
            evidence.push(Evidence::PipeWireState);
        }
        if snapshot.wireplumber.status != CollectorState::Available {
            unavailable.push(format!("WirePlumber {}", snapshot.wireplumber.status));
            evidence.push(Evidence::WirePlumberState);
        }
        if unavailable.is_empty() {
            return Vec::new();
        }

        vec![
            MediaFinding::new(self.id(), "screencast")
                .title("ScreenCast route exists but media path is unavailable")
                .summary(format!(
                    "ScreenCast is routed to {}, but {}.",
                    route.selected_candidates.join(", "),
                    unavailable.join(" and ")
                ))
                .explanation(
                    "A selected portal backend proves only the D-Bus route. ScreenCast also needs a reachable PipeWire graph and a functioning session manager before an active probe can create a usable stream.",
                )
                .evidence(evidence)
                .impact("The portal may open a source selector but fail before returning a usable media stream.")
                .recommendation(
                    "Repair the reported PipeWire/WirePlumber condition, then rerun the passive check before debugging the calling application.",
                )
                .build(),
        ]
    }
}

fn screencast_route(snapshot: &Snapshot) -> Option<&PortalRoute> {
    snapshot
        .portal_routes
        .value
        .as_ref()?
        .iter()
        .find(|route| route.interface == SCREENCAST_INTERFACE)
}

fn is_not_collected<T>(section: &Section<T>) -> bool {
    section.status == CollectorState::Unsupported
        && section
            .errors
            .iter()
            .any(|note| note.message == "not collected")
}

fn no_usable_screencast_route(snapshot: &Snapshot) -> bool {
    // A missing desktop identity means route resolution is operating without
    // enough context; report that environment gap instead of turning the
    // resulting no-provider route into a false ScreenCast diagnosis.
    let has_desktop_identity = snapshot
        .session
        .value
        .as_ref()
        .and_then(|session| session.current_desktop.as_deref())
        .is_some_and(|desktop| !desktop.trim().is_empty());
    if !has_desktop_identity {
        return false;
    }

    if let Some(route) = screencast_route(snapshot) {
        return route.status == RouteStatus::NoProvider;
    }
    snapshot
        .portal_backends
        .value
        .as_ref()
        .is_some_and(|backends| {
            !backends.is_empty()
                && !backends
                    .iter()
                    .any(|backend| backend.interfaces.contains(SCREENCAST_INTERFACE))
        })
}

#[cfg(test)]
mod tests {
    use super::{Pw001, Pw002, Pw003, Sc001, Sc002};
    use crate::model::environment::{SessionInfo, SessionType};
    use crate::model::finding::Finding;
    use crate::model::pipewire::{PipeWireInfo, WirePlumberInfo};
    use crate::model::portal::{PortalBackend, PortalRoute, RouteStatus};
    use crate::model::section::Section;
    use crate::model::snapshot::Snapshot;
    use crate::model::status::CollectorState;
    use crate::rules::engine::DiagnosticRule;

    const SCREENCAST: &str = "org.freedesktop.impl.portal.ScreenCast";

    fn pipewire_section(status: CollectorState) -> Section<PipeWireInfo> {
        match status {
            CollectorState::Available => Section::available(PipeWireInfo {
                model_version: 1,
                version: Some("1.6.2".to_owned()),
                object_count: 1,
                node_count: 0,
                link_count: 0,
                portal_client_count: 0,
                screen_cast_source_count: 0,
                nodes: Vec::new(),
                links: Vec::new(),
            }),
            CollectorState::Unsupported => Section::unsupported("pw-dump is not installed"),
            CollectorState::TimedOut => Section::timed_out("pw-dump timed out"),
            CollectorState::PermissionDenied => Section::permission_denied("permission denied"),
            CollectorState::ParseError => Section::parse_error("invalid JSON"),
            CollectorState::Unavailable => Section::unavailable("endpoint unavailable"),
        }
    }

    fn wireplumber_section(status: CollectorState) -> Section<WirePlumberInfo> {
        match status {
            CollectorState::Available => Section::available(WirePlumberInfo {
                model_version: 1,
                pipewire_version: Some("1.6.2".to_owned()),
                wireplumber_client_count: 1,
            }),
            CollectorState::Unsupported => Section::unsupported("wpctl is not installed"),
            CollectorState::TimedOut => Section::timed_out("wpctl timed out"),
            CollectorState::PermissionDenied => Section::permission_denied("permission denied"),
            CollectorState::ParseError => Section::parse_error("malformed status"),
            CollectorState::Unavailable => Section::unavailable("WirePlumber unavailable"),
        }
    }

    fn route(status: RouteStatus) -> PortalRoute {
        PortalRoute {
            interface: SCREENCAST.to_owned(),
            requested_candidates: vec!["gnome".to_owned()],
            available_candidates: if status == RouteStatus::Selected {
                vec!["gnome".to_owned()]
            } else {
                Vec::new()
            },
            selected_candidates: if status == RouteStatus::Selected {
                vec!["gnome".to_owned()]
            } else {
                Vec::new()
            },
            evidence: Vec::new(),
            status,
        }
    }

    fn snapshot(
        pipewire_status: CollectorState,
        wireplumber_status: CollectorState,
        route_status: RouteStatus,
    ) -> Snapshot {
        let mut snapshot = Snapshot::new(0);
        snapshot.pipewire = pipewire_section(pipewire_status);
        snapshot.wireplumber = wireplumber_section(wireplumber_status);
        snapshot.session = Section::available(SessionInfo {
            current_desktop: Some("GNOME".to_owned()),
            session_desktop: Some("GNOME".to_owned()),
            session_type: Some(SessionType::Wayland),
            session_type_raw: Some("wayland".to_owned()),
            wayland_display: Some("wayland-0".to_owned()),
            display: Some(":0".to_owned()),
        });
        snapshot.portal_routes = Section::available(vec![route(route_status)]);
        snapshot
    }

    fn evaluated(findings: Vec<Finding>) -> Vec<Finding> {
        crate::rules::contract::assert_contract(&findings);
        findings
    }

    fn ids(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|finding| finding.id.as_str()).collect()
    }

    #[test]
    fn unavailable_pipewire_tool_fires_pw001_not_pw003() {
        let snapshot = snapshot(
            CollectorState::Unsupported,
            CollectorState::Available,
            RouteStatus::Selected,
        );
        assert_eq!(ids(&evaluated(Pw001.evaluate(&snapshot))), ["PW001"]);
        assert!(evaluated(Pw003.evaluate(&snapshot)).is_empty());
    }

    #[test]
    fn failed_pipewire_query_fires_pw003() {
        let snapshot = snapshot(
            CollectorState::TimedOut,
            CollectorState::Available,
            RouteStatus::Selected,
        );
        assert!(evaluated(Pw001.evaluate(&snapshot)).is_empty());
        assert_eq!(ids(&evaluated(Pw003.evaluate(&snapshot))), ["PW003"]);
    }

    #[test]
    fn unavailable_wireplumber_fires_pw002() {
        let snapshot = snapshot(
            CollectorState::Available,
            CollectorState::Unavailable,
            RouteStatus::Selected,
        );
        assert_eq!(ids(&evaluated(Pw002.evaluate(&snapshot))), ["PW002"]);
    }

    #[test]
    fn missing_screencast_route_fires_sc001() {
        let snapshot = snapshot(
            CollectorState::Available,
            CollectorState::Available,
            RouteStatus::NoProvider,
        );
        assert_eq!(ids(&evaluated(Sc001.evaluate(&snapshot))), ["SC001"]);
        assert!(evaluated(Sc002.evaluate(&snapshot)).is_empty());
    }

    #[test]
    fn missing_desktop_context_does_not_create_sc001() {
        let mut snapshot = snapshot(
            CollectorState::Available,
            CollectorState::Available,
            RouteStatus::NoProvider,
        );
        snapshot.session = Section::available(SessionInfo {
            current_desktop: None,
            session_desktop: None,
            session_type: Some(SessionType::Wayland),
            session_type_raw: Some("wayland".to_owned()),
            wayland_display: Some("wayland-0".to_owned()),
            display: Some(":0".to_owned()),
        });
        assert!(evaluated(Sc001.evaluate(&snapshot)).is_empty());
    }

    #[test]
    fn backend_inventory_without_screencast_interface_fires_sc001() {
        let mut snapshot = snapshot(
            CollectorState::Available,
            CollectorState::Available,
            RouteStatus::Selected,
        );
        snapshot.portal_routes = Section::available(Vec::new());
        snapshot.portal_backends = Section::available(vec![PortalBackend {
            id: "fake".to_owned(),
            descriptor_path: "/usr/share/xdg-desktop-portal/portals/fake.portal".to_owned(),
            duplicate_descriptors: Vec::new(),
            dbus_name: "org.example.portal.desktop.fake".to_owned(),
            interfaces: ["org.freedesktop.impl.portal.Screenshot".to_owned()]
                .into_iter()
                .collect(),
            legacy_use_in: Vec::new(),
        }]);
        assert_eq!(ids(&evaluated(Sc001.evaluate(&snapshot))), ["SC001"]);
    }

    #[test]
    fn empty_backend_inventory_does_not_duplicate_xdp003() {
        let mut snapshot = snapshot(
            CollectorState::Available,
            CollectorState::Available,
            RouteStatus::Selected,
        );
        snapshot.portal_routes = Section::available(Vec::new());
        snapshot.portal_backends = Section::available(Vec::new());
        assert!(evaluated(Sc001.evaluate(&snapshot)).is_empty());
    }

    #[test]
    fn selected_route_with_missing_media_fires_sc002() {
        let snapshot = snapshot(
            CollectorState::Unavailable,
            CollectorState::TimedOut,
            RouteStatus::Selected,
        );
        let findings = evaluated(Sc002.evaluate(&snapshot));
        assert_eq!(ids(&findings), ["SC002"]);
        assert_eq!(findings[0].evidence.len(), 3);
    }

    #[test]
    fn healthy_route_and_media_stack_are_silent() {
        let snapshot = snapshot(
            CollectorState::Available,
            CollectorState::Available,
            RouteStatus::Selected,
        );
        assert!(evaluated(Pw001.evaluate(&snapshot)).is_empty());
        assert!(evaluated(Pw002.evaluate(&snapshot)).is_empty());
        assert!(evaluated(Pw003.evaluate(&snapshot)).is_empty());
        assert!(evaluated(Sc001.evaluate(&snapshot)).is_empty());
        assert!(evaluated(Sc002.evaluate(&snapshot)).is_empty());
    }
}
