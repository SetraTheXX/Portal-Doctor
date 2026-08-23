use std::collections::{BTreeMap, BTreeSet};

use crate::model::portal::{
    PortalBackend, PortalConfigInfo, PortalPreference, PortalRoute, RouteEvidence, RouteStatus,
};

/// Resolve the route table for every known portal interface
/// (architecture §10).
///
/// The universe of interfaces is the union of interfaces implemented by
/// discovered backends and interfaces referenced by configuration, so a
/// configured-but-implemented-nowhere interface still appears as a route.
#[must_use]
pub fn resolve_routes(
    desktops: &[String],
    config: &PortalConfigInfo,
    backends: &[PortalBackend],
) -> Vec<PortalRoute> {
    let mut interfaces: BTreeSet<String> = backends
        .iter()
        .flat_map(|b| b.interfaces.iter().cloned())
        .collect();
    interfaces.extend(config.preferences.iter().map(|p| p.interface.clone()));

    let preferences: BTreeMap<&str, &PortalPreference> = config
        .preferences
        .iter()
        .map(|p| (p.interface.as_str(), p))
        .collect();

    interfaces
        .into_iter()
        .map(|interface| resolve_interface(&interface, desktops, &preferences, backends))
        .collect()
}

fn resolve_interface(
    interface: &str,
    desktops: &[String],
    preferences: &BTreeMap<&str, &PortalPreference>,
    backends: &[PortalBackend],
) -> PortalRoute {
    let preference = preferences.get(interface);

    let available: Vec<&PortalBackend> = backends
        .iter()
        .filter(|b| b.interfaces.contains(interface))
        .collect();
    let usable: Vec<&PortalBackend> = available
        .iter()
        .copied()
        .filter(|b| use_in_matches(b, desktops))
        .collect();

    let mut evidence = Vec::new();
    if let Some(pref) = preference {
        evidence.push(RouteEvidence {
            message: format!(
                "preferred entry from {} (priority {}): {}",
                pref.source_file,
                pref.source_priority,
                pref.backends.join(", ")
            ),
        });
    } else {
        evidence.push(RouteEvidence {
            message: "no [preferred] entry; default to any available backend".to_owned(),
        });
    }
    for backend in &usable {
        evidence.push(RouteEvidence {
            message: format!(
                "backend {} from {} implements {}",
                backend.id, backend.descriptor_path, interface
            ),
        });
    }
    for backend in &available {
        if !use_in_matches(backend, desktops) {
            evidence.push(RouteEvidence {
                message: format!(
                    "backend {} excluded by UseIn (allowed: {})",
                    backend.id,
                    backend.legacy_use_in.join(", ")
                ),
            });
        }
    }

    let requested: Vec<String> = preference.map(|p| p.backends.clone()).unwrap_or_default();
    let (selected, status) = select_candidates(&requested, &usable, preference.is_some());

    PortalRoute {
        interface: interface.to_owned(),
        requested_candidates: requested,
        available_candidates: usable.iter().map(|b| b.id.clone()).collect(),
        selected_candidates: selected,
        evidence,
        status,
    }
}

/// Whether a backend serves this desktop: no `UseIn` restrictions, a `*`
/// wildcard, or any overlap with the current desktop names. Matching is
/// ASCII case-insensitive because desktop identifiers routinely differ in
/// case between the session (e.g. `GNOME`) and descriptors (`gnome`).
fn use_in_matches(backend: &PortalBackend, desktops: &[String]) -> bool {
    if backend.legacy_use_in.is_empty() {
        return true;
    }
    if backend.legacy_use_in.iter().any(|entry| entry == "*") {
        return true;
    }
    desktops.iter().any(|d| {
        backend
            .legacy_use_in
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(d))
    })
}

fn select_candidates(
    requested: &[String],
    usable: &[&PortalBackend],
    has_preference: bool,
) -> (Vec<String>, RouteStatus) {
    if has_preference && requested.iter().any(|token| token == "none") {
        return (Vec::new(), RouteStatus::Disabled);
    }
    if requested.iter().any(|token| token == "*") || !has_preference {
        let selected: Vec<String> = usable.iter().map(|b| b.id.clone()).collect();
        let empty = selected.is_empty();
        return (selected, status_for(empty));
    }
    let selected: Vec<String> = requested
        .iter()
        .filter(|token| usable.iter().any(|b| b.id == **token))
        .cloned()
        .collect();
    let empty = selected.is_empty();
    (selected, status_for(empty))
}

