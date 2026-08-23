use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{CheckArgs, CheckDomain, Cli, PortalArgs, PortalCmd};
use crate::collectors;
use crate::error::Error;
use crate::model::finding::Finding;
use crate::model::portal::PortalRoute;
use crate::model::section::Section;
use crate::model::service::ServiceInfo;
use crate::model::snapshot::Snapshot;
use crate::report::{
    JsonRenderer, PortalExplainRenderer, PortalListRenderer, PortalRoutesRenderer, Renderer,
    Report, TerminalRenderer,
};
use crate::resolver;
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
        crate::cli::Command::Portal(args) => run_portal(&args, cli.json),
    }
}

fn run_check(args: &CheckArgs, json: bool, verbose: bool) -> Result<(), Error> {
    let collected = collect_snapshot();
    let findings = rules::engine::evaluate(&collected.snapshot);
    let findings = match args.domain {
        None => findings,
        Some(CheckDomain::Environment) => filter_findings(findings, is_environment_finding),
        Some(CheckDomain::Portal) => filter_findings(findings, is_portal_finding),
    };
    let report = Report::new(collected.snapshot, findings, env!("CARGO_PKG_VERSION"));
    write_report(&report, json, verbose)
}

fn run_portal(args: &PortalArgs, json: bool) -> Result<(), Error> {
    let collected = collect_snapshot();
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
        write_stdout(&rendered)
    } else {
        write_stdout(&rendered)
    }
}

/// Everything the current run collected, in one snapshot.
struct Collected {
    snapshot: Snapshot,
}

fn collect_snapshot() -> Collected {
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
    if let Some(backends) = &portal_backends.value {
        unit_names.extend(backends.iter().map(|b| ServiceInfo::backend_unit(&b.id)));
    }
    let services = collectors::systemd_user::collect(&unit_names);

    let mut snapshot = Snapshot::new(unix_epoch_ms());
    snapshot.system = system;
    snapshot.session = session;
    snapshot.environment = environment;
    snapshot.portal_config = portal_config;
    snapshot.portal_backends = portal_backends;
    snapshot.portal_routes = portal_routes;
    snapshot.dbus = dbus;
    snapshot.services = services;

    let findings = rules::engine::evaluate(&snapshot);
    tracing::debug!(findings = findings.len(), "evaluation finished");
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
    finding.id.starts_with("XDP") || finding.id.starts_with("CFG") || finding.id.starts_with("DBUS")
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
