//! Conversation and message RPC handlers.

use super::*;
use crate::attachment_crypto::{AttachmentCipher, attachment_associated_data};

struct StoredMessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    created_at: Option<i64>,
    output_tokens: Option<i64>,
    tokens_per_second: Option<f64>,
    elapsed_microseconds: Option<i64>,
    provider_id: Option<String>,
    model_id: Option<String>,
    citation_sources: String,
    personal_memory_fact_ids: String,
    reasoning_summaries: String,
    tool_activities: String,
    status: String,
    failure_code: Option<String>,
}

fn stored_message_row_from_storage(row: &StorageRow) -> rusqlite::Result<StoredMessageRow> {
    Ok(StoredMessageRow {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
        output_tokens: row.get(5)?,
        tokens_per_second: row.get(6)?,
        elapsed_microseconds: row.get(7)?,
        provider_id: row.get(8)?,
        model_id: row.get(9)?,
        citation_sources: row.get(10)?,
        personal_memory_fact_ids: row.get(11)?,
        reasoning_summaries: row.get(12)?,
        tool_activities: row.get(13)?,
        status: row.get(14)?,
        failure_code: row.get(15)?,
    })
}

fn message_from_stored_row(
    row: StoredMessageRow,
    remaining_nodes: &mut usize,
) -> rusqlite::Result<Value> {
    let citation_sources =
        sanitize_stored_metadata(row.citation_sources, 10, false, remaining_nodes)?;
    let personal_memory_fact_ids =
        sanitize_stored_metadata(row.personal_memory_fact_ids, 11, false, remaining_nodes)?;
    let reasoning_summaries =
        sanitize_stored_metadata(row.reasoning_summaries, 12, false, remaining_nodes)?;
    let tool_activities = sanitize_stored_metadata(row.tool_activities, 13, true, remaining_nodes)?;
    Ok(json!({
        "id": row.id,
        "conversationId": row.conversation_id,
        "role": row.role,
        "content": row.content,
        "createdAt": row.created_at,
        "outputTokens": row.output_tokens,
        "tokensPerSecond": row.tokens_per_second,
        "elapsedMicroseconds": row.elapsed_microseconds,
        "providerId": row.provider_id,
        "modelId": row.model_id,
        "citationSources": citation_sources,
        "personalMemoryFactIds": personal_memory_fact_ids,
        "reasoningSummaries": reasoning_summaries,
        "toolActivities": tool_activities,
        "status": row.status,
        "failureCode": row.failure_code,
    }))
}

pub(super) fn list_conversations(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id,
                    is_pinned, is_archived, created_at, updated_at, agent_json, reasoning_effort
             FROM conversations
             WHERE NOT EXISTS (
                SELECT 1 FROM security_scans WHERE security_scans.conversation_id = conversations.id
             )
             ORDER BY is_pinned DESC, updated_at DESC, id ASC",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| conversation_from_row!(row))
        .map_err(storage_error)?;
    let mut conversations = Vec::new();
    for row in rows {
        conversations.push(row.map_err(storage_error)?);
    }
    Ok(json!({"conversations": conversations}))
}

pub(super) fn get_conversation(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "conversationId")?;
    let connection = storage.connect().map_err(storage_error)?;
    let conversation = connection
        .query_row(
            "SELECT id, title, title_source, provider_id, model_id, connection_id,
                    workspace_id, api_key_connection_id, project_id,
                    is_pinned, is_archived, created_at, updated_at, agent_json, reasoning_effort
             FROM conversations WHERE id = ?1",
            [id],
            |row| conversation_from_row!(row),
        )
        .optional()
        .map_err(storage_error)?;
    Ok(json!({"conversation": conversation}))
}

pub(super) fn create_conversation(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "id")?;
    let title = required_title(input, "title")?;
    let provider_id = required_safe_id(input, "providerId")?;
    let model_id = required_model_id(input, "modelId")?;
    let connection_id = optional_safe_id(input, "connectionId")?;
    let workspace_id = optional_safe_id(input, "workspaceId")?;
    let api_key_connection_id = optional_safe_id(input, "apiKeyConnectionId")?;
    let project_id = optional_safe_id(input, "projectId")?;
    let agent_json = optional_json_object(input, "agent")?
        .map(serialize_json_without_sensitive_values)
        .transpose()?;
    let reasoning_effort = optional_bounded_string(input, "reasoningEffort", 128, true)?;
    let now = unix_time_millis()?;
    let connection = storage.connect().map_err(storage_error)?;
    connection
        .execute(
            "INSERT INTO conversations (
                id, title, title_source, provider_id, model_id, connection_id,
                workspace_id, api_key_connection_id, project_id, agent_json, reasoning_effort,
                created_at, updated_at
             ) VALUES (?1, ?2, 'automatic', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            params![
                id,
                title,
                provider_id,
                model_id,
                connection_id,
                workspace_id,
                api_key_connection_id,
                project_id,
                agent_json,
                reasoning_effort,
                now,
            ],
        )
        .map_err(storage_error)?;
    Ok(json!({"saved": true}))
}

