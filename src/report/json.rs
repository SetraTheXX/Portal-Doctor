use crate::report::{Renderer, Report, ShareableReport};

/// Renderer that emits the v1 `JSON` report (PRD §7.4).
pub struct JsonRenderer;

impl Renderer for JsonRenderer {
    fn render(&self, report: &Report, _verbose: bool) -> String {
        serde_json::to_string_pretty(report).expect("plain-data report serialization cannot fail")
    }
}

/// Renderer for the explicit privacy-aware report document.
pub struct ShareableJsonRenderer;

impl ShareableJsonRenderer {
    /// Serialize the redacted report with its document and privacy metadata.
    #[must_use]
    pub fn render(report: &ShareableReport) -> String {
        serde_json::to_string_pretty(report).expect("shareable report serialization cannot fail")
    }
}
