use crate::collectors::timeouts::{NORMAL_RUNTIME_QUERY, run_bounded};
use crate::model::dbus::{DbusCheck, DbusInfo, DbusOutcome, PORTAL_FRONTEND_NAME};
use crate::model::section::Section;

/// Collect runtime D-Bus state: session bus reachability plus owner checks
/// for the portal frontend and the selected backend names. Every call runs
/// inside the central timeout policy; a wedged service yields a classified
/// outcome instead of hanging the run.
pub fn collect(selected_backend_names: &[String]) -> Section<DbusInfo> {
    let selected_backend_names = selected_backend_names.to_vec();
    run_bounded(NORMAL_RUNTIME_QUERY, move || {
        Section::available(probe(&selected_backend_names))
    })
    .unwrap_or_else(|| {
        Section::available(DbusInfo {
            connected: false,
            checks: vec![DbusCheck {
                name: PORTAL_FRONTEND_NAME.to_owned(),
                outcome: DbusOutcome::Timeout,
            }],
        })
    })
}

fn probe(selected_backend_names: &[String]) -> DbusInfo {
    let mut checks = Vec::new();
    let connection = zbus::blocking::Connection::session();
    match connection {
        Ok(connection) => {
            match zbus::blocking::fdo::DBusProxy::new(&connection) {
                Ok(proxy) => {
                    checks.push(check(&proxy, PORTAL_FRONTEND_NAME));
                    // Deduplicated and sorted for deterministic output.
                    let mut names: Vec<&str> =
                        selected_backend_names.iter().map(String::as_str).collect();
                    names.sort_unstable();
                    names.dedup();
                    for name in names {
                        checks.push(check(&proxy, name));
                    }
                }
                Err(err) => {
                    let classified = classify(&err);
                    checks.push(DbusCheck {
                        name: PORTAL_FRONTEND_NAME.to_owned(),
                        outcome: classified,
                    });
                }
            }
        }
        // Any connection failure means we could not reach the session bus;
        // finer classification happens at the call level below.
        Err(err) => {
            checks.push(DbusCheck {
                name: PORTAL_FRONTEND_NAME.to_owned(),
                outcome: DbusOutcome::NoSessionBus,
            });
            let _ = err.to_string();
            return DbusInfo {
                connected: false,
                checks,
            };
        }
    }
    DbusInfo {
        connected: true,
        checks,
    }
}

fn check(proxy: &zbus::blocking::fdo::DBusProxy<'_>, name: &str) -> DbusCheck {
    let outcome = match zbus::names::BusName::try_from(name.to_owned()) {
        // The fdo proxy reports its own error type; map it onto zbus::Error
        // so the whole taxonomy lives in one classifier.
        Ok(bus_name) => match proxy.name_has_owner(bus_name) {
            Ok(true) => DbusOutcome::HasOwner,
            Ok(false) => DbusOutcome::NoOwner,
            Err(err) => classify(&zbus::Error::from(err)),
        },
        Err(_) => DbusOutcome::MalformedResponse,
    };
    DbusCheck {
        name: name.to_owned(),
        outcome,
    }
}

/// Map a `zbus` error onto the architecture §11 failure taxonomy using its
/// well-known message fragments.
fn classify(err: &zbus::Error) -> DbusOutcome {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        DbusOutcome::Timeout
    } else if lower.contains("accessdenied")
        || lower.contains("access denied")
        || lower.contains("not allowed")
        || lower.contains("permission")
    {
        DbusOutcome::AccessDenied
    } else if lower.contains("serviceunknown") || lower.contains("was not provided by any") {
        DbusOutcome::NoOwner
    } else if lower.contains("activation") {
        DbusOutcome::ActivationFailure
    } else if lower.contains("malformed") || lower.contains("invalid") {
        DbusOutcome::MalformedResponse
    } else {
        DbusOutcome::Other(message)
    }
}

#[cfg(test)]
mod tests {
    use super::classify;
    use crate::model::dbus::DbusOutcome;

    #[test]
    fn classify_maps_message_fragments_to_taxonomy() {
        // Build errors through their public Display round-trip: the classifier
        // works on message fragments by design.
        let make = |msg: &str| zbus::Error::Handshake(msg.to_owned());
        assert!(matches!(
            classify(&make("Connection timed out")),
            DbusOutcome::Timeout
        ));
        assert!(matches!(
            classify(&make("AccessDenied: not allowed")),
            DbusOutcome::AccessDenied
        ));
        assert!(matches!(
            classify(&make("activation of x failed")),
            DbusOutcome::ActivationFailure
        ));
        assert!(matches!(
            classify(&make("malformed reply")),
            DbusOutcome::MalformedResponse
        ));
        assert!(matches!(
            classify(&make("something entirely new")),
            DbusOutcome::Other(_)
        ));
    }
}
