use std::ffi::OsStr;
use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Map, Value};

use crate::collectors::timeouts::{BoundedOutput, NORMAL_RUNTIME_QUERY, output_bounded_with_limit};
use crate::model::journal::{
    JOURNAL_MODEL_VERSION, JournalClassification, JournalEntry, JournalInfo, JournalMatchState,
};
use crate::model::section::Section;

/// Current-boot journal window used by the opt-in collector.
pub const JOURNAL_WINDOW_MINUTES: u64 = 30;
/// Maximum number of JSON records requested from `journalctl`.
pub const JOURNAL_MAX_ENTRIES: usize = 80;

const JOURNAL_COMMAND: &str = "journalctl";
const JOURNAL_OUTPUT_LIMIT: usize = 512 * 1024;
const MAX_MESSAGE_LENGTH: usize = 240;
const MAX_PRIORITY: u8 = 4;
const ALLOWED_FIXED_UNITS: [&str; 4] = [
    "pipewire.service",
    "pipewire-pulse.service",
    "wireplumber.service",
    "xdg-desktop-portal.service",
];

/// Collect sanitized, classified evidence from the current user session.
/// Nothing is queried unless the caller explicitly opts into journal data.
pub fn collect(units: &[String]) -> Section<JournalInfo> {
    collect_with_timeout(OsStr::new(JOURNAL_COMMAND), units, NORMAL_RUNTIME_QUERY)
}

fn collect_with_timeout(
    program: &OsStr,
    units: &[String],
    timeout: Duration,
) -> Section<JournalInfo> {
    let units = allowlisted_units(units);
    if units.is_empty() {
        return Section::unsupported("no allowlisted journal units requested");
    }

    let mut command = Command::new(program);
    command.args([
        "--user",
        "--boot=0",
        "--since=-30min",
        "--output=json",
        "--no-pager",
        "--quiet",
        "--lines=80",
    ]);
    for unit in &units {
        command.args(["--unit", unit]);
    }
    command.stderr(Stdio::piped());

    match output_bounded_with_limit(timeout, JOURNAL_OUTPUT_LIMIT, command) {
        Err(err) => spawn_failure("journalctl", &err),
        Ok(BoundedOutput::TimedOut) => {
            Section::timed_out(format!("journalctl did not finish within {timeout:?}"))
        }
        Ok(BoundedOutput::OutputLimitExceeded) => Section::unavailable(format!(
            "journalctl output exceeded the {} KiB safety limit",
            JOURNAL_OUTPUT_LIMIT / 1024
        )),
        Ok(BoundedOutput::Completed(output)) if !output.status.success() => {
            command_failure("journalctl", &output)
        }
        Ok(BoundedOutput::Completed(output)) => match String::from_utf8(output.stdout) {
            Ok(text) => match parse_journal_output(&text, &units) {
                Ok(info) => Section::available(info),
                Err(message) => Section::parse_error(format!("journalctl JSON: {message}")),
            },
            Err(_) => Section::parse_error("journalctl returned non-UTF-8 output"),
        },
    }
}

fn allowlisted_units(units: &[String]) -> Vec<String> {
    let mut selected: Vec<String> = units
        .iter()
        .filter(|unit| is_allowlisted_unit(unit))
        .cloned()
        .collect();
    selected.sort_unstable();
    selected.dedup();
    selected
}

fn is_allowlisted_unit(unit: &str) -> bool {
    ALLOWED_FIXED_UNITS.contains(&unit)
        || (unit.starts_with("xdg-desktop-portal-") && unit.ends_with(".service"))
}

fn spawn_failure<T>(label: &str, err: &std::io::Error) -> Section<T> {
    match err.kind() {
        ErrorKind::NotFound => Section::unsupported(format!("{label} is not installed")),
        ErrorKind::PermissionDenied => {
            Section::permission_denied(format!("cannot execute {label}: permission denied"))
        }
        _ => Section::unavailable(format!("cannot execute {label}: {}", err.kind())),
    }
}

fn command_failure<T>(label: &str, output: &std::process::Output) -> Section<T> {
    let permission_denied = output.status.code() == Some(126)
        || String::from_utf8_lossy(&output.stderr)
            .to_ascii_lowercase()
            .contains("permission denied");
    if permission_denied {
        return Section::permission_denied(format!("{label} was denied permission to run"));
    }
    if output.status.code() == Some(127) {
        return Section::unsupported(format!("{label} command is unavailable"));
    }
    Section::unavailable(format!("{label} exited with {}", output.status))
}

