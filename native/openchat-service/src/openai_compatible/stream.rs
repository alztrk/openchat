use std::collections::BTreeMap;

use serde_json::Value;

use crate::{protocol::ServiceError, provider_schema::ToolCall, tools};

use super::invalid_response_error;

#[derive(Default)]
pub(super) struct StreamedToolCall {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) arguments: String,
}

#[derive(Clone, Copy, Default)]
pub(super) struct ProviderRequestUsage {
    pub(super) prompt_tokens: Option<i64>,
    pub(super) completion_tokens: Option<i64>,
    pub(super) cached_tokens: Option<i64>,
    pub(super) cache_write_tokens: Option<i64>,
    pub(super) cache_discount: Option<f64>,
}

pub(super) enum SseLine {
    Ignore,
    Done,
    Data(Value),
}

pub(super) fn pop_sse_line(pending: &mut Vec<u8>, at_end_of_stream: bool) -> Option<Vec<u8>> {
    let newline = pending.iter().position(|byte| *byte == b'\n');
    let end = match newline {
        Some(newline) => newline + 1,
        None if at_end_of_stream && !pending.is_empty() => pending.len(),
        None => return None,
    };
    let mut line = pending.drain(..end).collect::<Vec<_>>();
    if newline.is_some() {
        line.pop();
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    Some(line)
}

pub(super) fn parse_sse_line(line: &[u8]) -> Result<SseLine, ()> {
    let line = String::from_utf8(line.to_vec()).map_err(|_| ())?;
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(SseLine::Ignore);
    };
    let data = data.trim_start();
    if data.is_empty() {
        return Ok(SseLine::Ignore);
    }
    if data == "[DONE]" {
        return Ok(SseLine::Done);
    }
    serde_json::from_str(data)
        .map(SseLine::Data)
        .map_err(|_| ())
}

pub(super) fn finish_reason(value: &Value) -> Option<&str> {
    value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.is_empty())
}

pub(super) fn stream_is_complete(saw_done: bool, saw_finish_reason: bool) -> bool {
    saw_done || saw_finish_reason
}

pub(super) fn update_chat_completion_usage(
    value: &Value,
    input_tokens: &mut Option<i64>,
    output_tokens: &mut Option<i64>,
) {
    if let Some(tokens) = value
        .pointer("/usage/prompt_tokens")
        .and_then(Value::as_i64)
        .filter(|tokens| *tokens >= 0)
    {
        *input_tokens = Some(tokens);
    }
    if let Some(tokens) = value
        .pointer("/usage/completion_tokens")
        .and_then(Value::as_i64)
        .filter(|tokens| *tokens >= 0)
    {
        *output_tokens = Some(output_tokens.unwrap_or(0).saturating_add(tokens));
    }
}

pub(super) fn update_provider_request_usage(
    value: &Value,
    usage: &mut Option<ProviderRequestUsage>,
) {
    let current = ProviderRequestUsage {
        prompt_tokens: value
            .pointer("/usage/prompt_tokens")
            .and_then(Value::as_i64)
            .filter(|tokens| *tokens >= 0),
        completion_tokens: value
            .pointer("/usage/completion_tokens")
            .and_then(Value::as_i64)
            .filter(|tokens| *tokens >= 0),
        cached_tokens: value
            .pointer("/usage/prompt_tokens_details/cached_tokens")
            .and_then(Value::as_i64)
            .filter(|tokens| *tokens >= 0),
        cache_write_tokens: value
            .pointer("/usage/prompt_tokens_details/cache_write_tokens")
            .and_then(Value::as_i64)
            .filter(|tokens| *tokens >= 0),
        cache_discount: value
            .get("cache_discount")
            .and_then(Value::as_f64)
            .filter(|discount| discount.is_finite()),
    };
    if current.prompt_tokens.is_none()
        && current.completion_tokens.is_none()
        && current.cached_tokens.is_none()
        && current.cache_write_tokens.is_none()
        && current.cache_discount.is_none()
    {
        return;
    }

    let previous = *usage;
    *usage = Some(ProviderRequestUsage {
        prompt_tokens: current
            .prompt_tokens
            .or_else(|| previous.and_then(|usage| usage.prompt_tokens)),
        completion_tokens: current
            .completion_tokens
            .or_else(|| previous.and_then(|usage| usage.completion_tokens)),
        cached_tokens: current
            .cached_tokens
            .or_else(|| previous.and_then(|usage| usage.cached_tokens)),
        cache_write_tokens: current
            .cache_write_tokens
            .or_else(|| previous.and_then(|usage| usage.cache_write_tokens)),
        cache_discount: current
            .cache_discount
            .or_else(|| previous.and_then(|usage| usage.cache_discount)),
    });
}

