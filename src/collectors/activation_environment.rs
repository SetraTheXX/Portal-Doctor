use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::collectors::environment::ALLOWLISTED_VARIABLES;
use crate::model::section::Section;

/// Bounded wait for `systemctl` so a wedged `systemd --user` cannot hang the
/// whole run (PRD REL-003: no unbounded commands).
const SYSTEMCTL_TIMEOUT: Duration = Duration::from_secs(2);

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
    let mut child = match Command::new("systemctl")
        .args(["--user", "show-environment"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Section::unsupported("systemctl is not installed".to_owned());
        }
        Err(err) => {
            return Section::unavailable(format!("cannot run systemctl: {err}"));
        }
    };

    let deadline = Instant::now() + SYSTEMCTL_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("systemctl did not finish within 2s".to_owned());
            }
            // The environment dump is small; reading stdout only after exit
            // cannot fill the pipe buffer.
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(err) => break Err(format!("cannot poll systemctl: {err}")),
        }
    };

    match status {
        Ok(status) if status.success() => {
            let Some(mut stdout) = child.stdout.take() else {
                return Section::unavailable("systemctl produced no output stream".to_owned());
            };
            match std::io::read_to_string(&mut stdout) {
                Ok(text) => Section::available(parse_show_environment(&text)),
                Err(err) => Section::unavailable(format!("cannot read systemctl output: {err}")),
            }
        }
        Ok(status) => Section::unavailable(format!("systemctl exited with {status}")),
        Err(message) => Section::timed_out(message),
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
