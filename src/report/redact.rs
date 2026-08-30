use std::env;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::collectors::environment::ALLOWLISTED_VARIABLES;
use crate::model::finding::Finding;
use crate::model::snapshot::Snapshot;
use crate::report::Report;

/// Version of the explicit, privacy-aware report document.
pub const SHAREABLE_REPORT_VERSION: u32 = 1;

/// Raw diagnostic streams are deliberately not retained by the normalized
/// snapshot. Keeping this policy explicit in the report prevents readers from
/// mistaking a normalized excerpt for a raw dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawDataPolicy {
    Excluded,
}

/// Privacy metadata attached to every shareable report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareablePrivacy {
    pub redacted: bool,
    pub home_normalized: bool,
    pub hostname_suppressed: bool,
    pub raw_journal: RawDataPolicy,
    pub raw_pipewire: RawDataPolicy,
}

/// Explicit report document emitted by `portaldoctor report`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareableReport {
    pub report_version: u32,
    pub schema_version: u32,
    pub portaldoctor_version: String,
    pub privacy: ShareablePrivacy,
    pub snapshot: Snapshot,
    pub findings: Vec<Finding>,
}

impl ShareableReport {
    /// Wrap an already-redacted report with an explicit document version and
    /// privacy declaration.
    #[must_use]
    pub fn from_report(report: &Report, options: &RedactionOptions) -> Self {
        Self {
            report_version: SHAREABLE_REPORT_VERSION,
            schema_version: report.schema_version,
            portaldoctor_version: report.portaldoctor_version.clone(),
            privacy: ShareablePrivacy {
                redacted: true,
                home_normalized: options.home.is_some(),
                hostname_suppressed: options.suppress_hostname,
                raw_journal: RawDataPolicy::Excluded,
                raw_pipewire: RawDataPolicy::Excluded,
            },
            snapshot: report.snapshot.clone(),
            findings: report.findings.clone(),
        }
    }
}

/// Runtime inputs controlling shareable-report redaction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedactionOptions {
    /// Home directory to normalize. The value is never serialized.
    pub home: Option<String>,
    /// Replace the current hostname with `<hostname>` when enabled.
    pub suppress_hostname: bool,
    /// Hostname captured by the caller, kept separate for deterministic tests.
    pub hostname: Option<String>,
}

impl RedactionOptions {
    /// Build redaction inputs from the current process environment.
    #[must_use]
    pub fn from_environment(suppress_hostname: bool) -> Self {
        Self {
            home: env::var("HOME").ok().filter(|value| !value.is_empty()),
            suppress_hostname,
            hostname: suppress_hostname
                .then(|| env::var("HOSTNAME").ok())
                .flatten()
                .filter(|value| !value.is_empty()),
        }
    }
}

/// Redact a normalized report before it is serialized for sharing.
///
/// The transformation operates on the serialized tree and then reconstructs
/// the typed report. This keeps the redaction boundary exhaustive as the
/// snapshot model grows, while preserving the existing v1 report shape for
/// legacy `check --json` output.
#[must_use]
pub fn redact_report(report: &Report, options: &RedactionOptions) -> Report {
    let mut value = serde_json::to_value(report)
        .expect("normalized report serialization should always produce JSON");
    redact_value(&mut value, None, options);
    serde_json::from_value(value).expect("redacted report must retain the report contract")
}

