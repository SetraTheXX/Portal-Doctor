mod json;
mod markdown;
pub mod portal;
mod redact;
pub mod terminal;

use serde::{Deserialize, Serialize};

use crate::model::finding::Finding;
use crate::model::snapshot::{PUBLIC_JSON_SCHEMA_VERSION, Snapshot};

pub use json::{JsonRenderer, ShareableJsonRenderer};
pub use markdown::MarkdownRenderer;
pub use portal::{PortalExplainRenderer, PortalListRenderer, PortalRoutesRenderer};
pub use redact::{RedactionOptions, ShareableReport, redact_report};
pub use terminal::TerminalRenderer;

/// Top-level run output; matches the v1 `JSON` contract (PRD §7.4).
#[derive(Debug, Deserialize)]
struct ReportWire {
    schema_version: u32,
    portaldoctor_version: String,
    snapshot: Snapshot,
    findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ReportWire")]
pub struct Report {
    /// Top-level JSON schema version.
    #[serde(serialize_with = "crate::model::snapshot::serialize_public_schema_version")]
    pub schema_version: u32,
    /// `portaldoctor` version that produced this report.
    pub portaldoctor_version: String,
    /// Normalized diagnostic snapshot.
    pub snapshot: Snapshot,
    /// Deterministic findings of this run.
    pub findings: Vec<Finding>,
}

impl TryFrom<ReportWire> for Report {
    type Error = String;

    fn try_from(wire: ReportWire) -> Result<Self, Self::Error> {
        if wire.schema_version != PUBLIC_JSON_SCHEMA_VERSION {
            return Err(format!(
                "unsupported public JSON schema version {}; expected {}",
                wire.schema_version, PUBLIC_JSON_SCHEMA_VERSION
            ));
        }

        Ok(Self {
            schema_version: wire.schema_version,
            portaldoctor_version: wire.portaldoctor_version,
            snapshot: wire.snapshot,
            findings: wire.findings,
        })
    }
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
            schema_version: PUBLIC_JSON_SCHEMA_VERSION,
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
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        JsonRenderer, MarkdownRenderer, RedactionOptions, Renderer, Report, ShareableJsonRenderer,
        ShareableReport, TerminalRenderer, redact_report,
    };
    use crate::model::environment::{
        EnvironmentComparison, EnvironmentInfo, EnvironmentRelation, EnvironmentValue, SearchRoots,
        SessionInfo, SessionType,
    };
    use crate::model::evidence::Evidence;
    use crate::model::finding::{Confidence, Finding, Severity};
    use crate::model::journal::{
        JournalClassification, JournalEntry, JournalInfo, JournalMatchState,
    };
    use crate::model::pipewire::{PipeWireInfo, WirePlumberInfo};
    use crate::model::section::Section;
    use crate::model::snapshot::{PUBLIC_JSON_SCHEMA_VERSION, Snapshot};
    use crate::report::redact::SHAREABLE_REPORT_VERSION;
    use serde_json::json;

    fn empty_snapshot() -> Snapshot {
        let mut snapshot = Snapshot::new(42);
        snapshot.environment = Section::unavailable("test");
        snapshot
    }

    #[test]
    fn new_locks_top_level_schema_version() {
        let report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        assert_eq!(report.schema_version, PUBLIC_JSON_SCHEMA_VERSION);
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["schema_version"], json!(PUBLIC_JSON_SCHEMA_VERSION));
        assert!(value["schema_version"].is_u64());
        assert_eq!(value["portaldoctor_version"], json!("0.1.0"));
        assert_eq!(
            value["snapshot"]["schema_version"],
            json!(PUBLIC_JSON_SCHEMA_VERSION)
        );
        assert_eq!(value["findings"], json!([]));
        assert!(value.get("probes").is_none());

        let fields = value
            .as_object()
            .expect("report serializes as an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = [
            "schema_version",
            "portaldoctor_version",
            "snapshot",
            "findings",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(fields, expected);
    }

