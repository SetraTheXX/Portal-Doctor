use crate::model::evidence::Evidence;
use crate::model::finding::{Confidence, Finding, Severity};
use crate::model::portal::RouteStatus;
use crate::model::snapshot::Snapshot;
use crate::rules::engine::DiagnosticRule;

/// Build a portal/config finding.
fn portal_finding(
    id: &'static str,
    severity: Severity,
    confidence: Confidence,
    title: &'static str,
    summary: String,
    evidence: Evidence,
    recommendation: &str,
) -> Finding {
    Finding {
        id: id.to_owned(),
        severity,
        confidence,
        title: title.to_owned(),
        summary,
        explanation: String::new(),
        evidence: vec![evidence],
        impact: None,
        recommendation: vec![recommendation.to_owned()],
        source_component: "portal".to_owned(),
    }
}

/// Current desktop names from `XDG_CURRENT_DESKTOP`, normalized like
/// upstream (trimmed, lowercased).
fn desktops(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .session
        .value
        .as_ref()
        .and_then(|s| s.current_desktop.as_ref())
        .map(|raw| crate::resolver::portal_routes::normalize_desktops(raw))
        .unwrap_or_default()
}

/// XDP003 — no portal backend definitions discovered (PRD §8).
pub struct Xdp003;

impl DiagnosticRule for Xdp003 {
    fn id(&self) -> &'static str {
        "XDP003"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(backends) = &snapshot.portal_backends.value else {
            return Vec::new();
        };
        if !backends.is_empty() {
            return Vec::new();
        }
        vec![portal_finding(
            self.id(),
            Severity::Warning,
            Confidence::High,
            "No portal backend definitions discovered",
            "No `.portal` backend descriptors were found in any effective `XDG` data root."
                .to_owned(),
            Evidence::MissingProvider,
            "Verify the portal backend packages (e.g. xdg-desktop-portal-gnome) are installed.",
        )]
    }
}

/// XDP004 — requested interface has no usable implementation (PRD §8).
pub struct Xdp004;

impl DiagnosticRule for Xdp004 {
    fn id(&self) -> &'static str {
        "XDP004"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let (Some(routes), Some(config)) =
            (&snapshot.portal_routes.value, &snapshot.portal_config.value)
        else {
            return Vec::new();
        };
        let configured: Vec<&str> = config
            .preferences
            .iter()
            .map(|p| p.interface.as_str())
            .collect();
        routes
            .iter()
            .filter(|route| {
                route.status != RouteStatus::Disabled
                    && route.available_candidates.is_empty()
                    && configured.contains(&route.interface.as_str())
            })
            .map(|route| {
                portal_finding(
                    self.id(),
                    Severity::Warning,
                    Confidence::High,
                    "Requested interface has no usable implementation",
                    format!(
                        "Interface {} is configured, but no discovered backend implements it \
                         in this desktop context.",
                        route.interface
                    ),
                    Evidence::MissingProvider,
                    "Install a backend that provides this interface, or remove the \
                     [preferred] entry for it.",
                )
            })
            .collect()
    }
}

/// XDP005 — configuration references an unavailable backend (PRD §8).
pub struct Xdp005;

impl DiagnosticRule for Xdp005 {
    fn id(&self) -> &'static str {
        "XDP005"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let (Some(config), Some(backends)) = (
            &snapshot.portal_config.value,
            &snapshot.portal_backends.value,
        ) else {
            return Vec::new();
        };
        let known: Vec<&str> = backends.iter().map(|b| b.id.as_str()).collect();
        let missing: Vec<String> = config
            .preferences
            .iter()
            .flat_map(|p| p.backends.iter())
            .filter(|token| !matches!(token.as_str(), "*" | "none"))
            .filter(|token| !known.contains(&token.as_str()))
            .cloned()
            .collect();
        if missing.is_empty() {
            return Vec::new();
        }
        let mut unique: Vec<String> = missing.clone();
        unique.sort_unstable();
        unique.dedup();
        vec![portal_finding(
            self.id(),
            Severity::Warning,
            Confidence::High,
            "Configuration references an unavailable backend",
            format!(
                "The selected `portals.conf` references backend(s) with no discovered \
                 descriptor: {}.",
                unique.join(", ")
            ),
            Evidence::ConfigSelection,
            "Install the referenced backend package or fix the [preferred] entry.",
        )]
    }
}

