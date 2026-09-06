//! Explicit `FileChooser` portal lifecycle.
//!
//! This module intentionally owns only the first bounded active-probe slice.
//! It never reads the selected file: the response is validated for protocol
//! shape and discarded. The request path is retained only long enough to issue
//! the required `Request.Close` cleanup call.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use zbus::zvariant::{Array, OwnedObjectPath, OwnedValue};
use zbus::{Connection, MessageStream};

use crate::collectors::timeouts::{
    ACTIVE_PROBE_REQUEST, ACTIVE_PROBE_REQUEST_RECOVERY, ACTIVE_PROBE_SETUP,
};
use crate::error::Error;
use crate::model::probe::{
    CleanupResource, CleanupResult, CleanupStatus, ProbeKind, ProbeResult, ProbeStage, ProbeStatus,
};
use crate::probes::portal::{
    HandleTokenError, INTROSPECTABLE_INTERFACE, ResponseWait, bounded_proxy, classify_error,
    close_request, open_session, request_metadata, request_options, response_match_rule,
    valid_request_path_for_sender, wait_for_response,
};

const FILE_CHOOSER_INTERFACE: &str = "org.freedesktop.portal.FileChooser";
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
    let connection = match open_session().await {
        Ok(connection) => connection,
        Err(outcome) => return result(ProbeStage::Prepare, outcome, CleanupResult::not_required()),
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
    let options = request_options(&handle_token);
    let request_body = ("", FILE_CHOOSER_TITLE, options);
    let open_file = chooser.call::<_, _, OwnedObjectPath>("OpenFile", &request_body);
    tokio::pin!(open_file);
    let cancellation = tokio::signal::ctrl_c();
    tokio::pin!(cancellation);
    let request_call = tokio::select! {
        reply = &mut open_file => RequestCall::Reply(reply),
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
                open_file.as_mut(),
                &expected_path,
                ProbeStatus::UserCancelled,
            )
            .await;
        }
        RequestCall::TimedOut => {
            return recover_interrupted_request(
                &connection,
                open_file.as_mut(),
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

/// Recover a method reply after cancellation or a request-stage deadline.
///
/// The XDG portal convention lets us predict the Request path from the
/// caller's unique name and `handle_token`. Keeping the original future alive
/// for this bounded grace period covers the normal late-reply race. If the
/// service never replies, the predicted path is still closed once, but an
/// unknown-object response is deliberately reported as `unverified`: a
/// transport that never returned cannot prove whether a request was created.
async fn recover_interrupted_request<F>(
    connection: &Connection,
    open_file: Pin<&mut F>,
    expected_path: &OwnedObjectPath,
    status: ProbeStatus,
) -> ProbeResult
where
    F: Future<Output = Result<OwnedObjectPath, zbus::Error>>,
{
    match tokio::time::timeout(ACTIVE_PROBE_REQUEST_RECOVERY, open_file).await {
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
        decode_response, exit_code, finalize, has_uri_array, introspection_supports_filechooser,
        render_terminal,
    };
    use crate::model::probe::{
        CleanupResource, CleanupResult, CleanupStatus, ProbeKind, ProbeResult, ProbeStage,
        ProbeStatus,
    };
    use crate::probes::portal::{
        HandleTokenError, ResponseWait, classify_error_message, handle_token,
        handle_token_with_entropy, request_options, request_path_for_sender, response_match_rule,
        valid_request_path, valid_request_path_for_sender,
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
        let mut successful_results = HashMap::new();
        successful_results.insert(
            "uris".to_owned(),
            OwnedValue::try_from(Array::from(vec!["file:///tmp/not-read"])).unwrap(),
        );
        let result = finalize(
            ResponseWait::Outcome {
                code: 0,
                results: successful_results,
            },
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
    fn request_path_validation_keeps_sender_correlation_but_allows_legacy_tokens() {
        let expected = OwnedObjectPath::try_from(
            "/org/freedesktop/portal/desktop/request/1_42/portaldoctor_expected",
        )
        .unwrap();
        let legacy = OwnedObjectPath::try_from(
            "/org/freedesktop/portal/desktop/request/1_42/portal_generated",
        )
        .unwrap();
        let unrelated = OwnedObjectPath::try_from(
            "/org/freedesktop/portal/desktop/request/1_99/portaldoctor_expected",
        )
        .unwrap();
        assert!(valid_request_path_for_sender(&expected, &expected));
        assert!(valid_request_path_for_sender(&legacy, &expected));
        assert!(!valid_request_path_for_sender(&unrelated, &expected));
    }

    #[test]
    fn token_and_predicted_path_are_private_dbus_request_metadata() {
        let token = handle_token().expect("the operating system entropy source is available");
        assert!(token.starts_with("portaldoctor_"));
        assert!(
            token
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        );
        let options = request_options(&token);
        let encoded = String::try_from(&**options.get("handle_token").unwrap())
            .expect("handle_token is encoded as a D-Bus string");
        assert_eq!(encoded, token);
        assert_eq!(
            request_path_for_sender(":1.42", &token).unwrap().as_str(),
            format!("/org/freedesktop/portal/desktop/request/1_42/{token}")
        );
        assert!(request_path_for_sender("org.freedesktop.portal.Desktop", &token).is_none());
    }

    #[test]
    fn generated_handle_tokens_are_unique() {
        let mut tokens = std::collections::HashSet::new();
        for _ in 0..64 {
            let token = handle_token().expect("the operating system entropy source is available");
            assert!(tokens.insert(token));
        }
    }

    #[test]
    fn handle_token_generation_fails_closed_without_entropy() {
        let result = handle_token_with_entropy(|_| Err(HandleTokenError::EntropyUnavailable));
        assert_eq!(result, Err(HandleTokenError::EntropyUnavailable));
    }

    #[test]
    fn response_match_is_registered_for_portal_request_responses() {
        let rule = response_match_rule().to_string();
        assert!(rule.contains("sender='org.freedesktop.portal.Desktop'"));
        assert!(rule.contains("interface='org.freedesktop.portal.Request'"));
        assert!(rule.contains("member='Response'"));
    }
}
