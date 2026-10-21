//! Routes file-change history, comparison, revert, and cleanup requests.

use serde_json::{Value, json};

use crate::{file_changes, protocol::ServiceError, storage::AppStorage};

use super::required_string;

/// Dispatches one file-change request without exposing filesystem paths to the renderer.
pub(super) fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "chat.file_changes.list" => {
            let conversation_id = required_string(params, "conversationId")?;
            let changes = file_changes::list_changes(storage, conversation_id)
                .map_err(file_changes_unavailable)?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.diff" => {
            let conversation_id = required_string(params, "conversationId")?;
            let change_id = required_string(params, "changeId")?;
            file_changes::read_diff(storage, conversation_id, change_id)
                .map_err(file_changes_unavailable)
        }
        "chat.file_changes.revert" => {
            let conversation_id = required_string(params, "conversationId")?;
            let change_id = required_string(params, "changeId")?;
            let changes = file_changes::revert_change(storage, conversation_id, change_id)
                .map_err(map_revert_error)?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.approve" => {
            let conversation_id = required_string(params, "conversationId")?;
            let change_id = required_string(params, "changeId")?;
            let changes = file_changes::approve_change(storage, conversation_id, change_id)
                .map_err(map_revert_error)?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.approve_all" => {
            let conversation_id = required_string(params, "conversationId")?;
            let changes = file_changes::approve_all_changes(storage, conversation_id)
                .map_err(map_revert_error)?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.revert_all" => {
            let conversation_id = required_string(params, "conversationId")?;
            let changes = file_changes::revert_all_changes(storage, conversation_id)
                .map_err(map_revert_error)?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.revert_hunk" => {
            let conversation_id = required_string(params, "conversationId")?;
            let change_id = required_string(params, "changeId")?;
            let hunk_index = params
                .get("hunkIndex")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(super::invalid_request_params)?;
            let changes =
                file_changes::revert_hunk(storage, conversation_id, change_id, hunk_index)
                    .map_err(map_revert_error)?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.delete" => {
            let conversation_id = required_string(params, "conversationId")?;
            file_changes::delete_conversation_changes(storage, conversation_id)
                .map_err(file_changes_unavailable)?;
            Ok(json!({"deleted": true}))
        }
        "chat.file_changes.delete_all" => {
            file_changes::delete_all_changes(storage).map_err(file_changes_unavailable)?;
            Ok(json!({"deleted": true}))
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

/// Maps revert/approve failures to stable RPC error codes.
fn map_revert_error(error: file_changes::RevertError) -> ServiceError {
    match error {
        file_changes::RevertError::Conflict => ServiceError::new(
            "file_change_conflict",
            "This file changed after the assistant edited it. The current file was kept intact.",
            false,
        ),
        file_changes::RevertError::AlreadyReverted => ServiceError::new(
            "file_change_already_reverted",
            "This file change has already been reverted.",
            false,
        ),
        file_changes::RevertError::Unavailable => file_changes_unavailable(()),
    }
}

/// Maps storage failures to the stable file-change RPC error.
fn file_changes_unavailable<T>(_: T) -> ServiceError {
    ServiceError::new(
        "file_changes_unavailable",
        "Conversation file changes could not be loaded or updated.",
        true,
    )
}
