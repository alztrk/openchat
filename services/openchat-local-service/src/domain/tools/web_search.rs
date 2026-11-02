//! Searches the web with Exa and a DuckDuckGo fallback, and fetches bounded public-page content.
//!
//! Provider responses and page bodies are size-limited; the fetcher revalidates redirect
//! targets and pins DNS results so private or local network addresses are not requested.

use std::{
    sync::{Arc, LazyLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;

pub(crate) mod url_fetch;

use reqwest::StatusCode;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use url::Url;

const DEFAULT_TIMEOUT_SECS: u64 = 8;
const EXA_TIMEOUT_SECS: u64 = 15;
const MAX_HTML_BYTES: usize = 1024 * 1024; // 1 MB max fetch
const MAX_SEARCH_RESPONSE_BYTES: usize = MAX_HTML_BYTES;
const DEFAULT_MAX_CHARS: usize = 6000;
const DEFAULT_SEARCH_LIMIT: usize = 8;
const MAX_SEARCH_LIMIT: usize = 20;
const CACHE_TTL_SECS: u64 = 300; // 5 minutes cache
const EXA_MCP_ENDPOINT: &str = "https://mcp.exa.ai/mcp";

static HTTP_CLIENT: LazyLock<Result<reqwest::Client, reqwest::Error>> = LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36 Edg/123.0.0.0",
        ),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        ),
    );
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("tr-TR,tr;q=0.9,en-US;q=0.8,en;q=0.7"),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        // Search providers must not redirect requests to arbitrary hosts.
        .redirect(reqwest::redirect::Policy::none())
        .tcp_nodelay(true)
        .pool_idle_timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(10)
        .build()
});

fn http_client() -> Result<&'static reqwest::Client, String> {
    HTTP_CLIENT
        .as_ref()
        .map_err(|_| "The web search client could not be initialized.".to_owned())
}

static EXA_HTTP_CLIENT: LazyLock<Result<reqwest::Client, reqwest::Error>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(EXA_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::none())
        .tcp_nodelay(true)
        .pool_idle_timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(10)
        .build()
});

fn exa_http_client() -> Result<&'static reqwest::Client, String> {
    EXA_HTTP_CLIENT
        .as_ref()
        .map_err(|_| "The Exa search client could not be initialized.".to_owned())
}

/// Reads a search provider response while bounding the decoded body in memory.
async fn read_bounded_search_response(response: reqwest::Response) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SEARCH_RESPONSE_BYTES as u64)
    {
        return Err("Search provider response exceeded the configured size limit.".to_owned());
    }

    let mut body = Vec::with_capacity(MAX_SEARCH_RESPONSE_BYTES.min(64 * 1024));
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|_| "Search provider response could not be read.".to_owned())?;
        append_bounded_search_chunk(&mut body, &chunk)?;
    }
    Ok(body)
}

/// Appends one response chunk only when it fits inside the shared body limit.
fn append_bounded_search_chunk(body: &mut Vec<u8>, chunk: &[u8]) -> Result<(), String> {
    if chunk.len() > MAX_SEARCH_RESPONSE_BYTES.saturating_sub(body.len()) {
        return Err("Search provider response exceeded the configured size limit.".to_owned());
    }
    body.extend_from_slice(chunk);
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub engine: String,
}

// In-memory cache entry
struct CacheEntry<T> {
    data: T,
    timestamp: Instant,
    retrieved_at_unix_ms: i64,
}

struct SearchCache {
    searches: Vec<(String, usize, CacheEntry<Vec<SearchResultItem>>)>,
    pages: Vec<(String, CacheEntry<Value>)>,
}

static CACHE: LazyLock<Arc<Mutex<SearchCache>>> = LazyLock::new(|| {
    Arc::new(Mutex::new(SearchCache {
        searches: Vec::new(),
        pages: Vec::new(),
    }))
});

