use serde::Serialize;

/// Version of the normalized snapshot schema (architecture §6).
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Normalized snapshot: the single internal state collected during a run.
///
/// Phase 1 collectors add the system/session/environment sections; phase 0
/// carries the schema anchor and collection time so the v1 pipeline is
/// exercised end to end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    /// Snapshot schema version.
    pub schema_version: u32,
    /// Collection start time as Unix epoch milliseconds.
    pub collected_at: u64,
}

impl Snapshot {
    /// Create a snapshot with the given schema version and collection time.
    #[must_use]
    pub fn new(schema_version: u32, collected_at: u64) -> Self {
        Self {
            schema_version,
            collected_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SNAPSHOT_SCHEMA_VERSION, Snapshot};
    use serde_json::json;

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SNAPSHOT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn serializes_schema_and_time_keys() {
        let snapshot = Snapshot::new(SNAPSHOT_SCHEMA_VERSION, 42);
        let value = serde_json::to_value(snapshot).unwrap();
        assert_eq!(value, json!({ "schema_version": 1, "collected_at": 42 }));
    }
}
