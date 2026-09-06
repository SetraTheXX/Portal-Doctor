//! Explicit, bounded Screenshot portal lifecycle.
//!
//! The probe validates only the v3 Window target. It never opens, reads,
//! decodes, copies, logs or persists the portal-created screenshot artifact or
//! its URI. The URI is inspected only for its D-Bus value type and then
//! dropped with the raw response map.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MessageStream, Proxy};

use crate::collectors::timeouts::{
    ACTIVE_PROBE_REQUEST, ACTIVE_PROBE_REQUEST_RECOVERY, ACTIVE_PROBE_SETUP,
};
use crate::error::Error;
use crate::model::probe::{
    CleanupResult, CleanupStatus, ProbeKind, ProbeResult, ProbeStage, ProbeStatus,
};
use crate::probes::portal::{
    HandleTokenError, INTROSPECTABLE_INTERFACE, ResponseWait, bounded_proxy, classify_error,
    close_request, open_session, request_metadata, request_options, response_match_rule,
    valid_request_path_for_sender, wait_for_response,
};

const SCREENSHOT_INTERFACE: &str = "org.freedesktop.portal.Screenshot";
const WINDOW_TARGET: u32 = 2;
const MINIMUM_SCREENSHOT_VERSION: u32 = 3;

/// Run the explicit Screenshot lifecycle in a short-lived current-thread
/// Tokio runtime. Portal outcomes stay in `ProbeResult`; only local runtime
/// creation is a process-level error.
pub fn run() -> Result<ProbeResult, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::ProbeRuntime(error.to_string()))?;
    Ok(runtime.block_on(run_async()))
}

async fn run_async() -> ProbeResult {
    let connection = match open_session().await {
        Ok(connection) => connection,
        Err(outcome) => return result(ProbeStage::Prepare, outcome, CleanupResult::not_required()),
    };

    match inspect_screenshot(&connection).await {
        Ok(()) => {}
        Err(outcome) => return result(ProbeStage::Prepare, outcome, CleanupResult::not_required()),
    }

    // Subscribe before Screenshot is called so a fast portal response cannot
    // race the first signal.
    let mut response_stream =
        match MessageStream::for_match_rule(response_match_rule(), &connection, Some(4)).await {
            Ok(stream) => stream,
            Err(error) => {
                return result(
                    ProbeStage::Request,
                    classify_error(&error),
                    CleanupResult::not_required(),
                );
            }
        };

    let screenshot = match bounded_proxy(&connection, SCREENSHOT_INTERFACE).await {
        Ok(proxy) => proxy,
        Err(outcome) => return result(ProbeStage::Request, outcome, CleanupResult::not_required()),
    };
    let (handle_token, expected_path) = match request_metadata(&connection) {
        Ok(metadata) => metadata,
        Err(HandleTokenError::EntropyUnavailable | HandleTokenError::UniqueNameUnavailable) => {
            return result(
                ProbeStage::Request,
                ProbeStatus::InfrastructureFailure,
                CleanupResult::not_required(),
            );
        }
    };
    let options = screenshot_options(&handle_token);
    let request_body = ("", options);
    let screenshot_call = screenshot.call::<_, _, OwnedObjectPath>("Screenshot", &request_body);
    tokio::pin!(screenshot_call);
    let cancellation = tokio::signal::ctrl_c();
    tokio::pin!(cancellation);
    let request_call = tokio::select! {
        reply = &mut screenshot_call => RequestCall::Reply(reply),
        _ = &mut cancellation => RequestCall::UserCancelled,
        () = tokio::time::sleep(ACTIVE_PROBE_REQUEST) => RequestCall::TimedOut,
    };

    let request_path = match request_call {
        RequestCall::Reply(Ok(path)) if valid_request_path_for_sender(&path, &expected_path) => {
            path
        }
        RequestCall::Reply(Ok(_)) => {
            return result(
                ProbeStage::Request,
                ProbeStatus::MalformedResponse,
                close_request(&connection, expected_path.as_str(), false).await,
            );
        }
        RequestCall::Reply(Err(error)) => {
            let status = classify_error(&error);
            let cleanup = cleanup_for_possible_request(&connection, &expected_path, status).await;
            return result(ProbeStage::Request, status, cleanup);
        }
        RequestCall::UserCancelled => {
            return recover_interrupted_request(
                &connection,
                screenshot_call.as_mut(),
                &expected_path,
                ProbeStatus::UserCancelled,
            )
            .await;
        }
        RequestCall::TimedOut => {
            return recover_interrupted_request(
                &connection,
                screenshot_call.as_mut(),
                &expected_path,
                ProbeStatus::TimedOut,
            )
            .await;
        }
    };

    let response = wait_for_response(&mut response_stream, request_path.as_str()).await;
    let cleanup = close_request(&connection, request_path.as_str(), true).await;
    finalize(response, cleanup)
}