pub(super) fn rename_conversation(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "conversationId")?;
    let title = required_title(input, "title")?;
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute(
            "UPDATE conversations SET title = ?1, title_source = 'manual', updated_at = ?2
             WHERE id = ?3",
            params![title, unix_time_millis()?, id],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found("conversation"));
    }
    Ok(json!({"saved": true}))
}

pub(super) fn set_conversation_route(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let provider_id = required_safe_id(input, "providerId")?;
    let model_id = required_model_id(input, "modelId")?;
    let connection_id = optional_safe_id(input, "connectionId")?;
    let workspace_id = optional_safe_id(input, "workspaceId")?;
    let api_key_connection_id = optional_safe_id(input, "apiKeyConnectionId")?;
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute(
            "UPDATE conversations
             SET provider_id = ?1, model_id = ?2, connection_id = ?3,
                 workspace_id = ?4, api_key_connection_id = ?5, updated_at = ?6
             WHERE id = ?7
               AND NOT EXISTS (
                   SELECT 1 FROM security_scans WHERE security_scans.conversation_id = conversations.id
               )",
            params![
                provider_id,
                model_id,
                connection_id,
                workspace_id,
                api_key_connection_id,
                unix_time_millis()?,
                conversation_id,
            ],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found("conversation"));
    }
    Ok(json!({"saved": true}))
}

pub(super) fn set_conversation_reasoning(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let reasoning_effort = optional_bounded_string(input, "reasoningEffort", 128, true)?;
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute(
            "UPDATE conversations SET reasoning_effort = ?1 WHERE id = ?2
             AND NOT EXISTS (
                 SELECT 1 FROM security_scans WHERE security_scans.conversation_id = conversations.id
             )",
            params![reasoning_effort, conversation_id],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found("conversation"));
    }
    Ok(json!({"saved": true}))
}

pub(super) fn set_conversation_pinned(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "conversationId")?;
    let is_pinned = input
        .get("isPinned")
        .and_then(Value::as_bool)
        .ok_or_else(invalid_request_params)?;
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute(
            "UPDATE conversations SET is_pinned = ?1 WHERE id = ?2",
            params![is_pinned, id],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found("conversation"));
    }
    Ok(json!({"saved": true}))
}

pub(super) fn move_conversations(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let ids = required_id_array(input, "conversationIds")?;
    let project_id = optional_safe_id(input, "projectId")?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    if let Some(project_id) = project_id {
        let exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                [project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage_error)?;
        if !exists {
            return Err(not_found("project"));
        }
    }
    for id in &ids {
        let changed = transaction
            .execute(
                "UPDATE conversations SET project_id = ?1, is_pinned = 0
                 WHERE id = ?2
                   AND NOT EXISTS (
                       SELECT 1 FROM security_scans WHERE security_scans.conversation_id = conversations.id
                   )",
                params![project_id, id],
            )
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(not_found("conversation"));
        }
    }
    transaction.commit().map_err(storage_error)?;
    Ok(json!({"saved": true}))
}

pub(super) fn set_conversations_archived(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let ids = required_id_array(input, "conversationIds")?;
    let is_archived = input
        .get("isArchived")
        .and_then(Value::as_bool)
        .ok_or_else(invalid_request_params)?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    for id in &ids {
        let changed = transaction
            .execute(
                "UPDATE conversations
                 SET is_archived = ?1,
                     is_pinned = CASE WHEN ?1 = 1 THEN 0 ELSE is_pinned END
                 WHERE id = ?2",
                params![is_archived, id],
            )
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(not_found("conversation"));
        }
    }
    transaction.commit().map_err(storage_error)?;
    Ok(json!({"saved": true}))
}

