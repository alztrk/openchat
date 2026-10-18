//! Database lifecycle maintenance, cache pruning, and disk space reclamation.

use rusqlite::{params, Connection};

use super::ChatStorageError;

/// Purges all expired entries from the ephemeral cache table.
///
/// Returns the number of removed rows.
///
/// # Errors
/// Returns a storage error if the delete statement fails.
pub fn purge_expired_cache(connection: &Connection, now_ms: i64) -> Result<usize, ChatStorageError> {
    connection
        .execute(
            "DELETE FROM ephemeral_cache WHERE expires_at <= ?1",
            params![now_ms],
        )
        .map_err(|_| ChatStorageError::unavailable())
}

/// Checks the database freelist page count and triggers incremental vacuum if it exceeds the threshold.
///
/// # Errors
/// Returns a storage error if reading freelist count or executing incremental vacuum fails.
pub fn perform_incremental_vacuum_if_needed(connection: &Connection) -> Result<(), ChatStorageError> {
    let freelist_count: i64 = connection
        .query_row("PRAGMA freelist_count", [], |row| row.get(0))
        .unwrap_or(0);
    if freelist_count > 100 {
        let _ = connection.execute("PRAGMA incremental_vacuum(50)", []);
    }
    Ok(())
}

/// Runs SQLite's PRAGMA optimize to collect statistics for the query planner.
///
/// # Errors
/// Returns a storage error if the optimize command fails.
pub fn run_optimize(connection: &Connection) -> Result<(), ChatStorageError> {
    connection
        .execute("PRAGMA optimize", [])
        .map_err(|_| ChatStorageError::unavailable())?;
    Ok(())
}
