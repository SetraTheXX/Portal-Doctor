use serde::Serialize;

use crate::model::dbus::DbusInfo;
use crate::model::environment::{EnvironmentInfo, SessionInfo, SystemInfo};
use crate::model::portal::{PortalBackend, PortalConfigInfo, PortalRoute};
use crate::model::section::Section;
use crate::model::service::ServiceInfo;

/// Version of the normalized snapshot schema (architecture §6).
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Normalized snapshot: the single internal state collected during a run.
/// Rules consume this snapshot only (architecture §15 rule purity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    /// Snapshot schema version.
    pub schema_version: u32,
    /// Collection start time as Unix epoch milliseconds.
    pub collected_at: u64,
    /// Operating-system identity from `/etc/os-release`.
    pub system: Section<SystemInfo>,
    /// Desktop/session context from allowlisted variables.
    pub session: Section<SessionInfo>,
    /// Process environment, search roots and activation comparison.
    pub environment: Section<EnvironmentInfo>,
    /// Parsed `portals.conf` state for the current desktop.
    pub portal_config: Section<PortalConfigInfo>,
    /// Discovered `.portal` backend descriptors.
    pub portal_backends: Section<Vec<PortalBackend>>,
    /// Resolved portal route table.
    pub portal_routes: Section<Vec<PortalRoute>>,
    /// Runtime D-Bus reachability checks.
    pub dbus: Section<DbusInfo>,
    /// Portal-relevant systemd user unit states.
    pub services: Section<ServiceInfo>,
}

impl Snapshot {
    /// Start a snapshot for `collected_at`; every section starts as
    /// `Unsupported` until its collector fills it in.
    #[must_use]
    pub fn new(collected_at: u64) -> Self {
        Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            collected_at,
            system: Section::unsupported("not collected"),
            session: Section::unsupported("not collected"),
            environment: Section::unsupported("not collected"),
            portal_config: Section::unsupported("not collected"),
            portal_backends: Section::unsupported("not collected"),
            portal_routes: Section::unsupported("not collected"),
            dbus: Section::unsupported("not collected"),
            services: Section::unsupported("not collected"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SNAPSHOT_SCHEMA_VERSION, Snapshot};
    use crate::model::dbus::DbusInfo;
    use crate::model::environment::SystemInfo;
    use crate::model::section::Section;
    use crate::model::service::ServiceInfo;
    use serde_json::json;

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SNAPSHOT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn serializes_schema_time_and_sections() {
        let mut snapshot = Snapshot::new(42);
        snapshot.system = Section::available(SystemInfo {
            id: Some("ubuntu".to_owned()),
            name: None,
            pretty_name: None,
            version_id: None,
        });
        snapshot.dbus = Section::available(DbusInfo {
            connected: true,
            checks: Vec::new(),
        });
        snapshot.services = Section::available(ServiceInfo { units: Vec::new() });
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["schema_version"], json!(1));
        assert_eq!(value["collected_at"], json!(42));
        assert_eq!(value["system"]["status"], json!("available"));
        assert_eq!(value["session"]["status"], json!("unsupported"));
        assert_eq!(value["dbus"]["status"], json!("available"));
        assert_eq!(value["services"]["status"], json!("available"));
    }
}