pub(super) fn delete_conversation(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "conversationId")?;
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM attachment_text_index WHERE conversation_id = ?1",
            [id],
        )
        .map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM conversation_todos WHERE conversation_id = ?1",
            [id],
        )
        .map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM saved_output_versions WHERE conversation_id = ?1",
            [id],
        )
        .map_err(storage_error)?;
    transaction
        .execute("DELETE FROM saved_outputs WHERE conversation_id = ?1", [id])
        .map_err(storage_error)?;
    let deleted = transaction
        .execute("DELETE FROM conversations WHERE id = ?1", [id])
        .map_err(storage_error)?;
    if deleted != 1 {
        return Err(not_found("conversation"));
    }
    transaction.commit().map_err(storage_error)?;
    drop(connection);
    let attachment_cleanup_pending = remove_attachment_directory(storage.root(), id).is_err();
    if attachment_cleanup_pending {
        eprintln!("conversation_attachment_cleanup_pending");
    }
    Ok(json!({
        "deleted": true,
        "attachmentCleanupPending": attachment_cleanup_pending,
    }))
}

pub(super) fn clear_conversation_history(storage: &AppStorage) -> Result<Value, ServiceError> {
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute_batch(
            "DELETE FROM conversation_todos;
             DELETE FROM attachment_text_index;
             DELETE FROM saved_output_versions;
             DELETE FROM saved_outputs;
             DELETE FROM conversations;",
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    drop(connection);
    let attachment_cleanup_pending = remove_all_attachment_directories(storage.root()).is_err();
    if attachment_cleanup_pending {
        eprintln!("conversation_history_attachment_cleanup_pending");
    }
    Ok(json!({
        "cleared": true,
        "attachmentCleanupPending": attachment_cleanup_pending,
    }))
}

pub(super) fn list_messages(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let limit = input
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .and_then(|limit| usize::try_from(limit).ok())
                .filter(|limit| (1..=MAX_MESSAGE_PAGE).contains(limit))
                .ok_or_else(invalid_request_params)
        })
        .transpose()?
        .unwrap_or(MAX_MESSAGE_PAGE);
    let before_message_id = optional_safe_id(input, "beforeMessageId")?;
    let connection = storage.connect().map_err(storage_error)?;
    let before_created_at = before_message_id
        .map(|message_id| {
            connection
                .query_row(
                    "SELECT COALESCE(created_at, 0) FROM messages
                     WHERE conversation_id = ?1 AND id = ?2",
                    params![conversation_id, message_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(storage_error)?
                .ok_or_else(invalid_request_params)
        })
        .transpose()?;
    let limit_with_lookahead = i64::try_from(limit + 1).map_err(|_| invalid_request_params())?;
    let rows = if let Some(before_created_at) = before_created_at {
        let mut statement = connection
            .prepare(
                "SELECT id, conversation_id, role, content, created_at, output_tokens,
                        tokens_per_second, elapsed_microseconds, provider_id, model_id,
                        citation_sources, personal_memory_fact_ids, reasoning_summaries,
                        tool_activities, status, failure_code
                 FROM messages
                 WHERE conversation_id = ?1
                   AND (COALESCE(created_at, 0) < ?2
                        OR (COALESCE(created_at, 0) = ?2 AND id < ?3))
                 ORDER BY COALESCE(created_at, 0) DESC, id DESC LIMIT ?4",
            )
            .map_err(storage_error)?;
        statement
            .query_map(
                params![
                    conversation_id,
                    before_created_at,
                    before_message_id,
                    limit_with_lookahead
                ],
                stored_message_row_from_storage,
            )
            .map_err(storage_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)?
    } else {
        let mut statement = connection
            .prepare(
                "SELECT id, conversation_id, role, content, created_at, output_tokens,
                        tokens_per_second, elapsed_microseconds, provider_id, model_id,
                        citation_sources, personal_memory_fact_ids, reasoning_summaries,
                        tool_activities, status, failure_code
                 FROM messages WHERE conversation_id = ?1
                 ORDER BY COALESCE(created_at, 0) DESC, id DESC LIMIT ?2",
            )
            .map_err(storage_error)?;
        statement
            .query_map(
                params![conversation_id, limit_with_lookahead],
                stored_message_row_from_storage,
            )
            .map_err(storage_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)?
    };
    drop(connection);

    let has_more = rows.len() > limit;
    let mut rows = rows;
    rows.truncate(limit);
    rows.reverse();
    let next_before_message_id = if has_more {
        rows.first().map(|row| row.id.clone())
    } else {
        None
    };
    let mut remaining_nodes = MAX_REQUEST_PARAMETER_NODES;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(message_from_stored_row(row, &mut remaining_nodes).map_err(storage_error)?);
    }
    Ok(json!({
        "messages": messages,
        "hasMore": has_more,
        "nextBeforeMessageId": next_before_message_id,
    }))
}

