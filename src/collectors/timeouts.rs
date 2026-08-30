#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
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

#[cfg(not(unix))]
fn kill_group(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Result of a bounded command whose output may be capped before parsing.
#[derive(Debug, PartialEq, Eq)]
pub enum BoundedOutput {
    /// The child exited and both captured streams were drained successfully.
    Completed(Output),
    /// The child or one of its output streams did not finish before `timeout`.
    TimedOut,
    /// A captured stream exceeded the caller-provided byte limit.
    OutputLimitExceeded,
}

#[derive(Debug, Clone, Copy)]
enum PipeKind {
    Stdout,
    Stderr,
}

#[derive(Debug, PartialEq, Eq)]
enum PipeReadResult {
    Complete(Vec<u8>),
    TooLarge,
}

/// Drain one child stream on a worker so a large `pw-dump` response cannot
/// fill the OS pipe while the parent waits for the child to exit.
fn spawn_pipe_reader<R>(
    kind: PipeKind,
    mut reader: R,
    max_output_bytes: usize,
    sender: Sender<(PipeKind, std::io::Result<PipeReadResult>)>,
) -> thread::JoinHandle<()>
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let result = read_pipe(&mut reader, max_output_bytes);
        let _ = sender.send((kind, result));
    })
}

fn read_pipe<R>(reader: &mut R, max_output_bytes: usize) -> std::io::Result<PipeReadResult>
where
    R: std::io::Read,
{
    let mut captured = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(PipeReadResult::Complete(captured));
        }
        if count > max_output_bytes.saturating_sub(captured.len()) {
            return Ok(PipeReadResult::TooLarge);
        }
        captured.extend_from_slice(&buffer[..count]);
    }
}

fn join_reader(handle: thread::JoinHandle<()>) -> std::io::Result<()> {
    handle
        .join()
        .map_err(|_| std::io::Error::other("child output reader thread panicked"))
}

fn limit_exceeded(
    stdout: Option<&std::io::Result<PipeReadResult>>,
    stderr: Option<&std::io::Result<PipeReadResult>>,
) -> bool {
    [stdout, stderr]
        .into_iter()
        .flatten()
        .any(|result| matches!(result, Ok(PipeReadResult::TooLarge)))
}

/// Spawn `command`, bound its completion and drain stdout/stderr concurrently.
/// The process is always killed and reaped on timeout or output overflow. A
/// separate output limit keeps a pathological `PipeWire` graph from becoming an
/// unbounded allocation in the diagnostic process.
pub fn output_bounded_with_limit(
    timeout: Duration,
    max_output_bytes: usize,
    mut command: Command,
) -> Result<BoundedOutput, std::io::Error> {
    // Isolate the child in its own process group so shell wrappers cannot
    // leave unrelated grandchildren attached to our session.
    #[cfg(unix)]
    {
        // setpgid on the child happens pre-exec; the call itself cannot fail
        // here, only the child-side primitive can.
        let _ = command.process_group(0);
    }
    let mut child = command.stdout(Stdio::piped()).spawn()?;
    let Some(stdout) = child.stdout.take() else {
        kill_group(&mut child);
        return Err(std::io::Error::other("child stdout pipe was not created"));
    };
    let stderr = child.stderr.take();
    let has_stderr = stderr.is_some();
    let (sender, receiver) = mpsc::channel();
    let stdout_reader =
        spawn_pipe_reader(PipeKind::Stdout, stdout, max_output_bytes, sender.clone());
    let stderr_reader = stderr
        .map(|pipe| spawn_pipe_reader(PipeKind::Stderr, pipe, max_output_bytes, sender.clone()));
    drop(sender);

    let deadline = Instant::now() + timeout;
    let mut child_status = None;
    let mut stdout_result = None;
    let mut stderr_result = None;
    loop {
        receive_pipe_results(&receiver, &mut stdout_result, &mut stderr_result);
        if limit_exceeded(stdout_result.as_ref(), stderr_result.as_ref()) {
            kill_group(&mut child);
            drop(stdout_reader);
            drop(stderr_reader);
            return Ok(BoundedOutput::OutputLimitExceeded);
        }

        if child_status.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => child_status = Some(status),
                Ok(None) => {}
                Err(err) => {
                    kill_group(&mut child);
                    drop(stdout_reader);
                    drop(stderr_reader);
                    return Err(err);
                }
            }
        }

        let output_drained = stdout_result.is_some() && (!has_stderr || stderr_result.is_some());
        if child_status.is_some() && output_drained {
            break;
        }
        if Instant::now() >= deadline {
            kill_group(&mut child);
            drop(stdout_reader);
            drop(stderr_reader);
            return Ok(BoundedOutput::TimedOut);
        }
        thread::sleep(Duration::from_millis(10));
    }

    // The result channel is complete, so joins cannot block on a live pipe.
    join_reader(stdout_reader)?;
    if let Some(reader) = stderr_reader {
        join_reader(reader)?;
    }
    receive_pipe_results(&receiver, &mut stdout_result, &mut stderr_result);

    let stdout = match stdout_result {
        Some(Ok(PipeReadResult::Complete(bytes))) => bytes,
        Some(Ok(PipeReadResult::TooLarge)) => return Ok(BoundedOutput::OutputLimitExceeded),
        Some(Err(err)) => return Err(err),
        None => {
            return Err(std::io::Error::other(
                "child stdout result was not received",
            ));
        }
    };
    let stderr = match stderr_result {
        Some(Ok(PipeReadResult::Complete(bytes))) => bytes,
        Some(Ok(PipeReadResult::TooLarge)) => return Ok(BoundedOutput::OutputLimitExceeded),
        Some(Err(err)) => return Err(err),
        None => Vec::new(),
    };
    let status = child_status.expect("child status is set when output is drained");
    Ok(BoundedOutput::Completed(Output {
        status,
        stdout,
        stderr,
    }))
}