pub(super) fn append_tool_call_deltas(
    calls: &mut BTreeMap<usize, StreamedToolCall>,
    deltas: &[Value],
) -> Result<(), ServiceError> {
    for delta in deltas {
        let index = delta
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(invalid_response_error)?;
        if !calls.contains_key(&index) && calls.len() >= tools::MAX_TOOL_CALLS_PER_TURN {
            return Err(tools::tool_call_limit_error());
        }
        let call = calls.entry(index).or_default();
        if let Some(id) = delta.get("id").and_then(Value::as_str) {
            call.id.push_str(id);
        }
        if let Some(function) = delta.get("function") {
            if let Some(name) = function.get("name").and_then(Value::as_str) {
                call.name.push_str(name);
            }
            if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                if call.arguments.len().saturating_add(arguments.len())
                    > tools::MAX_TOOL_ARGUMENT_BYTES
                {
                    return Err(invalid_response_error());
                }
                call.arguments.push_str(arguments);
            }
        }
    }
    Ok(())
}

pub(super) fn parse_streamed_tool_calls(
    calls: BTreeMap<usize, StreamedToolCall>,
    is_opencode: bool,
) -> Result<Vec<ToolCall>, ServiceError> {
    calls
        .into_values()
        .map(|call| {
            if call.id.is_empty() || call.name.is_empty() {
                return Err(invalid_response_error());
            }
            let name = tools::internal_tool_name(is_opencode, &call.name);
            Ok(ToolCall {
                id: call.id,
                name,
                arguments: serde_json::from_str(&call.arguments)
                    .map_err(|_| invalid_response_error())?,
            })
        })
        .collect()
}

pub(super) enum ResponsesStreamEffect {
    TextDelta(String),
    ReasoningDelta(String),
    UsageTokens(i64),
    Completed,
    Failed(String),
    None,
}

