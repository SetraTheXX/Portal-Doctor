//! Canonical controlled-versus-live compatibility qualification matrix.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum QualificationState {
    LiveQualified,
    ControlledOnly,
    ExternallyBlocked,
    NotAvailable,
}

impl QualificationState {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::LiveQualified => "live_qualified",
            Self::ControlledOnly => "controlled_only",
            Self::ExternallyBlocked => "externally_blocked",
            Self::NotAvailable => "not_available",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct CompatibilityEntry {
    pub key: &'static str,
    pub desktop: &'static str,
    pub implementation: &'static str,
    pub controlled: &'static str,
    pub live: QualificationState,
    pub prerequisite: &'static str,
    pub provider_model: &'static str,
    pub support_claim: &'static str,
    pub active_gate: &'static str,
    pub blocker: &'static str,
}

#[allow(dead_code)]
impl CompatibilityEntry {
    /// Stable machine-readable row mirrored by the roadmap and compatibility
    /// documents.
    #[must_use]
    pub(crate) fn doc_row(self) -> String {
        format!(
            "{}|desktop={}|implementation={}|controlled={}|live={}|prerequisite={}|provider_model={}|support_claim={}|active_gate={}|blocker={}",
            self.key,
            self.desktop,
            self.implementation,
            self.controlled,
            self.live.as_str(),
            self.prerequisite,
            self.provider_model,
            self.support_claim,
            self.active_gate,
            self.blocker,
        )
    }

    /// Validate that a row cannot silently claim live support without a
    /// concrete recheck prerequisite and, for non-live rows, a blocker.
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        if self.key.is_empty()
            || self.desktop.is_empty()
            || self.implementation.is_empty()
            || self.controlled.is_empty()
            || self.prerequisite.is_empty()
            || self.provider_model.is_empty()
            || self.support_claim.is_empty()
            || self.active_gate.is_empty()
        {
            return Err("compatibility inventory fields must not be empty");
        }
        if self.live != QualificationState::LiveQualified && self.blocker.is_empty() {
            return Err("non-live qualification requires an explicit blocker");
        }
        Ok(())
    }
}

/// Compatibility evidence currently recorded by the repository. Controlled
/// fixture coverage is intentionally not promoted to live support.
#[allow(dead_code)]
pub(crate) const COMPATIBILITY_INVENTORY: &[CompatibilityEntry] = &[
    CompatibilityEntry {
        key: "gnome",
        desktop: "GNOME",
        implementation: "complete",
        controlled: "complete",
        live: QualificationState::LiveQualified,
        prerequisite: "Ubuntu_26.04+GNOME+Wayland+systemd_user+portal_frontend",
        provider_model: "GNOME_capture+GTK_fallback+standard_portal_descriptors",
        support_claim: "passive_baseline_only;active_probes_unreleased",
        active_gate: "externally_blocked_Screenshot_and_ScreenCast",
        blocker: "Screenshot_GNOME_provider_hang_crash;ScreenCast_AvailableSourceTypes_Window_bit_2_missing",
    },
    CompatibilityEntry {
        key: "kde_plasma",
        desktop: "KDE_Plasma",
        implementation: "complete",
        controlled: "complete",
        live: QualificationState::ControlledOnly,
        prerequisite: "real_Plasma_Wayland+selected_kde_route+healthy_kde_backend_service+canonical_KDE_DBus_owner+capability_evidence+PipeWire_WirePlumber",
        provider_model: "KDE_backend_selected_by_standard_portals_conf_and_portal_descriptor",
        support_claim: "no_live_support_claim",
        active_gate: "not_started",
        blocker: "current_host_GNOME;KDE_package_service_owner_absent",
    },
    CompatibilityEntry {
        key: "sway_wlroots",
        desktop: "Sway_wlroots",
        implementation: "complete",
        controlled: "complete",
        live: QualificationState::ControlledOnly,
        prerequisite: "real_Sway_Wayland+wlr_backend_package_service+canonical_WLR_DBus_owner+WLR_passive_routes+Window_capability_evidence+PipeWire_WirePlumber",
        provider_model: "WLR_for_Screenshot_ScreenCast+GTK_fallback_for_FileChooser_Settings",
        support_claim: "no_live_support_claim",
        active_gate: "not_started",
        blocker: "current_host_GNOME;WLR_package_service_owner_absent",
    },
    CompatibilityEntry {
        key: "hyprland",
        desktop: "Hyprland",
        implementation: "complete",
        controlled: "complete",
        live: QualificationState::ControlledOnly,
        prerequisite: "real_Hyprland_Wayland+Hyprland_backend_package_service+canonical_Hyprland_DBus_owner+Hyprland_passive_routes+capability_evidence+PipeWire_WirePlumber",
        provider_model: "Hyprland_for_Screenshot_ScreenCast+GTK_fallback_for_FileChooser_Settings",
        support_claim: "no_live_support_claim",
        active_gate: "not_started",
        blocker: "current_host_GNOME;Hyprland_session_package_service_owner_absent",
    },
    CompatibilityEntry {
        key: "niri",
        desktop: "Niri",
        implementation: "complete",
        controlled: "complete",
        live: QualificationState::ControlledOnly,
        prerequisite: "real_Niri_Wayland+upstream_shaped_effective_niri_portals_conf+healthy_GNOME_GTK_services+canonical_GNOME_GTK_DBus_owners+expected_mixed_routes+capability_evidence+PipeWire_WirePlumber",
        provider_model: "GNOME_capture+GTK_fallback;no_Niri_specific_backend",
        support_claim: "no_live_support_claim",
        active_gate: "not_started",
        blocker: "current_host_GNOME;real_Niri_session_absent",
    },
];