    #[test]
    fn documented_schema_version_matches_runtime_constant() {
        let markdown = include_str!("../../docs/json-schema.md");
        let heading_version = markdown
            .lines()
            .find_map(|line| line.strip_prefix("# JSON Schema v"))
            .and_then(|value| value.trim().parse::<u32>().ok())
            .expect("docs must declare a numeric JSON schema version");

        let top_level = markdown
            .split_once("## Top-level contract")
            .map(|(_, section)| section)
            .expect("docs must contain the top-level contract");
        let json_start = top_level
            .find("```json")
            .map(|offset| offset + "```json".len())
            .expect("top-level contract must contain a JSON example");
        let after_fence = &top_level[json_start..];
        let json_end = after_fence
            .find("```")
            .expect("top-level JSON example must be terminated");
        let example: serde_json::Value =
            serde_json::from_str(after_fence[..json_end].trim()).unwrap();
        let example_version = u32::try_from(
            example["schema_version"]
                .as_u64()
                .expect("top-level schema_version must be an integer"),
        )
        .expect("top-level schema_version must fit in u32");

        assert_eq!(heading_version, PUBLIC_JSON_SCHEMA_VERSION);
        assert_eq!(example_version, PUBLIC_JSON_SCHEMA_VERSION);
    }

    #[test]
    fn report_schema_rejects_missing_wrong_type_and_wrong_version() {
        let report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        let base = serde_json::to_value(report).unwrap();

        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove("schema_version");
        assert!(serde_json::from_value::<Report>(missing).is_err());

        let mut wrong_type = base.clone();
        wrong_type["schema_version"] = json!("1");
        assert!(serde_json::from_value::<Report>(wrong_type).is_err());

        let mut wrong_version = base;
        wrong_version["schema_version"] = json!(2);
        assert!(serde_json::from_value::<Report>(wrong_version).is_err());
    }

    #[test]
    fn report_serialization_rejects_mutated_schema_version() {
        let mut report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        report.schema_version = PUBLIC_JSON_SCHEMA_VERSION + 1;
        assert!(serde_json::to_value(report).is_err());
    }

