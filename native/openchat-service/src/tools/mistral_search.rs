use std::{
    collections::HashSet,
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::{Client, Response};
use serde_json::{Value, json};
use tokio::sync::watch;
use zeroize::Zeroizing;

const CONVERSATIONS_URL: &str = "https://api.mistral.ai/v1/conversations";
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_QUERY_CHARS: usize = 4_000;
const MAX_QUERY_BYTES: usize = 8 * 1024;
const MAX_RESULTS: usize = 10;
const MAX_SUMMARY_BYTES: usize = 12_000;
const MAX_SNIPPET_BYTES: usize = 1_024;
const MAX_URL_BYTES: usize = 2_048;

pub(crate) struct MistralSearchConfig {
    api_key: Zeroizing<String>,
    model_id: String,
}

impl MistralSearchConfig {
    pub(crate) fn new(api_key: &str, model_id: &str) -> Self {
        Self {
            api_key: Zeroizing::new(api_key.to_owned()),
            model_id: model_id.to_owned(),
        }
    }
}

pub(crate) async fn execute(
    config: &MistralSearchConfig,
    query: &str,
    limit: Option<usize>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, String> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > MAX_QUERY_CHARS || query.len() > MAX_QUERY_BYTES
    {
        return Err("The search query is empty or exceeds the supported length.".to_owned());
    }
    if *cancellation.borrow() {
        return Err("The search was stopped.".to_owned());
    }

    let client = client()?;
    execute_at(
        config,
        query,
        limit.unwrap_or(5).clamp(1, MAX_RESULTS),
        cancellation,
        &client,
        CONVERSATIONS_URL,
    )
    .await
}

async fn execute_at(
    config: &MistralSearchConfig,
    query: &str,
    limit: usize,
    cancellation: &mut watch::Receiver<bool>,
    client: &Client,
    endpoint: &str,
) -> Result<Value, String> {
    let request = client
        .post(endpoint)
        .bearer_auth(config.api_key.as_str())
        .json(&request_body(&config.model_id, query));
    let response = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err("The search was stopped.".to_owned());
        }
        response = request.send() => response.map_err(|_| "Mistral web search could not connect.".to_owned())?,
    };
    if !response.status().is_success() {
        return Err("Mistral web search was rejected by the provider.".to_owned());
    }

    let bytes = read_bounded_response(response, cancellation).await?;
    let response: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Mistral web search returned an invalid response.".to_owned())?;
    let retrieved_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "The system clock is invalid.".to_owned())?
        .as_millis();
    let retrieved_at_unix_ms = i64::try_from(retrieved_at_unix_ms)
        .map_err(|_| "The system clock is outside the supported range.".to_owned())?;
    response_to_result(&response, query, limit, retrieved_at_unix_ms)
}

fn client() -> Result<Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .timeout(Duration::from_secs(90))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "Mistral web search is unavailable.".to_owned())
        })
        .clone()
}

fn request_body(model_id: &str, query: &str) -> Value {
    json!({
        "model": model_id,
        "inputs": [{"role": "user", "content": query}],
        "tools": [{"type": "web_search"}],
    })
}

async fn read_bounded_response(
    mut response: Response,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("Mistral web search returned too much data.".to_owned());
    }
    let mut body = Vec::new();
    loop {
        let chunk = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Err("The search was stopped.".to_owned());
            }
            chunk = response.chunk() => chunk.map_err(|_| "Mistral web search response could not be read.".to_owned())?,
        };
        let Some(chunk) = chunk else {
            return Ok(body);
        };
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("Mistral web search returned too much data.".to_owned());
        }
        body.extend_from_slice(&chunk);
    }
}

