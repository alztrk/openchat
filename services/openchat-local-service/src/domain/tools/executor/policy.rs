//! Global and project-level permission policies, rule evaluation, and MCP tool rule parsing.

use std::collections::HashMap;

use serde_json::Value;

use crate::protocol::ServiceError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Global policy applied before a model-requested tool is allowed to execute.
pub enum ToolPermissionMode {
    /// Denies tool calls so the model can produce a plan without taking action.
    Plan,
    /// Requires explicit approval for every operation.
    RequireApproval,
    /// Automatically approves only operations classified as safe by the executor.
    ApproveSafeOperations,
    /// Executes allowed operations without an additional per-call approval request.
    FullAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Per-tool project policy that asks, allows, or denies an operation.
pub enum ToolPermissionRule {
    /// Ask the user before running the operation.
    Ask,
    /// Allow the operation within the active permission mode and path scope.
    Allow,
    /// Deny the operation before it touches a resource.
    Deny,
}

pub(crate) type ToolPermissionRules = HashMap<String, ToolPermissionRule>;
pub(crate) const MAX_TOOL_PERMISSION_RULES: usize = 1080;
pub(crate) const MAX_PROJECT_RULE_PRECHECKS: usize = 256;

pub(crate) fn project_task_permission_rule<'a>(
    rules: &'a ToolPermissionRules,
    task_id: &str,
) -> Option<&'a ToolPermissionRule> {
    crate::domain::tools::project_tasks::permission_rule_name(task_id)
        .and_then(|name| rules.get(&name))
        .or_else(|| rules.get("run_project_task"))
}

#[cfg(windows)]
pub(crate) fn mcp_tool_permission_rule(
    rules: &ToolPermissionRules,
    server_id: &str,
    server_permission_fingerprint: &str,
    tool_name: &str,
    definition_fingerprint: &str,
) -> ToolPermissionRule {
    let server_rule = crate::domain::tools::mcp::server_permission_rule(
        rules,
        server_id,
        server_permission_fingerprint,
    );
    if server_rule == ToolPermissionRule::Deny {
        return ToolPermissionRule::Deny;
    }
    if rules.get(&format!("mcp__{server_id}__{tool_name}")) == Some(&ToolPermissionRule::Deny) {
        return ToolPermissionRule::Deny;
    }
    if let Some(rule) = rules.get(&crate::domain::tools::mcp::tool_permission_rule_name(
        server_id,
        tool_name,
        definition_fingerprint,
    )) {
        return *rule;
    }
    ToolPermissionRule::Ask
}

pub(crate) fn parse_tool_permission_rules(
    value: Option<&Value>,
) -> Result<ToolPermissionRules, ServiceError> {
    let Some(value) = value else {
        return Ok(ToolPermissionRules::new());
    };
    let Some(rules) = value.as_object() else {
        return Err(invalid_tool_permission_rules());
    };
    if rules.len() > MAX_TOOL_PERMISSION_RULES {
        return Err(invalid_tool_permission_rules());
    }

    let mut parsed = ToolPermissionRules::with_capacity(rules.len());
    for (name, rule) in rules {
        let rule = match rule.as_str() {
            Some("ask") => ToolPermissionRule::Ask,
            Some("allow") => ToolPermissionRule::Allow,
            Some("deny") => ToolPermissionRule::Deny,
            _ => return Err(invalid_tool_permission_rules()),
        };
        if matches!(name.as_str(), "git_status" | "git_diff" | "git_history") {
            continue;
        }
        if !is_permission_rule_tool(name) {
            return Err(invalid_tool_permission_rules());
        }
        parsed.insert(name.clone(), rule);
    }
    Ok(parsed)
}

pub(crate) fn is_permission_rule_tool(name: &str) -> bool {
    if crate::domain::tools::project_tasks::is_permission_rule_name(name) {
        return true;
    }
    if crate::domain::tools::project_tools::is_permission_rule_name(name) {
        return true;
    }
    if let Some(mcp_tool_prefix) = name.strip_prefix("mcpv1__") {
        #[cfg(windows)]
        return crate::domain::tools::mcp::valid_bound_permission_rule_suffix(mcp_tool_prefix);
        #[cfg(not(windows))]
        return {
            let _ = mcp_tool_prefix;
            false
        };
    }
    if let Some(mcp_tool_prefix) = name.strip_prefix("mcp__") {
        #[cfg(windows)]
        return crate::domain::tools::mcp::valid_permission_rule_suffix(mcp_tool_prefix);
        #[cfg(not(windows))]
        return {
            let _ = mcp_tool_prefix;
            false
        };
    }
    #[cfg(windows)]
    if name == "computer_use" {
        return true;
    }
    matches!(
        name,
        "list_files"
            | "search_files"
            | "read_file"
            | "write_file"
            | "edit_file"
            | "get_file_info"
            | "execute_command"
            | "send_terminal_input"
            | "web_search"
            | "read_url_content"
            | "save_memory"
            | "save_project_memory"
            | "run_project_task"
            | "delegate_task"
            | "list_subagent_models"
            | "get_subagent_result"
            | "cancel_subagent"
    )
}

pub(crate) fn invalid_tool_permission_rules() -> ServiceError {
    ServiceError::new(
        "invalid_tool_permission_rules",
        "One or more project tool permission rules are invalid.",
        false,
    )
}
