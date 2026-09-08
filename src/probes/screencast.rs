#![allow(dead_code)] // Internal bounded slices precede the public command.

//! Internal, bounded `ScreenCast` lifecycle slices through
//! `OpenPipeWireRemote`.
//!
//! The module owns no public command: it validates the five-stage lifecycle,
//! releases the returned FD before `Session.Close`, and stops before any
//! `PipeWire` handshake, media access or capture-readiness claim.

use std::collections::HashMap;
use std::future::Future;
use std::os::fd::IntoRawFd;
use std::pin::Pin;
use std::time::Duration;

use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MessageStream, Proxy};

use crate::collectors::timeouts::{
    ACTIVE_PROBE_REQUEST, ACTIVE_PROBE_REQUEST_RECOVERY, ACTIVE_PROBE_SETUP,
};
use crate::error::Error;
use crate::model::probe::{
    CleanupResource, CleanupResult, CleanupStatus, ProbeKind, ProbeResult, ProbeStage, ProbeStatus,
};
use crate::probes::portal::{
    HandleTokenError, INTROSPECTABLE_INTERFACE, ResponseWait, bounded_proxy, classify_error,
    cleanup_after_response_wait, close_request, object_already_gone, open_session,
    request_metadata, request_options, response_match_rule, session_metadata,
    valid_request_path_for_sender, valid_session_path_for_sender, wait_for_response_with_cancel,
};

const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
const SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";
const WINDOW_SOURCE: u32 = 2;
/// Explicit headless parent policy: no desktop/window identifier is sent.
const HEADLESS_PARENT_WINDOW: &str = "";

/// Test-only shorter budgets keep the controlled matrix bounded without
/// weakening the production central timeout policy.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CreateSessionTimeouts {
    request: Duration,
    recovery: Duration,
    response: Duration,
}

impl CreateSessionTimeouts {
    const fn production() -> Self {
        Self {
            request: ACTIVE_PROBE_REQUEST,
            recovery: ACTIVE_PROBE_REQUEST_RECOVERY,
            response: crate::collectors::timeouts::ACTIVE_PROBE_RESPONSE,
        }
    }

    #[cfg(test)]
    const fn controlled() -> Self {
        Self {
            request: Duration::from_millis(40),
            recovery: Duration::from_millis(100),
            response: Duration::from_millis(120),
        }
    }
}

/// Internal adapter entry point. It is intentionally not wired to a public
/// command until the complete `ScreenCast` lifecycle exists.
pub(crate) fn run_create_session() -> Result<ProbeResult, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::ProbeRuntime(error.to_string()))?;
    Ok(runtime.block_on(run_create_session_with(
        || async {
            let _ = tokio::signal::ctrl_c().await;
        },
        CreateSessionTimeouts::production(),
    )))
}

/// Internal adapter entry point for the bounded `CreateSession` plus
/// `SelectSources` slice. It is intentionally not wired to a public command.
pub(crate) fn run_select_sources() -> Result<ProbeResult, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::ProbeRuntime(error.to_string()))?;
    Ok(runtime.block_on(run_select_sources_with(
        || async {
            let _ = tokio::signal::ctrl_c().await;
        },
        CreateSessionTimeouts::production(),
    )))
}

/// Internal adapter entry point for the bounded
/// `CreateSession` + `SelectSources` + `Start` slice. It is intentionally not
/// wired to a public command and does not decode the `Start` stream payload.
pub(crate) fn run_start() -> Result<ProbeResult, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::ProbeRuntime(error.to_string()))?;
    Ok(runtime.block_on(run_start_with(
        || async {
            let _ = tokio::signal::ctrl_c().await;
        },
        CreateSessionTimeouts::production(),
    )))
}

/// Internal adapter entry point for the bounded
/// `CreateSession` + `SelectSources` + `Start` + `StreamsReturned` slice. It
/// is intentionally not wired to a public command and does not open media.
pub(crate) fn run_streams_returned() -> Result<ProbeResult, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::ProbeRuntime(error.to_string()))?;
    Ok(runtime.block_on(run_streams_returned_with(
        || async {
            let _ = tokio::signal::ctrl_c().await;
        },
        CreateSessionTimeouts::production(),
    )))
}

/// Internal adapter entry point for the bounded
/// `CreateSession` + `SelectSources` + `Start` + `StreamsReturned` +
/// `OpenPipeWireRemote` slice. It is intentionally not wired to a public
/// command, does not connect to `PipeWire` and does not read media.
pub(crate) fn run_open_pipe_wire_remote() -> Result<ProbeResult, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::ProbeRuntime(error.to_string()))?;
    Ok(runtime.block_on(run_open_pipe_wire_remote_with(
        || async {
            let _ = tokio::signal::ctrl_c().await;
        },
        CreateSessionTimeouts::production(),
    )))
}

#[allow(clippy::too_many_lines)] // The bounded state machine is kept in lifecycle order.
async fn run_create_session_with<CFactory, CFuture>(
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
) -> ProbeResult
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
{
    let connection = match open_session().await {
        Ok(connection) => connection,
        Err(outcome) => return result(ProbeStage::Prepare, outcome, CleanupResult::not_required()),
    };

    if let Err(outcome) = inspect_screencast(&connection).await {
        return result(ProbeStage::Prepare, outcome, CleanupResult::not_required());
    }

    // Install the Response match before CreateSession to preserve the XDG
    // response-race invariant.
    let mut response_stream =
        match MessageStream::for_match_rule(response_match_rule(), &connection, Some(4)).await {
            Ok(stream) => stream,
            Err(error) => {
                return result(
                    ProbeStage::CreateSession,
                    classify_error(&error),
                    CleanupResult::not_required(),
                );
            }
        };

    let screencast = match bounded_proxy(&connection, SCREENCAST_INTERFACE).await {
        Ok(proxy) => proxy,
        Err(outcome) => {
            return result(
                ProbeStage::CreateSession,
                outcome,
                CleanupResult::not_required(),
            );
        }
    };
    let (handle_token, expected_request_path) = match request_metadata(&connection) {
        Ok(metadata) => metadata,
        Err(HandleTokenError::EntropyUnavailable | HandleTokenError::UniqueNameUnavailable) => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::InfrastructureFailure,
                CleanupResult::not_required(),
            );
        }
    };
    let (session_handle_token, expected_session_path) = match session_metadata(&connection) {
        Ok(metadata) => metadata,
        Err(HandleTokenError::EntropyUnavailable | HandleTokenError::UniqueNameUnavailable) => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::InfrastructureFailure,
                CleanupResult::not_required(),
            );
        }
    };
    let options = create_session_options(&handle_token, &session_handle_token);
    let create_session = screencast.call::<_, _, OwnedObjectPath>("CreateSession", &options);
    tokio::pin!(create_session);
    let cancellation = cancel_factory();
    tokio::pin!(cancellation);
    let request_call = tokio::select! {
        reply = &mut create_session => RequestCall::Reply(reply),
        () = &mut cancellation => RequestCall::UserCancelled,
        () = tokio::time::sleep(timeouts.request) => RequestCall::TimedOut,
    };

    let request_path = match request_call {
        RequestCall::Reply(Ok(path))
            if valid_request_path_for_sender(&path, &expected_request_path) =>
        {
            path
        }
        RequestCall::Reply(Ok(_)) => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::MalformedResponse,
                close_request(&connection, expected_request_path.as_str(), false).await,
            );
        }
        RequestCall::Reply(Err(error)) => {
            let status = classify_error(&error);
            let cleanup =
                cleanup_for_possible_request(&connection, &expected_request_path, status).await;
            return result(ProbeStage::CreateSession, status, cleanup);
        }
        RequestCall::UserCancelled => {
            return recover_interrupted_request(
                &connection,
                create_session.as_mut(),
                &expected_request_path,
                ProbeStatus::UserCancelled,
                timeouts.recovery,
            )
            .await;
        }
        RequestCall::TimedOut => {
            return recover_interrupted_request(
                &connection,
                create_session.as_mut(),
                &expected_request_path,
                ProbeStatus::TimedOut,
                timeouts.recovery,
            )
            .await;
        }
    };

    let response = wait_for_response_with_cancel(
        &mut response_stream,
        request_path.as_str(),
        cancel_factory(),
        timeouts.response,
    )
    .await;
    finalize_response(
        &connection,
        request_path.as_str(),
        response,
        &expected_session_path,
    )
    .await
}

/// Run the bounded lifecycle through `SelectSources`, then release the owned
/// Session. The Session cleanup is deliberately performed for every path after
/// a valid `CreateSession` response, including capability rejection and every
/// `SelectSources` request/response outcome.
#[allow(clippy::too_many_lines)]
async fn run_select_sources_with<CFactory, CFuture>(
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
) -> ProbeResult
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
{
    run_screencast_with(cancel_factory, timeouts, ProbeStage::SelectSources).await
}

/// Run the same bounded lifecycle through `Start`, then release the owned
/// Session. The stop stage is internal-only so the earlier
/// CreateSession/SelectSources controlled matrix remains an independently
/// testable boundary.
#[allow(clippy::too_many_lines)]
async fn run_start_with<CFactory, CFuture>(
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
) -> ProbeResult
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
{
    run_screencast_with(cancel_factory, timeouts, ProbeStage::Start).await
}

async fn run_streams_returned_with<CFactory, CFuture>(
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
) -> ProbeResult
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
{
    run_screencast_with(cancel_factory, timeouts, ProbeStage::StreamsReturned).await
}

async fn run_open_pipe_wire_remote_with<CFactory, CFuture>(
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
) -> ProbeResult
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
{
    run_screencast_with(cancel_factory, timeouts, ProbeStage::OpenPipeWireRemote).await
}

#[allow(clippy::too_many_lines)]
async fn run_screencast_with<CFactory, CFuture>(
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
    stop_at: ProbeStage,
) -> ProbeResult
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
{
    run_screencast_with_fd(cancel_factory, timeouts, stop_at, close_owned_pipewire_fd).await
}

