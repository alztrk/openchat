use super::{invalid_response_error, now_unix_millis};
use crate::{
    chatgpt_store::{ChatGptModel, NewUsageSnapshot, ResetCredit, UsageBucket},
    protocol::ServiceError,
    provider_schema::{ToolCall, ToolDefinition},
    tools,
};
use reqwest::StatusCode;
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

pub(super) fn parse_models(value: &Value) -> Result<Vec<ChatGptModel>, ServiceError> {
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(invalid_response_error)?;
    let mut parsed = Vec::with_capacity(models.len());
    for model in models {
        let id = model
            .get("slug")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(invalid_response_error)?
            .to_owned();
        let display_name = model
            .get("display_name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(&id)
            .to_owned();
        let reasoning_levels: Vec<String> = model
            .get("supported_reasoning_levels")
            .and_then(Value::as_array)
            .map(|levels| {
                levels
                    .iter()
                    .filter_map(|level| level.get("effort").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let context_window = model.get("context_window").and_then(Value::as_i64);
        let default_reasoning_level = model
            .get("default_reasoning_level")
            .and_then(Value::as_str)
            .filter(|level| reasoning_levels.iter().any(|supported| supported == *level))
            .map(str::to_owned);
        let supports_reasoning_summary_parameter = model
            .get("supports_reasoning_summary_parameter")
            .map(|supported| supported.as_bool().ok_or_else(invalid_response_error))
            .transpose()?
            .unwrap_or(true);
        parsed.push(ChatGptModel {
            id,
            display_name,
            description: model
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
            context_window,
            default_reasoning_level,
            reasoning_levels,
            supports_reasoning_summary_parameter,
            is_available: model.get("visibility").and_then(Value::as_str) == Some("list")
                && model.get("supported_in_api").and_then(Value::as_bool) == Some(true),
        });
    }
    Ok(parsed)
}

pub(super) fn parse_usage(
    value: &Value,
    connection_id: &str,
    workspace_id: &str,
    reset_credits: Vec<ResetCredit>,
    credit_count: Option<i64>,
    reset_credit_details_state: &str,
) -> Result<NewUsageSnapshot, ServiceError> {
    let rate_limit = value.get("rate_limit");
    let ordinary_usage_allowed = rate_limit
        .and_then(|limit| limit.get("allowed"))
        .and_then(Value::as_bool);
    let mut buckets = Vec::new();
    if let Some(limit) = rate_limit {
        add_rate_limit_buckets(&mut buckets, "codex", limit)?;
    }
    if let Some(additional_limits) = value
        .get("additional_rate_limits")
        .and_then(Value::as_array)
    {
        for limit in additional_limits {
            let id = limit
                .get("limit_name")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(invalid_response_error)?;
            if let Some(rate_limit) = limit.get("rate_limit") {
                add_rate_limit_buckets(&mut buckets, id, rate_limit)?;
            }
        }
    }
    let usage_credit_count = value
        .get("rate_limit_reset_credits")
        .and_then(|credits| credits.get("available_count"))
        .and_then(Value::as_i64);
    Ok(NewUsageSnapshot {
        id: Uuid::new_v4().simple().to_string(),
        connection_id: connection_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        fetched_at_unix_ms: now_unix_millis()?,
        freshness: "current".to_owned(),
        ordinary_usage_allowed,
        reset_credit_count: usage_credit_count.or(credit_count),
        reset_credit_details_state: reset_credit_details_state.to_owned(),
        buckets,
        reset_credits,
    })
}

fn add_rate_limit_buckets(
    buckets: &mut Vec<UsageBucket>,
    limit_id: &str,
    rate_limit: &Value,
) -> Result<(), ServiceError> {
    for (window_name, suffix) in [
        ("primary_window", "primary"),
        ("secondary_window", "secondary"),
    ] {
        let Some(window) = rate_limit.get(window_name).filter(|value| !value.is_null()) else {
            continue;
        };
        let used_percent = window.get("used_percent").and_then(Value::as_f64);
        if used_percent.is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value)) {
            return Err(invalid_response_error());
        }
        let reset_at_unix_ms = window
            .get("reset_at")
            .and_then(Value::as_i64)
            .and_then(|seconds| seconds.checked_mul(1000));
        buckets.push(UsageBucket {
            limit_id: format!("{limit_id}:{suffix}"),
            used_percent,
            window_seconds: window.get("limit_window_seconds").and_then(Value::as_i64),
            reset_at_unix_ms,
        });
    }
    Ok(())
}

pub(super) fn parse_reset_credits(
    value: &Value,
) -> Result<(Vec<ResetCredit>, Option<i64>, &'static str), ServiceError> {
    let available_count = value.get("available_count").and_then(Value::as_i64);
    let Some(credits_value) = value.get("credits") else {
        return Ok((Vec::new(), available_count, "omitted"));
    };
    if credits_value.is_null() {
        return Ok((Vec::new(), available_count, "omitted"));
    }
    let credits = credits_value
        .as_array()
        .ok_or_else(invalid_response_error)?;
    let mut parsed = Vec::with_capacity(credits.len());
    for credit in credits {
        let id = credit
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(invalid_response_error)?
            .to_owned();
        parsed.push(ResetCredit {
            id,
            reset_type: credit
                .get("reset_type")
                .and_then(Value::as_str)
                .map(str::to_owned),
            status: credit
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_owned),
            granted_at_unix_ms: parse_timestamp(credit.get("granted_at")),
            expires_at_unix_ms: parse_timestamp(credit.get("expires_at")),
            title: credit
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_owned),
            description: credit
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
        });
    }
    Ok((parsed, available_count, "available"))
}