pub(super) fn handle_responses_api_event(
    value: &Value,
    calls: &mut BTreeMap<usize, StreamedToolCall>,
) -> Result<ResponsesStreamEffect, ServiceError> {
    let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    match event_type {
        "response.output_text.delta" => {
            if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                return Ok(ResponsesStreamEffect::TextDelta(delta.to_owned()));
            }
        }
        "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
            if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                return Ok(ResponsesStreamEffect::ReasoningDelta(delta.to_owned()));
            }
        }
        "response.output_item.added" => {
            if let Some(item) = value.get("item")
                && item.get("type").and_then(Value::as_str) == Some("function_call")
            {
                let index = response_output_index(value)?;
                let call = response_tool_call(calls, index)?;
                if let Some(id) = item.get("call_id").and_then(Value::as_str) {
                    call.id = id.to_owned();
                }
                if let Some(name) = item.get("name").and_then(Value::as_str) {
                    call.name = name.to_owned();
                }
                if let Some(arguments) = item.get("arguments").and_then(Value::as_str)
                    && !arguments.is_empty()
                {
                    validate_tool_arguments(arguments)?;
                    call.arguments = arguments.to_owned();
                }
            }
        }
        "response.function_call_arguments.delta" => {
            let index = response_output_index(value)?;
            let delta = value
                .get("delta")
                .and_then(Value::as_str)
                .ok_or_else(invalid_response_error)?;
            let call = existing_response_tool_call(calls, index)?;
            if call.arguments.len().saturating_add(delta.len()) > tools::MAX_TOOL_ARGUMENT_BYTES {
                return Err(invalid_response_error());
            }
            call.arguments.push_str(delta);
        }
        "response.function_call_arguments.done" => {
            let index = response_output_index(value)?;
            let call = existing_response_tool_call(calls, index)?;
            if let Some(name) = value.get("name").and_then(Value::as_str)
                && call.name.is_empty()
            {
                call.name = name.to_owned();
            }
            if let Some(arguments) = value.get("arguments").and_then(Value::as_str) {
                validate_tool_arguments(arguments)?;
                call.arguments = arguments.to_owned();
            }
        }
        "response.output_item.done" => {
            if let Some(item) = value.get("item")
                && item.get("type").and_then(Value::as_str) == Some("function_call")
            {
                let index = response_output_index(value)?;
                let call = existing_response_tool_call(calls, index)?;
                if let Some(id) = item.get("call_id").and_then(Value::as_str)
                    && call.id.is_empty()
                {
                    call.id = id.to_owned();
                }
                if let Some(name) = item.get("name").and_then(Value::as_str)
                    && call.name.is_empty()
                {
                    call.name = name.to_owned();
                }
                if let Some(arguments) = item.get("arguments").and_then(Value::as_str)
                    && call.arguments.is_empty()
                {
                    validate_tool_arguments(arguments)?;
                    call.arguments = arguments.to_owned();
                }
            }
        }
        "response.completed" => {
            let output_tokens = value
                .pointer("/response/usage/output_tokens")
                .and_then(Value::as_i64);
            if let Some(output) = value.pointer("/response/output").and_then(Value::as_array) {
                for (idx, item) in output.iter().enumerate() {
                    if item.get("type").and_then(Value::as_str) == Some("function_call") {
                        let call = response_tool_call(calls, idx)?;
                        if let Some(id) = item.get("call_id").and_then(Value::as_str)
                            && call.id.is_empty()
                        {
                            call.id = id.to_owned();
                        }
                        if let Some(name) = item.get("name").and_then(Value::as_str)
                            && call.name.is_empty()
                        {
                            call.name = name.to_owned();
                        }
                        if let Some(arguments) = item.get("arguments").and_then(Value::as_str)
                            && call.arguments.is_empty()
                        {
                            validate_tool_arguments(arguments)?;
                            call.arguments = arguments.to_owned();
                        }
                    }
                }
            }
            if let Some(tokens) = output_tokens {
                return Ok(ResponsesStreamEffect::UsageTokens(tokens));
            }
            return Ok(ResponsesStreamEffect::Completed);
        }
        "response.failed" => {
            let message = value
                .pointer("/response/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Model response failed");
            return Ok(ResponsesStreamEffect::Failed(message.to_owned()));
        }
        _ => {}
    }
    Ok(ResponsesStreamEffect::None)
}

fn response_output_index(value: &Value) -> Result<usize, ServiceError> {
    value
        .get("output_index")
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(invalid_response_error)
}

fn response_tool_call(
    calls: &mut BTreeMap<usize, StreamedToolCall>,
    index: usize,
) -> Result<&mut StreamedToolCall, ServiceError> {
    if !calls.contains_key(&index) && calls.len() >= tools::MAX_TOOL_CALLS_PER_TURN {
        return Err(tools::tool_call_limit_error());
    }
    Ok(calls.entry(index).or_default())
}

fn existing_response_tool_call(
    calls: &mut BTreeMap<usize, StreamedToolCall>,
    index: usize,
) -> Result<&mut StreamedToolCall, ServiceError> {
    calls.get_mut(&index).ok_or_else(invalid_response_error)
}

