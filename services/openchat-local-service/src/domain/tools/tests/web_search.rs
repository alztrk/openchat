//! Split from tests.rs (3 items).
use super::*;

#[test]
fn web_search_clean_html_tags_and_ddg_html_parsing() {
    use super::super::web_search::{clean_html_tags, parse_duckduckgo_html};

    let messy_text =
        "<b>Hello &amp; Welcome</b> to the &quot;OpenChat&quot; test! &#39;Fast&#39; &lt;&gt;";
    assert_eq!(
        clean_html_tags(messy_text),
        "Hello & Welcome to the \"OpenChat\" test! 'Fast' <>"
    );

    let sample_ddg_html = r#"
        <div class="result results_links">
            <h2 class="result__title">
                <a class="result__a" href="/l/?kh=-1&uddg=https%3A%2F%2Fexample.com%2Ftarget">Example <b>Title</b></a>
            </h2>
            <a class="result__snippet" href="/l/?kh=-1&uddg=https%3A%2F%2Fexample.com%2Ftarget">This is the <b>snippet</b> description.</a>
        </div>
    "#;

    let parsed = parse_duckduckgo_html(sample_ddg_html, 5);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].title, "Example Title");
    assert_eq!(parsed[0].url, "https://example.com/target");
    assert_eq!(parsed[0].snippet, "This is the snippet description.");
    assert_eq!(parsed[0].engine, "duckduckgo");
}

#[test]
fn web_search_tool_wire_and_internal_names() {
    use super::super::{internal_tool_name, opencode_wire_name};

    assert_eq!(opencode_wire_name("web_search"), "web_search");
    assert_eq!(opencode_wire_name("read_url_content"), "read_url_content");
    assert_eq!(opencode_wire_name("read_url"), "read_url_content");

    assert_eq!(internal_tool_name(true, "web_search"), "web_search");
    assert_eq!(
        internal_tool_name(true, "read_url_content"),
        "read_url_content"
    );
    assert_eq!(internal_tool_name(true, "read_url"), "read_url_content");
}

#[test]
fn tool_executor_prepares_web_search_and_read_url() {
    use super::super::executor::{ToolExecutor, ToolOperation, ToolPermissionMode};
    use crate::provider_schema::ToolCall;

    let temp_dir = std::env::temp_dir();
    let executor = ToolExecutor::new(
        Some(&temp_dir),
        &temp_dir,
        ToolPermissionMode::RequireApproval,
    );

    // 1. Valid web_search
    let call = ToolCall {
        id: "call-ws-1".to_owned(),
        name: "web_search".to_owned(),
        arguments: json!({"query": "rust async tokio", "limit": 3}),
    };
    let prepared = executor.prepare_call(&call).expect("prepare web_search");
    assert_eq!(prepared.requested_path, "rust async tokio");
    if let ToolOperation::WebSearch { query, limit } = prepared.operation {
        assert_eq!(query, "rust async tokio");
        assert_eq!(limit, Some(3));
    } else {
        panic!("expected WebSearch operation");
    }

    // 2. Empty query error
    let invalid_call = ToolCall {
        id: "call-ws-2".to_owned(),
        name: "web_search".to_owned(),
        arguments: json!({"query": "   "}),
    };
    assert!(executor.prepare_call(&invalid_call).is_err());

    // 3. Valid read_url_content
    let read_call = ToolCall {
        id: "call-ru-1".to_owned(),
        name: "read_url_content".to_owned(),
        arguments: json!({"url": "https://example.com/article", "max_chars": 2000}),
    };
    let prepared_read = executor
        .prepare_call(&read_call)
        .expect("prepare read_url_content");
    assert_eq!(prepared_read.requested_path, "https://example.com/article");
    if let ToolOperation::ReadUrlContent { url, max_chars } = prepared_read.operation {
        assert_eq!(url, "https://example.com/article");
        assert_eq!(max_chars, Some(2000));
    } else {
        panic!("expected ReadUrlContent operation");
    }
}
