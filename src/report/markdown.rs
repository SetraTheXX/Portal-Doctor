use std::fmt::Write as _;

use crate::model::dbus::{DbusOutcome, PORTAL_FRONTEND_NAME};
use crate::model::environment::EnvironmentRelation;
use crate::model::section::Section;
use crate::model::snapshot::Snapshot;
use crate::report::ShareableReport;

/// Renderer for a stable, issue-friendly Markdown report.
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Render a shareable report without exposing data outside the redacted
    /// document supplied by the caller.
    #[must_use]
    pub fn render(report: &ShareableReport, verbose: bool) -> String {
        let mut out = String::new();
        writeln!(out, "# PortalDoctor diagnostic report").expect("String writes cannot fail");
        writeln!(
            out,
            "> Report v{} · Snapshot schema v{} · PortalDoctor {}",
            report.report_version,
            report.schema_version,
            inline(&report.portaldoctor_version)
        )
        .expect("String writes cannot fail");
        out.push('\n');

        write_summary(&mut out, report);
        write_system_and_session(&mut out, &report.snapshot);
        write_environment(&mut out, &report.snapshot, verbose);
        write_portal(&mut out, &report.snapshot, verbose);
        write_runtime(&mut out, &report.snapshot, verbose);
        write_media(&mut out, &report.snapshot, verbose);
        write_journal(&mut out, &report.snapshot);
        write_findings(&mut out, report);
        write_collection_notes(&mut out, &report.snapshot);
        write_privacy(&mut out, report);
        out
    }
}

fn write_summary(out: &mut String, report: &ShareableReport) {
    writeln!(out, "## Summary").expect("String writes cannot fail");
    writeln!(out, "| Field | Value |").expect("String writes cannot fail");
    writeln!(out, "| --- | --- |").expect("String writes cannot fail");
    row(out, "Findings", &report.findings.len().to_string());
    row(
        out,
        "Collected at (Unix ms)",
        &report.snapshot.collected_at.to_string(),
    );
    row(out, "Privacy mode", "redacted");
    row(
        out,
        "HOME paths",
        if report.privacy.home_normalized {
            "normalized to `$HOME`"
        } else {
            "not normalized (HOME was unavailable)"
        },
    );
    row(
        out,
        "Hostname",
        if report.privacy.hostname_suppressed {
            "suppressed"
        } else {
            "retained; use `--suppress-hostname` before sharing publicly"
        },
    );
    row(
        out,
        "Raw journal / PipeWire",
        "excluded; normalized evidence only",
    );
    out.push('\n');
}

fn write_system_and_session(out: &mut String, snapshot: &Snapshot) {
    writeln!(out, "## System and session").expect("String writes cannot fail");
    writeln!(out, "### System").expect("String writes cannot fail");
    row(out, "Collection", &section_status(&snapshot.system));
    if let Some(system) = &snapshot.system.value {
        writeln!(out, "| Field | Value |").expect("String writes cannot fail");
        writeln!(out, "| --- | --- |").expect("String writes cannot fail");
        row(out, "ID", &option_value(system.id.as_ref()));
        row(out, "Name", &option_value(system.name.as_ref()));
        row(
            out,
            "Pretty name",
            &option_value(system.pretty_name.as_ref()),
        );
        row(out, "Version", &option_value(system.version_id.as_ref()));
    }
    out.push('\n');

    writeln!(out, "### Session").expect("String writes cannot fail");
    row(out, "Collection", &section_status(&snapshot.session));
    if let Some(session) = &snapshot.session.value {
        writeln!(out, "| Field | Value |").expect("String writes cannot fail");
        writeln!(out, "| --- | --- |").expect("String writes cannot fail");
        row(
            out,
            "Current desktop",
            &option_value(session.current_desktop.as_ref()),
        );
        row(
            out,
            "Session desktop",
            &option_value(session.session_desktop.as_ref()),
        );
        row(
            out,
            "Session type",
            &session.session_type.map_or_else(
                || option_value(session.session_type_raw.as_ref()),
                |value| value.as_str().to_owned(),
            ),
        );
        row(
            out,
            "Wayland display",
            &option_value(session.wayland_display.as_ref()),
        );
        row(out, "X11 display", &option_value(session.display.as_ref()));
    }
    out.push('\n');
}

