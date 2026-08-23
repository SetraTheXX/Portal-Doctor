mod cli;
mod error;
mod model;
mod report;
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
    // Logs go to `stderr` so the `JSON` report on `stdout` stays machine-readable.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .init();
}
