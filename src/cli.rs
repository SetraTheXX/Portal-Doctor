use clap::{Parser, Subcommand};

/// Deterministic diagnostic `CLI` for `XDG` Desktop Portals, `Wayland` and `PipeWire`
/// integration on `Linux`.
#[derive(Debug, Parser)]
#[command(name = "portaldoctor", version)]
pub struct Cli {
    /// Emit a versioned JSON report instead of terminal text.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Supported subcommands.
#[derive(Debug, Clone, Copy, Subcommand)]
pub enum Command {
    /// Run the passive read-only diagnostic check. This is also the default
    /// command when no subcommand is given.
    Check,
}