fn write_environment(out: &mut String, snapshot: &Snapshot, verbose: bool) {
    writeln!(out, "## Environment").expect("String writes cannot fail");
    row(out, "Collection", &section_status(&snapshot.environment));
    let Some(environment) = &snapshot.environment.value else {
        out.push('\n');
        return;
    };

    let comparison = &environment.activation_comparison;
    let mismatch_count = comparison
        .entries
        .iter()
        .filter(|entry| entry.relation != EnvironmentRelation::Equal)
        .count();
    let comparison_summary = if comparison.performed {
        format!(
            "{} variable(s), {} mismatch(es)",
            comparison.entries.len(),
            mismatch_count
        )
    } else {
        "not performed".to_owned()
    };
    row(out, "Activation comparison", &comparison_summary);
    writeln!(out, "### Allowlisted process environment").expect("String writes cannot fail");
    writeln!(out, "| Variable | Value |").expect("String writes cannot fail");
    writeln!(out, "| --- | --- |").expect("String writes cannot fail");
    if environment.process.is_empty() {
        row(out, "—", "no allowlisted variables observed");
    } else {
        for (key, value) in &environment.process {
            row(out, key, value);
        }
    }

    if verbose {
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
        if !comparison.entries.is_empty() {
            writeln!(out, "### Activation comparison details").expect("String writes cannot fail");
            writeln!(out, "| Variable | Relation | Process | Activation |")
                .expect("String writes cannot fail");
            writeln!(out, "| --- | --- | --- | --- |").expect("String writes cannot fail");
            for entry in &comparison.entries {
                writeln!(
                    out,
                    "| {} | {} | {} | {} |",
                    escape(&entry.key),
                    escape(entry.relation.as_str()),
                    escape(&option_value(entry.process_value.as_ref())),
                    escape(&option_value(entry.activation_value.as_ref()))
                )
                .expect("String writes cannot fail");
            }
        }
    }
    out.push('\n');
}

fn write_roots(out: &mut String, title: &str, roots: &[String]) {
    writeln!(out, "### {title}").expect("String writes cannot fail");
    if roots.is_empty() {
        writeln!(out, "_none_").expect("String writes cannot fail");
        return;
    }
    for root in roots {
        writeln!(out, "- {}", inline(root)).expect("String writes cannot fail");
    }
}

