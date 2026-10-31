use std::time::{Duration, Instant};

use super::{
    ProviderBody, ResponseStreamRequest, StreamedTurn, anthropic_event_has_output,
    append_provider_citation_anchors, collect_provider_citations, connection_error,
    local_inference_error, mistral_content_delta, mistral_reference_sources, openai_processing_ms,
    provider_rate_limit_observation, rate_limit_duration_ms, read_error_body_until,
    receive_with_client, request_contains_tool_payload, responses_event_has_output,
};
use crate::usage_statistics::FirstOutputLatencyRecorder;
use crate::{
    openai_compatible::route::ChatRoute,
    protocol::{EventSink, ServiceError},
    response_budget::ProviderResponseBudget,
};
use bytes::Bytes;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{oneshot, watch},
    task::JoinHandle,
};

struct FixtureOutcome {
    result: Result<StreamedTurn, ServiceError>,
    content: String,
    reasoning: String,
    reasoning_summaries: Vec<crate::provider_schema::ReasoningSummary>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    request: Vec<u8>,
    events: Vec<crate::protocol::Response>,
}

#[test]
fn first_output_detection_uses_nonempty_provider_deltas() {
    assert!(responses_event_has_output(&json!({
        "type": "response.output_text.delta",
        "delta": "hello"
    })));
    assert!(!responses_event_has_output(&json!({
        "type": "response.completed",
        "response": {}
    })));
    assert!(anthropic_event_has_output(&json!({
        "type": "content_block_delta",
        "delta": {"type": "input_json_delta", "partial_json": "{}"}
    })));
    assert!(!anthropic_event_has_output(&json!({
        "type": "content_block_delta",
        "delta": {"type": "text_delta", "text": ""}
    })));
}

#[test]
fn provider_processing_time_is_restricted_to_direct_openai_api_responses() {
    let url = reqwest::Url::parse("https://api.openai.com/v1/responses")
        .expect("parse official OpenAI API URL");
    let mut headers = HeaderMap::new();
    headers.insert("openai-processing-ms", HeaderValue::from_static("123"));

    assert_eq!(
        openai_processing_ms(Some("openai"), &url, &headers),
        Some(123)
    );
    assert_eq!(
        openai_processing_ms(Some("chatgpt_api"), &url, &headers),
        Some(123)
    );
    assert_eq!(
        openai_processing_ms(Some("openrouter"), &url, &headers),
        None
    );

    let proxy_url =
        reqwest::Url::parse("https://api.openai.example/v1/responses").expect("parse proxy URL");
    assert_eq!(
        openai_processing_ms(Some("openai"), &proxy_url, &headers),
        None
    );

    let insecure_url =
        reqwest::Url::parse("http://api.openai.com/v1/responses").expect("parse insecure URL");
    assert_eq!(
        openai_processing_ms(Some("openai"), &insecure_url, &headers),
        None
    );

    let custom_port_url = reqwest::Url::parse("https://api.openai.com:8443/v1/responses")
        .expect("parse custom-port URL");
    assert_eq!(
        openai_processing_ms(Some("openai"), &custom_port_url, &headers),
        None
    );

    headers.insert("openai-processing-ms", HeaderValue::from_static("-1"));
    assert_eq!(openai_processing_ms(Some("openai"), &url, &headers), None);
    headers.insert("openai-processing-ms", HeaderValue::from_static("invalid"));
    assert_eq!(openai_processing_ms(Some("openai"), &url, &headers), None);
    headers.insert(
        "openai-processing-ms",
        HeaderValue::from_static("9223372036854775808"),
    );
    assert_eq!(openai_processing_ms(Some("openai"), &url, &headers), None);
}

#[test]
fn provider_rate_limits_are_restricted_to_documented_https_hosts() {
    let mut headers = HeaderMap::new();
    headers.insert("x-ratelimit-limit-requests", HeaderValue::from_static("60"));
    headers.insert(
        "x-ratelimit-remaining-requests",
        HeaderValue::from_static("58"),
    );
    headers.insert("x-ratelimit-reset-requests", HeaderValue::from_static("1s"));
    headers.insert(
        "x-ratelimit-limit-tokens",
        HeaderValue::from_static("150000"),
    );
    headers.insert(
        "x-ratelimit-remaining-tokens",
        HeaderValue::from_static("149984"),
    );
    headers.insert("x-ratelimit-reset-tokens", HeaderValue::from_static("6m0s"));

    let openai_url = reqwest::Url::parse("https://api.openai.com/v1/chat/completions")
        .expect("parse official OpenAI API URL");
    let limits =
        provider_rate_limit_observation(Some("openai"), &openai_url, &headers, Some(1_000))
            .expect("read OpenAI rate-limit headers");
    assert_eq!(limits.observed_at_unix_ms, 1_000);
    assert_eq!(limits.request_limit, Some(60));
    assert_eq!(limits.request_remaining, Some(58));
    assert_eq!(limits.request_reset_at_unix_ms, Some(2_000));
    assert_eq!(limits.token_limit, Some(150_000));
    assert_eq!(limits.token_remaining, Some(149_984));
    assert_eq!(limits.token_reset_at_unix_ms, Some(361_000));

    let groq_url = reqwest::Url::parse("https://api.groq.com/openai/v1/chat/completions")
        .expect("parse official Groq API URL");
    assert!(
        provider_rate_limit_observation(Some("groq"), &groq_url, &headers, Some(1_000)).is_some()
    );
    assert!(
        provider_rate_limit_observation(Some("openai"), &groq_url, &headers, Some(1_000)).is_none()
    );

    let proxy_url = reqwest::Url::parse("https://api.openai.example/v1/chat/completions")
        .expect("parse proxy URL");
    assert!(
        provider_rate_limit_observation(Some("openai"), &proxy_url, &headers, Some(1_000))
            .is_none()
    );
    let insecure_url = reqwest::Url::parse("http://api.openai.com/v1/chat/completions")
        .expect("parse insecure URL");
    assert!(
        provider_rate_limit_observation(Some("openai"), &insecure_url, &headers, Some(1_000))
            .is_none()
    );
    let custom_port_url = reqwest::Url::parse("https://api.openai.com:8443/v1/chat/completions")
        .expect("parse custom-port URL");
    assert!(
        provider_rate_limit_observation(Some("openai"), &custom_port_url, &headers, Some(1_000))
            .is_none()
    );
    assert!(
        provider_rate_limit_observation(Some("custom_openai"), &openai_url, &headers, Some(1_000))
            .is_none()
    );
}

