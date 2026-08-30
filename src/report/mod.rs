mod json;
mod markdown;
pub mod portal;
mod redact;
pub mod terminal;

use serde::{Deserialize, Serialize};

use crate::model::finding::Finding;
use crate::model::snapshot::{SNAPSHOT_SCHEMA_VERSION, Snapshot};

pub use json::{JsonRenderer, ShareableJsonRenderer};
pub use markdown::MarkdownRenderer;
pub use portal::{PortalExplainRenderer, PortalListRenderer, PortalRoutesRenderer};
pub use redact::{RedactionOptions, ShareableReport, redact_report};
pub use terminal::TerminalRenderer;

/// Top-level run output; matches the v1 `JSON` contract (PRD §7.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Top-level JSON schema version.
    pub schema_version: u32,
    /// `portaldoctor` version that produced this report.
    pub portaldoctor_version: String,
    /// Normalized diagnostic snapshot.
    pub snapshot: Snapshot,
    /// Deterministic findings of this run.
    pub findings: Vec<Finding>,
}

impl Report {
    /// Build a report carrying the current schema version.
    #[must_use]
    pub fn new(
        snapshot: Snapshot,
        findings: Vec<Finding>,
        portaldoctor_version: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            portaldoctor_version: portaldoctor_version.into(),
            snapshot,
            findings,
        }
    }
}

