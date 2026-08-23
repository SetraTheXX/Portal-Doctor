use std::fmt::Write as _;

use crate::report::{Renderer, Report};

/// Renderer that emits concise terminal text (PRD §7.1).
pub struct TerminalRenderer;

impl Renderer for TerminalRenderer {
    fn render(&self, report: &Report) -> String {
        let mut out = String::new();
        writeln!(out, "PortalDoctor {}", report.portaldoctor_version)
            .expect("writing to a String cannot fail");
        writeln!(out, "Snapshot schema v{}", report.schema_version)
            .expect("writing to a String cannot fail");
        out.push('\n');
        if report.findings.is_empty() {
            out.push_str("Findings: none detected.\n");
        } else {
            writeln!(out, "Findings: {}", report.findings.len())
                .expect("writing to a String cannot fail");
            for finding in &report.findings {
                writeln!(
                    out,
                    "  [{}] {} ({})",
                    finding.severity, finding.title, finding.id
                )
                .expect("writing to a String cannot fail");
                if let Some(impact) = &finding.impact {
                    writeln!(out, "    impact: {impact}").expect("writing to a String cannot fail");
                }
            }
        }
        out
    }
}
