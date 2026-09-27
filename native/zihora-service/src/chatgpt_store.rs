use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension, params, types::Type};
use serde::Serialize;

use crate::storage::AppStorage;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptConnection {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub auth_status: String,
    pub is_selected: bool,
    pub workspaces: Vec<ChatGptWorkspace>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptWorkspace {
    pub id: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub is_selected: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptModel {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub context_window: Option<i64>,
    pub default_reasoning_level: Option<String>,
    pub reasoning_levels: Vec<String>,
    pub supports_reasoning_summary_parameter: bool,
    pub is_available: bool,
}

#[derive(Clone, Debug)]
pub struct NewChatGptConnection {
    pub id: String,
    pub external_user_id: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub credential_reference: String,
    pub workspaces: Vec<NewChatGptWorkspace>,
}

#[derive(Clone, Debug)]
pub struct NewChatGptWorkspace {
    pub id: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub is_selected: bool,
}

#[derive(Clone, Debug)]
pub struct ConversationRoute {
    pub connection_id: String,
    pub workspace_id: String,
    pub model_id: String,
    pub title_is_automatic: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitlePreference {
    pub connection_id: Option<String>,
    pub workspace_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StoredMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub fetched_at_unix_ms: i64,
    pub freshness: String,
    pub ordinary_usage_allowed: Option<bool>,
    pub reset_credit_count: Option<i64>,
    pub reset_credit_details_state: String,
    pub buckets: Vec<UsageBucket>,
    pub reset_credits: Vec<ResetCredit>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageBucket {
    pub limit_id: String,
    pub used_percent: Option<f64>,
    pub window_seconds: Option<i64>,
    pub reset_at_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredit {
    pub id: String,
    pub reset_type: Option<String>,
    pub status: Option<String>,
    pub granted_at_unix_ms: Option<i64>,
    pub expires_at_unix_ms: Option<i64>,
    pub title: Option<String>,
    pub description: Option<String>,
}

pub struct NewUsageSnapshot {
    pub id: String,
    pub connection_id: String,
    pub workspace_id: String,
    pub fetched_at_unix_ms: i64,
    pub freshness: String,
    pub ordinary_usage_allowed: Option<bool>,
    pub reset_credit_count: Option<i64>,
    pub reset_credit_details_state: String,
    pub buckets: Vec<UsageBucket>,
    pub reset_credits: Vec<ResetCredit>,
}

pub fn create_connection(
    storage: &AppStorage,
    connection: NewChatGptConnection,
) -> rusqlite::Result<()> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    let has_selected_connection = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM chatgpt_connections WHERE is_selected = 1)",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    let timestamp = unix_time_millis()?;

    transaction.execute(
        "INSERT INTO chatgpt_connections (
            id, external_user_id, email, display_name, plan_type,
            credential_reference, auth_status, created_at_unix_ms,
            updated_at_unix_ms, last_authenticated_at_unix_ms, is_selected
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?7, ?7, ?8)",
        params![
            connection.id,
            connection.external_user_id,
            connection.email,
            connection.display_name,
            connection.plan_type,
            connection.credential_reference,
            timestamp,
            !has_selected_connection,
        ],
    )?;

    let single_workspace = connection.workspaces.len() == 1;
    for workspace in connection.workspaces {
        transaction.execute(
            "INSERT INTO chatgpt_workspaces (
                id, connection_id, external_workspace_id, display_name,
                plan_type, is_selected, fetched_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                workspace.id,
                connection.id,
                workspace.external_id,
                workspace.display_name,
                workspace.plan_type,
                workspace.is_selected || single_workspace,
                timestamp,
            ],
        )?;
    }

    transaction.commit()
}

pub fn connection_id_for_external_user(
    storage: &AppStorage,
    external_user_id: &str,
) -> rusqlite::Result<Option<String>> {
    storage
        .connect()?
        .query_row(
            "SELECT id FROM chatgpt_connections
             WHERE external_user_id = ?1
             ORDER BY is_selected DESC, created_at_unix_ms ASC
             LIMIT 1",
            [external_user_id],
            |row| row.get(0),
        )
        .optional()
}

pub fn reconnect_connection(
    storage: &AppStorage,
    connection: NewChatGptConnection,
) -> rusqlite::Result<Option<String>> {
    let Some(external_user_id) = connection.external_user_id.as_deref() else {
        return Ok(None);
    };
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    let existing = transaction
        .query_row(
            "SELECT id, credential_reference FROM chatgpt_connections
             WHERE external_user_id = ?1
             ORDER BY is_selected DESC, created_at_unix_ms ASC
             LIMIT 1",
            [external_user_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((existing_id, previous_reference)) = existing else {
        return Ok(None);
    };
    let timestamp = unix_time_millis()?;
    transaction.execute(
        "UPDATE chatgpt_connections
         SET email = COALESCE(NULLIF(trim(?2), ''), email),
             display_name = COALESCE(NULLIF(trim(?3), ''), display_name), plan_type = ?4,
             credential_reference = ?5, auth_status = 'active',
             updated_at_unix_ms = ?6, last_authenticated_at_unix_ms = ?6
         WHERE id = ?1",
        params![
            existing_id,
            connection.email,
            connection.display_name,
            connection.plan_type,
            connection.credential_reference,
            timestamp,
        ],
    )?;

    let previous_workspace = transaction
        .query_row(
            "SELECT external_workspace_id FROM chatgpt_workspaces
             WHERE connection_id = ?1 AND is_selected = 1",
            [&existing_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let selected_workspace_id = if connection.workspaces.len() == 1 {
        connection
            .workspaces
            .first()
            .map(|workspace| workspace.external_id.clone())
    } else {
        previous_workspace
            .as_deref()
            .filter(|previous| {
                connection
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.external_id == *previous)
            })
            .map(str::to_owned)
    };
    if previous_workspace.as_deref() != selected_workspace_id.as_deref() {
        transaction.execute(
            "UPDATE chatgpt_workspaces SET is_selected = 0 WHERE connection_id = ?1",
            [&existing_id],
        )?;
    }

    for workspace in connection.workspaces {
        let workspace_id = transaction
            .query_row(
                "SELECT id FROM chatgpt_workspaces
                 WHERE connection_id = ?1 AND external_workspace_id = ?2",
                params![existing_id, workspace.external_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let is_selected = selected_workspace_id.as_deref() == Some(workspace.external_id.as_str());
        if let Some(workspace_id) = workspace_id {
            transaction.execute(
                "UPDATE chatgpt_workspaces
                 SET display_name = ?3, plan_type = ?4, is_selected = ?5,
                     fetched_at_unix_ms = ?6
                 WHERE connection_id = ?1 AND id = ?2",
                params![
                    existing_id,
                    workspace_id,
                    workspace.display_name,
                    workspace.plan_type,
                    is_selected,
                    timestamp,
                ],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO chatgpt_workspaces (
                    id, connection_id, external_workspace_id, display_name,
                    plan_type, is_selected, fetched_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    workspace.id,
                    existing_id,
                    workspace.external_id,
                    workspace.display_name,
                    workspace.plan_type,
                    is_selected,
                    timestamp,
                ],
            )?;
        }
    }
    transaction.commit()?;
    Ok(Some(previous_reference))
}

pub fn list_connections(storage: &AppStorage) -> rusqlite::Result<Vec<ChatGptConnection>> {
    let database = storage.connect()?;
    let mut statement = database.prepare(
        "SELECT id, email, display_name, plan_type, auth_status, is_selected
         FROM chatgpt_connections ORDER BY is_selected DESC, created_at_unix_ms ASC, id ASC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ChatGptConnection {
            id: row.get(0)?,
            email: row.get(1)?,
            display_name: row.get(2)?,
            plan_type: row.get(3)?,
            auth_status: row.get(4)?,
            is_selected: row.get(5)?,
            workspaces: Vec::new(),
        })
    })?;
    let mut connections = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut workspace_statement = database.prepare(
        "SELECT id, connection_id, external_workspace_id, display_name,
                plan_type, is_selected
         FROM chatgpt_workspaces
         ORDER BY connection_id, is_selected DESC, display_name COLLATE NOCASE, id",
    )?;
    let workspace_rows = workspace_statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?,
            ChatGptWorkspace {
                id: row.get(0)?,
                external_id: row.get(2)?,
                display_name: row.get(3)?,
                plan_type: row.get(4)?,
                is_selected: row.get(5)?,
            },
        ))
    })?;
    let mut workspaces_by_connection: HashMap<String, Vec<ChatGptWorkspace>> = HashMap::new();
    for row in workspace_rows {
        let (connection_id, workspace) = row?;
        workspaces_by_connection
            .entry(connection_id)
            .or_default()
            .push(workspace);
    }
    for connection in &mut connections {
        connection.workspaces = workspaces_by_connection
            .remove(&connection.id)
            .unwrap_or_default();
    }
    Ok(connections)
}

