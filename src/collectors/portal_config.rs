use std::fs;

use crate::model::environment::SearchRoots;
use crate::model::portal::{PortalConfigInfo, PortalPreference};
use crate::model::section::Section;
use crate::resolver::search_paths;

/// Parse `portals.conf` text: a GLib-style key file. Only the `[preferred]`
/// section is interpreted; malformed lines are reported as parse errors.
/// `*` and `none` are preserved as literal backend tokens.
#[must_use]
pub fn parse_config(
    text: &str,
    source_file: &str,
    source_priority: usize,
) -> (Vec<PortalPreference>, Vec<String>) {
    let mut preferences = Vec::new();
    let mut errors = Vec::new();
    let mut section = String::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            line.trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .clone_into(&mut section);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            errors.push(format!("line {}: missing '='", index + 1));
            continue;
        };
        if section != "preferred" {
            continue;
        }
        let key = key.trim();
        let backends = split_tokens(value.trim());
        if backends.is_empty() {
            errors.push(format!("line {}: empty backend list for {key}", index + 1));
            continue;
        }
        preferences.push(PortalPreference {
            interface: key.to_owned(),
            backends,
            source_file: source_file.to_owned(),
            source_priority,
        });
    }
    (preferences, errors)
}

/// Split a GLib-style list value on `;` or `,`, trimming and dropping empties.
fn split_tokens(value: &str) -> Vec<String> {
    value
        .split([';', ','])
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Discover and parse the effective `portals.conf` for the current desktop.
pub fn collect(roots: &SearchRoots, desktops: &[String]) -> Section<PortalConfigInfo> {
    let candidates = search_paths::portal_config_candidates(roots, desktops);
    let candidate_files: Vec<String> = candidates
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let Some(selected) = candidates.iter().position(|p| p.is_file()) else {
        return Section::available(PortalConfigInfo {
            candidate_files,
            selected_file: None,
            preferences: Vec::new(),
            parse_errors: Vec::new(),
        });
    };
    let path = &candidates[selected];
    match fs::read_to_string(path) {
        Ok(text) => {
            let (preferences, parse_errors) =
                parse_config(&text, &path.to_string_lossy(), selected);
            Section::available(PortalConfigInfo {
                candidate_files,
                selected_file: Some(path.to_string_lossy().into_owned()),
                preferences,
                parse_errors,
            })
        }
        Err(err) => Section::unavailable(format!("cannot read {}: {err}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_config;

    #[test]
    fn parses_preferred_entries_with_lists() {
        let (prefs, errors) = parse_config(
            "# comment\n[preferred]\n\
             org.freedesktop.impl.portal.Screenshot=gnome;gtk\n\
             org.freedesktop.impl.portal.FileChooser=gtk\n",
            "/cfg/xdg-desktop-portal/portals.conf",
            1,
        );
        assert!(errors.is_empty());
        assert_eq!(prefs.len(), 2);
        assert_eq!(prefs[0].interface, "org.freedesktop.impl.portal.Screenshot");
        assert_eq!(prefs[0].backends, ["gnome", "gtk"]);
        assert_eq!(
            prefs[1].interface,
            "org.freedesktop.impl.portal.FileChooser"
        );
        assert_eq!(prefs[1].source_priority, 1);
    }

    #[test]
    fn preserves_star_and_none_tokens() {
        let (prefs, _) = parse_config(
            "[preferred]\n\
             org.freedesktop.impl.portal.Secret=*\n\
             org.freedesktop.impl.portal.Print=none\n",
            "portals.conf",
            0,
        );
        assert_eq!(prefs[0].backends, ["*"]);
        assert_eq!(prefs[1].backends, ["none"]);
    }

    #[test]
    fn tolerates_comma_and_space_separated_lists() {
        let (prefs, _) = parse_config(
            "[preferred]\norg.freedesktop.impl.portal.Screenshot=gnome, gtk\n",
            "portals.conf",
            0,
        );
        assert_eq!(prefs[0].backends, ["gnome", "gtk"]);
    }

    #[test]
    fn reports_malformed_lines_and_ignores_other_sections() {
        let (prefs, errors) = parse_config(
            "[other]\nkey=value\n[preferred]\nbroken-line\n\
             org.freedesktop.impl.portal.Screenshot=\n",
            "portals.conf",
            0,
        );
        assert!(prefs.is_empty());
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("missing '='"));
        assert!(errors[1].contains("empty backend list"));
    }
}
