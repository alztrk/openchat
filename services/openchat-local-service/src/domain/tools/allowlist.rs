//! Profile tool allowlist parsing and application.

use super::*;

pub(crate) fn parse_profile_tool_allowlist(
    value: Option<&Value>,
) -> Result<Option<HashSet<String>>, ServiceError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        return Err(invalid_profile_tool_allowlist());
    };
    if values.len() > MAX_PROFILE_TOOL_ALLOWLIST {
        return Err(invalid_profile_tool_allowlist());
    }
    let mut supported = definitions()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<HashSet<_>>();
    supported.extend([
        "run_project_task".to_owned(),
        "delegate_task".to_owned(),
        "list_subagent_models".to_owned(),
        "get_subagent_result".to_owned(),
        "cancel_subagent".to_owned(),
        crate::goals::START_GOAL_TOOL_NAME.to_owned(),
        crate::goals::CONTROL_TOOL_NAME.to_owned(),
        crate::goals::STOP_GOAL_TOOL_NAME.to_owned(),
    ]);
    let mut allowed = HashSet::with_capacity(values.len());
    for value in values {
        let Some(name) = value.as_str() else {
            return Err(invalid_profile_tool_allowlist());
        };
        let is_project_tool = name
            .strip_prefix("project_tool__")
            .is_some_and(crate::tools::project_tools::is_valid_id);
        let is_mcp_tool = valid_profile_mcp_tool_name(name);
        if (!supported.contains(name) && !is_project_tool && !is_mcp_tool)
            || !allowed.insert(name.to_owned())
        {
            return Err(invalid_profile_tool_allowlist());
        }
    }
    Ok(Some(allowed))
}

pub(super) fn valid_profile_mcp_tool_name(name: &str) -> bool {
    if name.len() > 64 {
        return false;
    }
    let Some((server_id, tool_name)) = name
        .strip_prefix("mcp__")
        .and_then(|suffix| suffix.split_once("__"))
    else {
        return false;
    };
    !server_id.is_empty()
        && server_id.len() <= 24
        && server_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !tool_name.is_empty()
        && tool_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(super) fn invalid_profile_tool_allowlist() -> ServiceError {
    ServiceError::new(
        "invalid_profile_tool_allowlist",
        "The assistant profile tool allowlist is invalid.",
        false,
    )
}

pub(crate) fn apply_profile_tool_allowlist(
    definitions: &mut Vec<ToolDefinition>,
    allowed_names: Option<&HashSet<String>>,
) {
    if let Some(allowed_names) = allowed_names {
        definitions.retain(|tool| allowed_names.contains(&tool.name));
    }
}