pub fn title_preference(storage: &AppStorage) -> rusqlite::Result<TitlePreference> {
    storage.connect()?.query_row(
        "SELECT connection_id, workspace_id FROM chatgpt_title_preferences
         WHERE preference_id = 1",
        [],
        |row| {
            Ok(TitlePreference {
                connection_id: row.get(0)?,
                workspace_id: row.get(1)?,
            })
        },
    )
}

pub fn set_title_preference(
    storage: &AppStorage,
    connection_id: Option<&str>,
    workspace_id: Option<&str>,
) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "UPDATE chatgpt_title_preferences
         SET connection_id = ?1, workspace_id = ?2, updated_at_unix_ms = ?3
         WHERE preference_id = 1",
        params![connection_id, workspace_id, unix_time_millis()?],
    )?;
    Ok(())
}

pub fn set_selected_connection(
    storage: &AppStorage,
    connection_id: &str,
) -> rusqlite::Result<bool> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    transaction.execute("UPDATE chatgpt_connections SET is_selected = 0", [])?;
    let changed = transaction.execute(
        "UPDATE chatgpt_connections SET is_selected = 1, updated_at_unix_ms = ?2
         WHERE id = ?1 AND auth_status = 'active'",
        params![connection_id, unix_time_millis()?],
    )?;
    if changed == 0 {
        return Ok(false);
    }
    transaction.commit()?;
    Ok(true)
}

