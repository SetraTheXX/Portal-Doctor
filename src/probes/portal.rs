//! Shared, privacy-neutral pieces of the explicit portal request lifecycle.
//!
//! Probe modules own their operation-specific preflight and response mapping.
//! This module owns only the mechanics that must stay identical across active
//! probes: response-match registration, request metadata, bounded D-Bus
//! proxies, response waiting and no-response Request.Close cleanup.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_lite::StreamExt;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream, Proxy};

use crate::collectors::timeouts::{ACTIVE_PROBE_CLEANUP, ACTIVE_PROBE_SETUP};
use crate::model::probe::{CleanupResource, CleanupResult, ProbeStatus};

pub(crate) const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
pub(crate) const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
pub(crate) const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
pub(crate) const INTROSPECTABLE_INTERFACE: &str = "org.freedesktop.DBus.Introspectable";
pub(crate) const RESPONSE_MEMBER: &str = "Response";
pub(crate) const REQUEST_PATH_PREFIX: &str = "/org/freedesktop/portal/desktop/request/";
pub(crate) const SESSION_PATH_PREFIX: &str = "/org/freedesktop/portal/desktop/session/";
const HANDLE_TOKEN_PREFIX: &str = "portaldoctor";
const HANDLE_TOKEN_RANDOM_BYTES: usize = 16;

static HANDLE_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Raw response data is kept only until the operation-specific decoder
/// checks the response code and the required value type. It must never be
/// rendered, logged or copied into a result.
#[derive(Debug)]
pub(crate) enum ResponseWait {
    Outcome {
        code: u32,
        results: HashMap<String, OwnedValue>,
    },
    UserCancelled,
    TimedOut,
    InfrastructureFailure,
    MalformedResponse,
}

impl ResponseWait {
    /// The XDG `Response` signal is terminal for the request object. This is
    /// true even when the signal body is malformed: a matching signal was
    /// received, so sending `Request.Close` afterwards would be a protocol
    /// violation rather than cleanup.
    #[must_use]
    pub(crate) const fn ends_request(&self) -> bool {
        matches!(self, Self::Outcome { .. } | Self::MalformedResponse)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandleTokenError {
    EntropyUnavailable,
    UniqueNameUnavailable,
}

/// Active probes require a graphical display before they can issue a portal
/// request that may open UI. The environment values are checked only as
/// presence signals and are never copied into a result.
pub(crate) fn graphical_context_available() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}

pub(crate) async fn open_session() -> Result<Connection, ProbeStatus> {
    if !graphical_context_available() {
        return Err(ProbeStatus::Unavailable);
    }
    match tokio::time::timeout(ACTIVE_PROBE_SETUP, Connection::session()).await {
        Ok(Ok(connection)) => Ok(connection),
        Ok(Err(error)) => Err(classify_error(&error)),
        Err(_) => Err(ProbeStatus::TimedOut),
    }
}

/// Build the signal match before the portal method is sent. This is the
/// response-race boundary required by the XDG request lifecycle.
pub(crate) fn response_match_rule() -> MatchRule<'static> {
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

pub(crate) async fn bounded_proxy<'a>(
    connection: &'a Connection,
    interface: &'a str,
) -> Result<Proxy<'a>, ProbeStatus> {
    match tokio::time::timeout(
        crate::collectors::timeouts::ACTIVE_PROBE_SETUP,
        Proxy::new(connection, PORTAL_DESTINATION, PORTAL_PATH, interface),
    )
    .await
    {
        Ok(Ok(proxy)) => Ok(proxy),
        Ok(Err(error)) => Err(classify_error(&error)),
        Err(_) => Err(ProbeStatus::TimedOut),
    }
}

pub(crate) fn valid_request_path(path: &OwnedObjectPath) -> bool {
    let value = path.as_str();
    value.starts_with(REQUEST_PATH_PREFIX) && value.len() > REQUEST_PATH_PREFIX.len()
}

pub(crate) fn valid_request_path_for_sender(
    path: &OwnedObjectPath,
    expected_path: &OwnedObjectPath,
) -> bool {
    if !valid_request_path(path) {
        return false;
    }
    let Some((expected_sender, _)) = expected_path.as_str().rsplit_once('/') else {
        return false;
    };
    let sender_prefix = format!("{expected_sender}/");
    path.as_str().starts_with(&sender_prefix)
}

pub(crate) fn request_options(handle_token: &str) -> HashMap<String, OwnedValue> {
    HashMap::from([(
        "handle_token".to_owned(),
        OwnedValue::try_from(Value::from(handle_token.to_owned()))
            .expect("a token is a valid D-Bus string"),
    )])
}

