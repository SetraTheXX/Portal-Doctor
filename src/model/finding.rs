use std::fmt;

use serde::Serialize;

use crate::model::evidence::Evidence;

/// Impact severity of a finding (PRD §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // variants are constructed by Phase 1 rules
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

impl Severity {
    /// Stable uppercase label.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
            Self::Critical => "CRITICAL",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How strongly collected evidence supports a finding (PRD §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // variants are constructed by Phase 1 rules
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    /// Stable uppercase label.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
        }
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One deterministic diagnostic result (PRD §8 finding contract).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Stable rule identifier, e.g. `ENV001`.
    pub id: String,
    /// Impact severity.
    pub severity: Severity,
    /// Support strength of the collected evidence.
    pub confidence: Confidence,
    /// Short human title.
    pub title: String,
    /// One-paragraph summary.
    pub summary: String,
    /// Detailed explanation of the finding.
    pub explanation: String,
    /// Structured evidence backing the finding.
    pub evidence: Vec<Evidence>,
    /// Consequence when the finding applies; absent when not applicable.
    pub impact: Option<String>,
    /// Suggested next steps, in order.
    pub recommendation: Vec<String>,
    /// Collector/rule subsystem that produced this finding.
    pub source_component: String,
}

#[cfg(test)]
mod tests {
    use super::{Confidence, Finding, Severity};
    use crate::model::evidence::Evidence;
    use serde_json::json;

    #[test]
    fn severity_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_value(Severity::Critical).unwrap(),
            json!("critical")
        );
    }

    #[test]
    fn severity_display_is_uppercase() {
        assert_eq!(Severity::Warning.to_string(), "WARNING");
        assert_eq!(Confidence::High.to_string(), "HIGH");
    }

    #[test]
    fn finding_serializes_the_full_contract() {
        let finding = Finding {
            id: "ENV001".to_owned(),
            severity: Severity::Warning,
            confidence: Confidence::High,
            title: "Test finding".to_owned(),
            summary: "Summary".to_owned(),
            explanation: "Explanation".to_owned(),
            evidence: vec![Evidence::EnvironmentMismatch],
            impact: Some("Impact".to_owned()),
            recommendation: vec!["Do something".to_owned()],
            source_component: "environment".to_owned(),
        };
        let value = serde_json::to_value(&finding).unwrap();
        for key in [
            "id",
            "severity",
            "confidence",
            "title",
            "summary",
            "explanation",
            "evidence",
            "impact",
            "recommendation",
            "source_component",
        ] {
            assert!(value.get(key).is_some(), "missing contract field {key}");
        }
    }
}
