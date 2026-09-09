use std::fmt;

use serde::{Deserialize, Serialize};

use crate::model::evidence::Evidence;

/// Impact severity of a finding (PRD §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    use std::collections::BTreeSet;

    use super::{Confidence, Finding, Severity};
    use crate::model::evidence::Evidence;
    use serde_json::{Value, json};

    const FINDING_FIELDS: &[&str] = &[
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
    ];

    fn sample_finding(impact: Option<&str>) -> Finding {
        Finding {
            id: "ENV001".to_owned(),
            severity: Severity::Warning,
            confidence: Confidence::High,
            title: "Test finding".to_owned(),
            summary: "Summary".to_owned(),
            explanation: "Explanation".to_owned(),
            evidence: vec![Evidence::EnvironmentMismatch],
            impact: impact.map(str::to_owned),
            recommendation: vec!["Do something".to_owned()],
            source_component: "environment".to_owned(),
        }
    }

    fn findings_json_example(markdown: &str) -> Result<Value, String> {
        let findings_section = markdown
            .split_once("## Findings")
            .map(|(_, section)| section)
            .ok_or_else(|| "missing ## Findings section".to_owned())?;
        let findings_section = findings_section
            .split_once("\n## ")
            .map_or(findings_section, |(section, _)| section);
        let fence_start = findings_section
            .find("```json")
            .ok_or_else(|| "missing Findings JSON example".to_owned())?;
        let json_start = fence_start + "```json".len();
        let after_fence = &findings_section[json_start..];
        let fence_end = after_fence
            .find("```")
            .ok_or_else(|| "unterminated Findings JSON example".to_owned())?;
        let json_text = after_fence[..fence_end].trim();

        serde_json::from_str(json_text)
            .map_err(|error| format!("invalid Findings JSON example: {error}"))
    }

    fn exact_field_set<'a>(value: &'a Value, label: &str) -> Result<BTreeSet<&'a str>, String> {
        value
            .as_object()
            .ok_or_else(|| format!("{label} must be a JSON object"))
            .map(|object| object.keys().map(String::as_str).collect())
    }

    fn require_string(object: &serde_json::Map<String, Value>, field: &str) -> Result<(), String> {
        if object.get(field).is_some_and(Value::is_string) {
            Ok(())
        } else {
            Err(format!("{field} must be a string"))
        }
    }

    fn require_enum_string(
        object: &serde_json::Map<String, Value>,
        field: &str,
        allowed: &[&str],
    ) -> Result<(), String> {
        let value = object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{field} must be a string"))?;
        if allowed.contains(&value) {
            Ok(())
        } else {
            Err(format!("{field} has an undocumented value"))
        }
    }

    fn require_string_array(
        object: &serde_json::Map<String, Value>,
        field: &str,
    ) -> Result<(), String> {
        let values = object
            .get(field)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{field} must be an array"))?;
        if values.iter().all(Value::is_string) {
            Ok(())
        } else {
            Err(format!("{field} must be an array of strings"))
        }
    }

    fn require_string_or_null(
        object: &serde_json::Map<String, Value>,
        field: &str,
    ) -> Result<(), String> {
        match object.get(field) {
            Some(value) if value.is_string() || value.is_null() => Ok(()),
            _ => Err(format!("{field} must be a string or null")),
        }
    }

    fn validate_finding_shape(value: &Value, label: &str) -> Result<(), String> {
        let object = value
            .as_object()
            .ok_or_else(|| format!("{label} must be a JSON object"))?;

        require_string(object, "id")?;
        require_enum_string(
            object,
            "severity",
            &["info", "warning", "error", "critical"],
        )?;
        require_enum_string(object, "confidence", &["low", "medium", "high"])?;
        require_string(object, "title")?;
        require_string(object, "summary")?;
        require_string(object, "explanation")?;
        require_string_array(object, "evidence")?;
        require_string_or_null(object, "impact")?;
        require_string_array(object, "recommendation")?;
        require_string(object, "source_component")?;
        Ok(())
    }

    fn assert_finding_shape_parity(documented: &Value, runtime: &Value) -> Result<(), String> {
        let expected = FINDING_FIELDS.iter().copied().collect::<BTreeSet<_>>();
        let documented_fields = exact_field_set(documented, "documented finding")?;
        let runtime_fields = exact_field_set(runtime, "runtime finding")?;
        if documented_fields != expected {
            return Err(format!(
                "documented finding field set differs from contract: {documented_fields:?}"
            ));
        }
        if runtime_fields != expected {
            return Err(format!(
                "runtime finding field set differs from contract: {runtime_fields:?}"
            ));
        }
        if documented_fields != runtime_fields {
            return Err(format!(
                "documented/runtime finding field sets differ: {documented_fields:?} vs {runtime_fields:?}"
            ));
        }

        validate_finding_shape(documented, "documented finding")?;
        validate_finding_shape(runtime, "runtime finding")?;
        Ok(())
    }

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
        let finding = sample_finding(Some("Impact"));
        let value = serde_json::to_value(&finding).unwrap();
        let actual = value
            .as_object()
            .expect("finding serializes as an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = FINDING_FIELDS.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);

        assert!(value["id"].is_string());
        assert_eq!(value["severity"], json!("warning"));
        assert_eq!(value["confidence"], json!("high"));
        assert!(value["title"].is_string());
        assert!(value["summary"].is_string());
        assert!(value["explanation"].is_string());
        assert!(value["evidence"].is_array());
        assert!(value["impact"].is_string() || value["impact"].is_null());
        assert!(value["recommendation"].is_array());
        assert!(value["source_component"].is_string());
    }

    #[test]
    fn finding_json_matches_bounded_schema_example_and_nullable_impact() {
        let documented = findings_json_example(include_str!("../../docs/json-schema.md")).unwrap();
        let runtime = serde_json::to_value(sample_finding(Some("Impact"))).unwrap();
        assert_finding_shape_parity(&documented, &runtime).unwrap();

        let nullable_runtime = serde_json::to_value(sample_finding(None)).unwrap();
        assert_finding_shape_parity(&documented, &nullable_runtime).unwrap();
    }

    #[test]
    fn finding_schema_parity_rejects_documented_extra_field() {
        let mut documented =
            findings_json_example(include_str!("../../docs/json-schema.md")).unwrap();
        documented
            .as_object_mut()
            .unwrap()
            .insert("extra".to_owned(), json!(true));
        let runtime = serde_json::to_value(sample_finding(Some("Impact"))).unwrap();

        assert!(assert_finding_shape_parity(&documented, &runtime).is_err());
    }

    #[test]
    fn finding_schema_parity_rejects_documented_missing_field() {
        let mut documented =
            findings_json_example(include_str!("../../docs/json-schema.md")).unwrap();
        documented.as_object_mut().unwrap().remove("summary");
        let runtime = serde_json::to_value(sample_finding(Some("Impact"))).unwrap();

        assert!(assert_finding_shape_parity(&documented, &runtime).is_err());
    }

    #[test]
    fn finding_schema_parity_rejects_documented_wrong_type() {
        let mut documented =
            findings_json_example(include_str!("../../docs/json-schema.md")).unwrap();
        documented
            .as_object_mut()
            .unwrap()
            .insert("severity".to_owned(), json!(1));
        let runtime = serde_json::to_value(sample_finding(Some("Impact"))).unwrap();

        assert!(assert_finding_shape_parity(&documented, &runtime).is_err());
    }

    #[test]
    fn finding_schema_parity_rejects_documented_wrong_array_shape() {
        let mut documented =
            findings_json_example(include_str!("../../docs/json-schema.md")).unwrap();
        documented
            .as_object_mut()
            .unwrap()
            .insert("recommendation".to_owned(), json!([{"step": "do it"}]));
        let runtime = serde_json::to_value(sample_finding(Some("Impact"))).unwrap();

        assert!(assert_finding_shape_parity(&documented, &runtime).is_err());
    }
}
