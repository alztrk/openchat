use rusqlite::{OptionalExtension, params};

use crate::storage::AppStorage;

use super::{NewUsageSnapshot, ResetCredit, UsageBucket, UsageSnapshot};

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
