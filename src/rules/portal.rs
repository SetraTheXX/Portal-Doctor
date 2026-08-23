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

/// Current desktop names from `XDG_CURRENT_DESKTOP` (colon-separated).
fn desktops(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .session
        .value
        .as_ref()
        .and_then(|s| s.current_desktop.as_ref())
        .map(|raw| {
            raw.split(':')
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_owned)
                .collect()
        })
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
        let has_desktop_specific = config
            .selected_file
            .as_deref()
            .is_some_and(|f| !f.ends_with("portals.conf"));
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
        let (Some(routes), Some(config)) =
            (&snapshot.portal_routes.value, &snapshot.portal_config.value)
        else {
            return Vec::new();
        };
        routes
            .iter()
            .filter(|route| {
                !config
                    .preferences
                    .iter()
                    .any(|p| p.interface == route.interface)
            })
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
