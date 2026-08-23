use crate::report::{Renderer, Report};

/// Renderer that emits the v1 `JSON` report (PRD §7.4).
pub struct JsonRenderer;

impl Renderer for JsonRenderer {
    fn render(&self, report: &Report, _verbose: bool) -> String {
        serde_json::to_string_pretty(report).expect("plain-data report serialization cannot fail")
    }
}
