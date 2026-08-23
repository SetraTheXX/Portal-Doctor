use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::model::environment::SearchRoots;
use crate::model::portal::PortalBackend;
use crate::model::section::Section;
use crate::resolver::search_paths;

/// Parse one `.portal` descriptor. Missing keys stay empty; unknown sections
/// and lines are ignored.
#[must_use]
pub fn parse_portal_file(text: &str, descriptor_path: &str, id: String) -> PortalBackend {
    let mut dbus_name = String::new();
    let mut interfaces = BTreeSet::new();
    let mut legacy_use_in = Vec::new();
    let mut in_portal_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            in_portal_section =
                line.trim_start_matches('[').trim_end_matches(']').trim() == "portal";
            continue;
        }
        if !in_portal_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "DBusName" => value.clone_into(&mut dbus_name),
            "Interfaces" => interfaces.extend(split_tokens(value)),
            "UseIn" => legacy_use_in = split_tokens(value),
            _ => {}
        }
    }
    PortalBackend {
        id,
        descriptor_path: descriptor_path.to_owned(),
        duplicate_descriptors: Vec::new(),
        dbus_name,
        interfaces,
        legacy_use_in,
    }
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

/// Discover `.portal` descriptors across the effective data roots. Identical
/// backend ids keep the highest-precedence descriptor and record the losing
/// paths as duplicate provenance.
pub fn collect(roots: &SearchRoots) -> Section<Vec<PortalBackend>> {
    let mut by_id: BTreeMap<String, PortalBackend> = BTreeMap::new();
    let mut duplicates: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for root in search_paths::portal_descriptor_roots(roots) {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|e| e == "portal"))
            .collect();
        files.sort_by_key(std::fs::DirEntry::file_name);
        for entry in files {
            let path = entry.path();
            let id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(id) if !id.is_empty() => id.to_owned(),
                _ => continue,
            };
            if let Ok(text) = fs::read_to_string(&path) {
                if by_id.contains_key(&id) {
                    duplicates
                        .entry(id.clone())
                        .or_default()
                        .push(path.to_string_lossy().into_owned());
                } else {
                    by_id.insert(
                        id.clone(),
                        parse_portal_file(&text, &path.to_string_lossy(), id),
                    );
                }
            }
        }
    }
    let mut backends: Vec<PortalBackend> = by_id.into_values().collect();
    for backend in &mut backends {
        if let Some(dups) = duplicates.remove(&backend.id) {
            backend.duplicate_descriptors = dups;
        }
    }
    backends.sort_by(|a, b| a.id.cmp(&b.id));
    Section::available(backends)
}

#[cfg(test)]
mod tests {
    use super::{parse_portal_file, split_tokens};

    #[test]
    fn parses_descriptor_fields_and_lists() {
        let text = "[portal]\n\
                    DBusName=org.freedesktop.impl.portal.desktop.gnome\n\
                    Interfaces=org.freedesktop.impl.portal.Screenshot;org.freedesktop.impl.portal.ScreenCast\n\
                    UseIn=gnome\n";
        let backend = parse_portal_file(
            text,
            "/usr/share/xdg-desktop-portal/portals/gnome.portal",
            "gnome".to_owned(),
        );
        assert_eq!(backend.id, "gnome");
        assert_eq!(
            backend.dbus_name,
            "org.freedesktop.impl.portal.desktop.gnome"
        );
        assert!(
            backend
                .interfaces
                .contains("org.freedesktop.impl.portal.Screenshot")
        );
        assert!(
            backend
                .interfaces
                .contains("org.freedesktop.impl.portal.ScreenCast")
        );
        assert_eq!(backend.legacy_use_in, ["gnome"]);
    }

    #[test]
    fn tolerates_missing_keys_and_foreign_sections() {
        let text = "[other]\nkey=value\n[portal]\nDBusName=x\n";
        let backend = parse_portal_file(text, "p.portal", "p".to_owned());
        assert_eq!(backend.dbus_name, "x");
        assert!(backend.interfaces.is_empty());
        assert!(backend.legacy_use_in.is_empty());
    }

    #[test]
    fn split_tokens_handles_mixed_separators() {
        assert_eq!(split_tokens("a;b, c"), ["a", "b", "c"]);
        assert!(split_tokens("").is_empty());
    }
}