fn validate_tool_arguments(arguments: &str) -> Result<(), ServiceError> {
    if arguments.len() > tools::MAX_TOOL_ARGUMENT_BYTES {
        return Err(invalid_response_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_usage_only_stream_chunks_without_choices() {
        let event = json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 1200,
                "completion_tokens": 85
            }
        });
        let mut input_tokens = None;
        let mut output_tokens = None;

        update_chat_completion_usage(&event, &mut input_tokens, &mut output_tokens);

        assert_eq!(input_tokens, Some(1200));
        assert_eq!(output_tokens, Some(85));
    }

    #[test]
    fn ignores_invalid_usage_values_and_accumulates_output_tokens() {
        let event = json!({
            "usage": {
                "prompt_tokens": -1,
                "completion_tokens": -1
            }
        });
        let mut input_tokens = Some(42);
        let mut output_tokens = Some(7);

        update_chat_completion_usage(&event, &mut input_tokens, &mut output_tokens);

        assert_eq!(input_tokens, Some(42));
        assert_eq!(output_tokens, Some(7));

        let next_event = json!({"usage": {"completion_tokens": 5}});
        update_chat_completion_usage(&next_event, &mut input_tokens, &mut output_tokens);

        assert_eq!(output_tokens, Some(12));
    }

    #[test]
    fn responses_tool_events_require_valid_indices_and_started_calls() {
        let mut calls = BTreeMap::new();

        for event in [
            json!({"type": "response.output_item.added", "item": {"type": "function_call", "name": "search"}}),
            json!({"type": "response.output_item.added", "output_index": -1, "item": {"type": "function_call", "name": "search"}}),
            json!({"type": "response.output_item.added", "output_index": "0", "item": {"type": "function_call", "name": "search"}}),
            json!({"type": "response.function_call_arguments.delta", "delta": "{}"}),
            json!({"type": "response.function_call_arguments.delta", "output_index": 4, "delta": "{}"}),
            json!({"type": "response.function_call_arguments.done", "output_index": 4, "arguments": "{}"}),
            json!({"type": "response.output_item.done", "output_index": 4, "item": {"type": "function_call", "name": "search"}}),
        ] {
            assert!(
                handle_responses_api_event(&event, &mut calls).is_err(),
                "event should be rejected: {event}"
            );
        }
        assert!(calls.is_empty());
    }

    #[test]
    fn responses_tool_events_reassemble_arguments_by_output_index() {
        let mut calls = BTreeMap::new();
        handle_responses_api_event(
            &json!({
                "type": "response.output_item.added",
                "output_index": 2,
                "item": {"type": "function_call", "call_id": "call_1", "name": "search"}
            }),
            &mut calls,
        )
        .expect("add tool call");
        handle_responses_api_event(
            &json!({
                "type": "response.function_call_arguments.delta",
                "output_index": 2,
                "delta": "{\"query\":"
            }),
            &mut calls,
        )
        .expect("append first argument chunk");
        handle_responses_api_event(
            &json!({
                "type": "response.function_call_arguments.done",
                "output_index": 2,
                "arguments": "{\"query\":\"rust\"}"
            }),
            &mut calls,
        )
        .expect("finish tool arguments");

        assert_eq!(calls.len(), 1);
        let call = calls.get(&2).expect("call at output index two");
        assert_eq!(call.id, "call_1");
        assert_eq!(call.name, "search");
        assert_eq!(call.arguments, "{\"query\":\"rust\"}");
    }

    #[test]
    fn responses_completed_event_enforces_tool_count_limit() {
        let mut calls = (0..tools::MAX_TOOL_CALLS_PER_TURN)
            .map(|index| (index, StreamedToolCall::default()))
            .collect::<BTreeMap<_, _>>();
        let output = (0..=tools::MAX_TOOL_CALLS_PER_TURN)
            .map(|_| json!({"type": "function_call", "call_id": "call", "name": "search", "arguments": "{}"}))
            .collect::<Vec<_>>();
        let event = json!({"type": "response.completed", "response": {"output": output}});

        let result = handle_responses_api_event(&event, &mut calls);

        assert!(matches!(
            result,
            Err(error) if error.code == "tool_iteration_limit"
        ));
        assert_eq!(calls.len(), tools::MAX_TOOL_CALLS_PER_TURN);
    }
}