/// Parse one JSON object per line from `journalctl --output=json`.
#[must_use = "the parse result contains normalized journal evidence"]
fn parse_journal_output(text: &str, allowed_units: &[String]) -> Result<JournalInfo, String> {
    let mut scanned_entry_count = 0;
    let mut ignored_entry_count = 0;
    let mut insufficient_entry_count = 0;
    let mut entries = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        scanned_entry_count += 1;
        if scanned_entry_count > JOURNAL_MAX_ENTRIES {
            ignored_entry_count += 1;
            continue;
        }
        let value: Value =
            serde_json::from_str(line).map_err(|err| format!("line {}: {err}", line_number + 1))?;
        match parse_entry(&value, allowed_units)? {
            ParsedEntry::Relevant(entry) => entries.push(entry),
            ParsedEntry::Irrelevant => ignored_entry_count += 1,
            ParsedEntry::Insufficient => {
                ignored_entry_count += 1;
                insufficient_entry_count += 1;
            }
        }
    }

    let match_state = if entries.is_empty() {
        if insufficient_entry_count == 0 {
            JournalMatchState::NoRelevantEvidence
        } else {
            JournalMatchState::InsufficientEvidence
        }
    } else {
        JournalMatchState::Matched
    };
    Ok(JournalInfo {
        model_version: JOURNAL_MODEL_VERSION,
        window_minutes: JOURNAL_WINDOW_MINUTES,
        max_entries: JOURNAL_MAX_ENTRIES,
        scanned_entry_count,
        ignored_entry_count,
        match_state,
        entries,
    })
}

enum ParsedEntry {
    Relevant(JournalEntry),
    Irrelevant,
    Insufficient,
}

fn parse_entry(value: &Value, allowed_units: &[String]) -> Result<ParsedEntry, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "journal record is not an object".to_owned())?;
    let Some(unit) = first_string(object, &["_SYSTEMD_USER_UNIT", "_SYSTEMD_UNIT"]) else {
        return Ok(ParsedEntry::Insufficient);
    };
    if !allowed_units.iter().any(|allowed| allowed == unit) {
        return Ok(ParsedEntry::Irrelevant);
    }
    let Some(priority) = object.get("PRIORITY").and_then(parse_priority) else {
        return Ok(ParsedEntry::Insufficient);
    };
    if priority > MAX_PRIORITY {
        return Ok(ParsedEntry::Irrelevant);
    }
    let Some(message) = object.get("MESSAGE").and_then(Value::as_str) else {
        return Ok(ParsedEntry::Insufficient);
    };
    let Some(classification) = classify_message(message, unit) else {
        return Ok(ParsedEntry::Irrelevant);
    };
    let message = sanitize_message(message);
    if message.is_empty() {
        return Ok(ParsedEntry::Insufficient);
    }
    Ok(ParsedEntry::Relevant(JournalEntry {
        unit: unit.to_owned(),
        priority,
        classification,
        message,
    }))
}

fn first_string<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .filter(|value| !value.is_empty())
}