#[test]
fn rate_limit_reset_values_are_bounded_and_parse_documented_durations() {
    assert_eq!(rate_limit_duration_ms("2m59.56s"), Some(179_560));
    assert_eq!(rate_limit_duration_ms("7.66s"), Some(7_660));
    assert_eq!(rate_limit_duration_ms("250ms"), Some(250));
    assert_eq!(rate_limit_duration_ms("1h2m3s"), Some(3_723_000));
    assert_eq!(rate_limit_duration_ms(""), None);
    assert_eq!(rate_limit_duration_ms("-1s"), None);
    assert_eq!(rate_limit_duration_ms("1.5"), None);
    assert_eq!(rate_limit_duration_ms("999999999h"), None);
}

#[test]
fn provider_rate_limits_ignore_invalid_values_and_empty_headers() {
    let url = reqwest::Url::parse("https://api.openai.com/v1/chat/completions")
        .expect("parse official OpenAI API URL");
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-ratelimit-limit-requests",
        HeaderValue::from_static("invalid"),
    );
    headers.insert(
        "x-ratelimit-remaining-requests",
        HeaderValue::from_static("-1"),
    );
    headers.insert(
        "x-ratelimit-reset-requests",
        HeaderValue::from_static("never"),
    );

    assert!(provider_rate_limit_observation(Some("openai"), &url, &headers, Some(1_000)).is_none());

    headers.insert(
        "x-ratelimit-remaining-tokens",
        HeaderValue::from_static("0"),
    );
    let limits = provider_rate_limit_observation(Some("openai"), &url, &headers, Some(1_000))
        .expect("keep valid values when neighboring headers are invalid");
    assert_eq!(limits.token_remaining, Some(0));
    assert_eq!(limits.token_limit, None);
}

async fn run_fixture(
    status: u16,
    response_body: Vec<u8>,
    uses_responses_api: bool,
) -> FixtureOutcome {
    let mut response_budget = ProviderResponseBudget::default();
    run_fixture_with_budget(
        status,
        response_body,
        uses_responses_api,
        &mut response_budget,
    )
    .await
}

async fn run_fixture_with_budget(
    status: u16,
    response_body: Vec<u8>,
    uses_responses_api: bool,
    response_budget: &mut ProviderResponseBudget,
) -> FixtureOutcome {
    run_fixture_with_provider_budget(
        "gemini",
        status,
        response_body,
        uses_responses_api,
        response_budget,
    )
    .await
}

async fn run_fixture_for_provider(
    provider_id: &str,
    status: u16,
    response_body: Vec<u8>,
    uses_responses_api: bool,
) -> FixtureOutcome {
    let mut response_budget = ProviderResponseBudget::default();
    run_fixture_with_provider_budget(
        provider_id,
        status,
        response_body,
        uses_responses_api,
        &mut response_budget,
    )
    .await
}

async fn run_fixture_with_provider_budget(
    provider_id: &str,
    status: u16,
    response_body: Vec<u8>,
    uses_responses_api: bool,
    response_budget: &mut ProviderResponseBudget,
) -> FixtureOutcome {
    let (url, server) = spawn_fixture_server(status, response_body).await;
    let mut route = route(provider_id);
    route.chat_url = url;
    route.is_free = false;
    route.uses_responses_api = uses_responses_api;
    let body = if uses_responses_api {
        json!({
            "model": "test-model",
            "input": [
                {"type": "function_call", "call_id": "prior-call", "name": "read", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "prior-call", "output": "stored result"}
            ],
            "tools": [{"type": "function", "name": "search"}]
        })
    } else {
        json!({
            "model": "test-model",
            "messages": [
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "prior-call",
                        "type": "function",
                        "function": {"name": "read", "arguments": "{}"}
                    }]
                },
                {"role": "tool", "tool_call_id": "prior-call", "content": "stored result"}
            ],
            "tools": [{"type": "function", "function": {"name": "search"}}]
        })
    };
    let client = fixture_client(Duration::from_secs(5));
    let (events, mut event_receiver) = EventSink::test_channel();
    let (_cancel_sender, mut cancellation) = watch::channel(false);
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_summaries = Vec::new();
    let mut output_tokens = None;
    let mut input_tokens = None;
    let request_id = json!("fixture-request");
    let result = receive_for_test(
        &client,
        ResponseStreamRequest {
            route: &route,
            api_key: Some("fixture-api-key"),
            session_id: None,
            body: &body,
            request_id: &request_id,
            conversation_id: "fixture-conversation",
            message_id: "fixture-message",
            created_at: 1,
            content: &mut content,
            reasoning_content: &mut reasoning,
            reasoning_summaries: &mut reasoning_summaries,
            output_tokens: &mut output_tokens,
            input_tokens: &mut input_tokens,
            started: Instant::now(),
            first_output_recorder: None,
            rate_limit_observation: None,
            cancellation: &mut cancellation,
            events: &events,
            response_budget,
        },
    )
    .await;
    let mut emitted_events = Vec::new();
    while let Ok(event) = event_receiver.try_recv() {
        emitted_events.push(event);
    }
    let request = server.await.expect("fixture server should finish");
    FixtureOutcome {
        result,
        content,
        reasoning,
        reasoning_summaries,
        input_tokens,
        output_tokens,
        request,
        events: emitted_events,
    }
}

