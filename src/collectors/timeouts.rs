use std::thread;
use std::time::Duration;

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

#[cfg(test)]
mod tests {
    use super::{Duration, run_bounded};

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
    fn returns_none_when_worker_panics() {
        let result = run_bounded(Duration::from_secs(2), || -> u8 { panic!("boom") });
        assert_eq!(result, None);
    }
}