fn status_for(empty: bool) -> RouteStatus {
    if empty {
        RouteStatus::NoProvider
    } else {
        RouteStatus::Selected
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_routes;
    use crate::model::portal::{PortalBackend, PortalConfigInfo, PortalPreference, RouteStatus};

    fn backend(id: &str, dbus: &str, interfaces: &[&str], use_in: &[&str]) -> PortalBackend {
        PortalBackend {
            id: id.to_owned(),
            descriptor_path: format!("/usr/share/xdg-desktop-portal/portals/{id}.portal"),
            duplicate_descriptors: Vec::new(),
            dbus_name: dbus.to_owned(),
            interfaces: interfaces.iter().map(|s| (*s).to_owned()).collect(),
            legacy_use_in: use_in.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn preference(interface: &str, backends: &[&str]) -> PortalPreference {
        PortalPreference {
            interface: interface.to_owned(),
            backends: backends.iter().map(|s| (*s).to_owned()).collect(),
            source_file: "/cfg/xdg-desktop-portal/portals.conf".to_owned(),
            source_priority: 1,
        }
    }

    fn config(preferences: Vec<PortalPreference>) -> PortalConfigInfo {
        PortalConfigInfo {
            candidate_files: vec!["/cfg/xdg-desktop-portal/portals.conf".to_owned()],
            selected_file: Some("/cfg/xdg-desktop-portal/portals.conf".to_owned()),
            preferences,
            parse_errors: Vec::new(),
        }
    }

    fn route<'a>(
        interface: &str,
        routes: &'a [crate::model::portal::PortalRoute],
    ) -> &'a crate::model::portal::PortalRoute {
        routes.iter().find(|r| r.interface == interface).unwrap()
    }

    const FILE_CHOOSER: &str = "org.freedesktop.impl.portal.FileChooser";
    const SCREENCAST: &str = "org.freedesktop.impl.portal.ScreenCast";
    const SCREENSHOT: &str = "org.freedesktop.impl.portal.Screenshot";

    #[test]
    fn gnome_default_routing_selects_preferred_backend() {
        let backends = vec![
            backend(
                "gnome",
                "org.freedesktop.impl.portal.desktop.gnome",
                &[SCREENSHOT],
                &["gnome"],
            ),
            backend(
                "gtk",
                "org.freedesktop.impl.portal.desktop.gtk",
                &[SCREENSHOT],
                &[],
            ),
        ];
        let config = config(vec![preference(SCREENSHOT, &["gnome", "gtk"])]);
        let routes = resolve_routes(&["GNOME".to_owned()], &config, &backends);
        let shot = route(SCREENSHOT, &routes);
        assert_eq!(shot.requested_candidates, ["gnome", "gtk"]);
        assert_eq!(shot.available_candidates, ["gnome", "gtk"]);
        // Both preferred backends are selected candidates in preference order.
        assert_eq!(shot.selected_candidates, ["gnome", "gtk"]);
        assert_eq!(shot.status, RouteStatus::Selected);
    }

    #[test]
    fn explicit_filechooser_override_selects_override_backend() {
        let backends = vec![
            backend("gnome", "d.gnome", &[FILE_CHOOSER], &["gnome"]),
            backend("gtk", "d.gtk", &[FILE_CHOOSER], &[]),
        ];
        let config = config(vec![preference(FILE_CHOOSER, &["gtk"])]);
        let routes = resolve_routes(&["GNOME".to_owned()], &config, &backends);
        assert_eq!(route(FILE_CHOOSER, &routes).selected_candidates, ["gtk"]);
    }

    #[test]
    fn configured_backend_not_installed_keeps_available_ones() {
        let backends = vec![backend("gtk", "d.gtk", &[SCREENSHOT], &[])];
        let config = config(vec![preference(SCREENSHOT, &["gnome", "gtk"])]);
        let routes = resolve_routes(&["GNOME".to_owned()], &config, &backends);
        let shot = route(SCREENSHOT, &routes);
        assert_eq!(shot.available_candidates, ["gtk"]);
        assert_eq!(shot.selected_candidates, ["gtk"]);
        assert_eq!(shot.status, RouteStatus::Selected);
    }

    #[test]
    fn backend_installed_but_missing_interface_yields_no_selection() {
        let backends = vec![backend("gnome", "d.gnome", &[], &["gnome"])];
        let config = config(vec![preference(SCREENSHOT, &["gnome"])]);
        let routes = resolve_routes(&["GNOME".to_owned()], &config, &backends);
        let shot = route(SCREENSHOT, &routes);
        assert!(shot.available_candidates.is_empty());
        assert!(shot.selected_candidates.is_empty());
        assert_eq!(shot.status, RouteStatus::NoProvider);
    }

    #[test]
    fn none_preference_disables_the_interface() {
        let backends = vec![backend("gnome", "d.gnome", &[SCREENSHOT], &[])];
        let config = config(vec![preference(SCREENSHOT, &["none"])]);
        let routes = resolve_routes(&["GNOME".to_owned()], &config, &backends);
        let shot = route(SCREENSHOT, &routes);
        assert!(shot.selected_candidates.is_empty());
        assert_eq!(shot.status, RouteStatus::Disabled);
    }

    #[test]
    fn star_preference_selects_every_available_backend() {
        let backends = vec![
            backend("gnome", "d.gnome", &[SCREENSHOT], &[]),
            backend("gtk", "d.gtk", &[SCREENSHOT], &[]),
        ];
        let config = config(vec![preference(SCREENSHOT, &["*"])]);
        let routes = resolve_routes(&["GNOME".to_owned()], &config, &backends);
        assert_eq!(
            route(SCREENSHOT, &routes).selected_candidates,
            ["gnome", "gtk"]
        );
    }

    #[test]
    fn no_preference_defaults_to_all_available() {
        let backends = vec![
            backend("gnome", "d.gnome", &[SCREENCAST], &[]),
            backend("gtk", "d.gtk", &[SCREENCAST], &[]),
        ];
        let routes = resolve_routes(&["GNOME".to_owned()], &config(Vec::new()), &backends);
        let cast = route(SCREENCAST, &routes);
        assert!(cast.requested_candidates.is_empty());
        assert_eq!(cast.available_candidates, ["gnome", "gtk"]);
        assert_eq!(cast.selected_candidates, ["gnome", "gtk"]);
        assert_eq!(cast.status, RouteStatus::Selected);
    }

    #[test]
    fn use_in_excludes_backend_from_other_desktops() {
        let backends = vec![backend("gnome", "d.gnome", &[SCREENSHOT], &["gnome"])];
        let routes = resolve_routes(&["KDE".to_owned()], &config(Vec::new()), &backends);
        let shot = route(SCREENSHOT, &routes);
        assert!(shot.available_candidates.is_empty());
        assert_eq!(shot.status, RouteStatus::NoProvider);
        assert!(shot.evidence.iter().any(|e| e.message.contains("UseIn")));
    }

    #[test]
    fn multiple_desktop_names_allow_any_matching_backend() {
        let backends = vec![backend("gnome", "d.gnome", &[SCREENSHOT], &["GNOME"])];
        let routes = resolve_routes(
            &["ubuntu".to_owned(), "GNOME".to_owned()],
            &config(Vec::new()),
            &backends,
        );
        assert_eq!(route(SCREENSHOT, &routes).available_candidates, ["gnome"]);
    }

    #[test]
    fn routes_are_deterministic_and_sorted_by_interface() {
        let backends = vec![
            backend("gtk", "d.gtk", &[SCREENSHOT], &[]),
            backend("gnome", "d.gnome", &[SCREENCAST, SCREENSHOT], &[]),
        ];
        let routes = resolve_routes(&["GNOME".to_owned()], &config(Vec::new()), &backends);
        let interfaces: Vec<&str> = routes.iter().map(|r| r.interface.as_str()).collect();
        let mut sorted = interfaces.clone();
        sorted.sort_unstable();
        assert_eq!(interfaces, sorted);
        assert!(routes.iter().any(|r| r.interface == SCREENCAST));
        assert!(routes.iter().any(|r| r.interface == SCREENSHOT));
    }

    #[test]
    fn configured_interface_without_backends_is_no_provider() {
        let backends: Vec<PortalBackend> = Vec::new();
        let config = config(vec![preference(SCREENCAST, &["gnome"])]);
        let routes = resolve_routes(&["GNOME".to_owned()], &config, &backends);
        let cast = route(SCREENCAST, &routes);
        assert_eq!(cast.status, RouteStatus::NoProvider);
        assert!(cast.available_candidates.is_empty());
    }
}
