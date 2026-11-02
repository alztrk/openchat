use serde::Serialize;

/// Broad, stable classification for errors crossing the local-service boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorCategory {
    Validation,
    Authentication,
    Authorization,
    NotFound,
    Conflict,
    RateLimit,
    Network,
    Provider,
    Storage,
    Filesystem,
    Process,
    Protocol,
    Timeout,
    ResourceLimit,
    Cancelled,
    Unavailable,
    Internal,
}

impl ErrorCategory {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Authentication => "authentication",
            Self::Authorization => "authorization",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::RateLimit => "rate_limit",
            Self::Network => "network",
            Self::Provider => "provider",
            Self::Storage => "storage",
            Self::Filesystem => "filesystem",
            Self::Process => "process",
            Self::Protocol => "protocol",
            Self::Timeout => "timeout",
            Self::ResourceLimit => "resource_limit",
            Self::Cancelled => "cancelled",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }

    pub(crate) fn from_code(code: &str) -> Self {
        let explicit_category = match code {
            "browser_url_blocked"
            | "opencode_free_tier_restricted"
            | "personal_memory_disabled"
            | "plan_mode_tools_disabled"
            | "project_memory_assistant_saves_disabled"
            | "project_memory_disabled"
            | "tool_not_available" => Some(Self::Authorization),
            "project_skill_missing" => Some(Self::NotFound),
            "connection_changed"
            | "connector_package_in_use"
            | "duplicate_request_id"
            | "filesystem_already_exists"
            | "filesystem_file_locked"
            | "goal_state_changed"
            | "hugging_face_revision_changed"
            | "image_collection_name_exists"
            | "project_task_changed"
            | "question_route_changed"
            | "security_scan_already_exists"
            | "security_scan_not_active" => Some(Self::Conflict),
            "context_window_exceeded" | "project_skills_too_many" | "request_too_complex" => {
                Some(Self::ResourceLimit)
            }
            "browser_inspect_failed" | "hugging_face_download_incomplete" | "refresh_failed" => {
                Some(Self::Network)
            }
            "browser_open_failed" | "connection_remove_worker_failed" => Some(Self::Process),
            "question_delivery_failed"
            | "semantic_memory_progress_failed"
            | "tool_execution_unavailable" => Some(Self::Unavailable),
            "context_compaction_failed"
            | "image_generation_failed"
            | "image_generation_incomplete"
            | "title_generation_failed"
            | "title_generation_incomplete" => Some(Self::Provider),
            "connector_catalog_invalid_data"
            | "context_compaction_unavailable"
            | "local_service_output_failed"
            | "serialization_failed" => Some(Self::Protocol),
            "connector_catalog_invalid_source"
            | "connector_package_integrity_failed"
            | "conversation_not_routed"
            | "fast_mode_unsupported"
            | "goal_objective_too_long"
            | "project_instructions_invalid_encoding"
            | "project_memory_invalid_content"
            | "project_not_git_repository"
            | "project_skill_invalid_encoding"
            | "project_skill_invalid_id"
            | "project_skill_invalid_selection"
            | "security_scan_tools_unsupported" => Some(Self::Validation),
            "project_history_context_cleanup_failed" | "semantic_memory_prepare_failed" => {
                Some(Self::Storage)
            }
            "question_run_checkpoint_missing" => Some(Self::Storage),
            "question_wait_interrupted" => Some(Self::Cancelled),
            "refresh_rejected" => Some(Self::Authentication),
            _ => None,
        };
        if let Some(category) = explicit_category {
            return category;
        }

        if code.contains("cancel") || code.contains("aborted") {
            Self::Cancelled
        } else if code.contains("authorization")
            || code.contains("permission")
            || code.contains("approval")
            || code.contains("denied")
            || code.contains("forbidden")
            || code.contains("outside_project")
            || code.contains("unsafe")
        {
            Self::Authorization
        } else if code.contains("authentication")
            || code.contains("credential")
            || code.contains("passphrase")
            || code.contains("auth_")
            || code.contains("oauth_")
            || code.contains("unauthorized")
            || code == "invalid_api_key"
        {
            Self::Authentication
        } else if code.contains("not_found") {
            Self::NotFound
        } else if code.contains("rate_limit")
            || code.contains("rate_limited")
            || code.contains("quota")
            || code.contains("usage_limit")
        {
            Self::RateLimit
        } else if code.contains("conflict")
            || code.contains("stale")
            || code.contains("already_active")
            || code.contains("already_reverted")
            || code.contains("in_progress")
        {
            Self::Conflict
        } else if code.contains("storage")
            || code.contains("sqlite")
            || (code.contains("database")
                && !code.contains("queue_full")
                && !code.contains("queue_timeout"))
            || code.ends_with("_save_failed")
            || code.ends_with("_load_failed")
            || code.ends_with("_persistence_failed")
        {
            Self::Storage
        } else if code.contains("timeout") || code.contains("timed_out") {
            Self::Timeout
        } else if code.contains("network")
            || code.contains("connection_refused")
            || code.contains("connection_reset")
            || code == "connection_unavailable"
            || code.contains("dns")
            || code.ends_with("_search_failed")
            || code.ends_with("_read_url_failed")
            || code.ends_with("_navigation_failed")
        {
            Self::Network
        } else if code.contains("filesystem")
            || code.contains("file_")
            || code.contains("path_")
            || code.contains("workspace")
            || code.contains("worktree")
        {
            Self::Filesystem
        } else if code.contains("protocol")
            || code == "invalid_service_response"
            || code.contains("schema_version")
            || code.starts_with("rpc_")
        {
            Self::Protocol
        } else if code.contains("provider")
            || code.contains("model")
            || code.contains("inference")
            || code.starts_with("response_")
        {
            Self::Provider
        } else if code.contains("process")
            || code.contains("terminal")
            || code.starts_with("local_engine")
            || code.starts_with("mcp_server_")
            || code.starts_with("service_start_")
            || code == "command_failed"
            || code == "mcp_tool_failed"
            || code.starts_with("project_task_")
            || code.starts_with("project_tool_")
            || code.starts_with("computer_use_")
        {
            Self::Process
        } else if code.contains("limit")
            || code.contains("too_large")
            || code.contains("queue_full")
            || code.contains("capacity")
            || code.contains("busy")
        {
            Self::ResourceLimit
        } else if code.contains("unavailable") || code == "local_service_operation_failed" {
            Self::Unavailable
        } else if code.starts_with("invalid_")
            || code.starts_with("unsupported_")
            || code.starts_with("missing_")
            || code.starts_with("required_")
            || code.ends_with("_invalid")
            || code.ends_with("_required")
            || code.ends_with("_empty")
        {
            Self::Validation
        } else {
            Self::Internal
        }
    }
}

pub(crate) fn is_safe_diagnostic_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 96
        && code.as_bytes()[0].is_ascii_lowercase()
        && code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
