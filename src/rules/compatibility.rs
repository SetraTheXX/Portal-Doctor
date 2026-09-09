use crate::model::evidence::Evidence;
use crate::model::finding::{Confidence, Finding, Severity};
use crate::model::portal::{PortalConfigCandidate, RouteStatus};
use crate::model::portal_frontend::{PORTAL_FRONTEND_COMPONENT, SemanticVersion};
use crate::model::snapshot::Snapshot;
use crate::rules::engine::DiagnosticRule;

const SETTINGS_INTERFACE: &str = "org.freedesktop.impl.portal.Settings";
const GNOME_BACKEND: &str = "gnome";
const GTK_BACKEND: &str = "gtk";
const NIRI_DESKTOP: &str = "niri";
const AFFECTED_FRONTEND_VERSION: SemanticVersion = SemanticVersion::new(1, 22, 0);

/// XDP006 — narrowly scoped XDP 1.22.0/Niri Settings compatibility risk.
///
/// This is a known-upstream-behavior warning, not an observation that a
/// duplicate `SettingsChanged` stream was seen on this machine. It therefore
/// requires exact frontend version evidence, an effective explicit Settings
/// override and both capable descriptors before it fires.
pub struct Xdp006;

impl DiagnosticRule for Xdp006 {
    fn id(&self) -> &'static str {
        "XDP006"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(frontend) = &snapshot.portal_frontend.value else {
            return Vec::new();
        };
        if frontend.component != PORTAL_FRONTEND_COMPONENT
            || frontend.normalized_version != AFFECTED_FRONTEND_VERSION
        {
            return Vec::new();
        }
        if !is_canonical_niri(snapshot)
            || !has_effective_settings_override(snapshot)
            || !has_lower_settings_gnome_candidate(snapshot)
        {
            return Vec::new();
        }
        let Some(backends) = &snapshot.portal_backends.value else {
            return Vec::new();
        };
        let Some(gnome) = backends.iter().find(|backend| backend.id == GNOME_BACKEND) else {
            return Vec::new();
        };
        let Some(gtk) = backends.iter().find(|backend| backend.id == GTK_BACKEND) else {
            return Vec::new();
        };
        if !gnome.interfaces.contains(SETTINGS_INTERFACE)
            || !gtk.interfaces.contains(SETTINGS_INTERFACE)
        {
            return Vec::new();
        }

        vec![Finding {
            id: self.id().to_owned(),
            severity: Severity::Warning,
            confidence: Confidence::Medium,
            title: "Known xdg-desktop-portal Settings compatibility risk".to_owned(),
            summary: format!(
                "Niri explicitly routes Settings to GTK while xdg-desktop-portal {} matches the exact XDP #2033 regression context.",
                frontend.normalized_version
            ),
            explanation: "The exact frontend version and effective Niri configuration match a narrowly bounded upstream compatibility report involving Settings provider resolution. PortalDoctor did not observe duplicate SettingsChanged traffic or claim that a runtime conflict occurred; this finding is a known compatibility risk only.".to_owned(),
            evidence: vec![
                Evidence::PortalFrontendVersion,
                Evidence::ConfigSelection,
                Evidence::ConfigCandidate,
            ],
            impact: Some(
                "Settings updates may be inconsistent on the affected frontend/configuration combination until the upstream behavior is confirmed or fixed.".to_owned(),
            ),
            recommendation: vec![
                "Verify the upstream fix or confirm actual SettingsChanged behavior before changing the selected backend.".to_owned(),
            ],
            source_component: "compatibility".to_owned(),
        }]
    }
}

fn is_canonical_niri(snapshot: &Snapshot) -> bool {
    let Some(session) = &snapshot.session.value else {
        return false;
    };
    let current = session
        .current_desktop
        .as_deref()
        .map(crate::resolver::portal_routes::normalize_desktops);
    let session_desktop = session
        .session_desktop
        .as_deref()
        .map(|desktop| desktop.trim().to_ascii_lowercase());
    current
        .as_ref()
        .is_some_and(|desktops| desktops.len() == 1 && desktops[0] == NIRI_DESKTOP)
        && session_desktop.as_deref() == Some(NIRI_DESKTOP)
}