fn write_portal(out: &mut String, snapshot: &Snapshot, verbose: bool) {
    writeln!(out, "## Portal routing").expect("String writes cannot fail");
    row(
        out,
        "Configuration",
        &section_status(&snapshot.portal_config),
    );
    if let Some(config) = &snapshot.portal_config.value {
        row(
            out,
            "Selected configuration",
            &option_value(config.selected_file.as_ref()),
        );
        row(out, "Preferences", &config.preferences.len().to_string());
        if verbose && !config.preferences.is_empty() {
            writeln!(out, "### Preferences").expect("String writes cannot fail");
            writeln!(out, "| Interface | Backends | Source |").expect("String writes cannot fail");
            writeln!(out, "| --- | --- | --- |").expect("String writes cannot fail");
            for preference in &config.preferences {
                writeln!(
                    out,
                    "| {} | {} | {} |",
                    escape(&preference.interface),
                    escape(&preference.backends.join(", ")),
                    escape(&preference.source_file)
                )
                .expect("String writes cannot fail");
            }
        }
    }

    row(out, "Backends", &section_status(&snapshot.portal_backends));
    if let Some(backends) = &snapshot.portal_backends.value {
        row(out, "Discovered backend count", &backends.len().to_string());
        if verbose && !backends.is_empty() {
            writeln!(out, "### Backend descriptors").expect("String writes cannot fail");
            writeln!(out, "| ID | D-Bus name | Interfaces |").expect("String writes cannot fail");
            writeln!(out, "| --- | --- | --- |").expect("String writes cannot fail");
            for backend in backends {
                writeln!(
                    out,
                    "| {} | {} | {} |",
                    escape(&backend.id),
                    escape(&backend.dbus_name),
                    escape(
                        &backend
                            .interfaces
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                )
                .expect("String writes cannot fail");
            }
        }
    }

    row(out, "Routes", &section_status(&snapshot.portal_routes));
    if let Some(routes) = &snapshot.portal_routes.value {
        writeln!(
            out,
            "| Interface | Status | Requested | Available | Selected |"
        )
        .expect("String writes cannot fail");
        writeln!(out, "| --- | --- | --- | --- | --- |").expect("String writes cannot fail");
        if routes.is_empty() {
            row(out, "—", "no route facts collected");
        } else {
            for route in routes {
                writeln!(
                    out,
                    "| {} | {} | {} | {} | {} |",
                    escape(&route.interface),
                    escape(route.status.as_str()),
                    escape(&join_or_any(&route.requested_candidates)),
                    escape(&join_or_none(&route.available_candidates)),
                    escape(&join_or_none(&route.selected_candidates))
                )
                .expect("String writes cannot fail");
            }
        }
    }
    out.push('\n');
}

fn write_runtime(out: &mut String, snapshot: &Snapshot, verbose: bool) {
    writeln!(out, "## Runtime").expect("String writes cannot fail");
    row(out, "D-Bus collection", &section_status(&snapshot.dbus));
    if let Some(dbus) = &snapshot.dbus.value {
        row(
            out,
            "Session bus",
            if dbus.connected {
                "connected"
            } else {
                "unavailable"
            },
        );
        writeln!(out, "| Name | Outcome |").expect("String writes cannot fail");
        writeln!(out, "| --- | --- |").expect("String writes cannot fail");
        for check in &dbus.checks {
            let outcome = dbus_outcome_label(&check.outcome);
            row(out, &check.name, &outcome);
        }
        if dbus
            .checks
            .iter()
            .all(|check| check.name != PORTAL_FRONTEND_NAME)
        {
            row(out, PORTAL_FRONTEND_NAME, "not checked");
        }
    }
    row(
        out,
        "Services collection",
        &section_status(&snapshot.services),
    );
    if verbose && let Some(services) = &snapshot.services.value {
        writeln!(out, "### Systemd user units").expect("String writes cannot fail");
        writeln!(out, "| Unit | State | Sub-state | Unit-file state |")
            .expect("String writes cannot fail");
        writeln!(out, "| --- | --- | --- | --- |").expect("String writes cannot fail");
        for unit in &services.units {
            writeln!(
                out,
                "| {} | {} | {} | {} |",
                escape(&unit.unit),
                escape(unit.state.as_str()),
                escape(&option_value(unit.sub_state.as_ref())),
                escape(&option_value(unit.unit_file_state.as_ref()))
            )
            .expect("String writes cannot fail");
        }
    }
    out.push('\n');
}

fn write_media(out: &mut String, snapshot: &Snapshot, verbose: bool) {
    writeln!(out, "## Media path").expect("String writes cannot fail");
    row(
        out,
        "PipeWire collection",
        &section_status(&snapshot.pipewire),
    );
    if let Some(pipewire) = &snapshot.pipewire.value {
        row(
            out,
            "PipeWire version",
            &option_value(pipewire.version.as_ref()),
        );
        row(out, "Objects", &pipewire.object_count.to_string());
        row(out, "Nodes", &pipewire.node_count.to_string());
        row(out, "Links", &pipewire.link_count.to_string());
        row(
            out,
            "Portal clients",
            &pipewire.portal_client_count.to_string(),
        );
        row(
            out,
            "ScreenCast sources",
            &pipewire.screen_cast_source_count.to_string(),
        );
        if verbose {
            row(
                out,
                "Normalized video nodes",
                &pipewire.nodes.len().to_string(),
            );
            row(out, "Normalized links", &pipewire.links.len().to_string());
        }
    }
    row(
        out,
        "WirePlumber collection",
        &section_status(&snapshot.wireplumber),
    );
    if let Some(wireplumber) = &snapshot.wireplumber.value {
        row(
            out,
            "WirePlumber PipeWire version",
            &option_value(wireplumber.pipewire_version.as_ref()),
        );
        row(
            out,
            "WirePlumber clients",
            &wireplumber.wireplumber_client_count.to_string(),
        );
    }
    out.push('\n');
}

fn write_journal(out: &mut String, snapshot: &Snapshot) {
    writeln!(out, "## Journal evidence").expect("String writes cannot fail");
    row(out, "Collection", &section_status(&snapshot.journal));
    let Some(journal) = &snapshot.journal.value else {
        out.push('\n');
        return;
    };
    row(
        out,
        "Window",
        &format!("{} minutes", journal.window_minutes),
    );
    row(out, "Match state", journal.match_state.as_str());
    row(
        out,
        "Scanned records",
        &journal.scanned_entry_count.to_string(),
    );
    row(
        out,
        "Sanitized excerpts",
        &journal.entries.len().to_string(),
    );
    if !journal.entries.is_empty() {
        writeln!(
            out,
            "| Unit | Priority | Classification | Sanitized message |"
        )
        .expect("String writes cannot fail");
        writeln!(out, "| --- | ---: | --- | --- |").expect("String writes cannot fail");
        for entry in &journal.entries {
            writeln!(
                out,
                "| {} | {} | {} | {} |",
                escape(&entry.unit),
                entry.priority,
                escape(entry.classification.as_str()),
                escape(&entry.message)
            )
            .expect("String writes cannot fail");
        }
    }
    out.push('\n');
}

fn write_findings(out: &mut String, report: &ShareableReport) {
    writeln!(out, "## Findings").expect("String writes cannot fail");
    if report.findings.is_empty() {
        writeln!(out, "No findings were produced by the rule engine.")
            .expect("String writes cannot fail");
        out.push('\n');
        return;
    }
    for finding in &report.findings {
        writeln!(
            out,
            "### {} · {} ({})",
            escape(finding.severity.as_str()),
            escape(&finding.title),
            escape(&finding.id)
        )
        .expect("String writes cannot fail");
        row(out, "Confidence", finding.confidence.as_str());
        row(out, "Summary", &finding.summary);
        row(out, "Source", &finding.source_component);
        if let Some(impact) = &finding.impact {
            row(out, "Impact", impact);
        }
        if !finding.evidence.is_empty() {
            row(
                out,
                "Evidence",
                &finding
                    .evidence
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        writeln!(out, "**Explanation**  ").expect("String writes cannot fail");
        writeln!(out, "{}", escape(&finding.explanation)).expect("String writes cannot fail");
        if !finding.recommendation.is_empty() {
            writeln!(out, "**Recommended next steps**").expect("String writes cannot fail");
            for step in &finding.recommendation {
                writeln!(out, "- {}", escape(step)).expect("String writes cannot fail");
            }
        }
        out.push('\n');
    }
}

fn write_collection_notes(out: &mut String, snapshot: &Snapshot) {
    let sections = [
        ("system", &snapshot.system.errors),
        ("session", &snapshot.session.errors),
        ("environment", &snapshot.environment.errors),
        ("portal_config", &snapshot.portal_config.errors),
        ("portal_backends", &snapshot.portal_backends.errors),
        ("portal_routes", &snapshot.portal_routes.errors),
        ("dbus", &snapshot.dbus.errors),
        ("services", &snapshot.services.errors),
        ("pipewire", &snapshot.pipewire.errors),
        ("wireplumber", &snapshot.wireplumber.errors),
        ("journal", &snapshot.journal.errors),
    ];
    let notes: Vec<_> = sections
        .iter()
        .flat_map(|(name, errors)| errors.iter().map(move |error| (*name, &error.message)))
        .collect();
    if notes.is_empty() {
        return;
    }
    writeln!(out, "## Collection notes").expect("String writes cannot fail");
    for (section, message) in notes {
        writeln!(out, "- `{section}`: {}", escape(message)).expect("String writes cannot fail");
    }
    out.push('\n');
}

fn write_privacy(out: &mut String, report: &ShareableReport) {
    writeln!(out, "## Sharing checklist").expect("String writes cannot fail");
    writeln!(
        out,
        "- Environment keys are restricted to the existing allowlist."
    )
    .expect("String writes cannot fail");
    writeln!(
        out,
        "- Home-directory paths: {}.",
        if report.privacy.home_normalized {
            "normalized to `$HOME`"
        } else {
            "not normalized because HOME was unavailable"
        }
    )
    .expect("String writes cannot fail");
    writeln!(
        out,
        "- Hostname suppression: {}.",
        if report.privacy.hostname_suppressed {
            "enabled"
        } else {
            "not enabled; review host-related values before public sharing"
        }
    )
    .expect("String writes cannot fail");
    writeln!(out, "- Raw journal and raw PipeWire dumps are excluded; only bounded normalized evidence is present.")
        .expect("String writes cannot fail");
    writeln!(
        out,
        "- Review the report once before attaching it to a public issue."
    )
    .expect("String writes cannot fail");
}

fn section_status<T>(section: &Section<T>) -> String {
    if section.errors.is_empty() {
        section.status.to_string()
    } else {
        format!("{}: {}", section.status, first_note(section))
    }
}

fn first_note<T>(section: &Section<T>) -> &str {
    section
        .errors
        .first()
        .map_or("no details", |note| note.message.as_str())
}

fn row(out: &mut String, key: &str, value: &str) {
    writeln!(out, "| {} | {} |", escape(key), escape(value)).expect("String writes cannot fail");
}

fn option_value(value: Option<&String>) -> String {
    value.map_or_else(|| "—".to_owned(), Clone::clone)
}

fn join_or_any(values: &[String]) -> String {
    if values.is_empty() {
        "any".to_owned()
    } else {
        values.join(", ")
    }
}

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(", ")
    }
}

fn dbus_outcome_label(outcome: &DbusOutcome) -> String {
    match outcome {
        DbusOutcome::HasOwner => "has owner".to_owned(),
        DbusOutcome::NoOwner => "no owner".to_owned(),
        DbusOutcome::NoSessionBus => "no session bus".to_owned(),
        DbusOutcome::ActivationFailure => "activation failure".to_owned(),
        DbusOutcome::Timeout => "timeout".to_owned(),
        DbusOutcome::AccessDenied => "access denied".to_owned(),
        DbusOutcome::MalformedResponse => "malformed response".to_owned(),
        DbusOutcome::Other(message) => message.clone(),
    }
}

fn inline(value: &str) -> String {
    format!("`{}`", value.replace('`', "'"))
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
}
