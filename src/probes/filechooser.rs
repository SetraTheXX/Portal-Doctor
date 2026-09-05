//! Explicit `FileChooser` portal lifecycle.
//!
//! This module intentionally owns only the first bounded active-probe slice.
//! It never reads the selected file: the response is validated for protocol
//! shape and discarded. The request path is retained only long enough to issue
//! the required `Request.Close` cleanup call.

use std::collections::HashMap;

use futures_lite::StreamExt;
use zbus::zvariant::{Array, OwnedObjectPath, OwnedValue};
use zbus::{Connection, MatchRule, MessageStream, Proxy};

use crate::collectors::timeouts::{
    ACTIVE_PROBE_CLEANUP, ACTIVE_PROBE_RESPONSE, ACTIVE_PROBE_SETUP,
};
use crate::error::Error;
use crate::model::probe::{
    CleanupResource, CleanupResult, CleanupStatus, ProbeKind, ProbeResult, ProbeStage, ProbeStatus,
};

const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const FILE_CHOOSER_INTERFACE: &str = "org.freedesktop.portal.FileChooser";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const INTROSPECTABLE_INTERFACE: &str = "org.freedesktop.DBus.Introspectable";
const RESPONSE_MEMBER: &str = "Response";
const FILE_CHOOSER_TITLE: &str = "PortalDoctor FileChooser probe";

/// Run the explicit `FileChooser` lifecycle in a short-lived current-thread
/// Tokio runtime. Expected portal outcomes are represented by `ProbeResult`;
/// only failure to create the local runtime is a process-level error.
pub fn run() -> Result<ProbeResult, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::ProbeRuntime(error.to_string()))?;
    Ok(runtime.block_on(run_async()))
}

async fn run_async() -> ProbeResult {
    let connection = match tokio::time::timeout(ACTIVE_PROBE_SETUP, Connection::session()).await {
        Ok(Ok(connection)) => connection,
        Ok(Err(error)) => {
            return result(
                ProbeStage::Prepare,
                classify_error(&error),
                CleanupResult::not_required(),
            );
        }
        Err(_) => {
            return result(
                ProbeStage::Prepare,
                ProbeStatus::TimedOut,
                CleanupResult::not_required(),
            );
        }
    };

    match inspect_filechooser(&connection).await {
        Ok(()) => {}
        Err(outcome) => return result(ProbeStage::Prepare, outcome, CleanupResult::not_required()),
    }

    // The match is installed before OpenFile is sent. Matching by request path
    // after the method returns closes the response race required by ASHPD.
    let rule = response_match_rule();
    let mut response_stream = match MessageStream::for_match_rule(rule, &connection, Some(4)).await
    {
        Ok(stream) => stream,
        Err(error) => {
            return result(
                ProbeStage::Request,
                classify_error(&error),
                CleanupResult::not_required(),
            );
        }
    };

    let chooser = match bounded_proxy(&connection, FILE_CHOOSER_INTERFACE).await {
        Ok(proxy) => proxy,
        Err(outcome) => return result(ProbeStage::Request, outcome, CleanupResult::not_required()),
    };
    let options: HashMap<String, OwnedValue> = HashMap::new();
    let request_path: OwnedObjectPath = match tokio::time::timeout(
        ACTIVE_PROBE_SETUP,
        chooser.call("OpenFile", &("", FILE_CHOOSER_TITLE, options)),
    )
    .await
    {
        Ok(Ok(path)) if valid_request_path(&path) => path,
        Ok(Ok(_)) => {
            return result(
                ProbeStage::Request,
                ProbeStatus::MalformedResponse,
                CleanupResult::unverified(),
            );
        }
        Ok(Err(error)) => {
            return result(
                ProbeStage::Request,
                classify_error(&error),
                CleanupResult::not_required(),
            );
        }
        Err(_) => {
            // A portal may have created a request even when its method reply
            // timed out, but there is no safe handle to close in this branch.
            return result(
                ProbeStage::Request,
                ProbeStatus::TimedOut,
                CleanupResult::unverified(),
            );
        }
    };

    let response = wait_for_response(&mut response_stream, request_path.as_str()).await;
    let cleanup = close_request(&connection, request_path.as_str()).await;
    finalize(response, cleanup)
}

