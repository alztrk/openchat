use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::{
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
    provider_schema::{ChatStreamSnapshot, ToolCall},
    storage::AppStorage,
    tools::{ImageGenerationContext, ToolExecutor},
    user_question_broker::UserQuestionBroker,
};

use super::super::invalid_response_error;

pub(super) struct ToolRoundContext<'a> {
    pub(super) request_id: &'a Value,
    pub(super) conversation_id: &'a str,
    pub(super) message_id: &'a str,
    pub(super) created_at: i64,
    pub(super) content: &'a str,
    pub(super) storage: &'a AppStorage,
    pub(super) run_id: &'a str,
    pub(super) provider_id: &'a str,
    pub(super) image_generation: Option<ImageGenerationContext<'a>>,
    pub(super) permission_broker: &'a ToolPermissionBroker,
    pub(super) user_question_broker: &'a UserQuestionBroker,
    pub(super) cancellation: &'a mut tokio::sync::watch::Receiver<bool>,
    pub(super) events: &'a EventSink,
}

pub(super) async fn execute(
    calls: &[ToolCall],
    executable_calls: &[ToolCall],
    precomputed_results: Vec<crate::provider_schema::ToolResult>,
    round_content: &str,
    is_opencode: bool,
    messages: &mut Vec<Value>,
    tool_executor: &mut ToolExecutor,
    context: ToolRoundContext<'_>,
) -> Result<(), ServiceError> {
    tool_executor.begin_round(executable_calls)?;
    let mut results_by_id = HashMap::with_capacity(calls.len());
    for result in precomputed_results {
        if results_by_id
            .insert(result.call_id, result.output)
            .is_some()
        {
            return Err(invalid_response_error());
        }
    }
    for call in executable_calls {
        let snapshot = ChatStreamSnapshot::new(
            context.conversation_id,
            context.message_id,
            context.content,
            context.created_at,
        );
        let result = tool_executor
            .execute_call_with_image_context(
                call,
                context.permission_broker,
                context.request_id,
                &snapshot,
                context.events,
                context.cancellation,
                context.storage,
                context.run_id,
                context.provider_id,
                context.user_question_broker,
                context.image_generation.as_ref(),
            )
            .await?;
        if results_by_id
            .insert(result.call_id, result.output)
            .is_some()
        {
            return Err(invalid_response_error());
        }
    }

    let assistant_calls = calls
        .iter()
        .map(|call| {
            let wire_name = if is_opencode {
                crate::tools::opencode_wire_name(&call.name)
            } else {
                call.name.as_str()
            };
            let arguments =
                serde_json::to_string(&call.arguments).map_err(|_| invalid_response_error())?;
            Ok(json!({
                "id": call.id,
                "type": "function",
                "function": {"name": wire_name, "arguments": arguments}
            }))
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;

    messages.push(json!({
        "role": "assistant",
        "content": if round_content.is_empty() { Value::Null } else { json!(round_content) },
        "tool_calls": assistant_calls
    }));
    for call in calls {
        let output = results_by_id
            .remove(&call.id)
            .ok_or_else(invalid_response_error)?;
        let output = if context.provider_id == "mistral" {
            add_mistral_reference_map(output)
        } else {
            output
        };
        let output = serde_json::to_string(&output).map_err(|_| invalid_response_error())?;
        messages.push(json!({
            "role": "tool",
            "tool_call_id": call.id,
            "content": output
        }));
    }
    Ok(())
}

fn add_mistral_reference_map(output: Value) -> Value {
    let Value::Object(mut object) = output else {
        return output;
    };
    let mut reference_map = Map::new();
    if let Some(results) = object.get("results").and_then(Value::as_array) {
        for result in results {
            if !has_reference_metadata(result) {
                continue;
            }
            let index = reference_map.len().to_string();
            reference_map.insert(index, mistral_reference_metadata(result));
        }
    } else if has_reference_metadata(&Value::Object(object.clone())) {
        reference_map.insert(
            "0".to_owned(),
            mistral_reference_metadata(&Value::Object(object.clone())),
        );
    }
    if !reference_map.is_empty() {
        object.extend(reference_map);
    }
    Value::Object(object)
}

fn has_reference_metadata(value: &Value) -> bool {
    value.get("sourceId").and_then(Value::as_str).is_some()
        && value.get("title").and_then(Value::as_str).is_some()
        && value.get("url").and_then(Value::as_str).is_some()
}

fn mistral_reference_metadata(value: &Value) -> Value {
    json!({
        "sourceId": value.get("sourceId"),
        "title": value.get("title"),
        "url": value.get("url"),
        "snippets": value.get("snippet").into_iter().collect::<Vec<_>>(),
        "content": value.get("content"),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::add_mistral_reference_map;

    #[test]
    fn adds_indexed_mistral_references_while_preserving_tool_results() {
        let output = json!({
            "sourceType": "local_web_search",
            "results": [
                {
                    "sourceId": "S1-call1234",
                    "title": "First source",
                    "url": "https://example.org/first",
                    "snippet": "First excerpt"
                },
                {
                    "sourceId": "S2-call1234",
                    "title": "Second source",
                    "url": "https://example.org/second",
                    "snippet": "Second excerpt"
                }
            ]
        });

        let mapped = add_mistral_reference_map(output);

        assert_eq!(mapped["0"]["sourceId"], "S1-call1234");
        assert_eq!(mapped["1"]["url"], "https://example.org/second");
        assert_eq!(mapped["results"][0]["snippet"], "First excerpt");
    }

    #[test]
    fn leaves_tool_results_without_valid_source_metadata_unchanged() {
        let output = json!({"results": [{"title": "No URL"}]});
        assert_eq!(add_mistral_reference_map(output.clone()), output);
    }
}