fn receive_pipe_results(
    receiver: &Receiver<(PipeKind, std::io::Result<PipeReadResult>)>,
    stdout_result: &mut Option<std::io::Result<PipeReadResult>>,
    stderr_result: &mut Option<std::io::Result<PipeReadResult>>,
) {
    for (kind, result) in receiver.try_iter() {
        match kind {
            PipeKind::Stdout => *stdout_result = Some(result),
            PipeKind::Stderr => *stderr_result = Some(result),
        }
    }
}

pub fn output_bounded(
    timeout: Duration,
    command: Command,
) -> Result<Option<Output>, std::io::Error> {
    match output_bounded_with_limit(timeout, usize::MAX, command)? {
        BoundedOutput::Completed(output) => Ok(Some(output)),
        BoundedOutput::TimedOut | BoundedOutput::OutputLimitExceeded => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundedOutput, Duration, output_bounded, output_bounded_with_limit, run_bounded};
    use std::process::Command;
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
    fn drains_output_larger_than_the_pipe_buffer() {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("dd if=/dev/zero bs=131072 count=1 2>/dev/null");
        let result = output_bounded_with_limit(Duration::from_secs(2), 200_000, command).unwrap();
        let BoundedOutput::Completed(output) = result else {
            panic!("large command output did not complete");
        };
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 131_072);
    }

    #[test]
    #[cfg(unix)]
    fn stops_and_kills_when_output_exceeds_limit() {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("dd if=/dev/zero bs=131072 count=1 2>/dev/null");
        let result = output_bounded_with_limit(Duration::from_secs(2), 1024, command).unwrap();
        assert_eq!(result, BoundedOutput::OutputLimitExceeded);
    }

    /// Wait until every pid reports gone via `kill(pid, 0)`, with a bounded
    /// retry loop so kernel reaping latency cannot cause false failures.
    #[cfg(unix)]
    fn wait_gone(pids: &[i32]) -> bool {
        for _ in 0..40 {
            let alive = pids.iter().any(|pid| unsafe { libc::kill(*pid, 0) } == 0);
            if !alive {
                return true;
            }
            thread::sleep(Duration::from_millis(25));
        }
        !pids.iter().any(|pid| unsafe { libc::kill(*pid, 0) } == 0)
    }

    #[test]
    #[cfg(unix)]
    fn kills_direct_child_on_timeout_leaving_no_orphan() {
        // `sh -c 'echo $$ > f; exec sleep 30'` turns the shell into the real
        // sleep process, so $$ is its exact pid and no grandchild exists.
        let dir = std::env::temp_dir().join("portaldoctor_orphan_direct");
        std::fs::create_dir_all(&dir).unwrap();
        let pidfile = dir.join("child.pid");
        // exec replaces the shell with the real sleep process: $$ is the
        // exact pid that output_bounded must kill.
        let body = format!("echo \"$$\" > {}\nexec sleep 30", pidfile.display());
        let mut command = Command::new("sh");
        command.arg("-c").arg(&body);
        let result = output_bounded(Duration::from_millis(120), command).unwrap();
        assert!(result.is_none());

        let pid_text = std::fs::read_to_string(&pidfile).unwrap();
        let pid: i32 = pid_text.trim().parse().unwrap();
        assert!(
            wait_gone(&[pid]),
            "direct child {pid} survived the timeout kill"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[cfg(unix)]
    fn group_kill_reaps_shell_wrapper_grandchildren() {
        // Shell wrappers fork a grandchild (the actual command); killing only
        // the direct child would leave it running as an orphan. The process-
        // group kill must reap the whole tree, verified by exact pids.
        let dir = std::env::temp_dir().join("portaldoctor_orphan_wrapper");
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("slow-systemctl");
        let pidfile_child = dir.join("child.pid");
        let pidfile_grand = dir.join("grandchild.pid");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$$\" > {}\nsleep 30 &\necho \"$!\" > {}\nwait\n",
                pidfile_child.display(),
                pidfile_grand.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut command = Command::new(&script);
        command.arg("anything");
        let result = output_bounded(Duration::from_millis(200), command).unwrap();
        assert!(result.is_none());

        let child_pid: i32 = std::fs::read_to_string(&pidfile_child)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let grand_pid: i32 = std::fs::read_to_string(&pidfile_grand)
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        // Bounded wait: SIGKILL is immediate but reaping may lag slightly.
        assert!(
            wait_gone(&[child_pid, grand_pid]),
            "wrapper child {child_pid} or grandchild {grand_pid} survived the group kill"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn returns_none_when_worker_panics() {
        let result = run_bounded(Duration::from_secs(2), || -> u8 { panic!("boom") });
        assert_eq!(result, None);
    }
}
