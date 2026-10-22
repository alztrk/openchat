//! Fetches and parses ChatGPT account, model, and usage metadata.
//!
//! Responses are bounded and normalized before snapshots are persisted for the
//! account and quota surfaces.

use super::{
    CHATGPT_CLIENT_VERSION, CHATGPT_WHAM_BASE, ChatGptService, METADATA_REQUEST_TIMEOUT,
    MODEL_CATALOG_CACHE_AGE, RESET_CREDIT_CONSUME_TIMEOUT, connection_unavailable, database_error,
    invalid_response_error, provider_timeout, request_cancelled, serialization_error,
};
use crate::{
    chatgpt_store, oauth::parse_reference, protocol::ServiceError, service::OperationGate,
};
use reqwest::Method;
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};
use tokio::{sync::watch, time::timeout};
use uuid::Uuid;

impl ChatGptService {
    pub fn list_connections(&self) -> Result<Value, ServiceError> {
        let connections = chatgpt_store::list_connections(&self.storage).map_err(database_error)?;
        serde_json::to_value(json!({"connections": connections})).map_err(|_| serialization_error())
    }

    pub async fn delete_connection(
        &self,
        connection_id: &str,
        cancellation: &mut watch::Receiver<bool>,
        cancellation_gate: &OperationGate,
    ) -> Result<Value, ServiceError> {
        let started = Instant::now();
        log_connection_delete_event(&self.storage, "connection_delete_started", None, None);
        if *cancellation.borrow() {
            return Err(request_cancelled());
        }
        // A refresh commits only while the connection exists and discards its new credential otherwise.
        // Removing the row first avoids holding the UI operation open for an OAuth network request.
        if !cancellation_gate.try_begin_commit() {
            return Err(request_cancelled());
        }
        log_connection_delete_event(
            &self.storage,
            "connection_delete_commit_started",
            None,
            Some(started.elapsed().as_millis()),
        );
        let storage = Arc::clone(&self.storage);
        let connection_id = connection_id.to_owned();
        let deletion = tokio::task::spawn_blocking(move || {
            chatgpt_store::delete_connection(&storage, &connection_id).map_err(database_error)
        })
        .await
        .map_err(|_| {
            ServiceError::new(
                "connection_remove_worker_failed",
                "The ChatGPT connection could not be removed. Try again.",
                true,
            )
        })?;
        if let Err(error) = deletion {
            log_connection_delete_event(
                &self.storage,
                "connection_delete_failed",
                Some(error.code),
                Some(started.elapsed().as_millis()),
            );
            return Err(error);
        }
        self.schedule_pending_credential_cleanup();
        log_connection_delete_event(
            &self.storage,
            "connection_delete_queued",
            None,
            Some(started.elapsed().as_millis()),
        );

        Ok(json!({"removed": true}))
    }