/// Fallback: Fetches results from DuckDuckGo HTML endpoint (zero-JS)
async fn search_duckduckgo_html(
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResultItem>, String> {
    let resp = http_client()?
        .post("https://html.duckduckgo.com/html/")
        .form(&[("q", query), ("b", "")])
        .send()
        .await
        .map_err(|e| format!("DuckDuckGo request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("DuckDuckGo returned status {}", resp.status()));
    }

    let body = read_bounded_search_response(resp).await?;
    let html = String::from_utf8_lossy(&body);

    Ok(parse_duckduckgo_html(html.as_ref(), limit))
}

pub fn parse_duckduckgo_html(html: &str, limit: usize) -> Vec<SearchResultItem> {
    let mut items = Vec::new();
    let marker = "class=\"result__a\"";
    let mut cursor = 0;

    while let Some(rel_idx) = html[cursor..].find(marker) {
        if items.len() >= limit {
            break;
        }
        let a_marker = cursor + rel_idx;
        let tag_open = html[..a_marker].rfind("<a").unwrap_or(a_marker);
        let tag_close = match html[tag_open..].find('>') {
            Some(i) => tag_open + i,
            None => break,
        };
        let tag_attrs = &html[tag_open..tag_close];
        let href = tag_attrs
            .find("href=\"")
            .and_then(|h_start| {
                let after = &tag_attrs[h_start + 6..];
                after.find('"').map(|end| &after[..end])
            })
            .unwrap_or("");

        let a_end = match html[tag_close..].find("</a>") {
            Some(i) => tag_close + i,
            None => break,
        };
        let title_raw = &html[tag_close + 1..a_end];
        let title = clean_html_tags(title_raw);
        let url = extract_ddg_target_url(href);

        cursor = a_end + 4;

        let Some(remaining_html) = html.get(cursor..) else {
            break;
        };
        let snippet_search_window = 1500.min(remaining_html.len());
        let snippet_search_end = (0..=snippet_search_window)
            .rev()
            .find(|&end| remaining_html.is_char_boundary(end))
            .unwrap_or(0);
        let snippet_search = &remaining_html[..snippet_search_end];
        let snippet = if let Some(snip_idx) = snippet_search.find("result__snippet") {
            let snip_abs = cursor + snip_idx;
            if let Some(snip_close) = html[snip_abs..].find('>') {
                let text_start = snip_abs + snip_close + 1;
                if let Some(text_end) = html[text_start..].find("</a>") {
                    clean_html_tags(&html[text_start..text_start + text_end])
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if !url.is_empty() && !title.is_empty() {
            items.push(SearchResultItem {
                title,
                url,
                snippet,
                engine: "duckduckgo".to_owned(),
            });
        }
    }

    items
}

fn extract_ddg_target_url(raw_href: &str) -> String {
    match Url::parse(raw_href) {
        Ok(parsed) => parsed
            .query_pairs()
            .find(|(key, _)| key == "uddg")
            .map(|(_, target)| target.into_owned())
            .unwrap_or_else(|| raw_href.to_owned()),
        Err(_) => {
            let absolute_url = format!("https://duckduckgo.com{raw_href}");
            Url::parse(&absolute_url)
                .ok()
                .and_then(|parsed| {
                    parsed
                        .query_pairs()
                        .find(|(key, _)| key == "uddg")
                        .map(|(_, target)| target.into_owned())
                })
                .unwrap_or_else(|| raw_href.to_owned())
        }
    }
}

/// Calls Exa's hosted MCP search tool and converts its text response to search results.
async fn search_exa_mcp(query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
    let request_body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "web_search_exa",
            "arguments": {
                "query": query,
                "numResults": limit,
            }
        }
    });

    let response = exa_http_client()?
        .post(EXA_MCP_ENDPOINT)
        .header(ACCEPT, "application/json, text/event-stream")
        .json(&request_body)
        .send()
        .await
        .map_err(|_| "The Exa search request failed.".to_owned())?;
    if !response.status().is_success() {
        return Err(format!("Exa returned HTTP {}.", response.status()));
    }

    let body = read_bounded_search_response(response).await?;
    let body = std::str::from_utf8(&body)
        .map_err(|_| "Exa returned a response that was not valid UTF-8.".to_owned())?;
    parse_exa_mcp_response(body, limit)
}

fn parse_exa_mcp_response(body: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
    let mut payloads = Vec::new();
    let trimmed_body = body.trim();
    if trimmed_body.starts_with('{') {
        payloads.push(trimmed_body.to_owned());
    } else {
        let mut event_data = Vec::new();
        for line in body.lines() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                if !event_data.is_empty() {
                    payloads.push(event_data.join("\n"));
                    event_data.clear();
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                event_data.push(data.trim_start().to_owned());
            }
        }
        if !event_data.is_empty() {
            payloads.push(event_data.join("\n"));
        }
    }

    if payloads.is_empty() {
        return Err("Exa returned an empty MCP response.".to_owned());
    }

    let mut results = Vec::new();
    let mut found_text = false;
    for payload in payloads {
        if payload == "[DONE]" {
            continue;
        }
        let response: Value = serde_json::from_str(&payload)
            .map_err(|_| "Exa returned an unreadable MCP response.".to_owned())?;
        if response.get("error").is_some() {
            return Err("Exa returned a JSON-RPC error.".to_owned());
        }
        let result = response.get("result").unwrap_or(&response);
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            return Err("Exa reported that the search failed.".to_owned());
        }
        let content = result
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| "Exa returned an MCP response without content.".to_owned())?;

        for text in content
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
        {
            found_text = true;
            results.extend(parse_exa_search_text(
                text,
                limit.saturating_sub(results.len()),
            ));
            if results.len() >= limit {
                return Ok(results);
            }
        }
    }

    if found_text {
        Ok(results)
    } else {
        Err("Exa returned an MCP response without text results.".to_owned())
    }
}

