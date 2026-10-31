//! Unit tests for child subagent routing, payloads, and stream parsing.

use super::{
    CHILD_MAX_OUTPUT_TOKENS, ChildRequestProtocol, Client, chat_completion_child_payload,
    chat_completion_tool, child_tool_definitions, compatible_child_route,
    parse_chat_completion_child_response, send_api_key_child_request_to, truncate_text,
};
use crate::chat_operation::SubagentModelOption;
use crate::provider_schema::ToolDefinition;
use serde_json::Value;
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::watch,
};

#[test]
fn compatible_child_route_preserves_the_selected_connection_id() {
    let model = SubagentModelOption {
        key: "provider/model".to_owned(),
        provider_id: "provider".to_owned(),
        model_id: "model".to_owned(),
        display_name: "Model".to_owned(),
        connection_id: None,
        workspace_id: None,
        api_key_connection_id: Some("provider-key-1".to_owned()),
        supports_tools: Some(true),
        reasoning_levels: Vec::new(),
        request_protocol: "chatCompletions".to_owned(),
        preferred: false,
    };

    let route = compatible_child_route(
        &model,
        ChildRequestProtocol::ChatCompletions,
        None,
        None,
        "https://provider.example/v1/chat/completions".to_owned(),
    );

    assert_eq!(route.connection_id.as_deref(), Some("provider-key-1"));
    assert_eq!(route.workspace_id, None);
}

#[test]
fn child_tool_allowlist_exposes_only_web_tools_without_a_project() {
    let names = child_tool_definitions(false, &[])
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["web_search", "read_url_content"]);
}

#[test]
fn project_child_allowlist_includes_only_approved_project_task_execution() {
    let without_tasks = child_tool_definitions(true, &[])
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    assert!(!without_tasks.iter().any(|name| name == "run_project_task"));

    let task_ids = vec!["verify".to_owned()];
    let definitions = child_tool_definitions(true, &task_ids);
    let task_schema = definitions
        .iter()
        .find(|tool| tool.name == "run_project_task")
        .map(|tool| &tool.parameters["properties"]["task"]["enum"]);
    #[cfg(windows)]
    assert_eq!(task_schema, Some(&json!(["verify"])));
    #[cfg(not(windows))]
    assert_eq!(task_schema, None);
    let names = definitions
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    for name in [
        "list_files",
        "search_files",
        "read_file",
        "get_file_info",
        "web_search",
        "read_url_content",
    ] {
        assert!(names.iter().any(|candidate| candidate == name), "{name}");
    }
    for name in [
        "write_file",
        "edit_file",
        "execute_command",
        "ask_user",
        "delegate_task",
    ] {
        assert!(!names.iter().any(|candidate| candidate == name), "{name}");
    }
    #[cfg(windows)]
    assert!(names.iter().any(|name| name == "run_project_task"));
    #[cfg(not(windows))]
    assert!(!names.iter().any(|name| name == "run_project_task"));
}

#[test]
fn child_result_truncation_preserves_utf8_boundaries() {
    let truncated = truncate_text("ábc", 2);
    assert!(truncated.starts_with('á'));
    assert!(truncated.ends_with("[Child output truncated.]"));
}

#[test]
fn chat_completion_child_tool_uses_the_supported_function_schema() {
    let definition = ToolDefinition {
        name: "read_file".to_owned(),
        description: "Read a project file".to_owned(),
        parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
    };

    assert_eq!(
        chat_completion_tool(&definition),
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a project file",
                "parameters": {"type": "object", "properties": {"path": {"type": "string"}}}
            }
        })
    );
}

#[test]
fn chat_completion_child_payload_keeps_the_allowlist_and_output_cap() {
    let definitions = crate::tools::definitions_for_chatgpt_model()
        .into_iter()
        .filter(|tool| tool.name == "read_file" || tool.name == "web_search")
        .collect::<Vec<_>>();
    let payload = chat_completion_child_payload(
        "gemini",
        "selected-model",
        None,
        &definitions,
        "Read-only instructions",
        "Delegated question",
    );

    assert_eq!(payload["model"], "selected-model");
    assert_eq!(payload["stream"], true);
    assert_eq!(payload["max_tokens"], CHILD_MAX_OUTPUT_TOKENS);
    assert!(payload.get("max_completion_tokens").is_none());
    assert!(payload.get("input").is_none());
    assert_eq!(payload["messages"][0]["role"], "system");
    assert_eq!(payload["messages"][1]["role"], "user");
    assert_eq!(payload["tool_choice"], "auto");
    let mut names = payload["tools"]
        .as_array()
        .expect("tool definitions")
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, vec!["read_file", "web_search"]);
}

