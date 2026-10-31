//! Child model SSE streaming, event processing, and stream finalization.

use super::*;

#[derive(Default)]
struct ChildStreamState {
    response: Option<Value>,
    text: String,
    full_text: String,
    chat_tool_calls: BTreeMap<usize, Value>,
    chat_usage: Option<Value>,
    messages_usage: Value,
    messages_tools: BTreeMap<usize, StreamedMessagesTool>,
}

#[derive(Default)]
struct StreamedMessagesTool {
    id: String,
    name: String,
    initial_input: Option<Value>,
    partial_json: String,
}

enum ChildSseLine {
    Ignore,
    Done,
    Data(Value),
}

fn parse_child_sse_line(line: &[u8]) -> Result<ChildSseLine, ServiceError> {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let line = std::str::from_utf8(line).map_err(|_| subagent_response_error())?;
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(ChildSseLine::Ignore);
    };
    let data = data.trim();
    if data.is_empty() {
        return Ok(ChildSseLine::Ignore);
    }
    if data == "[DONE]" {
        return Ok(ChildSseLine::Done);
    }
    serde_json::from_str(data)
        .map(ChildSseLine::Data)
        .map_err(|_| subagent_response_error())
}

pub(super) async fn collect_child_stream(
    response: HttpResponse,
    protocol: ChildRequestProtocol,
    events: &EventSink,
    storage: &AppStorage,
    conversation_id: &str,
    child_run_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let mut stream = response.bytes_stream();
    let mut pending = Vec::new();
    let mut total_bytes = 0usize;
    let mut state = ChildStreamState {
        full_text: load_child_text(storage, conversation_id, child_run_id)?,
        ..ChildStreamState::default()
    };
    let mut stream_ended = false;

    while !stream_ended {
        let chunk = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Err(crate::permissions::operation_cancelled_error());
            }
            chunk = stream.next() => chunk,
        };
        if let Some(chunk) = chunk {
            let chunk = chunk.map_err(|_| subagent_response_error())?;
            total_bytes = total_bytes.saturating_add(chunk.len());
            if total_bytes > 4 * 1024 * 1024 {
                return Err(subagent_limit_error());
            }
            pending.extend_from_slice(&chunk);
        } else {
            stream_ended = true;
        }
        loop {
            let Some(line_end) = pending.iter().position(|byte| *byte == b'\n') else {
                if pending.len() > CHILD_MAX_EVENT_BYTES {
                    return Err(subagent_limit_error());
                }
                break;
            };
            if line_end > CHILD_MAX_EVENT_BYTES {
                return Err(subagent_limit_error());
            }
            let mut line = pending.drain(..=line_end).collect::<Vec<_>>();
            line.pop();
            match parse_child_sse_line(&line)? {
                ChildSseLine::Ignore | ChildSseLine::Done => {}
                ChildSseLine::Data(event) => {
                    process_child_stream_event(
                        &mut state,
                        protocol,
                        &event,
                        events,
                        storage,
                        conversation_id,
                        child_run_id,
                    )
                    .await?;
                }
            }
        }
        if stream_ended && !pending.is_empty() {
            match parse_child_sse_line(&pending)? {
                ChildSseLine::Ignore | ChildSseLine::Done => {}
                ChildSseLine::Data(event) => {
                    process_child_stream_event(
                        &mut state,
                        protocol,
                        &event,
                        events,
                        storage,
                        conversation_id,
                        child_run_id,
                    )
                    .await?;
                }
            }
            pending.clear();
        }
    }
    persist_child_text(storage, conversation_id, child_run_id, &state.full_text)?;
    finish_child_stream(state, protocol)
}

