//! HTTP transport for the OpenCode free tier.
//!
//! `opencode.ai` only serves its free models to clients whose TLS ClientHello
//! advertises a specific cipher list. `rustls` (used by `reqwest` everywhere
//! else) cannot offer the CBC and static-RSA suites that list contains, so this
//! path builds its own HTTP/1.1 client on top of OpenSSL with an explicit
//! cipher configuration. Everything else in the service keeps using `reqwest`.

use std::{pin::Pin, sync::OnceLock, time::Duration};

use bytes::Bytes;
use futures_util::{Stream, stream};
use http::{HeaderMap, Method, Request, StatusCode, Uri};
use http_body_util::{BodyExt, Full};
use hyper_openssl::client::legacy::HttpsConnector;
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode, SslVersion};
use openssl::x509::X509;
use tokio::time::Instant;

use crate::protocol::ServiceError;

use super::network_error;

/// TLS 1.3 suites in the order the reference client sends them.
const TLS13_CIPHERS: &str =
    "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256";

/// TLS 1.2 suites in the order the reference client sends them.
const TLS12_CIPHERS: &str = "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:\
ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:\
ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:\
ECDHE-ECDSA-AES128-SHA:ECDHE-RSA-AES128-SHA:\
ECDHE-ECDSA-AES256-SHA:ECDHE-RSA-AES256-SHA:\
AES128-GCM-SHA256:AES256-GCM-SHA384:AES128-SHA:AES256-SHA";

const FREE_TIER_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

type FreeClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;
type FreeBody = Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>>;

static CLIENT: OnceLock<FreeClient> = OnceLock::new();

/// A response whose body streams raw bytes, or an error description.
pub(super) struct FreeResponse {
    pub(super) status: StatusCode,
    pub(super) retry_after: Option<u64>,
    pub(super) body: FreeBody,
}

fn ssl_connector() -> Result<openssl::ssl::SslConnectorBuilder, String> {
    let mut builder = SslConnector::builder(SslMethod::tls()).map_err(|error| error.to_string())?;
    builder
        .set_cipher_list(TLS12_CIPHERS)
        .map_err(|error| error.to_string())?;
    builder
        .set_ciphersuites(TLS13_CIPHERS)
        .map_err(|error| error.to_string())?;
    builder
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .map_err(|error| error.to_string())?;
    builder.set_verify(SslVerifyMode::PEER);

    // Windows does not expose its trust store to OpenSSL by default.
    let native = rustls_native_certs::load_native_certs();
    let store = builder.cert_store_mut();
    for certificate in native.certs {
        if let Ok(certificate) = X509::from_der(certificate.as_ref()) {
            let _ = store.add_cert(certificate);
        }
    }

    Ok(builder)
}

fn client() -> Result<FreeClient, ServiceError> {
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let mut http = HttpConnector::new();
    http.enforce_http(false);
    http.set_nodelay(true);
    http.set_connect_timeout(Some(std::time::Duration::from_secs(15)));
    let connector = HttpsConnector::with_connector(
        http,
        ssl_connector().map_err(|details| {
            network_error().with_details(format!("TLS setup failed: {details}"))
        })?,
    )
    .map_err(|error| network_error().with_details(format!("TLS setup failed: {error}")))?;
    let client = Client::builder(TokioExecutor::new())
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build(connector);
    Ok(CLIENT.get_or_init(|| client).clone())
}

/// Posts `body` to `url` with the supplied headers over the OpenSSL client.
pub(super) async fn send(
    url: &str,
    headers: HeaderMap,
    body: Vec<u8>,
) -> Result<FreeResponse, ServiceError> {
    send_with_timeout(url, headers, body, FREE_TIER_REQUEST_TIMEOUT).await
}

async fn send_with_timeout(
    url: &str,
    headers: HeaderMap,
    body: Vec<u8>,
    timeout: Duration,
) -> Result<FreeResponse, ServiceError> {
    let uri = url
        .parse::<Uri>()
        .map_err(|error| network_error().with_details(format!("invalid provider URL: {error}")))?;
    let mut builder = Request::builder().method(Method::POST).uri(uri);
    for (name, value) in headers.iter() {
        builder = builder.header(name, value);
    }
    builder = builder.header(http::header::CONTENT_LENGTH, body.len());
    let request = builder
        .body(Full::new(Bytes::from(body)))
        .map_err(|error| network_error().with_details(error.to_string()))?;

    let deadline = Instant::now() + timeout;
    let response = tokio::time::timeout_at(deadline, client()?.request(request))
        .await
        .map_err(|_| network_error().with_details("provider request timed out"))?
        .map_err(|error| network_error().with_details(error.to_string()))?;
    let status = response.status();
    let retry_after = response
        .headers()
        .get(http::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| (1..=86_400).contains(seconds));

    let incoming = response.into_body();
    let body = stream::unfold(
        (incoming, deadline, false),
        |(mut incoming, deadline, done)| async move {
            if done {
                return None;
            }
            loop {
                let frame = match tokio::time::timeout_at(deadline, incoming.frame()).await {
                    Ok(frame) => frame,
                    Err(_) => {
                        return Some((
                            Err("provider response timed out".to_owned()),
                            (incoming, deadline, true),
                        ));
                    }
                };
                match frame {
                    None => return None,
                    Some(Err(error)) => {
                        return Some((Err(error.to_string()), (incoming, deadline, true)));
                    }
                    Some(Ok(frame)) => {
                        if let Ok(data) = frame.into_data() {
                            return Some((Ok(data), (incoming, deadline, false)));
                        }
                    }
                }
            }
        },
    );

    Ok(FreeResponse {
        status,
        retry_after,
        body: Box::pin(body),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::StreamExt;
    use http::HeaderMap;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    use super::send_with_timeout;

    #[tokio::test]
    async fn response_body_uses_the_request_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local HTTP fixture");
        let address = listener.local_addr().expect("read fixture address");
        let (started_sender, started_receiver) = oneshot::channel();
        let (release_sender, release_receiver) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept request");
            let mut request = Vec::new();
            loop {
                let mut chunk = [0_u8; 1024];
                let count = socket.read(&mut chunk).await.expect("read request headers");
                assert_ne!(count, 0, "request ended before its headers");
                request.extend_from_slice(&chunk[..count]);
                if request.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n")
                .await
                .expect("write response headers");
            let _ = started_sender.send(());
            let _ = release_receiver.await;
        });

        let mut response = send_with_timeout(
            &format!("http://{address}/v1/chat/completions"),
            HeaderMap::new(),
            Vec::new(),
            Duration::from_millis(500),
        )
        .await
        .expect("receive provider response headers");
        started_receiver
            .await
            .expect("fixture should start the response");

        let chunk = tokio::time::timeout(Duration::from_secs(2), response.body.next())
            .await
            .expect("response body deadline should be bounded");
        assert_eq!(chunk, Some(Err("provider response timed out".to_owned())));

        let _ = release_sender.send(());
        server.await.expect("fixture server should finish");
    }
}