fn parse_timestamp(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(seconds) = value.as_i64() {
        return seconds.checked_mul(1000);
    }
    let timestamp = value.as_str()?;
    let parsed = OffsetDateTime::parse(timestamp, &Rfc3339).ok()?;
    i64::try_from(parsed.unix_timestamp_nanos() / 1_000_000).ok()
}

pub(super) fn ensure_success(status: StatusCode) -> Result<(), ServiceError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(http_error(status))
    }
}

pub(super) fn http_error(status: StatusCode) -> ServiceError {
    match status {
        StatusCode::UNAUTHORIZED => ServiceError::new(
            "authentication_required",
            "ChatGPT needs you to sign in to this account again.",
            false,
        ),
        StatusCode::FORBIDDEN => ServiceError::new(
            "permission_denied",
            "This ChatGPT account or workspace does not allow the requested operation.",
            false,
        ),
        StatusCode::TOO_MANY_REQUESTS => ServiceError::new(
            "rate_limited",
            "ChatGPT has reached a usage limit for this account. Wait for its reset or choose another account.",
            false,
        ),
        StatusCode::NOT_FOUND => ServiceError::new(
            "provider_endpoint_unavailable",
            "The ChatGPT endpoint is unavailable or has changed.",
            false,
        ),
        status if status.is_server_error() => ServiceError::new(
            "provider_unavailable",
            "ChatGPT is temporarily unavailable. Try again later.",
            true,
        ),
        _ => ServiceError::new(
            "provider_request_failed",
            "ChatGPT rejected the request. Check the account and model selection.",
            false,
        ),
    }
}

pub(super) fn parse_responses_tool_calls(
    output_items: &[Value],
) -> Result<Vec<ToolCall>, ServiceError> {
    output_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .map(|item| {
            let id = item
                .get("call_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(invalid_response_error)?;
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(invalid_response_error)?;
            let arguments = item
                .get("arguments")
                .and_then(Value::as_str)
                .ok_or_else(invalid_response_error)?;
            if arguments.len() > tools::MAX_TOOL_ARGUMENT_BYTES {
                return Err(invalid_response_error());
            }
            let arguments =
                serde_json::from_str(arguments).map_err(|_| invalid_response_error())?;
            Ok(ToolCall {
                id: id.to_owned(),
                name: name.to_owned(),
                arguments,
            })
        })
        .collect()
}

pub(super) fn response_output_text(output_items: &[Value]) -> String {
    let mut text = String::new();
    for item in output_items {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(content) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in content {
            match part.get("type").and_then(Value::as_str) {
                Some("output_text") => {
                    if let Some(value) = part.get("text").and_then(Value::as_str) {
                        text.push_str(value);
                    }
                }
                Some("refusal") => {
                    if let Some(value) = part.get("refusal").and_then(Value::as_str) {
                        text.push_str(value);
                    }
                }
                _ => {}
            }
        }
    }
    text
}

pub(super) fn empty_response_shape(output_items: &[Value]) -> &'static str {
    if output_items.is_empty() {
        return "no_output_items";
    }
    if output_items
        .iter()
        .all(|item| item.get("type").and_then(Value::as_str) == Some("reasoning"))
    {
        return "reasoning_only";
    }
    if output_items
        .iter()
        .any(|item| item.get("type").and_then(Value::as_str) == Some("message"))
    {
        return "message_without_visible_text";
    }
    "other_output_items"
}

pub(super) fn responses_tool(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "strict": false,
        "parameters": tool.parameters.clone(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_responses_tool_calls;

    #[test]
    fn parses_completed_responses_function_calls() {
        let items = [
            json!({"type": "reasoning", "id": "rs_1"}),
            json!({
                "type": "function_call",
                "call_id": "call_1",
                "name": "list_files",
                "arguments": "{\"path\":\".\"}"
            }),
        ];

        let calls = parse_responses_tool_calls(&items).expect("valid Responses function call");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "list_files");
        assert_eq!(calls[0].arguments, json!({"path": "."}));
    }

    #[test]
    fn rejects_completed_responses_function_calls_with_invalid_arguments() {
        let items = [json!({
            "type": "function_call",
            "call_id": "call_1",
            "name": "list_files",
            "arguments": "{\"path\":"
        })];

        assert_eq!(
            parse_responses_tool_calls(&items)
                .expect_err("invalid tool arguments must be rejected")
                .code,
            "invalid_provider_response"
        );
    }
}
