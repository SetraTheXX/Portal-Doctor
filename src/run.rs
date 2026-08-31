use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{CheckArgs, CheckDomain, Cli, PortalArgs, PortalCmd, ReportArgs, ReportFormat};
use crate::collectors;
use crate::error::Error;
use crate::model::finding::{Finding, Severity};
use crate::model::portal::PortalRoute;
use crate::model::section::Section;
use crate::model::service::ServiceInfo;
use crate::model::snapshot::Snapshot;
use crate::report::{
    JsonRenderer, MarkdownRenderer, PortalExplainRenderer, PortalListRenderer,
    PortalRoutesRenderer, RedactionOptions, Renderer, Report, ShareableJsonRenderer,
    ShareableReport, TerminalRenderer, redact_report,
};
use crate::resolver;
use crate::rules;

/// Exit code for an incomplete run caused by an output or internal error.
pub const INTERNAL_ERROR_EXIT_CODE: u8 = 4;

/// Result of a completed diagnostic command and its stable process exit code.
///
/// CLI parsing errors are handled by `clap` before this type is produced and
/// use exit code `2`. A renderer/write failure is an incomplete run and uses
/// exit code `4` on the generic process-error path in `main`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// The diagnostic completed without an error/critical finding.
    Clean,
    /// The diagnostic completed and produced at least one error/critical finding.
    SevereFindings,
    /// The diagnostic could not establish the minimum session/runtime context.
    RuntimeContextUnavailable,
}

impl RunOutcome {
    /// Stable shell exit code for this completed diagnostic outcome.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Clean => 0,
            Self::SevereFindings => 1,
            Self::RuntimeContextUnavailable => 3,
        }
    }

    fn from_report(report: &Report) -> Self {
        if !minimum_runtime_context_available(&report.snapshot) {
            return Self::RuntimeContextUnavailable;
        }
        if report
            .findings
            .iter()
            .any(|finding| matches!(finding.severity, Severity::Error | Severity::Critical))
        {
            return Self::SevereFindings;
        }
        Self::Clean
    }
}

/// The minimum context required before a completed finding result can be
/// treated as a normal diagnostic outcome: a known graphical session/display
/// and a reachable user session D-Bus.
fn minimum_runtime_context_available(snapshot: &Snapshot) -> bool {
    let Some(session) = snapshot.session.value.as_ref() else {
        return false;
    };
    let display_available = match session.session_type {
        Some(crate::model::environment::SessionType::Wayland) => session.wayland_display.is_some(),
        Some(crate::model::environment::SessionType::X11) => session.display.is_some(),
        None => false,
    };
    display_available
        && snapshot
            .dbus
            .value
            .as_ref()
            .is_some_and(|dbus| dbus.connected)
}

/// Execute the parsed `CLI` and write the selected output to `stdout`.
///
/// # Errors
///
/// Returns [`Error::Write`] when writing the rendered report fails.
pub fn run(cli: &Cli) -> Result<RunOutcome, Error> {
    let command = cli
        .command
        .clone()
        .unwrap_or(crate::cli::Command::Check(CheckArgs::default()));
    tracing::info!(?command, "starting portaldoctor");
    match command {
        crate::cli::Command::Check(args) => run_check(&args, cli.json, cli.verbose, cli.journal),
        crate::cli::Command::Portal(args) => run_portal(&args, cli.json, cli.journal),
        crate::cli::Command::Report(args) => run_report(&args, cli.json, cli.verbose, cli.journal),
    }
}

fn run_check(
    args: &CheckArgs,
    json: bool,
    verbose: bool,
    include_journal: bool,
) -> Result<RunOutcome, Error> {
    let collected = collect_snapshot(include_journal);
    let findings = rules::engine::evaluate(&collected.snapshot);
    let findings = match args.domain {
        None => findings,
        Some(CheckDomain::Environment) => filter_findings(findings, is_environment_finding),
        Some(CheckDomain::Portal) => filter_findings(findings, is_portal_finding),
        Some(CheckDomain::PipeWire) => filter_findings(findings, is_pipewire_finding),
    };
    let report = Report::new(collected.snapshot, findings, env!("CARGO_PKG_VERSION"));
    let outcome = RunOutcome::from_report(&report);
    write_report(&report, json, verbose)?;
    Ok(outcome)
}

fn run_portal(args: &PortalArgs, json: bool, include_journal: bool) -> Result<RunOutcome, Error> {
    let collected = collect_snapshot(include_journal);
    let findings = rules::engine::evaluate(&collected.snapshot);
    let findings = filter_findings(findings, is_portal_finding);
    let report = Report::new(collected.snapshot, findings, env!("CARGO_PKG_VERSION"));
    let rendered = match &args.command {
        PortalCmd::List => PortalListRenderer.render(&report, false),
        PortalCmd::Routes => PortalRoutesRenderer.render(&report, false),
        PortalCmd::Explain { interface } => PortalExplainRenderer {
            interface: interface.clone(),
        }
        .render(&report, false),
    };
    if json {
        let rendered = JsonRenderer.render(&report, false);
        write_stdout(&rendered)?;
    } else {
        write_stdout(&rendered)?;
    }
    Ok(RunOutcome::from_report(&report))
}

