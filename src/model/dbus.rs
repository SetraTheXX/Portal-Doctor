use serde::{Deserialize, Serialize};

/// Outcome of one D-Bus name check, following the architecture §11 failure
/// taxonomy (no session bus, name absent, activation failure, timeout,
/// access denied, malformed response are kept distinct).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DbusOutcome {
    /// The name exists and has an owner.
    HasOwner,
    /// The name exists but nobody owns it (not activated).
    NoOwner,
    /// No session bus could be reached at all.
    NoSessionBus,
    ActivationFailure,
    Timeout,
    AccessDenied,
    MalformedResponse,
    /// Failure outside the known taxonomy; the message is preserved.
    Other(String),
}

/// One probed D-Bus name (the portal frontend or a selected backend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbusCheck {
    pub name: String,
    pub outcome: DbusOutcome,
}

/// Runtime D-Bus state collected during a run (architecture §11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbusInfo {
    /// Whether a session bus connection was established.
    pub connected: bool,
    pub checks: Vec<DbusCheck>,
}

/// The XDG Desktop Portal frontend well-known name.
pub const PORTAL_FRONTEND_NAME: &str = "org.freedesktop.portal.Desktop";

#[cfg(test)]
mod tests {
    use super::{DbusCheck, DbusInfo, DbusOutcome};
    use serde_json::json;

    #[test]
    fn outcomes_serialize_as_snake_case() {
        assert_eq!(
            serde_json::to_value(DbusOutcome::NoOwner).unwrap(),
            json!("no_owner")
        );
        assert_eq!(
            serde_json::to_value(DbusOutcome::MalformedResponse).unwrap(),
            json!("malformed_response")
        );
    }

    #[test]
    fn dbus_info_serializes_checks() {
        let info = DbusInfo {
            connected: true,
            checks: vec![DbusCheck {
                name: "org.freedesktop.portal.Desktop".to_owned(),
                outcome: DbusOutcome::HasOwner,
            }],
        };
        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(value["connected"], json!(true));
        assert_eq!(value["checks"][0]["outcome"], json!("has_owner"));
    }
}
