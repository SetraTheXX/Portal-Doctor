use std::process::{Command, Stdio};

use crate::collectors::timeouts::{SHORT_METADATA, run_bounded};
use crate::model::section::Section;
use crate::model::service::{ServiceInfo, UnitState, UnitStatus};

/// Collect portal-relevant systemd user unit states with a bounded
/// `systemctl --user show` subprocess per unit (architecture §12 permits the
/// subprocess form while parsing stays targeted and tested).
pub fn collect(units: &[String]) -> Section<ServiceInfo> {
    let units = units.to_vec();
    let Some(info) = run_bounded(SHORT_METADATA, move || {
        let statuses: Vec<UnitStatus> = units.iter().map(|unit| show_unit(unit)).collect();
        ServiceInfo { units: statuses }
    }) else {
        return Section::timed_out("systemctl --user show exceeded 2s");
    };
    if info.units.is_empty() {
        return Section::unsupported("no portal-relevant units requested");
    }
    Section::available(info)
}

/// Query one unit through `systemctl --user show`. A failing call maps to
/// `NotFound` (missing unit) or `Unreadable` (anything else), never a hang.
fn show_unit(unit: &str) -> UnitStatus {
    let unit_name = unit.to_owned();
    let bounded = run_bounded(SHORT_METADATA, move || {
        Command::new("systemctl")
            .args(["--user", "show", &unit_name])
            .args([
                "-p",
                "ActiveState",
                "-p",
                "SubState",
                "-p",
                "UnitFileState",
                "--value",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
    });
    let Some(Ok(output)) = bounded else {
        return unreadable(unit);
    };
    if !output.status.success() {
        return not_found(unit);
    }
    parse_show_output(&String::from_utf8_lossy(&output.stdout), unit)
}

/// Parse `systemctl --value` output for `ActiveState`, `SubState`,
/// `UnitFileState`.
#[must_use]
pub fn parse_show_output(text: &str, unit: &str) -> UnitStatus {
    let mut values = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let active_state = values.next().unwrap_or_default();
    if active_state == "unknown" {
        return not_found(unit);
    }
    UnitStatus {
        unit: unit.to_owned(),
        state: UnitState::parse_active_state(active_state),
        sub_state: values.next().filter(|v| !v.is_empty()).map(str::to_owned),
        unit_file_state: values.next().filter(|v| !v.is_empty()).map(str::to_owned),
    }
}

fn not_found(unit: &str) -> UnitStatus {
    UnitStatus {
        unit: unit.to_owned(),
        state: UnitState::NotFound,
        sub_state: None,
        unit_file_state: None,
    }
}

fn unreadable(unit: &str) -> UnitStatus {
    UnitStatus {
        unit: unit.to_owned(),
        state: UnitState::Unreadable,
        sub_state: None,
        unit_file_state: None,
    }
}
