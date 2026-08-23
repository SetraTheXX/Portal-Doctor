use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::process::{Command, Stdio};

use crate::collectors::environment::ALLOWLISTED_VARIABLES;
use crate::model::section::Section;

/// Bounded wait for `systemctl` so a wedged `systemd --user` cannot hang the
/// whole run (PRD REL-003: no unbounded commands).
const SYSTEMCTL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Parse `systemctl --user show-environment` output. Only allowlisted keys
/// are retained; quoting and non-assignment lines are tolerated.
#[must_use]
pub fn parse_show_environment(text: &str) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !ALLOWLISTED_VARIABLES.contains(&key) {
            continue;
        }
        let value = unquote(value.trim());
        if !value.is_empty() {
            vars.insert(key.to_owned(), value);
        }
    }
    vars
}

/// Collect the `systemd` user activation environment with a bounded timeout.
/// Status distinguishes missing tooling (`Unsupported`), command failure
/// (`Unavailable`) and timeout (`TimedOut`).
pub fn collect() -> Section<BTreeMap<String, String>> {
    let mut command = Command::new("systemctl");
    command
        .args(["--user", "show-environment"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    match crate::collectors::timeouts::output_bounded(SYSTEMCTL_TIMEOUT, command) {
        Err(err) if err.kind() == ErrorKind::NotFound => {
            Section::unsupported("systemctl is not installed".to_owned())
        }
        Err(err) => Section::unavailable(format!("cannot run systemctl: {err}")),
        Ok(None) => Section::timed_out("systemctl did not finish within 2s"),
        Ok(Some(output)) if !output.status.success() => {
            Section::unavailable(format!("systemctl exited with {}", output.status))
        }
        Ok(Some(output)) => match String::from_utf8(output.stdout) {
            Ok(text) => Section::available(parse_show_environment(&text)),
            Err(err) => Section::unavailable(format!("cannot read systemctl output: {err}")),
        },
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
    use super::{parse_show_environment, unquote};
    use std::collections::BTreeMap;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn keeps_only_allowlisted_keys_and_skips_junk() {
        let text = "XDG_CURRENT_DESKTOP=GNOME\nHOME=/home/secret\nNOT_AN_ASSIGNMENT\nDISPLAY=\" :0 \"\nEMPTY=\n";
        assert_eq!(
            parse_show_environment(text),
            map(&[
                ("XDG_CURRENT_DESKTOP", "GNOME"),
                // Quoted values are unquoted but inner spacing is preserved.
                ("DISPLAY", " :0 "),
            ])
        );
    }

    #[test]
    fn unquote_only_strips_matching_outer_pair() {
        assert_eq!(unquote("'x'"), "x");
        assert_eq!(unquote("\"y\""), "y");
        assert_eq!(unquote("z"), "z");
    }

    #[test]
    fn empty_result_is_valid() {
        assert_eq!(parse_show_environment(""), BTreeMap::new());
    }
}
