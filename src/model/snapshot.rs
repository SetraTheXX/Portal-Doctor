use serde::{Deserialize, Serialize, Serializer};

use crate::model::dbus::DbusInfo;
use crate::model::environment::{EnvironmentInfo, SessionInfo, SystemInfo};
use crate::model::journal::JournalInfo;
use crate::model::pipewire::{PipeWireInfo, WirePlumberInfo};
use crate::model::portal::{PortalBackend, PortalConfigInfo, PortalRoute};
use crate::model::portal_frontend::PortalFrontendInfo;
use crate::model::section::Section;
use crate::model::service::ServiceInfo;

/// Version of the normalized snapshot schema (architecture §6).
pub const PUBLIC_JSON_SCHEMA_VERSION: u32 = 1;

/// Version of the normalized snapshot schema (architecture §6).
pub const SNAPSHOT_SCHEMA_VERSION: u32 = PUBLIC_JSON_SCHEMA_VERSION;

/// Serialize a public schema version only when it is the supported version.
///
/// Keeping this guard at the serialization boundary prevents an accidentally
/// mutated in-memory model from emitting a document with an unsupported
/// version number.
#[allow(clippy::trivially_copy_pass_by_ref)] // serde's serialize_with callback receives &T
pub(crate) fn serialize_public_schema_version<S>(
    value: &u32,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if *value != PUBLIC_JSON_SCHEMA_VERSION {
        return Err(serde::ser::Error::custom(format!(
            "unsupported public JSON schema version {value}"
        )));
    }
    serializer.serialize_u32(*value)
}

#[derive(Debug, Deserialize)]
struct SnapshotWire {
    schema_version: u32,
    collected_at: u64,
    system: Section<SystemInfo>,
    session: Section<SessionInfo>,
    environment: Section<EnvironmentInfo>,
    portal_config: Section<PortalConfigInfo>,
    portal_backends: Section<Vec<PortalBackend>>,
    portal_routes: Section<Vec<PortalRoute>>,
    portal_frontend: Section<PortalFrontendInfo>,
    dbus: Section<DbusInfo>,
    services: Section<ServiceInfo>,
    pipewire: Section<PipeWireInfo>,
    wireplumber: Section<WirePlumberInfo>,
    journal: Section<JournalInfo>,
}

/// Normalized snapshot: the single internal state collected during a run.
/// Rules consume this snapshot only (architecture §15 rule purity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SnapshotWire")]
pub struct Snapshot {
    /// Snapshot schema version.
    #[serde(serialize_with = "serialize_public_schema_version")]
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
    /// xdg-desktop-portal frontend software-version evidence, kept separate
    /// from the operating-system `system.version_id` field.
    pub portal_frontend: Section<PortalFrontendInfo>,
    /// Runtime D-Bus reachability checks.
    pub dbus: Section<DbusInfo>,
    /// Portal-relevant systemd user unit states.
    pub services: Section<ServiceInfo>,
    /// Normalized `PipeWire` graph facts.
    pub pipewire: Section<PipeWireInfo>,
    /// Normalized `WirePlumber` reachability facts.
    pub wireplumber: Section<WirePlumberInfo>,
    /// Optional sanitized current-boot/user-session journal evidence.
    pub journal: Section<JournalInfo>,
}

impl TryFrom<SnapshotWire> for Snapshot {
    type Error = String;

    fn try_from(wire: SnapshotWire) -> Result<Self, Self::Error> {
        if wire.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported snapshot schema version {}; expected {}",
                wire.schema_version, SNAPSHOT_SCHEMA_VERSION
            ));
        }

        Ok(Self {
            schema_version: wire.schema_version,
            collected_at: wire.collected_at,
            system: wire.system,
            session: wire.session,
            environment: wire.environment,
            portal_config: wire.portal_config,
            portal_backends: wire.portal_backends,
            portal_routes: wire.portal_routes,
            portal_frontend: wire.portal_frontend,
            dbus: wire.dbus,
            services: wire.services,
            pipewire: wire.pipewire,
            wireplumber: wire.wireplumber,
            journal: wire.journal,
        })
    }
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
            portal_frontend: Section::unsupported("not collected"),
            dbus: Section::unsupported("not collected"),
            services: Section::unsupported("not collected"),
            pipewire: Section::unsupported("not collected"),
            wireplumber: Section::unsupported("not collected"),
            journal: Section::unsupported("not collected"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{PUBLIC_JSON_SCHEMA_VERSION, SNAPSHOT_SCHEMA_VERSION, Snapshot};
    use crate::model::dbus::DbusInfo;
    use crate::model::environment::SystemInfo;
    use crate::model::section::Section;
    use crate::model::service::ServiceInfo;
    use serde_json::json;

    #[test]
    fn schema_version_is_canonical_and_one() {
        assert_eq!(PUBLIC_JSON_SCHEMA_VERSION, 1);
        assert_eq!(SNAPSHOT_SCHEMA_VERSION, PUBLIC_JSON_SCHEMA_VERSION);
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
        assert_eq!(value["schema_version"], json!(PUBLIC_JSON_SCHEMA_VERSION));
        assert!(value["schema_version"].is_u64());
        assert_eq!(value["collected_at"], json!(42));
        assert_eq!(value["system"]["status"], json!("available"));
        assert_eq!(value["session"]["status"], json!("unsupported"));
        assert_eq!(value["dbus"]["status"], json!("available"));
        assert_eq!(value["services"]["status"], json!("available"));
        assert_eq!(value["journal"]["status"], json!("unsupported"));
        assert_eq!(value["portal_frontend"]["status"], json!("unsupported"));

        let fields = value
            .as_object()
            .expect("snapshot serializes as an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = [
            "schema_version",
            "collected_at",
            "system",
            "session",
            "environment",
            "portal_config",
            "portal_backends",
            "portal_routes",
            "portal_frontend",
            "dbus",
            "services",
            "pipewire",
            "wireplumber",
            "journal",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(fields, expected);
    }

    #[test]
    fn snapshot_schema_rejects_missing_wrong_type_and_wrong_version() {
        let base = serde_json::to_value(Snapshot::new(42)).unwrap();

        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove("schema_version");
        assert!(serde_json::from_value::<Snapshot>(missing).is_err());

        let mut wrong_type = base.clone();
        wrong_type["schema_version"] = json!("1");
        assert!(serde_json::from_value::<Snapshot>(wrong_type).is_err());

        let mut wrong_version = base;
        wrong_version["schema_version"] = json!(2);
        assert!(serde_json::from_value::<Snapshot>(wrong_version).is_err());
    }

    #[test]
    fn snapshot_serialization_rejects_mutated_schema_version() {
        let mut snapshot = Snapshot::new(42);
        snapshot.schema_version = PUBLIC_JSON_SCHEMA_VERSION + 1;
        assert!(serde_json::to_value(snapshot).is_err());
    }

    #[test]
    fn snapshot_schema_accepts_additive_unknown_fields() {
        let mut value = serde_json::to_value(Snapshot::new(42)).unwrap();
        value["future_optional_field"] = json!(true);
        assert!(serde_json::from_value::<Snapshot>(value).is_ok());
    }
}
