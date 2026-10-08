use std::{collections::HashSet, sync::OnceLock, time::Duration};

use reqwest::{Client, Response};
use serde_json::{Value, json};
use tokio::sync::watch;
use zeroize::Zeroizing;

const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_QUERY_BYTES: usize = 8 * 1024;
const MAX_QUERY_CHARS: usize = 4_000;
const MAX_RESULTS: usize = 10;

pub(crate) struct GeminiSearchConfig {
    api_key: Zeroizing<String>,
    model_id: String,
}

impl GeminiSearchConfig {
    pub(crate) fn new(api_key: &str, model_id: &str) -> Self {
        Self {
            api_key: Zeroizing::new(api_key.to_owned()),
            model_id: model_id.to_owned(),
        }
    }
}

pub(crate) async fn execute(
    config: &GeminiSearchConfig,
    query: &str,
    limit: Option<usize>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, String> {
    let query = query.trim();
    if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.chars().count() > MAX_QUERY_CHARS
    {
        return Err("The search query is empty or exceeds the supported length.".to_owned());
    }
    if *cancellation.borrow() {
        return Err("The search was stopped.".to_owned());
    }
    let model =
        url::form_urlencoded::byte_serialize(config.model_id.as_bytes()).collect::<String>();
    let endpoint =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
    execute_at(
        config,
        query,
        limit.unwrap_or(5).clamp(1, MAX_RESULTS),
        cancellation,
        &client()?,
        &endpoint,
    )
    .await
}

fn client() -> Result<Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .timeout(Duration::from_secs(90))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "Gemini web search is unavailable.".to_owned())
        })
        .clone()
}

fn request_body(query: &str) -> Value {
    json!({"contents":[{"role":"user","parts":[{"text":query}]}],"tools":[{"google_search":{}}]})
}

async fn execute_at(
    config: &GeminiSearchConfig,
    query: &str,
    limit: usize,
    cancellation: &mut watch::Receiver<bool>,
    client: &Client,
    endpoint: &str,
) -> Result<Value, String> {
    let request = client
        .post(endpoint)
        .header("x-goog-api-key", config.api_key.as_str())
        .json(&request_body(query));
    let response = tokio::select! { changed = cancellation.changed() => { let _ = changed; return Err("The search was stopped.".to_owned()); }, response = request.send() => response.map_err(|_| "Gemini web search could not connect.".to_owned())? };
    if !response.status().is_success() {
        return Err("Gemini web search was rejected by the provider.".to_owned());
    }
    let bytes = read_bounded_response(response, cancellation).await?;
    let response: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Gemini web search returned an invalid response.".to_owned())?;
    response_to_result(&response, query, limit)
}

async fn read_bounded_response(
    mut response: Response,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("Gemini web search returned too much data.".to_owned());
    }
    let mut body = Vec::new();
    loop {
        let chunk = tokio::select! { changed = cancellation.changed() => { let _ = changed; return Err("The search was stopped.".to_owned()); }, chunk = response.chunk() => chunk.map_err(|_| "Gemini web search response could not be read.".to_owned())? };
        let Some(chunk) = chunk else {
            return Ok(body);
        };
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("Gemini web search returned too much data.".to_owned());
        }
        body.extend_from_slice(&chunk);
    }
}

fn response_to_result(response: &Value, query: &str, limit: usize) -> Result<Value, String> {
    let candidate = response
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .ok_or_else(|| "Gemini web search returned no completed answer.".to_owned())?;
    let metadata = candidate
        .get("groundingMetadata")
        .ok_or_else(|| "Gemini did not return Google Search grounding sources.".to_owned())?;
    let mut summary = String::new();
    if let Some(parts) = candidate
        .pointer("/content/parts")
        .and_then(Value::as_array)
    {
        for part in parts {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                append_bounded(&mut summary, text, 12_000);
            }
        }
    }
    let chunks = metadata
        .get("groundingChunks")
        .and_then(Value::as_array)
        .ok_or_else(|| "Gemini returned invalid grounding sources.".to_owned())?;
    let supports = metadata
        .get("groundingSupports")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        if results.len() >= limit {
            break;
        }
        let Some(web) = chunk.get("web") else {
            continue;
        };
        let (Some(title), Some(raw_url)) = (
            web.get("title").and_then(Value::as_str),
            web.get("uri").and_then(Value::as_str),
        ) else {
            continue;
        };
        let Ok(url) = url::Url::parse(raw_url) else {
            continue;
        };
        if title.trim().is_empty()
            || title.len() > 512
            || raw_url.len() > 2048
            || url.host_str().is_none()
            || !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || !seen.insert(url.as_str().to_owned())
        {
            continue;
        }
        let mut snippet = String::new();
        for support in &supports {
            let cites_chunk = support
                .get("groundingChunkIndices")
                .and_then(Value::as_array)
                .is_some_and(|indices| indices.iter().any(|i| i.as_u64() == Some(index as u64)));
            if cites_chunk {
                if let Some(text) = support.pointer("/segment/text").and_then(Value::as_str) {
                    append_bounded(&mut snippet, text, 1024);
                }
            }
        }
        results.push(json!({"sourceId":format!("S{}", results.len()+1),"title":title,"url":url.as_str(),"snippet":snippet,"engine":"google"}));
    }
    let attribution = metadata
        .pointer("/searchEntryPoint/renderedContent")
        .and_then(Value::as_str)
        .map(|value| truncate_utf8(value, 32 * 1024));
    Ok(
        json!({"sourceType":"provider_native","query":query,"summary":summary,"results":results,"attributionHtml":attribution}),
    )
}