#[derive(Debug)]
enum RequestCall {
    Reply(Result<OwnedObjectPath, zbus::Error>),
    UserCancelled,
    TimedOut,
}

async fn recover_interrupted_request<F>(
    connection: &Connection,
    screenshot_call: Pin<&mut F>,
    expected_path: &OwnedObjectPath,
    status: ProbeStatus,
) -> ProbeResult
where
    F: Future<Output = Result<OwnedObjectPath, zbus::Error>>,
{
    match tokio::time::timeout(ACTIVE_PROBE_REQUEST_RECOVERY, screenshot_call).await {
        Ok(Ok(path)) if valid_request_path_for_sender(&path, expected_path) => {
            let cleanup = close_request(connection, path.as_str(), true).await;
            result(ProbeStage::Request, status, cleanup)
        }
        Ok(Ok(_)) => {
            let cleanup = close_request(connection, expected_path.as_str(), false).await;
            result(ProbeStage::Request, ProbeStatus::MalformedResponse, cleanup)
        }
        Ok(Err(_)) | Err(_) => {
            let cleanup = close_request(connection, expected_path.as_str(), false).await;
            result(ProbeStage::Request, status, cleanup)
        }
    }
}

async fn cleanup_for_possible_request(
    connection: &Connection,
    expected_path: &OwnedObjectPath,
    status: ProbeStatus,
) -> CleanupResult {
    if matches!(
        status,
        ProbeStatus::TimedOut
            | ProbeStatus::InfrastructureFailure
            | ProbeStatus::MalformedResponse
            | ProbeStatus::UserCancelled
    ) {
        close_request(connection, expected_path.as_str(), false).await
    } else {
        CleanupResult::not_required()
    }
}

async fn inspect_screenshot(connection: &Connection) -> Result<(), ProbeStatus> {
    let introspection = bounded_proxy(connection, INTROSPECTABLE_INTERFACE).await?;
    let xml: String =
        match tokio::time::timeout(ACTIVE_PROBE_SETUP, introspection.call("Introspect", &())).await
        {
            Ok(Ok(xml)) => xml,
            Ok(Err(error)) => return Err(classify_error(&error)),
            Err(_) => return Err(ProbeStatus::TimedOut),
        };
    if !introspection_supports_screenshot(&xml) {
        return Err(ProbeStatus::Unsupported);
    }

    let screenshot = bounded_proxy(connection, SCREENSHOT_INTERFACE).await?;
    let version = read_property(&screenshot, "version").await?;
    if version < MINIMUM_SCREENSHOT_VERSION {
        return Err(ProbeStatus::Unsupported);
    }
    let available_targets = read_property(&screenshot, "AvailableTargets").await?;
    if !window_target_is_available(available_targets) {
        return Err(ProbeStatus::Unsupported);
    }
    Ok(())
}

async fn read_property(proxy: &Proxy<'_>, name: &str) -> Result<u32, ProbeStatus> {
    match tokio::time::timeout(ACTIVE_PROBE_SETUP, proxy.get_property::<u32>(name)).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(classify_error(&error)),
        Err(_) => Err(ProbeStatus::TimedOut),
    }
}