fn parse_exa_search_text(text: &str, limit: usize) -> Vec<SearchResultItem> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut results = Vec::new();
    let mut cursor = 0;

    while cursor < lines.len() && results.len() < limit {
        let Some(title) = lines[cursor].strip_prefix("Title: ") else {
            cursor += 1;
            continue;
        };

        let block_start = cursor + 1;
        let block_end = lines[block_start..]
            .iter()
            .position(|line| line.starts_with("Title: "))
            .map(|offset| block_start + offset)
            .unwrap_or(lines.len());
        let block = &lines[block_start..block_end];
        let url = block
            .iter()
            .find_map(|line| line.strip_prefix("URL: "))
            .and_then(parse_search_result_url);
        let content_start = block
            .iter()
            .position(|line| matches!(*line, "Highlights:" | "Text:"));
        let snippet = content_start
            .map(|start| block[start + 1..].join("\n").trim().to_owned())
            .unwrap_or_default();

        if !title.trim().is_empty()
            && let Some(url) = url
        {
            results.push(SearchResultItem {
                title: title.trim().to_owned(),
                url,
                snippet: snippet.chars().take(4000).collect(),
                engine: "exa".to_owned(),
            });
        }
        cursor = block_end;
    }

    results
}

fn parse_search_result_url(raw_url: &str) -> Option<String> {
    let url = Url::parse(raw_url.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    Some(url.to_string())
}

pub fn clean_html_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    let mut last_was_space = true;
    let bytes = text.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let b = bytes[cursor];

        if in_tag {
            if b == b'>' {
                in_tag = false;
            }
            cursor += 1;
            continue;
        }

        if b == b'<' {
            in_tag = true;
            cursor += 1;
            continue;
        }

        // Entity check
        if b == b'&'
            && let Some(semi_rel) = bytes[cursor..].iter().position(|&x| x == b';')
            && semi_rel <= 10
        {
            let entity = &text[cursor..cursor + semi_rel + 1];
            let ch = if entity.starts_with("&#x") || entity.starts_with("&#X") {
                u32::from_str_radix(&entity[3..entity.len() - 1], 16)
                    .ok()
                    .and_then(char::from_u32)
            } else if entity.starts_with("&#") {
                entity[2..entity.len() - 1]
                    .parse::<u32>()
                    .ok()
                    .and_then(char::from_u32)
            } else {
                match entity {
                    "&amp;" => Some('&'),
                    "&lt;" => Some('<'),
                    "&gt;" => Some('>'),
                    "&quot;" => Some('"'),
                    "&#39;" | "&apos;" | "&#x27;" => Some('\''),
                    "&nbsp;" => Some(' '),
                    "&mdash;" | "&ndash;" => Some('-'),
                    "&hellip;" => Some('…'),
                    _ => None,
                }
            };

            if let Some(c) = ch {
                cursor += semi_rel + 1;
                if c.is_whitespace() {
                    if !last_was_space {
                        out.push(' ');
                        last_was_space = true;
                    }
                } else {
                    out.push(c);
                    last_was_space = false;
                }
                continue;
            }
        }

        if let Some(ch) = text[cursor..].chars().next() {
            cursor += ch.len_utf8();
            if ch.is_whitespace() {
                if !last_was_space {
                    out.push(' ');
                    last_was_space = true;
                }
            } else {
                out.push(ch);
                last_was_space = false;
            }
        } else {
            break;
        }
    }

    if last_was_space && !out.is_empty() {
        out.pop();
    }

    out
}