#[tokio::test]
async fn openai_api_surfaces_summary_groups_and_ignores_reasoning_text() {
    let mut response = Vec::new();
    response.extend(sse_event(
        json!({"type": "response.reasoning_text.delta", "delta": "private reasoning"}),
        true,
    ));
    response.extend(sse_event(
        json!({
            "type": "response.reasoning_summary_text.delta",
            "item_id": "rs_item",
            "summary_index": 0,
            "delta": "First summary"
        }),
        true,
    ));
    response.extend(sse_event(
        json!({
            "type": "response.reasoning_summary_text.delta",
            "item_id": "rs_item",
            "summary_index": 1,
            "delta": "Second summary"
        }),
        true,
    ));
    response.extend(sse_event(
        json!({
            "type": "response.reasoning_summary_text.done",
            "item_id": "rs_item",
            "summary_index": 0,
            "text": "First summary"
        }),
        true,
    ));
    response.extend(sse_event(
        json!({
            "type": "response.reasoning_summary_part.done",
            "item_id": "rs_item",
            "summary_index": 1,
            "part": {"type": "summary_text", "text": "Second summary"}
        }),
        true,
    ));
    response.extend(sse_event(
        json!({"type": "response.output_text.delta", "delta": "Final answer"}),
        true,
    ));
    response.extend(sse_event(
        json!({
            "type": "response.completed",
            "response": {"output": [], "usage": {"output_tokens": 1}}
        }),
        true,
    ));

    let outcome = run_fixture_for_provider("chatgpt_api", 200, response, true).await;

    assert!(
        outcome.result.is_ok(),
        "provider error: {:?}",
        outcome.result.as_ref().err().map(|error| error.code)
    );
    assert_eq!(outcome.content, "Final answer");
    assert!(outcome.reasoning.is_empty());
    assert_eq!(outcome.reasoning_summaries.len(), 2);
    assert_eq!(outcome.reasoning_summaries[0].content, "First summary");
    assert_eq!(outcome.reasoning_summaries[1].content, "Second summary");
    assert!(
        outcome
            .reasoning_summaries
            .iter()
            .all(|summary| summary.is_complete)
    );
}

async fn receive_for_test(
    client: &Client,
    request: ResponseStreamRequest<'_>,
) -> Result<StreamedTurn, ServiceError> {
    receive_with_client(request, client).await
}

fn fixture_client(timeout: Duration) -> Client {
    Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(timeout)
        .build()
        .expect("build local fixture client")
}

#[tokio::test]
async fn stalled_provider_error_body_respects_its_deadline() {
    let mut body: ProviderBody = Box::pin(futures_util::stream::pending::<Result<Bytes, String>>());
    let (_cancel_sender, mut cancellation) = watch::channel(false);

    let result =
        read_error_body_until(&mut body, &mut cancellation, tokio::time::Instant::now()).await;

    assert!(matches!(result, Ok(None)));
}

#[tokio::test]
async fn provider_error_body_read_observes_cancellation() {
    let mut body: ProviderBody = Box::pin(futures_util::stream::pending::<Result<Bytes, String>>());
    let (cancel_sender, mut cancellation) = watch::channel(false);
    cancel_sender
        .send(true)
        .expect("cancellation receiver should be active");

    let error = read_error_body_until(
        &mut body,
        &mut cancellation,
        tokio::time::Instant::now() + Duration::from_secs(1),
    )
    .await
    .expect_err("cancelled error-body reads must stop");

    assert_eq!(error.code, "request_cancelled");
}

async fn spawn_fixture_server(
    status: u16,
    response_body: Vec<u8>,
) -> (String, JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local HTTP fixture");
    let address = listener.local_addr().expect("read fixture address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept fixture request");
        let request = read_http_request(&mut stream).await;
        let headers = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response_body.len()
        );
        stream
            .write_all(headers.as_bytes())
            .await
            .expect("write fixture response headers");
        stream
            .write_all(&response_body)
            .await
            .expect("write fixture response body");
        request
    });
    (format!("http://{address}/v1/chat/completions"), server)
}

async fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    loop {
        let mut chunk = [0_u8; 4096];
        let count = stream.read(&mut chunk).await.expect("read fixture request");
        assert_ne!(count, 0, "fixture request ended before its body");
        request.extend_from_slice(&chunk[..count]);
        let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            return request;
        }
    }
}

fn sse_event(value: serde_json::Value, terminate_line: bool) -> Vec<u8> {
    let suffix = if terminate_line { "\n" } else { "" };
    format!("data: {value}{suffix}").into_bytes()
}

fn request_headers_and_body(request: &[u8]) -> (String, serde_json::Value) {
    let header_end = request
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .expect("fixture request should contain headers");
    let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
    let body = serde_json::from_slice(&request[header_end + 4..])
        .expect("fixture request body should be JSON");
    (headers, body)
}

fn expect_service_error<T>(result: Result<T, ServiceError>) -> ServiceError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected provider operation to fail"),
    }
}

async fn spawn_pending_stream_server(
    initial_body: Vec<u8>,
    remaining_body: Vec<u8>,
) -> (
    String,
    oneshot::Receiver<()>,
    oneshot::Sender<()>,
    JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind pending stream fixture");
    let address = listener.local_addr().expect("read fixture address");
    let (started_sender, started_receiver) = oneshot::channel();
    let (continue_sender, continue_receiver) = oneshot::channel();
    let body_length = initial_body.len() + remaining_body.len();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept fixture request");
        let _request = read_http_request(&mut stream).await;
        let headers = format!(
            "HTTP/1.1 200 Fixture\r\nContent-Type: text/event-stream\r\nContent-Length: {body_length}\r\n\r\n"
        );
        stream
            .write_all(headers.as_bytes())
            .await
            .expect("write pending response headers");
        stream
            .write_all(&initial_body)
            .await
            .expect("write initial response event");
        let _ = started_sender.send(());
        let _ = continue_receiver.await;
        let _ = stream.write_all(&remaining_body).await;
    });
    (
        format!("http://{address}/v1/chat/completions"),
        started_receiver,
        continue_sender,
        server,
    )
}

#[test]
fn recognizes_tools_in_definitions_and_both_history_wire_formats() {
    assert!(request_contains_tool_payload(&json!({
        "tools": [{"type": "function"}]
    })));
    assert!(request_contains_tool_payload(&json!({
        "messages": [{"role": "assistant", "tool_calls": [{"id": "call_1"}]}]
    })));
    assert!(request_contains_tool_payload(&json!({
        "messages": [{"role": "tool", "content": "done"}]
    })));
    assert!(request_contains_tool_payload(&json!({
        "input": [{"type": "function_call_output"}]
    })));
    assert!(!request_contains_tool_payload(&json!({
        "tools": [],
        "messages": [{"role": "user", "content": "hello"}],
        "input": [{"type": "message", "role": "user"}]
    })));
}