#[allow(clippy::too_many_lines)]
async fn run_screencast_with_fd<CFactory, CFuture, FCloser>(
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
    stop_at: ProbeStage,
    fd_closer: FCloser,
) -> ProbeResult
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
    FCloser: Fn(OwnedFd) -> CleanupResult,
{
    let connection = match open_session().await {
        Ok(connection) => connection,
        Err(outcome) => return result(ProbeStage::Prepare, outcome, CleanupResult::not_required()),
    };

    if let Err(outcome) = inspect_screencast_for_select_sources(&connection).await {
        return result(ProbeStage::Prepare, outcome, CleanupResult::not_required());
    }

    // One response match covers all three request-producing methods and
    // is installed before CreateSession, preserving the first response race
    // boundary for the entire slice.
    let mut response_stream =
        match MessageStream::for_match_rule(response_match_rule(), &connection, Some(4)).await {
            Ok(stream) => stream,
            Err(error) => {
                return result(
                    ProbeStage::CreateSession,
                    classify_error(&error),
                    CleanupResult::not_required(),
                );
            }
        };
    let screencast = match bounded_proxy(&connection, SCREENCAST_INTERFACE).await {
        Ok(proxy) => proxy,
        Err(outcome) => {
            return result(
                ProbeStage::CreateSession,
                outcome,
                CleanupResult::not_required(),
            );
        }
    };
    let (handle_token, expected_request_path) = match request_metadata(&connection) {
        Ok(metadata) => metadata,
        Err(HandleTokenError::EntropyUnavailable | HandleTokenError::UniqueNameUnavailable) => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::InfrastructureFailure,
                CleanupResult::not_required(),
            );
        }
    };
    let (session_handle_token, expected_session_path) = match session_metadata(&connection) {
        Ok(metadata) => metadata,
        Err(HandleTokenError::EntropyUnavailable | HandleTokenError::UniqueNameUnavailable) => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::InfrastructureFailure,
                CleanupResult::not_required(),
            );
        }
    };
    let create_options = create_session_options(&handle_token, &session_handle_token);
    let create_session = screencast.call::<_, _, OwnedObjectPath>("CreateSession", &create_options);
    tokio::pin!(create_session);
    let cancellation = cancel_factory();
    tokio::pin!(cancellation);
    let request_call = tokio::select! {
        reply = &mut create_session => RequestCall::Reply(reply),
        () = &mut cancellation => RequestCall::UserCancelled,
        () = tokio::time::sleep(timeouts.request) => RequestCall::TimedOut,
    };

    let request_path = match request_call {
        RequestCall::Reply(Ok(path))
            if valid_request_path_for_sender(&path, &expected_request_path) =>
        {
            path
        }
        RequestCall::Reply(Ok(_)) => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::MalformedResponse,
                close_request(&connection, expected_request_path.as_str(), false).await,
            );
        }
        RequestCall::Reply(Err(error)) => {
            let status = classify_error(&error);
            let cleanup =
                cleanup_for_possible_request(&connection, &expected_request_path, status).await;
            return result(ProbeStage::CreateSession, status, cleanup);
        }
        RequestCall::UserCancelled => {
            let outcome = recover_interrupted_request_outcome(
                &connection,
                create_session.as_mut(),
                &expected_request_path,
                ProbeStatus::UserCancelled,
                timeouts.recovery,
            )
            .await;
            return result(ProbeStage::CreateSession, outcome.status, outcome.cleanup);
        }
        RequestCall::TimedOut => {
            let outcome = recover_interrupted_request_outcome(
                &connection,
                create_session.as_mut(),
                &expected_request_path,
                ProbeStatus::TimedOut,
                timeouts.recovery,
            )
            .await;
            return result(ProbeStage::CreateSession, outcome.status, outcome.cleanup);
        }
    };

    let create_response = wait_for_response_with_cancel(
        &mut response_stream,
        request_path.as_str(),
        cancel_factory(),
        timeouts.response,
    )
    .await;
    let create_request_cleanup =
        cleanup_after_response_wait(&connection, request_path.as_str(), &create_response).await;
    let session_path = match create_response {
        ResponseWait::Outcome { code: 0, results } => {
            match decode_session_handle(&results, &expected_session_path) {
                Ok(session_path) => session_path,
                Err(status) => {
                    let session_cleanup =
                        close_expected_session(&connection, &expected_session_path).await;
                    return result(ProbeStage::CreateSession, status, session_cleanup);
                }
            }
        }
        ResponseWait::Outcome { code, .. } => {
            let status = match code {
                1 => ProbeStatus::UserCancelled,
                2 => ProbeStatus::InfrastructureFailure,
                _ => ProbeStatus::MalformedResponse,
            };
            return result(ProbeStage::CreateSession, status, create_request_cleanup);
        }
        ResponseWait::MalformedResponse => {
            let session_cleanup = close_expected_session(&connection, &expected_session_path).await;
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::MalformedResponse,
                session_cleanup,
            );
        }
        ResponseWait::UserCancelled => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::UserCancelled,
                create_request_cleanup,
            );
        }
        ResponseWait::TimedOut => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::TimedOut,
                create_request_cleanup,
            );
        }
        ResponseWait::InfrastructureFailure => {
            return result(
                ProbeStage::CreateSession,
                ProbeStatus::InfrastructureFailure,
                create_request_cleanup,
            );
        }
    };

    let available_source_types = match read_available_source_types(&screencast).await {
        Ok(value) => value,
        Err(status) => {
            let session_cleanup = close_session(&connection, &session_path).await;
            return result(ProbeStage::SelectSources, status, session_cleanup);
        }
    };
    if available_source_types & WINDOW_SOURCE == 0 {
        let session_cleanup = close_session(&connection, &session_path).await;
        return result(
            ProbeStage::SelectSources,
            ProbeStatus::Unsupported,
            session_cleanup,
        );
    }

    let (select_handle_token, expected_select_request_path) = match request_metadata(&connection) {
        Ok(metadata) => metadata,
        Err(HandleTokenError::EntropyUnavailable | HandleTokenError::UniqueNameUnavailable) => {
            let session_cleanup = close_session(&connection, &session_path).await;
            return result(
                ProbeStage::SelectSources,
                ProbeStatus::InfrastructureFailure,
                session_cleanup,
            );
        }
    };
    let select_options = select_sources_options(&select_handle_token);
    let select_request_body = (session_path.clone(), select_options);
    let select_sources =
        screencast.call::<_, _, OwnedObjectPath>("SelectSources", &select_request_body);
    tokio::pin!(select_sources);
    let cancellation = cancel_factory();
    tokio::pin!(cancellation);
    let request_call = tokio::select! {
        reply = &mut select_sources => RequestCall::Reply(reply),
        () = &mut cancellation => RequestCall::UserCancelled,
        () = tokio::time::sleep(timeouts.request) => RequestCall::TimedOut,
    };

    let select_request_path = match request_call {
        RequestCall::Reply(Ok(path))
            if valid_request_path_for_sender(&path, &expected_select_request_path) =>
        {
            path
        }
        RequestCall::Reply(Ok(_)) => {
            let request_cleanup =
                close_request(&connection, expected_select_request_path.as_str(), false).await;
            let session_cleanup = close_session(&connection, &session_path).await;
            return result(
                ProbeStage::SelectSources,
                ProbeStatus::MalformedResponse,
                combine_cleanup(&request_cleanup, &session_cleanup),
            );
        }
        RequestCall::Reply(Err(error)) => {
            let status = classify_error(&error);
            let request_cleanup =
                cleanup_for_possible_request(&connection, &expected_select_request_path, status)
                    .await;
            let session_cleanup = close_session(&connection, &session_path).await;
            return result(
                ProbeStage::SelectSources,
                status,
                combine_cleanup(&request_cleanup, &session_cleanup),
            );
        }
        RequestCall::UserCancelled => {
            let outcome = recover_interrupted_request_outcome(
                &connection,
                select_sources.as_mut(),
                &expected_select_request_path,
                ProbeStatus::UserCancelled,
                timeouts.recovery,
            )
            .await;
            let session_cleanup = close_session(&connection, &session_path).await;
            return result(
                ProbeStage::SelectSources,
                outcome.status,
                combine_cleanup(&outcome.cleanup, &session_cleanup),
            );
        }
        RequestCall::TimedOut => {
            let outcome = recover_interrupted_request_outcome(
                &connection,
                select_sources.as_mut(),
                &expected_select_request_path,
                ProbeStatus::TimedOut,
                timeouts.recovery,
            )
            .await;
            let session_cleanup = close_session(&connection, &session_path).await;
            return result(
                ProbeStage::SelectSources,
                outcome.status,
                combine_cleanup(&outcome.cleanup, &session_cleanup),
            );
        }
    };

    let select_response = wait_for_response_with_cancel(
        &mut response_stream,
        select_request_path.as_str(),
        cancel_factory(),
        timeouts.response,
    )
    .await;
    let request_cleanup =
        cleanup_after_response_wait(&connection, select_request_path.as_str(), &select_response)
            .await;
    let outcome = select_sources_outcome(&select_response, request_cleanup);
    if stop_at != ProbeStage::SelectSources && matches!(outcome.status, ProbeStatus::Success) {
        let mut terminal_stage = ProbeStage::Start;
        let start_outcome = run_start_stage(
            &connection,
            &screencast,
            &mut response_stream,
            &session_path,
            cancel_factory,
            timeouts,
            stop_at,
            &mut terminal_stage,
            fd_closer,
        )
        .await;
        let session_cleanup = close_session(&connection, &session_path).await;
        return result(
            terminal_stage,
            start_outcome.status,
            combine_cleanup(&start_outcome.cleanup, &session_cleanup),
        );
    }
    let session_cleanup = close_session(&connection, &session_path).await;
    result(
        ProbeStage::SelectSources,
        outcome.status,
        combine_cleanup(&outcome.cleanup, &session_cleanup),
    )
}

#[derive(Debug)]
enum RequestCall {
    Reply(Result<OwnedObjectPath, zbus::Error>),
    UserCancelled,
    TimedOut,
}

enum DirectCall<T> {
    Reply(Result<T, zbus::Error>),
    UserCancelled,
    TimedOut,
}

#[derive(Debug)]
struct LifecycleOutcome {
    status: ProbeStatus,
    cleanup: CleanupResult,
}

async fn recover_interrupted_request<F>(
    connection: &Connection,
    create_session: Pin<&mut F>,
    expected_path: &OwnedObjectPath,
    status: ProbeStatus,
    recovery_timeout: Duration,
) -> ProbeResult
where
    F: Future<Output = Result<OwnedObjectPath, zbus::Error>>,
{
    let outcome = recover_interrupted_request_outcome(
        connection,
        create_session,
        expected_path,
        status,
        recovery_timeout,
    )
    .await;
    result(ProbeStage::CreateSession, outcome.status, outcome.cleanup)
}