fn parse_priority(value: &Value) -> Option<u8> {
    value
        .as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn classify_message(message: &str, unit: &str) -> Option<JournalClassification> {
    let lower = message.to_ascii_lowercase();
    if !has_error_marker(&lower) {
        return None;
    }
    if lower.contains("screencast") || lower.contains("screen cast") {
        Some(JournalClassification::ScreenCast)
    } else if lower.contains("wireplumber") {
        Some(JournalClassification::WirePlumber)
    } else if lower.contains("pipewire") {
        Some(JournalClassification::PipeWire)
    } else if lower.contains("xdg-desktop-portal") || lower.contains("portal") {
        Some(JournalClassification::Portal)
    } else if unit == "wireplumber.service" {
        Some(JournalClassification::WirePlumber)
    } else if unit == "pipewire.service" || unit == "pipewire-pulse.service" {
        Some(JournalClassification::PipeWire)
    } else if unit == "xdg-desktop-portal.service"
        || (unit.starts_with("xdg-desktop-portal-") && unit.ends_with(".service"))
    {
        Some(JournalClassification::Portal)
    } else {
        None
    }
}

fn has_error_marker(message: &str) -> bool {
    [
        "error",
        "failed",
        "failure",
        "unavailable",
        "unreachable",
        "timed out",
        "timeout",
        "refused",
        "cannot",
        "could not",
        "unable",
        "not found",
        "no such",
    ]
    .iter()
    .any(|marker| message.contains(marker))
}

fn sanitize_message(message: &str) -> String {
    let normalized: String = message
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let normalized = redact_labeled_values(&redact_at_identities(&normalized));
    let normalized = redact_absolute_paths(&normalized);
    let compact = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_message(compact)
}

fn redact_at_identities(text: &str) -> String {
    redact_tokens(text, |token| {
        let Some((left, right)) = token.split_once('@') else {
            return false;
        };
        !left.is_empty() && !right.is_empty()
    })
}

fn redact_labeled_values(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        if !is_token_character(chars[index]) {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() && is_token_character(chars[index]) {
            index += 1;
        }
        let token: String = chars[start..index].iter().collect();
        let Some((key, _)) = token.split_once('=') else {
            output.push_str(&token);
            continue;
        };
        if ["user", "username", "host", "hostname", "machine"]
            .iter()
            .any(|candidate| key.eq_ignore_ascii_case(candidate))
        {
            output.push_str(key);
            output.push_str("=<redacted>");
        } else {
            output.push_str(&token);
        }
    }
    output
}

fn redact_tokens<F>(text: &str, should_redact: F) -> String
where
    F: Fn(&str) -> bool,
{
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        if !is_token_character(chars[index]) {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() && is_token_character(chars[index]) {
            index += 1;
        }
        let token: String = chars[start..index].iter().collect();
        if should_redact(&token) {
            output.push_str("<identity>");
        } else {
            output.push_str(&token);
        }
    }
    output
}

fn is_token_character(character: char) -> bool {
    !character.is_whitespace()
        && !matches!(
            character,
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '"' | '\''
        )
}

fn redact_absolute_paths(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let path_start = chars[index] == '/'
            && (index == 0 || is_path_boundary(chars[index - 1]))
            && chars.get(index + 1) != Some(&'/');
        if !path_start {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        index += 1;
        while index < chars.len()
            && !chars[index].is_whitespace()
            && !matches!(
                chars[index],
                ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '"' | '\''
            )
        {
            index += 1;
        }
        output.push_str("<path>");
    }
    output
}

fn is_path_boundary(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '=' | ':' | ',' | '(' | '[' | '{' | '"' | '\'' | '|'
        )
}