#[allow(clippy::too_many_arguments)]
async fn process_child_stream_event(
    state: &mut ChildStreamState,
    protocol: ChildRequestProtocol,
    event: &Value,
    events: &EventSink,
    storage: &AppStorage,
    conversation_id: &str,
    child_run_id: &str,
) -> Result<(), ServiceError> {
    if event.get("error").is_some()
        || matches!(
            event.get("type").and_then(Value::as_str),
            Some("error" | "response.failed")
        )
    {
        return Err(subagent_response_error());
    }
    match protocol {
        ChildRequestProtocol::Responses => match event.get("type").and_then(Value::as_str) {
            Some("response.output_text.delta") => {
                if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                    append_child_stream_text(
                        state,
                        delta,
                        events,
                        storage,
                        conversation_id,
                        child_run_id,
                    )
                    .await?;
                }
            }
            Some("response.completed") => {
                state.response = Some(
                    event
                        .get("response")
                        .cloned()
                        .unwrap_or_else(|| event.clone()),
                );
            }
            _ => {}
        },
        ChildRequestProtocol::ChatCompletions => {
            if let Some(delta) = event
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
            {
                append_child_stream_text(
                    state,
                    delta,
                    events,
                    storage,
                    conversation_id,
                    child_run_id,
                )
                .await?;
            }
            if let Some(calls) = event
                .pointer("/choices/0/delta/tool_calls")
                .and_then(Value::as_array)
            {
                for call_delta in calls {
                    let index = call_delta
                        .get("index")
                        .and_then(Value::as_u64)
                        .and_then(|index| usize::try_from(index).ok())
                        .filter(|index| *index < CHILD_MAX_TOOL_CALLS)
                        .ok_or_else(subagent_response_error)?;
                    let call = state.chat_tool_calls.entry(index).or_insert_with(|| {
                        json!({
                            "id": "",
                            "type": "function",
                            "function": {"name": "", "arguments": ""}
                        })
                    });
                    if let Some(id) = call_delta.get("id").and_then(Value::as_str) {
                        call["id"] = json!(id);
                    }
                    if let Some(function_delta) = call_delta.get("function") {
                        if let Some(name) = function_delta.get("name").and_then(Value::as_str) {
                            let current = call
                                .pointer("/function/name")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            call["function"]["name"] = json!(format!("{current}{name}"));
                        }
                        if let Some(arguments) =
                            function_delta.get("arguments").and_then(Value::as_str)
                        {
                            let current = call
                                .pointer("/function/arguments")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            let combined = format!("{current}{arguments}");
                            if combined.len() > tools::MAX_TOOL_ARGUMENT_BYTES {
                                return Err(subagent_limit_error());
                            }
                            call["function"]["arguments"] = json!(combined);
                        }
                    }
                }
            }
            if let Some(usage) = event.get("usage").filter(|usage| !usage.is_null()) {
                state.chat_usage = Some(usage.clone());
            }
        }
        ChildRequestProtocol::Messages => match event.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                if let Some(usage) = event.pointer("/message/usage") {
                    merge_messages_usage(&mut state.messages_usage, usage);
                }
            }
            Some("content_block_start") => {
                let index = event
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok())
                    .filter(|index| *index < CHILD_MAX_TOOL_CALLS)
                    .ok_or_else(subagent_response_error)?;
                if let Some(block) = event.get("content_block") {
                    if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                        let tool = state.messages_tools.entry(index).or_default();
                        tool.id = block
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        tool.name = block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        tool.initial_input = block.get("input").cloned();
                    } else if block.get("type").and_then(Value::as_str) == Some("text")
                        && let Some(text) = block.get("text").and_then(Value::as_str)
                    {
                        append_child_stream_text(
                            state,
                            text,
                            events,
                            storage,
                            conversation_id,
                            child_run_id,
                        )
                        .await?;
                    }
                }
            }
            Some("content_block_delta") => {
                let delta = event.get("delta").ok_or_else(subagent_response_error)?;
                match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        let text = delta
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        append_child_stream_text(
                            state,
                            text,
                            events,
                            storage,
                            conversation_id,
                            child_run_id,
                        )
                        .await?;
                    }
                    Some("input_json_delta") => {
                        let index = event
                            .get("index")
                            .and_then(Value::as_u64)
                            .and_then(|index| usize::try_from(index).ok())
                            .filter(|index| *index < CHILD_MAX_TOOL_CALLS)
                            .ok_or_else(subagent_response_error)?;
                        let partial = delta
                            .get("partial_json")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let tool = state.messages_tools.entry(index).or_default();
                        tool.partial_json.push_str(partial);
                        if tool.partial_json.len() > tools::MAX_TOOL_ARGUMENT_BYTES {
                            return Err(subagent_limit_error());
                        }
                    }
                    _ => {}
                }
            }
            Some("message_delta") => {
                if let Some(usage) = event.get("usage") {
                    merge_messages_usage(&mut state.messages_usage, usage);
                }
            }
            _ => {}
        },
    }
    Ok(())
}

async fn append_child_stream_text(
    state: &mut ChildStreamState,
    delta: &str,
    events: &EventSink,
    storage: &AppStorage,
    conversation_id: &str,
    child_run_id: &str,
) -> Result<(), ServiceError> {
    if delta.is_empty() {
        return Ok(());
    }
    if state.text.len().saturating_add(delta.len()) > CHILD_MAX_OUTPUT_BYTES {
        return Err(subagent_limit_error());
    }
    state.text.push_str(delta);
    if state.full_text.len().saturating_add(delta.len()) > CHILD_MAX_TOTAL_TEXT_BYTES {
        return Err(subagent_limit_error());
    }
    state.full_text.push_str(delta);
    send_subagent_delta(
        events,
        storage,
        conversation_id,
        child_run_id,
        delta,
        &state.full_text,
    )
    .await
}

fn merge_messages_usage(target: &mut Value, source: &Value) {
    const USAGE_FIELDS: &[&str] = &[
        "input_tokens",
        "output_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ];
    for field in USAGE_FIELDS {
        if let Some(value) = source
            .get(*field)
            .and_then(Value::as_i64)
            .filter(|value| *value >= 0)
        {
            target[*field] = json!(value);
        }
    }
}

fn finish_child_stream(
    state: ChildStreamState,
    protocol: ChildRequestProtocol,
) -> Result<Value, ServiceError> {
    match protocol {
        ChildRequestProtocol::Responses => state.response.ok_or_else(subagent_response_error),
        ChildRequestProtocol::ChatCompletions => {
            let tool_calls = state.chat_tool_calls.into_values().collect::<Vec<_>>();
            let mut value = json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": if state.text.is_empty() { Value::Null } else { json!(state.text) },
                        "tool_calls": tool_calls,
                    }
                }]
            });
            if let Some(usage) = state.chat_usage {
                value["usage"] = usage;
            }
            Ok(value)
        }
        ChildRequestProtocol::Messages => {
            let mut content = Vec::new();
            if !state.text.is_empty() {
                content.push(json!({"type":"text","text":state.text}));
            }
            for (_, tool) in state.messages_tools {
                let input = if tool.partial_json.is_empty() {
                    tool.initial_input.unwrap_or_else(|| json!({}))
                } else {
                    serde_json::from_str::<Value>(&tool.partial_json)
                        .map_err(|_| subagent_response_error())?
                };
                if !input.is_object() || tool.id.is_empty() || tool.name.is_empty() {
                    return Err(subagent_response_error());
                }
                content.push(json!({
                    "type":"tool_use",
                    "id":tool.id,
                    "name":tool.name,
                    "input":input,
                }));
            }
            Ok(json!({"content":content,"usage":state.messages_usage}))
        }
    }
}