pub(crate) fn request_metadata(
    connection: &Connection,
) -> Result<(String, OwnedObjectPath), HandleTokenError> {
    let handle_token = handle_token()?;
    let expected_path = expected_request_path(connection, &handle_token)
        .ok_or(HandleTokenError::UniqueNameUnavailable)?;
    Ok((handle_token, expected_path))
}

pub(crate) fn session_metadata(
    connection: &Connection,
) -> Result<(String, OwnedObjectPath), HandleTokenError> {
    let session_handle_token = handle_token()?;
    let expected_path = expected_session_path(connection, &session_handle_token)
        .ok_or(HandleTokenError::UniqueNameUnavailable)?;
    Ok((session_handle_token, expected_path))
}

pub(crate) fn handle_token() -> Result<String, HandleTokenError> {
    handle_token_with_entropy(|random| {
        getrandom::fill(random).map_err(|_| HandleTokenError::EntropyUnavailable)
    })
}

pub(crate) fn handle_token_with_entropy<F>(fill: F) -> Result<String, HandleTokenError>
where
    F: FnOnce(&mut [u8; HANDLE_TOKEN_RANDOM_BYTES]) -> Result<(), HandleTokenError>,
{
    let mut random = [0_u8; HANDLE_TOKEN_RANDOM_BYTES];
    fill(&mut random)?;
    let sequence = HANDLE_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut random_hex = String::with_capacity(random.len() * 2);
    for byte in random {
        write!(&mut random_hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(format!("{HANDLE_TOKEN_PREFIX}_{random_hex}_{sequence:x}"))
}

fn expected_request_path(connection: &Connection, handle_token: &str) -> Option<OwnedObjectPath> {
    request_path_for_sender(connection.unique_name()?.as_str(), handle_token)
}

pub(crate) fn request_path_for_sender(
    unique_name: &str,
    handle_token: &str,
) -> Option<OwnedObjectPath> {
    let sender = unique_name.strip_prefix(':')?;
    let sender = sender.replace('.', "_");
    let path = format!("{REQUEST_PATH_PREFIX}{sender}/{handle_token}");
    OwnedObjectPath::try_from(path).ok()
}

pub(crate) fn valid_session_path(path: &str) -> bool {
    path.starts_with(SESSION_PATH_PREFIX) && path.len() > SESSION_PATH_PREFIX.len()
}

pub(crate) fn valid_session_path_for_sender(path: &str, expected_path: &OwnedObjectPath) -> bool {
    if !valid_session_path(path) {
        return false;
    }
    let Some((expected_sender, _)) = expected_path.as_str().rsplit_once('/') else {
        return false;
    };
    let sender_prefix = format!("{expected_sender}/");
    path.starts_with(&sender_prefix)
}

fn expected_session_path(
    connection: &Connection,
    session_handle_token: &str,
) -> Option<OwnedObjectPath> {
    session_path_for_sender(connection.unique_name()?.as_str(), session_handle_token)
}

pub(crate) fn session_path_for_sender(
    unique_name: &str,
    session_handle_token: &str,
) -> Option<OwnedObjectPath> {
    let sender = unique_name.strip_prefix(':')?;
    let sender = sender.replace('.', "_");
    let path = format!("{SESSION_PATH_PREFIX}{sender}/{session_handle_token}");
    OwnedObjectPath::try_from(path).ok()
}

/// Wait for one request response, preserving only the typed D-Bus payload for
/// the operation-specific decoder. The caller must drop the returned raw map
/// after checking the required type; this function never renders it.
///
/// A matching `Response` signal ends the request lifecycle. Callers must not
/// issue `Request.Close` for `Outcome` or `MalformedResponse`; cleanup is
/// represented as `not_required`. `UserCancelled`, `TimedOut` and
/// `InfrastructureFailure` mean that no matching response was received and
/// require the bounded close path.
pub(crate) async fn wait_for_response(
    stream: &mut MessageStream,
    request_path: &str,
) -> ResponseWait {
    wait_for_response_with_cancel(
        stream,
        request_path,
        async {
            let _ = tokio::signal::ctrl_c().await;
        },
        crate::collectors::timeouts::ACTIVE_PROBE_RESPONSE,
    )
    .await
}

/// Testable form of `wait_for_response` with an injected cancellation future and
/// response budget. Production callers use the central active-probe timeout;
/// controlled fakes use a shorter budget so every mode remains fast without
/// changing production policy.
pub(crate) async fn wait_for_response_with_cancel<C>(
    stream: &mut MessageStream,
    request_path: &str,
    cancellation: C,
    response_timeout: Duration,
) -> ResponseWait
where
    C: Future<Output = ()>,
{
    let response = async {
        loop {
            let Some(message) = stream.next().await else {
                return ResponseWait::InfrastructureFailure;
            };
            let Ok(message) = message else {
                return ResponseWait::InfrastructureFailure;
            };
            let header = message.header();
            let Some(path) = header.path() else {
                continue;
            };
            if path.as_str() != request_path {
                continue;
            }
            let (code, results): (u32, HashMap<String, OwnedValue>) =
                match message.body().deserialize() {
                    Ok(body) => body,
                    Err(_) => return ResponseWait::MalformedResponse,
                };
            return ResponseWait::Outcome { code, results };
        }
    };
    tokio::pin!(response);
    tokio::pin!(cancellation);
    tokio::select! {
        outcome = &mut response => outcome,
        () = &mut cancellation => ResponseWait::UserCancelled,
        () = tokio::time::sleep(response_timeout) => ResponseWait::TimedOut,
    }
}

/// Apply the shared XDG request lifecycle rule after response waiting.
///
/// A request that emitted `Response` is already terminal and must not receive
/// a later `Request.Close`. If waiting ended without a response, the returned
/// request path is still the bounded abort target. The path is known here
/// because this helper is called only after the portal method returned a
/// validated request object, so an already-gone object is a verified outcome.
pub(crate) async fn cleanup_after_response_wait(
    connection: &Connection,
    request_path: &str,
    response: &ResponseWait,
) -> CleanupResult {
    if response.ends_request() {
        CleanupResult::not_required()
    } else {
        close_request(connection, request_path, true).await
    }
}

/// Abort a request that has not produced a matching `Response`.
///
/// This is intentionally not a generic post-processing step: XDG Request
/// objects end with `Response`, so callers must not invoke this function after
/// `ResponseWait::Outcome` or `ResponseWait::MalformedResponse`. The
/// `unknown_is_completed` flag is used only for a known returned request whose
/// object may already have disappeared while the client was cancelling it.
pub(crate) async fn close_request(
    connection: &Connection,
    request_path: &str,
    unknown_is_completed: bool,
) -> CleanupResult {
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
        Ok(Err(error)) if object_already_gone(&error) && unknown_is_completed => {
            CleanupResult::completed()
        }
        Ok(Err(error)) if object_already_gone(&error) => CleanupResult::unverified(),
        Ok(Err(_)) | Err(_) => failed_request_cleanup(),
    }
}

fn failed_request_cleanup() -> CleanupResult {
    CleanupResult::failed(vec![CleanupResource::Request])
        .expect("an active request cleanup resource is valid")
}

pub(crate) fn classify_error(error: &zbus::Error) -> ProbeStatus {
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
        || message.contains("unknownproperty")
        || message.contains("unknown property")
        || message.contains("not supported")
        || message.contains("notsupported")
        || message.contains("notimplemented")
    {
        ProbeStatus::Unsupported
    } else {
        ProbeStatus::InfrastructureFailure
    }
}

pub(crate) fn object_already_gone(error: &zbus::Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("unknownobject")
        || message.contains("unknown object")
        || message.contains("no such object")
        || message.contains("does not exist")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        HandleTokenError, ResponseWait, handle_token, handle_token_with_entropy,
        request_path_for_sender, session_path_for_sender, valid_session_path,
        valid_session_path_for_sender,
    };

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
    fn request_path_uses_only_a_valid_sender_name() {
        let token = handle_token().expect("the operating system entropy source is available");
        assert_eq!(
            request_path_for_sender(":1.42", &token).unwrap().as_str(),
            format!("/org/freedesktop/portal/desktop/request/1_42/{token}")
        );
        assert!(request_path_for_sender("org.freedesktop.portal.Desktop", &token).is_none());
    }

    #[test]
    fn session_path_uses_only_a_valid_sender_name() {
        let token = handle_token().expect("the operating system entropy source is available");
        let path = session_path_for_sender(":1.42", &token).unwrap();
        assert!(valid_session_path(path.as_str()));
        let expected = session_path_for_sender(":1.42", "expected").unwrap();
        assert!(valid_session_path_for_sender(path.as_str(), &expected));
        assert!(!valid_session_path_for_sender(
            "/org/freedesktop/portal/desktop/session/1_43/other",
            &expected
        ));
        assert!(session_path_for_sender("org.freedesktop.portal.Desktop", &token).is_none());
    }

    #[test]
    fn response_terminality_is_independent_of_response_status() {
        assert!(
            ResponseWait::Outcome {
                code: 0,
                results: HashMap::new(),
            }
            .ends_request()
        );
        assert!(
            ResponseWait::Outcome {
                code: 1,
                results: HashMap::new(),
            }
            .ends_request()
        );
        assert!(ResponseWait::MalformedResponse.ends_request());
        assert!(!ResponseWait::UserCancelled.ends_request());
        assert!(!ResponseWait::TimedOut.ends_request());
        assert!(!ResponseWait::InfrastructureFailure.ends_request());
    }
}