async fn inspect_filechooser(connection: &Connection) -> Result<(), ProbeStatus> {
    let introspection = bounded_proxy(connection, INTROSPECTABLE_INTERFACE).await?;
    let xml: String =
        match tokio::time::timeout(ACTIVE_PROBE_SETUP, introspection.call("Introspect", &())).await
        {
            Ok(Ok(xml)) => xml,
            Ok(Err(error)) => return Err(classify_error(&error)),
            Err(_) => return Err(ProbeStatus::TimedOut),
        };
    if introspection_supports_filechooser(&xml) {
        Ok(())
    } else {
        Err(ProbeStatus::Unsupported)
    }
}

async fn bounded_proxy<'a>(
    connection: &'a Connection,
    interface: &'a str,
) -> Result<Proxy<'a>, ProbeStatus> {
    match tokio::time::timeout(
        ACTIVE_PROBE_SETUP,
        Proxy::new(connection, PORTAL_DESTINATION, PORTAL_PATH, interface),
    )
    .await
    {
        Ok(Ok(proxy)) => Ok(proxy),
        Ok(Err(error)) => Err(classify_error(&error)),
        Err(_) => Err(ProbeStatus::TimedOut),
    }
}

fn response_match_rule() -> MatchRule<'static> {
    MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(PORTAL_DESTINATION)
        .expect("portal destination is a valid D-Bus bus name")
        .interface(REQUEST_INTERFACE)
        .expect("request interface is a valid D-Bus interface name")
        .member(RESPONSE_MEMBER)
        .expect("Response is a valid D-Bus member name")
        .build()
}

fn valid_request_path(path: &OwnedObjectPath) -> bool {
    let value = path.as_str();
    value.starts_with("/org/freedesktop/portal/desktop/request/")
        && value.len() > "/org/freedesktop/portal/desktop/request/".len()
}

#[derive(Clone, Copy)]
enum ResponseWait {
    Outcome(ProbeStatus),
    UserCancelled,
    TimedOut,
}

async fn wait_for_response(stream: &mut MessageStream, request_path: &str) -> ResponseWait {
    let response = async {
        loop {
            let Some(message) = stream.next().await else {
                return ResponseWait::Outcome(ProbeStatus::InfrastructureFailure);
            };
            let Ok(message) = message else {
                return ResponseWait::Outcome(ProbeStatus::InfrastructureFailure);
            };
            let header = message.header();
            let Some(path) = header.path() else {
                continue;
            };
            if path.as_str() != request_path {
                continue;
            }
            let body: (u32, HashMap<String, OwnedValue>) = match message.body().deserialize() {
                Ok(body) => body,
                Err(_) => return ResponseWait::Outcome(ProbeStatus::MalformedResponse),
            };
            return ResponseWait::Outcome(decode_response(body.0, &body.1));
        }
    };
    tokio::pin!(response);
    let cancellation = async { tokio::signal::ctrl_c().await.ok() };
    tokio::pin!(cancellation);
    tokio::select! {
        outcome = &mut response => outcome,
        _ = &mut cancellation => ResponseWait::UserCancelled,
        () = tokio::time::sleep(ACTIVE_PROBE_RESPONSE) => ResponseWait::TimedOut,
    }
}

async fn close_request(connection: &Connection, request_path: &str) -> CleanupResult {
    let Ok(Ok(proxy)) = tokio::time::timeout(
        ACTIVE_PROBE_CLEANUP,
        Proxy::new(
            connection,
            PORTAL_DESTINATION,
            request_path,
            REQUEST_INTERFACE,
        ),
    )
    .await
    else {
        return failed_request_cleanup();
    };
    match tokio::time::timeout(ACTIVE_PROBE_CLEANUP, proxy.call::<_, _, ()>("Close", &())).await {
        Ok(Ok(())) => CleanupResult::completed(),
        Ok(Err(error)) if request_already_gone(&error) => CleanupResult::completed(),
        Ok(Err(_)) | Err(_) => failed_request_cleanup(),
    }
}