fn truncate_message(message: String) -> String {
    if message.chars().count() <= MAX_MESSAGE_LENGTH {
        return message;
    }
    let mut truncated: String = message
        .chars()
        .take(MAX_MESSAGE_LENGTH.saturating_sub(1))
        .collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::{
        JOURNAL_MAX_ENTRIES, collect_with_timeout, is_allowlisted_unit, parse_journal_output,
    };
    use crate::model::journal::{JournalClassification, JournalMatchState};
    use crate::model::status::CollectorState;
    use std::ffi::OsStr;
    use std::time::Duration;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn allowlist_accepts_only_portal_and_media_units() {
        assert!(is_allowlisted_unit("pipewire.service"));
        assert!(is_allowlisted_unit("xdg-desktop-portal-gnome.service"));
        assert!(!is_allowlisted_unit("sshd.service"));
        assert!(!is_allowlisted_unit("xdg-desktop-portal-gnome.socket"));
    }

    #[test]
    fn parses_classified_entries_and_redacts_private_content() {
        let text = concat!(
            r#"{"_SYSTEMD_USER_UNIT":"pipewire.service","PRIORITY":"3","MESSAGE":"PipeWire failed for /run/user/1000/pipewire-0 user=tuncay host=setra-dev-node tuncay@setra-dev-node","_HOSTNAME":"private-host"}"#,
            "\n",
            r#"{"_SYSTEMD_USER_UNIT":"pipewire.service","PRIORITY":"6","MESSAGE":"PipeWire is ready"}"#,
            "\n",
            r#"{"_SYSTEMD_USER_UNIT":"pipewire.service","PRIORITY":"3","MESSAGE":"routine startup completed"}"#,
            "\n",
            r#"{"_SYSTEMD_USER_UNIT":"sshd.service","PRIORITY":"3","MESSAGE":"PipeWire failed elsewhere"}"#,
            "\n",
        );
        let allowed = vec!["pipewire.service".to_owned()];
        let info = parse_journal_output(text, &allowed).unwrap();
        assert_eq!(info.match_state, JournalMatchState::Matched);
        assert_eq!(info.scanned_entry_count, 4);
        assert_eq!(info.entries.len(), 1);
        assert_eq!(
            info.entries[0].classification,
            JournalClassification::PipeWire
        );
        assert!(!info.entries[0].message.contains("tuncay"));
        assert!(!info.entries[0].message.contains("setra-dev-node"));
        assert!(!info.entries[0].message.contains("/run/user"));
        assert!(
            !serde_json::to_string(&info)
                .unwrap()
                .contains("private-host")
        );
    }

    #[test]
    fn classifies_relevant_units_without_requiring_component_names_in_messages() {
        let text = concat!(
            r#"{"_SYSTEMD_USER_UNIT":"xdg-desktop-portal.service","PRIORITY":3,"MESSAGE":"Failed to activate backend"}"#,
            "\n",
            r#"{"_SYSTEMD_USER_UNIT":"wireplumber.service","PRIORITY":3,"MESSAGE":"Failed to connect to session manager"}"#,
            "\n",
            r#"{"_SYSTEMD_USER_UNIT":"pipewire-pulse.service","PRIORITY":3,"MESSAGE":"Connection refused"}"#,
            "\n",
        );
        let allowed = vec![
            "pipewire-pulse.service".to_owned(),
            "wireplumber.service".to_owned(),
            "xdg-desktop-portal.service".to_owned(),
        ];
        let info = parse_journal_output(text, &allowed).unwrap();
        assert_eq!(info.entries.len(), 3);
        assert_eq!(
            info.entries
                .iter()
                .map(|entry| entry.classification)
                .collect::<Vec<_>>(),
            vec![
                JournalClassification::Portal,
                JournalClassification::WirePlumber,
                JournalClassification::PipeWire,
            ]
        );
    }

    #[test]
    fn sanitizes_colon_prefixed_paths_and_caps_long_messages() {
        let path = "path:/home/tuncay/private-file ";
        let message = format!("PipeWire failed {path}{}", "x".repeat(400));
        let text = serde_json::json!({
            "_SYSTEMD_USER_UNIT": "pipewire.service",
            "PRIORITY": 3,
            "MESSAGE": message,
        })
        .to_string();
        let info = parse_journal_output(&text, &["pipewire.service".to_owned()]).unwrap();
        let retained = &info.entries[0].message;
        assert!(!retained.contains("/home/tuncay"));
        assert!(retained.chars().count() <= 240);
        assert!(retained.ends_with('…'));
    }

    #[test]
    fn represents_empty_and_insufficient_evidence_without_guessing() {
        let allowed = vec!["pipewire.service".to_owned()];
        let empty = parse_journal_output("", &allowed).unwrap();
        assert_eq!(empty.match_state, JournalMatchState::NoRelevantEvidence);
        assert!(empty.entries.is_empty());

        let insufficient = parse_journal_output(
            r#"{"_SYSTEMD_USER_UNIT":"pipewire.service","MESSAGE":"PipeWire failed"}"#,
            &allowed,
        )
        .unwrap();
        assert_eq!(
            insufficient.match_state,
            JournalMatchState::InsufficientEvidence
        );
        assert!(insufficient.entries.is_empty());
    }

    #[test]
    fn rejects_malformed_json_records() {
        let allowed = vec!["pipewire.service".to_owned()];
        assert!(parse_journal_output("not-json\n", &allowed).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn bounded_collector_classifies_success_and_timeout() {
        let success = fixture("journal-command.sh");
        let units = vec!["pipewire.service".to_owned()];
        let section = collect_with_timeout(success.as_os_str(), &units, Duration::from_secs(1));
        assert_eq!(section.status, CollectorState::Available);
        assert_eq!(section.value.unwrap().entries.len(), 1);

        let timeout = fixture("slow-command.sh");
        let section = collect_with_timeout(timeout.as_os_str(), &units, Duration::from_millis(120));
        assert_eq!(section.status, CollectorState::TimedOut);
    }

    #[test]
    fn missing_command_is_unsupported() {
        let units = vec!["pipewire.service".to_owned()];
        let section = collect_with_timeout(
            OsStr::new("/definitely/missing/portaldoctor-journalctl"),
            &units,
            Duration::from_millis(200),
        );
        assert_eq!(section.status, CollectorState::Unsupported);
    }

    #[test]
    fn parser_enforces_entry_limit() {
        let allowed = vec!["pipewire.service".to_owned()];
        let line =
            r#"{"_SYSTEMD_USER_UNIT":"pipewire.service","PRIORITY":"6","MESSAGE":"routine"}"#;
        let text = std::iter::repeat_n(line, JOURNAL_MAX_ENTRIES + 2)
            .collect::<Vec<_>>()
            .join("\n");
        let info = parse_journal_output(&text, &allowed).unwrap();
        assert_eq!(info.scanned_entry_count, JOURNAL_MAX_ENTRIES + 2);
        assert_eq!(info.ignored_entry_count, JOURNAL_MAX_ENTRIES + 2);
    }
}
