use serde::{Deserialize, Serialize};

/// Runtime state of one systemd user unit (architecture §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitStatus {
    pub unit: String,
    pub state: UnitState,
    /// `SubState` property (e.g. `running`, `dead`).
    pub sub_state: Option<String>,
    /// `UnitFileState` property (e.g. `static`, `enabled`).
    pub unit_file_state: Option<String>,
}

/// Coarse load/active state of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitState {
    Active,
    Activating,
    Deactivating,
    Inactive,
    Failed,
    /// The unit does not exist on this system.
    NotFound,
    Unreadable,
}

impl UnitState {
    /// Parse the `ActiveState` property value; unknown values stay readable.
    #[must_use]
    pub fn parse_active_state(value: &str) -> Self {
        match value {
            "active" => Self::Active,
            "activating" => Self::Activating,
            "deactivating" => Self::Deactivating,
            "inactive" => Self::Inactive,
            "failed" => Self::Failed,
            _ => Self::Unreadable,
        }
    }

    /// Human-readable label used by renderers.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Activating => "activating",
            Self::Deactivating => "deactivating",
            Self::Inactive => "inactive",
            Self::Failed => "failed",
            Self::NotFound => "not found",
            Self::Unreadable => "unreadable",
        }
    }
}

/// Collected systemd user-service state for portal-relevant units
/// (architecture §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub units: Vec<UnitStatus>,
}

impl ServiceInfo {
    /// Look up one unit by name.
    #[must_use]
    pub fn unit(&self, name: &str) -> Option<&UnitStatus> {
        self.units.iter().find(|unit| unit.unit == name)
    }

    /// The frontend unit name (`xdg-desktop-portal.service`).
    #[must_use]
    pub fn frontend_unit() -> &'static str {
        "xdg-desktop-portal.service"
    }

    /// Best-effort unit name for a discovered backend id
    /// (`xdg-desktop-portal-<id>.service`).
    #[must_use]
    pub fn backend_unit(backend_id: &str) -> String {
        format!("xdg-desktop-portal-{backend_id}.service")
    }
}

#[cfg(test)]
mod tests {
    use super::{ServiceInfo, UnitState, UnitStatus};
    use serde_json::json;

    #[test]
    fn parses_known_active_states() {
        assert_eq!(UnitState::parse_active_state("active"), UnitState::Active);
        assert_eq!(UnitState::parse_active_state("failed"), UnitState::Failed);
        assert_eq!(
            UnitState::parse_active_state("whatever"),
            UnitState::Unreadable
        );
    }

    #[test]
    fn serializes_unit_status() {
        let unit = UnitStatus {
            unit: "xdg-desktop-portal.service".to_owned(),
            state: UnitState::Active,
            sub_state: Some("running".to_owned()),
            unit_file_state: Some("static".to_owned()),
        };
        let value = serde_json::to_value(&unit).unwrap();
        assert_eq!(value["state"], json!("active"));
        assert_eq!(value["sub_state"], json!("running"));
    }

    #[test]
    fn backend_unit_naming_follows_convention() {
        let unit = UnitStatus {
            unit: "xdg-desktop-portal.service".to_owned(),
            state: UnitState::Active,
            sub_state: None,
            unit_file_state: None,
        };
        assert_eq!(ServiceInfo::frontend_unit(), "xdg-desktop-portal.service");
        assert_eq!(
            ServiceInfo::backend_unit("gnome"),
            "xdg-desktop-portal-gnome.service"
        );
        let info = ServiceInfo { units: vec![unit] };
        assert!(info.unit("xdg-desktop-portal.service").is_some());
    }
}