#[test]
fn parses_provider_url_citations_and_appends_local_references() {
    let value = json!({
        "choices": [{
            "delta": {
                "annotations": [
                    {
                        "type": "url_citation",
                        "url": "https://example.org/article",
                        "title": "Example article"
                    },
                    {
                        "type": "url_citation",
                        "url": "javascript:alert(1)",
                        "title": "Unsafe"
                    }
                ]
            }
        }]
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();
    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["id"], "P1");
    assert_eq!(sources[0]["sourceType"], "provider_native");

    let mut content = "A sourced claim.".to_owned();
    let mut round_content = "A sourced claim.".to_owned();
    append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);
    assert_eq!(content, "A sourced claim. [P1]");
    assert_eq!(round_content, content);
}

#[test]
fn parses_openai_nested_url_citations() {
    let value = json!({
        "choices": [{
            "message": {
                "annotations": [{
                    "type": "url_citation",
                    "url_citation": {
                        "url": "https://example.org/article",
                        "title": "Example article",
                        "start_index": 4,
                        "end_index": 18
                    }
                }]
            }
        }]
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();
    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["title"], "Example article");
    assert_eq!(sources[0]["url"], "https://example.org/article");
    assert_eq!(sources[0]["sourceType"], "provider_native");
}

#[test]
fn parses_gemini_grounding_chunks_and_places_sources_after_supported_text() {
    let value = json!({
        "candidates": [{
            "content": {"parts": [{"text": "Claim. More text."}]},
            "groundingMetadata": {
                "groundingChunks": [
                    {"web": {"uri": "https://example.org/grounded", "title": "Grounded source"}},
                    {"web": {"uri": "https://example.org/other", "title": "Other source"}}
                ],
                "groundingSupports": [{
                    "segment": {"partIndex": 0, "endIndex": 6, "text": "Claim."},
                    "groundingChunkIndices": [0]
                }]
            }
        }]
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();

    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0]["title"], "Grounded source");
    assert_eq!(positions.get("P1"), Some(&6));
    assert!(!positions.contains_key("P2"));
    let mut content = "Claim. More text.".to_owned();
    let mut round_content = content.clone();
    append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);
    assert_eq!(content, "Claim. [P1] More text. [P2]");
}

