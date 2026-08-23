use clap::{Args, Parser, Subcommand};

/// Deterministic diagnostic `CLI` for `XDG` Desktop Portals, `Wayland` and `PipeWire`
/// integration on `Linux`.
#[derive(Debug, Parser)]
#[command(name = "portaldoctor", version)]
pub struct Cli {
    /// Emit a versioned JSON report instead of terminal text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Show collected details and full finding explanations.
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Supported subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Run the passive read-only diagnostic checks. This is also the default
    /// command when no subcommand is given.
    Check(CheckArgs),
    /// Inspect portal backends and routing.
    Portal(PortalArgs),
}

/// Options for the `check` command.
#[derive(Debug, Clone, Default, Args)]
pub struct CheckArgs {
    /// Restrict the run to a single diagnostic domain.
    #[command(subcommand)]
    pub domain: Option<CheckDomain>,
}

/// Diagnostic domains selectable under `check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum CheckDomain {
    /// Desktop/session/environment discovery checks.
    Environment,
    /// Portal configuration, backends and routing checks.
    Portal,
}

/// Options for the `portal` command.
#[derive(Debug, Clone, Args)]
pub struct PortalArgs {
    #[command(subcommand)]
    pub command: PortalCmd,
}

/// Portal inspection subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum PortalCmd {
    /// List discovered portal backends.
    List,
    /// Print the resolved route table.
    Routes,
    /// Explain routing for one interface, e.g. `ScreenCast`.
    Explain {
        /// Interface name or suffix, e.g. `ScreenCast` or
        /// `org.freedesktop.impl.portal.ScreenCast`.
        interface: String,
    },
}
