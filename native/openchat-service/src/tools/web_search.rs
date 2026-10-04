use std::{
    collections::HashSet,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

mod url_fetch;

use base64::Engine;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use url::Url;

const DEFAULT_TIMEOUT_SECS: u64 = 8;
const MAX_HTML_BYTES: usize = 1024 * 1024; // 1 MB max fetch
const DEFAULT_MAX_CHARS: usize = 6000;
const DEFAULT_SEARCH_LIMIT: usize = 8;
const MAX_SEARCH_LIMIT: usize = 20;
const CACHE_TTL_SECS: u64 = 300; // 5 minutes cache

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
        .redirect(reqwest::redirect::Policy::limited(5))
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
}

struct SearchCache {
    searches: Vec<(String, CacheEntry<Vec<SearchResultItem>>)>,
    pages: Vec<(String, CacheEntry<Value>)>,
}

static CACHE: LazyLock<Arc<Mutex<SearchCache>>> = LazyLock::new(|| {
    Arc::new(Mutex::new(SearchCache {
        searches: Vec::new(),
        pages: Vec::new(),
    }))
});

// SearXNG Public Instances Pool
const SEARXNG_INSTANCES: &[&str] = &[
    "https://searx.be",
    "https://priv.au",
    "https://search.ononoki.org",
    "https://searx.fmac.xyz",
    "https://baresearch.org",
];

/// Fetches search results directly from Bing HTML (fast, rich, anti-bot resilient)
async fn search_bing_html(query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let url = format!("https://www.bing.com/search?q={encoded}&setlang=tr");
    let resp = http_client()?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Bing request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Bing returned status {}", resp.status()));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read Bing response: {e}"))?;

    Ok(parse_bing_html(&html, limit))
}