#[test]
fn converts_gemini_utf8_byte_offsets_to_character_positions() {
    let value = json!({
        "candidates": [{
            "content": {"parts": [{"text": "İzmir güzel."}]},
            "groundingMetadata": {
                "groundingChunks": [{"web": {
                    "uri": "https://example.org/izmir",
                    "title": "İzmir source"
                }}],
                "groundingSupports": [{
                    "segment": {"partIndex": 0, "endIndex": 6, "text": "İzmir"},
                    "groundingChunkIndices": [0]
                }]
            }
        }]
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();

    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

    let mut content = "İzmir güzel.".to_owned();
    let mut round_content = content.clone();
    append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);
    assert_eq!(content, "İzmir [P1] güzel.");
}

#[test]
fn rejects_unsafe_gemini_grounding_chunk_urls() {
    let value = json!({
        "candidates": [{
            "content": {"parts": [{"text": "Claim."}]},
            "groundingMetadata": {
                "groundingChunks": [
                    {"web": {"uri": "file:///private/data", "title": "Local file"}},
                    {"web": {"uri": "https://user:pass@example.org/private", "title": "Credential URL"}}
                ],
                "groundingSupports": []
            }
        }]
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();

    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

    assert!(sources.is_empty());
    assert!(positions.is_empty());
}

#[test]
fn ignores_gemini_grounding_chunks_beyond_the_metadata_limit() {
    let mut chunks = vec![json!({"retrievedContext": {"title": "Ignored context"}}); 64];
    chunks.push(json!({"web": {
        "uri": "https://example.org/over-limit",
        "title": "Over-limit source"
    }}));
    let value = json!({
        "candidates": [{
            "content": {"parts": [{"text": "Claim."}]},
            "groundingMetadata": {
                "groundingChunks": chunks,
                "groundingSupports": [{
                    "segment": {"partIndex": 0, "endIndex": 6},
                    "groundingChunkIndices": [64]
                }]
            }
        }]
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();

    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

    assert!(sources.is_empty());
    assert!(positions.is_empty());
}

#[test]
fn places_provider_citations_at_valid_unicode_text_offsets() {
    let value = json!({
        "choices": [{
            "message": {
                "annotations": [{
                    "type": "url_citation",
                    "url_citation": {
                        "url": "https://example.org/article",
                        "title": "Example article",
                        "start_index": 0,
                        "end_index": 2
                    }
                }]
            }
        }]
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();
    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

    let mut content = "Earlier. Été is warm.".to_owned();
    let mut round_content = "Été is warm.".to_owned();
    append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);

    assert_eq!(content, "Earlier. Été [P1] is warm.");
    assert_eq!(round_content, "Été [P1] is warm.");
}

#[test]
fn appends_provider_citations_when_offsets_are_missing_or_out_of_range() {
    let value = json!({
        "choices": [{
            "message": {
                "annotations": [{
                    "type": "url_citation",
                    "url_citation": {
                        "url": "https://example.org/article",
                        "title": "Example article",
                        "end_index": 9000
                    }
                }]
            }
        }]
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();
    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

    let mut content = "Answer.".to_owned();
    let mut round_content = "Answer.".to_owned();
    append_provider_citation_anchors(&mut content, &mut round_content, &sources, &positions);

    assert_eq!(content, "Answer. [P1]");
    assert_eq!(round_content, "Answer. [P1]");
}

#[test]
fn parses_responses_api_completed_output_citations() {
    let value = json!({
        "type": "response.completed",
        "response": {
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": "A sourced answer.",
                    "annotations": [{
                        "type": "url_citation",
                        "url": "https://example.org/source",
                        "title": "Provider source"
                    }]
                }]
            }]
        }
    });
    let mut ids = std::collections::BTreeMap::new();
    let mut sources = Vec::new();
    let mut positions = std::collections::BTreeMap::new();

    collect_provider_citations(&value, &mut ids, &mut sources, &mut positions);

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["url"], "https://example.org/source");
    assert_eq!(sources[0]["title"], "Provider source");
}

fn route(provider_id: &str) -> ChatRoute {
    ChatRoute {
        custom_enable_thinking: false,
        model_id: "test-model".to_owned(),
        provider_model_id: "test-model".to_owned(),
        provider_id: Some(provider_id.to_owned()),
        chat_url: String::new(),
        is_free: true,
        is_opencode: false,
        is_copilot: false,
        uses_responses_api: false,
        reasoning_levels: Vec::new(),
        context_window: None,
        input_token_limit: None,
        max_output_tokens: None,
        context_limit_override: None,
        supports_images: false,
        supports_tool_calls: None,
        connection_id: None,
        local_api_key: None,
        project_id: None,
    }
}

#[test]
fn identifies_local_runtime_disconnects_separately_from_provider_network_errors() {
    assert_eq!(
        connection_error(&route("llama_cpp"), "connection reset".to_owned()).code,
        "local_engine_runtime_unavailable"
    );
    assert_eq!(
        connection_error(&route("opencode"), "connection reset".to_owned()).code,
        "network_unavailable"
    );
}

#[test]
fn reports_local_server_request_rejections_as_model_inference_errors() {
    assert_eq!(local_inference_error().code, "local_model_inference_failed");
}

#[test]
fn parses_mistral_thinking_and_text_content_parts() {
    let delta = mistral_content_delta(&json!({
        "choices": [{
            "delta": {
                "content": [
                    {
                        "type": "thinking",
                        "thinking": [{"type": "text", "text": "First step. "}]
                    },
                    {"type": "text", "text": "Answer."}
                ]
            }
        }]
    }));

    assert_eq!(delta.reasoning, "First step. ");
    assert_eq!(delta.text, "Answer.");
}

#[test]
fn parses_plain_mistral_text_deltas() {
    let delta = mistral_content_delta(&json!({
        "choices": [{"delta": {"content": "Answer."}}]
    }));

    assert!(delta.reasoning.is_empty());
    assert_eq!(delta.text, "Answer.");
}

#[test]
fn maps_mistral_reference_ids_to_one_tool_result_and_skips_ambiguous_results() {
    let body = json!({
        "messages": [{
            "role": "tool",
            "content": "{\"0\":{\"sourceId\":\"S1-call1234\"},\"1\":{\"sourceId\":\"S2-call1234\"}}"
        }]
    });
    assert_eq!(
        mistral_reference_sources(&body),
        ["S1-call1234", "S2-call1234"]
    );

    let ambiguous = json!({
        "messages": [
            {"role": "tool", "content": "{\"sourceId\":\"U1-call1234\"}"},
            {"role": "tool", "content": "{\"results\":[{\"sourceId\":\"S1-call5678\"}]}"}
        ]
    });
    assert!(mistral_reference_sources(&ambiguous).is_empty());
}

#[tokio::test]
async fn chat_completions_fixture_flushes_final_event_and_orders_tool_calls() {
    let mut response_body = Vec::new();
    response_body.extend(sse_event(
        json!({"choices": [{"delta": {"content": "Searching."}}]}),
        true,
    ));
    response_body.extend(sse_event(
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 1,
            "id": "call_second",
            "function": {"name": "read", "arguments": "{\"path\":\"notes.md\"}"}
        }]}}]}),
        true,
    ));
    response_body.extend(sse_event(
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 0,
            "id": "call_first",
            "function": {"name": "search", "arguments": "{\"query\":\"rust\"}"}
        }]}}]}),
        true,
    ));
    response_body.extend(sse_event(
        json!({
            "choices": [{"finish_reason": "tool_calls", "delta": {}}],
            "usage": {"prompt_tokens": 11, "completion_tokens": 3}
        }),
        false,
    ));

    let outcome = run_fixture(200, response_body, false).await;
    let turn = outcome.result.expect("fixture stream should complete");
    assert!(turn.first_output_latency.is_some());
    assert_eq!(outcome.content, "Searching.");
    assert_eq!(turn.round_content, "Searching.");
    let text_event = outcome
        .events
        .iter()
        .find(|event| event.event.as_deref() == Some("chat.text.delta"))
        .expect("provider text should be streamed as an append-only event");
    let text_data = text_event
        .data
        .as_ref()
        .expect("text delta should contain event data");
    assert_eq!(text_data["delta"], "Searching.");
    assert!(text_data.get("content").is_none());
    assert_eq!(outcome.input_tokens, Some(11));
    assert_eq!(outcome.output_tokens, Some(3));
    assert_eq!(turn.tool_calls.len(), 2);
    assert_eq!(turn.tool_calls[0].id, "call_first");
    assert_eq!(turn.tool_calls[0].name, "search");
    assert_eq!(turn.tool_calls[0].arguments["query"], "rust");
    assert_eq!(turn.tool_calls[1].id, "call_second");
    assert_eq!(turn.tool_calls[1].name, "read");
    assert_eq!(turn.tool_calls[1].arguments["path"], "notes.md");

    let (headers, body) = request_headers_and_body(&outcome.request);
    assert!(headers.starts_with("post /v1/chat/completions "));
    assert!(headers.contains("authorization: bearer fixture-api-key"));
    assert_eq!(body["messages"][1]["tool_call_id"], "prior-call");
    assert_eq!(body["messages"][1]["content"], "stored result");
    assert_eq!(body["tools"][0]["function"]["name"], "search");
    assert!(outcome.reasoning.is_empty());
}

#[tokio::test]
async fn gemini_grounding_metadata_streams_clickable_citation_sources() {
    let mut response_body = Vec::new();
    response_body.extend(sse_event(
        json!({
            "candidates": [{
                "content": {"parts": [{"text": "Claim. More text."}]},
                "groundingMetadata": {
                    "groundingChunks": [{"web": {
                        "uri": "https://example.org/grounded",
                        "title": "Grounded source"
                    }}],
                    "groundingSupports": [{
                        "segment": {"partIndex": 0, "endIndex": 6, "text": "Claim."},
                        "groundingChunkIndices": [0]
                    }]
                }
            }]
        }),
        true,
    ));
    response_body.extend(sse_event(
        json!({"choices": [{"delta": {}, "finish_reason": "stop"}]}),
        true,
    ));
    response_body.extend(b"data: [DONE]\n\n".iter().copied());

    let outcome = run_fixture(200, response_body, false).await;

    assert!(
        outcome.result.is_ok(),
        "provider error: {:?}",
        outcome.result.as_ref().err().map(|error| error.code)
    );
    assert_eq!(outcome.content, "Claim. [P1] More text.");
    let citation_event = outcome
        .events
        .iter()
        .find(|event| event.event.as_deref() == Some("chat.citations.updated"))
        .expect("stream should emit the citation update");
    assert_eq!(
        citation_event.data.as_ref().unwrap()["sources"][0]["id"],
        "P1"
    );
    assert_eq!(
        citation_event.data.as_ref().unwrap()["sources"][0]["url"],
        "https://example.org/grounded"
    );
}