fn failed_request_cleanup() -> CleanupResult {
    CleanupResult::failed(vec![CleanupResource::Request])
        .expect("the FileChooser cleanup resource is valid")
}

fn finalize(response: ResponseWait, cleanup: CleanupResult) -> ProbeResult {
    let (status, stage) = match response {
        ResponseWait::Outcome(status) => {
            let stage = if matches!(status, ProbeStatus::Success | ProbeStatus::UserCancelled) {
                ProbeStage::Complete
            } else {
                ProbeStage::Response
            };
            (status, stage)
        }
        ResponseWait::UserCancelled => (ProbeStatus::UserCancelled, ProbeStage::Complete),
        ResponseWait::TimedOut => (ProbeStatus::TimedOut, ProbeStage::Response),
    };
    result(stage, status, cleanup)
}

fn result(stage: ProbeStage, status: ProbeStatus, cleanup: CleanupResult) -> ProbeResult {
    let stage = if matches!(
        cleanup.status(),
        CleanupStatus::Failed | CleanupStatus::Unverified
    ) {
        ProbeStage::Cleanup
    } else {
        stage
    };
    ProbeResult::new(ProbeKind::FileChooser, stage, status, cleanup)
        .expect("FileChooser lifecycle must emit a valid ProbeResult v1")
}

fn decode_response(status: u32, results: &HashMap<String, OwnedValue>) -> ProbeStatus {
    match status {
        0 if has_uri_array(results) => ProbeStatus::Success,
        1 => ProbeStatus::UserCancelled,
        2 => ProbeStatus::InfrastructureFailure,
        _ => ProbeStatus::MalformedResponse,
    }
}

fn has_uri_array(results: &HashMap<String, OwnedValue>) -> bool {
    let Some(value) = results.get("uris") else {
        return false;
    };
    let Ok(array) = <&Array<'_>>::try_from(value) else {
        return false;
    };
    array.signature().to_string() == "as"
}

/// Check only the introspection structure needed for this explicit probe.
/// No portal-specific payload or user-selected URI is retained here.
pub(crate) fn introspection_supports_filechooser(xml: &str) -> bool {
    let Some(interface_start) = xml.find("<interface name=\"org.freedesktop.portal.FileChooser\"")
    else {
        return false;
    };
    let interface = &xml[interface_start..];
    let Some(interface_end) = interface.find("</interface>") else {
        return false;
    };
    interface[..interface_end].contains("<method name=\"OpenFile\"")
}

fn classify_error(error: &zbus::Error) -> ProbeStatus {
    classify_error_message(&error.to_string())
}

pub(crate) fn classify_error_message(message: &str) -> ProbeStatus {
    let message = message.to_ascii_lowercase();
    if message.contains("timedout")
        || message.contains("timed out")
        || message.contains("noreply")
        || message.contains("no reply")
    {
        ProbeStatus::TimedOut
    } else if message.contains("serviceunknown")
        || message.contains("namehasnoowner")
        || message.contains("no such service")
        || message.contains("no session bus")
        || message.contains("dbus_session_bus_address")
        || message.contains("connection refused")
        || message.contains("failed to connect")
        || message.contains("could not connect")
        || message.contains("no server")
    {
        ProbeStatus::Unavailable
    } else if message.contains("unknownmethod")
        || message.contains("unknown method")
        || message.contains("unknowninterface")
        || message.contains("unknown interface")
        || message.contains("not supported")
        || message.contains("notsupported")
        || message.contains("notimplemented")
    {
        ProbeStatus::Unsupported
    } else {
        ProbeStatus::InfrastructureFailure
    }
}

fn request_already_gone(error: &zbus::Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("unknownobject")
        || message.contains("unknown object")
        || message.contains("no such object")
        || message.contains("does not exist")
}

