use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    chatgpt_store, goals, history, instructions,
    protocol::ServiceError,
    provider_schema::ProviderChatRequest,
    storage::AppStorage,
    tools::{self, ToolPermissionMode},
};

use super::{chat_completion_tool, invalid_retry_target_error};

pub(super) struct ProviderRequestOptions<'a> {
    pub(super) model_id: String,
    pub(super) provider_id: &'a str,
    pub(super) excluded_assistant_message_id: Option<&'a str>,
    pub(super) custom_instructions: Option<&'a str>,
    pub(super) permission_mode: ToolPermissionMode,
    pub(super) has_project: bool,
    pub(super) reasoning_effort: Option<&'a str>,
    pub(super) supports_tool_calls: Option<bool>,
    pub(super) goal_objective: Option<&'a str>,
}

pub(super) fn build_provider_request(
    storage: &AppStorage,
    stored_messages: &[chatgpt_store::StoredMessage],
    options: ProviderRequestOptions<'_>,
) -> Result<ProviderChatRequest, ServiceError> {
    let ProviderRequestOptions {
        model_id,
        provider_id,
        excluded_assistant_message_id,
        custom_instructions,
        permission_mode,
        has_project,
        reasoning_effort,
        supports_tool_calls,
        goal_objective,
    } = options;
    history::validate_attachments(stored_messages)?;
    let reasoning_effort = match (provider_id, reasoning_effort) {
        ("opencode", Some(level)) => {
            if !super::models::supports_reasoning_level(storage, &model_id, level)? {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "The selected reasoning level is not supported by this OpenCode model.",
                    false,
                ));
            }
            Some(level.to_owned())
        }
        ("mistral", Some(level)) => {
            if !super::provider_models::mistral_reasoning_levels(&model_id)
                .iter()
                .any(|supported| supported == level)
            {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "The selected reasoning level is not supported by this Mistral model.",
                    false,
                ));
            }
            Some(level.to_owned())
        }
        _ => None,
    };
    if excluded_assistant_message_id.is_some_and(|id| {
        !chatgpt_store::is_retryable_latest_assistant_message(stored_messages, id)
    }) {
        return Err(invalid_retry_target_error());
    }
    let included_messages = stored_messages
        .iter()
        .filter(|message| Some(message.id.as_str()) != excluded_assistant_message_id)
        .collect::<Vec<_>>();
    let last_message_id = included_messages.last().map(|message| message.id.clone());
    let messages = included_messages
        .iter()
        .map(|message| history::provider_messages_for_provider(message, provider_id))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if messages.is_empty() {
        return Err(ServiceError::new(
            "conversation_empty",
            "Write a message before starting the response.",
            false,
        ));
    }

    let mut tools = tools::definitions_for_request(provider_id, supports_tool_calls);
    if has_project && !tools.is_empty() {
        tools.push(tools::project_tasks::tool_definition());
    }
    if goal_objective.is_some() && tools.is_empty() {
        return Err(ServiceError::new(
            "goal_tool_calls_unavailable",
            "Goal mode requires a model that supports tool calling. Select a model with tool support.",
            false,
        ));
    }
    if !tools.is_empty() {
        tools.extend(goals::control_tool_definitions());
    }
    let mut shared_instructions = instructions::shared_instructions(
        custom_instructions,
        permission_mode,
        has_project,
        !tools.is_empty(),
        provider_id,
    );
    if !tools.is_empty() {
        goals::append_goal_tool_instructions(&mut shared_instructions);
    }
    if let Some(objective) = goal_objective {
        goals::append_goal_instructions(&mut shared_instructions, objective);
    }

    Ok(ProviderChatRequest {
        model: model_id,
        instructions: shared_instructions,
        messages,
        last_message_id,
        tools,
        reasoning_effort,
    })
}

