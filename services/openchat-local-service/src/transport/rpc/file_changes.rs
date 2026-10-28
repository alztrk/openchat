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
        "chat.file_changes.checkpoints.list" => {
            let conversation_id = required_string(params, "conversationId")?;
            let checkpoints = file_changes::checkpoints::list_checkpoints(storage, conversation_id)
                .map_err(file_changes_unavailable)?;
            Ok(json!({"checkpoints": checkpoints}))
        }
        "chat.file_changes.checkpoints.diff" => {
            let conversation_id = required_string(params, "conversationId")?;
            let checkpoint_id = required_string(params, "checkpointId")?;
            let file_id = required_string(params, "fileId")?;
            file_changes::checkpoints::read_checkpoint_diff(
                storage,
                conversation_id,
                checkpoint_id,
                file_id,
            )
            .map_err(file_changes_unavailable)
        }
        "chat.file_changes.checkpoints.recovery_status" => {
            let conversation_id = required_string(params, "conversationId")?;
            let checkpoint_id = required_string(params, "checkpointId")?;
            let status = file_changes::checkpoints::checkpoint_recovery_state(
                storage,
                conversation_id,
                checkpoint_id,
            )
            .map_err(map_checkpoint_restore_error)?;
            Ok(json!({"status": status}))
        }
        "chat.file_changes.checkpoints.restore" => {
            let conversation_id = required_string(params, "conversationId")?;
            let checkpoint_id = required_string(params, "checkpointId")?;
            let changes = file_changes::checkpoints::restore_checkpoint(
                storage,
                conversation_id,
                checkpoint_id,
            )
            .map_err(map_checkpoint_restore_error)?;
            Ok(json!({"changes": changes}))
        }
        "chat.file_changes.checkpoints.revert_operation" => {
            let conversation_id = required_string(params, "conversationId")?;
            let checkpoint_id = required_string(params, "checkpointId")?;
            let changes = file_changes::checkpoints::revert_checkpoint_operation(
                storage,
                conversation_id,
                checkpoint_id,
            )
            .map_err(map_checkpoint_restore_error)?;
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
        file_changes::RevertError::AppliedButResponseUnavailable => ServiceError::new(
            "file_change_applied_response_unavailable",
            "The file-change update was saved, but its current state could not be read. Refresh file changes before retrying.",
            false,
        ),
        file_changes::RevertError::RollbackFailed => ServiceError::new(
            "file_change_rollback_failed",
            "A file change could not be saved, and restoring its previous contents did not complete. Inspect the current file before continuing.",
            false,
        ),
        file_changes::RevertError::Unavailable => file_changes_unavailable(()),
    }
}

fn map_checkpoint_restore_error(
    error: file_changes::checkpoints::CheckpointRestoreError,
) -> ServiceError {
    match error {
        file_changes::checkpoints::CheckpointRestoreError::Conflict => ServiceError::new(
            "file_checkpoint_conflict",
            "A file changed after this failed operation. No files were restored.",
            false,
        ),
        file_changes::checkpoints::CheckpointRestoreError::AlreadyRestored => ServiceError::new(
            "file_checkpoint_already_restored",
            "The failed operation has already been restored.",
            false,
        ),
        file_changes::checkpoints::CheckpointRestoreError::NotFound => ServiceError::new(
            "file_checkpoint_not_found",
            "The selected checkpoint is no longer available.",
            false,
        ),
        file_changes::checkpoints::CheckpointRestoreError::NotRecoverable => ServiceError::new(
            "file_checkpoint_not_recoverable",
            "This checkpoint is not eligible for failed-operation recovery.",
            false,
        ),
        file_changes::checkpoints::CheckpointRestoreError::RollbackFailed => ServiceError::new(
            "file_checkpoint_rollback_failed",
            "The checkpoint could not be restored completely. Inspect the current file changes before continuing.",
            false,
        ),
        file_changes::checkpoints::CheckpointRestoreError::Unavailable => {
            file_changes_unavailable(())
        }
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

#[cfg(test)]
mod tests {
    use super::map_revert_error;
    use crate::file_changes::RevertError;

    #[test]
    fn committed_update_with_unavailable_response_uses_refresh_error() {
        let error = map_revert_error(RevertError::AppliedButResponseUnavailable);

        assert_eq!(error.code, "file_change_applied_response_unavailable");
        assert!(!error.retryable);
        assert!(error.message.contains("Refresh file changes"));
    }
}
