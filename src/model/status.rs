use std::fmt;

use serde::{Deserialize, Serialize};

/// Outcome of a collector execution (architecture §3.3).
///
/// This status is shared by every collector and remains explicit when a
/// dependency is unavailable, blocked or returns data that cannot be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // constructed by collectors
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