pub(super) fn completion_messages(request: &ProviderChatRequest, provider_id: &str) -> Vec<Value> {
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    messages.push(json!({
        "role": "system",
        "content": request.instructions,
    }));
    messages.extend(request.messages.iter().map(|message| {
        let mut serialized = json!({
            "role": message.role.as_str(),
            "content": if message.images.is_empty() {
                json!(message.content)
            } else {
                let mut parts = Vec::with_capacity(message.images.len().saturating_add(1));
                if !message.content.is_empty() {
                    parts.push(json!({"type": "text", "text": message.content}));
                }
                parts.extend(message.images.iter().map(|url| json!({
                    "type": "image_url",
                    "image_url": {"url": url, "detail": "auto"},
                })));
                json!(parts)
            },
        });
        if !message.tool_calls.is_empty() {
            serialized["content"] = if message.content.is_empty() {
                Value::Null
            } else {
                json!(message.content)
            };
            serialized["tool_calls"] = json!(
                message
                    .tool_calls
                    .iter()
                    .map(|call| json!({
                        "id": call.id,
                        "type": "function",
                        "function": {
                                "name": if provider_id == "opencode" {
                                    tools::opencode_wire_name(&call.name)
                                } else {
                                        call.name.clone()
                                },
                            "arguments": call.arguments.to_string(),
                        },
                    }))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(tool_call_id) = message.tool_call_id.as_deref() {
            serialized["tool_call_id"] = json!(tool_call_id);
        }
        serialized
    }));
    messages
}

pub(super) fn chat_completion_body(
    request: &ProviderChatRequest,
    messages: &[Value],
    provider_id: &str,
) -> Value {
    let mut body = json!({
        "model": request.model,
        "messages": messages,
        "stream": true,
    });
    if matches!(
        provider_id,
        "chatgpt_api" | "opencode" | "gemini" | "groq" | "mistral" | "openrouter"
    ) {
        body["stream_options"] = json!({"include_usage": true});
    }
    if provider_id == "opencode" {
        if !request.tools.is_empty() {
            body["tools"] = json!(tools::opencode_wire_tools(&request.tools));
            body["tool_choice"] = json!("auto");
            body["parallel_tool_calls"] = json!(false);
        }
        if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
            body["reasoning_effort"] = json!(reasoning_effort);
        }
    } else {
        if provider_id == "mistral"
            && let Some(reasoning_effort) = request.reasoning_effort.as_deref()
        {
            body["reasoning_effort"] = json!(reasoning_effort);
        }
        if request.tools.is_empty() {
            return body;
        }
        body["tools"] = json!(
            request
                .tools
                .iter()
                .map(chat_completion_tool)
                .collect::<Vec<_>>()
        );
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(false);
    }
    body
}

pub(super) fn apply_fast_mode(body: &mut Value, provider_id: &str, enabled: bool) {
    if provider_id == "chatgpt_api" && enabled {
        body["service_tier"] = json!("fast");
    }
}

pub(super) fn apply_model_output_limit(
    body: &mut Value,
    provider_id: &str,
    uses_responses_api: bool,
    max_output_tokens: Option<i64>,
) {
    let Some(max_output_tokens) = max_output_tokens.filter(|limit| *limit > 0) else {
        return;
    };
    let field = if uses_responses_api {
        "max_output_tokens"
    } else if matches!(provider_id, "gemini" | "mistral" | "opencode") {
        "max_tokens"
    } else {
        "max_completion_tokens"
    };
    body[field] = json!(max_output_tokens);
}

pub(super) fn apply_prompt_cache_affinity(
    body: &mut Value,
    provider_id: &str,
    connection_id: Option<&str>,
    model_id: &str,
    conversation_id: &str,
) {
    let field = match provider_id {
        "cerebras" | "mistral" => "prompt_cache_key",
        "openrouter" => "session_id",
        _ => return,
    };

    let mut hasher = Sha256::new();
    hasher.update(b"openchat-prompt-cache-v1\0");
    for part in [
        provider_id.as_bytes(),
        connection_id.unwrap_or_default().as_bytes(),
        model_id.as_bytes(),
        conversation_id.as_bytes(),
    ] {
        hasher.update(part);
        hasher.update([0]);
    }
    body[field] = json!(format!("oc_{:x}", hasher.finalize()));
}

pub(super) fn responses_input_items(messages: &[Value]) -> Vec<Value> {
    let mut items = Vec::new();
    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        match role {
            "system" => {
                if let Some(content) = response_content(msg.get("content")) {
                    items.push(json!({
                        "role": "developer",
                        "content": content
                    }));
                }
            }
            "user" => {
                if let Some(content) = response_content(msg.get("content")) {
                    items.push(json!({
                        "role": "user",
                        "content": content
                    }));
                }
            }
            "assistant" => {
                if let Some(content) = msg.get("content").and_then(Value::as_str)
                    && !content.is_empty()
                {
                    items.push(json!({
                        "role": "assistant",
                        "content": [{
                            "type": "output_text",
                            "text": content,
                        }]
                    }));
                }
                if let Some(tool_calls) = msg.get("tool_calls").and_then(Value::as_array) {
                    for call in tool_calls {
                        let id = call.get("id").and_then(Value::as_str).unwrap_or("");
                        let name = call
                            .pointer("/function/name")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let arguments = call
                            .pointer("/function/arguments")
                            .and_then(Value::as_str)
                            .unwrap_or("{}");
                        items.push(json!({
                            "type": "function_call",
                            "call_id": id,
                            "name": name,
                            "arguments": arguments,
                        }));
                    }
                }
            }
            "tool" => {
                let call_id = msg
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let content = msg.get("content").and_then(Value::as_str).unwrap_or("");
                items.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": content,
                }));
            }
            _ => {}
        }
    }
    items
}

fn response_content(content: Option<&Value>) -> Option<Vec<Value>> {
    match content? {
        Value::String(text) => Some(vec![json!({
            "type": "input_text",
            "text": text,
        })]),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                    Some("text") => part
                        .get("text")
                        .and_then(Value::as_str)
                        .map(|text| json!({"type": "input_text", "text": text})),
                    Some("image_url") => part
                        .pointer("/image_url/url")
                        .and_then(Value::as_str)
                        .map(|image_url| {
                            json!({
                                "type": "input_image",
                                "image_url": image_url,
                                "detail": "auto",
                            })
                        }),
                    _ => None,
                })
                .collect(),
        ),
        _ => None,
    }
}