/// Stable shell mapping for the explicit command. A user cancellation and
/// every non-clean lifecycle outcome return `1`; passive exit codes are
/// unchanged because this is a separate `RunOutcome` variant.
#[must_use]
pub fn exit_code(result: &ProbeResult) -> u8 {
    u8::from(!result.is_clean_success())
}

/// Render a human-readable result without exposing URIs, filenames or raw
/// portal errors.
#[must_use]
pub fn render_terminal(result: &ProbeResult) -> String {
    let failed_resources = result
        .cleanup()
        .failed_resources()
        .iter()
        .map(|&resource| cleanup_resource_name(resource))
        .collect::<Vec<_>>();
    let failed_resources = if failed_resources.is_empty() {
        "none".to_owned()
    } else {
        failed_resources.join(", ")
    };
    format!(
        "PortalDoctor FileChooser probe\nOperation: {}\nStage: {}\nCleanup: {}\nCleanup resources: {failed_resources}",
        probe_status_name(result.status()),
        probe_stage_name(result.stage()),
        cleanup_status_name(result.cleanup().status()),
    )
}

fn probe_status_name(status: ProbeStatus) -> &'static str {
    match status {
        ProbeStatus::Success => "success",
        ProbeStatus::UserCancelled => "user_cancelled",
        ProbeStatus::TimedOut => "timed_out",
        ProbeStatus::Unavailable => "unavailable",
        ProbeStatus::Unsupported => "unsupported",
        ProbeStatus::MalformedResponse => "malformed_response",
        ProbeStatus::InfrastructureFailure => "infrastructure_failure",
    }
}

fn probe_stage_name(stage: ProbeStage) -> &'static str {
    match stage {
        ProbeStage::Prepare => "prepare",
        ProbeStage::Request => "request",
        ProbeStage::Response => "response",
        ProbeStage::CreateSession => "create_session",
        ProbeStage::SelectSources => "select_sources",
        ProbeStage::Start => "start",
        ProbeStage::StreamsReturned => "streams_returned",
        ProbeStage::OpenPipeWireRemote => "open_pipe_wire_remote",
        ProbeStage::Cleanup => "cleanup",
        ProbeStage::Complete => "complete",
    }
}

fn cleanup_status_name(status: CleanupStatus) -> &'static str {
    match status {
        CleanupStatus::NotRequired => "not_required",
        CleanupStatus::Completed => "completed",
        CleanupStatus::Failed => "failed",
        CleanupStatus::Unverified => "unverified",
    }
}