fn run_report(
    args: &ReportArgs,
    json: bool,
    verbose: bool,
    include_journal: bool,
) -> Result<RunOutcome, Error> {
    let collected = collect_snapshot(include_journal);
    let findings = rules::engine::evaluate(&collected.snapshot);
    let report = Report::new(collected.snapshot, findings, env!("CARGO_PKG_VERSION"));
    let options = RedactionOptions::from_environment(args.suppress_hostname);
    let redacted = redact_report(&report, &options);
    let shareable = ShareableReport::from_report(&redacted, &options);
    let format = if json {
        ReportFormat::Json
    } else {
        args.format
    };
    let rendered = match format {
        ReportFormat::Terminal => {
            let hostname = if shareable.privacy.hostname_suppressed {
                "suppressed"
            } else {
                "not suppressed"
            };
            format!(
                "PortalDoctor shareable report v{}\nPrivacy: redacted · HOME normalized · hostname {hostname}\nRaw journal/PipeWire: excluded\n\n{}",
                shareable.report_version,
                TerminalRenderer.render(&redacted, verbose)
            )
        }
        ReportFormat::Json => ShareableJsonRenderer::render(&shareable),
        ReportFormat::Markdown => MarkdownRenderer::render(&shareable, verbose),
    };
    let outcome = RunOutcome::from_report(&report);
    write_stdout(&rendered)?;
    Ok(outcome)
}

/// Everything the current run collected, in one snapshot.
struct Collected {
    snapshot: Snapshot,
}

fn collect_snapshot(include_journal: bool) -> Collected {
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

    let desktops = desktop_names(&session);
    let roots = environment.value.as_ref().map_or_else(
        || {
            collectors::environment::search_roots(
                &std::collections::BTreeMap::new(),
                std::env::var("HOME").ok().as_deref(),
            )
        },
        |info| info.search_roots.clone(),
    );

    let portal_config = collectors::portal_config::collect(&roots, &desktops);
    let portal_backends = collectors::portal_files::collect(&roots);
    let portal_routes = match (&portal_config.value, &portal_backends.value) {
        (Some(config), Some(backends)) => Section::available(
            resolver::portal_routes::resolve_routes(&desktops, config, backends),
        ),
        _ => Section::<Vec<PortalRoute>>::unsupported("portal collection incomplete"),
    };

    // Phase 3: runtime verification targets the frontend and every selected
    // backend bus name.
    let selected_backend_names = selected_backend_dbus_names(&portal_routes, &portal_backends);
    let dbus = collectors::dbus::collect(&selected_backend_names);
    let mut unit_names = vec![ServiceInfo::frontend_unit().to_owned()];
    unit_names.push("pipewire.service".to_owned());
    unit_names.push("wireplumber.service".to_owned());
    if let Some(backends) = &portal_backends.value {
        unit_names.extend(backends.iter().map(|b| ServiceInfo::backend_unit(&b.id)));
    }
    unit_names.sort_unstable();
    unit_names.dedup();
    let services = collectors::systemd_user::collect(&unit_names);
    let (pipewire, wireplumber) = collectors::pipewire::collect();
    let journal = if include_journal {
        collectors::journal::collect(&unit_names)
    } else {
        Section::unsupported("not requested")
    };

    let mut snapshot = Snapshot::new(unix_epoch_ms());
    snapshot.system = system;
    snapshot.session = session;
    snapshot.environment = environment;
    snapshot.portal_config = portal_config;
    snapshot.portal_backends = portal_backends;
    snapshot.portal_routes = portal_routes;
    snapshot.dbus = dbus;
    snapshot.services = services;
    snapshot.pipewire = pipewire;
    snapshot.wireplumber = wireplumber;
    snapshot.journal = journal;

    Collected { snapshot }
}

/// Desktop names from `XDG_CURRENT_DESKTOP`, normalized like upstream
/// (trimmed, lowercased).
fn desktop_names(session: &Section<crate::model::environment::SessionInfo>) -> Vec<String> {
    session
        .value
        .as_ref()
        .and_then(|s| s.current_desktop.as_ref())
        .map(|raw| resolver::portal_routes::normalize_desktops(raw))
        .unwrap_or_default()
}

