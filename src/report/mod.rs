mod json;
pub mod portal;
pub mod terminal;

use serde::Serialize;

use crate::model::finding::Finding;
use crate::model::snapshot::{SNAPSHOT_SCHEMA_VERSION, Snapshot};

pub use json::JsonRenderer;
pub use portal::{PortalExplainRenderer, PortalListRenderer, PortalRoutesRenderer};
pub use terminal::TerminalRenderer;

/// Top-level run output; matches the v1 `JSON` contract (PRD §7.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    use super::{JsonRenderer, Renderer, Report, TerminalRenderer};
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
    fn json_renderer_ignores_verbose_and_stays_parseable() {
        let report = Report::new(empty_snapshot(), Vec::new(), "0.1.0");
        let text = JsonRenderer.render(&report, true);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["snapshot"]["collected_at"], json!(42));
    }
}
