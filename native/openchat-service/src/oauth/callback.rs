use std::net::{Ipv4Addr, SocketAddr};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use tokio::{io::AsyncReadExt, net::TcpListener};
use url::Url;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    AUTHORIZATION_ENDPOINT, CALLBACK_PATH, CLIENT_ID, DEFAULT_CALLBACK_PORT,
    FALLBACK_CALLBACK_PORT, MAX_CALLBACK_REQUEST_LINE_BYTES, browser_error, callback_error,
};
pub(super) fn authorization_url(
    redirect_uri: &str,
    state: &str,
    challenge: &str,
) -> Result<Url, ServiceError> {
    let mut url = Url::parse(AUTHORIZATION_ENDPOINT).map_err(|_| browser_error())?;
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        (
            "scope",
            "openid profile email offline_access api.connectors.read api.connectors.invoke",
        ),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "Codex"),
    ]);
    Ok(url)
}

pub(super) async fn bind_callback_listener(
    storage: &AppStorage,
) -> Result<(TcpListener, u16), ServiceError> {
    match bind_callback_listener_on(DEFAULT_CALLBACK_PORT).await {
        Ok(listener) => {
            record_oauth_event(
                storage,
                "oauth_callback_listener_ready",
                Some("127.0.0.1"),
                Some(DEFAULT_CALLBACK_PORT),
                None,
                None,
            );
            Ok(listener)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            record_oauth_event(
                storage,
                "oauth_callback_primary_port_in_use",
                Some("127.0.0.1"),
                Some(DEFAULT_CALLBACK_PORT),
                Some("address_in_use"),
                None,
            );
            match bind_callback_listener_on(FALLBACK_CALLBACK_PORT).await {
                Ok(listener) => {
                    record_oauth_event(
                        storage,
                        "oauth_callback_listener_ready",
                        Some("127.0.0.1"),
                        Some(FALLBACK_CALLBACK_PORT),
                        None,
                        None,
                    );
                    Ok(listener)
                }
                Err(error) => {
                    record_oauth_event(
                        storage,
                        "oauth_callback_listener_bind_failed",
                        Some("127.0.0.1"),
                        Some(FALLBACK_CALLBACK_PORT),
                        Some(io_error_code(error.kind())),
                        None,
                    );
                    Err(callback_error())
                }
            }
        }
        Err(error) => {
            record_oauth_event(
                storage,
                "oauth_callback_listener_bind_failed",
                Some("127.0.0.1"),
                Some(DEFAULT_CALLBACK_PORT),
                Some(io_error_code(error.kind())),
                None,
            );
            Err(callback_error())
        }
    }
}

async fn bind_callback_listener_on(port: u16) -> std::io::Result<(TcpListener, u16)> {
    let ipv4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
    Ok((ipv4, port))
}

pub(super) fn record_oauth_event(
    storage: &AppStorage,
    event: &'static str,
    address: Option<&'static str>,
    port: Option<u16>,
    code: Option<&'static str>,
    status: Option<u16>,
) {
    if storage
        .log_oauth_event(event, address, port, code, status)
        .is_err()
    {
        eprintln!("oauth_diagnostic_log_write_failed");
    }
}

fn io_error_code(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::AddrInUse => "address_in_use",
        std::io::ErrorKind::AddrNotAvailable => "address_not_available",
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::Unsupported => "unsupported",
        _ => "other",
    }
}

pub(super) fn create_verifier() -> Zeroizing<String> {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    let verifier = URL_SAFE_NO_PAD.encode(&bytes);
    bytes.zeroize();
    Zeroizing::new(verifier)
}

pub(super) struct Callback {
    pub(super) code: Zeroizing<String>,
}

pub(super) enum CallbackRequestOutcome {
    Accepted(Callback),
    Ignored {
        code: &'static str,
        status: &'static str,
    },
    Rejected(ServiceError),
}

pub(super) async fn read_callback_request_line(
    socket: &mut tokio::net::TcpStream,
) -> Result<Option<Zeroizing<String>>, &'static str> {
    let mut request_bytes = Zeroizing::new(Vec::with_capacity(512));
    let mut chunk = [0u8; 1024];
    loop {
        let byte_count = socket
            .read(&mut chunk)
            .await
            .map_err(|_| "oauth_callback_read_failed")?;
        if byte_count == 0 {
            return Ok(None);
        }
        request_bytes.extend_from_slice(&chunk[..byte_count]);
        if let Some(line_end) = request_bytes
            .windows(2)
            .position(|window| window == b"\r\n")
        {
            if line_end > MAX_CALLBACK_REQUEST_LINE_BYTES {
                return Err("oauth_callback_request_too_large");
            }
            let request_line = std::str::from_utf8(&request_bytes[..line_end])
                .map_err(|_| "oauth_callback_invalid_encoding")?;
            return Ok(Some(Zeroizing::new(request_line.to_owned())));
        }
        if request_bytes.len() > MAX_CALLBACK_REQUEST_LINE_BYTES {
            return Err("oauth_callback_request_too_large");
        }
    }
}

pub(super) fn parse_callback_request(
    request_line: &str,
    expected_state: &str,
    port: u16,
) -> CallbackRequestOutcome {
    let mut request_parts = request_line.split_ascii_whitespace();
    if request_parts.next() != Some("GET") {
        return CallbackRequestOutcome::Ignored {
            code: "oauth_callback_unexpected_method",
            status: "400 Bad Request",
        };
    }
    let Some(target) = request_parts.next() else {
        return CallbackRequestOutcome::Ignored {
            code: "oauth_callback_target_missing",
            status: "400 Bad Request",
        };
    };
    let Some(url) = callback_target_url(target, port) else {
        return CallbackRequestOutcome::Ignored {
            code: "oauth_callback_target_invalid",
            status: "400 Bad Request",
        };
    };
    if url.path() != CALLBACK_PATH {
        return CallbackRequestOutcome::Ignored {
            code: "oauth_callback_path_unmatched",
            status: "404 Not Found",
        };
    }

    let mut code = None;
    let mut state = None;
    let mut provider_error = false;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(Zeroizing::new(value.into_owned())),
            "state" => state = Some(Zeroizing::new(value.into_owned())),
            "error" => provider_error = true,
            _ => {}
        }
    }
    if state.as_ref().map(|value| value.as_str()) != Some(expected_state) {
        return CallbackRequestOutcome::Ignored {
            code: "oauth_state_mismatch",
            status: "400 Bad Request",
        };
    }
    if provider_error {
        return CallbackRequestOutcome::Rejected(ServiceError::new(
            "oauth_denied",
            "ChatGPT sign-in was declined or could not be completed.",
            false,
        ));
    }
    let Some(code) = code.filter(|value| !value.is_empty()) else {
        return CallbackRequestOutcome::Rejected(ServiceError::new(
            "oauth_callback_code_missing",
            "ChatGPT did not include an authorization code in the sign-in callback.",
            false,
        ));
    };
    CallbackRequestOutcome::Accepted(Callback { code })
}

fn callback_target_url(target: &str, port: u16) -> Option<Url> {
    if target.starts_with('/') {
        return Url::parse(&format!("http://127.0.0.1:{port}{target}")).ok();
    }

    let url = Url::parse(target).ok()?;
    let is_loopback_host = matches!(url.host_str(), Some("127.0.0.1") | Some("localhost"));
    if url.scheme() != "http"
        || !is_loopback_host
        || url.port_or_known_default() != Some(port)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(url)
}