pub(super) fn save_message(
    params: &Value,
    storage: &AppStorage,
    cancellation_gate: Option<&OperationGate>,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let id = required_safe_id(input, "id")?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let role = required_message_role(input, "role")?;
    let mut content = required_content(input, "content")?;
    let status = required_message_status(input, "status")?;
    let provider_id = optional_safe_id(input, "providerId")?;
    let model_id = optional_model_id(input, "modelId")?;
    let mut remaining_metadata_nodes = MAX_REQUEST_PARAMETER_NODES;
    let citation_sources =
        optional_json_array(input, "citationSources", &mut remaining_metadata_nodes)?;
    let personal_memory_fact_ids = optional_json_array(
        input,
        "personalMemoryFactIds",
        &mut remaining_metadata_nodes,
    )?;
    let reasoning_summaries =
        optional_json_array(input, "reasoningSummaries", &mut remaining_metadata_nodes)?;
    let tool_activities = if input
        .get("toolActivities")
        .is_some_and(|value| !value.is_null())
    {
        Some(optional_json_array(
            input,
            "toolActivities",
            &mut remaining_metadata_nodes,
        )?)
    } else {
        None
    };
    let failure_code = optional_failure_code(input, "failureCode")?;
    let created_at = input
        .get("createdAt")
        .map(|value| value.as_i64().ok_or_else(invalid_request_params))
        .transpose()?
        .unwrap_or(unix_time_millis()?);
    let output_tokens = optional_nonnegative_i64(input, "outputTokens")?;
    let tokens_per_second = optional_nonnegative_f64(input, "tokensPerSecond")?;
    let elapsed_microseconds = optional_nonnegative_i64(input, "elapsedMicroseconds")?;
    let attachments = parse_attachment_inputs(input.get("attachments"))?;
    if role != "user" && !attachments.is_empty() {
        return Err(invalid_request_params());
    }
    if !attachments.is_empty() && content.starts_with(MESSAGE_CONTENT_PREFIX) {
        let envelope = parse_attachment_envelope(&content).map_err(|_| invalid_request_params())?;
        let matches_payload = envelope.attachments.len() == attachments.len()
            && envelope
                .attachments
                .iter()
                .zip(&attachments)
                .all(|(metadata, attachment)| {
                    metadata.id == attachment.id
                        && metadata.name == attachment.name
                        && metadata.mime_type == attachment.mime_type
                        && metadata.size_bytes == attachment.size_bytes
                        && metadata.kind == attachment.kind
                });
        if !matches_payload {
            return Err(invalid_request_params());
        }
    }
    let attachment_metadata = attachments
        .iter()
        .map(|attachment| {
            json!({
                "id": attachment.id,
                "name": attachment.name,
                "mimeType": attachment.mime_type,
                "sizeBytes": attachment.size_bytes,
                "kind": attachment.kind,
            })
        })
        .collect::<Vec<_>>();
    if attachments.is_empty() {
        content = crate::attachments::encode_plain_message_content(&content);
    } else if !content.starts_with(MESSAGE_CONTENT_PREFIX) {
        content = format!(
            "{MESSAGE_CONTENT_PREFIX}{}",
            json!({"content": content, "attachments": attachment_metadata})
        );
    }
    if content.len() > MAX_MESSAGE_CONTENT_BYTES {
        return Err(invalid_request_params());
    }
    let mut staged = if attachments.is_empty() {
        None
    } else {
        match stage_attachments(storage, conversation_id, id, &attachments) {
            Ok(staged) => Some(staged),
            Err(error) => {
                log_message_save_failure(storage, "attachment_staging", &error);
                return Err(error);
            }
        }
    };
    let mut connection = match storage.connect() {
        Ok(connection) => connection,
        Err(error) => {
            if let Some(staging) = staged.as_ref().and_then(|staged| staged.staging.as_ref()) {
                let _ = remove_directory_if_present(staging);
            }
            let error = storage_error(error);
            log_message_save_failure(storage, "database_connect", &error);
            return Err(error);
        }
    };
    let mut failure_stage = "transaction_begin";
    let mut published_attachment_directory = false;
    let mut attachment_cleanup_required = false;
    let mut durable_commit_mode_enabled = false;
    let write_result = (|| -> Result<(), ServiceError> {
        if !attachments.is_empty() {
            failure_stage = "attachment_durability_setup";
            connection
                .execute_batch("PRAGMA synchronous = FULL;")
                .map_err(storage_error)?;
            durable_commit_mode_enabled = true;
        }

        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        let previous_content = transaction
            .query_row(
                "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
                params![conversation_id, id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(storage_error)?;
        attachment_cleanup_required = previous_content.as_deref().is_some_and(|previous| {
            previous.starts_with(MESSAGE_CONTENT_PREFIX)
                && !content.starts_with(MESSAGE_CONTENT_PREFIX)
        });
        if let Some(staged) = staged.as_mut()
            && staged.staging.is_none()
        {
            failure_stage = "attachment_revalidate";
            match fs::symlink_metadata(&staged.destination) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(storage_unavailable());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    *staged = stage_attachments(storage, conversation_id, id, &attachments)?;
                }
                Err(_) => return Err(storage_unavailable()),
            }
        }
        failure_stage = "message_insert";
        transaction
            .execute(
                "INSERT INTO messages (
                    id, conversation_id, role, content, created_at, output_tokens,
                    tokens_per_second, elapsed_microseconds, provider_id, model_id,
                    citation_sources, personal_memory_fact_ids, reasoning_summaries,
                    tool_activities, status, failure_code
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                            ?11, ?12, ?13, COALESCE(?14, '[]'), ?15, ?16)
                 ON CONFLICT(conversation_id, id) DO UPDATE SET
                    role = excluded.role,
                    content = excluded.content,
                    created_at = excluded.created_at,
                    output_tokens = excluded.output_tokens,
                    tokens_per_second = excluded.tokens_per_second,
                    elapsed_microseconds = excluded.elapsed_microseconds,
                    provider_id = excluded.provider_id,
                    model_id = excluded.model_id,
                    citation_sources = excluded.citation_sources,
                    personal_memory_fact_ids = excluded.personal_memory_fact_ids,
                    reasoning_summaries = excluded.reasoning_summaries,
                    tool_activities = COALESCE(?14, messages.tool_activities),
                    status = excluded.status,
                    failure_code = excluded.failure_code",
                params![
                    id,
                    conversation_id,
                    role,
                    content,
                    created_at,
                    output_tokens,
                    tokens_per_second,
                    elapsed_microseconds,
                    provider_id,
                    model_id,
                    citation_sources,
                    personal_memory_fact_ids,
                    reasoning_summaries,
                    tool_activities,
                    status,
                    failure_code,
                ],
            )
            .map_err(storage_error)?;
        failure_stage = "conversation_update";
        transaction
            .execute(
                "UPDATE conversations SET updated_at = MAX(updated_at, ?1) WHERE id = ?2",
                params![created_at, conversation_id],
            )
            .map_err(storage_error)?;
        failure_stage = "transaction_commit";
        if cancellation_gate.is_some_and(|gate| !gate.try_begin_commit()) {
            return Err(ServiceError::new(
                "operation_cancelled",
                "The message save was cancelled before it committed.",
                false,
            ));
        }
        failure_stage = "attachment_commit";
        if let Some(staged) = staged.as_ref()
            && let Some(staging) = staged.staging.as_ref()
        {
            match publish_staged_attachment_directory(staging, &staged.destination) {
                Ok(()) => published_attachment_directory = true,
                Err(AttachmentDirectoryPublishError::Rename) => {
                    let cipher = AttachmentCipher::from_profile_key(storage.database_key())
                        .map_err(|_| storage_unavailable())?;
                    if !attachment_directory_matches(
                        &staged.destination,
                        conversation_id,
                        id,
                        &attachments,
                        &cipher,
                    )? {
                        return Err(storage_unavailable());
                    }
                    let _ = remove_directory_if_present(staging);
                }
                Err(_) => return Err(storage_unavailable()),
            }
        }
        failure_stage = "transaction_commit";
        transaction.commit().map_err(storage_error)
    })();
    if durable_commit_mode_enabled
        && let Err(error) = connection.execute_batch("PRAGMA synchronous = NORMAL;")
    {
        log_message_save_failure(
            storage,
            "attachment_durability_reset",
            &storage_error(error),
        );
    }
    if let Err(error) = &write_result {
        if error.code != "operation_cancelled" {
            log_message_save_failure(storage, failure_stage, error);
        }
    }
    if write_result.is_err() {
        if published_attachment_directory && let Some(staged) = staged.as_ref() {
            rollback_published_attachment_directory(
                storage,
                &mut connection,
                staged,
                conversation_id,
                id,
            );
        }
        if let Some(staging) = staged.as_ref().and_then(|staged| staged.staging.as_ref()) {
            let _ = remove_directory_if_present(staging);
        }
        return write_result.map(|_| json!({"saved": true}));
    }
    let attachment_cleanup_pending = if attachment_cleanup_required
        && remove_message_attachment_directory_if_unreferenced(
            storage.root(),
            &mut connection,
            conversation_id,
            id,
        )
        .is_err()
    {
        eprintln!("message_attachment_cleanup_pending");
        true
    } else {
        false
    };
    Ok(json!({
        "saved": true,
        "attachmentCleanupPending": attachment_cleanup_pending,
    }))
}

