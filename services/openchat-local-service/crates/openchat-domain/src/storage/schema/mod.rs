//! Defines the versioned local SQLite schema and migration operations.
//!
//! Schema updates keep persisted conversations and derived indexes aligned; migrations run
//! through the storage layer's integrity and backup guards.

use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const SCHEMA_VERSION: i64 = 57;
pub(super) const INITIAL_SCHEMA_VERSION: i64 = 2;
pub mod chat_migrations;

pub(super) mod agent_migrations;
pub(super) mod legacy_migrations;
pub(super) mod repair;
pub(super) mod triggers;

#[cfg(test)]
mod tests;

pub(super) fn unix_time_millis() -> rusqlite::Result<i64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?
        .as_millis();
    i64::try_from(millis).map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

pub(super) fn initialize_schema(
    connection: &Connection,
    target_version: i64,
) -> rusqlite::Result<i64> {
    super::tool_index_redaction::register_sqlite_function(connection)?;
    if !(1..=SCHEMA_VERSION).contains(&target_version) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let migrations_table_exists =
        repair::schema_table_exists(connection, "openchat_backend_migrations")?;
    let mut current_version = if migrations_table_exists {
        connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM openchat_backend_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )?
    } else {
        0
    };
    if current_version > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if current_version >= target_version {
        if target_version >= SCHEMA_VERSION && current_version >= 13 {
            repair::repair_missing_conversation_context_state(connection, current_version)?;
            let indexes_repaired =
                repair::repair_missing_memory_indexes_and_triggers(connection, current_version)?;
            if indexes_repaired {
                let transaction = connection.unchecked_transaction()?;
                transaction.execute_batch(triggers::REDACTED_TOOL_INDEX_TRIGGERS)?;
                transaction.execute_batch(triggers::MEMORY_INDEX_TRIGGERS)?;
                transaction.commit()?;
            }
        }
        return Ok(current_version);
    }

    if current_version >= 11 {
        repair::repair_missing_conversation_context_state(connection, current_version)?;
    }

    if current_version >= 13 {
        repair::repair_missing_memory_indexes_and_triggers(connection, current_version)?;
    }

    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS openchat_backend_migrations (
            version INTEGER PRIMARY KEY,
            applied_at_unix_ms INTEGER NOT NULL
        );",
    )?;

    current_version =
        legacy_migrations::apply_legacy_migrations(connection, current_version, target_version)?;
    if current_version >= target_version {
        return Ok(current_version);
    }

    current_version =
        agent_migrations::apply_agent_migrations(connection, current_version, target_version)?;
    Ok(current_version)
}