fn screenshot_options(handle_token: &str) -> HashMap<String, OwnedValue> {
    let mut options = request_options(handle_token);
    options.insert(
        "modal".to_owned(),
        OwnedValue::try_from(Value::from(true)).expect("a boolean is a valid D-Bus value"),
    );
    options.insert(
        "interactive".to_owned(),
        OwnedValue::try_from(Value::from(true)).expect("a boolean is a valid D-Bus value"),
    );
    options.insert(
        "target".to_owned(),
        OwnedValue::try_from(Value::from(WINDOW_TARGET))
            .expect("a target bitmask is a valid D-Bus value"),
    );
    options
}

fn finalize(response: ResponseWait, cleanup: CleanupResult) -> ProbeResult {
    let (status, stage) = match response {
        ResponseWait::Outcome { code, results } => {
            let status = decode_response(code, &results);
            let stage = if matches!(status, ProbeStatus::Success | ProbeStatus::UserCancelled) {
                ProbeStage::Complete
            } else {
                ProbeStage::Response
            };
            (status, stage)
        }
        ResponseWait::UserCancelled => (ProbeStatus::UserCancelled, ProbeStage::Complete),
        ResponseWait::TimedOut => (ProbeStatus::TimedOut, ProbeStage::Response),
        ResponseWait::InfrastructureFailure => {
            (ProbeStatus::InfrastructureFailure, ProbeStage::Response)
        }
        ResponseWait::MalformedResponse => (ProbeStatus::MalformedResponse, ProbeStage::Response),
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
    ProbeResult::new(ProbeKind::Screenshot, stage, status, cleanup)
        .expect("Screenshot lifecycle must emit a valid ProbeResult v1")
}

fn decode_response(status: u32, results: &HashMap<String, OwnedValue>) -> ProbeStatus {
    match status {
        0 if has_uri_string(results) => ProbeStatus::Success,
        1 => ProbeStatus::UserCancelled,
        2 => ProbeStatus::InfrastructureFailure,
        _ => ProbeStatus::MalformedResponse,
    }
}

fn has_uri_string(results: &HashMap<String, OwnedValue>) -> bool {
    results
        .get("uri")
        .is_some_and(|value| value.value_signature().to_string() == "s")
}

/// Validate only the Screenshot interface structure needed by this slice.
/// No portal payload or screenshot URI is retained by this preflight check.
pub(crate) fn introspection_supports_screenshot(xml: &str) -> bool {
    let Some(interface_start) = xml.find("<interface name=\"org.freedesktop.portal.Screenshot\"")
    else {
        return false;
    };
    let interface = &xml[interface_start..];
    let Some(interface_end) = interface.find("</interface>") else {
        return false;
    };
    let interface = &interface[..interface_end];
    interface.contains("<method name=\"Screenshot\"")
        && interface.contains("<property name=\"version\"")
        && interface.contains("<property name=\"AvailableTargets\"")
}

pub(crate) const fn window_target_is_available(available_targets: u32) -> bool {
    available_targets & WINDOW_TARGET != 0
}

/// Stable shell mapping for the explicit command. A user cancellation and
/// every non-clean lifecycle outcome return 1; passive exit codes are
/// unchanged because this is a separate `RunOutcome` variant.
#[must_use]
pub fn exit_code(result: &ProbeResult) -> u8 {
    u8::from(!result.is_clean_success())
}

/// Render a human-readable result without exposing the URI, path, filename or
/// raw portal response.
#[must_use]
pub fn render_terminal(result: &ProbeResult) -> String {
    let failed_resources = result
        .cleanup()
        .failed_resources()
        .iter()
        .map(|_| "request")
        .collect::<Vec<_>>();
    let failed_resources = if failed_resources.is_empty() {
        "none".to_owned()
    } else {
        failed_resources.join(", ")
    };
    format!(
        "PortalDoctor Screenshot probe\nOperation: {}\nStage: {}\nCleanup: {}\nCleanup resources: {failed_resources}",
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

#[cfg(test)]
mod tests {
    use super::{
        decode_response, exit_code, has_uri_string, introspection_supports_screenshot,
        render_terminal, screenshot_options, window_target_is_available,
    };
    use crate::model::probe::{CleanupResult, ProbeKind, ProbeResult, ProbeStage, ProbeStatus};
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedValue, Value};

    #[test]
    fn introspection_requires_v3_properties_and_method() {
        let xml = r#"
            <node>
              <interface name="org.freedesktop.portal.Screenshot">
                <property name="version" type="u" access="read"/>
                <property name="AvailableTargets" type="u" access="read"/>
                <method name="Screenshot"/>
              </interface>
            </node>
        "#;
        assert!(introspection_supports_screenshot(xml));
        assert!(!introspection_supports_screenshot(
            "<interface name=\"org.freedesktop.portal.Screenshot\"><method name=\"Screenshot\"/></interface>"
        ));
        assert!(!introspection_supports_screenshot(
            "<interface name=\"org.freedesktop.portal.FileChooser\"><method name=\"OpenFile\"/></interface>"
        ));
    }

    #[test]
    fn window_target_policy_does_not_accept_other_targets() {
        assert!(window_target_is_available(2));
        assert!(window_target_is_available(3));
        assert!(!window_target_is_available(1));
        assert!(!window_target_is_available(4));
        assert!(!window_target_is_available(8));
    }

    #[test]
    fn options_are_explicitly_window_interactive_and_modal() {
        let options = screenshot_options("private-token");
        assert_eq!(
            String::try_from(&**options.get("handle_token").unwrap()).unwrap(),
            "private-token"
        );
        assert!(bool::try_from(&**options.get("modal").unwrap()).unwrap());
        assert!(bool::try_from(&**options.get("interactive").unwrap()).unwrap());
        assert_eq!(u32::try_from(&**options.get("target").unwrap()).unwrap(), 2);
    }

    #[test]
    fn response_requires_a_string_uri_without_reading_its_contents() {
        let mut success = HashMap::new();
        success.insert(
            "uri".to_owned(),
            OwnedValue::try_from(Value::from(
                "document://portal/private-screenshot.png".to_owned(),
            ))
            .unwrap(),
        );
        assert!(has_uri_string(&success));
        assert_eq!(decode_response(0, &success), ProbeStatus::Success);
        assert_eq!(
            decode_response(0, &HashMap::new()),
            ProbeStatus::MalformedResponse
        );

        let mut wrong_type = HashMap::new();
        wrong_type.insert(
            "uri".to_owned(),
            OwnedValue::try_from(Value::from(7_u32)).unwrap(),
        );
        assert!(!has_uri_string(&wrong_type));
        assert_eq!(
            decode_response(0, &wrong_type),
            ProbeStatus::MalformedResponse
        );
        assert_eq!(
            decode_response(1, &HashMap::new()),
            ProbeStatus::UserCancelled
        );
        assert_eq!(
            decode_response(2, &HashMap::new()),
            ProbeStatus::InfrastructureFailure
        );
        assert_eq!(
            decode_response(9, &HashMap::new()),
            ProbeStatus::MalformedResponse
        );
    }

    #[test]
    fn terminal_and_json_results_never_contain_portal_artifact_data() {
        let result = ProbeResult::new(
            ProbeKind::Screenshot,
            ProbeStage::Complete,
            ProbeStatus::Success,
            CleanupResult::completed(),
        )
        .unwrap();
        let terminal = render_terminal(&result);
        let json = serde_json::to_string(&result).unwrap();

        for output in [terminal, json] {
            assert!(!output.contains("document://"));
            assert!(!output.contains("file://"));
            assert!(!output.contains("fake-screenshot"));
            assert!(!output.contains(".png"));
            assert!(!output.contains("window-secret"));
        }
        assert_eq!(exit_code(&result), 0);
    }
}