fn response_to_result(
    response: &Value,
    query: &str,
    limit: usize,
    retrieved_at_unix_ms: i64,
) -> Result<Value, String> {
    let outputs = response
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| "Mistral web search returned no completed messages.".to_owned())?;
    let final_message = outputs
        .iter()
        .rev()
        .find(|output| output.get("type").and_then(Value::as_str) == Some("message.output"))
        .ok_or_else(|| "Mistral web search returned no completed answer.".to_owned())?;
    let content = final_message.get("content");
    let mut summary = String::new();
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();

    match content {
        Some(Value::String(text)) => append_bounded(&mut summary, text),
        Some(Value::Array(chunks)) => {
            for chunk in chunks {
                match chunk.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(text) = chunk.get("text").and_then(Value::as_str) {
                            append_bounded(&mut summary, text);
                        }
                    }
                    Some("tool_reference") if results.len() < limit => {
                        let Some(result) = citation_result(chunk, results.len() + 1) else {
                            continue;
                        };
                        if seen_urls.insert(result["url"].as_str().unwrap_or_default().to_owned()) {
                            results.push(result);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => return Err("Mistral web search returned an invalid answer.".to_owned()),
    }

    Ok(json!({
        "sourceType": "provider_native",
        "query": query,
        "summary": summary,
        "results": results,
        "retrievedAtUnixMs": retrieved_at_unix_ms,
    }))
}

fn append_bounded(output: &mut String, value: &str) {
    let remaining = MAX_SUMMARY_BYTES.saturating_sub(output.len());
    if remaining == 0 {
        return;
    }
    for character in value.chars() {
        if output.len().saturating_add(character.len_utf8()) > MAX_SUMMARY_BYTES {
            break;
        }
        output.push(character);
    }
}

fn citation_result(chunk: &Value, index: usize) -> Option<Value> {
    let title = chunk.get("title")?.as_str()?.trim();
    let raw_url = chunk.get("url")?.as_str()?;
    let url = url::Url::parse(raw_url).ok()?;
    if title.is_empty()
        || title.len() > 512
        || raw_url.len() > MAX_URL_BYTES
        || url.host_str().is_none()
        || !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let snippet = chunk
        .get("source")
        .and_then(Value::as_str)
        .map(|value| truncate_utf8(value, MAX_SNIPPET_BYTES))
        .filter(|value| !value.trim().is_empty());
    Some(json!({
        "sourceId": format!("S{index}"),
        "title": title,
        "url": url.as_str(),
        "snippet": snippet,
        "engine": "mistral",
    }))
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let end = value
        .char_indices()
        .take_while(|(index, character)| index.saturating_add(character.len_utf8()) <= max_bytes)
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::watch,
    };

    use super::{
        MAX_SUMMARY_BYTES, MistralSearchConfig, client, execute_at, request_body,
        response_to_result, truncate_utf8,
    };

    #[test]
    fn builds_a_mistral_conversation_request_with_hosted_web_search() {
        assert_eq!(
            request_body("mistral-medium-latest", "current Rust release"),
            json!({
                "model": "mistral-medium-latest",
                "inputs": [{"role": "user", "content": "current Rust release"}],
                "tools": [{"type": "web_search"}],
            })
        );
    }

    #[test]
    fn maps_final_message_text_and_references_to_citation_sources() {
        let response = json!({
            "outputs": [
                {"type": "message.output", "content": [{"type": "text", "text": "Searching."}]},
                {"type": "tool.execution", "name": "web_search"},
                {"type": "message.output", "content": [
                    {"type": "text", "text": "Rust 1.99 is current."},
                    {"type": "tool_reference", "tool": "web_search", "title": "Rust releases", "url": "https://www.rust-lang.org/releases", "source": "Release announcements"}
                ]}
            ]
        });

        let result = response_to_result(&response, "Rust release", 5, 1_800_000_000_000)
            .expect("Mistral final answer should map to a search result");
        assert_eq!(result["sourceType"], "provider_native");
        assert_eq!(result["summary"], "Rust 1.99 is current.");
        assert_eq!(result["results"][0]["sourceId"], "S1");
        assert_eq!(result["results"][0]["title"], "Rust releases");
        assert_eq!(
            result["results"][0]["url"],
            "https://www.rust-lang.org/releases"
        );
        assert_eq!(result["retrievedAtUnixMs"], 1_800_000_000_000i64);
    }

    #[test]
    fn ignores_unsafe_and_duplicate_references_and_bounds_result_count() {
        let response = json!({
            "outputs": [{"type": "message.output", "content": [
                {"type": "tool_reference", "title": "Unsafe", "url": "javascript:alert(1)"},
                {"type": "tool_reference", "title": "First", "url": "https://example.org/a"},
                {"type": "tool_reference", "title": "Duplicate", "url": "https://example.org/a"},
                {"type": "tool_reference", "title": "Second", "url": "https://example.org/b"}
            ]}]
        });

        let result = response_to_result(&response, "query", 1, 1)
            .expect("a completed answer with valid references should be accepted");
        assert_eq!(result["results"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn rejects_missing_final_message_instead_of_fabricating_results() {
        let response = json!({"outputs": [{"type": "tool.execution", "name": "web_search"}]});
        assert!(response_to_result(&response, "query", 5, 1).is_err());
    }

    #[test]
    fn truncates_provider_text_at_utf8_boundaries_and_within_the_output_limit() {
        let text = "é".repeat(MAX_SUMMARY_BYTES);
        let response = json!({
            "outputs": [{"type": "message.output", "content": [{"type": "text", "text": text}]}]
        });
        let result = response_to_result(&response, "query", 5, 1)
            .expect("a valid completed response should be accepted");
        let summary = result["summary"].as_str().expect("summary should be text");
        assert_eq!(summary.len(), MAX_SUMMARY_BYTES);
        assert!(summary.is_char_boundary(summary.len()));
        assert_eq!(truncate_utf8("éé", 3), "é");
    }

    #[tokio::test]
    async fn posts_a_bounded_conversation_search_request_and_maps_the_response() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local HTTP test listener");
        let address = listener.local_addr().expect("read test listener address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept test request");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];
            let mut expected_request_bytes = None;
            loop {
                let count = stream.read(&mut chunk).await.expect("read test request");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
                if expected_request_bytes.is_none()
                    && let Some(header_end) =
                        request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let header_end = header_end + 4;
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .expect("request should declare its body length");
                    expected_request_bytes = Some(header_end + content_length);
                }
                if expected_request_bytes.is_some_and(|expected| request.len() >= expected) {
                    break;
                }
            }
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("request should contain HTTP headers");
            let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
            assert!(headers.starts_with("post "));
            assert!(headers.contains(" http/1.1"));
            assert!(headers.contains("authorization: bearer test-key"));
            assert!(headers.contains("content-type: application/json"));
            let body: Value = serde_json::from_slice(&request[header_end + 4..])
                .expect("request body should be JSON");
            assert_eq!(
                body,
                json!({
                    "model": "mistral-small-latest",
                    "inputs": [{"role": "user", "content": "find a source"}],
                    "tools": [{"type": "web_search"}],
                })
            );
            let body = json!({
                "outputs": [{"type": "message.output", "content": [
                    {"type": "text", "text": "Found one source."},
                    {"type": "tool_reference", "title": "Example", "url": "https://example.org"}
                ]}]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write test response");
        });
        let config = MistralSearchConfig::new("test-key", "mistral-small-latest");
        let (_sender, mut cancellation) = watch::channel(false);
        let endpoint = format!("http://{address}/");

        let result = execute_at(
            &config,
            "find a source",
            5,
            &mut cancellation,
            &client().expect("build HTTP client"),
            &endpoint,
        )
        .await
        .expect("local Mistral response should be parsed");
        assert_eq!(result["summary"], "Found one source.");
        assert_eq!(result["results"][0]["url"], "https://example.org/");
        server.await.expect("local HTTP server should finish");
    }
}