fn has_effective_settings_override(snapshot: &Snapshot) -> bool {
    let Some(config) = &snapshot.portal_config.value else {
        return false;
    };
    let Some(selected_file) = config.selected_file.as_deref() else {
        return false;
    };
    if !config.parse_errors.is_empty() {
        return false;
    }
    let settings: Vec<_> = config
        .preferences
        .iter()
        .filter(|preference| preference.interface == SETTINGS_INTERFACE)
        .collect();
    if settings.len() != 1 {
        return false;
    }
    let preference = settings[0];
    if preference.source_file != selected_file
        || preference.backends.len() != 1
        || preference.backends[0] != GTK_BACKEND
    {
        return false;
    }
    let Some(routes) = &snapshot.portal_routes.value else {
        return false;
    };
    routes.iter().any(|route| {
        route.interface == SETTINGS_INTERFACE
            && route.status == RouteStatus::Selected
            && route.selected_candidates.len() == 1
            && route.selected_candidates[0] == GTK_BACKEND
    })
}

fn has_lower_settings_gnome_candidate(snapshot: &Snapshot) -> bool {
    let Some(config) = &snapshot.portal_config.value else {
        return false;
    };
    let Some(selected_file) = config.selected_file.as_deref() else {
        return false;
    };
    let Some(selected_settings) = config
        .preferences
        .iter()
        .find(|preference| preference.interface == SETTINGS_INTERFACE)
    else {
        return false;
    };

    config.lower_priority_candidates.iter().any(|candidate| {
        candidate.path != selected_file
            && candidate.status == crate::model::status::CollectorState::Available
            && candidate.parse_errors.is_empty()
            && candidate.source_priority > selected_settings.source_priority
            && lower_candidate_settings_preference(candidate).is_some_and(|preference| {
                preference
                    .backends
                    .iter()
                    .any(|backend| backend == GNOME_BACKEND)
            })
    })
}

