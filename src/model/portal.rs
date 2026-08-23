use std::collections::BTreeSet;

use serde::Serialize;

/// A portal backend descriptor parsed from a `.portal` file (architecture §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortalBackend {
    /// Backend identifier; the `.portal` file stem.
    pub id: String,
    /// Descriptor path that won discovery precedence.
    pub descriptor_path: String,
    /// Descriptor paths that defined the same id at lower precedence
    /// (provenance for duplicate handling).
    pub duplicate_descriptors: Vec<String>,
    pub dbus_name: String,
    pub interfaces: BTreeSet<String>,
    /// Legacy `UseIn` desktop restrictions; empty means unrestricted.
    pub legacy_use_in: Vec<String>,
}

/// One parsed preference from the selected `portals.conf` (architecture §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortalPreference {
    pub interface: String,
    /// Ordered backend tokens; may include `*` or `none`.
    pub backends: Vec<String>,
    pub source_file: String,
    pub source_priority: usize,
}

/// Parsed portal configuration state (architecture §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortalConfigInfo {
    /// Every candidate file in precedence order (desktop-specific first).
    pub candidate_files: Vec<String>,
    /// The candidate that existed and was parsed, if any.
    pub selected_file: Option<String>,
    pub preferences: Vec<PortalPreference>,
    pub parse_errors: Vec<String>,
}

/// Outcome of route resolution for one portal interface (architecture §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortalRoute {
    pub interface: String,
    /// Backend tokens requested by configuration; empty means "any".
    pub requested_candidates: Vec<String>,
    /// Backends discovered with the interface and allowed by `UseIn`.
    pub available_candidates: Vec<String>,
    /// Backends actually selected for this desktop.
    pub selected_candidates: Vec<String>,
    pub evidence: Vec<RouteEvidence>,
    pub status: RouteStatus,
}

/// One provenance step of a resolved route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteEvidence {
    pub message: String,
}

/// Why a route ended in its current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    /// At least one backend is selected.
    Selected,
    /// Explicitly disabled via `none`.
    Disabled,
    /// No backend can serve the interface in this desktop context.
    NoProvider,
}

impl RouteStatus {
    /// Human-readable label used by renderers.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Disabled => "disabled",
            Self::NoProvider => "no provider",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RouteStatus;
    use serde_json::json;

    #[test]
    fn route_status_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_value(RouteStatus::NoProvider).unwrap(),
            json!("no_provider")
        );
    }

    #[test]
    fn route_status_labels_are_human_readable() {
        assert_eq!(RouteStatus::Disabled.as_str(), "disabled");
        assert_eq!(RouteStatus::Selected.as_str(), "selected");
    }
}
