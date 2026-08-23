use std::fmt;

use serde::Serialize;

/// Outcome of a collector execution (architecture §3.3).
///
/// Collectors run from Phase 1 on; this anchors the collection-status
/// contract until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // constructed by Phase 1 collectors
pub enum CollectorState {
    Available,
    Unavailable,
    Unsupported,
    TimedOut,
    PermissionDenied,
    ParseError,
}

impl fmt::Display for CollectorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
            Self::TimedOut => "timed out",
            Self::PermissionDenied => "permission denied",
            Self::ParseError => "parse error",
        };
        f.write_str(label)
    }
}

#[cfg(test)]
mod tests {
    use super::CollectorState;
    use serde_json::json;

    #[test]
    fn serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_value(CollectorState::TimedOut).unwrap(),
            json!("timed_out")
        );
        assert_eq!(
            serde_json::to_value(CollectorState::PermissionDenied).unwrap(),
            json!("permission_denied")
        );
    }

    #[test]
    fn display_is_human_readable() {
        assert_eq!(CollectorState::ParseError.to_string(), "parse error");
    }
}