    pub(crate) fn schedule_pending_credential_cleanup(&self) {
        let storage = Arc::clone(&self.storage);
        let oauth = Arc::clone(&self.oauth);
        let cleanup_lock = Arc::clone(&self.credential_cleanup_lock);
        tokio::spawn(async move {
            let _guard = cleanup_lock.lock().await;
            let cleanup_storage = Arc::clone(&storage);
            let result = tokio::task::spawn_blocking(move || {
                let pending = chatgpt_store::pending_credential_deletions(&cleanup_storage)
                    .map_err(|_| "database_read_failed")?;
                for (connection_id, credential_reference) in pending {
                    let reference = match parse_reference(&connection_id, &credential_reference) {
                        Ok(reference) => reference,
                        Err(error) => {
                            log_connection_delete_event(
                                &cleanup_storage,
                                "connection_credentials_delete_failed",
                                Some(error.code),
                                None,
                            );
                            continue;
                        }
                    };
                    log_connection_delete_event(
                        &cleanup_storage,
                        "connection_credentials_delete_started",
                        None,
                        None,
                    );
                    let mut deletion_error = None;
                    for attempt in 0..3 {
                        match oauth.delete_token_pair(&reference) {
                            Ok(()) => {
                                deletion_error = None;
                                break;
                            }
                            Err(error) => {
                                deletion_error = Some(error.code);
                                if attempt < 2 {
                                    std::thread::sleep(std::time::Duration::from_millis(
                                        250 * (attempt + 1),
                                    ));
                                }
                            }
                        }
                    }
                    if let Some(code) = deletion_error {
                        log_connection_delete_event(
                            &cleanup_storage,
                            "connection_credentials_delete_failed",
                            Some(code),
                            None,
                        );
                        continue;
                    }
                    let mut queue_update_failed = true;
                    for attempt in 0..3 {
                        if chatgpt_store::complete_credential_deletion(
                            &cleanup_storage,
                            &connection_id,
                            &credential_reference,
                        )
                        .is_ok()
                        {
                            queue_update_failed = false;
                            break;
                        }
                        if attempt < 2 {
                            std::thread::sleep(std::time::Duration::from_millis(
                                250 * (attempt + 1),
                            ));
                        }
                    }
                    if queue_update_failed {
                        log_connection_delete_event(
                            &cleanup_storage,
                            "connection_credentials_queue_update_failed",
                            Some("database_write_failed"),
                            None,
                        );
                    } else {
                        log_connection_delete_event(
                            &cleanup_storage,
                            "connection_credentials_deleted",
                            None,
                            None,
                        );
                    }
                }
                Ok::<(), &'static str>(())
            })
            .await;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(code)) => log_connection_delete_event(
                    &storage,
                    "connection_credentials_cleanup_worker_failed",
                    Some(code),
                    None,
                ),
                Err(_) => log_connection_delete_event(
                    &storage,
                    "connection_credentials_cleanup_worker_failed",
                    Some("worker_failed"),
                    None,
                ),
            }
        });
    }

    pub fn select_connection(&self, connection_id: &str) -> Result<Value, ServiceError> {
        let selected = chatgpt_store::set_selected_connection(&self.storage, connection_id)
            .map_err(database_error)?;
        if !selected {
            return Err(connection_unavailable());
        }
        self.list_connections()
    }

    pub fn select_workspace(
        &self,
        connection_id: &str,
        workspace_id: &str,
    ) -> Result<Value, ServiceError> {
        let selected =
            chatgpt_store::set_selected_workspace(&self.storage, connection_id, workspace_id)
                .map_err(database_error)?;
        if !selected {
            return Err(ServiceError::new(
                "workspace_not_found",
                "The selected ChatGPT workspace is no longer available. Refresh the connection list.",
                false,
            ));
        }
        self.list_connections()
    }

    pub async fn models(
        &self,
        connection_id: &str,
        workspace_id: &str,
        force_refresh: bool,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        let started = Instant::now();
        self.record_chatgpt_event("request_started", "models", None, None, None, None);
        if !force_refresh {
            let cached = chatgpt_store::list_fresh_models(
                &self.storage,
                connection_id,
                workspace_id,
                CHATGPT_CLIENT_VERSION,
                i64::try_from(MODEL_CATALOG_CACHE_AGE.as_millis()).unwrap_or(i64::MAX),
            )
            .map_err(database_error)?;
            if let Some(models) = cached {
                self.record_chatgpt_event(
                    "request_cache_hit",
                    "models",
                    None,
                    None,
                    Some(started.elapsed().as_millis()),
                    Some(models.len()),
                );
                return Ok(json!({
                    "connectionId": connection_id,
                    "workspaceId": workspace_id,
                    "models": models,
                    "freshness": "current",
                    "errorCode": Value::Null,
                }));
            }
        }
        let fetch_result = if *cancellation.borrow() {
            Err(request_cancelled())
        } else {
            tokio::select! {
                _ = cancellation.changed() => Err(request_cancelled()),
                result = tokio::time::timeout(
                    METADATA_REQUEST_TIMEOUT,
                    self.fetch_models(connection_id, workspace_id),
                ) => result.unwrap_or_else(|_| Err(provider_timeout())),
            }
        };

        match fetch_result {
            Ok(models) => {
                let model_count = models.len();
                let save_result = chatgpt_store::save_models(
                    &self.storage,
                    connection_id,
                    workspace_id,
                    CHATGPT_CLIENT_VERSION,
                    &models,
                )
                .map_err(database_error);
                if let Err(error) = save_result {
                    self.record_chatgpt_event(
                        "request_failed",
                        "models",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    return Err(error);
                }
                let result = serde_json::to_value(json!({
                    "connectionId": connection_id,
                    "workspaceId": workspace_id,
                    "models": models,
                    "freshness": "current",
                    "errorCode": Value::Null,
                }))
                .map_err(|_| serialization_error());
                match result {
                    Ok(value) => {
                        self.record_chatgpt_event(
                            "request_completed",
                            "models",
                            None,
                            None,
                            Some(started.elapsed().as_millis()),
                            Some(model_count),
                        );
                        Ok(value)
                    }
                    Err(error) => {
                        self.record_chatgpt_event(
                            "request_failed",
                            "models",
                            None,
                            Some(error.code),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        Err(error)
                    }
                }
            }

            Err(error) => {
                if error.code == "request_cancelled" {
                    self.record_chatgpt_event(
                        "request_cancelled",
                        "models",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        None,
                    );
                    return Err(error);
                }
                let cached = chatgpt_store::list_models(&self.storage, connection_id, workspace_id)
                    .map_err(database_error)?;
                if cached.is_empty() {
                    self.record_chatgpt_event(
                        "request_failed",
                        "models",
                        None,
                        Some(error.code),
                        Some(started.elapsed().as_millis()),
                        Some(0),
                    );
                    return Err(error);
                }
                let cached_count = cached.len();
                let result = serde_json::to_value(json!({
                    "connectionId": connection_id,
                    "workspaceId": workspace_id,
                    "models": cached,
                    "freshness": "stale",
                    "errorCode": error.code,
                }))
                .map_err(|_| serialization_error());
                match result {
                    Ok(value) => {
                        self.record_chatgpt_event(
                            "request_stale",
                            "models",
                            None,
                            Some(error.code),
                            Some(started.elapsed().as_millis()),
                            Some(cached_count),
                        );
                        Ok(value)
                    }
                    Err(serialization_error) => {
                        self.record_chatgpt_event(
                            "request_failed",
                            "models",
                            None,
                            Some(serialization_error.code),
                            Some(started.elapsed().as_millis()),
                            None,
                        );
                        Err(serialization_error)
                    }
                }
            }
        }
    }

    pub fn title_preference(&self) -> Result<Value, ServiceError> {
        serde_json::to_value(
            chatgpt_store::title_preference(&self.storage).map_err(database_error)?,
        )
        .map_err(|_| serialization_error())
    }

    pub fn select_title_target(
        &self,
        connection_id: Option<&str>,
        workspace_id: Option<&str>,
    ) -> Result<Value, ServiceError> {
        match (connection_id, workspace_id) {
            (None, None) => {}
            (Some(connection_id), None) => {
                let connections =
                    chatgpt_store::list_connections(&self.storage).map_err(database_error)?;
                if !connections
                    .iter()
                    .any(|connection| connection.id == connection_id)
                {
                    return Err(connection_unavailable());
                }
            }
            (Some(connection_id), Some(workspace_id)) => {
                let workspace =
                    chatgpt_store::connection_workspace(&self.storage, connection_id, workspace_id)
                        .map_err(database_error)?;
                if workspace.is_none() {
                    return Err(ServiceError::new(
                        "workspace_not_found",
                        "The selected title workspace is no longer available.",
                        false,
                    ));
                }
            }
            (None, Some(_)) => {
                return Err(ServiceError::new(
                    "invalid_request_params",
                    "Choose a ChatGPT account before choosing a title workspace.",
                    false,
                ));
            }
        }
        chatgpt_store::set_title_preference(&self.storage, connection_id, workspace_id)
            .map_err(database_error)?;
        self.title_preference()
    }

    pub async fn usage(
        &self,
        connection_id: &str,
        workspace_id: &str,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        let started = Instant::now();
        self.record_chatgpt_event("request_started", "usage", None, None, None, None);
        let refresh_result = if *cancellation.borrow() {
            Err(request_cancelled())
        } else {
            tokio::select! {
                _ = cancellation.changed() => Err(request_cancelled()),
                result = tokio::time::timeout(
                    METADATA_REQUEST_TIMEOUT,
                    self.refresh_usage(connection_id, workspace_id),
                ) => result.unwrap_or_else(|_| Err(provider_timeout())),
            }
        };
        let snapshot = match refresh_result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let event = if error.code == "request_cancelled" {
                    "request_cancelled"
                } else {
                    "request_failed"
                };
                self.record_chatgpt_event(
                    event,
                    "usage",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                return Err(error);
            }
        };
        let result = serde_json::to_value(&snapshot).map_err(|_| serialization_error());
        match result {
            Ok(value) => {
                self.record_chatgpt_event(
                    "request_completed",
                    "usage",
                    None,
                    None,
                    Some(started.elapsed().as_millis()),
                    Some(snapshot.buckets.len()),
                );
                Ok(value)
            }
            Err(error) => {
                self.record_chatgpt_event(
                    "request_failed",
                    "usage",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                Err(error)
            }
        }
    }

    pub async fn copilot_usage(
        &self,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        if *cancellation.borrow() {
            return Err(request_cancelled());
        }
        tokio::time::timeout(METADATA_REQUEST_TIMEOUT, async {
            let access_token = tokio::select! {
                _ = cancellation.changed() => return Err(request_cancelled()),
                result = self.copilot_access_token() => result?,
            };
            if *cancellation.borrow() {
                return Err(request_cancelled());
            }
            crate::openai_compatible::copilot_usage(&access_token, cancellation).await
        })
        .await
        .unwrap_or_else(|_| Err(provider_timeout()))
    }

    pub async fn consume_reset_credit(
        &self,
        connection_id: &str,
        workspace_id: &str,
        credit_id: &str,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        if credit_id.len() > 512 || credit_id.chars().any(char::is_control) {
            return Err(ServiceError::new(
                "invalid_request_params",
                "The ChatGPT reset credit identifier is invalid.",
                false,
            ));
        }

        let external_workspace_id = self.external_workspace_id(connection_id, workspace_id)?;
        let snapshot = self.read_latest_usage(connection_id, workspace_id)?;
        if snapshot.reset_credit_details_state != "available"
            || !snapshot.reset_credits.iter().any(|credit| {
                credit.id == credit_id && credit.status.as_deref() == Some("available")
            })
        {
            return Err(ServiceError::new(
                "reset_credit_unavailable",
                "The selected ChatGPT reset credit is no longer available.",
                false,
            ));
        }

        let redeem_request_id = Uuid::new_v4().to_string();
        let body = json!({
            "redeem_request_id": redeem_request_id,
            "credit_id": credit_id,
        });
        let started = Instant::now();
        self.record_chatgpt_event(
            "request_started",
            "reset_credit_consume",
            None,
            None,
            None,
            None,
        );

        let request = self.authorized_json_request(
            "reset_credit_consume",
            Method::POST,
            format!("{CHATGPT_WHAM_BASE}/rate-limit-reset-credits/consume"),
            connection_id,
            &external_workspace_id,
            Some(body),
        );
        let response = if *cancellation.borrow() {
            Err(request_cancelled())
        } else {
            tokio::select! {
                _ = cancellation.changed() => Err(request_cancelled()),
                result = timeout(RESET_CREDIT_CONSUME_TIMEOUT, request) => {
                    result.unwrap_or_else(|_| Err(provider_timeout()))
                }
            }
        };
        let (status, value) = match response {
            Ok(response) => response,
            Err(error) => {
                let event = if error.code == "request_cancelled" {
                    "request_cancelled"
                } else {
                    "request_failed"
                };
                self.record_chatgpt_event(
                    event,
                    "reset_credit_consume",
                    None,
                    Some(error.code),
                    Some(started.elapsed().as_millis()),
                    None,
                );
                return Err(error);
            }
        };

        let outcome = value
            .get("outcome")
            .and_then(Value::as_str)
            .filter(|outcome| {
                matches!(
                    *outcome,
                    "reset" | "alreadyRedeemed" | "nothingToReset" | "noCredit"
                )
            });
        let Some(outcome) = outcome else {
            let error = invalid_response_error();
            self.record_chatgpt_event(
                "response_failed",
                "reset_credit_consume",
                Some(status),
                Some(error.code),
                Some(started.elapsed().as_millis()),
                None,
            );
            return Err(error);
        };
        self.record_chatgpt_event(
            "request_completed",
            "reset_credit_consume",
            Some(status),
            None,
            Some(started.elapsed().as_millis()),
            None,
        );
        Ok(json!({"outcome": outcome}))
    }
}

fn log_connection_delete_event(
    storage: &crate::storage::AppStorage,
    event: &'static str,
    code: Option<&'static str>,
    duration_ms: Option<u128>,
) {
    if storage
        .log_chatgpt_event(event, "connection_delete", None, code, duration_ms, None)
        .is_err()
    {
        eprintln!("chatgpt_connection_delete_diagnostic_write_failed");
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };

    use super::{ChatGptService, OperationGate};
    use crate::storage::AppStorage;
    use tokio::sync::watch;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "openchat-chatgpt-account-delete-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn deleting_an_already_removed_connection_confirms_completion() {
        let directory = TestDirectory::new();
        let storage = Arc::new(AppStorage::open_at(directory.0.clone()).expect("open storage"));
        storage
            .connect()
            .expect("connect storage")
            .execute_batch(
                "CREATE TABLE pending_chatgpt_credential_deletions (
                    credential_reference TEXT PRIMARY KEY NOT NULL,
                    connection_id TEXT NOT NULL,
                    requested_at_unix_ms INTEGER NOT NULL
                );",
            )
            .expect("create cleanup queue");
        let service = ChatGptService::new(Arc::clone(&storage)).expect("create provider service");

        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let cancellation_gate = OperationGate::default();
        let result = service
            .delete_connection("already-removed", &mut cancellation, &cancellation_gate)
            .await
            .expect("confirm idempotent removal");

        assert_eq!(result["removed"], true);
    }

    #[tokio::test]
    async fn connection_removal_does_not_wait_for_an_inflight_token_refresh() {
        let directory = TestDirectory::new();
        let storage = Arc::new(AppStorage::open_at(directory.0.clone()).expect("open storage"));
        let connection = storage.connect().expect("connect storage");
        connection
            .execute_batch(
                "CREATE TABLE pending_chatgpt_credential_deletions (
                    credential_reference TEXT PRIMARY KEY NOT NULL,
                    connection_id TEXT NOT NULL,
                    requested_at_unix_ms INTEGER NOT NULL
                );
                CREATE TABLE conversations (
                    id TEXT PRIMARY KEY NOT NULL
                );",
            )
            .expect("create credential cleanup queue");
        connection
            .execute(
                "INSERT INTO chatgpt_connections (
                    id, credential_reference, auth_status, created_at_unix_ms,
                    updated_at_unix_ms, is_selected
                 ) VALUES (?1, ?2, 'active', 1, 1, 0)",
                rusqlite::params!["connection-1", "credential-1"],
            )
            .expect("insert ChatGPT connection");
        drop(connection);
        let service =
            Arc::new(ChatGptService::new(Arc::clone(&storage)).expect("create provider service"));
        let lock = service.connection_refresh_lock("connection-1").await;
        let _refresh_guard = lock.lock().await;
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let cancellation_gate = OperationGate::default();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            service.delete_connection("connection-1", &mut cancellation, &cancellation_gate),
        )
        .await
        .expect("removal should not wait for a network refresh to release its lock")
        .expect("remove connection while token refresh is in flight");
        assert_eq!(result["removed"], true);
        let remaining = storage
            .connect()
            .expect("reconnect storage")
            .query_row(
                "SELECT count(*) FROM chatgpt_connections WHERE id = 'connection-1'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("check connection row");
        assert_eq!(remaining, 0);
        assert_eq!(
            crate::chatgpt_store::pending_credential_deletions(&storage)
                .expect("load queued credential cleanup"),
            vec![("connection-1".to_owned(), "credential-1".to_owned())]
        );
    }

    #[tokio::test]
    async fn cancelled_connection_removal_keeps_the_connection_row() {
        let directory = TestDirectory::new();
        let storage = Arc::new(AppStorage::open_at(directory.0.clone()).expect("open storage"));
        let connection = storage.connect().expect("connect storage");
        connection
            .execute_batch(
                "CREATE TABLE pending_chatgpt_credential_deletions (
                    credential_reference TEXT PRIMARY KEY NOT NULL,
                    connection_id TEXT NOT NULL,
                    requested_at_unix_ms INTEGER NOT NULL
                );",
            )
            .expect("create credential cleanup queue");
        connection
            .execute(
                "INSERT INTO chatgpt_connections (
                    id, credential_reference, auth_status, created_at_unix_ms,
                    updated_at_unix_ms, is_selected
                 ) VALUES (?1, ?2, 'active', 1, 1, 0)",
                rusqlite::params!["connection-1", "credential-1"],
            )
            .expect("insert ChatGPT connection");
        drop(connection);
        let service = ChatGptService::new(Arc::clone(&storage)).expect("create provider service");
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let cancellation_gate = OperationGate::default();
        assert!(cancellation_gate.request_cancellation());

        let error = service
            .delete_connection("connection-1", &mut cancellation, &cancellation_gate)
            .await
            .expect_err("cancelled removal must not commit");
        assert_eq!(error.code, "request_cancelled");
        let remaining = storage
            .connect()
            .expect("reconnect storage")
            .query_row(
                "SELECT count(*) FROM chatgpt_connections WHERE id = 'connection-1'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("check connection row");
        assert_eq!(remaining, 1);
    }
}
