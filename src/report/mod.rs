mod json;
mod terminal;

use serde::Serialize;

use crate::model::finding::Finding;
use crate::model::snapshot::{SNAPSHOT_SCHEMA_VERSION, Snapshot};

pub use json::JsonRenderer;
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

/// Renders a finished report into its final textual form.
pub trait Renderer {
    /// Render `report` into text ready for stdout.
    fn render(&self, report: &Report) -> String;
}

#[cfg(test)]
mod tests {
    use super::{JsonRenderer, Renderer, Report, TerminalRenderer};
    use crate::model::snapshot::Snapshot;
    use serde_json::json;

    #[test]
    fn new_locks_top_level_schema_version() {
        let report = Report::new(Snapshot::new(1, 42), Vec::new(), "0.1.0");
        assert_eq!(report.schema_version, 1);
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(
            value,
            json!({
                "schema_version": 1,
                "portaldoctor_version": "0.1.0",
                "snapshot": { "schema_version": 1, "collected_at": 42 },
                "findings": []
            })
        );
    }

    #[test]
    fn renderers_emit_findings_state() {
        let report = Report::new(Snapshot::new(1, 42), Vec::new(), "0.1.0");
        let terminal = TerminalRenderer.render(&report);
        assert!(terminal.contains("Findings: none detected."));
        let json = JsonRenderer.render(&report);
        assert!(json.contains("\"findings\": []"));
    }
}