#[tokio::test]
async fn responses_fixture_reassembles_tool_arguments_and_unterminated_completion() {
    let mut response_body = Vec::new();
    response_body.extend(sse_event(
        json!({
            "type": "response.output_item.added",
            "output_index": 0,
            "item": {"type": "function_call", "call_id": "call_response", "name": "search"}
        }),
        true,
    ));
    response_body.extend(sse_event(
        json!({
            "type": "response.function_call_arguments.delta",
            "output_index": 0,
            "delta": "{\"query\":\"docs\"}"
        }),
        true,
    ));
    response_body.extend(sse_event(
        json!({"type": "response.output_text.delta", "delta": "Found it."}),
        true,
    ));
    response_body.extend(sse_event(
        json!({
            "type": "response.completed",
            "response": {
                "output": [{
                    "type": "function_call",
                    "call_id": "call_response",
                    "name": "search",
                    "arguments": "{\"query\":\"docs\"}"
                }],
                "usage": {"input_tokens": 20, "output_tokens": 4}
            }
        }),
        false,
    ));

    let outcome = run_fixture(200, response_body, true).await;
    let turn = outcome.result.expect("Responses fixture should complete");
    assert!(turn.first_output_latency.is_some());
    assert_eq!(outcome.content, "Found it.");
    assert_eq!(outcome.input_tokens, Some(20));
    assert_eq!(outcome.output_tokens, Some(4));
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].id, "call_response");
    assert_eq!(turn.tool_calls[0].arguments["query"], "docs");

    let (headers, body) = request_headers_and_body(&outcome.request);
    assert!(headers.starts_with("post /v1/chat/completions "));
    assert!(body.get("messages").is_none());
    assert_eq!(body["input"][1]["type"], "function_call_output");
}

#[tokio::test]
async fn malformed_and_incomplete_sse_streams_return_safe_provider_errors() {
    let malformed = run_fixture(200, b"data: {not-json}\n".to_vec(), false).await;
    let malformed_error = expect_service_error(malformed.result);
    assert_eq!(malformed_error.code, "invalid_provider_response");
    assert!(!malformed_error.message.contains("not-json"));

    let incomplete = run_fixture(
        200,
        sse_event(
            json!({"choices": [{"delta": {"content": "partial"}}]}),
            false,
        ),
        false,
    )
    .await;
    let incomplete_error = expect_service_error(incomplete.result);
    assert_eq!(incomplete_error.code, "invalid_provider_response");
    assert_eq!(incomplete.content, "partial");
}

#[tokio::test]
async fn provider_response_budget_is_shared_across_follow_up_turns() {
    let mut first_response = Vec::new();
    first_response.extend(sse_event(
        json!({"choices": [{"delta": {"content": "first turn"}}]}),
        true,
    ));
    first_response.extend(sse_event(
        json!({"choices": [{"finish_reason": "tool_calls", "delta": {}}]}),
        true,
    ));
    let mut response_budget = ProviderResponseBudget::with_limit(first_response.len());

    let first = run_fixture_with_budget(200, first_response, false, &mut response_budget).await;
    assert!(first.result.is_ok());
    assert_eq!(first.content, "first turn");

    let second = run_fixture_with_budget(
        200,
        sse_event(
            json!({"choices": [{"delta": {"content": "follow-up turn"}}]}),
            true,
        ),
        false,
        &mut response_budget,
    )
    .await;

    let error = expect_service_error(second.result);
    assert_eq!(error.code, "response_too_large");
}

#[tokio::test]
async fn oversized_provider_stream_keeps_content_received_within_the_budget() {
    let initial = sse_event(
        json!({"choices": [{"delta": {"content": "</think>partial"}}]}),
        true,
    );
    let remaining = sse_event(
        json!({"choices": [{"delta": {"content": "over limit"}}]}),
        true,
    );
    let (url, started, continue_server, server) =
        spawn_pending_stream_server(initial.clone(), remaining).await;
    let mut route = route("gemini");
    route.chat_url = url;
    route.is_free = false;
    let client = fixture_client(Duration::from_secs(5));
    let (events, mut event_receiver) = EventSink::test_channel();
    let (_cancel_sender, mut cancellation) = watch::channel(false);
    let body = json!({"model": "test-model", "messages": []});
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_summaries = Vec::new();
    let mut output_tokens = None;
    let mut input_tokens = None;
    let request_id = json!("fixture-request");
    let mut response_budget = ProviderResponseBudget::with_limit(initial.len());
    let first_output_recorder = FirstOutputLatencyRecorder::default();
    let mut receive = Box::pin(receive_for_test(
        &client,
        ResponseStreamRequest {
            route: &route,
            api_key: Some("fixture-api-key"),
            session_id: None,
            body: &body,
            request_id: &request_id,
            conversation_id: "fixture-conversation",
            message_id: "fixture-message",
            created_at: 1,
            content: &mut content,
            reasoning_content: &mut reasoning,
            reasoning_summaries: &mut reasoning_summaries,
            output_tokens: &mut output_tokens,
            input_tokens: &mut input_tokens,
            started: Instant::now(),
            first_output_recorder: Some(&first_output_recorder),
            rate_limit_observation: None,
            cancellation: &mut cancellation,
            events: &events,
            response_budget: &mut response_budget,
        },
    ));

    tokio::select! {
        started = started => {
            started.expect("fixture server should remain active");
        }
        result = &mut receive => {
            let _ = result;
            panic!("stream ended before the initial event was released");
        }
    }
    tokio::select! {
        event = tokio::time::timeout(Duration::from_secs(2), event_receiver.recv()) => {
            event
                .expect("the initial event should be processed before the next chunk")
                .expect("the initial content delta should emit an event");
        }
        result = &mut receive => {
            let _ = result;
            panic!("stream ended before the initial content was confirmed");
        }
    }
    continue_server
        .send(())
        .expect("release fixture server after the initial content is processed");

    let error = expect_service_error(receive.await);
    assert_eq!(error.code, "response_too_large");
    assert_eq!(content, "partial");
    assert!(first_output_recorder.value().is_some());
    server.await.expect("fixture server should finish");
}

