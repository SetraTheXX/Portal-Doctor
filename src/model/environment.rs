use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Operating-system identity parsed from `/etc/os-release`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub pretty_name: Option<String>,
    pub version_id: Option<String>,
}

/// Graphical session type reported via `XDG_SESSION_TYPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Wayland,
    X11,
}

impl SessionType {
    /// Parse a raw `XDG_SESSION_TYPE` value; unknown values yield `None`.
    #[must_use]
    pub fn from_raw(raw: &str) -> Option<Self> {
        match raw {
            "wayland" => Some(Self::Wayland),
            "x11" => Some(Self::X11),
            _ => None,
        }
    }

    /// Stable lowercase label.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wayland => "wayland",
            Self::X11 => "x11",
        }
    }
}

/// Desktop/session context discovered from allowlisted environment variables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Raw `XDG_CURRENT_DESKTOP`; colon-separated desktop names are preserved.
    pub current_desktop: Option<String>,
    /// Raw `XDG_SESSION_DESKTOP`.
    pub session_desktop: Option<String>,
    /// Parsed `XDG_SESSION_TYPE`; `None` when absent or unrecognized.
    pub session_type: Option<SessionType>,
    /// Raw session-type value, retained when it does not parse.
    pub session_type_raw: Option<String>,
    pub wayland_display: Option<String>,
    pub display: Option<String>,
}

/// Effective `XDG` search roots in precedence order (`XDG` base directory spec).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRoots {
    pub config_roots: Vec<String>,
    pub data_roots: Vec<String>,
}

/// Relation between the process/session value and the `systemd` user
/// activation value for one allowlisted variable (architecture §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentRelation {
    Equal,
    MissingProcess,
    MissingActivation,
    Different,
    /// Contract variant from architecture §7; produced once per-value
    /// comparisons gain skip conditions.
    #[allow(dead_code)]
    NotChecked,
}

impl EnvironmentRelation {
    /// Human-readable label used by renderers.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Equal => "equal",
            Self::MissingProcess => "missing in process environment",
            Self::MissingActivation => "missing in activation environment",
            Self::Different => "different values",
            Self::NotChecked => "not checked",
        }
    }
}

/// Per-variable comparison entry (architecture §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentValue {
    pub key: String,
    pub process_value: Option<String>,
    pub activation_value: Option<String>,
    pub relation: EnvironmentRelation,
}

/// Comparison result between the two environments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentComparison {
    /// `false` when the activation environment was not collectable.
    pub performed: bool,
    pub entries: Vec<EnvironmentValue>,
}

/// Collected desktop/environment state for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    /// Allowlisted variables observed in the process environment.
    pub process: BTreeMap<String, String>,
    pub search_roots: SearchRoots,
    pub activation_comparison: EnvironmentComparison,
}