/// Searches Exa first and uses DuckDuckGo only when Exa fails or has no matching results.
pub async fn execute_web_search(query: &str, limit_opt: Option<usize>) -> Result<Value, String> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return Err("Search query cannot be empty.".to_owned());
    }
    let limit = limit_opt
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT);

    // 1. Check cache
    {
        let cache = CACHE.lock().await;
        if let Some((_, _, entry)) =
            cache
                .searches
                .iter()
                .find(|(cached_query, cached_limit, _)| {
                    cached_query == query_trimmed && *cached_limit == limit
                })
            && entry.timestamp.elapsed() < Duration::from_secs(CACHE_TTL_SECS)
        {
            return Ok(format_search_response(
                query_trimmed,
                &entry.data,
                entry.retrieved_at_unix_ms,
            ));
        }
    }

    let site_restriction = site_restriction_from_query(query_trimmed);
    if let Some(restriction) = site_restriction.as_ref()
        && let Some(item) = search_github_repository(restriction).await?
    {
        let results = [item];
        let retrieved_at_unix_ms = now_unix_millis()?;
        cache_search_results(query_trimmed, limit, &results, retrieved_at_unix_ms).await;
        return Ok(format_search_response(
            query_trimmed,
            &results,
            retrieved_at_unix_ms,
        ));
    }

    let mut exa_error = None;
    let mut final_results = match search_exa_mcp(query_trimmed, limit).await {
        Ok(results) => results
            .into_iter()
            .filter(|item| matches_site_restriction(item, site_restriction.as_ref()))
            .take(limit)
            .collect::<Vec<_>>(),
        Err(error) => {
            exa_error = Some(error);
            Vec::new()
        }
    };

    if final_results.is_empty() {
        let duckduckgo_results = match search_duckduckgo_html(query_trimmed, limit).await {
            Ok(results) => results
                .into_iter()
                .filter(|item| matches_site_restriction(item, site_restriction.as_ref()))
                .take(limit)
                .collect::<Vec<_>>(),
            Err(error) => {
                return Err(match exa_error {
                    Some(exa_error) => {
                        format!(
                            "Exa search failed: {exa_error} DuckDuckGo fallback failed: {error}"
                        )
                    }
                    None => format!("DuckDuckGo fallback failed: {error}"),
                });
            }
        };

        if duckduckgo_results.is_empty()
            && let Some(exa_error) = exa_error
        {
            return Err(format!(
                "Exa search failed: {exa_error} DuckDuckGo returned no matching results."
            ));
        }
        final_results = duckduckgo_results;
    }

    let retrieved_at_unix_ms = now_unix_millis()?;

    cache_search_results(query_trimmed, limit, &final_results, retrieved_at_unix_ms).await;

    Ok(format_search_response(
        query_trimmed,
        &final_results,
        retrieved_at_unix_ms,
    ))
}

fn format_search_response(
    query: &str,
    results: &[SearchResultItem],
    retrieved_at_unix_ms: i64,
) -> Value {
    json!({
        "query": query,
        "sourceType": "local_web_search",
        "retrievedAtUnixMs": retrieved_at_unix_ms,
        "total_results": results.len(),
        "results": results.iter().enumerate().map(|(index, item)| json!({
            "sourceId": format!("S{}", index + 1),
            "title": item.title,
            "url": item.url,
            "snippet": item.snippet,
            "engine": item.engine,
        })).collect::<Vec<_>>()
    })
}

async fn cache_search_results(
    query: &str,
    limit: usize,
    results: &[SearchResultItem],
    retrieved_at_unix_ms: i64,
) {
    let mut cache = CACHE.lock().await;
    if cache.searches.len() > 100 {
        cache.searches.remove(0);
    }
    cache.searches.push((
        query.to_owned(),
        limit,
        CacheEntry {
            data: results.to_vec(),
            timestamp: Instant::now(),
            retrieved_at_unix_ms,
        },
    ));
}