pub(super) fn rollback_published_attachment_directory(
    storage: &AppStorage,
    connection: &mut StorageConnection<'_>,
    staged: &StagedAttachments,
    conversation_id: &str,
    message_id: &str,
) {
    let cleanup_result = (|| -> Result<(), ServiceError> {
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        let persisted_message = transaction
            .query_row(
                "SELECT 1 FROM messages WHERE conversation_id = ?1 AND id = ?2 LIMIT 1",
                params![conversation_id, message_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(storage_error)?;
        if persisted_message.is_none() {
            remove_directory_if_present(&staged.destination).map_err(|_| storage_unavailable())?;
        }
        transaction.commit().map_err(storage_error)
    })();

    if let Err(error) = cleanup_result {
        log_message_save_failure(storage, "attachment_commit_cleanup", &error);
    }
}

fn log_message_save_failure(storage: &AppStorage, stage: &'static str, error: &ServiceError) {
    if storage
        .log_chat_message_save_failure(stage, error.code, error.diagnostic_code.as_deref())
        .is_err()
    {
        eprintln!("chat_message_save_diagnostic_write_failed");
    }
}

pub(super) fn search_messages(params: &Value, storage: &AppStorage) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let query =
        optional_bounded_string(input, "query", MAX_SEARCH_QUERY_BYTES, false)?.unwrap_or_default();
    let role = match input.get("roleFilter") {
        None | Some(Value::Null) => None,
        Some(Value::String(role)) if role == "user" || role == "assistant" => Some(role.as_str()),
        _ => return Err(invalid_request_params()),
    };
    let has_code_only = input
        .get("hasCodeOnly")
        .map(|value| value.as_bool().ok_or_else(invalid_request_params))
        .transpose()?
        .unwrap_or(false);
    let has_query = !query.trim().is_empty();
    if !has_query && role.is_none() && !has_code_only {
        return Ok(json!({"messages": []}));
    }
    let limit = input
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .and_then(|limit| usize::try_from(limit).ok())
                .filter(|limit| (1..=MAX_SEARCH_RESULTS).contains(limit))
                .ok_or_else(invalid_request_params)
        })
        .transpose()?
        .unwrap_or(40);
    let sql = "SELECT id, conversation_id, role, content, created_at
         FROM messages
         WHERE (?1 = '' OR instr(openchat_normalize_text(content), ?1) > 0)
           AND (?2 = '' OR role = ?2)
           AND (?3 = 0 OR content LIKE '%```%')
         ORDER BY created_at DESC, id DESC LIMIT ?4";
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection.prepare(sql).map_err(storage_error)?;
    let normalized = normalize_search_text(query);
    let role_value = role.unwrap_or("");
    let rows = statement
        .query_map(
            params![normalized, role_value, has_code_only, limit as i64],
            |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "conversationId": row.get::<_, String>(1)?,
                    "role": row.get::<_, String>(2)?,
                    "content": row.get::<_, String>(3)?,
                    "createdAt": row.get::<_, Option<i64>>(4)?,
                }))
            },
        )
        .map_err(storage_error)?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(storage_error)?);
    }
    Ok(json!({"messages": messages}))
}

