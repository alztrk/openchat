//! Schema definitions and migration management for the SQLite chat storage.

use rusqlite::Connection;

pub const CHAT_SCHEMA_VERSION: i64 = 22;

/// Applies forward schema migrations to the chat database up to `CHAT_SCHEMA_VERSION`.
///
/// # Errors
/// Returns an error if SQLite fails to apply any migration statement or update the schema version.
pub fn migrate_chat_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let mut version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > CHAT_SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                is_pinned INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER,
                output_tokens INTEGER,
                tokens_per_second REAL,
                elapsed_microseconds INTEGER,
                status TEXT NOT NULL,
                PRIMARY KEY (conversation_id, id)
             );",
        )?;
        transaction.pragma_update(None, "user_version", 1)?;
        transaction.commit()?;
        version = 1;
    }

    let transaction = connection.transaction()?;
    if version < 2 {
        add_column(
            &transaction,
            "conversations",
            "title_source",
            "TEXT NOT NULL DEFAULT 'automatic'",
        )?;
        add_column(&transaction, "conversations", "connection_id", "TEXT")?;
        add_column(&transaction, "conversations", "workspace_id", "TEXT")?;
        transaction.pragma_update(None, "user_version", 2)?;
        version = 2;
    }
    if version < 3 {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                folder_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );",
        )?;
        add_column(
            &transaction,
            "conversations",
            "project_id",
            "TEXT REFERENCES projects(id) ON DELETE SET NULL",
        )?;
        transaction.pragma_update(None, "user_version", 3)?;
        version = 3;
    }
    if version < 4 {
        add_column(
            &transaction,
            "messages",
            "reasoning_summaries",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 4)?;
        version = 4;
    }
    if version < 5 {
        add_column(&transaction, "conversations", "provider_id", "TEXT")?;
        transaction.pragma_update(None, "user_version", 5)?;
        version = 5;
    }
    if version < 6 {
        create_model_favorites(&transaction)?;
        transaction.pragma_update(None, "user_version", 6)?;
        version = 6;
    }
    if version < 7 {
        add_column(
            &transaction,
            "messages",
            "tool_activities",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 7)?;
        version = 7;
    }
    if version < 8 {
        add_column(
            &transaction,
            "conversations",
            "api_key_connection_id",
            "TEXT",
        )?;
        add_column(
            &transaction,
            "model_favorites",
            "source_connection_id",
            "TEXT",
        )?;
        transaction.pragma_update(None, "user_version", 8)?;
        version = 8;
    }
    if version < 9 {
        transaction.execute_batch(
            "UPDATE conversations SET title_source = 'manual' WHERE title_source = 'user';
             UPDATE messages SET status = 'completed' WHERE status = 'complete';",
        )?;
        transaction.pragma_update(None, "user_version", 9)?;
        version = 9;
    }
    if version < 10 {
        add_column(&transaction, "messages", "failure_code", "TEXT")?;
        transaction.pragma_update(None, "user_version", 10)?;
        version = 10;
    }
    if version < 11 {
        add_column(
            &transaction,
            "conversations",
            "is_archived",
            "INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1))",
        )?;
        transaction.pragma_update(None, "user_version", 11)?;
        version = 11;
    }
    if version < 12 {
        add_column(&transaction, "messages", "provider_id", "TEXT")?;
        add_column(&transaction, "messages", "model_id", "TEXT")?;
        add_column(
            &transaction,
            "messages",
            "citation_sources",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 12)?;
        version = 12;
    }
    if version < 13 {
        add_column(
            &transaction,
            "conversations",
            "tags",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 13)?;
        version = 13;
    }
    if version < 14 {
        add_column(
            &transaction,
            "conversations",
            "is_bookmarked",
            "INTEGER NOT NULL DEFAULT 0 CHECK (is_bookmarked IN (0, 1))",
        )?;
        transaction.pragma_update(None, "user_version", 14)?;
        version = 14;
    }
    if version < 15 {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS saved_outputs (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                conversation_title TEXT NOT NULL,
                content TEXT NOT NULL,
                provider_id TEXT,
                model_id TEXT,
                saved_at INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, message_id)
             );",
        )?;
        add_column(
            &transaction,
            "conversations",
            "product_workspace_id",
            "TEXT REFERENCES workspaces(id) ON DELETE SET NULL",
        )?;
        transaction.pragma_update(None, "user_version", 15)?;
        version = 15;
    }
    if version < 16 {
        add_column(
            &transaction,
            "saved_outputs",
            "selected_version",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS saved_output_versions (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                version_number INTEGER NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, message_id, version_number)
             );
             INSERT INTO saved_output_versions
                (conversation_id, message_id, version_number, content, created_at)
             SELECT conversation_id, message_id, 1, content, saved_at FROM saved_outputs;",
        )?;
        transaction.pragma_update(None, "user_version", 16)?;
        version = 16;
    }
    if version < 17 {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS attachment_text_index (
                conversation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                attachment_id TEXT NOT NULL,
                attachment_name TEXT NOT NULL,
                content TEXT NOT NULL,
                indexed_at INTEGER NOT NULL,
                PRIMARY KEY (conversation_id, message_id, attachment_id)
             );",
        )?;
        transaction.pragma_update(None, "user_version", 17)?;
        version = 17;
    }
    if version < 18 {
        add_column(
            &transaction,
            "messages",
            "personal_memory_fact_ids",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        transaction.pragma_update(None, "user_version", 18)?;
        version = 18;
    }
    if version < 19 {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY NOT NULL,
                value_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS personal_memory_facts (
                id TEXT PRIMARY KEY NOT NULL,
                fact_text TEXT NOT NULL,
                category TEXT,
                source_conversation_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_personal_memory_facts_category
                ON personal_memory_facts(category);
             CREATE TABLE IF NOT EXISTS secure_vault (
                key TEXT PRIMARY KEY NOT NULL,
                ciphertext BLOB NOT NULL,
                description TEXT,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS ephemeral_cache (
                key TEXT PRIMARY KEY NOT NULL,
                value_json TEXT NOT NULL,
                expires_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_ephemeral_cache_expires
                ON ephemeral_cache(expires_at);",
        )?;
        transaction.pragma_update(None, "user_version", 19)?;
        version = 19;
    }
    if version < 20 {
        transaction.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_conversations_sort_list
                ON conversations(is_bookmarked DESC, is_pinned DESC, updated_at DESC, id ASC);
             CREATE INDEX IF NOT EXISTS idx_messages_conversation_sort
                ON messages(conversation_id, created_at ASC, id ASC);",
        )?;
        transaction.pragma_update(None, "user_version", 20)?;
    }
    if version < 21 {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_workspace_roots (
                root_id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                root_path TEXT NOT NULL,
                position INTEGER NOT NULL CHECK (position >= 0),
                created_at INTEGER NOT NULL,
                UNIQUE (project_id, position)
             );
             CREATE INDEX IF NOT EXISTS idx_project_workspace_roots_project
                ON project_workspace_roots(project_id, position);
             INSERT OR IGNORE INTO project_workspace_roots
                (root_id, project_id, root_path, position, created_at)
             SELECT lower(hex(randomblob(16))), id, folder_path, 0, created_at
             FROM projects;",
        )?;
        transaction.pragma_update(None, "user_version", 21)?;
        version = 21;
    }
    if version < 22 {
        add_column(&transaction, "conversations", "agent_json", "TEXT")?;
        transaction.pragma_update(None, "user_version", 22)?;
    }
    transaction.commit()
}

/// Adds a column to a table if it does not already exist.
pub fn add_column(
    connection: &Connection,
    table: &str,
    name: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == name {
            return Ok(());
        }
    }
    connection.execute_batch(&format!(
        "ALTER TABLE \"{table}\" ADD COLUMN \"{name}\" {definition}"
    ))
}

/// Creates the model_favorites table if missing.
pub fn create_model_favorites(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS model_favorites (
            provider_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            favorited_at INTEGER NOT NULL,
            PRIMARY KEY (provider_id, model_id)
         );",
    )
}