fn site_restriction_from_query(query: &str) -> Option<Url> {
    let site = query.split_whitespace().find_map(|term| {
        let (operator, value) = term.split_once(':')?;
        operator.eq_ignore_ascii_case("site").then_some(value)
    })?;
    let site_url = Url::parse(&format!("https://{site}")).ok()?;
    if site_url.host_str().is_none() || site_url.query().is_some() || site_url.fragment().is_some()
    {
        return None;
    }
    Some(site_url)
}

fn github_repository_segments(restriction: &Url) -> Option<(&str, &str)> {
    if restriction.host_str() != Some("github.com") {
        return None;
    }
    let segments = restriction
        .path()
        .trim_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    let [owner, repository] = segments.as_slice() else {
        return None;
    };
    let is_safe_segment = |segment: &str| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    (is_safe_segment(owner) && is_safe_segment(repository)).then_some((*owner, *repository))
}

async fn search_github_repository(restriction: &Url) -> Result<Option<SearchResultItem>, String> {
    let Some((owner, repository)) = github_repository_segments(restriction) else {
        return Ok(None);
    };

    let mut api_url = Url::parse("https://api.github.com/")
        .map_err(|_| "The GitHub API address is invalid.".to_owned())?;
    api_url
        .path_segments_mut()
        .map_err(|_| "The GitHub API address cannot accept a repository path.".to_owned())?
        .extend(["repos", owner, repository]);

    let response = http_client()?
        .get(api_url)
        .header(ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|_| "The GitHub repository request failed.".to_owned())?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!(
            "GitHub returned HTTP {} while looking up the repository.",
            response.status()
        ));
    }

    let body = read_bounded_search_response(response).await?;
    let repository_data: Value = serde_json::from_slice(&body)
        .map_err(|_| "GitHub returned an unreadable repository response.".to_owned())?;
    let title = repository_data
        .get("full_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "GitHub's repository response omitted its name.".to_owned())?;
    let url = repository_data
        .get("html_url")
        .and_then(Value::as_str)
        .ok_or_else(|| "GitHub's repository response omitted its page URL.".to_owned())?;
    let item = SearchResultItem {
        title: title.to_owned(),
        url: url.to_owned(),
        snippet: repository_data
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        engine: "github".to_owned(),
    };
    if !matches_site_restriction(&item, Some(restriction)) {
        return Err("GitHub returned a repository outside the requested site path.".to_owned());
    }

    Ok(Some(item))
}