fn append_bounded(output: &mut String, value: &str, limit: usize) {
    for character in value.chars() {
        if output.len().saturating_add(character.len_utf8()) > limit {
            break;
        }
        output.push(character);
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let end = value
        .char_indices()
        .take_while(|(i, c)| i.saturating_add(c.len_utf8()) <= max_bytes)
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::{GeminiSearchConfig, client, execute_at, request_body, response_to_result};
    use serde_json::{Value, json};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::watch,
    };

    #[test]
    fn request_contains_only_query_and_google_search_tool() {
        assert_eq!(
            request_body("query"),
            json!({"contents":[{"role":"user","parts":[{"text":"query"}]}],"tools":[{"google_search":{}}]})
        );
    }

    #[test]
    fn maps_google_grounding_chunks_supports_and_required_attribution() {
        let input = json!({"candidates":[{"content":{"parts":[{"text":"A grounded answer."}]},"groundingMetadata":{"groundingChunks":[{"web":{"uri":"https://example.org","title":"Example"}}],"groundingSupports":[{"segment":{"text":"Supporting text."},"groundingChunkIndices":[0]}],"searchEntryPoint":{"renderedContent":"<div>Search</div>"}}}]});
        let result = response_to_result(&input, "query", 5).unwrap();
        assert_eq!(result["summary"], "A grounded answer.");
        assert_eq!(result["results"][0]["snippet"], "Supporting text.");
        assert_eq!(result["attributionHtml"], "<div>Search</div>");
    }

    #[test]
    fn rejects_missing_grounding_metadata_and_ignores_unsafe_urls() {
        assert!(response_to_result(&json!({"candidates":[{}]}), "query", 5).is_err());
        let input = json!({"candidates":[{"groundingMetadata":{"groundingChunks":[{"web":{"uri":"javascript:alert(1)","title":"unsafe"}}]}}]});
        let result = response_to_result(&input, "query", 5).unwrap();
        assert_eq!(result["results"].as_array().map(Vec::len), Some(0));
    }

    #[tokio::test]
    async fn sends_query_and_search_tool_with_key_in_header_only() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local HTTP test listener");
        let address = listener.local_addr().expect("read local HTTP address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 2048];
            let mut expected = None;
            loop {
                let count = stream.read(&mut chunk).await.expect("read request");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
                if expected.is_none()
                    && let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n")
                {
                    let header_end = header_end + 4;
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let body_len = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .expect("content length should be present");
                    expected = Some(header_end + body_len);
                }
                if expected.is_some_and(|expected| request.len() >= expected) {
                    break;
                }
            }
            let header_end = request
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .expect("HTTP headers should be present");
            let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
            assert!(headers.starts_with("post /v1beta/models/gemini-2.5-flash:generatecontent"));
            assert!(headers.contains("x-goog-api-key: test-key"));
            let body: Value = serde_json::from_slice(&request[header_end + 4..])
                .expect("request should contain JSON");
            assert_eq!(body, request_body("current Rust release"));
            let body = json!({"candidates":[{"content":{"parts":[{"text":"Grounded."}]},"groundingMetadata":{"groundingChunks":[{"web":{"uri":"https://example.org","title":"Example"}}]}}]}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write local response");
        });

        let config = GeminiSearchConfig::new("test-key", "gemini-2.5-flash");
        let (_sender, mut cancellation) = watch::channel(false);
        let endpoint = format!("http://{address}/v1beta/models/gemini-2.5-flash:generateContent");
        let result = execute_at(
            &config,
            "current Rust release",
            5,
            &mut cancellation,
            &client().expect("create HTTP client"),
            &endpoint,
        )
        .await
        .expect("local provider response should map to grounding sources");
        assert_eq!(result["results"][0]["url"], "https://example.org/");
        server.await.expect("local HTTP server should finish");
    }
}