    #[test]
    fn report_schema_accepts_additive_unknown_fields() {
        let report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        let mut value = serde_json::to_value(report).unwrap();
        value["future_optional_field"] = json!(true);
        assert!(serde_json::from_value::<Report>(value).is_ok());
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
                title: "Host workstation token=title-secret needs review".to_owned(),
                summary: "A path /home/alice password=summary-secret was observed".to_owned(),
                explanation: "token=abc host=workstation".to_owned(),
                evidence: Vec::new(),
                impact: Some(
                    "Impact from /home/alice access_token=impact-secret on workstation".to_owned(),
                ),
                recommendation: vec![
                    "Review /home/alice authorization=recommend-secret before sharing".to_owned(),
                ],
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
    fn shareable_constructor_is_a_privacy_boundary_for_both_renderers() {
        let mut report = redaction_fixture_report();
        report.snapshot.journal = Section::available(JournalInfo {
            model_version: 1,
            window_minutes: 30,
            max_entries: 80,
            scanned_entry_count: 1,
            ignored_entry_count: 0,
            match_state: JournalMatchState::Matched,
            entries: vec![JournalEntry {
                unit: "xdg-desktop-portal.service".to_owned(),
                priority: 3,
                classification: JournalClassification::Portal,
                message: "journal path=/home/alice token=journal-secret host=workstation"
                    .to_owned(),
            }],
        });
        report.snapshot.pipewire = Section::available(PipeWireInfo {
            model_version: 1,
            version: Some("1.6.2".to_owned()),
            object_count: 3,
            node_count: 1,
            link_count: 1,
            portal_client_count: 1,
            screen_cast_source_count: 1,
            nodes: Vec::new(),
            links: Vec::new(),
        });

        let document = ShareableReport::from_report(&report, &fixture_options());
        let json = ShareableJsonRenderer::render(&document);
        let markdown = MarkdownRenderer::render(&document, true);

        for sensitive in [
            "/home/alice",
            "workstation",
            "secret-value",
            "title-secret",
            "summary-secret",
            "impact-secret",
            "recommend-secret",
            "journal-secret",
        ] {
            assert!(!json.contains(sensitive), "JSON leaked {sensitive}");
            assert!(!markdown.contains(sensitive), "Markdown leaked {sensitive}");
        }

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let process = &value["snapshot"]["environment"]["value"]["process"];
        assert!(process.get("PATH").is_none());
        assert!(process.get("NOT_ALLOWED").is_none());
        assert_eq!(value["privacy"]["raw_journal"], json!("excluded"));
        assert_eq!(value["privacy"]["raw_pipewire"], json!("excluded"));
        assert!(value["snapshot"]["journal"]["value"].get("raw").is_none());
        assert!(value["snapshot"]["pipewire"]["value"].get("raw").is_none());
        assert!(markdown.contains("| Privacy mode | redacted |"));
        assert!(
            markdown.contains("| Raw journal / PipeWire | excluded; normalized evidence only |")
        );
        assert!(markdown.contains("| Hostname | suppressed |"));
    }

    #[test]
    fn shareable_home_normalization_and_rendering_are_deterministic() {
        let first = ShareableReport::from_report(&redaction_fixture_report(), &fixture_options());
        let second = ShareableReport::from_report(&redaction_fixture_report(), &fixture_options());

        assert_eq!(
            ShareableJsonRenderer::render(&first),
            ShareableJsonRenderer::render(&second)
        );
        assert_eq!(
            MarkdownRenderer::render(&first, true),
            MarkdownRenderer::render(&second, true)
        );
        let value = serde_json::to_value(first).unwrap();
        assert_eq!(
            value["snapshot"]["environment"]["value"]["process"]["XDG_CONFIG_HOME"],
            json!("$HOME/.config")
        );
    }

    #[test]
    fn shareable_privacy_metadata_matches_documented_contract() {
        let options = RedactionOptions {
            home: Some("/home/alice".to_owned()),
            suppress_hostname: false,
            hostname: None,
        };
        let document = ShareableReport::from_report(&redaction_fixture_report(), &options);
        let runtime: serde_json::Value =
            serde_json::from_str(&ShareableJsonRenderer::render(&document)).unwrap();

        let markdown = include_str!("../../docs/json-schema.md");
        let envelope = markdown
            .split_once("## Shareable report envelope")
            .map(|(_, section)| section)
            .expect("docs must contain the shareable envelope");
        let json_start = envelope
            .find("```json")
            .map(|offset| offset + "```json".len())
            .expect("shareable envelope must contain a JSON example");
        let after_fence = &envelope[json_start..];
        let json_end = after_fence
            .find("```")
            .expect("shareable envelope JSON must be terminated");
        let documented: serde_json::Value =
            serde_json::from_str(after_fence[..json_end].trim()).unwrap();

        assert_eq!(runtime["privacy"], documented["privacy"]);
        assert!(runtime["privacy"]["redacted"].is_boolean());
        assert!(runtime["privacy"]["home_normalized"].is_boolean());
        assert!(runtime["privacy"]["hostname_suppressed"].is_boolean());
        assert_eq!(runtime["privacy"]["raw_journal"], json!("excluded"));
        assert_eq!(runtime["privacy"]["raw_pipewire"], json!("excluded"));
    }

    #[test]
    fn shareable_json_has_explicit_version_and_privacy_envelope() {
        let options = fixture_options();
        let redacted = redact_report(&redaction_fixture_report(), &options);
        let document = ShareableReport::from_report(&redacted, &options);
        let text = ShareableJsonRenderer::render(&document);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["report_version"], json!(SHAREABLE_REPORT_VERSION));
        assert!(value["report_version"].is_u64());
        assert_eq!(value["schema_version"], json!(PUBLIC_JSON_SCHEMA_VERSION));
        assert!(value["schema_version"].is_u64());
        assert_eq!(value["privacy"]["redacted"], json!(true));
        assert_eq!(value["privacy"]["raw_journal"], json!("excluded"));
        assert!(
            value["snapshot"]["environment"]["value"]["process"]
                .get("NOT_ALLOWED")
                .is_none()
        );

        let fields = value
            .as_object()
            .expect("shareable report serializes as an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = [
            "report_version",
            "schema_version",
            "portaldoctor_version",
            "privacy",
            "snapshot",
            "findings",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(fields, expected);
    }

    #[test]
    fn shareable_schema_rejects_missing_wrong_type_and_wrong_version() {
        let options = fixture_options();
        let redacted = redact_report(&redaction_fixture_report(), &options);
        let document = ShareableReport::from_report(&redacted, &options);
        let base = serde_json::to_value(&document).unwrap();

        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove("schema_version");
        assert!(serde_json::from_value::<ShareableReport>(missing).is_err());

        let mut wrong_type = base.clone();
        wrong_type["schema_version"] = json!("1");
        assert!(serde_json::from_value::<ShareableReport>(wrong_type).is_err());

        let mut wrong_version = base;
        wrong_version["schema_version"] = json!(2);
        assert!(serde_json::from_value::<ShareableReport>(wrong_version).is_err());
    }

    #[test]
    fn shareable_serialization_rejects_mutated_schema_versions() {
        let options = fixture_options();
        let redacted = redact_report(&redaction_fixture_report(), &options);
        let document = ShareableReport::from_report(&redacted, &options);

        let mut schema_mutation = document.clone();
        schema_mutation.schema_version = PUBLIC_JSON_SCHEMA_VERSION + 1;
        assert!(serde_json::to_value(schema_mutation).is_err());

        let mut report_mutation = document;
        report_mutation.report_version = SHAREABLE_REPORT_VERSION + 1;
        assert!(serde_json::to_value(report_mutation).is_err());
    }

    #[test]
    fn shareable_serialization_rejects_mutated_privacy_metadata() {
        let document =
            ShareableReport::from_report(&redaction_fixture_report(), &fixture_options());

        let mut redaction_mutation = document.clone();
        redaction_mutation.privacy.redacted = false;
        assert!(serde_json::to_value(redaction_mutation).is_err());

        let mut journal_mutation = document.clone();
        journal_mutation.privacy.redacted = false;
        assert!(serde_json::to_value(journal_mutation).is_err());

        let mut false_metadata = serde_json::to_value(&document).unwrap();
        false_metadata["privacy"]["redacted"] = json!(false);
        assert!(serde_json::from_value::<ShareableReport>(false_metadata).is_err());

        let mut unknown_policy = serde_json::to_value(&document).unwrap();
        unknown_policy["privacy"]["raw_pipewire"] = json!("included");
        assert!(serde_json::from_value::<ShareableReport>(unknown_policy).is_err());
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

    #[test]
    fn markdown_report_explains_pipewire_failure_and_next_step() {
        let options = fixture_options();
        let mut report = redaction_fixture_report();
        report.findings = vec![Finding {
            id: "PW001".to_owned(),
            severity: Severity::Warning,
            confidence: Confidence::High,
            title: "PipeWire is unavailable".to_owned(),
            summary: "PipeWire state could not be collected: command unavailable".to_owned(),
            explanation: "ScreenCast needs a reachable PipeWire session.".to_owned(),
            evidence: vec![Evidence::PipeWireState],
            impact: Some("ScreenCast readiness cannot be confirmed.".to_owned()),
            recommendation: vec![
                "Install the package that provides `pw-dump`, then verify the user PipeWire session is running.".to_owned(),
            ],
            source_component: "pipewire".to_owned(),
        }];
        let redacted = redact_report(&report, &options);
        let document = ShareableReport::from_report(&redacted, &options);
        let rendered = MarkdownRenderer::render(&document, false);

        assert!(rendered.contains("PipeWire is unavailable"));
        assert!(rendered.contains("pipewire state"));
        assert!(rendered.contains("Install the package that provides `pw-dump`"));
    }
}