pub(super) fn responses_api_body(request: &ProviderChatRequest, messages: &[Value]) -> Value {
    let mut body = json!({
        "model": request.model,
        "input": responses_input_items(messages),
        "stream": true,
    });
    if !request.tools.is_empty() {
        body["tools"] = json!(tools::opencode_responses_wire_tools(&request.tools));
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(true);
    }
    if let Some(reasoning_effort) = request.reasoning_effort.as_deref() {
        body["reasoning"] = json!({
            "effort": reasoning_effort,
        });
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ProviderChatRequest {
        ProviderChatRequest {
            model: "test-model".to_owned(),
            instructions: String::new(),
            messages: Vec::new(),
            last_message_id: None,
            tools: Vec::new(),
            reasoning_effort: None,
        }
    }

    #[test]
    fn requests_usage_for_streaming_providers_that_need_the_flag() {
        for provider_id in [
            "chatgpt_api",
            "opencode",
            "gemini",
            "groq",
            "mistral",
            "openrouter",
        ] {
            let body = chat_completion_body(&request(), &[], provider_id);

            assert_eq!(
                body["stream_options"]["include_usage"], true,
                "{provider_id}"
            );
        }
    }

    #[test]
    fn leaves_usage_flags_out_for_cerebras_automatic_stream_usage() {
        for provider_id in ["cerebras"] {
            let body = chat_completion_body(&request(), &[], provider_id);

            assert!(body.get("stream_options").is_none(), "{provider_id}");
            assert!(body.get("usage").is_none(), "{provider_id}");
        }
    }

    #[test]
    fn keeps_opencode_tool_request_shape() {
        let mut request = request();
        request.tools = tools::definitions_for_provider("opencode");
        let body = chat_completion_body(&request, &[], "opencode");
        let tool_names = body["tools"]
            .as_array()
            .expect("OpenCode tools array")
            .iter()
            .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert!(tool_names.contains(&"bash"));
        assert!(tool_names.contains(&"read"));
        assert_eq!(body["tool_choice"], "auto");
        assert_eq!(body["parallel_tool_calls"], false);
    }

    #[test]
    fn omits_tool_fields_when_opencode_model_does_not_support_tools() {
        let body = chat_completion_body(&request(), &[], "opencode");

        assert!(body.get("tools").is_none());
        assert!(body.get("tool_choice").is_none());
        assert!(body.get("parallel_tool_calls").is_none());
        let body = responses_api_body(&request(), &[]);
        assert!(body.get("tools").is_none());
        assert!(body.get("tool_choice").is_none());
        assert!(body.get("parallel_tool_calls").is_none());
    }

    #[test]
    fn sends_mistral_reasoning_effort_with_stream_usage() {
        let mut request = request();
        request.reasoning_effort = Some("high".to_owned());
        let body = chat_completion_body(&request, &[], "mistral");

        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["stream_options"]["include_usage"], true);
    }

    #[test]
    fn restores_completed_tool_history_for_chat_completion_and_responses_routes() {
        let stored_message = crate::chatgpt_store::StoredMessage {
            id: "assistant-message".to_owned(),
            role: "assistant".to_owned(),
            content: "The setting is enabled.".to_owned(),
            status: "completed".to_owned(),
            output_tokens: None,
            tool_activities: vec![crate::provider_schema::ToolActivity {
                call_id: "call-42".to_owned(),
                name: "read_file".to_owned(),
                arguments: serde_json::json!({"path": "settings.json"}),
                round_id: None,
                assistant_text_before_byte_offset: None,
                target_path: None,
                output: Some(serde_json::json!({"content": "enabled"})),
                file_changes: Vec::new(),
                file_changes_error: None,
                status: crate::provider_schema::ToolActivityStatus::Completed,
            }],
            attachments: Vec::new(),
        };
        let mut request = request();
        request.messages = crate::history::provider_messages(&stored_message)
            .expect("valid tool activity history");

        let messages = completion_messages(&request, "opencode");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call-42");
        assert_eq!(messages[1]["tool_calls"][0]["function"]["name"], "read");
        assert_eq!(
            messages[1]["tool_calls"][0]["function"]["arguments"],
            r#"{"path":"settings.json"}"#
        );
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call-42");
        assert!(messages[2]["content"].as_str().unwrap().contains("enabled"));

        let responses_input = responses_input_items(&messages);
        assert_eq!(responses_input[1]["type"], "function_call");
        assert_eq!(responses_input[1]["call_id"], "call-42");
        assert_eq!(responses_input[2]["type"], "function_call_output");
        assert_eq!(responses_input[2]["call_id"], "call-42");
        assert_eq!(responses_input[3]["role"], "assistant");
        assert_eq!(
            responses_input[3]["content"][0]["text"],
            "The setting is enabled."
        );
    }
}