/// CFG001 — expected desktop-specific portal configuration not found (PRD §8).
pub struct Cfg001;

impl DiagnosticRule for Cfg001 {
    fn id(&self) -> &'static str {
        "CFG001"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let desktops = desktops(snapshot);
        let Some(config) = &snapshot.portal_config.value else {
            return Vec::new();
        };
        if desktops.is_empty() {
            return Vec::new();
        }
        let has_desktop_specific = config.selected_file.as_deref().is_some_and(|f| {
            std::path::Path::new(f)
                .file_name()
                .is_some_and(|name| name != "portals.conf")
        });
        if has_desktop_specific {
            return Vec::new();
        }
        let (severity, _impact) = match &config.selected_file {
            Some(_) => (
                Severity::Info,
                "Generic `portals.conf` is used for every desktop; desktop-specific \
                 routing is not configured."
                    .to_owned(),
            ),
            None => (
                Severity::Warning,
                "No `portals.conf` exists at all; routing falls back to upstream \
                 defaults for every desktop."
                    .to_owned(),
            ),
        };
        vec![portal_finding(
            self.id(),
            severity,
            Confidence::High,
            "Desktop-specific portal configuration not found",
            format!(
                "No `<desktop>-portals.conf` was found for {}.",
                desktops.join(", ")
            ),
            Evidence::ConfigSelection,
            "Create a desktop-specific config or accept the generic/default routing.",
        )]
    }
}

/// CFG002 — portal configuration parse error (PRD §8).
pub struct Cfg002;

impl DiagnosticRule for Cfg002 {
    fn id(&self) -> &'static str {
        "CFG002"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(config) = &snapshot.portal_config.value else {
            return Vec::new();
        };
        if config.parse_errors.is_empty() {
            return Vec::new();
        }
        vec![portal_finding(
            self.id(),
            Severity::Warning,
            Confidence::High,
            "Portal configuration parse error",
            format!(
                "`{}` contains malformed lines: {}.",
                config
                    .selected_file
                    .as_deref()
                    .unwrap_or("<no config file>"),
                config.parse_errors.join("; ")
            ),
            Evidence::ConfigSelection,
            "Fix or remove the malformed lines in the config file.",
        )]
    }
}

/// CFG003 — selected backend does not provide requested interface (PRD §8).
pub struct Cfg003;

impl DiagnosticRule for Cfg003 {
    fn id(&self) -> &'static str {
        "CFG003"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let (Some(routes), Some(config)) =
            (&snapshot.portal_routes.value, &snapshot.portal_config.value)
        else {
            return Vec::new();
        };
        routes
            .iter()
            .filter(|route| {
                !route.selected_candidates.is_empty() || route.status == RouteStatus::NoProvider
            })
            .filter(|route| {
                config
                    .preferences
                    .iter()
                    .any(|p| p.interface == route.interface)
            })
            .filter(|route| {
                !route.available_candidates.is_empty()
                    && route.selected_candidates.is_empty()
                    && !route
                        .requested_candidates
                        .iter()
                        .any(|t| matches!(t.as_str(), "*" | "none"))
            })
            .map(|route| {
                portal_finding(
                    self.id(),
                    Severity::Warning,
                    Confidence::High,
                    "Selected backend does not provide requested interface",
                    format!(
                        "Interface {} is configured for [{}], but none of those backends \
                         declares it.",
                        route.interface,
                        route.requested_candidates.join(", ")
                    ),
                    Evidence::ConfigSelection,
                    "Point the [preferred] entry at a backend that implements the interface.",
                )
            })
            .collect()
    }
}

/// CFG004 — suspicious duplicate/multi-provider resolution (PRD §8).
/// Conservative: fires only when several backends serve an interface without
/// any preference pinning the choice.
pub struct Cfg004;

