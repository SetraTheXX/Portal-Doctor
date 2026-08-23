use std::collections::BTreeMap;
use std::env;

use crate::model::environment::{
    EnvironmentComparison, EnvironmentInfo, EnvironmentRelation, EnvironmentValue, SearchRoots,
    SessionInfo, SessionType,
};

/// Variables `PortalDoctor` is allowed to read and report (architecture §7).
/// Anything outside this list is never collected.
pub const ALLOWLISTED_VARIABLES: &[&str] = &[
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_DESKTOP",
    "XDG_SESSION_TYPE",
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "XDG_CONFIG_HOME",
    "XDG_CONFIG_DIRS",
    "XDG_DATA_HOME",
    "XDG_DATA_DIRS",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_RUNTIME_DIR",
];

/// Keys whose process/activation mismatch is diagnostically relevant
/// (ENV004). Purely path-like configuration roots are excluded.
pub const COMPARISON_KEYS: &[&str] = &[
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_DESKTOP",
    "XDG_SESSION_TYPE",
    "WAYLAND_DISPLAY",
    "DISPLAY",
];

/// Read the allowlisted variables from the current process environment.
/// Absent and empty values are treated the same: not collected.
#[must_use]
pub fn collect_process_environment() -> BTreeMap<String, String> {
    ALLOWLISTED_VARIABLES
        .iter()
        .filter_map(|key| {
            env::var(key)
                .ok()
                .filter(|value| !value.is_empty())
                .map(|value| ((*key).to_owned(), value))
        })
        .collect()
}

/// Derive the desktop/session context from collected variables.
#[must_use]
pub fn session_info(vars: &BTreeMap<String, String>) -> SessionInfo {
    let get = |key| vars.get(key).cloned();
    let session_type_raw = get("XDG_SESSION_TYPE");
    let session_type = session_type_raw.as_deref().and_then(SessionType::from_raw);
    SessionInfo {
        current_desktop: get("XDG_CURRENT_DESKTOP"),
        session_desktop: get("XDG_SESSION_DESKTOP"),
        session_type,
        session_type_raw,
        wayland_display: get("WAYLAND_DISPLAY"),
        display: get("DISPLAY"),
    }
}

/// Compute effective `XDG` search roots in precedence order. Defaults follow
/// the `XDG` base directory specification; `home` feeds only those defaults
/// and is never reported as its own variable (architecture §7).
#[must_use]
pub fn search_roots(vars: &BTreeMap<String, String>, home: Option<&str>) -> SearchRoots {
    let default_of = |suffix: &str| home.map(|home| format!("{home}{suffix}"));

    let mut config_roots = Vec::new();
    if let Some(root) = vars
        .get("XDG_CONFIG_HOME")
        .cloned()
        .or_else(|| default_of("/.config"))
    {
        config_roots.push(root);
    }
    for dir in colon_list_or_default(vars.get("XDG_CONFIG_DIRS"), &["/etc/xdg"]) {
        if !config_roots.contains(&dir) {
            config_roots.push(dir);
        }
    }

    let mut data_roots = Vec::new();
    if let Some(root) = vars
        .get("XDG_DATA_HOME")
        .cloned()
        .or_else(|| default_of("/.local/share"))
    {
        data_roots.push(root);
    }
    for dir in colon_list_or_default(
        vars.get("XDG_DATA_DIRS"),
        &["/usr/local/share", "/usr/share"],
    ) {
        if !data_roots.contains(&dir) {
            data_roots.push(dir);
        }
    }

    SearchRoots {
        config_roots,
        data_roots,
    }
}

/// Split a colon-separated directory list; an absent or empty value falls
/// back to the given spec defaults (empty elements are ignored).
fn colon_list_or_default(value: Option<&String>, defaults: &[&str]) -> Vec<String> {
    match value.filter(|list| !list.is_empty()) {
        Some(list) => list
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(str::to_owned)
            .collect(),
        None => defaults.iter().map(|dir| (*dir).to_owned()).collect(),
    }
}

/// Compare the two environments over [`COMPARISON_KEYS`]. When the activation
/// map is `None`, nothing is compared (`performed == false`).
#[must_use]
pub fn compare_environments(
    process: &BTreeMap<String, String>,
    activation: Option<&BTreeMap<String, String>>,
) -> EnvironmentComparison {
    let Some(activation) = activation else {
        return EnvironmentComparison {
            performed: false,
            entries: Vec::new(),
        };
    };
    let entries = COMPARISON_KEYS
        .iter()
        .filter_map(|key| {
            let process_value = process.get(*key).cloned();
            let activation_value = activation.get(*key).cloned();
            let relation = match (&process_value, &activation_value) {
                (Some(p), Some(a)) if p == a => EnvironmentRelation::Equal,
                (Some(_), Some(_)) => EnvironmentRelation::Different,
                (Some(_), None) => EnvironmentRelation::MissingActivation,
                (None, Some(_)) => EnvironmentRelation::MissingProcess,
                (None, None) => return None,
            };
            Some(EnvironmentValue {
                key: (*key).to_owned(),
                process_value,
                activation_value,
                relation,
            })
        })
        .collect();
    EnvironmentComparison {
        performed: true,
        entries,
    }
}

