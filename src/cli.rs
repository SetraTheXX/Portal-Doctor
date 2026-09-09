use clap::{Args, Parser, Subcommand, ValueEnum};

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

    /// Collect bounded current-boot user journal evidence (opt-in).
    #[arg(long, global = true)]
    pub journal: bool,

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
    /// Generate a privacy-aware report suitable for sharing in an issue.
    Report(ReportArgs),
    /// Preview a bounded, read-only remediation proposal.
    Fix(FixArgs),
    /// Run an explicitly requested, bounded active portal probe.
    Probe(ProbeArgs),
}

/// Options for the read-only remediation preview command.
#[derive(Debug, Clone, Args)]
pub struct FixArgs {
    /// Finding to preview.
    #[arg(value_enum)]
    pub target: FixTarget,
    /// Required guard: applying remediation is not implemented in this slice.
    #[arg(long, required = true)]
    pub dry_run: bool,
}

/// Remediation targets with a bounded implementation contract.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FixTarget {
    /// Preview alignment of the systemd user activation environment.
    #[value(name = "ENV004")]
    Env004,
}

/// Options for explicit active portal probes.
#[derive(Debug, Clone, Args)]
pub struct ProbeArgs {
    #[command(subcommand)]
    pub command: ProbeCmd,
}

/// Supported explicit active portal probes.
#[derive(Debug, Clone, Subcommand)]
pub enum ProbeCmd {
    /// Open the desktop `FileChooser` portal and report only its lifecycle.
    #[command(name = "filechooser")]
    FileChooser,
    /// Ask the Screenshot portal to capture one selected window.
    Screenshot,
}

/// Options for the `check` command.
#[derive(Debug, Clone, Default, Args)]
pub struct CheckArgs {
    /// Restrict the run to a single diagnostic domain.
    #[command(subcommand)]
    pub domain: Option<CheckDomain>,
}

/// Options for the explicit shareable report command.
#[derive(Debug, Clone, Args)]
pub struct ReportArgs {
    /// Output format for the shareable report.
    #[arg(long, value_enum, default_value_t = ReportFormat::Terminal)]
    pub format: ReportFormat,

    /// Replace the current hostname with `<hostname>` in the report.
    #[arg(long)]
    pub suppress_hostname: bool,
}

/// Formats supported by `portaldoctor report`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReportFormat {
    Terminal,
    Json,
    Markdown,
}

/// Diagnostic domains selectable under `check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum CheckDomain {
    /// Desktop/session/environment discovery checks.
    Environment,
    /// Portal configuration, backends and routing checks.
    Portal,
    /// `PipeWire`, `WirePlumber` and `ScreenCast` media-path checks.
    #[command(name = "pipewire")]
    PipeWire,
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

#[cfg(test)]
mod tests {
    use super::{CheckDomain, Cli, Command, FixTarget, ProbeCmd, ReportFormat};
    use crate::run::{CLEAN_EXIT_CODE, CLI_USAGE_EXIT_CODE};
    use clap::Parser;

    #[test]
    fn parses_pipewire_check_domain() {
        let cli = Cli::parse_from(["portaldoctor", "check", "pipewire"]);
        let Command::Check(args) = cli.command.unwrap() else {
            panic!("expected check command");
        };
        assert_eq!(args.domain, Some(CheckDomain::PipeWire));
    }

    #[test]
    fn parses_opt_in_journal_flag() {
        let cli = Cli::parse_from(["portaldoctor", "--journal", "check"]);
        assert!(cli.journal);
        assert!(matches!(cli.command, Some(Command::Check(_))));
    }

    #[test]
    fn parses_shareable_report_options() {
        let cli = Cli::parse_from([
            "portaldoctor",
            "report",
            "--format",
            "markdown",
            "--suppress-hostname",
        ]);
        let Some(Command::Report(args)) = cli.command else {
            panic!("expected report command");
        };
        assert_eq!(args.format, ReportFormat::Markdown);
        assert!(args.suppress_hostname);
    }

    #[test]
    fn global_json_flag_can_select_report_json() {
        let cli = Cli::parse_from(["portaldoctor", "report", "--json"]);
        assert!(cli.json);
        assert!(matches!(cli.command, Some(Command::Report(_))));
    }

    #[test]
    fn parses_explicit_filechooser_probe() {
        let cli = Cli::parse_from(["portaldoctor", "probe", "filechooser", "--json"]);
        assert!(cli.json);
        let Some(Command::Probe(args)) = cli.command else {
            panic!("expected probe command");
        };
        assert!(matches!(args.command, ProbeCmd::FileChooser));
    }

    #[test]
    fn parses_explicit_screenshot_probe() {
        let cli = Cli::parse_from(["portaldoctor", "probe", "screenshot", "--json"]);
        assert!(cli.json);
        let Some(Command::Probe(args)) = cli.command else {
            panic!("expected probe command");
        };
        assert!(matches!(args.command, ProbeCmd::Screenshot));
    }

    #[test]
    fn parses_env004_dry_run_preview() {
        let cli = Cli::parse_from(["portaldoctor", "fix", "ENV004", "--dry-run"]);
        let Some(Command::Fix(args)) = cli.command else {
            panic!("expected fix command");
        };
        assert!(args.dry_run);
        assert!(matches!(args.target, FixTarget::Env004));
    }

    #[test]
    fn remediation_preview_requires_dry_run() {
        assert!(Cli::try_parse_from(["portaldoctor", "fix", "ENV004"]).is_err());
    }

    #[test]
    fn parser_error_uses_the_public_usage_exit_code() {
        let error = Cli::try_parse_from(["portaldoctor", "--definitely-invalid"]).unwrap_err();
        assert_eq!(error.exit_code(), i32::from(CLI_USAGE_EXIT_CODE));
    }

    #[test]
    fn help_uses_the_clean_exit_code() {
        let error = Cli::try_parse_from(["portaldoctor", "--help"]).unwrap_err();
        assert_eq!(error.exit_code(), i32::from(CLEAN_EXIT_CODE));
        assert!(error.to_string().contains("Usage:"));
    }
}