impl DiagnosticRule for Cfg004 {
    fn id(&self) -> &'static str {
        "CFG004"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(routes) = &snapshot.portal_routes.value else {
            return Vec::new();
        };
        routes
            .iter()
            .filter(|route| route.requested_candidates.is_empty())
            .filter(|route| route.available_candidates.len() > 1)
            .map(|route| {
                portal_finding(
                    self.id(),
                    Severity::Info,
                    Confidence::Low,
                    "Suspicious multi-provider resolution",
                    format!(
                        "Interface {} has {} available backends ({}) and no [preferred] \
                         entry; upstream chooses one arbitrarily.",
                        route.interface,
                        route.available_candidates.len(),
                        route.available_candidates.join(", ")
                    ),
                    Evidence::ConfigSelection,
                    "Add a [preferred] entry to make the selection deterministic.",
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Cfg001, Cfg002, Cfg003, Cfg004, Xdp003, Xdp004, Xdp005};
    use crate::model::environment::{SessionInfo, SessionType};
    use crate::model::finding::Finding;
    use crate::model::portal::{
        PortalBackend, PortalConfigInfo, PortalPreference, PortalRoute, RouteEvidence, RouteStatus,
    };
    use crate::model::section::Section;
    use crate::model::snapshot::Snapshot;
    use crate::rules::engine::DiagnosticRule;
    use std::collections::BTreeSet;

    const SCREENSHOT: &str = "org.freedesktop.impl.portal.Screenshot";

    fn session(desktop: &str) -> Section<SessionInfo> {
        Section::available(SessionInfo {
            current_desktop: Some(desktop.to_owned()),
            session_desktop: None,
            session_type: Some(SessionType::Wayland),
            session_type_raw: Some("wayland".to_owned()),
            wayland_display: Some("wayland-0".to_owned()),
            display: None,
        })
    }

    fn pref(interface: &str, backends: &[&str]) -> PortalPreference {
        PortalPreference {
            interface: interface.to_owned(),
            backends: backends.iter().map(|b| (*b).to_owned()).collect(),
            source_file: "/cfg/xdg-desktop-portal/portals.conf".to_owned(),
            source_priority: 1,
        }
    }

    fn config(
        preferences: Vec<PortalPreference>,
        selected: Option<&str>,
        errors: Vec<String>,
    ) -> Section<PortalConfigInfo> {
        Section::available(PortalConfigInfo {
            candidate_files: vec![
                "/cfg/xdg-desktop-portal/gnome-portals.conf".to_owned(),
                "/cfg/xdg-desktop-portal/portals.conf".to_owned(),
            ],
            selected_file: selected.map(str::to_owned),
            preferences,
            parse_errors: errors,
        })
    }

    fn backends(ids: &[&str]) -> Section<Vec<PortalBackend>> {
        Section::available(
            ids.iter()
                .map(|id| PortalBackend {
                    id: (*id).to_owned(),
                    descriptor_path: format!("/usr/share/xdg-desktop-portal/portals/{id}.portal"),
                    duplicate_descriptors: Vec::new(),
                    dbus_name: format!("org.freedesktop.impl.portal.desktop.{id}"),
                    interfaces: BTreeSet::from([SCREENSHOT.to_owned()]),
                    legacy_use_in: Vec::new(),
                })
                .collect(),
        )
    }

    fn route(
        interface: &str,
        requested: &[&str],
        available: &[&str],
        selected: &[&str],
        status: RouteStatus,
    ) -> PortalRoute {
        PortalRoute {
            interface: interface.to_owned(),
            requested_candidates: requested.iter().map(|s| (*s).to_owned()).collect(),
            available_candidates: available.iter().map(|s| (*s).to_owned()).collect(),
            selected_candidates: selected.iter().map(|s| (*s).to_owned()).collect(),
            evidence: vec![RouteEvidence {
                message: "fixture evidence".to_owned(),
            }],
            status,
        }
    }

    fn snapshot(
        session: Section<SessionInfo>,
        config: Section<PortalConfigInfo>,
        backends: Section<Vec<PortalBackend>>,
        routes: Section<Vec<PortalRoute>>,
    ) -> Snapshot {
        let mut snapshot = Snapshot::new(0);
        snapshot.session = session;
        snapshot.portal_config = config;
        snapshot.portal_backends = backends;
        snapshot.portal_routes = routes;
        snapshot
    }

    fn ids(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.id.as_str()).collect()
    }

    // ---- XDP003 ----

    #[test]
    fn xdp003_fires_without_any_backend() {
        let s = snapshot(
            session("GNOME"),
            config(Vec::new(), None, Vec::new()),
            backends(&[]),
            Section::unsupported("no routes"),
        );
        assert_eq!(ids(&Xdp003.evaluate(&s)), ["XDP003"]);
    }

    #[test]
    fn xdp003_silent_when_backends_exist() {
        let s = snapshot(
            session("GNOME"),
            config(Vec::new(), None, Vec::new()),
            backends(&["gnome"]),
            Section::unsupported("no routes"),
        );
        assert!(Xdp003.evaluate(&s).is_empty());
    }

    // ---- XDP004 ----

    #[test]
    fn xdp004_fires_when_configured_interface_has_no_provider() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["gnome"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&[]),
            Section::available(vec![route(
                SCREENSHOT,
                &["gnome"],
                &[],
                &[],
                RouteStatus::NoProvider,
            )]),
        );
        assert_eq!(ids(&Xdp004.evaluate(&s)), ["XDP004"]);
    }

    #[test]
    fn xdp004_silent_when_provider_exists() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["gnome"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome"]),
            Section::available(vec![route(
                SCREENSHOT,
                &["gnome"],
                &["gnome"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        assert!(Xdp004.evaluate(&s).is_empty());
    }

    #[test]
    fn xdp004_silent_for_disabled_interfaces() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["none"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&[]),
            Section::available(vec![route(
                SCREENSHOT,
                &["none"],
                &[],
                &[],
                RouteStatus::Disabled,
            )]),
        );
        assert!(Xdp004.evaluate(&s).is_empty());
    }

    #[test]
    fn xdp004_silent_when_interface_not_configured() {
        let s = snapshot(
            session("GNOME"),
            config(Vec::new(), None, Vec::new()),
            backends(&[]),
            Section::available(vec![route(
                SCREENSHOT,
                &[],
                &[],
                &[],
                RouteStatus::NoProvider,
            )]),
        );
        assert!(Xdp004.evaluate(&s).is_empty());
    }

    // ---- XDP005 ----

    #[test]
    fn xdp005_fires_for_missing_backend_in_config() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["gnome", "kde"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome"]),
            Section::available(vec![route(
                SCREENSHOT,
                &["gnome", "kde"],
                &["gnome"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        let findings = Xdp005.evaluate(&s);
        assert_eq!(ids(&findings), ["XDP005"]);
        assert!(findings[0].summary.contains("kde"));
    }

    #[test]
    fn xdp005_silent_when_all_referenced_backends_exist() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["gnome", "gtk"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome", "gtk"]),
            Section::available(vec![route(
                SCREENSHOT,
                &["gnome", "gtk"],
                &["gnome", "gtk"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        assert!(Xdp005.evaluate(&s).is_empty());
    }

    #[test]
    fn xdp005_ignores_star_and_none_tokens() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["*", "none"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&[]),
            Section::available(vec![route(
                SCREENSHOT,
                &["*", "none"],
                &[],
                &[],
                RouteStatus::Disabled,
            )]),
        );
        assert!(Xdp005.evaluate(&s).is_empty());
    }

    // ---- CFG001 ----

    #[test]
    fn cfg001_warns_when_no_config_exists() {
        let s = snapshot(
            session("ubuntu:GNOME"),
            config(Vec::new(), None, Vec::new()),
            backends(&["gnome"]),
            Section::unsupported("n/a"),
        );
        let findings = Cfg001.evaluate(&s);
        assert_eq!(ids(&findings), ["CFG001"]);
        assert_eq!(
            findings[0].severity,
            crate::model::finding::Severity::Warning
        );
    }

    #[test]
    fn cfg001_informs_on_generic_fallback() {
        let s = snapshot(
            session("ubuntu:GNOME"),
            config(
                Vec::new(),
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome"]),
            Section::unsupported("n/a"),
        );
        let findings = Cfg001.evaluate(&s);
        assert_eq!(ids(&findings), ["CFG001"]);
        assert_eq!(findings[0].severity, crate::model::finding::Severity::Info);
    }

    #[test]
    fn cfg001_silent_with_desktop_specific_config() {
        let s = snapshot(
            session("ubuntu:GNOME"),
            config(
                Vec::new(),
                Some("/cfg/xdg-desktop-portal/gnome-portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome"]),
            Section::unsupported("n/a"),
        );
        assert!(Cfg001.evaluate(&s).is_empty());
    }

    #[test]
    fn cfg001_silent_without_desktop_identity() {
        let s = snapshot(
            session(""),
            config(Vec::new(), None, Vec::new()),
            backends(&["gnome"]),
            Section::unsupported("n/a"),
        );
        assert!(Cfg001.evaluate(&s).is_empty());
    }

    // ---- CFG002 ----

    #[test]
    fn cfg002_fires_on_parse_errors() {
        let s = snapshot(
            session("GNOME"),
            config(
                Vec::new(),
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                vec!["line 3: missing '='".to_owned()],
            ),
            backends(&["gnome"]),
            Section::unsupported("n/a"),
        );
        assert_eq!(ids(&Cfg002.evaluate(&s)), ["CFG002"]);
    }

    #[test]
    fn cfg002_silent_without_parse_errors() {
        let s = snapshot(
            session("GNOME"),
            config(
                Vec::new(),
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome"]),
            Section::unsupported("n/a"),
        );
        assert!(Cfg002.evaluate(&s).is_empty());
    }

    // ---- CFG003 ----

    #[test]
    fn cfg003_fires_when_preferred_backend_lacks_interface() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["gtk"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome", "gtk"]),
            Section::available(vec![route(
                SCREENSHOT,
                &["gtk"],
                &["gnome"],
                &[],
                RouteStatus::NoProvider,
            )]),
        );
        assert_eq!(ids(&Cfg003.evaluate(&s)), ["CFG003"]);
    }

    #[test]
    fn cfg003_silent_when_selection_succeeds() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["gnome"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome"]),
            Section::available(vec![route(
                SCREENSHOT,
                &["gnome"],
                &["gnome"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        assert!(Cfg003.evaluate(&s).is_empty());
    }

    #[test]
    fn cfg003_silent_without_preference() {
        let s = snapshot(
            session("GNOME"),
            config(Vec::new(), None, Vec::new()),
            backends(&["gnome"]),
            Section::available(vec![route(
                SCREENSHOT,
                &[],
                &["gnome"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        assert!(Cfg003.evaluate(&s).is_empty());
    }

    // ---- CFG004 ----

    #[test]
    fn cfg004_fires_for_unpinned_multi_provider() {
        let s = snapshot(
            session("GNOME"),
            config(Vec::new(), None, Vec::new()),
            backends(&["gnome", "gtk"]),
            Section::available(vec![route(
                SCREENSHOT,
                &[],
                &["gnome", "gtk"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        let findings = Cfg004.evaluate(&s);
        assert_eq!(ids(&findings), ["CFG004"]);
        assert_eq!(findings[0].severity, crate::model::finding::Severity::Info);
    }

    #[test]
    fn cfg004_silent_with_single_provider() {
        let s = snapshot(
            session("GNOME"),
            config(Vec::new(), None, Vec::new()),
            backends(&["gnome"]),
            Section::available(vec![route(
                SCREENSHOT,
                &[],
                &["gnome"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        assert!(Cfg004.evaluate(&s).is_empty());
    }

    #[test]
    fn cfg004_silent_when_preference_pins_selection() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref(SCREENSHOT, &["gnome"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome", "gtk"]),
            Section::available(vec![route(
                SCREENSHOT,
                &["gnome"],
                &["gnome", "gtk"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        assert!(Cfg004.evaluate(&s).is_empty());
    }

    #[test]
    fn cfg004_silent_when_default_preference_applies() {
        let s = snapshot(
            session("GNOME"),
            config(
                vec![pref("org.freedesktop.impl.portal.Default", &["gnome"])],
                Some("/cfg/xdg-desktop-portal/portals.conf"),
                Vec::new(),
            ),
            backends(&["gnome", "gtk"]),
            Section::available(vec![route(
                SCREENSHOT,
                &["gnome"],
                &["gnome", "gtk"],
                &["gnome"],
                RouteStatus::Selected,
            )]),
        );
        assert!(Cfg004.evaluate(&s).is_empty());
    }
}
