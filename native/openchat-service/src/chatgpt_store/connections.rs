use std::collections::HashMap;

use rusqlite::{OptionalExtension, params};

use crate::storage::AppStorage;

use super::{
    ChatGptConnection, ChatGptWorkspace, NewChatGptConnection, TitlePreference, unix_time_millis,
};

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