fn cleanup_resource_name(resource: CleanupResource) -> &'static str {
    match resource {
        CleanupResource::Request => "request",
        CleanupResource::Session => "session",
        CleanupResource::PipeWireRemote => "pipe_wire_remote",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ResponseWait, classify_error_message, decode_response, exit_code, finalize, has_uri_array,
        introspection_supports_filechooser, render_terminal, response_match_rule,
        valid_request_path,
    };
    use crate::model::probe::{
        CleanupResource, CleanupResult, CleanupStatus, ProbeKind, ProbeResult, ProbeStage,
        ProbeStatus,
    };
    use std::collections::HashMap;
    use zbus::zvariant::{Array, OwnedObjectPath, OwnedValue};

    #[test]
    fn introspection_requires_the_interface_and_open_file_method() {
        let xml = r#"
            <node>
              <interface name="org.freedesktop.portal.FileChooser">
                <method name="OpenFile"/>
              </interface>
            </node>
        "#;
        assert!(introspection_supports_filechooser(xml));
        assert!(!introspection_supports_filechooser(
            "<interface name=\"org.freedesktop.portal.FileChooser\"><method name=\"SaveFile\"/></interface>"
        ));
        assert!(!introspection_supports_filechooser(
            "<interface name=\"org.freedesktop.portal.Screenshot\"><method name=\"OpenFile\"/></interface>"
        ));
    }

    #[test]
    fn response_statuses_are_distinct_without_reading_file_content() {
        let mut results = HashMap::new();
        let uris = Array::from(vec!["file:///tmp/not-read"]);
        results.insert(
            "uris".to_owned(),
            OwnedValue::try_from(uris).expect("string array is a valid D-Bus value"),
        );
        assert!(has_uri_array(&results));
        assert_eq!(decode_response(0, &results), ProbeStatus::Success);
        assert_eq!(
            decode_response(1, &HashMap::new()),
            ProbeStatus::UserCancelled
        );
        assert_eq!(
            decode_response(2, &HashMap::new()),
            ProbeStatus::InfrastructureFailure
        );
        assert_eq!(
            decode_response(7, &HashMap::new()),
            ProbeStatus::MalformedResponse
        );
        assert_eq!(
            decode_response(0, &HashMap::new()),
            ProbeStatus::MalformedResponse
        );
    }

    #[test]
    fn error_classes_preserve_unavailable_and_unsupported_boundaries() {
        assert_eq!(
            classify_error_message("org.freedesktop.DBus.Error.ServiceUnknown"),
            ProbeStatus::Unavailable
        );
        assert_eq!(
            classify_error_message("DBUS_SESSION_BUS_ADDRESS is not set"),
            ProbeStatus::Unavailable
        );
        assert_eq!(
            classify_error_message("org.freedesktop.DBus.Error.UnknownMethod"),
            ProbeStatus::Unsupported
        );
        assert_eq!(
            classify_error_message("org.freedesktop.DBus.Error.NoReply: timed out"),
            ProbeStatus::TimedOut
        );
        assert_eq!(
            classify_error_message("org.freedesktop.DBus.Error.AccessDenied"),
            ProbeStatus::InfrastructureFailure
        );
    }

    #[test]
    fn rendered_result_never_contains_a_selected_uri() {
        let result = ProbeResult::new(
            ProbeKind::FileChooser,
            ProbeStage::Complete,
            ProbeStatus::Success,
            CleanupResult::completed(),
        )
        .unwrap();
        let rendered = render_terminal(&result);
        assert!(!rendered.contains("file://"));
        assert!(!rendered.contains("not-read"));
    }

    #[test]
    fn cleanup_failure_moves_the_terminal_stage_to_cleanup_without_erasing_status() {
        let result = finalize(
            ResponseWait::Outcome(ProbeStatus::Success),
            CleanupResult::failed(vec![CleanupResource::Request]).unwrap(),
        );
        assert_eq!(result.status(), ProbeStatus::Success);
        assert_eq!(result.stage(), ProbeStage::Cleanup);
        assert_eq!(result.cleanup().status(), CleanupStatus::Failed);
        assert_eq!(exit_code(&result), 1);

        let cancelled = finalize(ResponseWait::UserCancelled, CleanupResult::completed());
        assert_eq!(cancelled.status(), ProbeStatus::UserCancelled);
        assert_eq!(cancelled.stage(), ProbeStage::Complete);
        assert_eq!(exit_code(&cancelled), 1);

        let timed_out = finalize(ResponseWait::TimedOut, CleanupResult::completed());
        assert_eq!(timed_out.status(), ProbeStatus::TimedOut);
        assert_eq!(timed_out.stage(), ProbeStage::Response);
        assert_eq!(exit_code(&timed_out), 1);
    }

    #[test]
    fn request_path_validation_does_not_accept_unrelated_objects() {
        let valid =
            OwnedObjectPath::try_from("/org/freedesktop/portal/desktop/request/1_42").unwrap();
        let invalid = OwnedObjectPath::try_from("/org/freedesktop/portal/desktop").unwrap();
        assert!(valid_request_path(&valid));
        assert!(!valid_request_path(&invalid));
    }

    #[test]
    fn response_match_is_registered_for_portal_request_responses() {
        let rule = response_match_rule().to_string();
        assert!(rule.contains("sender='org.freedesktop.portal.Desktop'"));
        assert!(rule.contains("interface='org.freedesktop.portal.Request'"));
        assert!(rule.contains("member='Response'"));
    }
}
