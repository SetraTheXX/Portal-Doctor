use std::fmt;

use serde::Serialize;

/// Structured evidence attached to findings (architecture §15).
///
/// Variant payloads are finalized together with the Phase 1 rule engine;
/// phase 0 defines the variant set only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // constructed by the Phase 1 rule engine
pub enum Evidence {
    EnvironmentMismatch,
    ConfigSelection,
    MissingProvider,
    DbusTimeout,
    ServiceState,
    JournalExcerpt,
}

impl Evidence {
    /// Short human label used by renderers.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EnvironmentMismatch => "environment mismatch",
            Self::ConfigSelection => "config selection",
            Self::MissingProvider => "missing provider",
            Self::DbusTimeout => "dbus timeout",
            Self::ServiceState => "service state",
            Self::JournalExcerpt => "journal excerpt",
        }
    }
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::Evidence;
    use serde_json::json;

    #[test]
    fn serializes_as_snake_case_strings() {
        assert_eq!(
            serde_json::to_value(Evidence::EnvironmentMismatch).unwrap(),
            json!("environment_mismatch")
        );
        assert_eq!(
            serde_json::to_value(Evidence::DbusTimeout).unwrap(),
            json!("dbus_timeout")
        );
    }

    #[test]
    fn display_renders_human_text() {
        assert_eq!(Evidence::MissingProvider.to_string(), "missing provider");
    }
}
