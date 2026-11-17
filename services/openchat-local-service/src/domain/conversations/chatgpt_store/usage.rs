//! Persists and reads ChatGPT usage and quota snapshots.
//!
//! Snapshots are keyed by provider account and preserve the provider's reset
//! timestamps and credit buckets for the usage interface.

use rusqlite::{OptionalExtension, params};

use crate::storage::AppStorage;

use super::{NewUsageSnapshot, ResetCredit, UsageBucket, UsageCredits, UsageSnapshot};

pub fn save_usage_snapshot(
    storage: &AppStorage,
    snapshot: NewUsageSnapshot,
) -> rusqlite::Result<()> {
    let mut database = storage.connect()?;
    let transaction = database.transaction()?;
    transaction.execute(
        "INSERT INTO chatgpt_usage_snapshots (
            id, connection_id, workspace_id, fetched_at_unix_ms, freshness,
            ordinary_usage_allowed, reset_credit_count, reset_credit_details_state,
            has_credits, unlimited_credits, credit_balance
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            snapshot.id,
            snapshot.connection_id,
            snapshot.workspace_id,
            snapshot.fetched_at_unix_ms,
            snapshot.freshness,
            snapshot.ordinary_usage_allowed,
            snapshot.reset_credit_count,
            snapshot.reset_credit_details_state,
            snapshot.credits.as_ref().map(|credits| credits.has_credits),
            snapshot.credits.as_ref().map(|credits| credits.unlimited),
            snapshot
                .credits
                .as_ref()
                .and_then(|credits| credits.balance),
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
                reset_credit_count, reset_credit_details_state,
                has_credits, unlimited_credits, credit_balance
         FROM chatgpt_usage_snapshots
         WHERE connection_id = ?1 AND workspace_id = ?2
         ORDER BY fetched_at_unix_ms DESC LIMIT 1",
            params![connection_id, workspace_id],
            |row| {
                let credit_balance = row.get(8)?;
                Ok((
                    row.get::<_, String>(0)?,
                    UsageSnapshot {
                        fetched_at_unix_ms: row.get(1)?,
                        freshness: row.get(2)?,
                        ordinary_usage_allowed: row.get(3)?,
                        reset_credit_count: row.get(4)?,
                        reset_credit_details_state: row.get(5)?,
                        credits: row.get::<_, Option<bool>>(6)?.zip(row.get(7)?).map(
                            |(has_credits, unlimited)| UsageCredits {
                                has_credits,
                                unlimited,
                                balance: credit_balance,
                            },
                        ),
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-chatgpt-usage-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("create usage test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn usage_snapshot_round_trip_preserves_credits_and_column_mapping() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open usage storage");
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("initialize complete usage schema");
        storage
            .connect()
            .expect("connect usage storage")
            .execute_batch(
                "INSERT INTO chatgpt_connections (
                    id, credential_reference, auth_status, created_at_unix_ms, updated_at_unix_ms
                 ) VALUES ('account', 'fixture-reference', 'active', 1, 1);
                 INSERT INTO chatgpt_workspaces (
                    id, connection_id, external_workspace_id, fetched_at_unix_ms
                 ) VALUES ('workspace', 'account', 'external-workspace', 1);",
            )
            .expect("insert usage account and workspace");

        for (index, credits) in [
            None,
            Some(UsageCredits {
                has_credits: true,
                unlimited: false,
                balance: Some(12.5),
            }),
            Some(UsageCredits {
                has_credits: false,
                unlimited: true,
                balance: None,
            }),
            Some(UsageCredits {
                has_credits: true,
                unlimited: false,
                balance: Some(0.0),
            }),
            None,
        ]
        .into_iter()
        .enumerate()
        {
            let fetched_at_unix_ms = 100 + index as i64;
            let expected_credits =
                serde_json::to_value(&credits).expect("serialize expected credits");
            save_usage_snapshot(
                &storage,
                NewUsageSnapshot {
                    id: format!("snapshot-{index}"),
                    connection_id: "account".to_owned(),
                    workspace_id: "workspace".to_owned(),
                    fetched_at_unix_ms,
                    freshness: "current".to_owned(),
                    ordinary_usage_allowed: Some(false),
                    reset_credit_count: Some(2),
                    credits,
                    reset_credit_details_state: "available".to_owned(),
                    buckets: vec![UsageBucket {
                        limit_id: "ordinary:primary".to_owned(),
                        used_percent: Some(37.5),
                        window_seconds: Some(18_000),
                        reset_at_unix_ms: Some(1_900_000_000_000),
                    }],
                    reset_credits: vec![ResetCredit {
                        id: "reset-credit".to_owned(),
                        reset_type: Some("ordinary".to_owned()),
                        status: Some("available".to_owned()),
                        granted_at_unix_ms: Some(10),
                        expires_at_unix_ms: Some(20),
                        title: Some("Reset usage".to_owned()),
                        description: Some("Reset credit description".to_owned()),
                    }],
                },
            )
            .expect("save usage snapshot");

            let loaded = latest_usage_snapshot(&storage, "account", "workspace")
                .expect("load latest usage snapshot")
                .expect("saved usage snapshot");
            assert_eq!(
                serde_json::to_value(loaded).expect("serialize stored snapshot"),
                json!({
                    "fetchedAtUnixMs": fetched_at_unix_ms,
                    "freshness": "current",
                    "ordinaryUsageAllowed": false,
                    "resetCreditCount": 2,
                    "credits": expected_credits,
                    "resetCreditDetailsState": "available",
                    "buckets": [{
                        "limitId": "ordinary:primary", "usedPercent": 37.5,
                        "windowSeconds": 18_000, "resetAtUnixMs": 1_900_000_000_000_i64
                    }],
                    "resetCredits": [{
                        "id": "reset-credit", "resetType": "ordinary", "status": "available",
                        "grantedAtUnixMs": 10, "expiresAtUnixMs": 20,
                        "title": "Reset usage", "description": "Reset credit description"
                    }]
                })
            );
        }
        assert!(
            latest_usage_snapshot(&storage, "account", "missing-workspace")
                .expect("load unknown workspace usage")
                .is_none()
        );
    }

    #[test]
    fn usage_credits_serialize_with_camel_case_keys() {
        for balance in [Some(12.5), None] {
            assert_eq!(
                serde_json::to_value(UsageCredits {
                    has_credits: true,
                    unlimited: false,
                    balance,
                })
                .expect("serialize usage credits"),
                json!({
                    "hasCredits": true, "unlimited": false, "balance": balance
                })
            );
        }
    }
}
