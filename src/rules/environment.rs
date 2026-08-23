use crate::model::environment::EnvironmentRelation;
use crate::model::evidence::Evidence;
use crate::model::finding::{Confidence, Finding, Severity};
use crate::model::snapshot::Snapshot;
use crate::rules::engine::DiagnosticRule;

/// Build a fully populated finding for the environment rule family.
fn env_finding(
    id: &'static str,
    confidence: Confidence,
    title: &'static str,
    summary: String,
    explanation: &'static str,
    impact: &'static str,
    recommendations: Vec<String>,
) -> Finding {
    Finding {
        id: id.to_owned(),
        severity: Severity::Warning,
        confidence,
        title: title.to_owned(),
        summary,
        explanation: explanation.to_owned(),
        evidence: vec![Evidence::EnvironmentMismatch],
        impact: Some(impact.to_owned()),
        recommendation: recommendations,
        source_component: "environment".to_owned(),
    }
}

/// ENV001 — `XDG` desktop identity missing (PRD §8).
pub struct Env001;

impl DiagnosticRule for Env001 {
    fn id(&self) -> &'static str {
        "ENV001"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(session) = &snapshot.session.value else {
            return Vec::new();
        };
        if session.current_desktop.is_some() {
            return Vec::new();
        }
        vec![env_finding(
            self.id(),
            Confidence::High,
            "XDG desktop identity is missing",
            "XDG_CURRENT_DESKTOP is not set in this session.".to_owned(),
            "Applications and portal frontends read XDG_CURRENT_DESKTOP to select \
             desktop-specific behavior and portal configuration. Without it they fall \
             back to generic defaults, which frequently breaks portal routing.",
            "Portal backend selection and desktop-specific integrations may silently \
             misbehave or fall back to the wrong backend.",
            vec![
                "Start the graphical session through the normal desktop launcher so \
                 the session manager sets XDG_CURRENT_DESKTOP."
                    .to_owned(),
                "Re-run `portaldoctor check environment` afterwards.".to_owned(),
            ],
        )]
    }
}

/// ENV002 — session type unavailable/inconsistent (PRD §8).
pub struct Env002;

impl DiagnosticRule for Env002 {
    fn id(&self) -> &'static str {
        "ENV002"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(session) = &snapshot.session.value else {
            return Vec::new();
        };
        if session.session_type.is_some() {
            return Vec::new();
        }
        let observed = match &session.session_type_raw {
            Some(raw) => format!("The reported value is {raw:?}, which is not recognized."),
            None => "No XDG_SESSION_TYPE value is present in this session.".to_owned(),
        };
        vec![env_finding(
            self.id(),
            Confidence::Medium,
            "Session type is unavailable or unrecognized",
            format!("PortalDoctor cannot determine the session type. {observed}"),
            "Wayland-specific and X11-specific diagnostics depend on knowing the \
             session type. An absent or unrecognized XDG_SESSION_TYPE makes every \
             downstream session check unreliable.",
            "Session-specific findings may be incomplete or wrong; Wayland paths are \
             not verifiable while the session type is unknown.",
            vec![
                "Check how the session is started; display managers normally set \
                 XDG_SESSION_TYPE."
                    .to_owned(),
                "Re-run inside a regular desktop session rather than SSH or a bare TTY.".to_owned(),
            ],
        )]
    }
}

/// ENV003 — `Wayland` session without usable `WAYLAND_DISPLAY` (PRD §8).
pub struct Env003;

impl DiagnosticRule for Env003 {
    fn id(&self) -> &'static str {
        "ENV003"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(session) = &snapshot.session.value else {
            return Vec::new();
        };
        if session.session_type != Some(crate::model::environment::SessionType::Wayland) {
            return Vec::new();
        }
        if session.wayland_display.is_some() {
            return Vec::new();
        }
        vec![env_finding(
            self.id(),
            Confidence::High,
            "Wayland session without usable WAYLAND_DISPLAY",
            "The session reports wayland, but WAYLAND_DISPLAY is not set, so clients \
             cannot find the compositor socket."
                .to_owned(),
            "Wayland clients locate the compositor through WAYLAND_DISPLAY inside \
             XDG_RUNTIME_DIR. A wayland session without it means applications cannot \
             connect natively; screen sharing portals depend on that connection.",
            "Native Wayland application startup and ScreenCast portal usage will fail \
             in this session.",
            vec![
                "Verify the compositor was started as part of the graphical session \
                 and exports WAYLAND_DISPLAY."
                    .to_owned(),
                "Confirm XDG_RUNTIME_DIR points to the same runtime directory the \
                 compositor created its socket in."
                    .to_owned(),
            ],
        )]
    }
}

/// ENV004 — relevant session vs activation environment mismatch (PRD §8).
pub struct Env004;

