use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{
    CheckArgs, CheckDomain, Cli, PortalArgs, PortalCmd, ProbeArgs, ProbeCmd, ReportArgs,
    ReportFormat,
};
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
    /// An explicit active probe completed with its own stable shell mapping.
    ActiveProbe { exit_code: u8 },
}

impl RunOutcome {
    /// Stable shell exit code for this completed diagnostic outcome.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Clean => 0,
            Self::SevereFindings => 1,
            Self::RuntimeContextUnavailable => 3,
            Self::ActiveProbe { exit_code } => exit_code,
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
        crate::cli::Command::Probe(args) => run_probe(&args, cli.json),
    }
}

fn run_probe(args: &ProbeArgs, json: bool) -> Result<RunOutcome, Error> {
    match &args.command {
        ProbeCmd::FileChooser => {
            eprintln!(
                "Warning: this explicit probe may open a desktop file chooser dialog. PortalDoctor will not read, copy, or modify the selected file."
            );
            let result = crate::probes::filechooser::run()?;
            let rendered = if json {
                serde_json::to_string_pretty(&result)
                    .map_err(|error| Error::ProbeOutput(error.to_string()))?
            } else {
                crate::probes::filechooser::render_terminal(&result)
            };
            write_stdout(&rendered)?;
            Ok(RunOutcome::ActiveProbe {
                exit_code: crate::probes::filechooser::exit_code(&result),
            })
        }
        ProbeCmd::Screenshot => {
            let result = crate::probes::screenshot::run()?;
            let rendered = if json {
                serde_json::to_string_pretty(&result)
                    .map_err(|error| Error::ProbeOutput(error.to_string()))?
            } else {
                crate::probes::screenshot::render_terminal(&result)
            };
            write_stdout(&rendered)?;
            Ok(RunOutcome::ActiveProbe {
                exit_code: crate::probes::screenshot::exit_code(&result),
            })
        }
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
    use super::{RunOutcome, minimum_runtime_context_available, selected_backend_dbus_names};
    use crate::model::dbus::{DbusCheck, DbusInfo, DbusOutcome, PORTAL_FRONTEND_NAME};
    use crate::model::environment::{SessionInfo, SessionType};
    use crate::model::finding::{Confidence, Finding, Severity};
    use crate::model::pipewire::{PipeWireInfo, WirePlumberInfo};
    use crate::model::portal::{PortalBackend, PortalConfigInfo, PortalRoute, RouteStatus};
    use crate::model::section::Section;
    use crate::model::service::{ServiceInfo, UnitState, UnitStatus};
    use crate::model::snapshot::Snapshot;
    use crate::report::Report;
    use crate::rules::engine::evaluate;
    use std::collections::{BTreeMap, BTreeSet};

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

    #[test]
    fn selected_kde_route_targets_the_kde_backend_bus_name() {
        const KDE_BACKEND: &str = "org.freedesktop.impl.portal.desktop.kde";

        let routes = Section::available(vec![PortalRoute {
            interface: "org.freedesktop.impl.portal.ScreenCast".to_owned(),
            requested_candidates: vec!["kde".to_owned()],
            available_candidates: vec!["kde".to_owned()],
            selected_candidates: vec!["kde".to_owned()],
            evidence: Vec::new(),
            status: RouteStatus::Selected,
        }]);
        let backends = Section::available(vec![PortalBackend {
            id: "kde".to_owned(),
            descriptor_path: "kde.portal".to_owned(),
            duplicate_descriptors: Vec::new(),
            dbus_name: KDE_BACKEND.to_owned(),
            interfaces: BTreeSet::new(),
            legacy_use_in: Vec::new(),
        }]);

        assert_eq!(
            selected_backend_dbus_names(&routes, &backends),
            vec![KDE_BACKEND.to_owned()]
        );
    }

    fn string_map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn kde_passive_snapshot(kde_outcome: DbusOutcome, kde_unit_state: UnitState) -> Snapshot {
        const KDE_BACKEND: &str = "org.freedesktop.impl.portal.desktop.kde";
        const SETTINGS: &str = "org.freedesktop.impl.portal.Settings";

        let process = string_map(&[
            ("XDG_CURRENT_DESKTOP", "KDE:Plasma"),
            ("XDG_SESSION_DESKTOP", "plasma"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
        ]);
        let session = crate::collectors::environment::session_info(&process);
        let environment =
            crate::collectors::environment::environment_info(process.clone(), None, Some(&process));

        let (preferences, parse_errors) = crate::collectors::portal_config::parse_config(
            include_str!("../tests/fixtures/portal-routing/kde-portals.conf"),
            "/fixture/kde-portals.conf",
            0,
        );
        let config = PortalConfigInfo {
            candidate_files: vec!["/fixture/kde-portals.conf".to_owned()],
            selected_file: Some("/fixture/kde-portals.conf".to_owned()),
            preferences,
            parse_errors,
        };
        let kde = crate::collectors::portal_files::parse_portal_file(
            include_str!("../tests/fixtures/portal-routing/kde.portal"),
            "/fixture/kde.portal",
            "kde".to_owned(),
        );
        // The fixture config references gtk for Settings. Keep that
        // descriptor in the controlled inventory so XDP005 tests the actual
        // aggregate inventory rather than a missing synthetic dependency.
        let gtk = PortalBackend {
            id: "gtk".to_owned(),
            descriptor_path: "/fixture/gtk.portal".to_owned(),
            duplicate_descriptors: Vec::new(),
            dbus_name: "org.freedesktop.impl.portal.desktop.gtk".to_owned(),
            interfaces: BTreeSet::from([SETTINGS.to_owned()]),
            legacy_use_in: Vec::new(),
        };
        let backends = vec![kde, gtk];
        let desktops = crate::resolver::portal_routes::normalize_desktops(
            process.get("XDG_CURRENT_DESKTOP").unwrap(),
        );
        let routes = crate::resolver::portal_routes::resolve_routes(&desktops, &config, &backends);

        let mut snapshot = Snapshot::new(0);
        snapshot.session = Section::available(session);
        snapshot.environment = Section::available(environment);
        snapshot.portal_config = Section::available(config);
        snapshot.portal_backends = Section::available(backends);
        snapshot.portal_routes = Section::available(routes);
        snapshot.dbus = Section::available(DbusInfo {
            connected: true,
            checks: vec![
                DbusCheck {
                    name: PORTAL_FRONTEND_NAME.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
                DbusCheck {
                    name: KDE_BACKEND.to_owned(),
                    outcome: kde_outcome,
                },
            ],
        });
        snapshot.services = Section::available(ServiceInfo {
            units: vec![
                UnitStatus {
                    unit: ServiceInfo::frontend_unit().to_owned(),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("kde"),
                    state: kde_unit_state,
                    sub_state: Some(kde_unit_state.as_str().to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
            ],
        });
        snapshot.pipewire = Section::available(PipeWireInfo {
            model_version: 1,
            version: Some("fixture".to_owned()),
            object_count: 1,
            node_count: 0,
            link_count: 0,
            portal_client_count: 0,
            screen_cast_source_count: 1,
            nodes: Vec::new(),
            links: Vec::new(),
        });
        snapshot.wireplumber = Section::available(WirePlumberInfo {
            model_version: 1,
            pipewire_version: Some("fixture".to_owned()),
            wireplumber_client_count: 1,
        });
        snapshot
    }

    fn sway_fixture_vars() -> BTreeMap<String, String> {
        include_str!("../tests/fixtures/environment/sway-wayland.env")
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect()
    }

    fn hyprland_fixture_vars() -> BTreeMap<String, String> {
        include_str!("../tests/fixtures/environment/hyprland-wayland.env")
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect()
    }

    fn hyprland_passive_snapshot(with_wayland_display: bool) -> Snapshot {
        const HYPRLAND_BACKEND: &str = "org.freedesktop.impl.portal.desktop.hyprland";
        const GTK_BACKEND: &str = "org.freedesktop.impl.portal.desktop.gtk";

        let mut process = hyprland_fixture_vars();
        if !with_wayland_display {
            process.remove("WAYLAND_DISPLAY");
        }
        let session = crate::collectors::environment::session_info(&process);
        let environment =
            crate::collectors::environment::environment_info(process.clone(), None, Some(&process));

        let (preferences, parse_errors) = crate::collectors::portal_config::parse_config(
            include_str!("../tests/fixtures/portal-routing/hyprland-portals.conf"),
            "/fixture/hyprland-portals.conf",
            0,
        );
        let config = PortalConfigInfo {
            candidate_files: vec!["/fixture/hyprland-portals.conf".to_owned()],
            selected_file: Some("/fixture/hyprland-portals.conf".to_owned()),
            preferences,
            parse_errors,
        };
        let backends = vec![
            crate::collectors::portal_files::parse_portal_file(
                include_str!("../tests/fixtures/portal-routing/hyprland.portal"),
                "/fixture/hyprland.portal",
                "hyprland".to_owned(),
            ),
            crate::collectors::portal_files::parse_portal_file(
                include_str!("../tests/fixtures/portal-routing/gtk.portal"),
                "/fixture/gtk.portal",
                "gtk".to_owned(),
            ),
        ];
        let desktops = crate::resolver::portal_routes::normalize_desktops(
            process
                .get("XDG_CURRENT_DESKTOP")
                .expect("Hyprland fixture desktop identity"),
        );
        let routes = crate::resolver::portal_routes::resolve_routes(&desktops, &config, &backends);

        let mut snapshot = Snapshot::new(0);
        snapshot.session = Section::available(session);
        snapshot.environment = Section::available(environment);
        snapshot.portal_config = Section::available(config);
        snapshot.portal_backends = Section::available(backends);
        snapshot.portal_routes = Section::available(routes);
        snapshot.dbus = Section::available(DbusInfo {
            connected: true,
            checks: vec![
                DbusCheck {
                    name: PORTAL_FRONTEND_NAME.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
                DbusCheck {
                    name: HYPRLAND_BACKEND.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
                DbusCheck {
                    name: GTK_BACKEND.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
            ],
        });
        snapshot.services = Section::available(ServiceInfo {
            units: vec![
                UnitStatus {
                    unit: ServiceInfo::frontend_unit().to_owned(),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("hyprland"),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("gtk"),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
            ],
        });
        snapshot.pipewire = Section::available(PipeWireInfo {
            model_version: 1,
            version: Some("fixture".to_owned()),
            object_count: 1,
            node_count: 0,
            link_count: 0,
            portal_client_count: 0,
            screen_cast_source_count: 1,
            nodes: Vec::new(),
            links: Vec::new(),
        });
        snapshot.wireplumber = Section::available(WirePlumberInfo {
            model_version: 1,
            pipewire_version: Some("fixture".to_owned()),
            wireplumber_client_count: 1,
        });
        snapshot
    }

    fn sway_passive_snapshot(with_wayland_display: bool) -> Snapshot {
        const WLR_BACKEND: &str = "org.freedesktop.impl.portal.desktop.wlr";
        const GTK_BACKEND: &str = "org.freedesktop.impl.portal.desktop.gtk";

        let mut process = sway_fixture_vars();
        if !with_wayland_display {
            process.remove("WAYLAND_DISPLAY");
        }
        let session = crate::collectors::environment::session_info(&process);
        let environment =
            crate::collectors::environment::environment_info(process.clone(), None, Some(&process));

        let (preferences, parse_errors) = crate::collectors::portal_config::parse_config(
            include_str!("../tests/fixtures/portal-routing/sway-portals.conf"),
            "/fixture/sway-portals.conf",
            0,
        );
        let config = PortalConfigInfo {
            candidate_files: vec!["/fixture/sway-portals.conf".to_owned()],
            selected_file: Some("/fixture/sway-portals.conf".to_owned()),
            preferences,
            parse_errors,
        };
        let backends = vec![
            crate::collectors::portal_files::parse_portal_file(
                include_str!("../tests/fixtures/portal-routing/wlr.portal"),
                "/fixture/wlr.portal",
                "wlr".to_owned(),
            ),
            crate::collectors::portal_files::parse_portal_file(
                include_str!("../tests/fixtures/portal-routing/gtk.portal"),
                "/fixture/gtk.portal",
                "gtk".to_owned(),
            ),
        ];
        let desktops = crate::resolver::portal_routes::normalize_desktops(
            process
                .get("XDG_CURRENT_DESKTOP")
                .expect("Sway fixture desktop identity"),
        );
        let routes = crate::resolver::portal_routes::resolve_routes(&desktops, &config, &backends);

        let mut snapshot = Snapshot::new(0);
        snapshot.session = Section::available(session);
        snapshot.environment = Section::available(environment);
        snapshot.portal_config = Section::available(config);
        snapshot.portal_backends = Section::available(backends);
        snapshot.portal_routes = Section::available(routes);
        snapshot.dbus = Section::available(DbusInfo {
            connected: true,
            checks: vec![
                DbusCheck {
                    name: PORTAL_FRONTEND_NAME.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
                DbusCheck {
                    name: WLR_BACKEND.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
                DbusCheck {
                    name: GTK_BACKEND.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
            ],
        });
        snapshot.services = Section::available(ServiceInfo {
            units: vec![
                UnitStatus {
                    unit: ServiceInfo::frontend_unit().to_owned(),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("wlr"),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("gtk"),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
            ],
        });
        snapshot.pipewire = Section::available(PipeWireInfo {
            model_version: 1,
            version: Some("fixture".to_owned()),
            object_count: 1,
            node_count: 0,
            link_count: 0,
            portal_client_count: 0,
            screen_cast_source_count: 1,
            nodes: Vec::new(),
            links: Vec::new(),
        });
        snapshot.wireplumber = Section::available(WirePlumberInfo {
            model_version: 1,
            pipewire_version: Some("fixture".to_owned()),
            wireplumber_client_count: 1,
        });
        snapshot
    }

    fn assert_sway_mixed_routes(snapshot: &Snapshot) {
        let routes = snapshot
            .portal_routes
            .value
            .as_ref()
            .expect("Sway route table");
        for (interface, backend) in [
            ("org.freedesktop.impl.portal.Screenshot", "wlr"),
            ("org.freedesktop.impl.portal.ScreenCast", "wlr"),
            ("org.freedesktop.impl.portal.FileChooser", "gtk"),
            ("org.freedesktop.impl.portal.Settings", "gtk"),
        ] {
            let route = routes
                .iter()
                .find(|route| route.interface == interface)
                .unwrap_or_else(|| panic!("missing Sway route for {interface}"));
            assert_eq!(route.status, RouteStatus::Selected, "{interface}");
            assert_eq!(route.selected_candidates, [backend], "{interface}");
        }
    }

    fn sway_runtime_snapshot(
        wlr_outcome: DbusOutcome,
        gtk_outcome: DbusOutcome,
        wlr_unit_state: UnitState,
        gtk_unit_state: UnitState,
    ) -> Snapshot {
        const WLR_BACKEND: &str = "org.freedesktop.impl.portal.desktop.wlr";
        const GTK_BACKEND: &str = "org.freedesktop.impl.portal.desktop.gtk";

        let mut snapshot = sway_passive_snapshot(true);
        snapshot.dbus = Section::available(DbusInfo {
            connected: true,
            checks: vec![
                DbusCheck {
                    name: PORTAL_FRONTEND_NAME.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
                DbusCheck {
                    name: WLR_BACKEND.to_owned(),
                    outcome: wlr_outcome,
                },
                DbusCheck {
                    name: GTK_BACKEND.to_owned(),
                    outcome: gtk_outcome,
                },
            ],
        });
        snapshot.services = Section::available(ServiceInfo {
            units: vec![
                UnitStatus {
                    unit: ServiceInfo::frontend_unit().to_owned(),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("wlr"),
                    state: wlr_unit_state,
                    sub_state: Some(wlr_unit_state.as_str().to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("gtk"),
                    state: gtk_unit_state,
                    sub_state: Some(gtk_unit_state.as_str().to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
            ],
        });
        snapshot
    }

    fn assert_sway_runtime_bindings(snapshot: &Snapshot) {
        assert_eq!(
            selected_backend_dbus_names(&snapshot.portal_routes, &snapshot.portal_backends),
            vec![
                "org.freedesktop.impl.portal.desktop.gtk".to_owned(),
                "org.freedesktop.impl.portal.desktop.wlr".to_owned(),
            ]
        );
        let services = snapshot.services.value.as_ref().expect("Sway services");
        assert_eq!(
            ServiceInfo::backend_unit("wlr"),
            "xdg-desktop-portal-wlr.service"
        );
        assert_eq!(
            ServiceInfo::backend_unit("gtk"),
            "xdg-desktop-portal-gtk.service"
        );
        assert!(services.unit("xdg-desktop-portal-wlr.service").is_some());
        assert!(services.unit("xdg-desktop-portal-gtk.service").is_some());
    }

    fn assert_hyprland_mixed_routes(snapshot: &Snapshot) {
        let routes = snapshot
            .portal_routes
            .value
            .as_ref()
            .expect("Hyprland route table");
        for (interface, backend) in [
            ("org.freedesktop.impl.portal.Screenshot", "hyprland"),
            ("org.freedesktop.impl.portal.ScreenCast", "hyprland"),
            ("org.freedesktop.impl.portal.FileChooser", "gtk"),
            ("org.freedesktop.impl.portal.Settings", "gtk"),
        ] {
            let route = routes
                .iter()
                .find(|route| route.interface == interface)
                .unwrap_or_else(|| panic!("missing Hyprland route for {interface}"));
            assert_eq!(route.status, RouteStatus::Selected, "{interface}");
            assert_eq!(route.selected_candidates, [backend], "{interface}");
        }
    }

    fn hyprland_runtime_snapshot(
        hyprland_outcome: DbusOutcome,
        gtk_outcome: DbusOutcome,
        hyprland_unit_state: UnitState,
        gtk_unit_state: UnitState,
    ) -> Snapshot {
        const HYPRLAND_BACKEND: &str = "org.freedesktop.impl.portal.desktop.hyprland";
        const GTK_BACKEND: &str = "org.freedesktop.impl.portal.desktop.gtk";

        let mut snapshot = hyprland_passive_snapshot(true);
        snapshot.dbus = Section::available(DbusInfo {
            connected: true,
            checks: vec![
                DbusCheck {
                    name: PORTAL_FRONTEND_NAME.to_owned(),
                    outcome: DbusOutcome::HasOwner,
                },
                DbusCheck {
                    name: HYPRLAND_BACKEND.to_owned(),
                    outcome: hyprland_outcome,
                },
                DbusCheck {
                    name: GTK_BACKEND.to_owned(),
                    outcome: gtk_outcome,
                },
            ],
        });
        snapshot.services = Section::available(ServiceInfo {
            units: vec![
                UnitStatus {
                    unit: ServiceInfo::frontend_unit().to_owned(),
                    state: UnitState::Active,
                    sub_state: Some("running".to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("hyprland"),
                    state: hyprland_unit_state,
                    sub_state: Some(hyprland_unit_state.as_str().to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
                UnitStatus {
                    unit: ServiceInfo::backend_unit("gtk"),
                    state: gtk_unit_state,
                    sub_state: Some(gtk_unit_state.as_str().to_owned()),
                    unit_file_state: Some("static".to_owned()),
                },
            ],
        });
        snapshot
    }

    fn assert_hyprland_runtime_bindings(snapshot: &Snapshot) {
        assert_eq!(
            selected_backend_dbus_names(&snapshot.portal_routes, &snapshot.portal_backends),
            vec![
                "org.freedesktop.impl.portal.desktop.gtk".to_owned(),
                "org.freedesktop.impl.portal.desktop.hyprland".to_owned(),
            ]
        );
        let services = snapshot.services.value.as_ref().expect("Hyprland services");
        assert_eq!(
            ServiceInfo::backend_unit("hyprland"),
            "xdg-desktop-portal-hyprland.service"
        );
        assert_eq!(
            ServiceInfo::backend_unit("gtk"),
            "xdg-desktop-portal-gtk.service"
        );
        assert!(
            services
                .unit("xdg-desktop-portal-hyprland.service")
                .is_some()
        );
        assert!(services.unit("xdg-desktop-portal-gtk.service").is_some());
    }

    fn assert_kde_routes_and_backend(snapshot: &Snapshot) {
        const KDE_BACKEND: &str = "org.freedesktop.impl.portal.desktop.kde";
        for interface in [
            "org.freedesktop.impl.portal.FileChooser",
            "org.freedesktop.impl.portal.Screenshot",
            "org.freedesktop.impl.portal.ScreenCast",
        ] {
            let route = snapshot
                .portal_routes
                .value
                .as_ref()
                .unwrap()
                .iter()
                .find(|route| route.interface == interface)
                .unwrap();
            assert_eq!(route.status, RouteStatus::Selected, "{interface}");
            assert_eq!(route.selected_candidates, ["kde"], "{interface}");
        }
        assert_eq!(
            selected_backend_dbus_names(&snapshot.portal_routes, &snapshot.portal_backends),
            vec![KDE_BACKEND.to_owned()]
        );
    }

    fn finding_ids(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|finding| finding.id.as_str()).collect()
    }

    #[test]
    fn aggregate_kde_plasma_wayland_healthy_passive_stack_is_clean() {
        let snapshot = kde_passive_snapshot(DbusOutcome::HasOwner, UnitState::Active);
        assert_kde_routes_and_backend(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert!(findings.is_empty());
        let report = Report::new(snapshot, findings, "0.2.1");
        assert!(minimum_runtime_context_available(&report.snapshot));
        assert_eq!(RunOutcome::from_report(&report), RunOutcome::Clean);
    }

    #[test]
    fn aggregate_hyprland_wayland_mixed_passive_stack_is_clean() {
        let snapshot = hyprland_passive_snapshot(true);
        assert_hyprland_mixed_routes(&snapshot);
        assert_hyprland_runtime_bindings(&snapshot);

        let routes = snapshot.portal_routes.value.as_ref().unwrap();
        let screenshot = routes
            .iter()
            .find(|route| route.interface == "org.freedesktop.impl.portal.Screenshot")
            .unwrap();
        assert_eq!(screenshot.requested_candidates, ["hyprland", "gtk"]);
        assert_eq!(
            snapshot
                .portal_config
                .value
                .as_ref()
                .unwrap()
                .selected_file
                .as_deref(),
            Some("/fixture/hyprland-portals.conf")
        );
        assert_eq!(snapshot.portal_backends.value.as_ref().unwrap().len(), 2);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert!(findings.is_empty());
        let report = Report::new(snapshot, findings, "0.2.1");
        assert!(minimum_runtime_context_available(&report.snapshot));
        assert_eq!(RunOutcome::from_report(&report), RunOutcome::Clean);
    }

    #[test]
    fn aggregate_hyprland_without_wayland_display_emits_env003_only() {
        let snapshot = hyprland_passive_snapshot(false);
        assert_hyprland_mixed_routes(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["ENV003"]);
        assert_eq!(
            RunOutcome::from_report(&Report::new(snapshot, findings, "0.2.1")),
            RunOutcome::RuntimeContextUnavailable
        );
    }

    #[test]
    fn hyprland_use_in_excludes_capture_backend_for_wrong_desktop() {
        let (preferences, parse_errors) = crate::collectors::portal_config::parse_config(
            include_str!("../tests/fixtures/portal-routing/hyprland-portals.conf"),
            "/fixture/hyprland-portals.conf",
            0,
        );
        assert!(parse_errors.is_empty());
        let config = PortalConfigInfo {
            candidate_files: vec!["/fixture/hyprland-portals.conf".to_owned()],
            selected_file: Some("/fixture/hyprland-portals.conf".to_owned()),
            preferences,
            parse_errors,
        };
        let backends = vec![
            crate::collectors::portal_files::parse_portal_file(
                include_str!("../tests/fixtures/portal-routing/hyprland.portal"),
                "/fixture/hyprland.portal",
                "hyprland".to_owned(),
            ),
            crate::collectors::portal_files::parse_portal_file(
                include_str!("../tests/fixtures/portal-routing/gtk.portal"),
                "/fixture/gtk.portal",
                "gtk".to_owned(),
            ),
        ];
        let routes = crate::resolver::portal_routes::resolve_routes(
            &crate::resolver::portal_routes::normalize_desktops("GNOME"),
            &config,
            &backends,
        );
        for interface in [
            "org.freedesktop.impl.portal.Screenshot",
            "org.freedesktop.impl.portal.ScreenCast",
        ] {
            let route = routes
                .iter()
                .find(|route| route.interface == interface)
                .unwrap();
            assert!(route.available_candidates.is_empty(), "{interface}");
            assert!(route.selected_candidates.is_empty(), "{interface}");
            assert_eq!(route.status, RouteStatus::NoProvider, "{interface}");
            assert!(
                route
                    .evidence
                    .iter()
                    .any(|evidence| evidence.message.contains("excluded by UseIn")),
                "{interface}"
            );
        }
        let file_chooser = routes
            .iter()
            .find(|route| route.interface == "org.freedesktop.impl.portal.FileChooser")
            .unwrap();
        assert_eq!(file_chooser.selected_candidates, ["gtk"]);
    }

    #[test]
    fn aggregate_hyprland_backend_missing_is_only_generic_dbus002() {
        let snapshot = hyprland_runtime_snapshot(
            DbusOutcome::NoOwner,
            DbusOutcome::HasOwner,
            UnitState::NotFound,
            UnitState::Active,
        );
        assert_hyprland_mixed_routes(&snapshot);
        assert_hyprland_runtime_bindings(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["DBUS002"]);
        assert!(
            findings[0]
                .summary
                .contains("org.freedesktop.impl.portal.desktop.hyprland")
        );
    }

    #[test]
    fn aggregate_hyprland_gtk_fallback_missing_is_only_generic_dbus002() {
        let snapshot = hyprland_runtime_snapshot(
            DbusOutcome::HasOwner,
            DbusOutcome::NoOwner,
            UnitState::Active,
            UnitState::NotFound,
        );
        assert_hyprland_mixed_routes(&snapshot);
        assert_hyprland_runtime_bindings(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["DBUS002"]);
        assert!(
            findings[0]
                .summary
                .contains("org.freedesktop.impl.portal.desktop.gtk")
        );
    }

    #[test]
    fn aggregate_sway_wayland_mixed_passive_stack_is_clean() {
        let snapshot = sway_passive_snapshot(true);
        assert_sway_mixed_routes(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert!(findings.is_empty());
        let report = Report::new(snapshot, findings, "0.2.1");
        assert!(minimum_runtime_context_available(&report.snapshot));
        assert_eq!(RunOutcome::from_report(&report), RunOutcome::Clean);
    }

    #[test]
    fn aggregate_sway_without_wayland_display_emits_env003_only() {
        let snapshot = sway_passive_snapshot(false);
        assert_sway_mixed_routes(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["ENV003"]);
        assert_eq!(
            RunOutcome::from_report(&Report::new(snapshot, findings, "0.2.1")),
            RunOutcome::RuntimeContextUnavailable
        );
    }

    #[test]
    fn aggregate_sway_runtime_healthy_wlr_and_gtk_are_silent() {
        let snapshot = sway_runtime_snapshot(
            DbusOutcome::HasOwner,
            DbusOutcome::HasOwner,
            UnitState::Active,
            UnitState::Active,
        );
        assert_sway_mixed_routes(&snapshot);
        assert_sway_runtime_bindings(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert!(findings.is_empty());
    }

    #[test]
    fn aggregate_sway_wlr_missing_is_only_generic_dbus002() {
        let snapshot = sway_runtime_snapshot(
            DbusOutcome::NoOwner,
            DbusOutcome::HasOwner,
            UnitState::NotFound,
            UnitState::Active,
        );
        assert_sway_mixed_routes(&snapshot);
        assert_sway_runtime_bindings(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["DBUS002"]);
        assert!(
            findings[0]
                .summary
                .contains("org.freedesktop.impl.portal.desktop.wlr")
        );
    }

    #[test]
    fn aggregate_sway_wlr_activation_failure_is_only_generic_dbus002() {
        let snapshot = sway_runtime_snapshot(
            DbusOutcome::ActivationFailure,
            DbusOutcome::HasOwner,
            UnitState::Failed,
            UnitState::Active,
        );
        assert_sway_mixed_routes(&snapshot);
        assert_sway_runtime_bindings(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["DBUS002"]);
        assert!(
            findings[0]
                .summary
                .contains("org.freedesktop.impl.portal.desktop.wlr")
        );
    }

    #[test]
    fn aggregate_sway_gtk_fallback_missing_is_only_generic_dbus002() {
        let snapshot = sway_runtime_snapshot(
            DbusOutcome::HasOwner,
            DbusOutcome::NoOwner,
            UnitState::Active,
            UnitState::NotFound,
        );
        assert_sway_mixed_routes(&snapshot);
        assert_sway_runtime_bindings(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["DBUS002"]);
        assert!(
            findings[0]
                .summary
                .contains("org.freedesktop.impl.portal.desktop.gtk")
        );
    }

    #[test]
    fn aggregate_kde_backend_missing_is_a_runtime_finding_only() {
        let snapshot = kde_passive_snapshot(DbusOutcome::NoOwner, UnitState::NotFound);
        assert_kde_routes_and_backend(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["DBUS002"]);
        assert!(
            findings[0]
                .summary
                .contains("org.freedesktop.impl.portal.desktop.kde")
        );
        assert_eq!(
            RunOutcome::from_report(&Report::new(snapshot, findings, "0.2.1")),
            RunOutcome::Clean
        );
    }

    #[test]
    fn aggregate_kde_backend_activation_failure_is_a_runtime_finding_only() {
        let snapshot = kde_passive_snapshot(DbusOutcome::ActivationFailure, UnitState::Failed);
        assert_kde_routes_and_backend(&snapshot);

        let findings = evaluate(&snapshot);
        crate::rules::contract::assert_contract(&findings);
        assert_eq!(finding_ids(&findings), ["DBUS002"]);
        assert!(
            findings[0]
                .summary
                .contains("org.freedesktop.impl.portal.desktop.kde")
        );
        assert_eq!(
            RunOutcome::from_report(&Report::new(snapshot, findings, "0.2.1")),
            RunOutcome::Clean
        );
    }

    /// Run only from `scripts/validate-kde-runtime-aggregate-ci.sh`: both
    /// runtime collectors must use the isolated D-Bus/fake-systemctl setup.
    #[test]
    #[cfg(unix)]
    #[ignore = "requires the explicit isolated KDE runtime aggregate gate"]
    #[allow(clippy::too_many_lines)]
    fn isolated_kde_runtime_collectors_feed_the_passive_rule_pipeline() {
        use std::fs;
        use std::path::Path;

        const FRONTEND: &str = PORTAL_FRONTEND_NAME;
        const KDE_BACKEND: &str = "org.freedesktop.impl.portal.desktop.kde";
        const KDE_UNIT: &str = "xdg-desktop-portal-kde.service";

        let mode_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_MODE_FILE")
            .expect("explicit aggregate wrapper must provide a mode file");
        let log_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_LOG")
            .expect("explicit aggregate wrapper must provide an invocation log");
        let fake_dir = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_FAKE_DIR")
            .expect("explicit aggregate wrapper must provide a fake directory");
        let path = std::env::var_os("PATH").expect("aggregate wrapper must provide PATH");
        assert_eq!(
            std::env::split_paths(&path).next(),
            Some(Path::new(&fake_dir).to_path_buf())
        );
        assert_eq!(
            std::env::var("PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD").as_deref(),
            Ok("isolated-kde-runtime-aggregate")
        );

        let set_mode = |mode: &str| {
            fs::write(&mode_file, format!("{mode}\n")).expect("write aggregate fake mode");
        };
        let collect_runtime = || {
            let dbus = crate::collectors::dbus::collect(&[KDE_BACKEND.to_owned()]);
            let services = crate::collectors::systemd_user::collect(&[
                ServiceInfo::frontend_unit().to_owned(),
                KDE_UNIT.to_owned(),
            ]);
            (dbus, services)
        };

        let owner = zbus::blocking::Connection::session().expect("isolated session bus");
        owner
            .request_name(FRONTEND)
            .expect("acquire isolated portal frontend name");
        owner
            .request_name(KDE_BACKEND)
            .expect("acquire isolated KDE backend name");

        set_mode("healthy");
        let (healthy_dbus, healthy_services) = collect_runtime();
        let healthy_dbus_info = healthy_dbus.value.as_ref().expect("healthy D-Bus result");
        assert!(healthy_dbus_info.connected);
        assert_eq!(
            healthy_dbus_info
                .checks
                .iter()
                .find(|check| check.name == FRONTEND)
                .expect("frontend D-Bus check")
                .outcome,
            DbusOutcome::HasOwner
        );
        assert_eq!(
            healthy_dbus_info
                .checks
                .iter()
                .find(|check| check.name == KDE_BACKEND)
                .expect("KDE D-Bus check")
                .outcome,
            DbusOutcome::HasOwner
        );
        let healthy_service_info = healthy_services.value.as_ref().expect("healthy services");
        assert_eq!(
            healthy_service_info
                .unit(KDE_UNIT)
                .expect("KDE service")
                .state,
            UnitState::Active
        );
        assert_eq!(
            healthy_service_info
                .unit(ServiceInfo::frontend_unit())
                .expect("frontend service")
                .state,
            UnitState::Active
        );
        let mut healthy = kde_passive_snapshot(DbusOutcome::HasOwner, UnitState::Active);
        healthy.dbus = healthy_dbus;
        healthy.services = healthy_services;
        assert_kde_routes_and_backend(&healthy);
        let healthy_findings = evaluate(&healthy);
        crate::rules::contract::assert_contract(&healthy_findings);
        assert!(healthy_findings.is_empty());
        assert_eq!(
            RunOutcome::from_report(&Report::new(healthy, healthy_findings, "0.2.1")),
            RunOutcome::Clean
        );

        owner
            .release_name(KDE_BACKEND)
            .expect("release isolated KDE backend name");

        set_mode("missing");
        let (missing_dbus, missing_services) = collect_runtime();
        let missing_info = missing_dbus.value.as_ref().expect("missing D-Bus result");
        assert!(missing_info.connected);
        assert_eq!(
            missing_info
                .checks
                .iter()
                .find(|check| check.name == KDE_BACKEND)
                .expect("missing KDE D-Bus check")
                .outcome,
            DbusOutcome::NoOwner
        );
        assert_eq!(
            missing_services
                .value
                .as_ref()
                .expect("missing services")
                .unit(KDE_UNIT)
                .expect("missing KDE service")
                .state,
            UnitState::NotFound
        );
        let mut missing = kde_passive_snapshot(DbusOutcome::NoOwner, UnitState::NotFound);
        missing.dbus = missing_dbus;
        missing.services = missing_services;
        let missing_findings = evaluate(&missing);
        crate::rules::contract::assert_contract(&missing_findings);
        assert_eq!(finding_ids(&missing_findings), ["DBUS002"]);
        assert_eq!(
            RunOutcome::from_report(&Report::new(missing, missing_findings, "0.2.1")),
            RunOutcome::Clean
        );

        set_mode("failed");
        let (failed_dbus, failed_services) = collect_runtime();
        let failed_info = failed_dbus.value.as_ref().expect("failed D-Bus result");
        assert!(failed_info.connected);
        assert_eq!(
            failed_info
                .checks
                .iter()
                .find(|check| check.name == KDE_BACKEND)
                .expect("failed KDE D-Bus check")
                .outcome,
            DbusOutcome::NoOwner
        );
        assert_eq!(
            failed_services
                .value
                .as_ref()
                .expect("failed services")
                .unit(KDE_UNIT)
                .expect("failed KDE service")
                .state,
            UnitState::Failed
        );
        let mut failed = kde_passive_snapshot(DbusOutcome::NoOwner, UnitState::Failed);
        failed.dbus = failed_dbus;
        failed.services = failed_services;
        let failed_findings = evaluate(&failed);
        crate::rules::contract::assert_contract(&failed_findings);
        assert_eq!(finding_ids(&failed_findings), ["DBUS002"]);
        assert_eq!(
            RunOutcome::from_report(&Report::new(failed, failed_findings, "0.2.1")),
            RunOutcome::Clean
        );

        let invocations = fs::read_to_string(log_file).expect("aggregate invocation log");
        assert_eq!(invocations.lines().count(), 6);
        assert!(invocations.lines().all(|line| {
            matches!(
                line,
                "--user show xdg-desktop-portal.service -p ActiveState -p SubState -p UnitFileState --value"
                    | "--user show xdg-desktop-portal-kde.service -p ActiveState -p SubState -p UnitFileState --value"
            )
        }));
    }

    /// Run only from `scripts/validate-sway-runtime-aggregate-ci.sh`: both
    /// runtime collectors must use the isolated D-Bus/fake-systemctl setup.
    #[test]
    #[cfg(unix)]
    #[ignore = "requires the explicit isolated Sway runtime aggregate gate"]
    #[allow(clippy::too_many_lines)]
    fn isolated_sway_runtime_collectors_feed_the_passive_rule_pipeline() {
        use std::fs;
        use std::path::Path;

        const FRONTEND: &str = PORTAL_FRONTEND_NAME;
        const WLR_BACKEND: &str = "org.freedesktop.impl.portal.desktop.wlr";
        const GTK_BACKEND: &str = "org.freedesktop.impl.portal.desktop.gtk";
        const WLR_UNIT: &str = "xdg-desktop-portal-wlr.service";
        const GTK_UNIT: &str = "xdg-desktop-portal-gtk.service";

        fn set_name(owner: &zbus::blocking::Connection, name: &str, wanted: bool, held: &mut bool) {
            if wanted && !*held {
                owner
                    .request_name(name)
                    .expect("acquire isolated Sway portal name");
                *held = true;
            } else if !wanted && *held {
                owner
                    .release_name(name)
                    .expect("release isolated Sway portal name");
                *held = false;
            }
        }

        let mode_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_MODE_FILE")
            .expect("explicit Sway aggregate wrapper must provide a mode file");
        let log_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_LOG")
            .expect("explicit Sway aggregate wrapper must provide an invocation log");
        let fake_dir = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_FAKE_DIR")
            .expect("explicit Sway aggregate wrapper must provide a fake directory");
        let path = std::env::var_os("PATH").expect("Sway aggregate wrapper must provide PATH");
        assert_eq!(
            std::env::split_paths(&path).next(),
            Some(Path::new(&fake_dir).to_path_buf())
        );
        assert_eq!(
            std::env::var("PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD").as_deref(),
            Ok("isolated-sway-runtime-aggregate")
        );

        let base = sway_passive_snapshot(true);
        assert_sway_mixed_routes(&base);
        assert_sway_runtime_bindings(&base);
        // The production D-Bus collector receives names derived from the
        // parsed descriptors and resolved routes, not a parallel test map.
        let selected_backend_names =
            selected_backend_dbus_names(&base.portal_routes, &base.portal_backends);
        assert_eq!(
            selected_backend_names,
            vec![GTK_BACKEND.to_owned(), WLR_BACKEND.to_owned()]
        );
        let selected_units = vec![
            ServiceInfo::frontend_unit().to_owned(),
            ServiceInfo::backend_unit("wlr"),
            ServiceInfo::backend_unit("gtk"),
        ];
        assert_eq!(
            selected_units,
            [
                ServiceInfo::frontend_unit().to_owned(),
                WLR_UNIT.to_owned(),
                GTK_UNIT.to_owned(),
            ]
        );

        let set_mode = |mode: &str| {
            fs::write(&mode_file, format!("{mode}\n")).expect("write Sway aggregate fake mode");
        };
        let collect_runtime = || {
            let dbus = crate::collectors::dbus::collect(&selected_backend_names);
            let services = crate::collectors::systemd_user::collect(&selected_units);
            (dbus, services)
        };
        let owner = zbus::blocking::Connection::session().expect("isolated session bus");
        owner
            .request_name(FRONTEND)
            .expect("acquire isolated portal frontend name");
        let mut wlr_owned = false;
        let mut gtk_owned = false;

        let mut run_scenario = |mode: &str,
                                want_wlr: bool,
                                want_gtk: bool,
                                expected_wlr: DbusOutcome,
                                expected_gtk: DbusOutcome,
                                expected_wlr_state: UnitState,
                                expected_gtk_state: UnitState,
                                expected_summary: Option<&str>,
                                expected_findings: &[&str]| {
            set_name(&owner, WLR_BACKEND, want_wlr, &mut wlr_owned);
            set_name(&owner, GTK_BACKEND, want_gtk, &mut gtk_owned);
            set_mode(mode);
            let (dbus, services) = collect_runtime();
            let dbus_info = dbus.value.as_ref().expect("Sway D-Bus result");
            assert!(dbus_info.connected);
            let names: Vec<&str> = dbus_info
                .checks
                .iter()
                .map(|check| check.name.as_str())
                .collect();
            assert_eq!(names, vec![FRONTEND, GTK_BACKEND, WLR_BACKEND]);
            assert_eq!(
                &dbus_info
                    .checks
                    .iter()
                    .find(|check| check.name == WLR_BACKEND)
                    .expect("WLR D-Bus check")
                    .outcome,
                &expected_wlr
            );
            assert_eq!(
                &dbus_info
                    .checks
                    .iter()
                    .find(|check| check.name == GTK_BACKEND)
                    .expect("GTK D-Bus check")
                    .outcome,
                &expected_gtk
            );
            let service_info = services.value.as_ref().expect("Sway service result");
            assert_eq!(
                service_info
                    .unit(ServiceInfo::frontend_unit())
                    .expect("portal frontend unit")
                    .state,
                UnitState::Active
            );
            assert_eq!(
                service_info.unit(WLR_UNIT).expect("WLR unit").state,
                expected_wlr_state
            );
            assert_eq!(
                service_info.unit(GTK_UNIT).expect("GTK unit").state,
                expected_gtk_state
            );

            let mut snapshot = base.clone();
            snapshot.dbus = dbus;
            snapshot.services = services;
            assert_sway_mixed_routes(&snapshot);
            let findings = evaluate(&snapshot);
            crate::rules::contract::assert_contract(&findings);
            assert_eq!(finding_ids(&findings), expected_findings);
            if let Some(summary) = expected_summary {
                assert!(findings[0].summary.contains(summary));
            }
            assert_eq!(
                RunOutcome::from_report(&Report::new(snapshot, findings, "0.2.1")),
                RunOutcome::Clean
            );
        };

        run_scenario(
            "healthy",
            true,
            true,
            DbusOutcome::HasOwner,
            DbusOutcome::HasOwner,
            UnitState::Active,
            UnitState::Active,
            None,
            &[],
        );
        run_scenario(
            "wlr-missing",
            false,
            true,
            DbusOutcome::NoOwner,
            DbusOutcome::HasOwner,
            UnitState::NotFound,
            UnitState::Active,
            Some(WLR_BACKEND),
            &["DBUS002"],
        );
        run_scenario(
            "gtk-missing",
            true,
            false,
            DbusOutcome::HasOwner,
            DbusOutcome::NoOwner,
            UnitState::Active,
            UnitState::NotFound,
            Some(GTK_BACKEND),
            &["DBUS002"],
        );
        run_scenario(
            "wlr-failed",
            false,
            true,
            DbusOutcome::NoOwner,
            DbusOutcome::HasOwner,
            UnitState::Failed,
            UnitState::Active,
            Some(WLR_BACKEND),
            &["DBUS002"],
        );

        set_name(&owner, WLR_BACKEND, false, &mut wlr_owned);
        set_name(&owner, GTK_BACKEND, false, &mut gtk_owned);
        owner
            .release_name(FRONTEND)
            .expect("release isolated portal frontend name");

        let invocations = fs::read_to_string(log_file).expect("Sway aggregate invocation log");
        let expected_invocations = [
            "--user show xdg-desktop-portal.service -p ActiveState -p SubState -p UnitFileState --value",
            "--user show xdg-desktop-portal-wlr.service -p ActiveState -p SubState -p UnitFileState --value",
            "--user show xdg-desktop-portal-gtk.service -p ActiveState -p SubState -p UnitFileState --value",
        ];
        assert_eq!(invocations.lines().count(), 12);
        for expected in expected_invocations {
            assert_eq!(
                invocations.lines().filter(|line| *line == expected).count(),
                4,
                "unexpected systemd invocation distribution"
            );
        }
    }

    /// Run only from `scripts/validate-sway-activation-ci.sh`: the activation
    /// collector and the runtime collectors must all use the isolated harness.
    #[test]
    #[cfg(unix)]
    #[ignore = "requires the explicit isolated Sway activation gate"]
    #[allow(clippy::too_many_lines)]
    fn isolated_sway_activation_environment_feed_the_passive_rule_pipeline() {
        use std::fs;
        use std::path::Path;
        use std::thread;
        use std::time::{Duration, Instant};

        use crate::model::status::CollectorState;

        const FRONTEND: &str = PORTAL_FRONTEND_NAME;
        const WLR_BACKEND: &str = "org.freedesktop.impl.portal.desktop.wlr";
        const GTK_BACKEND: &str = "org.freedesktop.impl.portal.desktop.gtk";
        const WLR_UNIT: &str = "xdg-desktop-portal-wlr.service";
        const GTK_UNIT: &str = "xdg-desktop-portal-gtk.service";

        fn assert_pid_gone(path: &Path) {
            let pid: i32 = fs::read_to_string(path)
                .expect("timeout fake must record its PID")
                .trim()
                .parse()
                .expect("timeout fake PID");
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                // SAFETY: kill(pid, 0) only probes the fake process created by
                // this isolated test wrapper.
                if unsafe { libc::kill(pid, 0) } != 0 {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "timed-out activation systemctl child was not reaped"
                );
                thread::sleep(Duration::from_millis(25));
            }
        }

        let mode_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_MODE_FILE")
            .expect("explicit Sway activation wrapper must provide a mode file");
        let log_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_LOG")
            .expect("explicit Sway activation wrapper must provide an invocation log");
        let pid_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_PID_FILE")
            .expect("explicit Sway activation wrapper must provide a PID file");
        let path = std::env::var_os("PATH").expect("Sway activation wrapper must provide PATH");
        let fake_bin = std::env::split_paths(&path)
            .next()
            .expect("Sway activation wrapper must prepend fake PATH");
        assert!(fake_bin.join("systemctl").is_file());
        assert_eq!(
            std::env::var("PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD").as_deref(),
            Ok("isolated-sway-activation")
        );

        let base = sway_passive_snapshot(true);
        assert_sway_mixed_routes(&base);
        assert_sway_runtime_bindings(&base);
        let selected_backend_names =
            selected_backend_dbus_names(&base.portal_routes, &base.portal_backends);
        assert_eq!(
            selected_backend_names,
            vec![GTK_BACKEND.to_owned(), WLR_BACKEND.to_owned()]
        );
        let selected_units = vec![
            ServiceInfo::frontend_unit().to_owned(),
            ServiceInfo::backend_unit("wlr"),
            ServiceInfo::backend_unit("gtk"),
        ];
        assert_eq!(
            selected_units,
            [
                ServiceInfo::frontend_unit().to_owned(),
                WLR_UNIT.to_owned(),
                GTK_UNIT.to_owned(),
            ]
        );

        let process = sway_fixture_vars();
        let environment_for = |activation: &Section<BTreeMap<String, String>>| {
            let mut environment =
                Section::available(crate::collectors::environment::environment_info(
                    process.clone(),
                    None,
                    activation.value.as_ref(),
                ));
            if activation.status != CollectorState::Available {
                let reason = super::activation_note_reason(activation);
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
            environment
        };
        let set_mode = |mode: &str| {
            fs::write(&mode_file, format!("{mode}\n")).expect("write Sway activation fake mode");
        };
        let collect_runtime = || {
            let dbus = crate::collectors::dbus::collect(&selected_backend_names);
            let services = crate::collectors::systemd_user::collect(&selected_units);
            (dbus, services)
        };

        let owner = zbus::blocking::Connection::session().expect("isolated session bus");
        owner
            .request_name(FRONTEND)
            .expect("acquire isolated portal frontend name");
        owner
            .request_name(WLR_BACKEND)
            .expect("acquire isolated WLR backend name");
        owner
            .request_name(GTK_BACKEND)
            .expect("acquire isolated GTK backend name");

        let run_scenario =
            |mode: &str,
             expected_activation_state: CollectorState,
             expected_key: Option<&str>,
             expected_findings: &[&str]| {
                set_mode(mode);
                let activation = crate::collectors::activation_environment::collect();
                assert_eq!(activation.status, expected_activation_state);
                if mode == "timeout" {
                    assert_pid_gone(Path::new(&pid_file));
                }
                let (dbus, services) = collect_runtime();
                let dbus_info = dbus.value.as_ref().expect("Sway D-Bus result");
                assert!(dbus_info.connected);
                assert_eq!(
                    dbus_info
                        .checks
                        .iter()
                        .map(|check| check.name.as_str())
                        .collect::<Vec<_>>(),
                    vec![FRONTEND, GTK_BACKEND, WLR_BACKEND]
                );
                let service_info = services.value.as_ref().expect("Sway service result");
                assert_eq!(
                    service_info
                        .unit(ServiceInfo::frontend_unit())
                        .expect("portal frontend unit")
                        .state,
                    UnitState::Active
                );
                assert_eq!(
                    service_info.unit(WLR_UNIT).expect("WLR unit").state,
                    UnitState::Active
                );
                assert_eq!(
                    service_info.unit(GTK_UNIT).expect("GTK unit").state,
                    UnitState::Active
                );

                let mut snapshot = base.clone();
                snapshot.environment = environment_for(&activation);
                snapshot.dbus = dbus;
                snapshot.services = services;
                assert_sway_mixed_routes(&snapshot);
                let environment = snapshot
                    .environment
                    .value
                    .as_ref()
                    .expect("environment section");
                assert_eq!(
                    environment.activation_comparison.performed,
                    expected_activation_state == CollectorState::Available
                );
                if expected_activation_state != CollectorState::Available {
                    assert_eq!(snapshot.environment.status, CollectorState::Available);
                    assert!(
                        snapshot.environment.errors.iter().any(|note| note
                            .message
                            .contains("systemd user activation environment"))
                    );
                }

                let findings = evaluate(&snapshot);
                crate::rules::contract::assert_contract(&findings);
                assert_eq!(finding_ids(&findings), expected_findings);
                if let Some(key) = expected_key {
                    assert!(findings[0].summary.contains(key));
                }
                assert_eq!(
                    RunOutcome::from_report(&Report::new(snapshot, findings, "0.2.1")),
                    RunOutcome::Clean
                );
            };

        run_scenario("healthy", CollectorState::Available, None, &[]);
        run_scenario(
            "stale-desktop",
            CollectorState::Available,
            Some("XDG_CURRENT_DESKTOP"),
            &["ENV004"],
        );
        run_scenario(
            "stale-wayland",
            CollectorState::Available,
            Some("WAYLAND_DISPLAY"),
            &["ENV004"],
        );
        run_scenario(
            "missing-wayland",
            CollectorState::Available,
            Some("WAYLAND_DISPLAY"),
            &["ENV004"],
        );
        run_scenario("unavailable", CollectorState::Unavailable, None, &[]);
        run_scenario("timeout", CollectorState::TimedOut, None, &[]);

        owner
            .release_name(GTK_BACKEND)
            .expect("release isolated GTK backend name");
        owner
            .release_name(WLR_BACKEND)
            .expect("release isolated WLR backend name");
        owner
            .release_name(FRONTEND)
            .expect("release isolated portal frontend name");

        let invocations = fs::read_to_string(log_file).expect("Sway activation invocation log");
        let expected_unit_invocations = [
            "--user show xdg-desktop-portal.service -p ActiveState -p SubState -p UnitFileState --value",
            "--user show xdg-desktop-portal-wlr.service -p ActiveState -p SubState -p UnitFileState --value",
            "--user show xdg-desktop-portal-gtk.service -p ActiveState -p SubState -p UnitFileState --value",
        ];
        assert_eq!(invocations.lines().count(), 24);
        assert_eq!(
            invocations
                .lines()
                .filter(|line| *line == "--user show-environment")
                .count(),
            6
        );
        for expected in expected_unit_invocations {
            assert_eq!(
                invocations.lines().filter(|line| *line == expected).count(),
                6,
                "unexpected systemd invocation distribution"
            );
        }
    }

    /// Run only from `scripts/validate-hyprland-runtime-activation-ci.sh`:
    /// production runtime and activation collectors must use the isolated
    /// Hyprland fixture and fake systemctl.
    #[test]
    #[cfg(unix)]
    #[ignore = "requires the explicit isolated Hyprland runtime/activation gate"]
    #[allow(clippy::too_many_lines)]
    fn isolated_hyprland_runtime_and_activation_collectors_feed_passive_rule_pipeline() {
        use std::fs;
        use std::path::Path;
        use std::thread;
        use std::time::{Duration, Instant};

        use crate::model::status::CollectorState;

        const FRONTEND: &str = PORTAL_FRONTEND_NAME;
        const HYPRLAND_BACKEND: &str = "org.freedesktop.impl.portal.desktop.hyprland";
        const GTK_BACKEND: &str = "org.freedesktop.impl.portal.desktop.gtk";
        const HYPRLAND_UNIT: &str = "xdg-desktop-portal-hyprland.service";
        const GTK_UNIT: &str = "xdg-desktop-portal-gtk.service";

        fn set_name(owner: &zbus::blocking::Connection, name: &str, wanted: bool, held: &mut bool) {
            if wanted && !*held {
                owner
                    .request_name(name)
                    .expect("acquire isolated Hyprland portal name");
                *held = true;
            } else if !wanted && *held {
                owner
                    .release_name(name)
                    .expect("release isolated Hyprland portal name");
                *held = false;
            }
        }

        fn assert_pid_gone(path: &Path) {
            let pid: i32 = fs::read_to_string(path)
                .expect("timeout fake must record its PID")
                .trim()
                .parse()
                .expect("timeout fake PID");
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                // SAFETY: kill(pid, 0) only probes the fake process created by
                // this isolated test wrapper.
                if unsafe { libc::kill(pid, 0) } != 0 {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "timed-out activation systemctl child was not reaped"
                );
                thread::sleep(Duration::from_millis(25));
            }
        }

        let mode_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_MODE_FILE")
            .expect("explicit Hyprland wrapper must provide a mode file");
        let log_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_LOG")
            .expect("explicit Hyprland wrapper must provide an invocation log");
        let pid_file = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_PID_FILE")
            .expect("explicit Hyprland wrapper must provide a PID file");
        let fake_dir = std::env::var_os("PORTALDOCTOR_SYSTEMCTL_FAKE_DIR")
            .expect("explicit Hyprland wrapper must provide a fake directory");
        let path = std::env::var_os("PATH").expect("Hyprland wrapper must provide PATH");
        assert_eq!(
            std::env::split_paths(&path).next(),
            Some(Path::new(&fake_dir).to_path_buf())
        );
        assert_eq!(
            std::env::var("PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD").as_deref(),
            Ok("isolated-hyprland-runtime-activation")
        );

        let base = hyprland_passive_snapshot(true);
        assert_hyprland_mixed_routes(&base);
        assert_hyprland_runtime_bindings(&base);
        let selected_backend_names =
            selected_backend_dbus_names(&base.portal_routes, &base.portal_backends);
        assert_eq!(
            selected_backend_names,
            vec![GTK_BACKEND.to_owned(), HYPRLAND_BACKEND.to_owned()]
        );
        let selected_units = vec![
            ServiceInfo::frontend_unit().to_owned(),
            ServiceInfo::backend_unit("hyprland"),
            ServiceInfo::backend_unit("gtk"),
        ];
        assert_eq!(
            selected_units,
            [
                ServiceInfo::frontend_unit().to_owned(),
                HYPRLAND_UNIT.to_owned(),
                GTK_UNIT.to_owned(),
            ]
        );

        let process = hyprland_fixture_vars();
        let environment_for = |activation: &Section<BTreeMap<String, String>>| {
            let mut environment =
                Section::available(crate::collectors::environment::environment_info(
                    process.clone(),
                    None,
                    activation.value.as_ref(),
                ));
            if activation.status != CollectorState::Available {
                let reason = super::activation_note_reason(activation);
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
            environment
        };
        let set_mode = |mode: &str| {
            fs::write(&mode_file, format!("{mode}\n"))
                .expect("write Hyprland runtime/activation fake mode");
        };
        let collect_runtime = || {
            let dbus = crate::collectors::dbus::collect(&selected_backend_names);
            let services = crate::collectors::systemd_user::collect(&selected_units);
            (dbus, services)
        };

        let owner = zbus::blocking::Connection::session().expect("isolated session bus");
        owner
            .request_name(FRONTEND)
            .expect("acquire isolated portal frontend name");
        let mut hyprland_owned = false;
        let mut gtk_owned = false;

        let mut run_scenario =
            |mode: &str,
             want_hyprland: bool,
             want_gtk: bool,
             expected_hyprland: DbusOutcome,
             expected_gtk: DbusOutcome,
             expected_hyprland_state: UnitState,
             expected_gtk_state: UnitState,
             expected_activation_state: CollectorState,
             expected_key: Option<&str>,
             expected_findings: &[&str]| {
                set_name(&owner, HYPRLAND_BACKEND, want_hyprland, &mut hyprland_owned);
                set_name(&owner, GTK_BACKEND, want_gtk, &mut gtk_owned);
                set_mode(mode);

                let activation = crate::collectors::activation_environment::collect();
                assert_eq!(activation.status, expected_activation_state);
                if mode == "timeout" {
                    assert_pid_gone(Path::new(&pid_file));
                }
                let (dbus, services) = collect_runtime();
                let dbus_info = dbus.value.as_ref().expect("Hyprland D-Bus result");
                assert!(dbus_info.connected);
                assert_eq!(
                    dbus_info
                        .checks
                        .iter()
                        .map(|check| check.name.as_str())
                        .collect::<Vec<_>>(),
                    vec![FRONTEND, GTK_BACKEND, HYPRLAND_BACKEND]
                );
                assert_eq!(
                    dbus_info
                        .checks
                        .iter()
                        .find(|check| check.name == HYPRLAND_BACKEND)
                        .expect("Hyprland D-Bus check")
                        .outcome,
                    expected_hyprland
                );
                assert_eq!(
                    dbus_info
                        .checks
                        .iter()
                        .find(|check| check.name == GTK_BACKEND)
                        .expect("GTK D-Bus check")
                        .outcome,
                    expected_gtk
                );
                let service_info = services.value.as_ref().expect("Hyprland services");
                assert_eq!(
                    service_info
                        .unit(ServiceInfo::frontend_unit())
                        .expect("portal frontend unit")
                        .state,
                    UnitState::Active
                );
                assert_eq!(
                    service_info
                        .unit(HYPRLAND_UNIT)
                        .expect("Hyprland unit")
                        .state,
                    expected_hyprland_state
                );
                assert_eq!(
                    service_info.unit(GTK_UNIT).expect("GTK unit").state,
                    expected_gtk_state
                );

                let mut snapshot = base.clone();
                snapshot.environment = environment_for(&activation);
                snapshot.dbus = dbus;
                snapshot.services = services;
                assert_hyprland_mixed_routes(&snapshot);
                let environment = snapshot
                    .environment
                    .value
                    .as_ref()
                    .expect("environment section");
                assert_eq!(
                    environment.activation_comparison.performed,
                    expected_activation_state == CollectorState::Available
                );
                if expected_activation_state != CollectorState::Available {
                    assert_eq!(snapshot.environment.status, CollectorState::Available);
                    assert!(
                        snapshot.environment.errors.iter().any(|note| note
                            .message
                            .contains("systemd user activation environment"))
                    );
                }

                let findings = evaluate(&snapshot);
                crate::rules::contract::assert_contract(&findings);
                assert_eq!(finding_ids(&findings), expected_findings);
                if let Some(key) = expected_key {
                    assert!(findings.iter().any(|finding| finding.summary.contains(key)));
                }
                assert_eq!(
                    RunOutcome::from_report(&Report::new(snapshot, findings, "0.2.1")),
                    RunOutcome::Clean
                );
            };

        run_scenario(
            "healthy",
            true,
            true,
            DbusOutcome::HasOwner,
            DbusOutcome::HasOwner,
            UnitState::Active,
            UnitState::Active,
            CollectorState::Available,
            None,
            &[],
        );
        run_scenario(
            "missing-hyprland",
            false,
            true,
            DbusOutcome::NoOwner,
            DbusOutcome::HasOwner,
            UnitState::NotFound,
            UnitState::Active,
            CollectorState::Available,
            Some(HYPRLAND_BACKEND),
            &["DBUS002"],
        );
        run_scenario(
            "missing-gtk",
            true,
            false,
            DbusOutcome::HasOwner,
            DbusOutcome::NoOwner,
            UnitState::Active,
            UnitState::NotFound,
            CollectorState::Available,
            Some(GTK_BACKEND),
            &["DBUS002"],
        );
        run_scenario(
            "failed-hyprland",
            false,
            true,
            DbusOutcome::NoOwner,
            DbusOutcome::HasOwner,
            UnitState::Failed,
            UnitState::Active,
            CollectorState::Available,
            Some(HYPRLAND_BACKEND),
            &["DBUS002"],
        );
        run_scenario(
            "stale-desktop",
            true,
            true,
            DbusOutcome::HasOwner,
            DbusOutcome::HasOwner,
            UnitState::Active,
            UnitState::Active,
            CollectorState::Available,
            Some("XDG_CURRENT_DESKTOP"),
            &["ENV004"],
        );
        run_scenario(
            "stale-wayland",
            true,
            true,
            DbusOutcome::HasOwner,
            DbusOutcome::HasOwner,
            UnitState::Active,
            UnitState::Active,
            CollectorState::Available,
            Some("WAYLAND_DISPLAY"),
            &["ENV004"],
        );
        run_scenario(
            "missing-wayland",
            true,
            true,
            DbusOutcome::HasOwner,
            DbusOutcome::HasOwner,
            UnitState::Active,
            UnitState::Active,
            CollectorState::Available,
            Some("WAYLAND_DISPLAY"),
            &["ENV004"],
        );
        run_scenario(
            "unavailable",
            true,
            true,
            DbusOutcome::HasOwner,
            DbusOutcome::HasOwner,
            UnitState::Active,
            UnitState::Active,
            CollectorState::Unavailable,
            None,
            &[],
        );
        run_scenario(
            "timeout",
            true,
            true,
            DbusOutcome::HasOwner,
            DbusOutcome::HasOwner,
            UnitState::Active,
            UnitState::Active,
            CollectorState::TimedOut,
            None,
            &[],
        );

        set_name(&owner, GTK_BACKEND, false, &mut gtk_owned);
        set_name(&owner, HYPRLAND_BACKEND, false, &mut hyprland_owned);
        owner
            .release_name(FRONTEND)
            .expect("release isolated portal frontend name");

        let invocations = fs::read_to_string(log_file).expect("Hyprland invocation log");
        let expected_unit_invocations = [
            "--user show xdg-desktop-portal.service -p ActiveState -p SubState -p UnitFileState --value",
            "--user show xdg-desktop-portal-hyprland.service -p ActiveState -p SubState -p UnitFileState --value",
            "--user show xdg-desktop-portal-gtk.service -p ActiveState -p SubState -p UnitFileState --value",
        ];
        assert_eq!(invocations.lines().count(), 36);
        assert_eq!(
            invocations
                .lines()
                .filter(|line| *line == "--user show-environment")
                .count(),
            9
        );
        for expected in expected_unit_invocations {
            assert_eq!(
                invocations.lines().filter(|line| *line == expected).count(),
                9,
                "unexpected Hyprland systemd invocation distribution"
            );
        }
    }
}