#[tokio::test]
async fn opencode_free_requests_send_affinity_headers_and_public_auth() {
    let response = sse_event(
        json!({"choices": [{"finish_reason": "stop", "delta": {"content": "ok"}}]}),
        true,
    );
    let (url, server) = spawn_fixture_server(200, response).await;
    let mut route = route("opencode");
    route.chat_url = url;
    route.is_opencode = true;
    route.is_free = true;
    route.project_id = Some("project-test".to_owned());
    let client = fixture_client(Duration::from_secs(5));
    let (events, _event_receiver) = EventSink::test_channel();
    let (_cancel_sender, mut cancellation) = watch::channel(false);
    let body = json!({"model": "big-pickle", "messages": [{"role": "user", "content": "hi"}]});
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_summaries = Vec::new();
    let mut output_tokens = None;
    let mut input_tokens = None;
    let request_id = json!("fixture-request");
    let mut response_budget = ProviderResponseBudget::default();

    let result = receive_for_test(
        &client,
        ResponseStreamRequest {
            route: &route,
            api_key: None,
            session_id: Some("ses_testaffinity"),
            body: &body,
            request_id: &request_id,
            conversation_id: "fixture-conversation",
            message_id: "fixture-message",
            created_at: 1,
            content: &mut content,
            reasoning_content: &mut reasoning,
            reasoning_summaries: &mut reasoning_summaries,
            output_tokens: &mut output_tokens,
            input_tokens: &mut input_tokens,
            started: Instant::now(),
            first_output_recorder: None,
            rate_limit_observation: None,
            cancellation: &mut cancellation,
            events: &events,
            response_budget: &mut response_budget,
        },
    )
    .await;

    assert!(result.is_ok());
    let request = server.await.expect("fixture server should finish");
    let request = String::from_utf8(request)
        .expect("request bytes are UTF-8")
        .to_ascii_lowercase();
    assert!(request.contains("authorization: bearer public"));
    assert!(request.contains("x-opencode-session-id: ses_testaffinity"));
    assert!(request.contains("x-session-affinity: ses_testaffinity"));
    assert!(request.contains("x-session-id: ses_testaffinity"));
    assert!(request.contains("x-opencode-session: ses_testaffinity"));
    assert!(request.contains("x-opencode-client: cli"));
    assert!(request.contains("user-agent: opencode/latest/2.1.4/cli"));
    assert!(request.contains("x-opencode-project: project-test"));
}

#[tokio::test]
async fn opencode_free_usage_limit_returns_a_specific_error() {
    let response = br#"{"type":"error","error":{"type":"FreeUsageLimitError","message":"Free usage limit reached. Free models work in OpenCode."}}"#.to_vec();
    let (url, server) = spawn_fixture_server(429, response).await;
    let mut route = route("opencode");
    route.chat_url = url;
    route.is_opencode = true;
    route.is_free = true;
    let client = fixture_client(Duration::from_secs(5));
    let (events, _event_receiver) = EventSink::test_channel();
    let (_cancel_sender, mut cancellation) = watch::channel(false);
    let body = json!({"model": "big-pickle", "messages": []});
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_summaries = Vec::new();
    let mut output_tokens = None;
    let mut input_tokens = None;
    let request_id = json!("fixture-request");
    let mut response_budget = ProviderResponseBudget::default();

    let error = expect_service_error(
        receive_for_test(
            &client,
            ResponseStreamRequest {
                route: &route,
                api_key: None,
                session_id: Some("ses_testaffinity"),
                body: &body,
                request_id: &request_id,
                conversation_id: "fixture-conversation",
                message_id: "fixture-message",
                created_at: 1,
                content: &mut content,
                reasoning_content: &mut reasoning,
                reasoning_summaries: &mut reasoning_summaries,
                output_tokens: &mut output_tokens,
                input_tokens: &mut input_tokens,
                started: Instant::now(),
                first_output_recorder: None,
                rate_limit_observation: None,
                cancellation: &mut cancellation,
                events: &events,
                response_budget: &mut response_budget,
            },
        )
        .await,
    );
    assert_eq!(error.code, "opencode_free_usage_limit");
    assert!(!error.retryable);
    server.await.expect("fixture server should finish");
}

#[tokio::test]
async fn local_http_status_fixtures_map_auth_quota_and_server_failures() {
    for (status, expected_code) in [
        (401, "authentication_required"),
        (429, "rate_limited"),
        (503, "provider_request_failed"),
    ] {
        let outcome = run_fixture(status, b"provider-secret-response-body".to_vec(), false).await;
        let error = expect_service_error(outcome.result);
        assert_eq!(error.code, expected_code);
        assert!(!error.message.contains("provider-secret-response-body"));
    }
}

#[tokio::test]
async fn cancellation_interrupts_an_open_local_provider_stream() {
    let initial = sse_event(
        json!({"choices": [{"delta": {"content": "partial"}}]}),
        true,
    );
    let remaining = sse_event(
        json!({"choices": [{"finish_reason": "stop", "delta": {}}]}),
        true,
    );
    let (url, started, continue_server, server) =
        spawn_pending_stream_server(initial, remaining).await;
    let mut route = route("gemini");
    route.chat_url = url;
    route.is_free = false;
    let client = fixture_client(Duration::from_secs(5));
    let (events, _event_receiver) = EventSink::test_channel();
    let (cancel_sender, mut cancellation) = watch::channel(false);
    let body = json!({"model": "test-model", "messages": []});
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_summaries = Vec::new();
    let mut output_tokens = None;
    let mut input_tokens = None;
    let request_id = json!("fixture-request");
    let mut response_budget = ProviderResponseBudget::default();
    let mut receive = Box::pin(receive_for_test(
        &client,
        ResponseStreamRequest {
            route: &route,
            api_key: Some("fixture-api-key"),
            session_id: None,
            body: &body,
            request_id: &request_id,
            conversation_id: "fixture-conversation",
            message_id: "fixture-message",
            created_at: 1,
            content: &mut content,
            reasoning_content: &mut reasoning,
            reasoning_summaries: &mut reasoning_summaries,
            output_tokens: &mut output_tokens,
            input_tokens: &mut input_tokens,
            started: Instant::now(),
            first_output_recorder: None,
            rate_limit_observation: None,
            cancellation: &mut cancellation,
            events: &events,
            response_budget: &mut response_budget,
        },
    ));
    tokio::select! {
        started = started => {
            started.expect("fixture should keep the stream open");
            cancel_sender.send(true).expect("cancellation receiver is active");
        }
        result = &mut receive => {
            let _ = result;
            panic!("stream ended before cancellation");
        },
    }
    let error = expect_service_error(receive.await);
    assert_eq!(error.code, "request_cancelled");
    continue_server
        .send(())
        .expect("release fixture server after cancellation");
    server.await.expect("fixture server should close");
}

