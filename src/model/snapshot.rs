use serde::Serialize;

use crate::model::environment::{EnvironmentInfo, SessionInfo, SystemInfo};
use crate::model::portal::{PortalBackend, PortalConfigInfo, PortalRoute};
use crate::model::section::Section;

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
}

impl Snapshot {
    /// Assemble a snapshot with the current schema version and collected sections.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        collected_at: u64,
        system: Section<SystemInfo>,
        session: Section<SessionInfo>,
        environment: Section<EnvironmentInfo>,
        portal_config: Section<PortalConfigInfo>,
        portal_backends: Section<Vec<PortalBackend>>,
        portal_routes: Section<Vec<PortalRoute>>,
    ) -> Self {
        Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            collected_at,
            system,
            session,
            environment,
            portal_config,
            portal_backends,
            portal_routes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SNAPSHOT_SCHEMA_VERSION, Snapshot};
    use crate::model::environment::SystemInfo;
    use crate::model::section::Section;
    use serde_json::json;

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SNAPSHOT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn serializes_schema_time_and_sections() {
        let snapshot = Snapshot::new(
            42,
            Section::available(SystemInfo {
                id: Some("ubuntu".to_owned()),
                name: None,
                pretty_name: None,
                version_id: None,
            }),
            Section::<crate::model::environment::SessionInfo>::unsupported("test"),
            Section::<crate::model::environment::EnvironmentInfo>::unavailable("test"),
            Section::<crate::model::portal::PortalConfigInfo>::unsupported("test"),
            Section::<Vec<crate::model::portal::PortalBackend>>::unsupported("test"),
            Section::<Vec<crate::model::portal::PortalRoute>>::unsupported("test"),
        );
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["schema_version"], json!(1));
        assert_eq!(value["collected_at"], json!(42));
        assert_eq!(value["system"]["status"], json!("available"));
        assert_eq!(value["session"]["status"], json!("unsupported"));
        assert_eq!(value["environment"]["status"], json!("unavailable"));
        assert_eq!(value["portal_config"]["status"], json!("unsupported"));
        assert_eq!(value["portal_backends"]["status"], json!("unsupported"));
        assert_eq!(value["portal_routes"]["status"], json!("unsupported"));
    }
}