fn matches_site_restriction(item: &SearchResultItem, restriction: Option<&Url>) -> bool {
    let Some(restriction) = restriction else {
        return true;
    };
    let Ok(result_url) = Url::parse(&item.url) else {
        return false;
    };
    if !matches!(result_url.scheme(), "http" | "https") {
        return false;
    }

    let Some(expected_host) = restriction.host_str() else {
        return false;
    };
    let Some(result_host) = result_url.host_str() else {
        return false;
    };
    if result_host != expected_host
        && !result_host
            .strip_suffix(expected_host)
            .is_some_and(|prefix| prefix.ends_with('.'))
    {
        return false;
    }

    let expected_path = restriction.path().trim_end_matches('/');
    expected_path.is_empty()
        || expected_path == "/"
        || result_url.path() == expected_path
        || result_url
            .path()
            .strip_prefix(expected_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

/// Reads a web page and converts its readable content to clean Markdown
pub async fn execute_read_url(
    url_str: &str,
    max_chars_opt: Option<usize>,
) -> Result<Value, String> {
    let url_trimmed = url_str.trim();
    if url_trimmed.is_empty() {
        return Err("URL cannot be empty.".to_owned());
    }

    let parsed_url = Url::parse(url_trimmed).map_err(|_| "The URL is invalid.".to_owned())?;
    let parsed_url = url_fetch::validate_http_url(parsed_url)?;

    let max_chars = max_chars_opt
        .unwrap_or(DEFAULT_MAX_CHARS)
        .clamp(500, 30_000);

    // 1. Check cache
    {
        let cache = CACHE.lock().await;
        if let Some((_, entry)) = cache.pages.iter().find(|(u, _)| u == url_trimmed)
            && entry.timestamp.elapsed() < Duration::from_secs(CACHE_TTL_SECS)
        {
            return Ok(entry.data.clone());
        }
    }

    // 2. Fetch page HTML
    // Stream and limit bytes dynamically based on max_chars
    let byte_limit = (max_chars * 16).clamp(128 * 1024, MAX_HTML_BYTES);
    let page = url_fetch::fetch_public_page(parsed_url, byte_limit).await?;
    let raw_text = String::from_utf8_lossy(&page.bytes).to_string();

    let mut result = if page.content_type.contains("text/plain")
        || page.content_type.contains("application/json")
    {
        let truncated = page.truncated || raw_text.len() > max_chars;
        let content: String = raw_text.chars().take(max_chars).collect();
        json!({
            "url": page.url.as_str(),
            "title": page.url.host_str().unwrap_or(""),
            "content": content,
            "length": content.len(),
            "truncated": truncated
        })
    } else {
        let title = extract_title(&raw_text)
            .unwrap_or_else(|| page.url.host_str().unwrap_or("").to_owned());

        let cleaned_html = strip_boilerplate_tags(&raw_text);
        let markdown =
            htmd::convert(&cleaned_html).unwrap_or_else(|_| clean_html_tags(&cleaned_html));
        let normalized = normalize_markdown_newlines(&markdown);

        let truncated = page.truncated || normalized.len() > max_chars;
        let content: String = normalized.chars().take(max_chars).collect();

        json!({
            "url": page.url.as_str(),
            "title": title,
            "content": content,
            "length": content.len(),
            "truncated": truncated
        })
    };

    let retrieved_at_unix_ms = now_unix_millis()?;
    if let Some(result) = result.as_object_mut() {
        result.insert("sourceType".to_owned(), json!("local_read_url"));
        result.insert("sourceId".to_owned(), json!("U1"));
        result.insert("retrievedAtUnixMs".to_owned(), json!(retrieved_at_unix_ms));
    }

    // Store in cache
    {
        let mut cache = CACHE.lock().await;
        if cache.pages.len() > 50 {
            cache.pages.remove(0);
        }
        cache.pages.push((
            url_trimmed.to_owned(),
            CacheEntry {
                data: result.clone(),
                timestamp: Instant::now(),
                retrieved_at_unix_ms,
            },
        ));
    }

    Ok(result)
}

fn now_unix_millis() -> Result<i64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "The system clock is invalid.".to_owned())?;
    i64::try_from(elapsed.as_millis())
        .map_err(|_| "The system clock is outside the supported range.".to_owned())
}

fn extract_title(html: &str) -> Option<String> {
    let bytes = html.as_bytes();
    let mut search_idx = 0;
    while search_idx + 7 < bytes.len() {
        if bytes[search_idx] == b'<'
            && bytes[search_idx + 1..search_idx + 6].eq_ignore_ascii_case(b"title")
            && matches!(bytes[search_idx + 6], b'>' | b' ' | b'\n' | b'\t' | b'\r')
            && let Some(tag_close) = bytes[search_idx + 6..].iter().position(|&b| b == b'>')
        {
            let content_start = search_idx + 6 + tag_close + 1;
            let mut close_idx = content_start;
            while close_idx + 8 <= bytes.len() {
                if bytes[close_idx] == b'<'
                    && bytes[close_idx + 1] == b'/'
                    && bytes[close_idx + 2..close_idx + 7].eq_ignore_ascii_case(b"title")
                    && bytes[close_idx + 7] == b'>'
                {
                    let raw_title = &bytes[content_start..close_idx];
                    let title_str = String::from_utf8_lossy(raw_title);
                    let cleaned = clean_html_tags(&title_str);
                    if cleaned.is_empty() {
                        return None;
                    } else {
                        return Some(cleaned);
                    }
                }
                close_idx += 1;
            }
        }
        search_idx += 1;
    }
    None
}

