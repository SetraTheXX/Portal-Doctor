use std::process::{Command, Stdio};

use crate::collectors::timeouts::{SHORT_METADATA, output_bounded};
use crate::model::section::Section;
use crate::model::service::{ServiceInfo, UnitState, UnitStatus};

/// Collect portal-relevant systemd user unit states with a bounded
/// `systemctl --user show` subprocess per unit (architecture §12 permits the
/// subprocess form while parsing stays targeted and tested).
/// Collect portal-relevant systemd user unit states. Each unit query is
/// individually bounded (`output_bounded`), so a wedged unit degrades to
/// `Unreadable` after 2s instead of hanging the run or leaving orphaned
/// children; no further subprocesses are spawned once every unit has been
/// probed.
pub fn collect(units: &[String]) -> Section<ServiceInfo> {
    if units.is_empty() {
        return Section::unsupported("no portal-relevant units requested");
    }
    let statuses: Vec<UnitStatus> = units.iter().map(|unit| show_unit(unit)).collect();
    Section::available(ServiceInfo { units: statuses })
}

/// Query one unit through `systemctl --user show`. A failing call maps to
/// `NotFound` (missing unit) or `Unreadable` (anything else), never a hang.
fn show_unit(unit: &str) -> UnitStatus {
    let mut command = Command::new("systemctl");
    command
        .args(["--user", "show", unit])
        .args([
            "-p",
            "ActiveState",
            "-p",
            "SubState",
            "-p",
            "UnitFileState",
            "--value",
        ])
        .stderr(Stdio::null());
    // output_bounded kills and reaps the child on timeout: no orphan remains.
    // Spawn failure and timeout both degrade to Unreadable.
    let Ok(Some(output)) = output_bounded(SHORT_METADATA, command) else {
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

#[cfg(test)]
mod tests {
    use super::parse_show_output;
    use crate::model::service::UnitState;

    const KDE_UNIT: &str = "xdg-desktop-portal-kde.service";

    #[test]
    fn parses_active_kde_backend_unit() {
        let status = parse_show_output("active\nrunning\nstatic\n", KDE_UNIT);

        assert_eq!(status.unit, KDE_UNIT);
        assert_eq!(status.state, UnitState::Active);
        assert_eq!(status.sub_state.as_deref(), Some("running"));
        assert_eq!(status.unit_file_state.as_deref(), Some("static"));
    }

    #[test]
    fn parses_failed_kde_backend_unit() {
        let status = parse_show_output("failed\nfailed\nstatic\n", KDE_UNIT);

        assert_eq!(status.unit, KDE_UNIT);
        assert_eq!(status.state, UnitState::Failed);
        assert_eq!(status.sub_state.as_deref(), Some("failed"));
        assert_eq!(status.unit_file_state.as_deref(), Some("static"));
    }

    #[test]
    fn maps_unknown_kde_backend_unit_to_not_found() {
        let status = parse_show_output("unknown\n\n\n", KDE_UNIT);

        assert_eq!(status.unit, KDE_UNIT);
        assert_eq!(status.state, UnitState::NotFound);
        assert_eq!(status.sub_state, None);
        assert_eq!(status.unit_file_state, None);
    }

    /// Run only from `scripts/validate-systemd-kde-ci.sh`: this test must
    /// never invoke the real user systemd manager.
    #[test]
    #[cfg(unix)]
    #[ignore = "requires the explicit isolated fake-systemctl gate"]
    fn isolated_kde_systemd_collector_contract() {
        use std::fs;
        use std::path::Path;
        use std::thread;
        use std::time::{Duration, Instant};

        use crate::model::section::Section;
        use crate::model::service::{ServiceInfo, UnitStatus};
        use crate::model::status::CollectorState;

        const EXPECTED_INVOCATION: &str = "--user show xdg-desktop-portal-kde.service -p ActiveState -p SubState -p UnitFileState --value";

        fn set_mode(path: &Path, mode: &str) {
            fs::write(path, format!("{mode}\n")).expect("write fake systemctl mode");
        }

        fn collect_kde() -> UnitStatus {
            let section: Section<ServiceInfo> = super::collect(&[KDE_UNIT.to_owned()]);
            assert_eq!(section.status, CollectorState::Available);
            section
                .value
                .expect("systemd collector value")
                .unit(KDE_UNIT)
                .cloned()
                .expect("KDE unit status")
        }

        let fake_dir = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_FAKE_DIR")
            .expect("explicit fake-systemctl wrapper must provide a fake directory");
        let path = std::env::var_os("PATH").expect("wrapper must provide PATH");
        let first_path = std::env::split_paths(&path)
            .next()
            .expect("PATH must contain the fake directory");
        assert_eq!(first_path, Path::new(&fake_dir));
        assert_eq!(
            std::env::var("PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD").as_deref(),
            Ok("isolated-kde-systemd")
        );

        let mode_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_MODE_FILE")
            .expect("wrapper must provide a mode file");
        let log_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_LOG")
            .expect("wrapper must provide an invocation log");
        let pid_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_PID_FILE")
            .expect("wrapper must provide a PID file");

        set_mode(Path::new(&mode_file), "active");
        let active = collect_kde();
        assert_eq!(active.unit, KDE_UNIT);
        assert_eq!(active.state, UnitState::Active);
        assert_eq!(active.sub_state.as_deref(), Some("running"));
        assert_eq!(active.unit_file_state.as_deref(), Some("static"));

        set_mode(Path::new(&mode_file), "failed");
        let failed = collect_kde();
        assert_eq!(failed.unit, KDE_UNIT);
        assert_eq!(failed.state, UnitState::Failed);
        assert_eq!(failed.sub_state.as_deref(), Some("failed"));
        assert_eq!(failed.unit_file_state.as_deref(), Some("static"));

        set_mode(Path::new(&mode_file), "missing");
        let missing = collect_kde();
        assert_eq!(missing.unit, KDE_UNIT);
        assert_eq!(missing.state, UnitState::NotFound);
        assert_eq!(missing.sub_state, None);
        assert_eq!(missing.unit_file_state, None);

        set_mode(Path::new(&mode_file), "timeout");
        let started = Instant::now();
        let timed_out = collect_kde();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "collector exceeded its central bounded timeout"
        );
        assert_eq!(timed_out.unit, KDE_UNIT);
        assert_eq!(timed_out.state, UnitState::Unreadable);
        assert_eq!(timed_out.sub_state, None);
        assert_eq!(timed_out.unit_file_state, None);

        let pid: i32 = fs::read_to_string(&pid_file)
            .expect("timeout fake must record its PID")
            .trim()
            .parse()
            .expect("timeout fake PID");
        let mut gone = false;
        for _ in 0..80 {
            // SAFETY: kill(pid, 0) only probes process existence; the fake
            // process is created by this test wrapper and is never signalled
            // here.
            if unsafe { libc::kill(pid, 0) } != 0 {
                gone = true;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(gone, "timed-out fake systemctl child was not reaped");

        let invocations = fs::read_to_string(&log_file).expect("invocation log");
        assert_eq!(invocations.lines().count(), 4);
        assert!(invocations.lines().all(|line| line == EXPECTED_INVOCATION));
    }
}