/// Assemble the environment section data from collected parts.
#[must_use]
pub fn environment_info(
    process: BTreeMap<String, String>,
    home: Option<&str>,
    activation: Option<&BTreeMap<String, String>>,
) -> EnvironmentInfo {
    let search_roots = search_roots(&process, home);
    let activation_comparison = compare_environments(&process, activation);
    EnvironmentInfo {
        process,
        search_roots,
        activation_comparison,
    }
}

#[cfg(test)]
mod tests {
    use super::{compare_environments, environment_info, search_roots, session_info};
    use crate::model::environment::{EnvironmentRelation, SessionType};
    use std::collections::BTreeMap;

    fn vars(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn session_info_parses_known_session_types() {
        let v = vars(&[
            ("XDG_CURRENT_DESKTOP", "ubuntu:GNOME"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ]);
        let info = session_info(&v);
        assert_eq!(info.current_desktop.as_deref(), Some("ubuntu:GNOME"));
        assert_eq!(info.session_type, Some(SessionType::Wayland));
        assert_eq!(info.wayland_display.as_deref(), Some("wayland-0"));
    }

    #[test]
    fn unknown_session_type_keeps_raw_value() {
        let v = vars(&[("XDG_SESSION_TYPE", "mir")]);
        let info = session_info(&v);
        assert_eq!(info.session_type, None);
        assert_eq!(info.session_type_raw.as_deref(), Some("mir"));
    }

    #[test]
    fn search_roots_follow_spec_defaults_without_xdg_overrides() {
        let roots = search_roots(&vars(&[]), Some("/home/tester"));
        assert_eq!(roots.config_roots, ["/home/tester/.config", "/etc/xdg"]);
        assert_eq!(
            roots.data_roots,
            [
                "/home/tester/.local/share",
                "/usr/local/share",
                "/usr/share"
            ]
        );
    }

    #[test]
    fn search_roots_without_home_skip_home_derived_defaults() {
        let roots = search_roots(&vars(&[]), None);
        assert_eq!(roots.config_roots, ["/etc/xdg"]);
        assert_eq!(roots.data_roots, ["/usr/local/share", "/usr/share"]);
    }

    #[test]
    fn search_roots_honor_explicit_overrides_and_skip_empty_entries() {
        let v = vars(&[
            ("XDG_CONFIG_HOME", "/cfg/home"),
            ("XDG_CONFIG_DIRS", "/cfg/global1::/cfg/global2"),
            ("XDG_DATA_HOME", "/data/home"),
            ("XDG_DATA_DIRS", ""),
        ]);
        let roots = search_roots(&v, Some("/home/tester"));
        assert_eq!(
            roots.config_roots,
            ["/cfg/home", "/cfg/global1", "/cfg/global2"]
        );
        // Empty XDG_DATA_DIRS falls back to the spec default list.
        assert_eq!(
            roots.data_roots,
            ["/data/home", "/usr/local/share", "/usr/share"]
        );
    }

    #[test]
    fn comparison_reports_relations_per_key() {
        let process = vars(&[
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_TYPE", "wayland"),
        ]);
        let activation = vars(&[
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("DISPLAY", ":0"),
        ]);
        let comparison = compare_environments(&process, Some(&activation));
        assert!(comparison.performed);
        let by_key = |k: &str| comparison.entries.iter().find(|e| e.key == k).unwrap();
        assert_eq!(
            by_key("XDG_CURRENT_DESKTOP").relation,
            EnvironmentRelation::Equal
        );
        assert_eq!(
            by_key("XDG_SESSION_TYPE").relation,
            EnvironmentRelation::MissingActivation
        );
        assert_eq!(
            by_key("WAYLAND_DISPLAY").relation,
            EnvironmentRelation::MissingProcess
        );
        assert_eq!(
            by_key("DISPLAY").relation,
            EnvironmentRelation::MissingProcess
        );
    }

    #[test]
    fn comparison_not_performed_without_activation_map() {
        let process = vars(&[("XDG_CURRENT_DESKTOP", "GNOME")]);
        let comparison = compare_environments(&process, None);
        assert!(!comparison.performed);
        assert!(comparison.entries.is_empty());
    }

    #[test]
    fn environment_info_assembles_all_parts() {
        let process = vars(&[("XDG_CURRENT_DESKTOP", "GNOME")]);
        let info = environment_info(process.clone(), Some("/h"), Some(&process));
        assert!(info.activation_comparison.performed);
        assert_eq!(info.process, process);
        assert_eq!(info.search_roots.config_roots, ["/h/.config", "/etc/xdg"]);

        let without_activation = environment_info(process, Some("/h"), None);
        assert!(!without_activation.activation_comparison.performed);
    }

    #[test]
    fn model_session_type_covers_x11() {
        assert_eq!(SessionType::from_raw("x11"), Some(SessionType::X11));
        assert_eq!(SessionType::from_raw("tty"), None);
    }
}