pub(super) fn has_completed_assistant_response(
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let completed = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM messages WHERE role = 'assistant' AND status = 'completed'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage_error)?;
    Ok(json!({"completed": completed}))
}

pub(super) fn list_model_favorites(storage: &AppStorage) -> Result<Value, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT provider_id, model_id, display_name, source_connection_id
             FROM model_favorites
             ORDER BY provider_id ASC, display_name ASC, model_id ASC",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(json!({
                "providerId": row.get::<_, String>(0)?,
                "modelId": row.get::<_, String>(1)?,
                "displayName": row.get::<_, String>(2)?,
                "sourceConnectionId": row.get::<_, Option<String>>(3)?,
            }))
        })
        .map_err(storage_error)?;
    let mut favorites = Vec::new();
    for row in rows {
        favorites.push(row.map_err(storage_error)?);
    }
    Ok(json!({"favorites": favorites}))
}

pub(super) fn set_model_favorite(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let provider_id = required_safe_id(input, "providerId")?;
    let model_id = required_model_id(input, "modelId")?;
    let display_name = required_bounded_string(input, "displayName", MAX_TITLE_CHARS * 4, true)?;
    let is_favorite = input
        .get("isFavorite")
        .and_then(Value::as_bool)
        .ok_or_else(invalid_request_params)?;
    let source_connection_id = optional_safe_id(input, "sourceConnectionId")?;
    let connection = storage.connect().map_err(storage_error)?;
    if is_favorite {
        connection
            .execute(
                "INSERT INTO model_favorites (
                    provider_id, model_id, display_name, favorited_at, source_connection_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(provider_id, model_id) DO UPDATE SET
                    display_name = excluded.display_name,
                    favorited_at = excluded.favorited_at,
                    source_connection_id = excluded.source_connection_id",
                params![
                    provider_id,
                    model_id,
                    display_name,
                    unix_time_millis()?,
                    source_connection_id
                ],
            )
            .map_err(storage_error)?;
    } else {
        connection
            .execute(
                "DELETE FROM model_favorites WHERE provider_id = ?1 AND model_id = ?2",
                params![provider_id, model_id],
            )
            .map_err(storage_error)?;
    }
    Ok(json!({"saved": true}))
}

