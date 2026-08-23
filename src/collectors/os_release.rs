use std::collections::BTreeMap;
use std::fs;

use crate::model::environment::SystemInfo;
use crate::model::section::Section;

const OS_RELEASE_PATH: &str = "/etc/os-release";

/// Parse `/etc/os-release` content: `KEY=VALUE` pairs with optional quoting.
/// Comments and blank lines are ignored; later assignments win.
#[must_use]
pub fn parse(text: &str) -> SystemInfo {
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        fields.insert(key.to_owned(), unquote(value.trim()));
    }
    SystemInfo {
        id: fields.get("ID").cloned(),
        name: fields.get("NAME").cloned(),
        pretty_name: fields.get("PRETTY_NAME").cloned(),
        version_id: fields.get("VERSION_ID").cloned(),
    }
}

/// Collect the operating-system identity from `/etc/os-release`.
pub fn collect() -> Section<SystemInfo> {
    match fs::read_to_string(OS_RELEASE_PATH) {
        Ok(text) => Section::available(parse(&text)),
        Err(err) => Section::unavailable(format!("cannot read {OS_RELEASE_PATH}: {err}")),
    }
}

/// Strip one pair of matching surrounding quotes, if present.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return value[1..value.len() - 1].to_owned();
    }
    value.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{parse, unquote};

    #[test]
    fn parses_quoted_fields_and_ignores_comments() {
        let info = parse(
            "# comment\nNAME=\"Ubuntu Linux\"\nID=ubuntu\nVERSION_ID='24.04'\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\n\n",
        );
        assert_eq!(info.id.as_deref(), Some("ubuntu"));
        assert_eq!(info.name.as_deref(), Some("Ubuntu Linux"));
        assert_eq!(info.version_id.as_deref(), Some("24.04"));
        assert_eq!(info.pretty_name.as_deref(), Some("Ubuntu 24.04 LTS"));
    }

    #[test]
    fn missing_fields_stay_none_and_later_assignment_wins() {
        let info = parse("ID=fedora\nID=ubuntu\n");
        assert_eq!(info.id.as_deref(), Some("ubuntu"));
        assert!(info.name.is_none());
        assert!(info.version_id.is_none());
        assert!(info.pretty_name.is_none());
    }

    #[test]
    fn unquote_only_strips_matching_outer_pair() {
        assert_eq!(unquote("\"quoted\""), "quoted");
        assert_eq!(unquote("'single'"), "single");
        assert_eq!(unquote("plain"), "plain");
        assert_eq!(unquote("\"mixed'"), "\"mixed'");
    }
}