pub fn parse_bing_html(html: &str, limit: usize) -> Vec<SearchResultItem> {
    let mut items = Vec::new();
    let marker = "class=\"b_algo\"";
    let mut cursor = 0;

    while let Some(rel_idx) = html[cursor..].find(marker) {
        if items.len() >= limit {
            break;
        }
        let block_start = cursor + rel_idx;
        let next_start = html[block_start + marker.len()..]
            .find(marker)
            .map(|i| block_start + marker.len() + i)
            .unwrap_or(html.len())
            .min(block_start + 12000);
        let block = &html[block_start..next_start];

        if let Some(h2_idx) = block.find("<h2") {
            let h2_part = &block[h2_idx..];
            if let Some(a_idx) = h2_part.find("<a") {
                let a_part = &h2_part[a_idx..];
                let href = a_part
                    .find("href=\"")
                    .and_then(|h_start| {
                        let after = &a_part[h_start + 6..];
                        after.find('"').map(|end| &after[..end])
                    })
                    .unwrap_or("");

                let title = if let Some(tag_close) = a_part.find('>') {
                    if let Some(a_close) = a_part[tag_close..].find("</a>") {
                        clean_html_tags(&a_part[tag_close + 1..tag_close + a_close])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let target_url = decode_bing_url(href)
                    .or_else(|| {
                        if href.starts_with("http") && !href.contains("bing.com/ck/") {
                            Some(href.to_owned())
                        } else {
                            extract_cite_url(block)
                        }
                    })
                    .unwrap_or_default();

                let snippet = if let Some(cap_idx) = block.find("b_caption") {
                    let cap_part = &block[cap_idx..];
                    if let Some(p_start) = cap_part.find("<p") {
                        if let Some(p_close_tag) = cap_part[p_start..].find('>') {
                            let text_start = p_start + p_close_tag + 1;
                            if let Some(p_end) = cap_part[text_start..].find("</p>") {
                                clean_html_tags(&cap_part[text_start..text_start + p_end])
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                if !target_url.is_empty() && !title.is_empty() {
                    items.push(SearchResultItem {
                        title,
                        url: target_url,
                        snippet,
                        engine: "bing".to_owned(),
                    });
                }
            }
        }

        cursor = block_start + marker.len();
    }

    items
}

fn decode_bing_url(raw_href: &str) -> Option<String> {
    let u_idx = if let Some(pos) = raw_href.find("&amp;u=a1") {
        pos + 9
    } else if let Some(pos) = raw_href.find("&u=a1") {
        pos + 5
    } else if let Some(pos) = raw_href.find("u=a1") {
        pos + 4
    } else {
        return None;
    };

    let b64_part = &raw_href[u_idx..];
    let end = b64_part
        .find('&')
        .or_else(|| b64_part.find('"'))
        .unwrap_or(b64_part.len());
    let b64_str = &b64_part[..end];

    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64_str)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(b64_str))
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(b64_str))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(b64_str));

    if let Ok(bytes) = decoded
        && let Ok(url) = String::from_utf8(bytes)
        && url.starts_with("http")
    {
        return Some(url);
    }
    None
}

fn extract_cite_url(block: &str) -> Option<String> {
    let start = block.find("<cite>")? + 6;
    let end = block[start..].find("</cite>")?;
    let raw = clean_html_tags(&block[start..start + end]);
    let first = raw.split_whitespace().next()?.to_owned();
    if first.starts_with("http://") || first.starts_with("https://") {
        Some(first)
    } else if first.starts_with("www.") {
        Some(format!("https://{first}"))
    } else {
        None
    }
}

/// Fetches search results from a SearXNG instance (returns combined Google/Bing results)
async fn search_searxng_instance(
    instance: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResultItem>, String> {
    let encoded_query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let url = format!("{instance}/search?q={encoded_query}&format=json&categories=general");
    let resp = http_client()?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("SearXNG request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("SearXNG returned status {}", resp.status()));
    }

    let json_val: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse SearXNG JSON: {e}"))?;

    let mut items = Vec::new();
    if let Some(results) = json_val.get("results").and_then(Value::as_array) {
        for res in results.iter().take(limit * 2) {
            let title = res
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let link = res.get("url").and_then(Value::as_str).unwrap_or("").trim();
            let snippet = res
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let engine = res
                .get("engine")
                .and_then(Value::as_str)
                .unwrap_or("google")
                .to_lowercase();

            if !title.is_empty() && !link.is_empty() && link.starts_with("http") {
                items.push(SearchResultItem {
                    title: clean_html_tags(title),
                    url: link.to_owned(),
                    snippet: clean_html_tags(snippet),
                    engine,
                });
            }
        }
    }

    if items.is_empty() {
        Err("SearXNG returned empty results".to_owned())
    } else {
        Ok(items)
    }
}

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

    let html = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read DuckDuckGo response: {e}"))?;

    Ok(parse_duckduckgo_html(&html, limit))
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

        let snippet_search_window = 1500.min(html.len().saturating_sub(cursor));
        let snippet = if let Some(snip_idx) =
            html[cursor..cursor + snippet_search_window].find("result__snippet")
        {
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

/// Fetches encyclopedic / factual entries directly from Wikipedia OpenSearch API concurrently
async fn search_wikipedia(query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let tr_url = format!(
        "https://tr.wikipedia.org/w/api.php?action=opensearch&search={encoded}&limit={limit}&namespace=0&format=json"
    );
    let en_url = format!(
        "https://en.wikipedia.org/w/api.php?action=opensearch&search={encoded}&limit={limit}&namespace=0&format=json"
    );

    let tr_fut = async {
        let response = http_client()?
            .get(&tr_url)
            .send()
            .await
            .map_err(|_| "The Turkish Wikipedia request failed.".to_owned())?;
        if !response.status().is_success() {
            return Err(format!(
                "Turkish Wikipedia returned HTTP {}.",
                response.status()
            ));
        }
        let json_value = response
            .json::<Value>()
            .await
            .map_err(|_| "The Turkish Wikipedia response could not be read.".to_owned())?;
        parse_wikipedia_opensearch(&json_value)
    };

    let en_fut = async {
        let response = http_client()?
            .get(&en_url)
            .send()
            .await
            .map_err(|_| "The English Wikipedia request failed.".to_owned())?;
        if !response.status().is_success() {
            return Err(format!(
                "English Wikipedia returned HTTP {}.",
                response.status()
            ));
        }
        let json_value = response
            .json::<Value>()
            .await
            .map_err(|_| "The English Wikipedia response could not be read.".to_owned())?;
        parse_wikipedia_opensearch(&json_value)
    };

    let (tr_res, en_res) = tokio::join!(tr_fut, en_fut);
    tr_res.or(en_res)
}

fn parse_wikipedia_opensearch(val: &Value) -> Result<Vec<SearchResultItem>, String> {
    let titles = val.get(1).and_then(Value::as_array);
    let descs = val.get(2).and_then(Value::as_array);
    let urls = val.get(3).and_then(Value::as_array);

    let (Some(titles), Some(urls)) = (titles, urls) else {
        return Err("No wikipedia results".to_string());
    };
    let mut items = Vec::new();
    for (i, title_val) in titles.iter().enumerate() {
        let title = title_val.as_str().unwrap_or("").trim();
        let url = urls.get(i).and_then(Value::as_str).unwrap_or("").trim();
        let snippet = descs
            .and_then(|d| d.get(i))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !title.is_empty() && !url.is_empty() {
            items.push(SearchResultItem {
                title: title.to_owned(),
                url: url.to_owned(),
                snippet: if snippet.is_empty() {
                    format!("Vikipedi makalesi: {title}")
                } else {
                    snippet.to_owned()
                },
                engine: "wikipedia".to_owned(),
            });
        }
    }
    if items.is_empty() {
        Err("No wikipedia results".to_string())
    } else {
        Ok(items)
    }
}

/// Executes a fast parallel multi-engine web search (Bing + Wikipedia + DuckDuckGo + SearXNG)
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
        if let Some((_, entry)) = cache.searches.iter().find(|(q, _)| q == query_trimmed)
            && entry.timestamp.elapsed() < Duration::from_secs(CACHE_TTL_SECS)
        {
            return Ok(format_search_response(query_trimmed, &entry.data));
        }
    }

    // 2. Primary Fast Race: Bing + Wikipedia (both sub-200ms)
    let bing_future = search_bing_html(query_trimmed, limit);
    let wiki_future = async {
        tokio::time::timeout(
            Duration::from_millis(300),
            search_wikipedia(query_trimmed, 2),
        )
        .await
        .unwrap_or(Err("Wikipedia timeout".to_string()))
    };

    let (bing_res, wiki_res) = tokio::join!(bing_future, wiki_future);

    let mut combined = Vec::new();
    let mut seen_urls = HashSet::new();

    // 1. Add Wikipedia result first if available (instant authoritative definition)
    if let Ok(results) = wiki_res {
        for item in results {
            if seen_urls.insert(item.url.clone()) {
                combined.push(item);
            }
        }
    }

    // 2. Add Bing results (highest quality web results)
    let bing_has_results = if let Ok(results) = bing_res {
        let has = !results.is_empty();
        for item in results {
            if seen_urls.insert(item.url.clone()) {
                combined.push(item);
            }
        }
        has
    } else {
        false
    };

    // 3. Fallback: Only if Bing produced 0 results (network error or blocked), query secondary engines
    if !bing_has_results {
        let ddg_future = search_duckduckgo_html(query_trimmed, limit);
        let searx_future = async {
            for instance in SEARXNG_INSTANCES.iter().take(2) {
                if let Ok(res) = search_searxng_instance(instance, query_trimmed, limit).await {
                    return Ok(res);
                }
            }
            Err("All SearXNG instances failed".to_owned())
        };

        let (ddg_res, searx_res) = tokio::join!(ddg_future, searx_future);

        if let Ok(results) = ddg_res {
            for item in results {
                if seen_urls.insert(item.url.clone()) {
                    combined.push(item);
                }
            }
        }

        if let Ok(results) = searx_res {
            for item in results {
                if seen_urls.insert(item.url.clone()) {
                    combined.push(item);
                }
            }
        }
    }

    let final_results: Vec<SearchResultItem> = combined.into_iter().take(limit).collect();

    // Store in cache
    {
        let mut cache = CACHE.lock().await;
        if cache.searches.len() > 100 {
            cache.searches.remove(0);
        }
        cache.searches.push((
            query_trimmed.to_owned(),
            CacheEntry {
                data: final_results.clone(),
                timestamp: Instant::now(),
            },
        ));
    }

    Ok(format_search_response(query_trimmed, &final_results))
}

fn format_search_response(query: &str, results: &[SearchResultItem]) -> Value {
    json!({
        "query": query,
        "total_results": results.len(),
        "results": results.iter().map(|item| json!({
            "title": item.title,
            "url": item.url,
            "snippet": item.snippet,
            "engine": item.engine,
        })).collect::<Vec<_>>()
    })
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

    let result = if page.content_type.contains("text/plain")
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
            },
        ));
    }

    Ok(result)
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
    use super::extract_ddg_target_url;

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
}
