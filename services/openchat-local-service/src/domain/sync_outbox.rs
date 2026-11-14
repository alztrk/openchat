//! Bounded encrypted-envelope delivery queue for a future hosted sync transport.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{protocol::ServiceError, storage::AppStorage};

const MAXIMUM_ENVELOPE_BYTES: usize = 32 * 1024 * 1024;
const MAXIMUM_PENDING_EVENTS: i64 = 1_000;
const MAXIMUM_PENDING_BYTES: i64 = 64 * 1024 * 1024;
const MAXIMUM_RECEIPTS: i64 = 8_192;
const MAXIMUM_ATTEMPTS: i64 = 8;
const MAXIMUM_BATCH: u64 = 20;
const LEASE_DURATION_MS: i64 = 30_000;
const RECEIPT_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;
const INITIAL_BACKOFF_MS: i64 = 1_000;
const MAXIMUM_BACKOFF_MS: i64 = 15 * 60 * 1_000;

#[derive(Debug, Eq, PartialEq, Serialize)]
/// Result of inserting an idempotent encrypted sync event.
#[serde(rename_all = "camelCase")]
pub(crate) enum EnqueueResult {
    /// A new event was durably queued.
    Queued,
    /// The same event and envelope were already queued or acknowledged.
    Duplicate,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// One leased opaque envelope ready for a relay upload.
pub(crate) struct ClaimedEnvelope {
    event_id: String,
    lease_id: String,
    envelope: Vec<u8>,
    attempt_count: i64,
}

pub(crate) fn enqueue(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
    envelope: &[u8],
) -> Result<EnqueueResult, ServiceError> {
    enqueue_at(
        storage,
        account_id,
        device_id,
        event_id,
        envelope,
        unix_time_millis()?,
    )
}

fn enqueue_at(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
    envelope: &[u8],
    now: i64,
) -> Result<EnqueueResult, ServiceError> {
    if envelope.is_empty() || envelope.len() > MAXIMUM_ENVELOPE_BYTES || now < 0 {
        return Err(invalid_request());
    }
    let account_id = account_id.to_string();
    let device_id = device_id.to_string();
    let event_id = event_id.to_string();
    let digest = Sha256::digest(envelope);
    let mut connection = storage.connect().map_err(storage_failure)?;
    let transaction = connection.transaction().map_err(storage_failure)?;
    transaction
        .execute(
            "DELETE FROM sync_outbox_receipts
             WHERE account_id = ?1 AND device_id = ?2 AND expires_at_unix_ms <= ?3",
            params![account_id, device_id, now],
        )
        .map_err(storage_failure)?;

    let receipt = transaction
        .query_row(
            "SELECT envelope_sha256 FROM sync_outbox_receipts
             WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3",
            params![account_id, device_id, event_id],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(storage_failure)?;
    if let Some(receipt) = receipt {
        if receipt.as_slice() != digest.as_slice() {
            return Err(idempotency_conflict());
        }
        transaction.commit().map_err(storage_failure)?;
        return Ok(EnqueueResult::Duplicate);
    }

    let existing = transaction
        .query_row(
            "SELECT envelope FROM sync_outbox
             WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3",
            params![account_id, device_id, event_id],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(storage_failure)?;
    if let Some(existing) = existing {
        if existing.as_slice() != envelope {
            return Err(idempotency_conflict());
        }
        transaction.commit().map_err(storage_failure)?;
        return Ok(EnqueueResult::Duplicate);
    }

    let (event_count, queued_bytes) = transaction
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(envelope)), 0) FROM sync_outbox
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(storage_failure)?;
    let receipt_count = transaction
        .query_row(
            "SELECT COUNT(*) FROM sync_outbox_receipts
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(storage_failure)?;
    if event_count >= MAXIMUM_PENDING_EVENTS
        || queued_bytes.saturating_add(envelope.len() as i64) > MAXIMUM_PENDING_BYTES
        || receipt_count >= MAXIMUM_RECEIPTS
    {
        return Err(queue_full());
    }
    transaction
        .execute(
            "INSERT INTO sync_outbox
                (account_id, device_id, event_id, envelope, created_at_unix_ms,
                 attempt_count, next_attempt_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?5)",
            params![account_id, device_id, event_id, envelope, now],
        )
        .map_err(storage_failure)?;
    transaction.commit().map_err(storage_failure)?;
    Ok(EnqueueResult::Queued)
}

pub(crate) fn claim_due(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    limit: u64,
) -> Result<Vec<ClaimedEnvelope>, ServiceError> {
    if !(1..=MAXIMUM_BATCH).contains(&limit) {
        return Err(invalid_request());
    }
    claim_due_at(storage, account_id, device_id, limit, unix_time_millis()?)
}

fn claim_due_at(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    limit: u64,
    now: i64,
) -> Result<Vec<ClaimedEnvelope>, ServiceError> {
    let lease_until = now
        .checked_add(LEASE_DURATION_MS)
        .ok_or_else(invalid_request)?;
    let account_id = account_id.to_string();
    let device_id = device_id.to_string();
    let mut connection = storage.connect().map_err(storage_failure)?;
    let transaction = connection.transaction().map_err(storage_failure)?;
    let mut rows = transaction
        .query(
            "SELECT event_id, envelope, attempt_count FROM sync_outbox
             WHERE account_id = ?1 AND device_id = ?2 AND exhausted = 0
               AND next_attempt_at_unix_ms <= ?3
               AND (lease_until_unix_ms IS NULL OR lease_until_unix_ms <= ?3)
             ORDER BY created_at_unix_ms, event_id LIMIT ?4",
            params![account_id, device_id, now, limit as i64],
        )
        .map_err(storage_failure)?;
    let mut claimed = Vec::with_capacity(limit as usize);
    while let Some(row) = rows.next().map_err(storage_failure)? {
        let event_id: String = row.get(0).map_err(storage_failure)?;
        let lease_id = Uuid::new_v4().to_string();
        transaction
            .execute(
                "UPDATE sync_outbox SET lease_id = ?4, lease_until_unix_ms = ?5
                 WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3
                   AND (lease_until_unix_ms IS NULL OR lease_until_unix_ms <= ?6)",
                params![account_id, device_id, event_id, lease_id, lease_until, now],
            )
            .map_err(storage_failure)?;
        claimed.push(ClaimedEnvelope {
            event_id,
            lease_id,
            envelope: row.get(1).map_err(storage_failure)?,
            attempt_count: row.get(2).map_err(storage_failure)?,
        });
    }
    transaction.commit().map_err(storage_failure)?;
    Ok(claimed)
}

pub(crate) fn record_retry(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
    lease_id: Uuid,
) -> Result<(), ServiceError> {
    record_retry_at(
        storage,
        account_id,
        device_id,
        event_id,
        lease_id,
        unix_time_millis()?,
    )
}

pub(crate) fn requeue_exhausted(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
) -> Result<bool, ServiceError> {
    requeue_exhausted_at(
        storage,
        account_id,
        device_id,
        event_id,
        unix_time_millis()?,
    )
}

fn requeue_exhausted_at(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
    now: i64,
) -> Result<bool, ServiceError> {
    let connection = storage.connect().map_err(storage_failure)?;
    let updated = connection
        .execute(
            "UPDATE sync_outbox SET attempt_count = 0, next_attempt_at_unix_ms = ?4,
                lease_until_unix_ms = NULL, exhausted = 0
             WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3 AND exhausted = 1",
            params![
                account_id.to_string(),
                device_id.to_string(),
                event_id.to_string(),
                now
            ],
        )
        .map_err(storage_failure)?;
    Ok(updated > 0)
}

fn record_retry_at(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
    lease_id: Uuid,
    now: i64,
) -> Result<(), ServiceError> {
    let account_id = account_id.to_string();
    let device_id = device_id.to_string();
    let event_id = event_id.to_string();
    let mut connection = storage.connect().map_err(storage_failure)?;
    let transaction = connection.transaction().map_err(storage_failure)?;
    let attempt_count = transaction
        .query_row(
            "SELECT attempt_count FROM sync_outbox
             WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3
               AND lease_id = ?4 AND lease_until_unix_ms > ?5",
            params![account_id, device_id, event_id, lease_id.to_string(), now],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage_failure)?
        .ok_or_else(|| outbox_conflict())?;
    let next_attempt = attempt_count.checked_add(1).ok_or_else(invalid_request)?;
    let exhausted = next_attempt >= MAXIMUM_ATTEMPTS;
    let delay = INITIAL_BACKOFF_MS
        .saturating_mul(
            1_i64
                .checked_shl(u32::try_from(next_attempt - 1).unwrap_or(63))
                .unwrap_or(i64::MAX),
        )
        .min(MAXIMUM_BACKOFF_MS);
    let next_at = now.checked_add(delay).ok_or_else(invalid_request)?;
    transaction
        .execute(
            "UPDATE sync_outbox SET attempt_count = ?4, next_attempt_at_unix_ms = ?5,
                lease_until_unix_ms = NULL, lease_id = NULL, exhausted = ?6
             WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3",
            params![
                account_id,
                device_id,
                event_id,
                next_attempt,
                next_at,
                exhausted
            ],
        )
        .map_err(storage_failure)?;
    transaction.commit().map_err(storage_failure)
}

pub(crate) fn acknowledge(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
    lease_id: Uuid,
    server_cursor: u64,
) -> Result<(), ServiceError> {
    acknowledge_at(
        storage,
        account_id,
        device_id,
        event_id,
        lease_id,
        server_cursor,
        unix_time_millis()?,
    )
}

fn acknowledge_at(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
    event_id: Uuid,
    lease_id: Uuid,
    server_cursor: u64,
    now: i64,
) -> Result<(), ServiceError> {
    let server_cursor = i64::try_from(server_cursor).map_err(|_| invalid_request())?;
    let account_id = account_id.to_string();
    let device_id = device_id.to_string();
    let event_id = event_id.to_string();
    let mut connection = storage.connect().map_err(storage_failure)?;
    let transaction = connection.transaction().map_err(storage_failure)?;
    transaction
        .execute(
            "DELETE FROM sync_outbox_receipts
             WHERE account_id = ?1 AND device_id = ?2 AND expires_at_unix_ms <= ?3",
            params![account_id, device_id, now],
        )
        .map_err(storage_failure)?;
    let already_acknowledged = transaction
        .query_row(
            "SELECT 1 FROM sync_outbox_receipts
             WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3",
            params![account_id, device_id, event_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(storage_failure)?
        .is_some();
    if already_acknowledged {
        transaction.commit().map_err(storage_failure)?;
        return Ok(());
    }
    let envelope = transaction
        .query_row(
            "SELECT envelope FROM sync_outbox
             WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3
               AND lease_id = ?4 AND lease_until_unix_ms > ?5",
            params![account_id, device_id, event_id, lease_id.to_string(), now],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(storage_failure)?
        .ok_or_else(|| outbox_conflict())?;
    let receipt_count = transaction
        .query_row(
            "SELECT COUNT(*) FROM sync_outbox_receipts
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(storage_failure)?;
    if receipt_count >= MAXIMUM_RECEIPTS {
        return Err(queue_full());
    }
    let current_cursor = transaction
        .query_row(
            "SELECT server_cursor FROM sync_device_cursors
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage_failure)?
        .unwrap_or(0);
    if server_cursor < current_cursor {
        return Err(cursor_regression());
    }
    let expires_at = now
        .checked_add(RECEIPT_RETENTION_MS)
        .ok_or_else(invalid_request)?;
    let digest = Sha256::digest(envelope);
    transaction
        .execute(
            "INSERT INTO sync_outbox_receipts
                (account_id, device_id, event_id, envelope_sha256, expires_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                account_id,
                device_id,
                event_id,
                digest.as_slice(),
                expires_at
            ],
        )
        .map_err(storage_failure)?;
    transaction
        .execute(
            "INSERT INTO sync_device_cursors (account_id, device_id, server_cursor, updated_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(account_id, device_id) DO UPDATE SET
                server_cursor = excluded.server_cursor,
                updated_at_unix_ms = excluded.updated_at_unix_ms",
            params![account_id, device_id, server_cursor, now],
        )
        .map_err(storage_failure)?;
    transaction
        .execute(
            "DELETE FROM sync_outbox WHERE account_id = ?1 AND device_id = ?2 AND event_id = ?3",
            params![account_id, device_id, event_id],
        )
        .map_err(storage_failure)?;
    transaction.commit().map_err(storage_failure)
}

pub(crate) fn server_cursor(
    storage: &AppStorage,
    account_id: Uuid,
    device_id: Uuid,
) -> Result<u64, ServiceError> {
    let connection = storage.connect().map_err(storage_failure)?;
    let cursor = connection
        .query_row(
            "SELECT server_cursor FROM sync_device_cursors WHERE account_id = ?1 AND device_id = ?2",
            params![account_id.to_string(), device_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage_failure)?
        .unwrap_or(0);
    u64::try_from(cursor).map_err(|_| invalid_request())
}

fn unix_time_millis() -> Result<i64, ServiceError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid_request())?
        .as_millis();
    i64::try_from(millis).map_err(|_| invalid_request())
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The sync outbox request is invalid.",
        false,
    )
}

fn idempotency_conflict() -> ServiceError {
    ServiceError::new(
        "sync_event_id_conflict",
        "The sync event ID is already bound to another envelope.",
        false,
    )
}

fn queue_full() -> ServiceError {
    ServiceError::new(
        "sync_outbox_full",
        "The sync outbox has reached its storage limit.",
        true,
    )
}

fn outbox_conflict() -> ServiceError {
    ServiceError::new(
        "sync_outbox_event_unavailable",
        "The sync event is not currently leased.",
        true,
    )
}

fn cursor_regression() -> ServiceError {
    ServiceError::new(
        "sync_cursor_regression",
        "The sync server cursor moved backwards.",
        false,
    )
}

fn storage_failure(_error: impl std::fmt::Display) -> ServiceError {
    ServiceError::new(
        "sync_outbox_storage_unavailable",
        "The sync outbox storage is unavailable.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use uuid::Uuid;

    use crate::storage::AppStorage;

    use super::{
        ClaimedEnvelope, EnqueueResult, acknowledge_at, claim_due_at, enqueue_at, record_retry_at,
        requeue_exhausted_at, server_cursor,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-sync-outbox-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("create isolated outbox directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated outbox directory");
        }
    }

    fn open_storage() -> (TestDirectory, AppStorage) {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open isolated storage");
        let connection = storage.connect().expect("connect isolated storage");
        connection
            .execute_batch(
                "CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                 CREATE TABLE messages (
                    id TEXT NOT NULL, conversation_id TEXT NOT NULL, role TEXT NOT NULL,
                    content TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER,
                    output_tokens INTEGER, tool_activities TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (conversation_id, id)
                 );",
            )
            .expect("create prerequisite host schema");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize outbox schema");
        (directory, storage)
    }

    fn ids() -> (Uuid, Uuid, Uuid) {
        (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4())
    }

    #[test]
    fn outbox_persists_idempotent_envelopes_and_enforces_event_identity() {
        let (_directory, storage) = open_storage();
        let (account, device, event) = ids();
        assert_eq!(
            enqueue_at(&storage, account, device, event, b"opaque", 10).unwrap(),
            EnqueueResult::Queued
        );
        assert_eq!(
            enqueue_at(&storage, account, device, event, b"opaque", 11).unwrap(),
            EnqueueResult::Duplicate
        );
        assert!(enqueue_at(&storage, account, device, event, b"different", 12).is_err());

        let first = claim_due_at(&storage, account, device, 10, 10).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].envelope, b"opaque");
        let stale_lease = Uuid::parse_str(&first[0].lease_id).unwrap();
        assert!(
            claim_due_at(&storage, account, device, 10, 20)
                .unwrap()
                .is_empty()
        );
        let reclaimed = claim_due_at(&storage, account, device, 10, 30_010).unwrap();
        assert_eq!(reclaimed.len(), 1);
        assert_ne!(reclaimed[0].lease_id, stale_lease.to_string());
        assert!(record_retry_at(&storage, account, device, event, stale_lease, 30_010).is_err());
    }

    #[test]
    fn retry_uses_bounded_backoff_and_stops_after_the_attempt_limit() {
        let (_directory, storage) = open_storage();
        let (account, device, event) = ids();
        enqueue_at(&storage, account, device, event, b"opaque", 0).unwrap();
        let mut time = 0;
        for _attempt in 1..=8 {
            let claimed = claim_due_at(&storage, account, device, 1, time).unwrap();
            assert_eq!(claimed.len(), 1);
            let lease_id = Uuid::parse_str(&claimed[0].lease_id).unwrap();
            record_retry_at(&storage, account, device, event, lease_id, time).unwrap();
            time += 15 * 60 * 1_000;
        }
        assert!(
            claim_due_at(&storage, account, device, 1, time)
                .unwrap()
                .is_empty()
        );
        assert!(requeue_exhausted_at(&storage, account, device, event, time).unwrap());
        assert_eq!(
            claim_due_at(&storage, account, device, 1, time)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn acknowledgement_is_atomic_with_receipt_and_monotonic_cursor() {
        let (_directory, storage) = open_storage();
        let (account, device, event) = ids();
        enqueue_at(&storage, account, device, event, b"opaque", 0).unwrap();
        let claimed = claim_due_at(&storage, account, device, 1, 0).unwrap();
        assert_eq!(claimed.len(), 1);
        let lease_id = Uuid::parse_str(&claimed[0].lease_id).unwrap();

        acknowledge_at(&storage, account, device, event, lease_id, 5, 10).unwrap();
        acknowledge_at(&storage, account, device, event, lease_id, 4, 11).unwrap();
        assert_eq!(server_cursor(&storage, account, device).unwrap(), 5);
        assert_eq!(
            enqueue_at(&storage, account, device, event, b"opaque", 11).unwrap(),
            EnqueueResult::Duplicate
        );

        let next = Uuid::new_v4();
        enqueue_at(&storage, account, device, next, b"another", 12).unwrap();
        let claimed = claim_due_at(&storage, account, device, 1, 12).unwrap();
        let next_lease = Uuid::parse_str(&claimed[0].lease_id).unwrap();
        assert!(acknowledge_at(&storage, account, device, next, next_lease, 4, 13).is_err());
        assert_eq!(server_cursor(&storage, account, device).unwrap(), 5);
    }

    #[test]
    fn payload_validation_rejects_empty_envelopes() {
        let (_directory, storage) = open_storage();
        let (account, device, event) = ids();
        assert!(enqueue_at(&storage, account, device, event, b"", 0).is_err());
    }

    #[test]
    fn claimed_envelope_has_only_opaque_payload_and_delivery_metadata() {
        let envelope = ClaimedEnvelope {
            event_id: "event".into(),
            lease_id: "lease".into(),
            envelope: vec![1],
            attempt_count: 0,
        };
        assert_eq!(envelope.envelope, vec![1]);
    }
}