pub fn set_selected_workspace(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
) -> rusqlite::Result<bool> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    transaction.execute(
        "UPDATE chatgpt_workspaces SET is_selected = 0 WHERE connection_id = ?1",
        [connection_id],
    )?;
    let changed = transaction.execute(
        "UPDATE chatgpt_workspaces SET is_selected = 1
         WHERE connection_id = ?1 AND id = ?2",
        params![connection_id, workspace_id],
    )?;
    if changed == 0 {
        return Ok(false);
    }
    transaction.commit()?;
    Ok(true)
}

pub fn credential_reference(
    storage: &AppStorage,
    connection_id: &str,
) -> rusqlite::Result<Option<String>> {
    storage
        .connect()?
        .query_row(
            "SELECT credential_reference FROM chatgpt_connections WHERE id = ?1",
            [connection_id],
            |row| row.get(0),
        )
        .optional()
}

pub fn delete_connection(storage: &AppStorage, connection_id: &str) -> rusqlite::Result<bool> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    let is_selected = transaction
        .query_row(
            "SELECT is_selected FROM chatgpt_connections WHERE id = ?1",
            [connection_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?;
    let Some(is_selected) = is_selected else {
        return Ok(false);
    };

    transaction.execute(
        "DELETE FROM title_generation_jobs WHERE connection_id = ?1",
        [connection_id],
    )?;
    transaction.execute(
        "DELETE FROM chatgpt_connections WHERE id = ?1",
        [connection_id],
    )?;

    if is_selected {
        let next_connection_id = transaction
            .query_row(
                "SELECT id FROM chatgpt_connections
                 WHERE auth_status = 'active'
                 ORDER BY created_at_unix_ms ASC, id ASC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(next_connection_id) = next_connection_id {
            transaction.execute(
                "UPDATE chatgpt_connections SET is_selected = 1, updated_at_unix_ms = ?2
                 WHERE id = ?1",
                params![next_connection_id, unix_time_millis()?],
            )?;
        }
    }

    transaction.commit()?;
    Ok(true)
}

pub fn connection_workspace(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
) -> rusqlite::Result<Option<ChatGptWorkspace>> {
    storage
        .connect()?
        .query_row(
            "SELECT id, external_workspace_id, display_name, plan_type, is_selected
         FROM chatgpt_workspaces WHERE connection_id = ?1 AND id = ?2",
            params![connection_id, workspace_id],
            |row| {
                Ok(ChatGptWorkspace {
                    id: row.get(0)?,
                    external_id: row.get(1)?,
                    display_name: row.get(2)?,
                    plan_type: row.get(3)?,
                    is_selected: row.get(4)?,
                })
            },
        )
        .optional()
}

pub fn workspace_external_id(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
) -> rusqlite::Result<Option<String>> {
    storage
        .connect()?
        .query_row(
            "SELECT external_workspace_id FROM chatgpt_workspaces
         WHERE connection_id = ?1 AND id = ?2",
            params![connection_id, workspace_id],
            |row| row.get(0),
        )
        .optional()
}

pub fn set_connection_auth_status(
    storage: &AppStorage,
    connection_id: &str,
    auth_status: &str,
) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "UPDATE chatgpt_connections SET auth_status = ?2, updated_at_unix_ms = ?3
         WHERE id = ?1",
        params![connection_id, auth_status, unix_time_millis()?],
    )?;
    Ok(())
}