#[test]
fn mistral_child_payload_carries_the_supported_reasoning_setting() {
    let payload = chat_completion_child_payload(
        "mistral",
        "selected-model",
        Some("high"),
        &[],
        "instructions",
        "task",
    );

    assert_eq!(payload["max_tokens"], CHILD_MAX_OUTPUT_TOKENS);
    assert_eq!(payload["reasoning_effort"], "high");
}

#[test]
fn parses_chat_completion_child_text_and_tool_calls() {
    let value = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "I will inspect the files.",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"README.md\"}"
                    }
                }]
            }
        }]
    });

    let (calls, summary, assistant_message) =
        parse_chat_completion_child_response(&value).expect("valid child response");

    assert_eq!(summary, "I will inspect the files.");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_1");
    assert_eq!(calls[0].name, "read_file");
    assert_eq!(calls[0].arguments, json!({"path": "README.md"}));
    assert_eq!(assistant_message["tool_calls"][0]["id"], "call_1");
}

#[test]
fn rejects_malformed_or_non_object_chat_completion_tool_arguments() {
    for arguments in ["{", "[]"] {
        let value = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "function": {"name": "read_file", "arguments": arguments}
                    }]
                }
            }]
        });

        assert_eq!(
            parse_chat_completion_child_response(&value)
                .expect_err("invalid function arguments should be rejected")
                .code,
            "invalid_provider_response"
        );
    }
}

#[tokio::test]
async fn api_key_child_request_sends_credentials_only_as_bearer_auth() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local request listener");
    let endpoint = format!(
        "http://{}/responses",
        listener.local_addr().expect("read listener address")
    );
    let request = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept API request");
        let mut bytes = Vec::new();
        let header_end = loop {
            let mut chunk = [0u8; 2048];
            let read = stream.read(&mut chunk).await.expect("read request");
            assert_ne!(read, 0, "request closed before its headers arrived");
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]).to_ascii_lowercase();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .and_then(|length| length.trim().parse::<usize>().ok())
            .expect("JSON request content length");
        while bytes.len() < header_end + content_length {
            let mut chunk = [0u8; 2048];
            let read = stream.read(&mut chunk).await.expect("read request body");
            assert_ne!(read, 0, "request closed before its body arrived");
            bytes.extend_from_slice(&chunk[..read]);
        }
        let body = String::from_utf8_lossy(&bytes[header_end..header_end + content_length]);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .await
            .expect("send local response");
        (headers, body.into_owned())
    });

    let http = Client::new();
    let (_cancel, mut cancellation) = watch::channel(false);
    let response = send_api_key_child_request_to(
        &http,
        &endpoint,
        Some("test-api-key"),
        &json!({"model": "test-model", "input": "hello"}),
        &mut cancellation,
    )
    .await
    .expect("send API request")
    .expect("request was not cancelled");
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let (headers, body) = request.await.expect("finish local request capture");
    assert!(headers.contains("authorization: bearer test-api-key"));
    assert!(!body.contains("test-api-key"));
    assert!(body.contains("test-model"));
}

#[tokio::test]
async fn local_child_request_can_use_a_loopback_endpoint_without_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local request listener");
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().expect("read listener address")
    );
    let request = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept local request");
        let mut bytes = Vec::new();
        let header_end = loop {
            let mut chunk = [0u8; 2048];
            let read = stream.read(&mut chunk).await.expect("read request");
            assert_ne!(read, 0, "request closed before its headers arrived");
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]).to_ascii_lowercase();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .and_then(|length| length.trim().parse::<usize>().ok())
            .expect("JSON request content length");
        while bytes.len() < header_end + content_length {
            let mut chunk = [0u8; 2048];
            let read = stream.read(&mut chunk).await.expect("read request body");
            assert_ne!(read, 0, "request closed before its body arrived");
            bytes.extend_from_slice(&chunk[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .await
            .expect("send local response");
        headers
    });

    let http = Client::new();
    let (_cancel, mut cancellation) = watch::channel(false);
    let response = send_api_key_child_request_to(
        &http,
        &endpoint,
        None,
        &json!({"model": "local-model", "messages": [{"role": "user", "content": "hello"}]}),
        &mut cancellation,
    )
    .await
    .expect("send local request")
    .expect("request was not cancelled");
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let headers = request.await.expect("finish local request capture");
    assert!(!headers.contains("authorization:"));
}
