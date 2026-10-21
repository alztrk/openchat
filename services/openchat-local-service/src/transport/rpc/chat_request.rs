//! Parses and validates renderer parameters for durable chat execution.
//!
//! Credential resolution runs only after the supplied chat options pass their
//! boundary checks. The returned strings borrow the decoded request payload;
//! owned collections are capped before their elements are copied.

use std::{collections::HashSet, time::SystemTime, time::UNIX_EPOCH};

use serde_json::Value;

use crate::{
    chat_operation::{TitleGenerationModel, TitleGenerationRequest},
    goals, instructions,
    protocol::ServiceError,
    provider_credentials::{self, ProviderCredentials},
    tools,
};

use super::{invalid_request_params, optional_bool, optional_string, required_string};

/// Validated request settings consumed by durable chat execution.
pub(super) struct ChatSendParameters<'a> {
    /// Durable conversation whose saved provider route will handle the request.
    pub(super) conversation_id: &'a str,
    /// Existing paused run to resume, if the request is a continuation.
    pub(super) resume_run_id: Option<&'a str>,
    /// Renderer-selected saved API-key connection, when provided.
    pub(super) api_key_connection_id: Option<&'a str>,
    /// Provider-specific reasoning setting requested for this send.
    pub(super) requested_reasoning_effort: Option<&'a str>,
    /// Whether the ChatGPT Fast mode route was requested.
    pub(super) requested_fast_mode: bool,
    /// Optional bounded context-window override.
    pub(super) context_limit_override: Option<i64>,
    /// Validated goal objective, when this request starts a persistent goal.
    pub(super) requested_goal_objective: Option<&'a str>,
    /// Shared instructions supplied for a new run; resumes use their checkpoint.
    pub(super) requested_custom_instructions: Option<&'a str>,
    /// Validated project Skill IDs selected for the request.
    pub(super) requested_project_skill_ids: Vec<String>,
    /// Renderer-requested plan mode, which can only restrict saved permissions.
    pub(super) plan_mode_requested: bool,
    /// Optional restriction on tools available to the selected profile.
    pub(super) profile_tool_allowlist: Option<HashSet<String>>,
    /// Assistant response excluded while regenerating a branch.
    pub(super) excluded_assistant_message_id: Option<&'a str>,
    /// Resolved title-generation route, when title generation is enabled.
    pub(super) title_generation: TitleGenerationRequest,
}

/// Validates chat input before credential reads or persistent execution work.
///
/// # Errors
/// Returns a stable RPC error when any renderer-supplied option is malformed,
/// exceeds its limit, or violates a chat-mode constraint.
pub(super) fn parse_send_request(
    params: &mut Value,
) -> Result<ChatSendParameters<'_>, ServiceError> {
    let _ = super::take_optional_api_key(params, "apiKey")?;
    let title_generation_request = parse_title_generation(params)?;
    let conversation_id = required_string(params, "conversationId")?;
    if crate::temporary_conversation::is_temporary_conversation_id(conversation_id) {
        return Err(invalid_request_params());
    }

    let resume_run_id = optional_string(params, "resumeRunId")?;
    let api_key_connection_id = optional_string(params, "apiKeyConnectionId")?;
    let requested_reasoning_effort = optional_string(params, "reasoningEffort")?;
    let requested_fast_mode = optional_bool(params, "fastMode")?.unwrap_or(false);
    let context_limit_override = match params.get("contextLimit") {
        None | Some(Value::Null) => None,
        Some(Value::Number(value)) => value
            .as_i64()
            .filter(|limit| (2048..=2_000_000).contains(limit))
            .ok_or_else(invalid_request_params)
            .map(Some)?,
        _ => return Err(invalid_request_params()),
    };
    let goal_requested = optional_bool(params, "goal")?.unwrap_or(false);
    let requested_goal_objective = if goal_requested {
        Some(goals::validate_objective(
            optional_string(params, "goalObjective")?
                .ok_or_else(|| invalid_chat_goal_request("goal objective"))?,
        )?)
    } else {
        if optional_string(params, "goalObjective")?.is_some() {
            return Err(invalid_chat_goal_request("goal objective"));
        }
        None
    };
    let requested_custom_instructions = optional_string(params, "customInstructions")?;
    if resume_run_id.is_none() {
        instructions::validate_custom_instructions(requested_custom_instructions)
            .map_err(|message| ServiceError::new("invalid_request_params", message, false))?;
    }
    let requested_project_skill_ids = parse_project_skill_ids(params)?;
    crate::project_skills::validate_selected_ids(&requested_project_skill_ids)?;
    if params.get("toolPermissionMode").is_some() || params.get("toolPermissionRules").is_some() {
        return Err(invalid_request_params());
    }
    let plan_mode_requested = optional_bool(params, "planMode")?.unwrap_or(false);
    let profile_tool_allowlist =
        tools::parse_profile_tool_allowlist(params.get("profileToolAllowlist"))?;
    let excluded_assistant_message_id = optional_string(params, "excludedAssistantMessageId")?;
    if goal_requested && (resume_run_id.is_some() || excluded_assistant_message_id.is_some()) {
        return Err(invalid_chat_goal_request("goal start parameters"));
    }
    let title_generation = resolve_title_generation_credentials(title_generation_request);

    Ok(ChatSendParameters {
        conversation_id,
        resume_run_id,
        api_key_connection_id,
        requested_reasoning_effort,
        requested_fast_mode,
        context_limit_override,
        requested_goal_objective,
        requested_custom_instructions,
        requested_project_skill_ids,
        plan_mode_requested,
        profile_tool_allowlist,
        excluded_assistant_message_id,
        title_generation,
    })
}

