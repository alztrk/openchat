//! Child run text persistence, tool definitions, usage, and error helpers.

use super::*;

pub(super) fn load_child_text(
    storage: &AppStorage,
    conversation_id: &str,
    run_id: &str,
) -> Result<String, ServiceError> {
    let connection = storage.connect().map_err(|_| subagent_storage_error())?;
    let run = user_questions::load_run_for_conversation(&connection, conversation_id, run_id)
        .map_err(|_| subagent_storage_error())?;
    Ok(run
        .checkpoint
        .and_then(|checkpoint| {
            checkpoint
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default())
}

pub(super) fn persist_child_text(
    storage: &AppStorage,
    conversation_id: &str,
    run_id: &str,
    text: &str,
) -> Result<(), ServiceError> {
    let mut connection = storage.connect().map_err(|_| subagent_storage_error())?;
    let run = user_questions::load_run_for_conversation(&connection, conversation_id, run_id)
        .map_err(|_| subagent_storage_error())?;
    let mut checkpoint = run.checkpoint.unwrap_or_else(|| json!({}));
    if checkpoint.get("text").and_then(Value::as_str) == Some(text) {
        return Ok(());
    }
    checkpoint["text"] = json!(text);
    checkpoint["messages"] = json!([{"role":"assistant","content":text}]);
    user_questions::save_checkpoint(
        &mut connection,
        conversation_id,
        run_id,
        run.checkpoint_revision,
        &checkpoint,
    )
    .map_err(|_| subagent_storage_error())?;
    Ok(())
}

#[cfg(test)]
pub(super) fn child_tool_definitions(
    include_project_tools: bool,
    project_task_ids: &[String],
) -> Vec<ToolDefinition> {
    let mut available_tools = tools::definitions_for_chatgpt_model();
    if cfg!(windows) && !project_task_ids.is_empty() {
        available_tools.push(tools::project_tasks::tool_definition(project_task_ids));
    }
    available_tools
        .into_iter()
        .filter(|tool| {
            CHILD_WEB_TOOL_NAMES.contains(&tool.name.as_str())
                || (include_project_tools
                    && (CHILD_PROJECT_TOOL_NAMES.contains(&tool.name.as_str())
                        || CHILD_PLATFORM_TOOL_NAMES.contains(&tool.name.as_str())))
        })
        .collect()
}

pub(super) fn add_usage(total: &mut UsageData, round: &UsageData) {
    total.input_tokens = sum_optional(total.input_tokens, round.input_tokens);
    total.output_tokens = sum_optional(total.output_tokens, round.output_tokens);
    total.reasoning_tokens = sum_optional(total.reasoning_tokens, round.reasoning_tokens);
    total.cached_input_tokens = sum_optional(total.cached_input_tokens, round.cached_input_tokens);
    total.cache_write_tokens = sum_optional(total.cache_write_tokens, round.cache_write_tokens);
    total.reported_cost_usd = match (total.reported_cost_usd, round.reported_cost_usd) {
        (Some(current), Some(next)) => Some(current + next),
        (Some(current), None) => Some(current),
        (None, Some(next)) => Some(next),
        (None, None) => None,
    };
}

pub(super) fn sum_optional(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

pub(super) fn usage_value(usage: &UsageData) -> Value {
    json!({
        "inputTokens": usage.input_tokens,
        "outputTokens": usage.output_tokens,
        "reasoningTokens": usage.reasoning_tokens,
        "cachedInputTokens": usage.cached_input_tokens,
        "cacheWriteTokens": usage.cache_write_tokens,
        "reportedCostUsd": usage.reported_cost_usd,
    })
}

pub(super) fn truncate_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[Child output truncated.]", &value[..end])
}

pub(super) fn tool_error(code: &str, message: &str) -> Value {
    json!({"error": {"code": code, "message": message}})
}

pub(super) fn subagent_timeout() -> ServiceError {
    ServiceError::new(
        "subagent_timeout",
        "The child analysis reached its time limit.",
        false,
    )
}

pub(super) fn subagent_limit_error() -> ServiceError {
    ServiceError::new(
        "subagent_limit_reached",
        "The child analysis reached its request or tool limit.",
        false,
    )
}

pub(super) fn subagent_response_error() -> ServiceError {
    ServiceError::new(
        "invalid_provider_response",
        "The child analysis returned an incomplete response.",
        false,
    )
}

pub(super) fn subagent_storage_error() -> ServiceError {
    ServiceError::new(
        "subagent_storage_unavailable",
        "The child run could not be saved locally.",
        false,
    )
}
