use std::fmt::Write as _;

use crate::model::dbus::{DbusOutcome, PORTAL_FRONTEND_NAME};
use crate::model::environment::EnvironmentRelation;
use crate::model::section::Section;
use crate::report::{Renderer, Report};

/// Renderer that emits concise terminal text (PRD §7.1); `--verbose` adds
/// collected details and full finding explanations.
pub struct TerminalRenderer;

impl Renderer for TerminalRenderer {
    fn render(&self, report: &Report, verbose: bool) -> String {
        let mut out = String::new();
        writeln!(out, "PortalDoctor {}", report.portaldoctor_version)
            .expect("writing to a String cannot fail");
        writeln!(out, "Snapshot schema v{}", report.schema_version)
            .expect("writing to a String cannot fail");
        out.push('\n');
        write_system(&mut out, report);
        write_session(&mut out, report);
        write_environment(&mut out, report, verbose);
        write_runtime(&mut out, report, verbose);
        write_media(&mut out, report);
        out.push('\n');
        write_findings(&mut out, report, verbose);
        out
    }
}

fn write_system(out: &mut String, report: &Report) {
    if let Some(system) = &report.snapshot.system.value {
        let label = system
            .pretty_name
            .as_deref()
            .or(system.name.as_deref())
            .unwrap_or("unknown operating system");
        if let Some(id) = &system.id {
            let labeled = format!("System: {label} ({id})");
            writeln!(out, "{labeled}").expect("writing to a String cannot fail");
        } else {
            let labeled = format!("System: {label}");
            writeln!(out, "{labeled}").expect("writing to a String cannot fail");
        }
    } else {
        let section = &report.snapshot.system;
        let status_line = format!("System: {} {}", section.status, first_note(section));
        writeln!(out, "{status_line}").expect("writing to a String cannot fail");
    }
}

fn write_session(out: &mut String, report: &Report) {
    let Some(session) = &report.snapshot.session.value else {
        writeln!(out, "Session: unavailable").expect("writing to a String cannot fail");
        return;
    };
    let mut parts = Vec::new();
    parts.push(match session.session_type {
        Some(session_type) => format!("{} session", session_type.as_str()),
        None => match &session.session_type_raw {
            Some(raw) => format!("unrecognized session type ({raw})"),
            None => "session type unknown".to_owned(),
        },
    });
    if let Some(desktop) = &session.current_desktop {
        parts.push(format!("desktop {desktop}"));
    }
    if let Some(desktop) = &session.session_desktop
        && session.current_desktop.as_deref() != Some(desktop)
    {
        parts.push(format!("session desktop {desktop}"));
    }
    let line = parts.join(" · ");
    writeln!(out, "Session: {line}").expect("writing to a String cannot fail");
}

fn write_environment(out: &mut String, report: &Report, verbose: bool) {
    let Some(environment) = &report.snapshot.environment.value else {
        return;
    };
    let comparison = &environment.activation_comparison;
    if !comparison.performed {
        writeln!(out, "Activation environment: not compared")
            .expect("writing to a String cannot fail");
        for note in &report.snapshot.environment.errors {
            let note_line = format!("  note: {}", note.message);
            writeln!(out, "{note_line}").expect("writing to a String cannot fail");
        }
        return;
    }
    let mismatch_count = comparison
        .entries
        .iter()
        .filter(|entry| entry.relation != EnvironmentRelation::Equal)
        .count();
    if mismatch_count == 0 {
        let consistent = format!(
            "Activation environment: consistent ({} variables compared)",
            comparison.entries.len()
        );
        writeln!(out, "{consistent}").expect("writing to a String cannot fail");
    } else {
        let mismatch_line = format!(
            "Activation environment: {mismatch_count} mismatch(es) across {} variables",
            comparison.entries.len()
        );
        writeln!(out, "{mismatch_line}").expect("writing to a String cannot fail");
    }

    if !verbose {
        return;
    }
    writeln!(out, "\nEnvironment variables (allowlisted):")
        .expect("writing to a String cannot fail");
    for (key, value) in &environment.process {
        let var_line = format!("  {key}={value}");
        writeln!(out, "{var_line}").expect("writing to a String cannot fail");
    }
    write_roots(
        out,
        "Config search roots",
        &environment.search_roots.config_roots,
    );
    write_roots(
        out,
        "Data search roots",
        &environment.search_roots.data_roots,
    );
    writeln!(out, "\nActivation environment details:").expect("writing to a String cannot fail");
    for entry in &comparison.entries {
        let detail = format!(
            "  {}: {}",
            entry.key,
            describe_relation(
                entry.relation,
                entry.process_value.as_ref(),
                entry.activation_value.as_ref(),
            )
        );
        writeln!(out, "{detail}").expect("writing to a String cannot fail");
    }
}

fn write_roots(out: &mut String, label: &str, roots: &[String]) {
    let header = format!("\n{label}:");
    writeln!(out, "{header}").expect("writing to a String cannot fail");
    for root in roots {
        let root_line = format!("  {root}");
        writeln!(out, "{root_line}").expect("writing to a String cannot fail");
    }
}

fn describe_relation(
    relation: EnvironmentRelation,
    process_value: Option<&String>,
    activation_value: Option<&String>,
) -> String {
    match relation {
        EnvironmentRelation::Equal => "equal".to_owned(),
        EnvironmentRelation::Different => {
            format!("different (process {process_value:?}, activation {activation_value:?})")
        }
        EnvironmentRelation::MissingProcess => {
            format!("missing in this session (activation {activation_value:?})")
        }
        EnvironmentRelation::MissingActivation => {
            format!("missing in activation environment (process {process_value:?})")
        }
        EnvironmentRelation::NotChecked => "not checked".to_owned(),
    }
}