/// Bus names of the backends selected by the resolved routes.
fn selected_backend_dbus_names(
    routes: &Section<Vec<PortalRoute>>,
    backends: &Section<Vec<crate::model::portal::PortalBackend>>,
) -> Vec<String> {
    let (Some(routes), Some(backends)) = (&routes.value, &backends.value) else {
        return Vec::new();
    };
    let mut names: Vec<String> = routes
        .iter()
        .flat_map(|route| route.selected_candidates.iter())
        .filter_map(|id| {
            backends
                .iter()
                .find(|backend| backend.id == *id)
                .map(|backend| backend.dbus_name.clone())
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

fn is_environment_finding(finding: &Finding) -> bool {
    finding.id.starts_with("ENV")
}

fn is_portal_finding(finding: &Finding) -> bool {
    finding.id.starts_with("XDP")
        || finding.id.starts_with("CFG")
        || finding.id.starts_with("DBUS")
        || finding.id.starts_with("PW")
        || finding.id.starts_with("SC")
}

fn is_pipewire_finding(finding: &Finding) -> bool {
    finding.id.starts_with("PW") || finding.id.starts_with("SC")
}

fn filter_findings(findings: Vec<Finding>, keep: fn(&Finding) -> bool) -> Vec<Finding> {
    findings.into_iter().filter(keep).collect()
}

fn write_report(report: &Report, json: bool, verbose: bool) -> Result<(), Error> {
    let rendered = if json {
        JsonRenderer.render(report, verbose)
    } else {
        TerminalRenderer.render(report, verbose)
    };
    write_stdout(&rendered)
}

fn write_stdout(rendered: &str) -> Result<(), Error> {
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

#[cfg(test)]
mod tests {
    use super::{RunOutcome, minimum_runtime_context_available};
    use crate::model::dbus::DbusInfo;
    use crate::model::environment::{SessionInfo, SessionType};
    use crate::model::finding::{Confidence, Finding, Severity};
    use crate::model::section::Section;
    use crate::model::snapshot::Snapshot;
    use crate::report::Report;

    fn runtime_ready_snapshot() -> Snapshot {
        let mut snapshot = Snapshot::new(0);
        snapshot.session = Section::available(SessionInfo {
            current_desktop: Some("GNOME".to_owned()),
            session_desktop: Some("gnome".to_owned()),
            session_type: Some(SessionType::Wayland),
            session_type_raw: Some("wayland".to_owned()),
            wayland_display: Some("wayland-0".to_owned()),
            display: None,
        });
        snapshot.dbus = Section::available(DbusInfo {
            connected: true,
            checks: Vec::new(),
        });
        snapshot
    }

    fn finding(severity: Severity) -> Finding {
        Finding {
            id: "TEST001".to_owned(),
            severity,
            confidence: Confidence::High,
            title: "Test finding".to_owned(),
            summary: "Test summary".to_owned(),
            explanation: "Test explanation".to_owned(),
            evidence: Vec::new(),
            impact: None,
            recommendation: vec!["Test recommendation".to_owned()],
            source_component: "test".to_owned(),
        }
    }

    #[test]
    fn exit_codes_keep_warnings_successful() {
        let report = Report::new(
            runtime_ready_snapshot(),
            vec![finding(Severity::Warning)],
            "0.2.0",
        );
        assert_eq!(RunOutcome::from_report(&report), RunOutcome::Clean);
        assert_eq!(RunOutcome::Clean.exit_code(), 0);
    }

    #[test]
    fn severe_findings_return_one() {
        let report = Report::new(
            runtime_ready_snapshot(),
            vec![finding(Severity::Error)],
            "0.2.0",
        );
        assert_eq!(RunOutcome::from_report(&report), RunOutcome::SevereFindings);
        assert_eq!(RunOutcome::SevereFindings.exit_code(), 1);
    }

    #[test]
    fn missing_runtime_context_returns_three_before_finding_severity() {
        let mut snapshot = runtime_ready_snapshot();
        snapshot.dbus = Section::available(DbusInfo {
            connected: false,
            checks: Vec::new(),
        });
        let report = Report::new(snapshot, vec![finding(Severity::Critical)], "0.2.0");
        assert!(!minimum_runtime_context_available(&report.snapshot));
        assert_eq!(
            RunOutcome::from_report(&report),
            RunOutcome::RuntimeContextUnavailable
        );
        assert_eq!(RunOutcome::RuntimeContextUnavailable.exit_code(), 3);
    }

    #[test]
    fn x11_display_is_valid_minimum_context() {
        let mut snapshot = runtime_ready_snapshot();
        snapshot.session = Section::available(SessionInfo {
            current_desktop: Some("GNOME".to_owned()),
            session_desktop: Some("gnome".to_owned()),
            session_type: Some(SessionType::X11),
            session_type_raw: Some("x11".to_owned()),
            wayland_display: None,
            display: Some(":0".to_owned()),
        });
        assert!(minimum_runtime_context_available(&snapshot));
    }
}
