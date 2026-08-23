use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{Cli, Command};
use crate::error::Error;
use crate::model::snapshot::{SNAPSHOT_SCHEMA_VERSION, Snapshot};
use crate::report::{JsonRenderer, Renderer, Report, TerminalRenderer};

/// Execute the parsed `CLI` and write the selected output to `stdout`.
///
/// # Errors
///
/// Returns [`Error::Write`] when writing the rendered report fails.
pub fn run(cli: &Cli) -> Result<(), Error> {
    let command = cli.command.unwrap_or(Command::Check);
    tracing::info!(?command, "starting portaldoctor");
    match command {
        Command::Check => run_check(cli),
    }
}

fn run_check(cli: &Cli) -> Result<(), Error> {
    // Phase 0 has no collectors yet; the empty snapshot still exercises the
    // full v1 snapshot -> report -> render pipeline.
    let report = Report::new(
        Snapshot::new(SNAPSHOT_SCHEMA_VERSION, unix_epoch_ms()),
        Vec::new(),
        env!("CARGO_PKG_VERSION"),
    );
    tracing::debug!(findings = report.findings.len(), "report built");
    let rendered = if cli.json {
        JsonRenderer.render(&report)
    } else {
        TerminalRenderer.render(&report)
    };
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{rendered}")?;
    stdout.flush()?;
    Ok(())
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