pub fn update_profile(
    storage: &AppStorage,
    connection_id: &str,
    email: Option<&str>,
    display_name: Option<&str>,
    plan_type: Option<&str>,
) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "UPDATE chatgpt_connections SET email = COALESCE(NULLIF(trim(?2), ''), email),
            display_name = COALESCE(NULLIF(trim(?3), ''), display_name),
            plan_type = COALESCE(?4, plan_type), updated_at_unix_ms = ?5
         WHERE id = ?1",
        params![
            connection_id,
            email,
            display_name,
            plan_type,
            unix_time_millis()?
        ],
    )?;
    Ok(())
}

pub fn update_workspace_plan(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
    plan_type: Option<&str>,
) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "UPDATE chatgpt_workspaces SET plan_type = ?3
         WHERE connection_id = ?1 AND id = ?2",
        params![connection_id, workspace_id, plan_type],
    )?;
    Ok(())
}

pub fn replace_credential_reference(
    storage: &AppStorage,
    connection_id: &str,
    credential_reference: &str,
) -> rusqlite::Result<Option<String>> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    let previous = transaction
        .query_row(
            "SELECT credential_reference FROM chatgpt_connections WHERE id = ?1",
            [connection_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(previous) = previous else {
        return Ok(None);
    };
    transaction.execute(
        "UPDATE chatgpt_connections SET credential_reference = ?2,
            auth_status = 'active', updated_at_unix_ms = ?3
         WHERE id = ?1",
        params![connection_id, credential_reference, unix_time_millis()?],
    )?;
    transaction.commit()?;
    Ok(Some(previous))
}

pub fn save_models(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
    client_version: &str,
    models: &[ChatGptModel],
) -> rusqlite::Result<()> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    let fetched_at = unix_time_millis()?;
    transaction.execute(
        "DELETE FROM chatgpt_models WHERE connection_id = ?1 AND workspace_id = ?2",
        params![connection_id, workspace_id],
    )?;
    for model in models {
        let reasoning_levels = serde_json::to_string(&model.reasoning_levels)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        transaction.execute(
            "INSERT INTO chatgpt_models (
                connection_id, workspace_id, model_id, display_name, description,
                context_window, reasoning_levels_json, is_available, fetched_at_unix_ms,
                default_reasoning_level,
                supports_reasoning_summary_parameter,
                client_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                connection_id,
                workspace_id,
                model.id,
                model.display_name,
                model.description,
                model.context_window,
                reasoning_levels,
                model.is_available,
                fetched_at,
                model.default_reasoning_level,
                model.supports_reasoning_summary_parameter,
                client_version,
            ],
        )?;
    }
    transaction.commit()
}