/// Renders a finished report into its final textual form. The `verbose` flag
/// controls the level of collected detail; machine-readable renderers ignore it.
pub trait Renderer {
    /// Render `report` into text ready for stdout.
    fn render(&self, report: &Report, verbose: bool) -> String;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        JsonRenderer, MarkdownRenderer, RedactionOptions, Renderer, Report, ShareableJsonRenderer,
        ShareableReport, TerminalRenderer, redact_report,
    };
    use crate::model::environment::{
        EnvironmentComparison, EnvironmentInfo, EnvironmentRelation, EnvironmentValue, SearchRoots,
        SessionInfo, SessionType,
    };
    use crate::model::finding::{Confidence, Finding, Severity};
    use crate::model::journal::{
        JournalClassification, JournalEntry, JournalInfo, JournalMatchState,
    };
    use crate::model::pipewire::{PipeWireInfo, WirePlumberInfo};
    use crate::model::section::Section;
    use crate::model::snapshot::Snapshot;
    use serde_json::json;

    fn empty_snapshot() -> Snapshot {
        let mut snapshot = Snapshot::new(42);
        snapshot.environment = Section::unavailable("test");
        snapshot
    }

    #[test]
    fn new_locks_top_level_schema_version() {
        let report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        assert_eq!(report.schema_version, 1);
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["schema_version"], json!(1));
        assert_eq!(value["portaldoctor_version"], json!("0.1.0"));
        assert_eq!(value["snapshot"]["schema_version"], json!(1));
        assert_eq!(value["findings"], json!([]));
    }

    #[test]
    fn terminal_renderer_reports_findings_state() {
        let report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        let terse = TerminalRenderer.render(&report, false);
        assert!(terse.contains("Findings: none detected."));
        let verbose = TerminalRenderer.render(&report, true);
        assert!(verbose.contains("Findings: none detected."));
    }

    #[test]
    fn terminal_renderer_reports_media_health() {
        let mut snapshot = empty_snapshot();
        snapshot.pipewire = Section::available(PipeWireInfo {
            model_version: 1,
            version: Some("1.6.2".to_owned()),
            object_count: 81,
            node_count: 10,
            link_count: 3,
            portal_client_count: 1,
            screen_cast_source_count: 1,
            nodes: Vec::new(),
            links: Vec::new(),
        });
        snapshot.wireplumber = Section::available(WirePlumberInfo {
            model_version: 1,
            pipewire_version: Some("1.6.2".to_owned()),
            wireplumber_client_count: 2,
        });
        let report = Report::new(snapshot, Vec::new(), "0.1.0");
        let text = TerminalRenderer.render(&report, false);
        assert!(text.contains("PipeWire: reachable · 1.6.2 · 81 objects · 10 nodes · 3 links"));
        assert!(text.contains("WirePlumber: reachable · 1.6.2 · 2 client(s)"));
    }

    #[test]
    fn terminal_renderer_reports_sanitized_journal_excerpts_only_in_verbose_mode() {
        let mut snapshot = empty_snapshot();
        snapshot.journal = Section::available(JournalInfo {
            model_version: 1,
            window_minutes: 30,
            max_entries: 80,
            scanned_entry_count: 1,
            ignored_entry_count: 0,
            match_state: JournalMatchState::Matched,
            entries: vec![JournalEntry {
                unit: "pipewire.service".to_owned(),
                priority: 3,
                classification: JournalClassification::PipeWire,
                message: "PipeWire failed for <path> user=<redacted>".to_owned(),
            }],
        });
        let report = Report::new(snapshot, Vec::new(), "0.1.0");
        let terse = TerminalRenderer.render(&report, false);
        assert!(terse.contains("Journal: current boot · 30 min · 1 relevant entry · matched"));
        assert!(terse.contains("use --verbose for sanitized journal excerpts"));
        let verbose = TerminalRenderer.render(&report, true);
        assert!(verbose.contains("PipeWire failure"));
        assert!(verbose.contains("user=<redacted>"));
        assert!(!verbose.contains("/home/tuncay"));
    }

    #[test]
    fn json_renderer_ignores_verbose_and_stays_parseable() {
        let report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        let text = JsonRenderer.render(&report, true);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["snapshot"]["collected_at"], json!(42));
    }

    fn redaction_fixture_report() -> Report {
        let mut snapshot = empty_snapshot();
        snapshot.session = Section::available(SessionInfo {
            current_desktop: Some("GNOME".to_owned()),
            session_desktop: Some("gnome".to_owned()),
            session_type: Some(SessionType::Wayland),
            session_type_raw: Some("wayland".to_owned()),
            wayland_display: Some("wayland-0".to_owned()),
            display: None,
        });
        snapshot.environment = Section::available(EnvironmentInfo {
            process: BTreeMap::from([
                ("PATH".to_owned(), "/home/alice/bin:/usr/bin".to_owned()),
                (
                    "XDG_CONFIG_HOME".to_owned(),
                    "/home/alice/.config".to_owned(),
                ),
                ("NOT_ALLOWED".to_owned(), "secret-value".to_owned()),
            ]),
            search_roots: SearchRoots {
                config_roots: vec!["/home/alice/.config".to_owned()],
                data_roots: vec!["/home/alice/.local/share".to_owned()],
            },
            activation_comparison: EnvironmentComparison {
                performed: true,
                entries: vec![EnvironmentValue {
                    key: "XDG_CONFIG_HOME".to_owned(),
                    process_value: Some("/home/alice/.config".to_owned()),
                    activation_value: Some("/home/alice/.config".to_owned()),
                    relation: EnvironmentRelation::Equal,
                }],
            },
        });
        Report::new(
            snapshot,
            vec![Finding {
                id: "TEST001".to_owned(),
                severity: Severity::Warning,
                confidence: Confidence::High,
                title: "Host workstation needs review".to_owned(),
                summary: "A path /home/alice was observed".to_owned(),
                explanation: "token=abc host=workstation".to_owned(),
                evidence: Vec::new(),
                impact: None,
                recommendation: vec!["Review /home/alice before sharing".to_owned()],
                source_component: "test".to_owned(),
            }],
            "0.1.0",
        )
    }

    fn fixture_options() -> RedactionOptions {
        RedactionOptions {
            home: Some("/home/alice".to_owned()),
            suppress_hostname: true,
            hostname: Some("workstation".to_owned()),
        }
    }

    #[test]
    fn shareable_redaction_enforces_allowlist_and_normalizes_sensitive_values() {
        let report = redaction_fixture_report();
        let redacted = redact_report(&report, &fixture_options());
        let value = serde_json::to_value(redacted).unwrap();
        let process = &value["snapshot"]["environment"]["value"]["process"];
        assert!(process.get("NOT_ALLOWED").is_none());
        assert!(process.get("PATH").is_none());
        assert_eq!(process["XDG_CONFIG_HOME"], json!("$HOME/.config"));
        assert_eq!(
            value["findings"][0]["explanation"],
            json!("token=<redacted> host=<hostname>")
        );
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("/home/alice"));
        assert!(!encoded.contains("workstation"));
        assert!(!encoded.contains("secret-value"));
    }

    #[test]
    fn shareable_json_has_explicit_version_and_privacy_envelope() {
        let options = fixture_options();
        let redacted = redact_report(&redaction_fixture_report(), &options);
        let document = ShareableReport::from_report(&redacted, &options);
        let text = ShareableJsonRenderer::render(&document);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["report_version"], json!(1));
        assert_eq!(value["schema_version"], json!(1));
        assert_eq!(value["privacy"]["redacted"], json!(true));
        assert_eq!(value["privacy"]["raw_journal"], json!("excluded"));
        assert!(
            value["snapshot"]["environment"]["value"]["process"]
                .get("NOT_ALLOWED")
                .is_none()
        );
    }

    #[test]
    fn markdown_renderer_matches_shareable_golden_fixture() {
        let options = RedactionOptions {
            home: Some("/home/alice".to_owned()),
            suppress_hostname: true,
            hostname: Some("workstation".to_owned()),
        };
        let report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        let redacted = redact_report(&report, &options);
        let document = ShareableReport::from_report(&redacted, &options);
        let rendered = MarkdownRenderer::render(&document, false);
        assert_eq!(
            rendered,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/shareable-report.md"
            ))
        );
    }
}