fn strip_boilerplate_tags(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let tags_to_strip: &[&[u8]] = &[
        b"script",
        b"style",
        b"svg",
        b"noscript",
        b"nav",
        b"footer",
        b"header",
        b"iframe",
    ];
    let mut cursor = 0;

    while cursor < bytes.len() {
        if let Some(tag_offset) = bytes[cursor..].iter().position(|&b| b == b'<') {
            let abs_tag_start = cursor + tag_offset;
            output.extend_from_slice(&bytes[cursor..abs_tag_start]);

            let rest = &bytes[abs_tag_start + 1..];
            let mut matched_tag: Option<&[u8]> = None;
            for &tag in tags_to_strip {
                if rest.len() >= tag.len() && rest[..tag.len()].eq_ignore_ascii_case(tag) {
                    let next = rest.get(tag.len()).copied();
                    if next == Some(b'>')
                        || next == Some(b' ')
                        || next == Some(b'/')
                        || next == Some(b'\n')
                        || next == Some(b'\t')
                        || next == Some(b'\r')
                    {
                        matched_tag = Some(tag);
                        break;
                    }
                }
            }

            if let Some(tag) = matched_tag {
                let mut search_idx = abs_tag_start + 1 + tag.len();
                let mut found_close = false;
                while search_idx + 3 + tag.len() <= bytes.len() {
                    if bytes[search_idx] == b'<' && bytes[search_idx + 1] == b'/' {
                        let after_slash = &bytes[search_idx + 2..];
                        if after_slash.len() >= tag.len()
                            && after_slash[..tag.len()].eq_ignore_ascii_case(tag)
                            && let Some(close_bracket) =
                                after_slash[tag.len()..].iter().position(|&b| b == b'>')
                        {
                            cursor = search_idx + 2 + tag.len() + close_bracket + 1;
                            found_close = true;
                            break;
                        }
                    }
                    search_idx += 1;
                }
                if !found_close {
                    cursor = bytes.len();
                }
            } else {
                output.push(b'<');
                cursor = abs_tag_start + 1;
            }
        } else {
            output.extend_from_slice(&bytes[cursor..]);
            break;
        }
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn normalize_markdown_newlines(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut consecutive_newlines = 0;
    for ch in text.chars() {
        if ch == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                result.push('\n');
            }
        } else {
            consecutive_newlines = 0;
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        MAX_SEARCH_RESPONSE_BYTES, SearchResultItem, append_bounded_search_chunk,
        extract_ddg_target_url, format_search_response, github_repository_segments,
        matches_site_restriction, parse_duckduckgo_html, parse_exa_mcp_response,
        parse_exa_search_text, site_restriction_from_query,
    };

    #[test]
    fn search_response_body_limit_accepts_exact_limit_and_rejects_overflow() {
        let mut body = Vec::new();
        let maximum_body = vec![b'x'; MAX_SEARCH_RESPONSE_BYTES];
        append_bounded_search_chunk(&mut body, &maximum_body)
            .expect("the exact response limit should be accepted");

        assert_eq!(body.len(), MAX_SEARCH_RESPONSE_BYTES);
        assert!(append_bounded_search_chunk(&mut body, b"x").is_err());
        assert_eq!(body.len(), MAX_SEARCH_RESPONSE_BYTES);
    }

    #[test]
    fn keeps_direct_urls_without_redirect_parameters() {
        let url = "https://example.org/articles?q=rust";

        assert_eq!(extract_ddg_target_url(url), url);
    }

    #[test]
    fn extracts_absolute_duckduckgo_redirect_targets() {
        let url = "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.org%2Farticles%3Fq%3Drust&rut=test";

        assert_eq!(
            extract_ddg_target_url(url),
            "https://example.org/articles?q=rust"
        );
    }

    #[test]
    fn resolves_relative_duckduckgo_redirect_targets() {
        let url = "/l/?uddg=https%3A%2F%2Fexample.org%2Farticles%3Fq%3Drust";

        assert_eq!(
            extract_ddg_target_url(url),
            "https://example.org/articles?q=rust"
        );
    }

    #[test]
    fn duckduckgo_parser_handles_utf8_at_snippet_window_edge() {
        let snippet_element = r#"<a class="result__snippet">Matched snippet</a>"#;
        let unicode_byte_offset = 1499;
        let filler_length = unicode_byte_offset - 1200 - snippet_element.len();
        let after_title = format!(
            "{}{}{}é",
            " ".repeat(1200),
            snippet_element,
            "x".repeat(filler_length)
        );
        assert_eq!(after_title.len(), 1501);
        assert!(!after_title.is_char_boundary(1500));

        let html =
            format!(r#"<a class="result__a" href="https://example.org/">Example</a>{after_title}"#);
        let results = parse_duckduckgo_html(&html, 1);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].snippet, "Matched snippet");
    }

    #[test]
    fn search_response_keeps_source_type_retrieval_time_and_source_ids() {
        let result = format_search_response(
            "test query",
            &[SearchResultItem {
                title: "OpenChat docs".to_owned(),
                url: "https://example.org/docs".to_owned(),
                snippet: "Documentation".to_owned(),
                engine: "exa".to_owned(),
            }],
            1_800_000_000_000,
        );

        assert_eq!(result["sourceType"], "local_web_search");
        assert_eq!(result["retrievedAtUnixMs"], 1_800_000_000_000i64);
        assert_eq!(result["results"][0]["sourceId"], "S1");
        assert_eq!(
            result["results"][0]["url"],
            json!("https://example.org/docs")
        );
    }

    #[test]
    fn parses_exa_sse_results_into_search_items() {
        let body = concat!(
            "event: message\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"Title: Rust documentation\\nURL: https://www.rust-lang.org/\\nPublished: N/A\\nAuthor: N/A\\nHighlights:\\nThe Rust programming language.\"}]}}\n\n"
        );

        let results = parse_exa_mcp_response(body, 8).expect("valid Exa SSE response");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust documentation");
        assert_eq!(results[0].url, "https://www.rust-lang.org/");
        assert_eq!(results[0].snippet, "The Rust programming language.");
        assert_eq!(results[0].engine, "exa");
    }

    #[test]
    fn parses_exa_json_results_and_respects_result_limit() {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{
                    "type": "text",
                    "text": concat!(
                        "Title: First result\nURL: https://example.org/one\nHighlights:\nFirst snippet\n\n---\n\n",
                        "Title: Second result\nURL: https://example.org/two\nHighlights:\nSecond snippet"
                    )
                }]
            }
        })
        .to_string();

        let results = parse_exa_mcp_response(&body, 1).expect("valid Exa JSON response");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "First result");
    }

    #[test]
    fn ignores_exa_results_with_non_http_urls() {
        let text = concat!(
            "Title: Invalid result\nURL: file:///etc/passwd\nHighlights:\nNot a web page\n\n---\n\n",
            "Title: Valid result\nURL: https://example.org/\nHighlights:\nA web page"
        );

        let results = parse_exa_search_text(text, 8);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Valid result");
    }

    #[test]
    fn site_restriction_matches_only_requested_host_and_path() {
        let restriction = site_restriction_from_query("site:github.com/rust-lang/rust")
            .expect("valid site restriction");
        let item = |url: &str| SearchResultItem {
            title: "Rust repository".to_owned(),
            url: url.to_owned(),
            snippet: String::new(),
            engine: "test".to_owned(),
        };

        assert!(matches_site_restriction(
            &item("https://github.com/rust-lang/rust/wiki"),
            Some(&restriction)
        ));
        assert!(!matches_site_restriction(
            &item("https://github.com/rust-lang/rustlings"),
            Some(&restriction)
        ));
        assert!(!matches_site_restriction(
            &item("https://github.com/other/repo"),
            Some(&restriction)
        ));
        assert!(!matches_site_restriction(
            &item("https://example.org/rust-lang/rust"),
            Some(&restriction)
        ));
    }

    #[test]
    fn site_restriction_allows_subdomains_and_unrestricted_queries() {
        let restriction = site_restriction_from_query("SITE:github.com").expect("valid site");
        let item = SearchResultItem {
            title: "Rust repository".to_owned(),
            url: "https://gist.github.com/user/id".to_owned(),
            snippet: String::new(),
            engine: "test".to_owned(),
        };

        assert!(matches_site_restriction(&item, Some(&restriction)));
        assert!(matches_site_restriction(&item, None));
    }

    #[test]
    fn github_site_paths_are_accepted_only_for_exact_repository_routes() {
        let repository = site_restriction_from_query("site:github.com/rust-lang/rust")
            .expect("repository site restriction");
        let nested_page = site_restriction_from_query("site:github.com/rust-lang/rust/tree/main")
            .expect("nested site restriction");
        let other_host = site_restriction_from_query("site:example.org/rust-lang/rust")
            .expect("other host restriction");

        assert_eq!(
            github_repository_segments(&repository),
            Some(("rust-lang", "rust"))
        );
        assert_eq!(github_repository_segments(&nested_page), None);
        assert_eq!(github_repository_segments(&other_host), None);
    }
}
