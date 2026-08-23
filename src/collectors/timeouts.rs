use std::io::Read as _;
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Central timeout policy for runtime operations (architecture §19):
/// short metadata calls 1–2 s, normal runtime queries 2–3 s. Every D-Bus and
/// subprocess interaction must run inside one of these bounds.
pub const SHORT_METADATA: Duration = Duration::from_secs(2);
pub const NORMAL_RUNTIME_QUERY: Duration = Duration::from_secs(3);

/// Run `f` on a worker thread bounded by `timeout`. `None` means the work did
/// not finish in time; the worker is detached so it cannot hang the CLI — a
/// wedged dependency is diagnostic data, not a hang (PRD REL-003).
pub fn run_bounded<T, F>(timeout: Duration, f: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(f());
    });
    // Timeout and a panicked worker (sender dropped) both mean "no result in
    // time"; the detached worker cannot hang the CLI.
    receiver.recv_timeout(timeout).ok()
}

/// Spawn `command`, bound its completion to `timeout`, and always reap the
/// child: on timeout it is killed and waited, so no orphan process survives
/// the call (PRD REL-003). Captures stdout via a pipe; stderr handling is
/// left to the caller's command configuration.
///
/// `Err` means the command could not be spawned; `Ok(None)` means it exceeded
/// `timeout`; `Ok(Some(output))` is the completed run.
/// Kill the child and its whole process group (`process_group(0)` put it in
/// one), then reap it, so shell wrappers or grandchildren cannot survive a
/// timeout as orphans.
#[cfg(unix)]
fn kill_group(child: &mut std::process::Child) {
    // PIDs above i32::MAX do not exist on Linux; the cast is safe in practice.
    #[allow(clippy::cast_possible_wrap)]
    let pid = child.id() as i32;
    // Negative pid targets the entire process group led by the child.
    // SAFETY: plain syscall wrapper; killing only the group we created above.
    unsafe { libc::kill(-pid, libc::SIGKILL) };
    let _ = child.wait();
}

pub fn output_bounded(
    timeout: Duration,
    mut command: Command,
) -> Result<Option<Output>, std::io::Error> {
    // Isolate the child in its own process group so shell wrappers cannot
    // leave unrelated grandchildren attached to our session.
    #[cfg(unix)]
    {
        // setpgid on the child happens pre-exec; the call itself cannot fail
        // here, only the child-side primitive can.
        let _ = command.process_group(0);
    }
    let mut child = command.stdout(Stdio::piped()).spawn()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            // The environment dumps are small; draining stdout after exit
            // cannot fill the pipe buffer.
            Ok(None) if Instant::now() >= deadline => {
                kill_group(&mut child);
                return Ok(None);
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(err) => return Err(err),
        }
    };
    let Some(mut stdout) = child.stdout.take() else {
        return Ok(None);
    };
    let mut captured = Vec::new();
    if stdout.read_to_end(&mut captured).is_err() {
        return Ok(None);
    }
    Ok(Some(Output {
        status,
        stdout: captured,
        stderr: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::{Duration, output_bounded, run_bounded};
    use std::process::{Command, Stdio};
    use std::thread;

    #[test]
    fn returns_value_within_timeout() {
        assert_eq!(run_bounded(Duration::from_secs(2), || 41 + 1), Some(42));
    }

    #[test]
    fn returns_none_when_work_exceeds_timeout() {
        let result = run_bounded(Duration::from_millis(50), || {
            std::thread::sleep(Duration::from_secs(2));
            7
        });
        assert_eq!(result, None);
    }

    #[test]
    #[cfg(unix)]
    fn kills_child_on_timeout_leaving_no_orphan() {
        use std::process::Command;

        let mut command = Command::new("sleep");
        command.arg("30");
        let result = output_bounded(Duration::from_millis(120), command).unwrap();
        assert!(result.is_none());
        // Give the kernel a moment to reap the killed child.
        thread::sleep(Duration::from_millis(80));
        let probe = Command::new("ps").arg("-o").arg("args=").output().unwrap();
        let listing = String::from_utf8_lossy(&probe.stdout);
        assert!(
            !listing.contains("sleep 30"),
            "orphaned child survived the timeout: {listing}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn group_kill_reaps_shell_wrapper_grandchildren() {
        // Shell wrappers fork a grandchild (the actual command); killing only
        // the direct child would leave it running as an orphan. The process-
        // group kill must reap the whole tree.
        let dir = std::env::temp_dir().join("portaldoctor_orphan_fixture");
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("slow-systemctl");
        std::fs::write(&script, "#!/bin/sh\nsleep 30\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut command = Command::new(&script);
        command.arg("anything");
        let result = output_bounded(Duration::from_millis(200), command).unwrap();
        assert!(result.is_none());

        // Give the kernel and the reaper a moment.
        thread::sleep(Duration::from_millis(120));
        let probe = Command::new("sh")
            .args(["-c", "ps -eo args | grep -c '[s]leep 30'"])
            .stderr(Stdio::null())
            .output()
            .unwrap();
        let count: i32 = String::from_utf8_lossy(&probe.stdout)
            .trim()
            .parse()
            .unwrap_or(i32::MAX);
        assert_eq!(count, 0, "grandchild survived the group kill");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn returns_none_when_worker_panics() {
        let result = run_bounded(Duration::from_secs(2), || -> u8 { panic!("boom") });
        assert_eq!(result, None);
    }
}