fn write_findings(out: &mut String, report: &Report, verbose: bool) {
    if report.findings.is_empty() {
        out.push_str("Findings: none detected.\n");
        return;
    }
    let count_line = format!("Findings: {}", report.findings.len());
    writeln!(out, "{count_line}").expect("writing to a String cannot fail");
    for finding in &report.findings {
        let headline = format!(
            "\n  [{}] {} ({})",
            finding.severity, finding.title, finding.id
        );
        writeln!(out, "{headline}").expect("writing to a String cannot fail");
        let summary_line = format!("  {}", finding.summary);
        writeln!(out, "{summary_line}").expect("writing to a String cannot fail");
        // Default output stays actionable: surface the first recommended step
        // even in the terse view.
        if let Some(next) = finding.recommendation.first() {
            writeln!(out, "    next: {next}").expect("writing to a String cannot fail");
        }
        if !verbose {
            continue;
        }
        let confidence_line = format!("  confidence: {}", finding.confidence);
        writeln!(out, "{confidence_line}").expect("writing to a String cannot fail");
        let explanation_line = format!("  explanation: {}", finding.explanation);
        writeln!(out, "{explanation_line}").expect("writing to a String cannot fail");
        if let Some(impact) = &finding.impact {
            let impact_line = format!("  impact: {impact}");
            writeln!(out, "{impact_line}").expect("writing to a String cannot fail");
        }
        out.push_str("  recommendation:\n");
        for step in &finding.recommendation {
            let step_line = format!("    - {step}");
            writeln!(out, "{step_line}").expect("writing to a String cannot fail");
        }
    }
    if !verbose {
        out.push_str("\nRun with --verbose for details.\n");
    }
}

fn write_runtime(out: &mut String, report: &Report, verbose: bool) {
    let Some(info) = &report.snapshot.dbus.value else {
        return;
    };
    if !info.connected {
        writeln!(out, "D-Bus: session bus unavailable").expect("writing to a String cannot fail");
        return;
    }
    let frontend_outcome = info
        .checks
        .iter()
        .find(|check| check.name == PORTAL_FRONTEND_NAME)
        .map(|check| &check.outcome);
    let frontend_line = match frontend_outcome {
        Some(DbusOutcome::HasOwner) => {
            "D-Bus: connected \u{b7} portal frontend reachable".to_owned()
        }
        _ => "D-Bus: connected \u{b7} portal frontend NOT reachable".to_owned(),
    };
    writeln!(out, "{frontend_line}").expect("writing to a String cannot fail");
    for check in &info.checks {
        if check.name == PORTAL_FRONTEND_NAME {
            continue;
        }
        writeln!(
            out,
            "  backend {}: {}",
            check.name,
            describe_outcome(&check.outcome)
        )
        .expect("writing to a String cannot fail");
    }

    if verbose && let Some(services) = &report.snapshot.services.value {
        writeln!(out, "\nSystemd user units:").expect("writing to a String cannot fail");
        for unit in &services.units {
            writeln!(out, "  {}: {}", unit.unit, unit.state.as_str())
                .expect("writing to a String cannot fail");
            if let Some(sub) = &unit.sub_state {
                writeln!(out, "    sub-state: {sub}").expect("writing to a String cannot fail");
            }
        }
    }
}

fn write_media(out: &mut String, report: &Report) {
    if let Some(info) = &report.snapshot.pipewire.value {
        let version = info.version.as_deref().unwrap_or("unknown version");
        writeln!(
            out,
            "PipeWire: reachable · {version} · {} objects · {} nodes · {} links",
            info.object_count, info.node_count, info.link_count
        )
        .expect("writing to a String cannot fail");
        let video_source_count = info
            .nodes
            .iter()
            .filter(|node| node.is_video_source)
            .count();
        writeln!(
            out,
            "  video sources: {video_source_count} · ScreenCast sources: {} · portal clients: {}",
            info.screen_cast_source_count, info.portal_client_count
        )
        .expect("writing to a String cannot fail");
    } else {
        let section = &report.snapshot.pipewire;
        writeln!(out, "PipeWire: {} {}", section.status, first_note(section))
            .expect("writing to a String cannot fail");
    }

    if let Some(info) = &report.snapshot.wireplumber.value {
        let version = info
            .pipewire_version
            .as_deref()
            .unwrap_or("unknown version");
        writeln!(
            out,
            "WirePlumber: reachable · {version} · {} client(s)",
            info.wireplumber_client_count
        )
        .expect("writing to a String cannot fail");
    } else {
        let section = &report.snapshot.wireplumber;
        writeln!(
            out,
            "WirePlumber: {} {}",
            section.status,
            first_note(section)
        )
        .expect("writing to a String cannot fail");
    }
}

fn describe_outcome(outcome: &DbusOutcome) -> String {
    match outcome {
        DbusOutcome::HasOwner => "reachable".to_owned(),
        DbusOutcome::NoOwner => "no owner on the bus".to_owned(),
        DbusOutcome::NoSessionBus => "no session bus".to_owned(),
        DbusOutcome::ActivationFailure => "activation failure".to_owned(),
        DbusOutcome::Timeout => "timed out".to_owned(),
        DbusOutcome::AccessDenied => "access denied".to_owned(),
        DbusOutcome::MalformedResponse => "malformed response".to_owned(),
        DbusOutcome::Other(msg) => format!("error: {msg}"),
    }
}

fn first_note<T>(section: &Section<T>) -> String {
    section.errors.first().map_or_else(
        || "- no details available".to_owned(),
        |note| format!("- {}", note.message),
    )
}