impl DiagnosticRule for Env004 {
    fn id(&self) -> &'static str {
        "ENV004"
    }

    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let Some(environment) = &snapshot.environment.value else {
            return Vec::new();
        };
        let comparison = &environment.activation_comparison;
        if !comparison.performed {
            return Vec::new();
        }
        let mismatched: Vec<_> = comparison
            .entries
            .iter()
            .filter(|entry| entry.relation != EnvironmentRelation::Equal)
            .collect();
        if mismatched.is_empty() {
            return Vec::new();
        }
        let keys: Vec<&str> = mismatched.iter().map(|entry| entry.key.as_str()).collect();
        let details: Vec<String> = mismatched
            .iter()
            .map(|entry| format!("{}: {}", entry.key, entry.relation.as_str()))
            .collect();
        vec![env_finding(
            self.id(),
            Confidence::Medium,
            "Session and systemd activation environments disagree",
            format!(
                "{} allowlisted variable(s) differ between this session and the \
                 systemd user activation environment: {}.",
                mismatched.len(),
                keys.join(", ")
            ),
            "Services activated by systemd --user inherit the activation environment, \
             not the shell environment. When relevant variables such as \
             XDG_CURRENT_DESKTOP or WAYLAND_DISPLAY diverge, portal backends started \
             by systemd see a different desktop than the applications talking to \
             them, a classic cause of broken portal routing.",
            "Portal backends may run with stale or missing desktop context; symptoms \
             include wrong file chooser style, missing screencast sources and \
             settings not propagating.",
            vec![
                format!(
                    "Align both environments for these keys: {}.",
                    details.join("; ")
                ),
                "Update the session import/export units (e.g. \
                 dbus-update-activation-environment or environment.d) so the \
                 activation environment receives the same values."
                    .to_owned(),
            ],
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::{Env001, Env002, Env003, Env004};
    use crate::collectors::environment::{environment_info, session_info};
    use crate::model::finding::Finding;
    use crate::model::section::Section;
    use crate::model::snapshot::Snapshot;
    use crate::rules::engine::{DiagnosticRule, evaluate};
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn map<const N: usize>(pairs: &[(&'static str, &'static str); N]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn fixture_snapshot<const NP: usize, const NA: usize>(
        process: &[(&'static str, &'static str); NP],
        activation: Option<&[(&'static str, &'static str); NA]>,
    ) -> Snapshot {
        let process_map = map(process);
        let activation_map = activation.map(map);
        let collected_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .try_into()
            .unwrap();
        let mut snapshot = Snapshot::new(collected_at);
        snapshot.session = Section::available(session_info(&process_map));
        snapshot.environment =
            Section::available(environment_info(process_map, None, activation_map.as_ref()));
        snapshot
    }

    fn ids(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.id.as_str()).collect()
    }

    #[test]
    fn healthy_ubuntu_gnome_wayland_yields_no_findings() {
        let process = [
            ("XDG_CURRENT_DESKTOP", "ubuntu:GNOME"),
            ("XDG_SESSION_DESKTOP", "ubuntu"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let snapshot = fixture_snapshot(&process, Some(&process));
        assert!(evaluate(&snapshot).is_empty());
    }

    #[test]
    fn missing_xdg_current_desktop_fires_env001_only() {
        let process = [
            ("XDG_SESSION_DESKTOP", "gnome"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let snapshot = fixture_snapshot(&process, Some(&process));
        assert_eq!(ids(&evaluate(&snapshot)), ["ENV001"]);
    }

    #[test]
    fn wayland_without_wayland_display_fires_env003_only() {
        let process = [
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_TYPE", "wayland"),
        ];
        let snapshot = fixture_snapshot(&process, Some(&process));
        assert_eq!(ids(&evaluate(&snapshot)), ["ENV003"]);
    }

    #[test]
    fn x11_session_without_display_is_recognized_and_quiet() {
        // Sanity: ENV003 must not fire outside Wayland sessions.
        let process = [
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_TYPE", "x11"),
        ];
        let snapshot = fixture_snapshot(&process, Some(&process));
        assert!(evaluate(&snapshot).is_empty());
    }

    #[test]
    fn activation_mismatch_fires_env004_with_details() {
        let process = [
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let activation = [("XDG_CURRENT_DESKTOP", "KDE"), ("XDG_SESSION_TYPE", "x11")];
        let snapshot = fixture_snapshot(&process, Some(&activation));
        let findings = evaluate(&snapshot);
        assert_eq!(ids(&findings), ["ENV004"]);
        crate::rules::contract::assert_contract(&findings);
        assert!(findings[0].summary.contains("XDG_CURRENT_DESKTOP"));
        assert!(findings[0].summary.contains("XDG_SESSION_TYPE"));
    }

    #[test]
    fn unavailable_activation_comparison_suppresses_env004() {
        // Session context is complete so only the suppressed ENV004 could fire.
        let process = [
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ];
        let snapshot = fixture_snapshot(&process, None::<&[(&'static str, &'static str); 0]>);
        assert!(evaluate(&snapshot).is_empty());
    }

    #[test]
    fn unknown_session_type_fires_env002() {
        let process = [
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("XDG_SESSION_TYPE", "mir"),
        ];
        let snapshot = fixture_snapshot(&process, Some(&process));
        let findings = evaluate(&snapshot);
        assert_eq!(ids(&findings), ["ENV002"]);
        crate::rules::contract::assert_contract(&findings);
    }

    #[test]
    fn evaluation_is_deterministic_across_runs() {
        let process = [
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_CURRENT_DESKTOP", "KDE"),
        ];
        let activation = [("XDG_CURRENT_DESKTOP", "GNOME")];
        let first = evaluate(&fixture_snapshot(&process, Some(&activation)));
        let second = evaluate(&fixture_snapshot(&process, Some(&activation)));
        assert_eq!(first, second);
    }

    #[test]
    fn rules_expose_stable_ids() {
        assert_eq!(Env001.id(), "ENV001");
        assert_eq!(Env002.id(), "ENV002");
        assert_eq!(Env003.id(), "ENV003");
        assert_eq!(Env004.id(), "ENV004");
    }
}