pub(super) fn read_message_attachment(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let conversation_id = required_safe_id(input, "conversationId")?;
    let message_id = required_safe_id(input, "messageId")?;
    let attachment_id = required_safe_id(input, "attachmentId")?;
    let connection = storage.connect().map_err(storage_error)?;
    let content = connection
        .query_row(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?
        .ok_or_else(|| not_found("message"))?;
    drop(connection);
    let envelope = parse_attachment_envelope(&content)?;
    let metadata = envelope
        .attachments
        .iter()
        .find(|attachment| attachment.id == attachment_id)
        .ok_or_else(|| not_found("attachment"))?;
    let expected_size = metadata.size_bytes;
    let cipher = AttachmentCipher::from_profile_key(storage.database_key())
        .map_err(|_| storage_unavailable())?;
    let file_name = format!("{attachment_id}.data");
    let (path, relative_path) =
        match attachment_path(storage.root(), conversation_id, message_id, attachment_id) {
            Ok(path) => (
                path,
                PathBuf::from(conversation_id)
                    .join(message_id)
                    .join(&file_name),
            ),
            Err(_) => {
                let legacy_path = storage
                    .root()
                    .join("attachments")
                    .join(conversation_id)
                    .join(&file_name);
                let metadata =
                    fs::symlink_metadata(&legacy_path).map_err(|_| storage_unavailable())?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(storage_unavailable());
                }
                let attachments_root = fs::canonicalize(storage.root().join("attachments"))
                    .map_err(|_| storage_unavailable())?;
                let path = fs::canonicalize(&legacy_path).map_err(|_| storage_unavailable())?;
                if !path.starts_with(&attachments_root) {
                    return Err(storage_unavailable());
                }
                (path, PathBuf::from(conversation_id).join(&file_name))
            }
        };
    let associated_data =
        attachment_associated_data(&relative_path).map_err(|_| storage_unavailable())?;
    let bytes = cipher
        .read_file(&path, &associated_data, expected_size)
        .map_err(|_| storage_unavailable())?;
    if bytes.len() != metadata.size_bytes {
        return Err(storage_unavailable());
    }
    Ok(json!({"dataBase64": STANDARD.encode(bytes.as_slice())}))
}
