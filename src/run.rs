use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{CheckArgs, CheckDomain, Cli};
use crate::collectors;
use crate::error::Error;
use crate::model::section::Section;
use crate::model::snapshot::Snapshot;
use crate::report::{JsonRenderer, Renderer, Report, TerminalRenderer};
use crate::rules;

/// Execute the parsed `CLI` and write the selected output to `stdout`.
///
/// # Errors
///
/// Returns [`Error::Write`] when writing the rendered report fails.
pub fn run(cli: &Cli) -> Result<(), Error> {
    let command = cli
        .command
        .clone()
        .unwrap_or(crate::cli::Command::Check(CheckArgs::default()));
    tracing::info!(?command, "starting portaldoctor");
    match command {
        crate::cli::Command::Check(args) => run_check(&args, cli.json, cli.verbose),
    }
}

fn run_check(args: &CheckArgs, json: bool, verbose: bool) -> Result<(), Error> {
    // Bare `check` runs every implemented domain; Phase 1 ships exactly one,
    // so both cases currently execute the environment checks.
    match args.domain {
        Some(CheckDomain::Environment) | None => run_environment_checks(json, verbose),
    }
}

fn run_environment_checks(json: bool, verbose: bool) -> Result<(), Error> {
    let system = collectors::os_release::collect();
    let process_env = collectors::environment::collect_process_environment();
    let session = Section::available(collectors::environment::session_info(&process_env));
    let activation = collectors::activation_environment::collect();

    let home = std::env::var("HOME").ok();
    let mut environment = Section::available(collectors::environment::environment_info(
        process_env,
        home.as_deref(),
        activation.value.as_ref(),
    ));
    if activation.status != crate::model::status::CollectorState::Available {
        let reason = activation_note_reason(&activation);
        if reason.is_empty() {
            environment.push_note(format!(
                "systemd user activation environment: {}",
                activation.status
            ));
        } else {
            environment.push_note(format!(
                "systemd user activation environment {}: {}",
                activation.status, reason
            ));
        }
    }

    let snapshot = Snapshot::new(unix_epoch_ms(), system, session, environment);
    let findings = rules::engine::evaluate(&snapshot);
    tracing::debug!(findings = findings.len(), "evaluation finished");
    let report = Report::new(snapshot, findings, env!("CARGO_PKG_VERSION"));

    let rendered = if json {
        JsonRenderer.render(&report, verbose)
    } else {
        TerminalRenderer.render(&report, verbose)
    };
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{rendered}")?;
    stdout.flush()?;
    Ok(())
}

fn activation_note_reason<T>(section: &Section<T>) -> String {
    section
        .errors
        .iter()
        .map(|note| note.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Current time as Unix epoch milliseconds: the snapshot collection anchor.
fn unix_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock precedes the Unix epoch")
        .as_millis()
        .try_into()
        .expect("timestamp does not fit into u64 milliseconds")
}
