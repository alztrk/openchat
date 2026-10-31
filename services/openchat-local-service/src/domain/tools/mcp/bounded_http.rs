//! Implements MCP streamable HTTP with strict response and event size bounds.
//!
//! Reserved protocol headers cannot be overridden by server configuration, and response bodies
//! plus server-sent events are incrementally limited before their contents are retained.

use std::{
    collections::HashMap,
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use futures_util::{StreamExt, TryStreamExt, stream::BoxStream};
use http::{HeaderName, HeaderValue, header};
use reqwest::{Client, Response, StatusCode};
use rmcp::{
    model::{ClientJsonRpcMessage, GetMeta, JsonRpcMessage, ServerJsonRpcMessage},
    transport::streamable_http_client::{
        AuthRequiredError, InsufficientScopeError, SseError, StreamableHttpClient,
        StreamableHttpError, StreamableHttpPostResponse,
    },
};
use sse_stream::{Sse, SseStream};
use zeroize::Zeroizing;

const MAX_RESPONSE_BYTES: usize = super::MAX_MCP_RESULT_BYTES;
/// Caps comments and empty separators without limiting the lifetime of a valid idle stream.
const MAX_SSE_NON_EVENT_BYTES_PER_WINDOW: usize = 8 * 1024;
const SSE_NON_EVENT_RATE_WINDOW: Duration = Duration::from_secs(10);
const SESSION_ID: &str = "mcp-session-id";
const LAST_EVENT_ID: &str = "last-event-id";
const PROTOCOL_VERSION: &str = "mcp-protocol-version";
const EVENT_STREAM: &str = "text/event-stream";
const JSON: &str = "application/json";

#[derive(Clone)]
/// MCP HTTP transport that enforces response bounds and shares stored credentials safely.
pub(super) struct BoundedMcpHttpClient {
    client: Client,
    auth_token: Option<Arc<Zeroizing<String>>>,
}

impl BoundedMcpHttpClient {
    /// Wraps a configured client and keeps a saved token in one shared zeroizing allocation.
    pub(super) fn new(client: Client, auth_token: Option<Zeroizing<String>>) -> Self {
        Self {
            client,
            auth_token: auth_token.map(Arc::new),
        }
    }

    fn apply_headers(
        &self,
        mut request: reqwest::RequestBuilder,
        auth: Option<String>,
        session: Option<Arc<str>>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<reqwest::RequestBuilder, StreamableHttpError<reqwest::Error>> {
        let request_auth = auth.map(Zeroizing::new);
        let auth_token = self
            .auth_token
            .as_ref()
            .map(|token| token.as_str())
            .or_else(|| request_auth.as_ref().map(|token| token.as_str()));
        if let Some(token) = auth_token {
            request = request.bearer_auth(token);
        }
        if let Some(session) = session {
            request = request.header(SESSION_ID, session.as_ref());
        }
        for (name, value) in headers {
            if ["accept", SESSION_ID, LAST_EVENT_ID]
                .iter()
                .any(|reserved| name.as_str().eq_ignore_ascii_case(reserved))
            {
                return Err(StreamableHttpError::ReservedHeaderConflict(
                    name.to_string(),
                ));
            }
            request = request.header(name, value);
        }
        Ok(request)
    }
}

/// Collects an HTTP response only while its cumulative body stays within the MCP limit.
async fn read_bounded(response: Response) -> Result<Vec<u8>, StreamableHttpError<reqwest::Error>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(StreamableHttpError::UnexpectedServerResponse(
            "MCP HTTP response exceeded the configured size limit".into(),
        ));
    }
    let mut body = Vec::with_capacity(MAX_RESPONSE_BYTES.min(8 * 1024));
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.try_next().await? {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(StreamableHttpError::UnexpectedServerResponse(
                "MCP HTTP response exceeded the configured size limit".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Copies the server session identifier when it is valid header text.
fn session_id(response: &Response) -> Option<String> {
    response
        .headers()
        .get(SESSION_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn bounded_sse(
    response: Response,
    max_event_size: usize,
) -> BoxStream<'static, Result<Sse, SseError>> {
    let source = response.bytes_stream();
    let bounded = futures_util::stream::unfold(
        (source, SseStreamLimiter::new(max_event_size), false),
        move |(mut source, mut limiter, finished)| async move {
            if finished {
                return None;
            }
            let chunk = source.next().await?;
            match chunk {
                Err(error) => Some((
                    Err(SseError::Body(Box::new(error))),
                    (source, limiter, true),
                )),
                Ok(bytes) => {
                    if limiter.observe(&bytes, Instant::now()).is_err() {
                        let error = io::Error::other(
                            "MCP SSE stream exceeded a configured size or comment-rate limit",
                        );
                        return Some((
                            Err(SseError::Body(Box::new(error))),
                            (source, limiter, true),
                        ));
                    }
                    Some((Ok(bytes), (source, limiter, false)))
                }
            }
        },
    );
    Box::pin(SseStream::from_bytes_stream(bounded))
}

/// Tracks the current event size and the rate of bytes that carry no event payload.
struct SseStreamLimiter {
    max_size: usize,
    retained_size: usize,
    line_size: usize,
    line_is_comment: bool,
    previous_cr: bool,
    non_event_window_started_at: Option<Instant>,
    non_event_bytes_in_window: usize,
}

impl SseStreamLimiter {
    /// Creates a limiter for events no larger than `max_size` bytes.
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            retained_size: 0,
            line_size: 0,
            line_is_comment: false,
            previous_cr: false,
            non_event_window_started_at: None,
            non_event_bytes_in_window: 0,
        }
    }

    /// Accounts for one response chunk before it is passed to the SSE parser.
    ///
    /// Returns `Err(())` when an event or the non-event byte budget exceeds its configured bound.
    fn observe(&mut self, bytes: &[u8], now: Instant) -> Result<(), ()> {
        for &byte in bytes {
            if self.previous_cr {
                self.previous_cr = false;
                if byte == b'\n' {
                    continue;
                }
            }
            match byte {
                b'\r' => {
                    self.finish_line(now)?;
                    self.previous_cr = true;
                }
                b'\n' => self.finish_line(now)?,
                _ => {
                    if self.line_size == 0 {
                        self.line_is_comment = byte == b':';
                    }
                    self.line_size = self.line_size.saturating_add(1);
                    if self.line_is_comment {
                        self.observe_non_event_byte(now)?;
                    }
                    self.check_limit()?;
                }
            }
        }
        Ok(())
    }

    /// Records a line boundary and clears the current line state.
    ///
    /// Returns `Err(())` if this boundary exceeds either configured stream limit.
    fn finish_line(&mut self, now: Instant) -> Result<(), ()> {
        if self.line_size == 0 {
            self.retained_size = 0;
            self.observe_non_event_byte(now)?;
        } else if !self.line_is_comment {
            self.retained_size = self
                .retained_size
                .saturating_add(self.line_size)
                .saturating_add(1);
        } else {
            self.observe_non_event_byte(now)?;
        }
        self.line_size = 0;
        self.line_is_comment = false;
        self.check_limit()
    }

    /// Checks the size of the event accumulated since its previous empty separator.
    ///
    /// Returns `Err(())` when the event exceeds `max_size`.
    fn check_limit(&self) -> Result<(), ()> {
        if self.retained_size.saturating_add(self.line_size) > self.max_size {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Counts a comment byte, comment line ending, or empty separator in a fixed time window.
    ///
    /// Returns `Err(())` once the window's non-event byte budget is exceeded.
    fn observe_non_event_byte(&mut self, now: Instant) -> Result<(), ()> {
        let window_expired = match self.non_event_window_started_at {
            Some(started_at) => {
                now.saturating_duration_since(started_at) >= SSE_NON_EVENT_RATE_WINDOW
            }
            None => true,
        };
        if window_expired {
            self.non_event_window_started_at = Some(now);
            self.non_event_bytes_in_window = 0;
        }

        self.non_event_bytes_in_window = self.non_event_bytes_in_window.saturating_add(1);
        if self.non_event_bytes_in_window > MAX_SSE_NON_EVENT_BYTES_PER_WINDOW {
            Err(())
        } else {
            Ok(())
        }
    }
}

fn message_version(message: &ClientJsonRpcMessage) -> Option<String> {
    if let ClientJsonRpcMessage::Request(request) = message {
        request
            .request
            .get_meta()
            .protocol_version()
            .map(|version| version.to_string())
    } else {
        None
    }
}

impl StreamableHttpClient for BoundedMcpHttpClient {
    type Error = reqwest::Error;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.post_message_with_max_sse_event_size(
            uri,
            message,
            session,
            auth,
            headers,
            MAX_RESPONSE_BYTES,
        )
        .await
    }

    async fn post_message_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let mut request = self
            .client
            .post(uri.as_ref())
            .header(header::ACCEPT, format!("{EVENT_STREAM}, {JSON}"))
            .json(&message);
        if let Some(version) = message_version(&message) {
            request = request.header(PROTOCOL_VERSION, version);
        }
        let response = self
            .apply_headers(request, auth, session.clone(), headers)?
            .send()
            .await?;
        if response.status() == StatusCode::UNAUTHORIZED
            && let Some(challenge) = response
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok())
        {
            return Err(StreamableHttpError::AuthRequired(AuthRequiredError::new(
                challenge.to_owned(),
            )));
        }
        if response.status() == StatusCode::FORBIDDEN
            && let Some(challenge) = response
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok())
        {
            return Err(StreamableHttpError::InsufficientScope(
                InsufficientScopeError::new(challenge.to_owned(), None),
            ));
        }
        let status = response.status();
        if status == StatusCode::ACCEPTED || status == StatusCode::NO_CONTENT {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if status == StatusCode::NOT_FOUND && session.is_some() {
            return Err(StreamableHttpError::SessionExpired);
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let response_session = session_id(&response);
        if !status.is_success() {
            let body = read_bounded(response).await?;
            if content_type
                .as_deref()
                .is_some_and(|value| value.starts_with(JSON))
                && let Ok(message) = serde_json::from_slice::<ServerJsonRpcMessage>(&body)
                && matches!(message, JsonRpcMessage::Error(_))
            {
                return Ok(StreamableHttpPostResponse::Json(message, response_session));
            }
            return Err(StreamableHttpError::UnexpectedServerResponse(
                format!("MCP server returned HTTP {status}").into(),
            ));
        }
        if content_type
            .as_deref()
            .is_some_and(|value| value.starts_with(EVENT_STREAM))
        {
            let stream = bounded_sse(response, max_sse_event_size);
            return Ok(StreamableHttpPostResponse::Sse(stream, response_session));
        }
        if content_type
            .as_deref()
            .is_some_and(|value| value.starts_with(JSON))
        {
            let body = read_bounded(response).await?;
            return match serde_json::from_slice::<ServerJsonRpcMessage>(&body) {
                Ok(message) => Ok(StreamableHttpPostResponse::Json(message, response_session)),
                Err(_) => Ok(StreamableHttpPostResponse::Accepted),
            };
        }
        Err(StreamableHttpError::UnexpectedContentType(content_type))
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        let request = self.client.delete(uri.as_ref());
        let response = self
            .apply_headers(request, auth, Some(session), headers)?
            .send()
            .await?;
        if response.status() != StatusCode::METHOD_NOT_ALLOWED {
            response.error_for_status()?;
        }
        Ok(())
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        self.get_stream_with_max_sse_event_size(
            uri,
            session,
            last_event_id,
            auth,
            headers,
            MAX_RESPONSE_BYTES,
        )
        .await
    }

    async fn get_stream_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        session: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
        max_event_size: usize,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        let mut request = self
            .client
            .get(uri.as_ref())
            .header(header::ACCEPT, format!("{EVENT_STREAM}, {JSON}"));
        if let Some(last_event_id) = last_event_id {
            request = request.header(LAST_EVENT_ID, last_event_id);
        }
        let response = self
            .apply_headers(request, auth, session, headers)?
            .send()
            .await?;
        if response.status() == StatusCode::METHOD_NOT_ALLOWED {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        let response = response.error_for_status()?;
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok());
        if !content_type
            .is_some_and(|value| value.starts_with(EVENT_STREAM) || value.starts_with(JSON))
        {
            return Err(StreamableHttpError::UnexpectedContentType(
                content_type.map(str::to_owned),
            ));
        }
        Ok(bounded_sse(response, max_event_size))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_RESPONSE_BYTES, MAX_SSE_NON_EVENT_BYTES_PER_WINDOW, SSE_NON_EVENT_RATE_WINDOW,
        SseStreamLimiter, bounded_sse, read_bounded,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn sse_stream_limiter_handles_split_crlf_and_multiple_events() {
        let mut limit = SseStreamLimiter::new(9);
        let now = Instant::now();
        assert!(limit.observe(b"data: hi\r", now).is_ok());
        assert!(limit.observe(b"\n\r", now).is_ok());
        assert!(limit.observe(b"\ndata: ok\n\n", now).is_ok());
    }

    #[test]
    fn sse_stream_limiter_rejects_an_oversized_unterminated_event() {
        let mut limit = SseStreamLimiter::new(8);
        assert!(limit.observe(b"data: too long", Instant::now()).is_err());
    }

    #[test]
    fn sse_comment_bytes_are_limited_per_window_across_chunks() {
        let mut limit = SseStreamLimiter::new(MAX_RESPONSE_BYTES);
        let started_at = Instant::now();
        let comment = vec![b':'; MAX_SSE_NON_EVENT_BYTES_PER_WINDOW];

        assert!(limit.observe(&comment, started_at).is_ok());
        assert!(
            limit
                .observe(b":", started_at + Duration::from_secs(1))
                .is_err()
        );
        assert!(
            limit
                .observe(b":", started_at + SSE_NON_EVENT_RATE_WINDOW)
                .is_ok()
        );
    }

    #[test]
    fn sse_empty_event_separators_are_rate_limited() {
        let mut limit = SseStreamLimiter::new(MAX_RESPONSE_BYTES);
        let now = Instant::now();
        let separators = vec![b'\n'; MAX_SSE_NON_EVENT_BYTES_PER_WINDOW];

        assert!(limit.observe(&separators, now).is_ok());
        assert!(limit.observe(b"\n", now).is_err());
    }

    #[test]
    fn sse_heartbeat_comments_support_split_crlf_lines() {
        let mut limit = SseStreamLimiter::new(MAX_RESPONSE_BYTES);
        let now = Instant::now();

        assert!(limit.observe(b": keep-alive\r", now).is_ok());
        assert!(limit.observe(b"\n:\n", now).is_ok());
    }

    /// Shares the zeroizing stored token across client clones and marks its request header sensitive.
    #[test]
    fn saved_auth_token_is_shared_and_sent_as_a_sensitive_header() {
        use std::{collections::HashMap, sync::Arc};

        use http::header::AUTHORIZATION;
        use zeroize::Zeroizing;

        let client = super::BoundedMcpHttpClient::new(
            reqwest::Client::new(),
            Some(Zeroizing::new("fixture-token".to_owned())),
        );
        let cloned_client = client.clone();
        let stored_token = client.auth_token.as_ref().expect("stored token is present");
        let cloned_token = cloned_client
            .auth_token
            .as_ref()
            .expect("cloned client shares its token");
        assert!(Arc::ptr_eq(stored_token, cloned_token));

        let request = client
            .apply_headers(
                client.client.get("https://mcp.example.test"),
                None,
                None,
                HashMap::new(),
            )
            .expect("apply saved authentication")
            .build()
            .expect("build authenticated MCP request");
        let authorization = request
            .headers()
            .get(AUTHORIZATION)
            .expect("authorization header is present");

        assert_eq!(
            authorization.to_str().expect("valid header text"),
            "Bearer fixture-token"
        );
        assert!(authorization.is_sensitive());
    }

    #[test]
    fn custom_headers_cannot_override_reserved_transport_headers() {
        use std::{collections::HashMap, sync::Arc};

        use http::{HeaderName, HeaderValue};

        let client = super::BoundedMcpHttpClient::new(reqwest::Client::new(), None);
        let mut headers = HashMap::new();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("text/plain"),
        );
        let result = client.apply_headers(
            client.client.get("https://example.com"),
            None,
            Some(Arc::from("session")),
            headers,
        );
        assert!(matches!(
            result,
            Err(rmcp::transport::streamable_http_client::StreamableHttpError::ReservedHeaderConflict(name))
                if name == "accept"
        ));
    }

    #[tokio::test]
    async fn json_response_is_rejected_from_oversized_content_length() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        MAX_RESPONSE_BYTES + 1
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });

        let response = reqwest::Client::new()
            .get(format!("http://{address}/mcp"))
            .send()
            .await
            .unwrap();
        assert!(read_bounded(response).await.is_err());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn json_response_stream_is_rejected_when_chunked_body_exceeds_limit() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            socket
                .write_all(format!("{:X}\r\n", MAX_RESPONSE_BYTES).as_bytes())
                .await
                .unwrap();
            socket
                .write_all(&vec![b'a'; MAX_RESPONSE_BYTES])
                .await
                .unwrap();
            socket.write_all(b"\r\n1\r\nb\r\n0\r\n\r\n").await.unwrap();
        });

        let response = reqwest::Client::new()
            .get(format!("http://{address}/mcp"))
            .send()
            .await
            .unwrap();
        assert!(read_bounded(response).await.is_err());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn sse_stream_is_stopped_before_parsing_an_oversized_event() {
        use futures_util::StreamExt;
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let body = b"data: an event larger than eight bytes\n\n";
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(body).await.unwrap();
        });

        let response = reqwest::Client::new()
            .get(format!("http://{address}/mcp"))
            .send()
            .await
            .unwrap();
        let mut stream = bounded_sse(response, 8);
        assert!(stream.next().await.unwrap().is_err());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn sse_stream_is_stopped_after_comment_volume_limit() {
        use futures_util::StreamExt;
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let body = vec![b':'; MAX_SSE_NON_EVENT_BYTES_PER_WINDOW + 1];
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(&body).await.unwrap();
        });

        let response = reqwest::Client::new()
            .get(format!("http://{address}/mcp"))
            .send()
            .await
            .unwrap();
        let mut stream = bounded_sse(response, MAX_RESPONSE_BYTES);
        assert!(stream.next().await.unwrap().is_err());
        assert!(stream.next().await.is_none());
        server.await.unwrap();
    }
}