fn redact_value(value: &mut Value, key: Option<&str>, options: &RedactionOptions) {
    match value {
        Value::Object(map) => {
            if key == Some("process") {
                map.retain(|name, _| ALLOWLISTED_VARIABLES.contains(&name.as_str()));
            }
            for (child_key, child_value) in map {
                redact_value(child_value, Some(child_key.as_str()), options);
            }
        }
        Value::Array(items) => {
            if key == Some("entries") {
                items.retain(|item| {
                    item.get("key")
                        .and_then(Value::as_str)
                        .is_none_or(|name| ALLOWLISTED_VARIABLES.contains(&name))
                });
            }
            for item in items {
                redact_value(item, key, options);
            }
        }
        Value::String(text) => {
            *text = redact_text(text, options);
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn redact_text(input: &str, options: &RedactionOptions) -> String {
    let mut output = normalize_home(input, options.home.as_deref());
    output = redact_labeled_values(
        &output,
        &[
            "token",
            "secret",
            "password",
            "passwd",
            "credential",
            "api_key",
            "access_token",
            "authorization",
        ],
        "<redacted>",
    );
    output = redact_labeled_values(&output, &["user", "username"], "<user>");
    if options.suppress_hostname {
        output = redact_labeled_values(&output, &["host", "hostname", "machine"], "<hostname>");
        if let Some(hostname) = options.hostname.as_deref() {
            output = replace_token(&output, hostname, "<hostname>");
        }
    }
    output
}

fn normalize_home(input: &str, home: Option<&str>) -> String {
    let Some(home) = home
        .map(|value| value.trim_end_matches('/'))
        .filter(|value| !value.is_empty())
    else {
        return input.to_owned();
    };
    if home == "/" {
        return input.to_owned();
    }

    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(relative_start) = input[cursor..].find(home) {
        let start = cursor + relative_start;
        let end = start + home.len();
        let before_ok = start == 0
            || input.as_bytes()[start - 1].is_ascii_whitespace()
            || matches!(
                input.as_bytes()[start - 1],
                b'=' | b':' | b'/' | b'(' | b'[' | b'{' | b'"' | b'\'' | b','
            );
        let after_ok = end == input.len()
            || input.as_bytes()[end].is_ascii_whitespace()
            || matches!(
                input.as_bytes()[end],
                b'/' | b'\\' | b'"' | b'\'' | b',' | b')' | b']' | b'}'
            );
        if before_ok && after_ok {
            output.push_str(&input[cursor..start]);
            output.push_str("$HOME");
            cursor = end;
        } else {
            output.push_str(&input[cursor..end]);
            cursor = end;
        }
    }
    output.push_str(&input[cursor..]);
    output
}

fn redact_labeled_values(input: &str, labels: &[&str], replacement: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while cursor < input.len() {
        let Some((_position, label_end)) = find_labeled_value(&lower, input, cursor, labels) else {
            output.push_str(&input[cursor..]);
            break;
        };
        output.push_str(&input[cursor..label_end]);
        let value_start = label_end;
        let quote = input
            .as_bytes()
            .get(value_start)
            .copied()
            .filter(|byte| *byte == b'"' || *byte == b'\'');
        let content_start = if quote.is_some() {
            value_start + 1
        } else {
            value_start
        };
        let content_end = if let Some(quote) = quote {
            input[content_start..]
                .find(char::from(quote))
                .map_or(input.len(), |relative| content_start + relative)
        } else {
            input[content_start..]
                .find(|character: char| {
                    character.is_ascii_whitespace()
                        || matches!(character, ',' | ';' | ')' | ']' | '}')
                })
                .map_or(input.len(), |relative| content_start + relative)
        };
        if let Some(quote) = quote {
            output.push_str(replacement);
            if content_end < input.len() {
                output.push(char::from(quote));
                cursor = content_end + 1;
            } else {
                cursor = content_end;
            }
        } else {
            output.push_str(replacement);
            cursor = content_end;
        }
    }

    output
}

fn find_labeled_value(
    lower: &str,
    original: &str,
    cursor: usize,
    labels: &[&str],
) -> Option<(usize, usize)> {
    let mut result = None;
    for label in labels {
        for separator in ['=', ':'] {
            let pattern = format!("{label}{separator}");
            let mut search_from = cursor;
            while let Some(relative) = lower[search_from..].find(&pattern) {
                let position = search_from + relative;
                let before_ok =
                    position == 0 || !is_label_character(original.as_bytes()[position - 1]);
                if before_ok {
                    let label_end = position + pattern.len();
                    if result.is_none_or(|(best, _)| position < best) {
                        result = Some((position, label_end));
                    }
                    break;
                }
                search_from = position + 1;
            }
        }
    }
    result
}

fn is_label_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn replace_token(input: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return input.to_owned();
    }
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(relative) = input[cursor..].find(needle) {
        let start = cursor + relative;
        let end = start + needle.len();
        let before_ok = start == 0
            || !input[..start]
                .chars()
                .next_back()
                .is_some_and(is_hostname_character);
        let after_ok = end == input.len()
            || !input[end..]
                .chars()
                .next()
                .is_some_and(is_hostname_character);
        if before_ok && after_ok {
            output.push_str(&input[cursor..start]);
            output.push_str(replacement);
            cursor = end;
        } else {
            output.push_str(&input[cursor..end]);
            cursor = end;
        }
    }
    output.push_str(&input[cursor..]);
    output
}

fn is_hostname_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
}

#[cfg(test)]
mod tests {
    use super::{RedactionOptions, normalize_home, redact_labeled_values, replace_token};

    #[test]
    fn normalizes_home_only_at_path_boundaries() {
        assert_eq!(
            normalize_home("/home/alice/.config and /home/alice2", Some("/home/alice")),
            "$HOME/.config and /home/alice2"
        );
    }

    #[test]
    fn redacts_secret_and_hostname_labels() {
        let options = RedactionOptions {
            home: Some("/home/alice".to_owned()),
            suppress_hostname: true,
            hostname: Some("workstation".to_owned()),
        };
        let text = super::redact_text(
            "/home/alice token=abc host=workstation workstation",
            &options,
        );
        assert_eq!(text, "$HOME token=<redacted> host=<hostname> <hostname>");
    }

    #[test]
    fn labeled_redaction_keeps_delimiters() {
        assert_eq!(
            redact_labeled_values("token=abc, next", &["token"], "<redacted>"),
            "token=<redacted>, next"
        );
    }

    #[test]
    fn hostname_token_matching_does_not_touch_longer_tokens() {
        assert_eq!(
            replace_token(
                "node workstation workstation-2",
                "workstation",
                "<hostname>"
            ),
            "node <hostname> workstation-2"
        );
    }
}