pub fn list_models(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
) -> rusqlite::Result<Vec<ChatGptModel>> {
    let database = storage.connect()?;
    let mut statement = database.prepare(
        "SELECT model_id, display_name, description, context_window,
                reasoning_levels_json, is_available, default_reasoning_level,
                supports_reasoning_summary_parameter
         FROM chatgpt_models
         WHERE connection_id = ?1 AND workspace_id = ?2
         ORDER BY is_available DESC, model_id COLLATE NOCASE",
    )?;
    let rows = statement.query_map(params![connection_id, workspace_id], |row| {
        let levels = row.get::<_, String>(4)?;
        let reasoning_levels = parse_reasoning_levels(&levels, 4)?;
        Ok(ChatGptModel {
            id: row.get(0)?,
            display_name: row.get(1)?,
            description: row.get(2)?,
            context_window: row.get(3)?,
            reasoning_levels,
            is_available: row.get(5)?,
            default_reasoning_level: row.get(6)?,
            supports_reasoning_summary_parameter: row.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn conversation_route(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<Option<ConversationRoute>> {
    let route = storage
        .connect()?
        .query_row(
            "SELECT connection_id, workspace_id, model_id, title_source
         FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((connection_id, workspace_id, model_id, title_source)) = route else {
        return Ok(None);
    };
    match (connection_id, workspace_id, model_id) {
        (Some(connection_id), Some(workspace_id), Some(model_id)) => Ok(Some(ConversationRoute {
            connection_id,
            workspace_id,
            model_id,
            title_is_automatic: title_source == "automatic",
        })),
        (None, None, None) => Ok(None),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

pub fn conversation_messages(
    storage: &AppStorage,
    conversation_id: &str,
) -> rusqlite::Result<Vec<StoredMessage>> {
    let database = storage.connect()?;
    let mut statement = database.prepare(
        "SELECT role, content FROM messages
         WHERE conversation_id = ?1 AND role IN ('user', 'assistant')
         ORDER BY created_at ASC, id ASC",
    )?;
    let rows = statement.query_map([conversation_id], |row| {
        Ok(StoredMessage {
            role: row.get(0)?,
            content: row.get(1)?,
        })
    })?;
    rows.collect()
}

pub fn apply_generated_title(
    storage: &AppStorage,
    conversation_id: &str,
    title: &str,
) -> rusqlite::Result<bool> {
    let changed = storage.connect()?.execute(
        "UPDATE conversations SET title = ?2, updated_at = MAX(updated_at, ?3)
         WHERE id = ?1 AND title_source = 'automatic'",
        params![conversation_id, title, unix_time_millis()?],
    )?;
    Ok(changed == 1)
}

pub fn save_assistant_message(
    storage: &AppStorage,
    conversation_id: &str,
    message_id: &str,
    content: &str,
    status: &str,
    created_at_unix_ms: i64,
    output_tokens: Option<i64>,
    tokens_per_second: Option<f64>,
    elapsed_microseconds: Option<i64>,
) -> rusqlite::Result<()> {
    let database = storage.connect()?;
    database.execute(
        "INSERT INTO messages (
            id, conversation_id, role, content, created_at, output_tokens,
            tokens_per_second, elapsed_microseconds, status
         ) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(conversation_id, id) DO UPDATE SET
            content = excluded.content,
            output_tokens = excluded.output_tokens,
            tokens_per_second = excluded.tokens_per_second,
            elapsed_microseconds = excluded.elapsed_microseconds,
            status = excluded.status",
        params![
            message_id,
            conversation_id,
            content,
            created_at_unix_ms,
            output_tokens,
            tokens_per_second,
            elapsed_microseconds,
            status,
        ],
    )?;
    database.execute(
        "UPDATE conversations SET updated_at = MAX(updated_at, ?2) WHERE id = ?1",
        params![conversation_id, created_at_unix_ms],
    )?;
    Ok(())
}

pub fn create_title_job(
    storage: &AppStorage,
    job_id: &str,
    conversation_id: &str,
    connection_id: &str,
    workspace_id: &str,
    model_id: Option<&str>,
    status: &str,
    reason_code: Option<&str>,
) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "INSERT INTO title_generation_jobs (
            id, conversation_id, connection_id, workspace_id, model_id,
            status, reason_code, created_at_unix_ms, completed_at_unix_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            job_id,
            conversation_id,
            connection_id,
            workspace_id,
            model_id,
            status,
            reason_code,
            unix_time_millis()?,
            (status != "queued").then(unix_time_millis).transpose()?,
        ],
    )?;
    Ok(())
}

pub fn finish_title_job(
    storage: &AppStorage,
    job_id: &str,
    status: &str,
    reason_code: Option<&str>,
) -> rusqlite::Result<()> {
    storage.connect()?.execute(
        "UPDATE title_generation_jobs SET status = ?2, reason_code = ?3,
            completed_at_unix_ms = ?4, attempt_count = attempt_count + 1
         WHERE id = ?1",
        params![job_id, status, reason_code, unix_time_millis()?],
    )?;
    Ok(())
}

pub fn save_usage_snapshot(
    storage: &AppStorage,
    snapshot: NewUsageSnapshot,
) -> rusqlite::Result<()> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    transaction.execute(
        "INSERT INTO chatgpt_usage_snapshots (
            id, connection_id, workspace_id, fetched_at_unix_ms, freshness,
            ordinary_usage_allowed, reset_credit_count, reset_credit_details_state
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            snapshot.id,
            snapshot.connection_id,
            snapshot.workspace_id,
            snapshot.fetched_at_unix_ms,
            snapshot.freshness,
            snapshot.ordinary_usage_allowed,
            snapshot.reset_credit_count,
            snapshot.reset_credit_details_state,
        ],
    )?;
    for bucket in snapshot.buckets {
        transaction.execute(
            "INSERT INTO chatgpt_usage_buckets (
                snapshot_id, limit_id, used_percent, window_seconds, reset_at_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                snapshot.id,
                bucket.limit_id,
                bucket.used_percent,
                bucket.window_seconds,
                bucket.reset_at_unix_ms,
            ],
        )?;
    }
    for credit in snapshot.reset_credits {
        transaction.execute(
            "INSERT INTO chatgpt_reset_credits (
                snapshot_id, external_credit_id, reset_type, status,
                granted_at_unix_ms, expires_at_unix_ms, title, description
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                snapshot.id,
                credit.id,
                credit.reset_type,
                credit.status,
                credit.granted_at_unix_ms,
                credit.expires_at_unix_ms,
                credit.title,
                credit.description,
            ],
        )?;
    }
    transaction.commit()
}

pub fn latest_usage_snapshot(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
) -> rusqlite::Result<Option<UsageSnapshot>> {
    let database = storage.connect()?;
    let snapshot = database
        .query_row(
            "SELECT id, fetched_at_unix_ms, freshness, ordinary_usage_allowed,
                reset_credit_count, reset_credit_details_state
         FROM chatgpt_usage_snapshots
         WHERE connection_id = ?1 AND workspace_id = ?2
         ORDER BY fetched_at_unix_ms DESC LIMIT 1",
            params![connection_id, workspace_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    UsageSnapshot {
                        fetched_at_unix_ms: row.get(1)?,
                        freshness: row.get(2)?,
                        ordinary_usage_allowed: row.get(3)?,
                        reset_credit_count: row.get(4)?,
                        reset_credit_details_state: row.get(5)?,
                        buckets: Vec::new(),
                        reset_credits: Vec::new(),
                    },
                ))
            },
        )
        .optional()?;
    let Some((snapshot_id, mut snapshot)) = snapshot else {
        return Ok(None);
    };
    let mut bucket_statement = database.prepare(
        "SELECT limit_id, used_percent, window_seconds, reset_at_unix_ms
         FROM chatgpt_usage_buckets WHERE snapshot_id = ?1 ORDER BY limit_id",
    )?;
    snapshot.buckets = bucket_statement
        .query_map([&snapshot_id], |row| {
            Ok(UsageBucket {
                limit_id: row.get(0)?,
                used_percent: row.get(1)?,
                window_seconds: row.get(2)?,
                reset_at_unix_ms: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut credit_statement = database.prepare(
        "SELECT external_credit_id, reset_type, status, granted_at_unix_ms,
                expires_at_unix_ms, title, description
         FROM chatgpt_reset_credits WHERE snapshot_id = ?1 ORDER BY granted_at_unix_ms DESC",
    )?;
    snapshot.reset_credits = credit_statement
        .query_map([&snapshot_id], |row| {
            Ok(ResetCredit {
                id: row.get(0)?,
                reset_type: row.get(1)?,
                status: row.get(2)?,
                granted_at_unix_ms: row.get(3)?,
                expires_at_unix_ms: row.get(4)?,
                title: row.get(5)?,
                description: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some(snapshot))
}

pub fn selected_model(
    storage: &AppStorage,
    connection_id: &str,
    workspace_id: &str,
    model_id: &str,
) -> rusqlite::Result<Option<ChatGptModel>> {
    let database = storage.connect()?;
    database
        .query_row(
            "SELECT model_id, display_name, description, context_window,
                reasoning_levels_json, is_available, default_reasoning_level,
                supports_reasoning_summary_parameter
         FROM chatgpt_models
         WHERE connection_id = ?1 AND workspace_id = ?2 AND model_id = ?3",
            params![connection_id, workspace_id, model_id],
            |row| {
                let levels = row.get::<_, String>(4)?;
                Ok(ChatGptModel {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    description: row.get(2)?,
                    context_window: row.get(3)?,
                    reasoning_levels: parse_reasoning_levels(&levels, 4)?,
                    is_available: row.get(5)?,
                    default_reasoning_level: row.get(6)?,
                    supports_reasoning_summary_parameter: row.get(7)?,
                })
            },
        )
        .optional()
}

fn parse_reasoning_levels(value: &str, column_index: usize) -> rusqlite::Result<Vec<String>> {
    serde_json::from_str::<Vec<String>>(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Text, Box::new(error))
    })
}

fn unix_time_millis() -> rusqlite::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    i64::try_from(elapsed.as_millis())
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}
