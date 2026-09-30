use super::{
    CHATGPT_CLIENT_VERSION, CHATGPT_WHAM_BASE, ChatGptService, METADATA_REQUEST_TIMEOUT,
    MODEL_CATALOG_CACHE_AGE, RESET_CREDIT_CONSUME_TIMEOUT, connection_unavailable, database_error,
    invalid_response_error, provider_timeout, request_cancelled, serialization_error,
};
use crate::{chatgpt_store, oauth::parse_reference, protocol::ServiceError};
use reqwest::Method;
use serde_json::{Value, json};
use std::time::Instant;
use tokio::{sync::watch, time::timeout};
use uuid::Uuid;

impl ChatGptService {
    pub fn list_connections(&self) -> Result<Value, ServiceError> {
        let connections = chatgpt_store::list_connections(&self.storage).map_err(database_error)?;
        serde_json::to_value(json!({"connections": connections})).map_err(|_| serialization_error())
    }

    pub async fn delete_connection(&self, connection_id: &str) -> Result<Value, ServiceError> {
        let lock = self.connection_refresh_lock(connection_id).await;
        let _guard = lock.lock().await;
        let credential_reference =
            chatgpt_store::credential_reference(&self.storage, connection_id)
                .map_err(database_error)?
                .ok_or_else(|| {
                    ServiceError::new(
                        "connection_not_found",
                        "The ChatGPT connection could not be found.",
                        false,
                    )
                })?;
        let reference = parse_reference(connection_id, &credential_reference)?;
        self.oauth.delete_token_pair(&reference)?;

        if !chatgpt_store::delete_connection(&self.storage, connection_id)
            .map_err(database_error)?
        {
            return Err(ServiceError::new(
                "connection_not_found",
                "The ChatGPT connection could not be found.",
                false,
            ));
        }

        self.list_connections()
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
