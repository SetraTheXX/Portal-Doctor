mod cli;
mod collectors;
mod error;
mod model;
mod report;
mod resolver;
mod rules;
mod run;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;

fn main() -> ExitCode {
    init_tracing();
    let cli = Cli::parse();
    match run::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("portaldoctor: {err}");
            ExitCode::FAILURE
        }
    }
}

fn init_tracing() {
    // Keep normal terminal and JSON runs quiet. Diagnostics are the user-facing
    // output; verbose runtime logging should not pollute a captured report or
    // the README demo. Logs still go to `stderr` if warning/error events are
    // added later.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::WARN)
        .init();
}