#[tokio::test]
async fn stalled_local_response_uses_the_request_timeout_error_path() {
    let body = sse_event(
        json!({"choices": [{"finish_reason": "stop", "delta": {}}]}),
        true,
    );
    let (url, started, continue_server, server) =
        spawn_pending_stream_server(Vec::new(), body).await;
    let mut route = route("gemini");
    route.chat_url = url;
    route.is_free = false;
    let client = fixture_client(Duration::from_millis(100));
    let (events, _event_receiver) = EventSink::test_channel();
    let (_cancel_sender, mut cancellation) = watch::channel(false);
    let body = json!({"model": "test-model", "messages": []});
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_summaries = Vec::new();
    let mut output_tokens = None;
    let mut input_tokens = None;
    let request_id = json!("fixture-request");
    let mut response_budget = ProviderResponseBudget::default();
    let mut receive = Box::pin(receive_for_test(
        &client,
        ResponseStreamRequest {
            route: &route,
            api_key: Some("fixture-api-key"),
            session_id: None,
            body: &body,
            request_id: &request_id,
            conversation_id: "fixture-conversation",
            message_id: "fixture-message",
            created_at: 1,
            content: &mut content,
            reasoning_content: &mut reasoning,
            reasoning_summaries: &mut reasoning_summaries,
            output_tokens: &mut output_tokens,
            input_tokens: &mut input_tokens,
            started: Instant::now(),
            first_output_recorder: None,
            rate_limit_observation: None,
            cancellation: &mut cancellation,
            events: &events,
            response_budget: &mut response_budget,
        },
    ));
    tokio::select! {
        started = tokio::time::timeout(Duration::from_secs(2), started) => {
            started.expect("fixture response should start")
                .expect("fixture should signal its open response");
        }
        result = &mut receive => {
            let _ = result;
            panic!("request finished before the stalled response timeout");
        }
    }
    let error = expect_service_error(
        tokio::time::timeout(Duration::from_secs(2), receive)
            .await
            .expect("request timeout should be bounded"),
    );
    assert_eq!(error.code, "network_unavailable");
    continue_server
        .send(())
        .expect("release fixture server after timeout");
    server.await.expect("fixture server should close");
}

#[tokio::test]
async fn consecutive_rounds_separate_visible_text_and_preserve_per_round_reasoning_summaries() {
    let mut round_one = Vec::new();
    round_one.extend(sse_event(
        json!({"choices": [{"delta": {"reasoning_content": "Inspecting workspace tree."}}]}),
        true,
    ));
    round_one.extend(sse_event(
        json!({"choices": [{"finish_reason": "stop", "delta": {"content": "I'll start by reconnoitering the project structure."}}]}),
        true,
    ));

    let mut round_two = Vec::new();
    round_two.extend(sse_event(
        json!({"choices": [{"delta": {"reasoning_content": "Building security scan checklist."}}]}),
        true,
    ));
    round_two.extend(sse_event(
        json!({"choices": [{"finish_reason": "stop", "delta": {"content": "Now let me establish my scan plan."}}]}),
        true,
    ));

    let client = fixture_client(Duration::from_secs(5));
    let (events, _event_receiver) = EventSink::test_channel();
    let (_cancel_sender, mut cancellation) = watch::channel(false);
    let body = json!({"model": "test-model", "messages": []});
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_summaries = Vec::new();
    let mut output_tokens = None;
    let mut input_tokens = None;
    let request_id = json!("fixture-request");
    let mut response_budget = ProviderResponseBudget::default();

    for payload in [round_one, round_two] {
        let (url, server) = spawn_fixture_server(200, payload).await;
        let mut turn_route = route("opencode");
        turn_route.chat_url = url;
        turn_route.is_opencode = true;
        let turn = receive_for_test(
            &client,
            ResponseStreamRequest {
                route: &turn_route,
                api_key: Some("fixture-api-key"),
                session_id: None,
                body: &body,
                request_id: &request_id,
                conversation_id: "fixture-conversation",
                message_id: "fixture-message",
                created_at: 1,
                content: &mut content,
                reasoning_content: &mut reasoning,
                reasoning_summaries: &mut reasoning_summaries,
                output_tokens: &mut output_tokens,
                input_tokens: &mut input_tokens,
                started: Instant::now(),
                first_output_recorder: None,
                rate_limit_observation: None,
                cancellation: &mut cancellation,
                events: &events,
                response_budget: &mut response_budget,
            },
        )
        .await
        .expect("round should succeed");
        assert!(!turn.round_content.starts_with('\n'));
        server.await.expect("fixture server should finish");
    }

    assert_eq!(
        content,
        "I'll start by reconnoitering the project structure.\n\nNow let me establish my scan plan."
    );
    assert_eq!(reasoning_summaries.len(), 2);
    assert_eq!(reasoning_summaries[0].id, "reasoning_fixture-message");
    assert_eq!(reasoning_summaries[0].content, "Inspecting workspace tree.");
    assert!(reasoning_summaries[0].is_complete);
    assert_eq!(reasoning_summaries[1].id, "reasoning_fixture-message_2");
    assert_eq!(
        reasoning_summaries[1].content,
        "Building security scan checklist."
    );
    assert!(reasoning_summaries[1].is_complete);
}