async fn recover_interrupted_request_outcome<F>(
    connection: &Connection,
    request_call: Pin<&mut F>,
    expected_path: &OwnedObjectPath,
    status: ProbeStatus,
    recovery_timeout: Duration,
) -> LifecycleOutcome
where
    F: Future<Output = Result<OwnedObjectPath, zbus::Error>>,
{
    match tokio::time::timeout(recovery_timeout, request_call).await {
        Ok(Ok(path)) if valid_request_path_for_sender(&path, expected_path) => {
            let cleanup = close_request(connection, path.as_str(), true).await;
            LifecycleOutcome { status, cleanup }
        }
        Ok(Ok(_)) => {
            let cleanup = close_request(connection, expected_path.as_str(), false).await;
            LifecycleOutcome {
                status: ProbeStatus::MalformedResponse,
                cleanup,
            }
        }
        Ok(Err(_)) | Err(_) => {
            let cleanup = close_request(connection, expected_path.as_str(), false).await;
            LifecycleOutcome { status, cleanup }
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

async fn finalize_response(
    connection: &Connection,
    request_path: &str,
    response: ResponseWait,
    expected_session_path: &OwnedObjectPath,
) -> ProbeResult {
    let request_cleanup = cleanup_after_response_wait(connection, request_path, &response).await;
    match response {
        ResponseWait::Outcome { code, results } => match code {
            0 => match decode_session_handle(&results, expected_session_path) {
                Ok(session_path) => {
                    let session_cleanup = close_session(connection, &session_path).await;
                    result(
                        ProbeStage::CreateSession,
                        ProbeStatus::Success,
                        session_cleanup,
                    )
                }
                Err(status) => {
                    // A terminal success response may have created the
                    // Session even when its payload is unusable. Never use a
                    // foreign path from the malformed payload; only the
                    // token-derived expected path is safe to probe for
                    // best-effort cleanup.
                    let session_cleanup =
                        close_expected_session(connection, expected_session_path).await;
                    result(ProbeStage::CreateSession, status, session_cleanup)
                }
            },
            1 => result(
                ProbeStage::CreateSession,
                ProbeStatus::UserCancelled,
                request_cleanup,
            ),
            2 => result(
                ProbeStage::CreateSession,
                ProbeStatus::InfrastructureFailure,
                request_cleanup,
            ),
            _ => result(
                ProbeStage::CreateSession,
                ProbeStatus::MalformedResponse,
                request_cleanup,
            ),
        },
        ResponseWait::UserCancelled => result(
            ProbeStage::CreateSession,
            ProbeStatus::UserCancelled,
            request_cleanup,
        ),
        ResponseWait::TimedOut => result(
            ProbeStage::CreateSession,
            ProbeStatus::TimedOut,
            request_cleanup,
        ),
        ResponseWait::InfrastructureFailure => result(
            ProbeStage::CreateSession,
            ProbeStatus::InfrastructureFailure,
            request_cleanup,
        ),
        ResponseWait::MalformedResponse => {
            // The Response signal is terminal even when its body cannot be
            // decoded. The provider may nevertheless have created the
            // Session, so apply the same expected-path-only cleanup rule as a
            // malformed successful payload. Request.Close remains forbidden.
            let session_cleanup = close_expected_session(connection, expected_session_path).await;
            result(
                ProbeStage::CreateSession,
                ProbeStatus::MalformedResponse,
                session_cleanup,
            )
        }
    }
}

fn decode_session_handle(
    results: &HashMap<String, OwnedValue>,
    expected_session_path: &OwnedObjectPath,
) -> Result<OwnedObjectPath, ProbeStatus> {
    let Some(value) = results.get("session_handle") else {
        return Err(ProbeStatus::MalformedResponse);
    };
    if value.value_signature().to_string() != "s" {
        return Err(ProbeStatus::MalformedResponse);
    }
    let Ok(path) = String::try_from(&**value) else {
        return Err(ProbeStatus::MalformedResponse);
    };
    let Ok(path) = OwnedObjectPath::try_from(path) else {
        return Err(ProbeStatus::MalformedResponse);
    };
    if valid_session_path_for_sender(path.as_str(), expected_session_path) {
        Ok(path)
    } else {
        Err(ProbeStatus::MalformedResponse)
    }
}

async fn close_session(connection: &Connection, session_path: &OwnedObjectPath) -> CleanupResult {
    close_session_with_ownership(connection, session_path, true).await
}

/// Best-effort cleanup for a Session that may have been created by a terminal
/// success response whose returned handle was unusable. The expected path is
/// derived from our sender and OS-random session token; a returned foreign
/// path is never passed here. Failure to observe the expected object is
/// unverified ownership, not proof that no Session existed.
async fn close_expected_session(
    connection: &Connection,
    session_path: &OwnedObjectPath,
) -> CleanupResult {
    close_session_with_ownership(connection, session_path, false).await
}

async fn close_session_with_ownership(
    connection: &Connection,
    session_path: &OwnedObjectPath,
    ownership_proven: bool,
) -> CleanupResult {
    let Ok(Ok(proxy)) = tokio::time::timeout(
        crate::collectors::timeouts::ACTIVE_PROBE_CLEANUP,
        Proxy::new(
            connection,
            crate::probes::portal::PORTAL_DESTINATION,
            session_path,
            SESSION_INTERFACE,
        ),
    )
    .await
    else {
        return if ownership_proven {
            failed_session_cleanup()
        } else {
            unverified_session_cleanup()
        };
    };
    match tokio::time::timeout(
        crate::collectors::timeouts::ACTIVE_PROBE_CLEANUP,
        proxy.call::<_, _, ()>("Close", &()),
    )
    .await
    {
        Ok(Ok(())) => CleanupResult::completed(),
        Ok(Err(error)) if object_already_gone(&error) => unverified_session_cleanup(),
        Ok(Err(_)) => failed_session_cleanup(),
        Err(_) if ownership_proven => failed_session_cleanup(),
        Err(_) => unverified_session_cleanup(),
    }
}

fn failed_session_cleanup() -> CleanupResult {
    CleanupResult::failed(vec![CleanupResource::Session])
        .expect("a ScreenCast session cleanup resource is valid")
}

fn unverified_session_cleanup() -> CleanupResult {
    CleanupResult::unverified_resources(vec![CleanupResource::Session])
        .expect("a ScreenCast session cleanup resource is valid")
}

fn create_session_options(
    handle_token: &str,
    session_handle_token: &str,
) -> HashMap<String, OwnedValue> {
    let mut options = request_options(handle_token);
    options.insert(
        "session_handle_token".to_owned(),
        OwnedValue::try_from(Value::from(session_handle_token.to_owned()))
            .expect("a session token is a valid D-Bus string"),
    );
    options
}

async fn inspect_screencast(connection: &Connection) -> Result<(), ProbeStatus> {
    let introspection = bounded_proxy(connection, INTROSPECTABLE_INTERFACE).await?;
    let xml: String =
        match tokio::time::timeout(ACTIVE_PROBE_SETUP, introspection.call("Introspect", &())).await
        {
            Ok(Ok(xml)) => xml,
            Ok(Err(error)) => return Err(classify_error(&error)),
            Err(_) => return Err(ProbeStatus::TimedOut),
        };
    if introspection_supports_create_session(&xml) {
        Ok(())
    } else {
        Err(ProbeStatus::Unsupported)
    }
}

pub(crate) fn introspection_supports_create_session(xml: &str) -> bool {
    let Some(interface_start) = xml.find("<interface name=\"org.freedesktop.portal.ScreenCast\"")
    else {
        return false;
    };
    let interface = &xml[interface_start..];
    let Some(interface_end) = interface.find("</interface>") else {
        return false;
    };
    interface[..interface_end]
        .lines()
        .any(|line| line.contains("<method ") && line.contains("name=\"CreateSession\""))
}

pub(crate) fn introspection_supports_select_sources(xml: &str) -> bool {
    let Some(interface_start) = xml.find("<interface name=\"org.freedesktop.portal.ScreenCast\"")
    else {
        return false;
    };
    let interface = &xml[interface_start..];
    let Some(interface_end) = interface.find("</interface>") else {
        return false;
    };
    let interface = &interface[..interface_end];
    interface
        .lines()
        .any(|line| line.contains("<method ") && line.contains("name=\"SelectSources\""))
        && interface.lines().any(|line| {
            line.contains("<property ")
                && line.contains("name=\"AvailableSourceTypes\"")
                && line.contains("type=\"u\"")
        })
}

async fn inspect_screencast_for_select_sources(connection: &Connection) -> Result<(), ProbeStatus> {
    let introspection = bounded_proxy(connection, INTROSPECTABLE_INTERFACE).await?;
    let xml: String =
        match tokio::time::timeout(ACTIVE_PROBE_SETUP, introspection.call("Introspect", &())).await
        {
            Ok(Ok(xml)) => xml,
            Ok(Err(error)) => return Err(classify_error(&error)),
            Err(_) => return Err(ProbeStatus::TimedOut),
        };
    if introspection_supports_select_sources(&xml) {
        Ok(())
    } else {
        Err(ProbeStatus::Unsupported)
    }
}

async fn read_available_source_types(proxy: &Proxy<'_>) -> Result<u32, ProbeStatus> {
    match tokio::time::timeout(
        ACTIVE_PROBE_SETUP,
        proxy.get_property::<u32>("AvailableSourceTypes"),
    )
    .await
    {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(classify_error(&error)),
        Err(_) => Err(ProbeStatus::TimedOut),
    }
}

fn select_sources_options(handle_token: &str) -> HashMap<String, OwnedValue> {
    let mut options = request_options(handle_token);
    options.insert(
        "types".to_owned(),
        OwnedValue::try_from(Value::from(WINDOW_SOURCE))
            .expect("a source type bitmask is a valid D-Bus value"),
    );
    options.insert(
        "multiple".to_owned(),
        OwnedValue::try_from(Value::from(false)).expect("a boolean is a valid D-Bus value"),
    );
    options
}

fn select_sources_outcome(
    response: &ResponseWait,
    request_cleanup: CleanupResult,
) -> LifecycleOutcome {
    let status = match response {
        ResponseWait::Outcome { code: 0, .. } => ProbeStatus::Success,
        ResponseWait::Outcome { code: 1, .. } | ResponseWait::UserCancelled => {
            ProbeStatus::UserCancelled
        }
        ResponseWait::Outcome { code: 2, .. } | ResponseWait::InfrastructureFailure => {
            ProbeStatus::InfrastructureFailure
        }
        ResponseWait::Outcome { .. } | ResponseWait::MalformedResponse => {
            ProbeStatus::MalformedResponse
        }
        ResponseWait::TimedOut => ProbeStatus::TimedOut,
    };
    LifecycleOutcome {
        status,
        cleanup: request_cleanup,
    }
}

/// Run only the `Start` request on an already selected, owned Session.
///
/// The Start-only boundary ignores successful results. The optional
/// `StreamsReturned` boundary validates their structure without exposing
/// stream metadata or node IDs.
#[allow(clippy::too_many_arguments)]
async fn run_start_stage<CFactory, CFuture>(
    connection: &Connection,
    screencast: &Proxy<'_>,
    response_stream: &mut MessageStream,
    session_path: &OwnedObjectPath,
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
    stop_at: ProbeStage,
    terminal_stage: &mut ProbeStage,
    fd_closer: impl Fn(OwnedFd) -> CleanupResult,
) -> LifecycleOutcome
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
{
    let (handle_token, expected_request_path) = match request_metadata(connection) {
        Ok(metadata) => metadata,
        Err(HandleTokenError::EntropyUnavailable | HandleTokenError::UniqueNameUnavailable) => {
            return LifecycleOutcome {
                status: ProbeStatus::InfrastructureFailure,
                cleanup: CleanupResult::not_required(),
            };
        }
    };
    let options = start_options(&handle_token);
    let start_body = (
        session_path.clone(),
        HEADLESS_PARENT_WINDOW.to_owned(),
        options,
    );
    let start = screencast.call::<_, _, OwnedObjectPath>("Start", &start_body);
    tokio::pin!(start);
    let cancellation = cancel_factory();
    tokio::pin!(cancellation);
    let request_call = tokio::select! {
        reply = &mut start => RequestCall::Reply(reply),
        () = &mut cancellation => RequestCall::UserCancelled,
        () = tokio::time::sleep(timeouts.request) => RequestCall::TimedOut,
    };

    let request_path = match request_call {
        RequestCall::Reply(Ok(path))
            if valid_request_path_for_sender(&path, &expected_request_path) =>
        {
            path
        }
        RequestCall::Reply(Ok(_)) => {
            return LifecycleOutcome {
                status: ProbeStatus::MalformedResponse,
                cleanup: close_request(connection, expected_request_path.as_str(), false).await,
            };
        }
        RequestCall::Reply(Err(error)) => {
            let status = classify_error(&error);
            let cleanup =
                cleanup_for_possible_request(connection, &expected_request_path, status).await;
            return LifecycleOutcome { status, cleanup };
        }
        RequestCall::UserCancelled => {
            return recover_interrupted_request_outcome(
                connection,
                start.as_mut(),
                &expected_request_path,
                ProbeStatus::UserCancelled,
                timeouts.recovery,
            )
            .await;
        }
        RequestCall::TimedOut => {
            return recover_interrupted_request_outcome(
                connection,
                start.as_mut(),
                &expected_request_path,
                ProbeStatus::TimedOut,
                timeouts.recovery,
            )
            .await;
        }
    };

    let response = wait_for_response_with_cancel(
        response_stream,
        request_path.as_str(),
        cancel_factory(),
        timeouts.response,
    )
    .await;
    let request_cleanup =
        cleanup_after_response_wait(connection, request_path.as_str(), &response).await;
    let mut outcome = start_outcome(&response, request_cleanup);
    if matches!(
        stop_at,
        ProbeStage::StreamsReturned | ProbeStage::OpenPipeWireRemote
    ) && let ResponseWait::Outcome { code: 0, results } = &response
    {
        *terminal_stage = ProbeStage::StreamsReturned;
        if let Err(status) = validate_stream_container(results) {
            outcome.status = status;
        } else if stop_at == ProbeStage::OpenPipeWireRemote {
            *terminal_stage = ProbeStage::OpenPipeWireRemote;
            return run_open_pipe_wire_remote_stage(
                screencast,
                session_path,
                cancel_factory,
                timeouts,
                fd_closer,
            )
            .await;
        }
    }
    outcome
}

/// Call the direct-FD `ScreenCast` method. Unlike the earlier stages this call
/// creates no XDG Request object, so there is no response match and no
/// `Request.Close` path. A timeout or cancellation retains the method future
/// for bounded recovery so a late owned FD can still be closed before the
/// Session is released.
async fn run_open_pipe_wire_remote_stage<CFactory, CFuture, FCloser>(
    screencast: &Proxy<'_>,
    session_path: &OwnedObjectPath,
    cancel_factory: CFactory,
    timeouts: CreateSessionTimeouts,
    fd_closer: FCloser,
) -> LifecycleOutcome
where
    CFactory: Fn() -> CFuture + Copy,
    CFuture: Future<Output = ()>,
    FCloser: Fn(OwnedFd) -> CleanupResult,
{
    let options = open_pipe_wire_remote_options();
    let body = (session_path.clone(), options);
    let open = screencast.call::<_, _, OwnedFd>("OpenPipeWireRemote", &body);
    tokio::pin!(open);
    let cancellation = cancel_factory();
    tokio::pin!(cancellation);
    let call = tokio::select! {
        reply = &mut open => DirectCall::Reply(reply),
        () = &mut cancellation => DirectCall::UserCancelled,
        () = tokio::time::sleep(timeouts.request) => DirectCall::TimedOut,
    };

    match call {
        DirectCall::Reply(Ok(fd)) => LifecycleOutcome {
            status: ProbeStatus::Success,
            cleanup: fd_closer(fd),
        },
        DirectCall::Reply(Err(error)) => open_pipe_wire_error_outcome(&error),
        DirectCall::UserCancelled => {
            recover_interrupted_pipe_wire_remote(
                open.as_mut(),
                ProbeStatus::UserCancelled,
                timeouts.recovery,
                fd_closer,
            )
            .await
        }
        DirectCall::TimedOut => {
            recover_interrupted_pipe_wire_remote(
                open.as_mut(),
                ProbeStatus::TimedOut,
                timeouts.recovery,
                fd_closer,
            )
            .await
        }
    }
}

async fn recover_interrupted_pipe_wire_remote<F, FCloser>(
    open: Pin<&mut F>,
    status: ProbeStatus,
    recovery_timeout: Duration,
    fd_closer: FCloser,
) -> LifecycleOutcome
where
    F: Future<Output = Result<OwnedFd, zbus::Error>>,
    FCloser: Fn(OwnedFd) -> CleanupResult,
{
    match tokio::time::timeout(recovery_timeout, open).await {
        Ok(Ok(fd)) => LifecycleOutcome {
            status,
            cleanup: fd_closer(fd),
        },
        Ok(Err(error)) => LifecycleOutcome {
            status,
            cleanup: open_pipe_wire_error_cleanup(&error),
        },
        Err(_) => LifecycleOutcome {
            status,
            cleanup: unverified_pipe_wire_cleanup(),
        },
    }
}

fn open_pipe_wire_error_outcome(error: &zbus::Error) -> LifecycleOutcome {
    let status = match error {
        zbus::Error::InvalidReply | zbus::Error::Variant(_) => ProbeStatus::MalformedResponse,
        _ => classify_error(error),
    };
    LifecycleOutcome {
        status,
        cleanup: open_pipe_wire_error_cleanup(error),
    }
}

fn open_pipe_wire_error_cleanup(error: &zbus::Error) -> CleanupResult {
    match error {
        // A terminal D-Bus error reply proves that no FD was returned.
        zbus::Error::MethodError(..)
        | zbus::Error::FDO(..)
        | zbus::Error::InterfaceNotFound
        | zbus::Error::Unsupported => CleanupResult::not_required(),
        // Type/reply failures and transport errors do not prove FD ownership
        // was absent, so the result stays fail-closed and unverified.
        _ => unverified_pipe_wire_cleanup(),
    }
}

fn open_pipe_wire_remote_options() -> HashMap<String, OwnedValue> {
    HashMap::new()
}

fn close_owned_pipewire_fd(fd: OwnedFd) -> CleanupResult {
    let owned_fd: std::os::fd::OwnedFd = fd.into();
    let raw_fd = owned_fd.into_raw_fd();
    // Ownership was transferred out of OwnedFd exactly once immediately
    // before this close. The raw descriptor never crosses an API boundary.
    // SAFETY: `raw_fd` is the unique descriptor extracted from `OwnedFd`, and
    // ownership is consumed exactly once by this close operation.
    if unsafe { libc::close(raw_fd) } == 0 {
        CleanupResult::completed()
    } else {
        failed_pipe_wire_cleanup()
    }
}

fn failed_pipe_wire_cleanup() -> CleanupResult {
    CleanupResult::failed(vec![CleanupResource::PipeWireRemote])
        .expect("a PipeWire remote cleanup resource is valid")
}

fn unverified_pipe_wire_cleanup() -> CleanupResult {
    CleanupResult::unverified_resources(vec![CleanupResource::PipeWireRemote])
        .expect("a PipeWire remote cleanup resource is valid")
}

/// Validate the XDG a(ua{sv}) container without extracting or retaining node IDs.
/// Optional metadata stays opaque, except an advertised `source_type` must agree
/// with our Window-only policy. Older providers may omit that property.
fn validate_stream_container(results: &HashMap<String, OwnedValue>) -> Result<(), ProbeStatus> {
    let malformed = ProbeStatus::MalformedResponse;
    let value = results.get("streams").ok_or(malformed)?;
    if value.value_signature().to_string() != "a(ua{sv})" {
        return Err(malformed);
    }
    let Value::Array(streams) = &**value else {
        return Err(malformed);
    };
    if streams.len() != 1 {
        return Err(malformed);
    }
    let Value::Structure(stream) = &streams.inner()[0] else {
        return Err(malformed);
    };
    let [Value::U32(_), Value::Dict(properties)] = stream.fields() else {
        return Err(malformed);
    };
    for (key, value) in properties.iter() {
        if matches!(key, Value::Str(name) if name.as_str() == "source_type") {
            let Value::Value(source) = value else {
                return Err(malformed);
            };
            if !matches!(&**source, Value::U32(WINDOW_SOURCE)) {
                return Err(malformed);
            }
        }
    }
    Ok(())
}

fn start_options(handle_token: &str) -> HashMap<String, OwnedValue> {
    request_options(handle_token)
}

fn start_outcome(response: &ResponseWait, request_cleanup: CleanupResult) -> LifecycleOutcome {
    let status = match response {
        // Do not inspect the results map here. The optional `StreamsReturned`
        // boundary owns that validation separately from Start-only mapping.
        ResponseWait::Outcome { code: 0, .. } => ProbeStatus::Success,
        ResponseWait::Outcome { code: 1, .. } | ResponseWait::UserCancelled => {
            ProbeStatus::UserCancelled
        }
        ResponseWait::Outcome { code: 2, .. } | ResponseWait::InfrastructureFailure => {
            ProbeStatus::InfrastructureFailure
        }
        ResponseWait::Outcome { .. } | ResponseWait::MalformedResponse => {
            ProbeStatus::MalformedResponse
        }
        ResponseWait::TimedOut => ProbeStatus::TimedOut,
    };
    LifecycleOutcome {
        status,
        cleanup: request_cleanup,
    }
}

fn combine_cleanup(first: &CleanupResult, second: &CleanupResult) -> CleanupResult {
    let mut resources = first.failed_resources().to_vec();
    for resource in second.failed_resources() {
        if !resources.contains(resource) {
            resources.push(*resource);
        }
    }
    if matches!(first.status(), CleanupStatus::Failed)
        || matches!(second.status(), CleanupStatus::Failed)
    {
        return CleanupResult::failed(resources)
            .expect("failed cleanup must retain at least one resource");
    }
    if matches!(first.status(), CleanupStatus::Unverified)
        || matches!(second.status(), CleanupStatus::Unverified)
    {
        return if resources.is_empty() {
            CleanupResult::unverified()
        } else {
            CleanupResult::unverified_resources(resources)
                .expect("combined cleanup resources must be unique")
        };
    }
    if matches!(first.status(), CleanupStatus::Completed)
        || matches!(second.status(), CleanupStatus::Completed)
    {
        CleanupResult::completed()
    } else {
        CleanupResult::not_required()
    }
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
    ProbeResult::new(ProbeKind::ScreenCast, stage, status, cleanup)
        .expect("ScreenCast bounded lifecycle must emit a valid ProbeResult v1")
}

#[cfg(test)]
mod tests {
    use super::{
        CreateSessionTimeouts, HEADLESS_PARENT_WINDOW, create_session_options,
        decode_session_handle, introspection_supports_create_session,
        introspection_supports_select_sources, run_create_session_with, run_screencast_with_fd,
        run_select_sources_with, run_start_with, select_sources_options, start_options,
    };
    use crate::model::probe::{
        CleanupResource, CleanupResult, CleanupStatus, ProbeStage, ProbeStatus,
    };
    use crate::probes::portal::session_path_for_sender;
    use serde_json::Value as JsonValue;
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use zbus::zvariant::{OwnedFd, OwnedValue, Value as ZValue};

    #[test]
    fn introspection_requires_the_screencast_create_session_method() {
        let xml = r#"
            <node>
              <interface name="org.freedesktop.portal.ScreenCast">
                <method name="CreateSession"/>
              </interface>
            </node>
        "#;
        assert!(introspection_supports_create_session(xml));
        assert!(!introspection_supports_create_session(
            r#"<interface name="org.freedesktop.portal.ScreenCast"><method name="Start"/></interface>"#
        ));
        assert!(!introspection_supports_create_session(
            r#"<interface name="org.freedesktop.portal.Screenshot"><method name="CreateSession"/></interface>"#
        ));
    }

    #[test]
    fn introspection_requires_select_sources_and_source_capability_property() {
        let xml = r#"
            <node>
              <interface name="org.freedesktop.portal.ScreenCast">
                <property name="AvailableSourceTypes" type="u" access="read"/>
                <method name="SelectSources"/>
              </interface>
            </node>
        "#;
        assert!(introspection_supports_select_sources(xml));
        assert!(!introspection_supports_select_sources(
            r#"<interface name="org.freedesktop.portal.ScreenCast"><method name="SelectSources"/></interface>"#
        ));
        assert!(!introspection_supports_select_sources(
            r#"<interface name="org.freedesktop.portal.ScreenCast"><property name="AvailableSourceTypes" type="u" access="read"/></interface>"#
        ));
    }

    #[test]
    fn create_session_options_contain_two_distinct_private_tokens() {
        let options = create_session_options("request-token", "session-token");
        assert_eq!(
            String::try_from(&**options.get("handle_token").unwrap()).unwrap(),
            "request-token"
        );
        assert_eq!(
            String::try_from(&**options.get("session_handle_token").unwrap()).unwrap(),
            "session-token"
        );
        assert_ne!(
            options.get("handle_token").unwrap(),
            options.get("session_handle_token").unwrap()
        );
    }

    #[test]
    fn select_sources_options_are_window_only_single_source_and_non_persistent() {
        let options = select_sources_options("select-token");
        assert_eq!(options.len(), 3);
        assert_eq!(
            options
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            ["handle_token", "multiple", "types"]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
        assert_eq!(u32::try_from(&**options.get("types").unwrap()).unwrap(), 2);
        assert!(!bool::try_from(&**options.get("multiple").unwrap()).unwrap());
        assert!(!options.contains_key("persist_mode"));
        assert!(!options.contains_key("restore_token"));
    }

    #[test]
    fn start_options_are_only_a_fresh_request_token_and_headless_parent_is_explicit() {
        let options = start_options("start-token");
        assert_eq!(options.len(), 1);
        assert_eq!(
            options
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            ["handle_token"]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
        assert_eq!(
            String::try_from(&**options.get("handle_token").unwrap()).unwrap(),
            "start-token"
        );
        assert_eq!(HEADLESS_PARENT_WINDOW, "");
    }

    #[test]
    fn open_pipe_wire_remote_options_are_empty_and_have_no_token() {
        let options = super::open_pipe_wire_remote_options();
        assert!(options.is_empty());
        assert!(!options.contains_key("handle_token"));
        assert!(!options.contains_key("session_handle_token"));
    }

    #[test]
    fn session_handle_requires_a_sender_bound_string_object_path() {
        let expected = session_path_for_sender(":1.42", "expected").unwrap();
        let valid = OwnedValue::try_from(ZValue::from(
            "/org/freedesktop/portal/desktop/session/1_42/actual",
        ))
        .unwrap();
        let mut results = std::collections::HashMap::new();
        results.insert("session_handle".to_owned(), valid);
        assert!(decode_session_handle(&results, &expected).is_ok());

        let wrong_type = OwnedValue::try_from(ZValue::from(7_u32)).unwrap();
        results.insert("session_handle".to_owned(), wrong_type);
        assert_eq!(
            decode_session_handle(&results, &expected),
            Err(ProbeStatus::MalformedResponse)
        );

        let unrelated = OwnedValue::try_from(ZValue::from(
            "/org/freedesktop/portal/desktop/session/1_43/other",
        ))
        .unwrap();
        results.insert("session_handle".to_owned(), unrelated);
        assert_eq!(
            decode_session_handle(&results, &expected),
            Err(ProbeStatus::MalformedResponse)
        );
    }

    #[test]
    fn create_session_cleanup_failure_is_independent_and_fail_closed() {
        let result = super::result(
            ProbeStage::CreateSession,
            ProbeStatus::Success,
            CleanupResult::failed(vec![CleanupResource::Session]).unwrap(),
        );
        assert_eq!(result.stage(), ProbeStage::Cleanup);
        assert_eq!(result.cleanup().status(), CleanupStatus::Failed);
        assert!(!result.is_clean_success());
    }

    #[derive(Clone, Copy)]
    struct MatrixCase {
        mode: &'static str,
        status: &'static str,
        cleanup: &'static str,
        stage: &'static str,
        request_closes: u64,
        session_closes: u64,
        request_open: bool,
        session_open: bool,
    }

    const CONTROLLED_CASES: [MatrixCase; 23] = [
        MatrixCase {
            mode: "success",
            status: "success",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "portal-cancel",
            status: "user_cancelled",
            cleanup: "not_required",
            stage: "create_session",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "portal-failure",
            status: "infrastructure_failure",
            cleanup: "not_required",
            stage: "create_session",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-missing",
            status: "malformed_response",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-wrong-type",
            status: "malformed_response",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-path",
            status: "malformed_response",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-signal",
            status: "malformed_response",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-missing-session-present",
            status: "malformed_response",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-wrong-type-session-present",
            status: "malformed_response",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-path-session-present",
            status: "malformed_response",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-signal-session-present",
            status: "malformed_response",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "malformed-expected-session-close-failure",
            status: "malformed_response",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: true,
        },
        MatrixCase {
            mode: "response-timeout",
            status: "timed_out",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 1,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "request-timeout",
            status: "timed_out",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 1,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "late-reply",
            status: "timed_out",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 1,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "client-cancel",
            status: "user_cancelled",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 1,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "request-close-failure",
            status: "user_cancelled",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 1,
            session_closes: 0,
            request_open: true,
            session_open: false,
        },
        MatrixCase {
            mode: "transport-failure",
            status: "infrastructure_failure",
            cleanup: "completed",
            stage: "create_session",
            request_closes: 1,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "unanswered",
            status: "timed_out",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "session-close-failure",
            status: "success",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: true,
        },
        MatrixCase {
            mode: "session-close-ambiguous",
            status: "success",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: true,
        },
        MatrixCase {
            mode: "unavailable",
            status: "unavailable",
            cleanup: "not_required",
            stage: "prepare",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "unsupported",
            status: "unsupported",
            cleanup: "not_required",
            stage: "prepare",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
    ];

    #[test]
    fn controlled_create_session_matrix() {
        if std::env::var_os("PORTALDOCTOR_RUN_CONTROLLED_MATRIX").is_none() {
            eprintln!("controlled ScreenCast matrix skipped outside its isolated gate");
            return;
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        for case in CONTROLLED_CASES {
            run_controlled_case(
                &runtime,
                case.mode,
                case.status,
                case.cleanup,
                case.stage,
                case.request_closes,
                case.session_closes,
                case.request_open,
                case.session_open,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_controlled_case(
        runtime: &tokio::runtime::Runtime,
        mode: &str,
        expected_status: &str,
        expected_cleanup: &str,
        expected_stage: &str,
        expected_request_closes: u64,
        expected_session_closes: u64,
        expected_request_open: bool,
        expected_session_open: bool,
    ) {
        let state_path = unique_state_path(mode);
        let mut fake = FakeGuard::spawn(mode, &state_path);
        wait_for_fake_ready(&state_path);
        let cancellation_delay = if matches!(mode, "client-cancel" | "request-close-failure") {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(500)
        };
        let result = runtime.block_on(run_create_session_with(
            move || async move {
                tokio::time::sleep(cancellation_delay).await;
            },
            CreateSessionTimeouts::controlled(),
        ));
        fake.stop();
        let state = read_state(&state_path);
        let encoded = serde_json::to_string(&result).expect("result serializes");
        assert!(!encoded.contains("org.freedesktop.portal/desktop"));
        assert!(!encoded.contains("request-token"));
        assert!(!encoded.contains("session-token"));
        assert_eq!(
            serde_json::to_value(result.status())
                .unwrap()
                .as_str()
                .unwrap(),
            expected_status,
            "{mode}"
        );
        assert_eq!(
            serde_json::to_value(result.cleanup().status())
                .unwrap()
                .as_str()
                .unwrap(),
            expected_cleanup,
            "{mode}"
        );
        assert_eq!(
            serde_json::to_value(result.stage())
                .unwrap()
                .as_str()
                .unwrap(),
            expected_stage,
            "{mode}"
        );
        let expected_resources: &[CleanupResource] = match mode {
            "request-close-failure" => &[CleanupResource::Request],
            "session-close-failure"
            | "session-close-ambiguous"
            | "malformed-expected-session-close-failure"
            | "malformed-missing"
            | "malformed-wrong-type"
            | "malformed-path"
            | "malformed-signal" => &[CleanupResource::Session],
            _ => &[],
        };
        assert_eq!(
            result.cleanup().failed_resources(),
            expected_resources,
            "{mode} cleanup resource"
        );
        assert_eq!(
            state["request_close_calls"].as_u64().unwrap(),
            expected_request_closes,
            "{mode}"
        );
        assert_eq!(
            state["session_close_calls"].as_u64().unwrap(),
            expected_session_closes,
            "{mode}"
        );
        assert_eq!(
            state["request_active"].as_bool().unwrap(),
            expected_request_open,
            "{mode}"
        );
        assert_eq!(
            state["session_active"].as_bool().unwrap(),
            expected_session_open,
            "{mode}"
        );
        assert!(!state["unexpected_close"].as_bool().unwrap(), "{mode}");
        assert!(state["tokens_valid"].as_bool().unwrap(), "{mode}");
        let _ = fs::remove_file(state_path);
    }

    const SELECT_SOURCES_CASES: [MatrixCase; 18] = [
        MatrixCase {
            mode: "select-success",
            status: "success",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-unsupported-window",
            status: "unsupported",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-unsupported-property",
            status: "unsupported",
            cleanup: "not_required",
            stage: "prepare",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-unavailable",
            status: "unavailable",
            cleanup: "not_required",
            stage: "prepare",
            request_closes: 0,
            session_closes: 0,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-portal-cancel",
            status: "user_cancelled",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-portal-failure",
            status: "infrastructure_failure",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-malformed",
            status: "malformed_response",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-malformed-signal",
            status: "malformed_response",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-response-timeout",
            status: "timed_out",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-request-timeout",
            status: "timed_out",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-late-reply",
            status: "timed_out",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-client-cancel",
            status: "user_cancelled",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-request-close-failure",
            status: "user_cancelled",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 1,
            session_closes: 1,
            request_open: true,
            session_open: false,
        },
        MatrixCase {
            mode: "select-transport-failure",
            status: "infrastructure_failure",
            cleanup: "completed",
            stage: "select_sources",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "select-session-close-failure",
            status: "success",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: true,
        },
        MatrixCase {
            mode: "select-session-close-ambiguous",
            status: "success",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: true,
        },
        MatrixCase {
            mode: "select-request-session-cleanup-failure",
            status: "timed_out",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 1,
            session_closes: 1,
            request_open: true,
            session_open: true,
        },
        MatrixCase {
            mode: "select-unanswered",
            status: "timed_out",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
    ];

    #[test]
    fn controlled_select_sources_matrix() {
        if std::env::var_os("PORTALDOCTOR_RUN_CONTROLLED_MATRIX").is_none() {
            eprintln!(
                "controlled ScreenCast SelectSources matrix skipped outside its isolated gate"
            );
            return;
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        for case in SELECT_SOURCES_CASES {
            run_controlled_select_case(&runtime, case);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn run_controlled_select_case(runtime: &tokio::runtime::Runtime, case: MatrixCase) {
        let state_path = unique_state_path(case.mode);
        let mut fake = FakeGuard::spawn_select(case.mode, &state_path);
        wait_for_fake_ready(&state_path);
        let cancellation_delay = if matches!(
            case.mode,
            "select-client-cancel" | "select-request-close-failure"
        ) {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(500)
        };
        let result = runtime.block_on(run_select_sources_with(
            move || async move {
                tokio::time::sleep(cancellation_delay).await;
            },
            CreateSessionTimeouts::controlled(),
        ));
        fake.stop();
        let state = read_state(&state_path);
        let encoded = serde_json::to_string(&result).expect("result serializes");
        assert!(!encoded.contains("org.freedesktop.portal/desktop"));
        assert!(!encoded.contains("handle_token"));
        assert!(!encoded.contains("session_handle"));
        assert_eq!(
            serde_json::to_value(result.status())
                .unwrap()
                .as_str()
                .unwrap(),
            case.status,
            "{}",
            case.mode
        );
        assert_eq!(
            serde_json::to_value(result.cleanup().status())
                .unwrap()
                .as_str()
                .unwrap(),
            case.cleanup,
            "{}",
            case.mode
        );
        assert_eq!(
            serde_json::to_value(result.stage())
                .unwrap()
                .as_str()
                .unwrap(),
            case.stage,
            "{}",
            case.mode
        );
        let expected_resources: &[CleanupResource] = match case.mode {
            "select-request-close-failure" => &[CleanupResource::Request],
            "select-session-close-failure" | "select-session-close-ambiguous" => {
                &[CleanupResource::Session]
            }
            "select-request-session-cleanup-failure" => {
                &[CleanupResource::Request, CleanupResource::Session]
            }
            _ => &[],
        };
        assert_eq!(
            result.cleanup().failed_resources(),
            expected_resources,
            "{} cleanup resources",
            case.mode
        );
        assert_eq!(
            state["request_close_calls"].as_u64().unwrap(),
            case.request_closes,
            "{} request close count",
            case.mode
        );
        assert_eq!(
            state["create_request_close_calls"].as_u64().unwrap(),
            0,
            "{} terminal CreateSession request close count",
            case.mode
        );
        assert_eq!(
            state["select_request_close_calls"].as_u64().unwrap(),
            case.request_closes,
            "{} SelectSources request close count",
            case.mode
        );
        assert_eq!(
            state["session_close_calls"].as_u64().unwrap(),
            case.session_closes,
            "{} session close count",
            case.mode
        );
        assert_eq!(
            state["select_calls"].as_u64().unwrap(),
            u64::from(!matches!(
                case.mode,
                "select-unsupported-property" | "select-unsupported-window" | "select-unavailable"
            )),
            "{} SelectSources call count",
            case.mode
        );
        assert_eq!(
            state["request_active"].as_bool().unwrap(),
            case.request_open,
            "{} request state",
            case.mode
        );
        assert_eq!(
            state["select_request_active"].as_bool().unwrap(),
            case.request_open,
            "{} SelectSources request state",
            case.mode
        );
        assert_eq!(
            state["session_active"].as_bool().unwrap(),
            case.session_open,
            "{} session state",
            case.mode
        );
        assert!(
            !state["unexpected_close"].as_bool().unwrap(),
            "{}",
            case.mode
        );
        assert!(state["tokens_valid"].as_bool().unwrap(), "{}", case.mode);
        assert!(
            state["select_options_valid"].as_bool().unwrap(),
            "{} exact SelectSources options",
            case.mode
        );
        let _ = fs::remove_file(state_path);
    }

    const START_CASES: [MatrixCase; 15] = [
        MatrixCase {
            mode: "start-success",
            status: "success",
            cleanup: "completed",
            stage: "start",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-portal-cancel",
            status: "user_cancelled",
            cleanup: "completed",
            stage: "start",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-portal-failure",
            status: "infrastructure_failure",
            cleanup: "completed",
            stage: "start",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-malformed",
            status: "malformed_response",
            cleanup: "completed",
            stage: "start",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-malformed-signal",
            status: "malformed_response",
            cleanup: "completed",
            stage: "start",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-response-timeout",
            status: "timed_out",
            cleanup: "completed",
            stage: "start",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-request-timeout",
            status: "timed_out",
            cleanup: "completed",
            stage: "start",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-late-reply",
            status: "timed_out",
            cleanup: "completed",
            stage: "start",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-client-cancel",
            status: "user_cancelled",
            cleanup: "completed",
            stage: "start",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-request-close-failure",
            status: "user_cancelled",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 1,
            session_closes: 1,
            request_open: true,
            session_open: false,
        },
        MatrixCase {
            mode: "start-transport-failure",
            status: "infrastructure_failure",
            cleanup: "completed",
            stage: "start",
            request_closes: 1,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
        MatrixCase {
            mode: "start-session-close-failure",
            status: "success",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: true,
        },
        MatrixCase {
            mode: "start-session-close-ambiguous",
            status: "success",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: true,
        },
        MatrixCase {
            mode: "start-request-session-cleanup-failure",
            status: "timed_out",
            cleanup: "failed",
            stage: "cleanup",
            request_closes: 1,
            session_closes: 1,
            request_open: true,
            session_open: true,
        },
        MatrixCase {
            mode: "start-unanswered",
            status: "timed_out",
            cleanup: "unverified",
            stage: "cleanup",
            request_closes: 0,
            session_closes: 1,
            request_open: false,
            session_open: false,
        },
    ];

    #[test]
    fn controlled_start_matrix() {
        if std::env::var_os("PORTALDOCTOR_RUN_CONTROLLED_MATRIX").is_none() {
            eprintln!("controlled ScreenCast Start matrix skipped outside its isolated gate");
            return;
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        for case in START_CASES {
            run_controlled_start_case(&runtime, case);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn run_controlled_start_case(runtime: &tokio::runtime::Runtime, case: MatrixCase) {
        let state_path = unique_state_path(case.mode);
        let mut fake = FakeGuard::spawn_select(case.mode, &state_path);
        wait_for_fake_ready(&state_path);
        let cancellation_delay = if matches!(
            case.mode,
            "start-client-cancel" | "start-request-close-failure"
        ) {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(500)
        };
        let result = runtime.block_on(run_start_with(
            move || async move {
                tokio::time::sleep(cancellation_delay).await;
            },
            CreateSessionTimeouts::controlled(),
        ));
        fake.stop();
        let state = read_state(&state_path);
        let encoded = serde_json::to_string(&result).expect("result serializes");
        assert!(!encoded.contains("org.freedesktop.portal/desktop"));
        assert!(!encoded.contains("handle_token"));
        assert!(!encoded.contains("session_handle"));
        assert!(!encoded.contains("synthetic-window"));
        assert!(!encoded.contains("node_id"));
        assert!(!encoded.contains("4242"));
        assert!(!encoded.contains("streams"));
        assert_eq!(
            serde_json::to_value(result.status())
                .unwrap()
                .as_str()
                .unwrap(),
            case.status,
            "{}",
            case.mode
        );
        assert_eq!(
            serde_json::to_value(result.cleanup().status())
                .unwrap()
                .as_str()
                .unwrap(),
            case.cleanup,
            "{}",
            case.mode
        );
        assert_eq!(
            serde_json::to_value(result.stage())
                .unwrap()
                .as_str()
                .unwrap(),
            case.stage,
            "{}",
            case.mode
        );
        let expected_resources: &[CleanupResource] = match case.mode {
            "start-request-close-failure" => &[CleanupResource::Request],
            "start-session-close-failure" | "start-session-close-ambiguous" => {
                &[CleanupResource::Session]
            }
            "start-request-session-cleanup-failure" => {
                &[CleanupResource::Request, CleanupResource::Session]
            }
            _ => &[],
        };
        assert_eq!(
            result.cleanup().failed_resources(),
            expected_resources,
            "{} cleanup resources",
            case.mode
        );
        assert_eq!(
            state["request_close_calls"].as_u64().unwrap(),
            case.request_closes,
            "{} request close count",
            case.mode
        );
        assert_eq!(
            state["create_request_close_calls"].as_u64().unwrap(),
            0,
            "{} terminal CreateSession request close count",
            case.mode
        );
        assert_eq!(
            state["select_request_close_calls"].as_u64().unwrap(),
            0,
            "{} terminal SelectSources request close count",
            case.mode
        );
        assert_eq!(
            state["start_request_close_calls"].as_u64().unwrap(),
            case.request_closes,
            "{} Start request close count",
            case.mode
        );
        assert_eq!(state["create_calls"].as_u64().unwrap(), 1, "{}", case.mode);
        assert_eq!(state["select_calls"].as_u64().unwrap(), 1, "{}", case.mode);
        assert_eq!(state["start_calls"].as_u64().unwrap(), 1, "{}", case.mode);
        assert_eq!(
            state["request_active"].as_bool().unwrap(),
            case.request_open,
            "{} request state",
            case.mode
        );
        assert_eq!(
            state["start_request_active"].as_bool().unwrap(),
            case.request_open,
            "{} Start request state",
            case.mode
        );
        assert_eq!(
            state["session_close_calls"].as_u64().unwrap(),
            case.session_closes,
            "{} session close count",
            case.mode
        );
        assert_eq!(
            state["session_active"].as_bool().unwrap(),
            case.session_open,
            "{} session state",
            case.mode
        );
        assert!(
            state["start_options_valid"].as_bool().unwrap(),
            "{} exact Start options",
            case.mode
        );
        let stream_payload_expected = matches!(
            case.mode,
            "start-success"
                | "start-portal-cancel"
                | "start-portal-failure"
                | "start-malformed"
                | "start-session-close-failure"
                | "start-session-close-ambiguous"
        );
        assert_eq!(
            state["start_streams_payload_emitted"].as_bool().unwrap(),
            stream_payload_expected,
            "{} synthetic Start streams payload boundary",
            case.mode
        );
        assert!(
            !state["unexpected_close"].as_bool().unwrap(),
            "{}",
            case.mode
        );
        assert!(state["tokens_valid"].as_bool().unwrap(), "{}", case.mode);
        assert!(
            state["select_options_valid"].as_bool().unwrap(),
            "{} exact SelectSources options",
            case.mode
        );
        let _ = fs::remove_file(state_path);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Keep matrix resource assertions together.
    fn controlled_streams_returned_matrix() {
        if std::env::var_os("PORTALDOCTOR_RUN_CONTROLLED_MATRIX").is_none() {
            return;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for (mode, valid, close_failure) in [
            ("streams-valid", true, false),
            ("streams-minimal", true, false),
            ("streams-missing", false, false),
            ("streams-wrong-type", false, false),
            ("streams-empty", false, false),
            ("streams-multiple", false, false),
            ("streams-malformed-tuple", false, false),
            ("streams-malformed-container", false, false),
            ("streams-extra", true, false),
            ("streams-opaque", true, false),
            ("streams-wrong-source", false, false),
            ("streams-wrong-source-type", false, false),
            ("streams-valid-close-failure", true, true),
            ("streams-malformed-close-failure", false, true),
        ] {
            let path = unique_state_path(mode);
            let mut fake = FakeGuard::spawn_select(mode, &path);
            wait_for_fake_ready(&path);
            let result = runtime.block_on(super::run_screencast_with(
                std::future::pending::<()>,
                CreateSessionTimeouts::controlled(),
                ProbeStage::StreamsReturned,
            ));
            let state = read_state(&path);
            fake.stop();
            assert_eq!(
                result.status(),
                if valid {
                    ProbeStatus::Success
                } else {
                    ProbeStatus::MalformedResponse
                },
                "{mode}"
            );
            assert_eq!(
                result.stage(),
                if close_failure {
                    ProbeStage::Cleanup
                } else {
                    ProbeStage::StreamsReturned
                },
                "{mode}"
            );
            assert_eq!(
                result.cleanup().status(),
                if close_failure {
                    CleanupStatus::Failed
                } else {
                    CleanupStatus::Completed
                },
                "{mode}"
            );
            assert_eq!(
                result.cleanup().failed_resources(),
                if close_failure {
                    &[CleanupResource::Session][..]
                } else {
                    &[]
                },
                "{mode}"
            );
            for counter in [
                "request_close_calls",
                "create_request_close_calls",
                "select_request_close_calls",
                "start_request_close_calls",
            ] {
                assert_eq!(state[counter], 0, "{mode}: {counter}");
            }
            for counter in [
                "create_calls",
                "select_calls",
                "start_calls",
                "session_close_calls",
            ] {
                assert_eq!(state[counter], 1, "{mode}: {counter}");
            }
            for flag in [
                "request_active",
                "select_request_active",
                "start_request_active",
                "unexpected_close",
            ] {
                assert_eq!(state[flag], false, "{mode}: {flag}");
            }
            assert_eq!(state["session_active"], close_failure, "{mode}");
            for flag in [
                "tokens_valid",
                "select_options_valid",
                "start_options_valid",
                "start_streams_payload_emitted",
            ] {
                assert_eq!(state[flag], true, "{mode}: {flag}");
            }
            let encoded = serde_json::to_string(&result).unwrap();
            let decoded: crate::model::probe::ProbeResult = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, result);
            for private in [
                "424242",
                "private-stream-sentinel",
                "private-node",
                "private-property",
                "source_type",
                "handle_token",
                "/org/freedesktop/",
                "session_handle",
            ] {
                assert!(!encoded.contains(private), "{mode}: result privacy");
                assert!(
                    !state.to_string().contains(private),
                    "{mode}: state privacy"
                );
            }
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Keep direct-FD ownership assertions together.
    fn controlled_open_pipe_wire_remote_matrix() {
        if std::env::var_os("PORTALDOCTOR_RUN_CONTROLLED_MATRIX").is_none() {
            return;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for (
            mode,
            expected_status,
            expected_cleanup,
            expected_stage,
            expected_fd_sent,
            fd_close_failure,
            session_close_failure,
            expected_fd_order,
        ) in [
            (
                "open-success",
                ProbeStatus::Success,
                CleanupStatus::Completed,
                ProbeStage::OpenPipeWireRemote,
                1,
                false,
                false,
                true,
            ),
            (
                "open-transport-failure",
                ProbeStatus::InfrastructureFailure,
                CleanupStatus::Completed,
                ProbeStage::OpenPipeWireRemote,
                0,
                false,
                false,
                false,
            ),
            (
                "open-unavailable",
                ProbeStatus::Unavailable,
                CleanupStatus::Completed,
                ProbeStage::OpenPipeWireRemote,
                0,
                false,
                false,
                false,
            ),
            (
                "open-unsupported",
                ProbeStatus::Unsupported,
                CleanupStatus::Completed,
                ProbeStage::OpenPipeWireRemote,
                0,
                false,
                false,
                false,
            ),
            (
                "open-wrong-reply",
                ProbeStatus::MalformedResponse,
                CleanupStatus::Unverified,
                ProbeStage::Cleanup,
                0,
                false,
                false,
                false,
            ),
            (
                "open-malformed-reply",
                ProbeStatus::MalformedResponse,
                CleanupStatus::Unverified,
                ProbeStage::Cleanup,
                0,
                false,
                false,
                false,
            ),
            (
                "open-response-timeout",
                ProbeStatus::TimedOut,
                CleanupStatus::Unverified,
                ProbeStage::Cleanup,
                0,
                false,
                false,
                false,
            ),
            (
                "open-method-timeout",
                ProbeStatus::TimedOut,
                CleanupStatus::Unverified,
                ProbeStage::Cleanup,
                0,
                false,
                false,
                false,
            ),
            (
                "open-client-cancel",
                ProbeStatus::UserCancelled,
                CleanupStatus::Unverified,
                ProbeStage::Cleanup,
                0,
                false,
                false,
                false,
            ),
            (
                "open-late-fd",
                ProbeStatus::TimedOut,
                CleanupStatus::Completed,
                ProbeStage::OpenPipeWireRemote,
                1,
                false,
                false,
                true,
            ),
            (
                "open-late-transport",
                ProbeStatus::TimedOut,
                CleanupStatus::Completed,
                ProbeStage::OpenPipeWireRemote,
                0,
                false,
                false,
                false,
            ),
            (
                "open-ambiguous",
                ProbeStatus::TimedOut,
                CleanupStatus::Unverified,
                ProbeStage::Cleanup,
                0,
                false,
                false,
                false,
            ),
            (
                "open-fd-close-failure",
                ProbeStatus::Success,
                CleanupStatus::Failed,
                ProbeStage::Cleanup,
                1,
                true,
                false,
                true,
            ),
            (
                "open-session-close-failure",
                ProbeStatus::Success,
                CleanupStatus::Failed,
                ProbeStage::Cleanup,
                1,
                false,
                true,
                true,
            ),
            (
                "open-fd-session-cleanup-failure",
                ProbeStatus::Success,
                CleanupStatus::Failed,
                ProbeStage::Cleanup,
                1,
                true,
                true,
                true,
            ),
        ] {
            let state_path = unique_state_path(mode);
            let marker_path = state_path.with_extension("fd-closed");
            let mut fake =
                FakeGuard::spawn_select_with_marker(mode, &state_path, Some(&marker_path));
            wait_for_fake_ready(&state_path);
            let cancel_at_open = mode == "open-client-cancel";
            let cancellation_calls: &'static std::sync::atomic::AtomicUsize =
                &*Box::leak(Box::new(std::sync::atomic::AtomicUsize::new(0)));
            let marker_for_closer = marker_path.clone();
            let result = runtime.block_on(super::run_screencast_with_fd(
                move || {
                    let call = cancellation_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    async move {
                        if cancel_at_open && call == 6 {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    }
                },
                CreateSessionTimeouts::controlled(),
                ProbeStage::OpenPipeWireRemote,
                move |fd: OwnedFd| {
                    let cleanup = super::close_owned_pipewire_fd(fd);
                    fs::write(&marker_for_closer, b"closed").unwrap();
                    if fd_close_failure {
                        super::failed_pipe_wire_cleanup()
                    } else {
                        cleanup
                    }
                },
            ));
            let state = read_state(&state_path);
            fake.stop();
            assert_eq!(result.status(), expected_status, "{mode}");
            assert_eq!(result.cleanup().status(), expected_cleanup, "{mode}");
            assert_eq!(result.stage(), expected_stage, "{mode}");
            let expected_resources: &[CleanupResource] =
                match (fd_close_failure, session_close_failure) {
                    (true, true) => &[CleanupResource::PipeWireRemote, CleanupResource::Session],
                    (true, false) => &[CleanupResource::PipeWireRemote],
                    (false, true) => &[CleanupResource::Session],
                    (false, false) if matches!(expected_cleanup, CleanupStatus::Unverified) => {
                        &[CleanupResource::PipeWireRemote]
                    }
                    (false, false) => &[],
                };
            assert_eq!(
                result.cleanup().failed_resources(),
                expected_resources,
                "{mode}"
            );
            assert_eq!(state["create_calls"], 1, "{mode}: CreateSession");
            assert_eq!(state["select_calls"], 1, "{mode}: SelectSources");
            assert_eq!(state["start_calls"], 1, "{mode}: Start");
            assert_eq!(state["open_calls"], 1, "{mode}: OpenPipeWireRemote");
            assert_eq!(state["open_fd_sent"], expected_fd_sent, "{mode}: fd sent");
            assert_eq!(state["session_close_calls"], 1, "{mode}: Session.Close");
            assert_eq!(state["session_active"], session_close_failure, "{mode}");
            assert_eq!(
                state["fd_close_before_session"], expected_fd_order,
                "{mode}: order"
            );
            assert!(
                state["open_options_valid"].as_bool().unwrap(),
                "{mode}: options"
            );
            assert_eq!(state["request_close_calls"], 0, "{mode}: Request.Close");
            assert_eq!(
                state["create_request_close_calls"], 0,
                "{mode}: Create close"
            );
            assert_eq!(
                state["select_request_close_calls"], 0,
                "{mode}: Select close"
            );
            assert_eq!(state["start_request_close_calls"], 0, "{mode}: Start close");
            assert!(
                !state["request_active"].as_bool().unwrap(),
                "{mode}: request"
            );
            assert!(
                !state["select_request_active"].as_bool().unwrap(),
                "{mode}: select"
            );
            assert!(
                !state["start_request_active"].as_bool().unwrap(),
                "{mode}: start"
            );
            assert!(
                !state["unexpected_close"].as_bool().unwrap(),
                "{mode}: close"
            );
            assert!(state["tokens_valid"].as_bool().unwrap(), "{mode}: tokens");
            let encoded = serde_json::to_string(&result).unwrap();
            let decoded: crate::model::probe::ProbeResult = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, result, "{mode}: result roundtrip");
            for private in [
                "private-fd",
                "424242",
                "synthetic-window",
                "/org/freedesktop/",
                "handle_token",
                "session_handle",
            ] {
                assert!(!encoded.contains(private), "{mode}: result privacy");
                assert!(
                    !state.to_string().contains(private),
                    "{mode}: state privacy"
                );
            }
            fs::remove_file(state_path).unwrap();
            let _ = fs::remove_file(marker_path);
        }
    }

    #[derive(Clone, Copy)]
    struct AggregateCase {
        mode: &'static str,
        status: ProbeStatus,
        cleanup: CleanupStatus,
        stage: ProbeStage,
        request_closes: u64,
        create_calls: u64,
        select_calls: u64,
        start_calls: u64,
        open_calls: u64,
        session_closes: u64,
        fd_sent: u64,
        cancel_call: Option<usize>,
        fd_close_failure: bool,
        session_close_failure: bool,
        fd_before_session: bool,
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Keep the aggregate state-machine audit together.
    fn controlled_aggregate_lifecycle_matrix() {
        if std::env::var_os("PORTALDOCTOR_RUN_CONTROLLED_MATRIX").is_none() {
            return;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let cases = [
            AggregateCase {
                mode: "open-success",
                status: ProbeStatus::Success,
                cleanup: CleanupStatus::Completed,
                stage: ProbeStage::OpenPipeWireRemote,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 1,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: true,
            },
            AggregateCase {
                mode: "aggregate-create-failure",
                status: ProbeStatus::InfrastructureFailure,
                cleanup: CleanupStatus::NotRequired,
                stage: ProbeStage::CreateSession,
                request_closes: 0,
                create_calls: 1,
                select_calls: 0,
                start_calls: 0,
                open_calls: 0,
                session_closes: 0,
                fd_sent: 0,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "select-portal-failure",
                status: ProbeStatus::InfrastructureFailure,
                cleanup: CleanupStatus::Completed,
                stage: ProbeStage::SelectSources,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 0,
                open_calls: 0,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "start-portal-failure",
                status: ProbeStatus::InfrastructureFailure,
                cleanup: CleanupStatus::Completed,
                stage: ProbeStage::Start,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 0,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "streams-malformed-container",
                status: ProbeStatus::MalformedResponse,
                cleanup: CleanupStatus::Completed,
                stage: ProbeStage::StreamsReturned,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 0,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "open-transport-failure",
                status: ProbeStatus::InfrastructureFailure,
                cleanup: CleanupStatus::Completed,
                stage: ProbeStage::OpenPipeWireRemote,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "open-method-timeout",
                status: ProbeStatus::TimedOut,
                cleanup: CleanupStatus::Unverified,
                stage: ProbeStage::Cleanup,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "open-response-timeout",
                status: ProbeStatus::TimedOut,
                cleanup: CleanupStatus::Unverified,
                stage: ProbeStage::Cleanup,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "open-ambiguous",
                status: ProbeStatus::TimedOut,
                cleanup: CleanupStatus::Unverified,
                stage: ProbeStage::Cleanup,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "open-late-fd",
                status: ProbeStatus::TimedOut,
                cleanup: CleanupStatus::Completed,
                stage: ProbeStage::OpenPipeWireRemote,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 1,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: true,
            },
            AggregateCase {
                mode: "open-fd-close-failure",
                status: ProbeStatus::Success,
                cleanup: CleanupStatus::Failed,
                stage: ProbeStage::Cleanup,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 1,
                cancel_call: None,
                fd_close_failure: true,
                session_close_failure: false,
                fd_before_session: true,
            },
            AggregateCase {
                mode: "open-session-close-failure",
                status: ProbeStatus::Success,
                cleanup: CleanupStatus::Failed,
                stage: ProbeStage::Cleanup,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 1,
                cancel_call: None,
                fd_close_failure: false,
                session_close_failure: true,
                fd_before_session: true,
            },
            AggregateCase {
                mode: "open-fd-session-cleanup-failure",
                status: ProbeStatus::Success,
                cleanup: CleanupStatus::Failed,
                stage: ProbeStage::Cleanup,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 1,
                cancel_call: None,
                fd_close_failure: true,
                session_close_failure: true,
                fd_before_session: true,
            },
            AggregateCase {
                mode: "select-client-cancel",
                status: ProbeStatus::UserCancelled,
                cleanup: CleanupStatus::Completed,
                stage: ProbeStage::SelectSources,
                request_closes: 1,
                create_calls: 1,
                select_calls: 1,
                start_calls: 0,
                open_calls: 0,
                session_closes: 1,
                fd_sent: 0,
                // The fake returns the SelectSources Request path first; this
                // cancellation therefore covers the response-wait stage.
                cancel_call: Some(3),
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "select-request-cancel",
                status: ProbeStatus::UserCancelled,
                cleanup: CleanupStatus::Completed,
                stage: ProbeStage::SelectSources,
                request_closes: 1,
                create_calls: 1,
                select_calls: 1,
                start_calls: 0,
                open_calls: 0,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: Some(2),
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
            AggregateCase {
                mode: "open-client-cancel",
                status: ProbeStatus::UserCancelled,
                cleanup: CleanupStatus::Unverified,
                stage: ProbeStage::Cleanup,
                request_closes: 0,
                create_calls: 1,
                select_calls: 1,
                start_calls: 1,
                open_calls: 1,
                session_closes: 1,
                fd_sent: 0,
                cancel_call: Some(6),
                fd_close_failure: false,
                session_close_failure: false,
                fd_before_session: false,
            },
        ];

        for case in cases {
            let state_path = unique_state_path(case.mode);
            let marker_path = state_path.with_extension("fd-closed");
            let mut fake =
                FakeGuard::spawn_select_with_marker(case.mode, &state_path, Some(&marker_path));
            wait_for_fake_ready(&state_path);
            let cancellation_calls: &'static std::sync::atomic::AtomicUsize =
                &*Box::leak(Box::new(std::sync::atomic::AtomicUsize::new(0)));
            let cancel_call = case.cancel_call;
            let marker_for_closer = marker_path.clone();
            let fd_close_failure = case.fd_close_failure;
            let result = runtime.block_on(run_screencast_with_fd(
                move || {
                    let call = cancellation_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    async move {
                        if cancel_call == Some(call) {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    }
                },
                CreateSessionTimeouts::controlled(),
                ProbeStage::OpenPipeWireRemote,
                move |fd: OwnedFd| {
                    let cleanup = super::close_owned_pipewire_fd(fd);
                    fs::write(&marker_for_closer, b"closed").unwrap();
                    if fd_close_failure {
                        super::failed_pipe_wire_cleanup()
                    } else {
                        cleanup
                    }
                },
            ));
            let state = read_state(&state_path);
            fake.stop();

            assert_eq!(result.status(), case.status, "{} status", case.mode);
            assert_eq!(
                result.cleanup().status(),
                case.cleanup,
                "{} cleanup",
                case.mode
            );
            assert_eq!(result.stage(), case.stage, "{} stage", case.mode);
            let expected_resources: &[CleanupResource] = match case.mode {
                "open-fd-close-failure"
                | "open-method-timeout"
                | "open-response-timeout"
                | "open-ambiguous"
                | "open-client-cancel" => &[CleanupResource::PipeWireRemote],
                "open-session-close-failure" => &[CleanupResource::Session],
                "open-fd-session-cleanup-failure" => {
                    &[CleanupResource::PipeWireRemote, CleanupResource::Session]
                }
                _ => &[],
            };
            assert_eq!(
                result.cleanup().failed_resources(),
                expected_resources,
                "{} cleanup resources",
                case.mode
            );
            assert_eq!(
                state["create_calls"], case.create_calls,
                "{} CreateSession",
                case.mode
            );
            assert_eq!(
                state["select_calls"], case.select_calls,
                "{} SelectSources",
                case.mode
            );
            assert_eq!(
                state["start_calls"], case.start_calls,
                "{} Start",
                case.mode
            );
            assert_eq!(
                state["open_calls"], case.open_calls,
                "{} OpenPipeWireRemote",
                case.mode
            );
            assert_eq!(
                state["open_fd_sent"], case.fd_sent,
                "{} FD replies",
                case.mode
            );
            assert_eq!(
                state["request_close_calls"], case.request_closes,
                "{} Request.Close",
                case.mode
            );
            assert_eq!(
                state["create_request_close_calls"], 0,
                "{} prior CreateSession Request.Close",
                case.mode
            );
            assert_eq!(
                state["select_request_close_calls"],
                u64::from(matches!(
                    case.mode,
                    "select-client-cancel" | "select-request-cancel"
                ),),
                "{} SelectSources Request.Close",
                case.mode
            );
            assert_eq!(
                state["start_request_close_calls"], 0,
                "{} prior Start Request.Close",
                case.mode
            );
            assert_eq!(
                state["session_close_calls"], case.session_closes,
                "{} Session.Close",
                case.mode
            );
            assert_eq!(
                state["session_active"], case.session_close_failure,
                "{} Session ownership",
                case.mode
            );
            assert!(
                !state["request_active"].as_bool().unwrap(),
                "{} request leak",
                case.mode
            );
            assert!(
                !state["select_request_active"].as_bool().unwrap(),
                "{} select leak",
                case.mode
            );
            assert!(
                !state["start_request_active"].as_bool().unwrap(),
                "{} start leak",
                case.mode
            );
            assert_eq!(
                state["fd_close_before_session"], case.fd_before_session,
                "{} FD/session ordering",
                case.mode
            );
            assert!(
                !state["unexpected_close"].as_bool().unwrap(),
                "{} close race",
                case.mode
            );
            assert!(
                state["tokens_valid"].as_bool().unwrap(),
                "{} tokens",
                case.mode
            );
            assert!(
                state["select_options_valid"].as_bool().unwrap(),
                "{} SelectSources options",
                case.mode
            );
            assert!(
                state["start_options_valid"].as_bool().unwrap(),
                "{} Start options",
                case.mode
            );
            assert!(
                state["open_options_valid"].as_bool().unwrap(),
                "{} OpenPipeWireRemote options",
                case.mode
            );
            assert_eq!(
                marker_path.exists(),
                case.fd_sent == 1,
                "{} FD release",
                case.mode
            );

            let encoded = serde_json::to_string(&result).unwrap();
            let decoded: crate::model::probe::ProbeResult = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, result, "{} result roundtrip", case.mode);
            for private in [
                "424242",
                "private-fd",
                "private-node",
                "private-property",
                "private-stream-sentinel",
                "synthetic-window",
                "/org/freedesktop/",
                "handle_token",
                "session_handle",
            ] {
                assert!(
                    !encoded.contains(private),
                    "{} result privacy: {private}",
                    case.mode
                );
                assert!(
                    !state.to_string().contains(private),
                    "{} state privacy: {private}",
                    case.mode
                );
            }
            fs::remove_file(state_path).unwrap();
            let _ = fs::remove_file(marker_path);
        }
    }

    fn unique_state_path(mode: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "portaldoctor-screencast-{mode}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    fn wait_for_fake_ready(path: &PathBuf) {
        for _ in 0..300 {
            if let Ok(content) = fs::read_to_string(path)
                && serde_json::from_str::<JsonValue>(&content)
                    .ok()
                    .and_then(|state| state["ready"].as_bool())
                    == Some(true)
            {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("controlled ScreenCast fake did not become ready");
    }

    fn read_state(path: &PathBuf) -> JsonValue {
        let content = fs::read_to_string(path).expect("fake state");
        serde_json::from_str(&content).expect("valid fake state")
    }

    struct FakeGuard {
        child: Child,
    }

    impl FakeGuard {
        fn spawn(mode: &str, state_path: &PathBuf) -> Self {
            let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("scripts/validate-screencast-create-session.py");
            Self::spawn_script(&script, mode, state_path)
        }

        fn spawn_select(mode: &str, state_path: &PathBuf) -> Self {
            Self::spawn_select_with_marker(mode, state_path, None)
        }

        fn spawn_select_with_marker(
            mode: &str,
            state_path: &PathBuf,
            fd_marker: Option<&PathBuf>,
        ) -> Self {
            let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("scripts/validate-screencast-select-sources.py");
            Self::spawn_script_with_marker(&script, mode, state_path, fd_marker)
        }

        fn spawn_script(script: &PathBuf, mode: &str, state_path: &PathBuf) -> Self {
            Self::spawn_script_with_marker(script, mode, state_path, None)
        }

        fn spawn_script_with_marker(
            script: &PathBuf,
            mode: &str,
            state_path: &PathBuf,
            fd_marker: Option<&PathBuf>,
        ) -> Self {
            let mut command = Command::new("python3");
            command
                .arg(script)
                .arg("--mode")
                .arg(mode)
                .arg("--state-file")
                .arg(state_path);
            if let Some(fd_marker) = fd_marker {
                command.arg("--fd-marker").arg(fd_marker);
            }
            let child = command
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn controlled ScreenCast fake");
            Self { child }
        }

        fn stop(&mut self) {
            if self.child.try_wait().expect("poll fake").is_none() {
                self.child.kill().expect("stop fake");
            }
            self.child.wait().expect("reap fake");
        }
    }

    impl Drop for FakeGuard {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