/// A controlled fixture must never be promoted directly to live qualification.
#[allow(dead_code)]
pub(crate) fn qualification_transition_allowed(
    from: QualificationState,
    to: QualificationState,
) -> bool {
    if to == QualificationState::LiveQualified {
        return from == QualificationState::LiveQualified;
    }
    true
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{COMPATIBILITY_INVENTORY, QualificationState, qualification_transition_allowed};

    fn documented_rows(markdown: &str, start: &str, end: &str) -> Vec<String> {
        let body = markdown
            .split_once(start)
            .and_then(|(_, remainder)| remainder.split_once(end).map(|(body, _)| body))
            .expect("compatibility inventory markers must exist");
        body.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("```"))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn inventory_is_unique_valid_and_docs_parity_is_exact() {
        let keys = COMPATIBILITY_INVENTORY
            .iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>();
        let unique = keys.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), 5);
        assert_eq!(unique.len(), keys.len());
        assert!(
            COMPATIBILITY_INVENTORY
                .iter()
                .all(|entry| entry.validate().is_ok())
        );

        let expected = COMPATIBILITY_INVENTORY
            .iter()
            .copied()
            .map(super::CompatibilityEntry::doc_row)
            .collect::<Vec<_>>();
        for document in [
            include_str!("../docs/compatibility.md"),
            include_str!("../docs/PORTALDOCTOR_ROADMAP.md"),
        ] {
            assert_eq!(
                documented_rows(
                    document,
                    "<!-- PORTALDOCTOR_COMPATIBILITY_MATRIX_START -->",
                    "<!-- PORTALDOCTOR_COMPATIBILITY_MATRIX_END -->",
                ),
                expected
            );
        }
    }

    #[test]
    fn controlled_or_blocked_qualification_cannot_be_promoted_to_live() {
        for state in [
            QualificationState::ControlledOnly,
            QualificationState::ExternallyBlocked,
            QualificationState::NotAvailable,
        ] {
            assert!(!qualification_transition_allowed(
                state,
                QualificationState::LiveQualified
            ));
        }
        assert!(qualification_transition_allowed(
            QualificationState::LiveQualified,
            QualificationState::LiveQualified
        ));
    }

    #[test]
    fn missing_live_prerequisite_or_blocker_fails_closed() {
        let mut row = COMPATIBILITY_INVENTORY[1];
        row.prerequisite = "";
        assert!(row.validate().is_err());

        let mut row = COMPATIBILITY_INVENTORY[1];
        row.blocker = "";
        assert!(row.validate().is_err());

        let mut row = COMPATIBILITY_INVENTORY[0];
        row.live = QualificationState::ControlledOnly;
        row.blocker = "";
        assert!(row.validate().is_err());
    }
}