/// Parses a bounded project Skill selection without cloning rejected IDs.
///
/// # Errors
/// Returns the existing RPC error for malformed values or oversized selections.
pub(super) fn parse_project_skill_ids(params: &Value) -> Result<Vec<String>, ServiceError> {
    let Some(value) = params.get("projectSkillIds") else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err(invalid_request_params());
    };
    crate::project_skills::validate_selected_id_count(values.len())?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(invalid_request_params)
        })
        .collect()
}

/// Parses a title-generation route and removes any renderer-provided API key.
///
/// # Errors
/// Returns an RPC validation error when the mode or route fields are invalid.
pub(super) fn parse_title_generation(
    params: &mut Value,
) -> Result<TitleGenerationRequest, ServiceError> {
    let Some(settings) = params.get_mut("titleGeneration") else {
        return Ok(TitleGenerationRequest::Conversation);
    };
    let mode = required_string(settings, "mode")?;
    match mode {
        "conversation" => Ok(TitleGenerationRequest::Conversation),
        "disabled" => Ok(TitleGenerationRequest::Disabled),
        "selectedModel" | "lightweight" => {
            let target = settings
                .get_mut("target")
                .ok_or_else(invalid_request_params)?;
            let _ = super::take_optional_api_key(target, "apiKey")?;
            let provider_id = required_string(target, "providerId")?;
            let model_id = required_string(target, "modelId")?;
            if provider_id.is_empty()
                || provider_id.len() > 128
                || provider_id.chars().any(char::is_control)
                || model_id.is_empty()
                || model_id.len() > 512
                || model_id.chars().any(char::is_control)
            {
                return Err(invalid_request_params());
            }
            let optional_route_id = |name| -> Result<Option<String>, ServiceError> {
                optional_string(target, name)?
                    .map(|value| {
                        if value.is_empty()
                            || value.len() > 256
                            || value.chars().any(char::is_control)
                        {
                            Err(invalid_request_params())
                        } else {
                            Ok(value.to_owned())
                        }
                    })
                    .transpose()
            };
            let connection_id = optional_route_id("connectionId")?;
            let workspace_id = optional_route_id("workspaceId")?;
            let api_key_connection_id = optional_route_id("apiKeyConnectionId")?;
            if provider_id.starts_with("custom_")
                && api_key_connection_id.as_deref() != Some(provider_id)
            {
                return Err(invalid_request_params());
            }
            Ok(TitleGenerationRequest::Model(TitleGenerationModel {
                provider_id: provider_id.to_owned(),
                model_id: model_id.to_owned(),
                connection_id,
                workspace_id,
                api_key_connection_id,
                api_key: None,
                custom_provider: None,
            }))
        }
        _ => Err(invalid_request_params()),
    }
}

/// Resolves optional title-generation credentials after request validation.
///
/// Missing saved credentials disable optional title generation, as before.
fn resolve_title_generation_credentials(request: TitleGenerationRequest) -> TitleGenerationRequest {
    let TitleGenerationRequest::Model(mut model) = request else {
        return request;
    };
    if !provider_credentials::uses_saved_api_key(&model.provider_id) {
        return TitleGenerationRequest::Model(model);
    }
    let Ok(credentials) = ProviderCredentials::load() else {
        return TitleGenerationRequest::Disabled;
    };
    let connection_id = (model.provider_id != "opencode")
        .then_some(model.api_key_connection_id.as_deref())
        .flatten();
    let Ok(api_key) = credentials.api_key(&model.provider_id, connection_id) else {
        return TitleGenerationRequest::Disabled;
    };
    if provider_credentials::requires_saved_api_key(&model.provider_id) && api_key.is_none() {
        return TitleGenerationRequest::Disabled;
    }
    if model.provider_id.starts_with("custom_") {
        let Ok(config) = credentials.custom_provider_config(&model.provider_id) else {
            return TitleGenerationRequest::Disabled;
        };
        model.custom_provider = Some(config);
    }
    model.api_key = api_key;
    TitleGenerationRequest::Model(model)
}

/// Creates the stable validation error for an invalid goal-specific field.
pub(super) fn invalid_chat_goal_request(field: &str) -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        format!("The {field} is invalid."),
        false,
    )
}

/// Returns the current Unix time in milliseconds for durable goal checkpoints.
///
/// # Errors
/// Returns `clock_unavailable` if the system clock precedes the Unix epoch or
/// its millisecond value cannot fit in a signed 64-bit integer.
pub(super) fn current_time_unix_ms() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        ServiceError::new("clock_unavailable", "System time is unavailable.", false)
    })?;
    i64::try_from(elapsed.as_millis())
        .map_err(|_| ServiceError::new("clock_unavailable", "System time is out of range.", false))
}

/// Identifies provider failures that pause a persistent goal for later retry.
pub(super) fn is_quota_or_rate_limit_error(error: &ServiceError) -> bool {
    matches!(
        error.code,
        "rate_limited" | "quota_exceeded" | "insufficient_quota" | "resource_exhausted"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_send_request;

    #[test]
    fn durable_chat_rejects_renderer_supplied_permission_policy() {
        for field in [
            json!({"toolPermissionMode": "full_access"}),
            json!({"toolPermissionRules": {"read_file": "allow"}}),
        ] {
            let mut params = json!({"conversationId": "conversation-1"});
            params
                .as_object_mut()
                .expect("request object")
                .extend(field.as_object().expect("permission field object").clone());

            let error = parse_send_request(&mut params)
                .err()
                .expect("renderer policy must not configure a durable run");
            assert_eq!(error.code, "invalid_request_params");
        }
    }

    #[test]
    fn renderer_can_request_plan_mode_as_a_restriction() {
        let mut params = json!({"conversationId": "conversation-1", "planMode": true});
        let request = parse_send_request(&mut params).expect("valid plan-mode request");

        assert!(request.plan_mode_requested);
    }
}
