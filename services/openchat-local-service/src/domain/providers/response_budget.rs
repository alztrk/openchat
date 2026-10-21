//! Shared byte limits for complete provider responses.

use futures_util::StreamExt;
use reqwest::Response;

use crate::protocol::ServiceError;

/// Maximum raw provider-stream bytes accepted for one assistant response.
pub(crate) const MAX_PROVIDER_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Maximum raw provider response bytes accepted for an automatic conversation title.
pub(crate) const MAX_TITLE_RESPONSE_BYTES: usize = 64 * 1024;

/// Maximum raw response bytes accepted for one provider model catalog request.
pub(crate) const MAX_MODEL_CATALOG_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Reads a provider response body while enforcing a cumulative byte limit.
///
/// The declared length is used only for early rejection and capacity planning; streamed bodies
/// are still counted as they arrive. Network failures and oversized bodies return typed errors.
pub(crate) async fn read_bounded_response_body(
    response: Response,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ServiceError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(response_too_large_error());
    }

    let capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .filter(|length| *length <= maximum_bytes)
        .unwrap_or_default();
    let mut body = Vec::with_capacity(capacity);
    let mut budget = ProviderResponseBudget::with_limit(maximum_bytes);
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|_| response_read_error())?;
        budget.consume(chunk.len())?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Tracks response bytes across provider calls that form one assistant turn.
#[derive(Debug)]
pub(crate) struct ProviderResponseBudget {
    received_bytes: usize,
    maximum_bytes: usize,
}

impl Default for ProviderResponseBudget {
    fn default() -> Self {
        Self {
            received_bytes: 0,
            maximum_bytes: MAX_PROVIDER_RESPONSE_BYTES,
        }
    }
}

impl ProviderResponseBudget {
    /// Accounts for bytes without changing state when the limit would be exceeded.
    pub(crate) fn consume(&mut self, bytes: usize) -> Result<(), ServiceError> {
        let Some(next_total) = self.received_bytes.checked_add(bytes) else {
            return Err(response_too_large_error());
        };
        if next_total > self.maximum_bytes {
            return Err(response_too_large_error());
        }
        self.received_bytes = next_total;
        Ok(())
    }

    pub(crate) fn with_limit(maximum_bytes: usize) -> Self {
        Self {
            received_bytes: 0,
            maximum_bytes,
        }
    }
}

/// Creates the stable error returned when a provider response exceeds its byte budget.
pub(crate) fn response_too_large_error() -> ServiceError {
    ServiceError::new(
        "response_too_large",
        "The provider response exceeded OpenChat's per-response size limit.",
        false,
    )
}

fn response_read_error() -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        "A provider response could not be read completely. Check the network and try again.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        task::JoinHandle,
    };

    use super::{ProviderResponseBudget, read_bounded_response_body};

    /// Serves a fixed HTTP response after consuming the complete test request headers.
    async fn local_response(raw_response: &'static [u8]) -> (reqwest::Response, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local response fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            read_request_headers(&mut stream).await;
            stream
                .write_all(raw_response)
                .await
                .expect("write fixture response");
        });
        let response = reqwest::Client::new()
            .get(format!("http://{address}/catalog"))
            .send()
            .await
            .expect("receive fixture response");
        (response, server)
    }

    /// Reads the loopback request before writing a deterministic fixture response.
    async fn read_request_headers(stream: &mut TcpStream) {
        let mut request = Vec::new();
        let mut byte = [0; 1];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut byte).await.expect("read request headers");
            assert_ne!(read, 0, "client closed before sending request headers");
            request.push(byte[0]);
        }
    }

    #[tokio::test]
    async fn bounded_response_reader_accepts_a_body_at_the_limit() {
        let (response, server) = local_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\njson",
        )
        .await;

        let body = read_bounded_response_body(response, 4)
            .await
            .expect("body at the configured limit should be accepted");

        assert_eq!(body, b"json");
        server.await.expect("fixture server should complete");
    }

    #[tokio::test]
    async fn bounded_response_reader_rejects_declared_length_over_the_limit() {
        let (response, server) = local_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n12345",
        )
        .await;

        let error = read_bounded_response_body(response, 4)
            .await
            .expect_err("declared body above the configured limit should be rejected");

        assert_eq!(error.code, "response_too_large");
        server.await.expect("fixture server should complete");
    }

    #[tokio::test]
    async fn bounded_response_reader_rejects_chunked_bodies_over_the_limit() {
        let (response, server) = local_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n12345\r\n0\r\n\r\n",
        )
        .await;

        let error = read_bounded_response_body(response, 4)
            .await
            .expect_err("chunked body above the configured limit should be rejected");

        assert_eq!(error.code, "response_too_large");
        server.await.expect("fixture server should complete");
    }

    #[test]
    fn accepts_bytes_at_the_limit_and_rejects_the_next_byte() {
        let mut budget = ProviderResponseBudget::with_limit(4);

        budget
            .consume(3)
            .expect("bytes below the limit should be accepted");
        budget
            .consume(1)
            .expect("bytes at the limit should be accepted");
        let error = budget
            .consume(1)
            .expect_err("bytes above the limit should be rejected");

        assert_eq!(error.code, "response_too_large");
        assert_eq!(budget.received_bytes, 4);
    }

    #[test]
    fn cumulative_calls_share_one_budget() {
        let mut budget = ProviderResponseBudget::with_limit(5);

        budget.consume(3).expect("first provider round should fit");
        let error = budget
            .consume(3)
            .expect_err("a later provider round should use the remaining budget");

        assert_eq!(error.code, "response_too_large");
        assert_eq!(budget.received_bytes, 3);
    }

    #[test]
    fn rejects_integer_overflow_without_mutating_the_budget() {
        let mut budget = ProviderResponseBudget::with_limit(usize::MAX);
        budget
            .consume(usize::MAX)
            .expect("maximum initial budget should fit");

        let error = budget
            .consume(1)
            .expect_err("counter overflow should fail closed");

        assert_eq!(error.code, "response_too_large");
        assert_eq!(budget.received_bytes, usize::MAX);
    }
}
