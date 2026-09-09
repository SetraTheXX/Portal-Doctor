//! Canonical inventory for the Phase 13 diagnostic coverage contract.
//!
//! The inventory describes the existing runtime/model/report seams and their
//! qualification boundary. It does not claim that controlled fixtures are
//! live desktop support.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct DiagnosticCapability {
    pub key: &'static str,
    pub runtime: &'static str,
    pub snapshot: &'static str,
    pub entry_point: &'static str,
    pub contract: &'static str,
    pub implementation: &'static str,
    pub controlled: &'static str,
    pub qualification: &'static str,
    pub blocker: &'static str,
}

#[allow(dead_code)]
impl DiagnosticCapability {
    /// Stable machine-readable row mirrored in the roadmap contract block.
    #[must_use]
    pub fn doc_row(self) -> String {
        format!(
            "{}|runtime={}|snapshot={}|entry_point={}|contract={}|implementation={}|controlled={}|qualification={}|blocker={}",
            self.key,
            self.runtime,
            self.snapshot,
            self.entry_point,
            self.contract,
            self.implementation,
            self.controlled,
            self.qualification,
            self.blocker,
        )
    }
}

/// The complete diagnostic capability inventory required by the Phase 13
/// diagnostics gate, in stable order.
#[allow(dead_code)]
pub(crate) const DIAGNOSTIC_CAPABILITIES: &[DiagnosticCapability] = &[
    DiagnosticCapability {
        key: "portal_routing",
        runtime: "portal_config+portal_files+resolver_portal_routes",
        snapshot: "portal_config+portal_backends+portal_routes",
        entry_point: "default_check+check_portal+portal_list_routes_explain",
        contract: "CFG_XDP_findings",
        implementation: "complete",
        controlled: "complete",
        qualification: "baseline_live;expanded_desktops_controlled_only",
        blocker: "none",
    },
    DiagnosticCapability {
        key: "environment_activation",
        runtime: "environment+activation_environment",
        snapshot: "session+environment",
        entry_point: "default_check+check_environment",
        contract: "ENV001_ENV004_findings",
        implementation: "complete",
        controlled: "complete",
        qualification: "baseline_live;expanded_desktops_controlled_only",
        blocker: "none",
    },
    DiagnosticCapability {
        key: "dbus_runtime",
        runtime: "dbus+rules_dbus",
        snapshot: "dbus",
        entry_point: "default_check+check_portal",
        contract: "DBUS001_DBUS002_XDP001_XDP002_findings",
        implementation: "complete",
        controlled: "complete",
        qualification: "baseline_live;expanded_desktops_controlled_only",
        blocker: "none",
    },
    DiagnosticCapability {
        key: "systemd_user_services",
        runtime: "systemd_user+rules_dbus",
        snapshot: "services",
        entry_point: "default_check+check_portal+verbose_runtime",
        contract: "DBUS002_XDP002_findings",
        implementation: "complete",
        controlled: "complete",
        qualification: "baseline_live;expanded_desktops_controlled_only",
        blocker: "none",
    },
    DiagnosticCapability {
        key: "pipewire_wireplumber",
        runtime: "pipewire+rules_pipewire",
        snapshot: "pipewire+wireplumber",
        entry_point: "default_check+check_pipewire+report",
        contract: "PW001_PW003_SC001_SC002_findings",
        implementation: "complete",
        controlled: "complete",
        qualification: "baseline_live;expanded_desktops_controlled_only",
        blocker: "none",
    },
    DiagnosticCapability {
        key: "journal_evidence",
        runtime: "journal+report_redaction",
        snapshot: "journal",
        entry_point: "journal_opt_in+report",
        contract: "sanitized_supporting_evidence_for_PW_SC",
        implementation: "complete",
        controlled: "complete",
        qualification: "baseline_live_opt_in;not_a_provider_gate",
        blocker: "none",
    },
    DiagnosticCapability {
        key: "active_core_probes",
        runtime: "filechooser+screenshot+screencast_internal",
        snapshot: "standalone_ProbeResult_v1",
        entry_point: "explicit_filechooser_screenshot;internal_screencast",
        contract: "ProbeResult_v1",
        implementation: "complete",
        controlled: "complete",
        qualification: "FileChooser_live;Screenshot_ScreenCast_external",
        blocker: "GNOME_Screenshot_provider;ScreenCast_Window_capability",
    },
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::DIAGNOSTIC_CAPABILITIES;

    #[test]
    fn inventory_keys_are_unique_and_stable() {
        let keys = DIAGNOSTIC_CAPABILITIES
            .iter()
            .map(|capability| capability.key)
            .collect::<Vec<_>>();
        let unique = keys.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), 7);
        assert_eq!(unique.len(), keys.len());
        assert_eq!(
            keys,
            vec![
                "portal_routing",
                "environment_activation",
                "dbus_runtime",
                "systemd_user_services",
                "pipewire_wireplumber",
                "journal_evidence",
                "active_core_probes",
            ]
        );
    }
}
