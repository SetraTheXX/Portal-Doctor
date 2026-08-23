use std::fmt::Write as _;

use crate::model::portal::PortalRoute;
use crate::report::{Renderer, Report};

/// Renderer for `portal list`: discovered backend descriptors.
pub struct PortalListRenderer;

impl Renderer for PortalListRenderer {
    fn render(&self, report: &Report, _verbose: bool) -> String {
        let mut out = String::new();
        if let Some(backends) = &report.snapshot.portal_backends.value {
            writeln!(out, "Portal backends ({}):", backends.len())
                .expect("writing to a String cannot fail");
            for backend in backends {
                writeln!(out, "  {}", backend.id).expect("writing to a String cannot fail");
                writeln!(out, "    dbus name: {}", backend.dbus_name)
                    .expect("writing to a String cannot fail");
                writeln!(out, "    descriptor: {}", backend.descriptor_path)
                    .expect("writing to a String cannot fail");
                let interfaces = backend.interfaces.iter().cloned().collect::<Vec<_>>();
                writeln!(out, "    interfaces: {}", interfaces.join(", "))
                    .expect("writing to a String cannot fail");
                if !backend.legacy_use_in.is_empty() {
                    writeln!(out, "    use_in: {}", backend.legacy_use_in.join(", "))
                        .expect("writing to a String cannot fail");
                }
                for duplicate in &backend.duplicate_descriptors {
                    writeln!(
                        out,
                        "    duplicate descriptor (lower precedence): {duplicate}"
                    )
                    .expect("writing to a String cannot fail");
                }
            }
        } else {
            let section = &report.snapshot.portal_backends;
            writeln!(
                out,
                "Portal backends: {} {}",
                section.status,
                section_errors(section)
            )
            .expect("writing to a String cannot fail");
        }
        out
    }
}

/// Renderer for `portal routes`: the resolved route table.
pub struct PortalRoutesRenderer;

impl Renderer for PortalRoutesRenderer {
    fn render(&self, report: &Report, _verbose: bool) -> String {
        let mut out = String::new();
        let Some(routes) = &report.snapshot.portal_routes.value else {
            let section = &report.snapshot.portal_routes;
            writeln!(
                out,
                "Portal routes: {} {}",
                section.status,
                section_errors(section)
            )
            .expect("writing to a String cannot fail");
            return out;
        };
        if routes.is_empty() {
            out.push_str("Portal routes: none discovered.\n");
            return out;
        }
        let interface = routes.iter().map(|r| r.interface.len()).max().unwrap_or(0);
        let status = 12usize;
        let requested = routes
            .iter()
            .map(|r| join_or_any(&r.requested_candidates))
            .map(|s| s.len())
            .max()
            .unwrap_or(0);
        let available = routes
            .iter()
            .map(|r| join_or_any(&r.available_candidates))
            .map(|s| s.len())
            .max()
            .unwrap_or(0);
        let header = format!(
            "{:<interface$}  {:<status$}  {:<requested$}  {:<available$}  Selected",
            "Interface", "Status", "Requested", "Available"
        );
        writeln!(out, "{header}").expect("writing to a String cannot fail");
        writeln!(out, "{}", "-".repeat(header.len())).expect("writing to a String cannot fail");
        for route in routes {
            let line = format!(
                "{:<interface$}  {:<status$}  {:<requested$}  {:<available$}  {}",
                route.interface,
                route.status.as_str(),
                join_or_any(&route.requested_candidates),
                join_or_any(&route.available_candidates),
                join_or_any(&route.selected_candidates),
            );
            writeln!(out, "{line}").expect("writing to a String cannot fail");
        }
        out
    }
}

/// Renderer for `portal explain <interface>`: one route with full provenance.
pub struct PortalExplainRenderer {
    pub interface: String,
}

impl Renderer for PortalExplainRenderer {
    fn render(&self, report: &Report, _verbose: bool) -> String {
        let mut out = String::new();
        let Some(routes) = &report.snapshot.portal_routes.value else {
            let section = &report.snapshot.portal_routes;
            writeln!(
                out,
                "Portal routes: {} {}",
                section.status,
                section_errors(section)
            )
            .expect("writing to a String cannot fail");
            return out;
        };
        let Some(route) = find_route(routes, &self.interface) else {
            writeln!(out, "No route for interface {:?}.", self.interface)
                .expect("writing to a String cannot fail");
            return out;
        };
        writeln!(out, "Interface: {}", route.interface).expect("writing to a String cannot fail");
        writeln!(out, "Status: {}", route.status.as_str())
            .expect("writing to a String cannot fail");
        writeln!(
            out,
            "Requested: {}",
            join_or_any(&route.requested_candidates)
        )
        .expect("writing to a String cannot fail");
        writeln!(
            out,
            "Available: {}",
            join_or_any(&route.available_candidates)
        )
        .expect("writing to a String cannot fail");
        writeln!(out, "Selected: {}", join_or_any(&route.selected_candidates))
            .expect("writing to a String cannot fail");
        writeln!(out, "\nEvidence:").expect("writing to a String cannot fail");
        for item in &route.evidence {
            writeln!(out, "  - {}", item.message).expect("writing to a String cannot fail");
        }
        if let Some(config) = &report.snapshot.portal_config.value
            && !config.parse_errors.is_empty()
        {
            writeln!(out, "\nConfig parse errors:").expect("writing to a String cannot fail");
            for error in &config.parse_errors {
                writeln!(out, "  - {error}").expect("writing to a String cannot fail");
            }
        }
        out
    }
}

/// Match by full interface name or by case-sensitive suffix (`ScreenCast`).
fn find_route<'a>(routes: &'a [PortalRoute], interface: &str) -> Option<&'a PortalRoute> {
    routes
        .iter()
        .find(|route| route.interface == interface || route.interface.ends_with(interface))
}

/// Render a candidate list; an empty list means "any" (unrestricted).
fn join_or_any(candidates: &[String]) -> String {
    if candidates.is_empty() {
        "any".to_owned()
    } else {
        candidates.join(", ")
    }
}

fn section_errors<T>(section: &crate::model::section::Section<T>) -> String {
    section.errors.first().map_or_else(
        || "- no details available".to_owned(),
        |note| format!("- {}", note.message),
    )
}