fn lower_candidate_settings_preference(
    candidate: &PortalConfigCandidate,
) -> Option<&crate::model::portal::PortalPreference> {
    let explicit: Vec<_> = candidate
        .preferences
        .iter()
        .filter(|preference| preference.interface == SETTINGS_INTERFACE)
        .collect();
    if explicit.len() > 1 {
        return None;
    }
    if let Some(preference) = explicit.into_iter().next() {
        return Some(preference);
    }

    let defaults: Vec<_> = candidate
        .preferences
        .iter()
        .filter(|preference| {
            preference.interface == crate::resolver::portal_routes::DEFAULT_INTERFACE
        })
        .collect();
    (defaults.len() == 1).then(|| defaults[0])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::Xdp006;
    use crate::model::dbus::DbusInfo;
    use crate::model::environment::{SessionInfo, SessionType};
    use crate::model::portal::{PortalConfigCandidate, PortalConfigInfo};
    use crate::model::portal_frontend::{
        PORTAL_FRONTEND_COMPONENT, PortalFrontendInfo, SemanticVersion, VersionEvidenceSource,
    };
    use crate::model::section::Section;
    use crate::model::snapshot::Snapshot;
    use crate::model::status::CollectorState;
    use crate::rules::engine::DiagnosticRule;

    const SETTINGS: &str = "org.freedesktop.impl.portal.Settings";

    fn snapshot(
        version: Option<SemanticVersion>,
        desktop: &str,
        settings_override: bool,
        gnome_settings: bool,
        gtk_settings: bool,
    ) -> Snapshot {
        let process = BTreeMap::from([
            ("XDG_CURRENT_DESKTOP".to_owned(), desktop.to_owned()),
            ("XDG_SESSION_DESKTOP".to_owned(), "niri".to_owned()),
            ("XDG_SESSION_TYPE".to_owned(), "wayland".to_owned()),
            ("WAYLAND_DISPLAY".to_owned(), "wayland-1".to_owned()),
        ]);
        let session = SessionInfo {
            current_desktop: Some(desktop.to_owned()),
            session_desktop: Some("niri".to_owned()),
            session_type: Some(SessionType::Wayland),
            session_type_raw: Some("wayland".to_owned()),
            wayland_display: Some("wayland-1".to_owned()),
            display: None,
        };
        let config_text = if settings_override {
            include_str!("../../tests/fixtures/portal-routing/niri-settings-override.conf")
        } else {
            include_str!("../../tests/fixtures/portal-routing/niri-portals.conf")
        };
        let selected_file = if settings_override {
            "/home/tester/.config/xdg-desktop-portal/niri-portals.conf"
        } else {
            "/usr/share/xdg-desktop-portal/niri-portals.conf"
        };
        let (preferences, parse_errors) =
            crate::collectors::portal_config::parse_config(config_text, selected_file, 0);
        let config = PortalConfigInfo {
            candidate_files: vec![selected_file.to_owned()],
            selected_file: Some(selected_file.to_owned()),
            preferences,
            parse_errors,
            lower_priority_candidates: Vec::new(),
        };
        let mut gnome = crate::collectors::portal_files::parse_portal_file(
            include_str!("../../tests/fixtures/portal-routing/gnome.portal"),
            "/fixture/gnome.portal",
            "gnome".to_owned(),
        );
        let mut gtk = crate::collectors::portal_files::parse_portal_file(
            include_str!("../../tests/fixtures/portal-routing/niri-gtk.portal"),
            "/fixture/gtk.portal",
            "gtk".to_owned(),
        );
        if !gnome_settings {
            gnome.interfaces.remove(SETTINGS);
        }
        if !gtk_settings {
            gtk.interfaces.remove(SETTINGS);
        }
        let backends = vec![gnome, gtk];
        let desktops = crate::resolver::portal_routes::normalize_desktops(desktop);
        let routes = crate::resolver::portal_routes::resolve_routes(&desktops, &config, &backends);

        let mut result = Snapshot::new(0);
        result.session = Section::available(session);
        result.environment = Section::available(crate::collectors::environment::environment_info(
            process, None, None,
        ));
        result.portal_config = Section::available(config);
        result.portal_backends = Section::available(backends);
        result.portal_routes = Section::available(routes);
        result.dbus = Section::available(DbusInfo {
            connected: true,
            checks: Vec::new(),
        });
        if let Some(version) = version {
            result.portal_frontend = Section::available(PortalFrontendInfo::new(
                version.to_string(),
                version,
                VersionEvidenceSource::DpkgQuery {
                    package: PORTAL_FRONTEND_COMPONENT.to_owned(),
                },
            ));
        }
        result
    }

    fn ids(snapshot: &Snapshot) -> Vec<String> {
        Xdp006
            .evaluate(snapshot)
            .into_iter()
            .map(|finding| finding.id)
            .collect()
    }

    fn with_lower_candidate(mut snapshot: Snapshot, text: &str) -> Snapshot {
        let (preferences, parse_errors) = crate::collectors::portal_config::parse_config(
            text,
            "/usr/share/xdg-desktop-portal/portals.conf",
            1,
        );
        snapshot
            .portal_config
            .value
            .as_mut()
            .expect("portal config fixture")
            .lower_priority_candidates = vec![PortalConfigCandidate {
            path: "/usr/share/xdg-desktop-portal/portals.conf".to_owned(),
            source_priority: 1,
            status: if parse_errors.is_empty() {
                CollectorState::Available
            } else {
                CollectorState::ParseError
            },
            preferences,
            parse_errors,
        }];
        snapshot
    }

    #[test]
    fn exact_affected_version_and_effective_override_emit_one_risk() {
        let findings = Xdp006.evaluate(&with_lower_candidate(
            snapshot(
                Some(SemanticVersion::new(1, 22, 0)),
                "niri",
                true,
                true,
                true,
            ),
            include_str!("../../tests/fixtures/portal-routing/generic-gnome-gtk-portals.conf"),
        ));
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(
            ids(&with_lower_candidate(
                snapshot(
                    Some(SemanticVersion::new(1, 22, 0)),
                    "niri",
                    true,
                    true,
                    true
                ),
                include_str!("../../tests/fixtures/portal-routing/generic-gnome-gtk-portals.conf"),
            )),
            ["XDP006"]
        );
        assert_eq!(
            findings[0].confidence,
            crate::model::finding::Confidence::Medium
        );
        assert!(findings[0].summary.contains("1.22.0"));
        assert!(!findings[0].summary.contains("duplicate"));
    }

    #[test]
    fn lower_candidate_is_required_for_the_regression_risk() {
        assert!(
            ids(&snapshot(
                Some(SemanticVersion::new(1, 22, 0)),
                "niri",
                true,
                true,
                true,
            ))
            .is_empty()
        );
    }

    #[test]
    fn lower_candidate_must_make_gnome_a_settings_candidate() {
        let no_gnome = with_lower_candidate(
            snapshot(
                Some(SemanticVersion::new(1, 22, 0)),
                "niri",
                true,
                true,
                true,
            ),
            "[preferred]\norg.freedesktop.impl.portal.Settings=gtk;\n",
        );
        assert!(ids(&no_gnome).is_empty());

        let malformed = with_lower_candidate(
            snapshot(
                Some(SemanticVersion::new(1, 22, 0)),
                "niri",
                true,
                true,
                true,
            ),
            "[preferred]\ndefault=gnome;gtk\nbroken-line\n",
        );
        assert!(ids(&malformed).is_empty());
    }

    #[test]
    fn unreadable_lower_candidate_is_silent() {
        let mut snapshot = snapshot(
            Some(SemanticVersion::new(1, 22, 0)),
            "niri",
            true,
            true,
            true,
        );
        snapshot
            .portal_config
            .value
            .as_mut()
            .expect("portal config fixture")
            .lower_priority_candidates = vec![PortalConfigCandidate {
            path: "/usr/share/xdg-desktop-portal/portals.conf".to_owned(),
            source_priority: 1,
            status: CollectorState::Unavailable,
            preferences: Vec::new(),
            parse_errors: vec!["cannot read lower-priority candidate".to_owned()],
        }];
        assert!(ids(&snapshot).is_empty());
    }

    #[test]
    fn absent_or_uncomparable_version_is_silent() {
        assert!(ids(&snapshot(None, "niri", true, true, true)).is_empty());
        let mut malformed = snapshot(
            Some(SemanticVersion::new(1, 22, 0)),
            "niri",
            true,
            true,
            true,
        );
        malformed.portal_frontend = Section::parse_error("not comparable");
        assert!(ids(&malformed).is_empty());
    }

    #[test]
    fn only_exact_affected_version_matches() {
        for version in [
            SemanticVersion::new(1, 21, 1),
            SemanticVersion::new(1, 23, 0),
        ] {
            assert!(ids(&snapshot(Some(version), "niri", true, true, true)).is_empty());
        }
    }

    #[test]
    fn desktop_and_configuration_negatives_are_silent() {
        for desktop in ["GNOME", "KDE", "Sway", "Hyprland", "niri:GNOME"] {
            assert!(
                ids(&snapshot(
                    Some(SemanticVersion::new(1, 22, 0)),
                    desktop,
                    true,
                    true,
                    true,
                ))
                .is_empty()
            );
        }
        assert!(
            ids(&snapshot(
                Some(SemanticVersion::new(1, 22, 0)),
                "niri",
                false,
                true,
                true
            ))
            .is_empty()
        );
    }

    #[test]
    fn descriptor_and_route_negatives_are_silent() {
        assert!(
            ids(&snapshot(
                Some(SemanticVersion::new(1, 22, 0)),
                "niri",
                true,
                false,
                true
            ))
            .is_empty()
        );
        assert!(
            ids(&snapshot(
                Some(SemanticVersion::new(1, 22, 0)),
                "niri",
                true,
                true,
                false
            ))
            .is_empty()
        );
    }
}
