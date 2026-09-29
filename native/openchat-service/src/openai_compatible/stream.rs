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
