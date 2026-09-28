use std::time::Duration;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, StatusCode};
use rusqlite::Error as DatabaseError;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::{io::AsyncWriteExt, sync::watch, time::timeout};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    chatgpt_store::{self, NewChatGptConnection, NewChatGptWorkspace},
    credentials::{
        CredentialStore, CredentialStoreError, OAuthCredentialReference, OAuthTokenPair,
    },
    protocol::ServiceError,
    storage::AppStorage,
};

mod callback;
mod identity;

use callback::{
    CallbackRequestOutcome, authorization_url, bind_callback_listener, create_verifier,
    parse_callback_request, read_callback_request_line, record_oauth_event,
};
use identity::{ParsedIdentity, parse_identity};

pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTHORIZATION_ENDPOINT: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_ENDPOINT: &str = "https://auth.openai.com/oauth/token";
const DEFAULT_CALLBACK_PORT: u16 = 1455;
const FALLBACK_CALLBACK_PORT: u16 = 1457;
const CALLBACK_PATH: &str = "/auth/callback";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(240);
const MAX_CALLBACK_REQUEST_LINE_BYTES: usize = 8192;

pub struct OAuthClient {
    http: Client,
    credential_store: CredentialStore,
}

pub struct OAuthConnectionResult {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
    pub external_user_id: Option<String>,
    pub workspaces: Vec<NewChatGptWorkspace>,
    pub credential_reference: OAuthCredentialReference,
    pub tokens: OAuthTokenPair,
}

impl OAuthClient {
    pub fn new() -> Result<Self, ServiceError> {
        let http = Client::builder()
            .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| {
                ServiceError::new(
                    "network_unavailable",
                    "A secure network client could not be initialized.",
                    true,
                )
            })?;
        Ok(Self {
            http,
            credential_store: CredentialStore,
        })
    }

    pub async fn authorize(
        &self,
        storage: &AppStorage,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        record_oauth_event(storage, "oauth_flow_started", None, None, None, None);
        let result = match self.complete_authorization(storage, cancellation).await {
            Ok(result) => result,
            Err(error) => {
                record_oauth_event(
                    storage,
                    "oauth_flow_failed",
                    None,
                    None,
                    Some(error.code),
                    None,
                );
                return Err(error);
            }
        };
        match self.persist_connection(storage, result) {
            Ok(connection) => {
                record_oauth_event(
                    storage,
                    "oauth_connection_persisted",
                    None,
                    None,
                    None,
                    None,
                );
                Ok(connection)
            }
            Err(error) => {
                record_oauth_event(
                    storage,
                    "oauth_connection_persist_failed",
                    None,
                    None,
                    Some(error.code),
                    None,
                );
                Err(error)
            }
        }
    }

    pub fn load_tokens(
        &self,
        storage: &AppStorage,
        connection_id: &str,
    ) -> Result<OAuthTokenPair, ServiceError> {
        let reference = chatgpt_store::credential_reference(storage, connection_id)
            .map_err(database_error)?
            .ok_or_else(|| {
                ServiceError::new(
                    "connection_not_found",
                    "The selected ChatGPT connection could not be found.",
                    false,
                )
            })?;
        let reference = parse_reference(connection_id, &reference)?;
        self.credential_store
            .load_token_pair(&reference)
            .map_err(credential_error)
    }

    pub async fn refresh_tokens(
        &self,
        old_tokens: &OAuthTokenPair,
    ) -> Result<OAuthTokenPair, ServiceError> {
        let form = [
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", old_tokens.refresh_token()),
        ];
        let response = self
            .http
            .post(TOKEN_ENDPOINT)
            .form(&form)
            .send()
            .await
            .map_err(|_| network_error())?;
        let status = response.status();
        let body = Zeroizing::new(response.text().await.map_err(|_| network_error())?);
        if !status.is_success() {
            return Err(
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST {
                    ServiceError::new(
                        "refresh_rejected",
                        "ChatGPT rejected this connection. Sign in to the account again.",
                        false,
                    )
                } else {
                    ServiceError::new(
                        "refresh_failed",
                        "ChatGPT could not refresh this connection. Try again later.",
                        status.is_server_error(),
                    )
                },
            );
        }
        let mut response: TokenResponse = serde_json::from_str(&body).map_err(|_| {
            ServiceError::new(
                "invalid_oauth_response",
                "ChatGPT returned an unsupported token response.",
                false,
            )
        })?;
        let access_token = std::mem::take(&mut response.access_token);
        let refresh_token = response
            .refresh_token
            .take()
            .unwrap_or_else(|| old_tokens.refresh_token().to_owned());
        OAuthTokenPair::new(access_token, refresh_token).map_err(credential_error)
    }

    pub fn store_token_pair(
        &self,
        reference: &OAuthCredentialReference,
        tokens: &OAuthTokenPair,
    ) -> Result<(), ServiceError> {
        self.credential_store
            .store_new_token_pair(reference, tokens)
            .map_err(credential_error)
    }

    pub fn replace_stored_reference(
        &self,
        storage: &AppStorage,
        connection_id: &str,
        reference: &OAuthCredentialReference,
    ) -> Result<Option<OAuthCredentialReference>, ServiceError> {
        let previous = chatgpt_store::replace_credential_reference(
            storage,
            connection_id,
            &reference_name(reference),
        )
        .map_err(database_error)?;
        previous
            .map(|value| parse_reference(connection_id, &value))
            .transpose()
    }

    pub fn delete_token_pair(
        &self,
        reference: &OAuthCredentialReference,
    ) -> Result<(), ServiceError> {
        self.credential_store
            .delete_token_pair(reference)
            .map_err(credential_error)
    }

    async fn complete_authorization(
        &self,
        storage: &AppStorage,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<OAuthConnectionResult, ServiceError> {
        let (listener, port) = bind_callback_listener(storage).await?;
        let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
        let state = Zeroizing::new(Uuid::new_v4().simple().to_string());
        let verifier = create_verifier();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let authorize_url = authorization_url(&redirect_uri, &state, &challenge)?;

        let browser_result =
            tokio::task::spawn_blocking(move || webbrowser::open(authorize_url.as_str())).await;
        match browser_result {
            Ok(Ok(())) => {
                record_oauth_event(storage, "oauth_browser_opened", None, None, None, None);
            }
            _ => {
                record_oauth_event(
                    storage,
                    "oauth_browser_open_failed",
                    None,
                    None,
                    Some("browser_unavailable"),
                    None,
                );
                return Err(browser_error());
            }
        }

        let callback_result = timeout(CALLBACK_TIMEOUT, async {
            loop {
                tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            return Err(ServiceError::new("oauth_cancelled", "ChatGPT sign-in was cancelled.", false));
                        }
                    }
                    accepted = listener.accept() => {
                        let (mut socket, peer) = accepted.map_err(|_| callback_error())?;
                        record_oauth_event(
                            storage,
                            "oauth_callback_connection_accepted",
                            Some(if peer.is_ipv4() { "ipv4" } else { "ipv6" }),
                            None,
                            None,
                            None,
                        );
                        let outcome = match timeout(
                            Duration::from_secs(10),
                            read_callback_request_line(&mut socket),
                        )
                        .await
                        {
                            Err(_) => CallbackRequestOutcome::Ignored {
                                code: "oauth_callback_read_timeout",
                                status: "400 Bad Request",
                            },
                            Ok(Err(code)) => CallbackRequestOutcome::Ignored {
                                code,
                                status: "400 Bad Request",
                            },
                            Ok(Ok(None)) => CallbackRequestOutcome::Ignored {
                                code: "oauth_callback_empty_request",
                                status: "400 Bad Request",
                            },
                            Ok(Ok(Some(request_line))) => {
                                parse_callback_request(&request_line, &state, port)
                            }
                        };
                        let (status, body) = match &outcome {
                            CallbackRequestOutcome::Accepted(_) => (
                                "200 OK",
                                "<html><body>Sign-in complete. You can return to OpenChat.</body></html>",
                            ),
                            CallbackRequestOutcome::Ignored { status, .. } => (
                                *status,
                                "<html><body>This request was not a sign-in callback. Return to OpenChat.</body></html>",
                            ),
                            CallbackRequestOutcome::Rejected(_) => (
                                "400 Bad Request",
                                "<html><body>Sign-in could not be completed. Return to OpenChat.</body></html>",
                            ),
                        };
                        let response = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        if socket.write_all(response.as_bytes()).await.is_err() {
                            record_oauth_event(
                                storage,
                                "oauth_callback_response_write_failed",
                                Some(if peer.is_ipv4() { "ipv4" } else { "ipv6" }),
                                Some(port),
                                Some("response_write_failed"),
                                None,
                            );
                        }
                        if socket.shutdown().await.is_err() {
                            record_oauth_event(
                                storage,
                                "oauth_callback_response_shutdown_failed",
                                Some(if peer.is_ipv4() { "ipv4" } else { "ipv6" }),
                                Some(port),
                                Some("response_shutdown_failed"),
                                None,
                            );
                        }
                        match outcome {
                            CallbackRequestOutcome::Accepted(callback) => return Ok(callback),
                            CallbackRequestOutcome::Ignored { code, .. } => {
                                record_oauth_event(
                                    storage,
                                    "oauth_callback_request_ignored",
                                    Some(if peer.is_ipv4() { "ipv4" } else { "ipv6" }),
                                    Some(port),
                                    Some(code),
                                    None,
                                );
                            }
                            CallbackRequestOutcome::Rejected(error) => return Err(error),
                        }
                    }
                }
            }
        })
        .await;
        let callback = match callback_result {
            Err(_) => {
                record_oauth_event(
                    storage,
                    "oauth_callback_timeout",
                    Some("127.0.0.1"),
                    Some(port),
                    Some("oauth_timeout"),
                    None,
                );
                return Err(ServiceError::new(
                    "oauth_timeout",
                    "ChatGPT sign-in expired. Try again.",
                    true,
                ));
            }
            Ok(Err(error)) => {
                record_oauth_event(
                    storage,
                    "oauth_callback_failed",
                    Some("127.0.0.1"),
                    Some(port),
                    Some(error.code),
                    None,
                );
                return Err(error);
            }
            Ok(Ok(callback)) => {
                record_oauth_event(
                    storage,
                    "oauth_callback_validated",
                    Some("127.0.0.1"),
                    Some(port),
                    None,
                    None,
                );
                callback
            }
        };

        self.exchange_code(storage, callback.code, &redirect_uri, &verifier)
            .await
    }

    async fn exchange_code(
        &self,
        storage: &AppStorage,
        code: Zeroizing<String>,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<OAuthConnectionResult, ServiceError> {
        let form = [
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ];
        let response = match self.http.post(TOKEN_ENDPOINT).form(&form).send().await {
            Ok(response) => response,
            Err(_) => {
                record_oauth_event(
                    storage,
                    "oauth_token_exchange_transport_failed",
                    None,
                    None,
                    Some("network_unavailable"),
                    None,
                );
                return Err(network_error());
            }
        };
        let status = response.status();
        let body = match response.text().await {
            Ok(body) => Zeroizing::new(body),
            Err(_) => {
                record_oauth_event(
                    storage,
                    "oauth_token_response_read_failed",
                    None,
                    None,
                    Some("network_unavailable"),
                    Some(status.as_u16()),
                );
                return Err(network_error());
            }
        };
        if !status.is_success() {
            record_oauth_event(
                storage,
                "oauth_token_exchange_rejected",
                None,
                None,
                Some("oauth_exchange_rejected"),
                Some(status.as_u16()),
            );
            return Err(ServiceError::new(
                "oauth_exchange_rejected",
                "ChatGPT did not accept the sign-in request. The OAuth client or callback may not be allowed.",
                false,
            ));
        }
        record_oauth_event(
            storage,
            "oauth_token_exchange_succeeded",
            None,
            None,
            None,
            Some(status.as_u16()),
        );
        let mut tokens: TokenResponse = match serde_json::from_str(&body) {
            Ok(tokens) => tokens,
            Err(_) => {
                record_oauth_event(
                    storage,
                    "oauth_token_response_invalid",
                    None,
                    None,
                    Some("invalid_oauth_response"),
                    None,
                );
                return Err(ServiceError::new(
                    "invalid_oauth_response",
                    "ChatGPT returned an unsupported token response.",
                    false,
                ));
            }
        };
        let access_token = std::mem::take(&mut tokens.access_token);
        let refresh_token = tokens.refresh_token.take().ok_or_else(|| {
            ServiceError::new(
                "offline_access_unavailable",
                "ChatGPT did not provide a refresh token. Check the account's sign-in permissions and try again.",
                false,
            )
        })?;
        let id_token = Zeroizing::new(std::mem::take(&mut tokens.id_token));
        if id_token.is_empty() {
            return Err(ServiceError::new(
                "account_profile_unavailable",
                "ChatGPT did not provide account information for this connection.",
                false,
            ));
        }
        let ParsedIdentity {
            email,
            display_name,
            plan_type,
            external_user_id,
            workspaces: external_workspaces,
        } = parse_identity(&id_token)?;
        let id = Uuid::new_v4().simple().to_string();
        let workspaces = external_workspaces
            .into_iter()
            .map(|workspace| NewChatGptWorkspace {
                id: Uuid::new_v4().simple().to_string(),
                external_id: workspace.id,
                display_name: workspace.name,
                plan_type: workspace.plan_type,
                is_selected: workspace.is_default,
            })
            .collect();
        let reference =
            OAuthCredentialReference::new(id.clone(), Uuid::new_v4().simple().to_string())
                .map_err(credential_error)?;
        let tokens = OAuthTokenPair::new(access_token, refresh_token).map_err(credential_error)?;
        Ok(OAuthConnectionResult {
            id,
            email,
            display_name,
            plan_type,
            external_user_id,
            workspaces,
            credential_reference: reference,
            tokens,
        })
    }

    fn persist_connection(
        &self,
        storage: &AppStorage,
        result: OAuthConnectionResult,
    ) -> Result<Value, ServiceError> {
        let existing_connection_id = result
            .external_user_id
            .as_deref()
            .map(|user_id| chatgpt_store::connection_id_for_external_user(storage, user_id))
            .transpose()
            .map_err(database_error)?
            .flatten();
        let connection_id = existing_connection_id
            .clone()
            .unwrap_or_else(|| result.id.clone());
        let credential_reference = if existing_connection_id.is_some() {
            OAuthCredentialReference::new(
                connection_id.clone(),
                result.credential_reference.generation_id().to_owned(),
            )
            .map_err(credential_error)?
        } else {
            result.credential_reference.clone()
        };
        self.store_token_pair(&credential_reference, &result.tokens)?;
        let connection = NewChatGptConnection {
            id: connection_id,
            external_user_id: result.external_user_id,
            email: result.email,
            display_name: result.display_name,
            plan_type: result.plan_type,
            credential_reference: reference_name(&credential_reference),
            workspaces: result.workspaces,
        };
        let previous_reference = if existing_connection_id.is_some() {
            match chatgpt_store::reconnect_connection(storage, connection.clone()) {
                Ok(Some(previous_reference)) => Some(previous_reference),
                Ok(None) => {
                    if self.delete_token_pair(&credential_reference).is_err() {
                        eprintln!("oauth_new_credential_cleanup_failed");
                    }
                    return Err(ServiceError::new(
                        "connection_changed",
                        "The ChatGPT account changed while reconnecting. Try signing in again.",
                        true,
                    ));
                }
                Err(error) => {
                    if self.delete_token_pair(&credential_reference).is_err() {
                        eprintln!("oauth_new_credential_cleanup_failed");
                    }
                    return Err(database_error(error));
                }
            }
        } else {
            if let Err(error) = chatgpt_store::create_connection(storage, connection) {
                if self.delete_token_pair(&credential_reference).is_err() {
                    eprintln!("oauth_new_credential_cleanup_failed");
                }
                return Err(database_error(error));
            }
            None
        };
        let old_credential_cleanup_failed = match previous_reference {
            Some(reference) => {
                match parse_reference(credential_reference.connection_id(), &reference) {
                    Ok(reference) => self.delete_token_pair(&reference).is_err(),
                    Err(_) => true,
                }
            }
            None => false,
        };
        let connections = chatgpt_store::list_connections(storage).map_err(database_error)?;
        serde_json::to_value(serde_json::json!({
            "connections": connections,
            "oldCredentialCleanupFailed": old_credential_cleanup_failed,
        }))
        .map_err(|_| {
            ServiceError::new(
                "serialization_failed",
                "The ChatGPT connection could not be returned.",
                false,
            )
        })
    }
}

pub fn parse_reference(
    connection_id: &str,
    value: &str,
) -> Result<OAuthCredentialReference, ServiceError> {
    let Some((stored_connection_id, generation_id)) = value.split_once(':') else {
        return Err(ServiceError::new(
            "credential_reference_invalid",
            "The saved ChatGPT credential reference is invalid. Sign in again.",
            false,
        ));
    };
    if stored_connection_id != connection_id || generation_id.contains(':') {
        return Err(ServiceError::new(
            "credential_reference_invalid",
            "The saved ChatGPT credential reference is invalid. Sign in again.",
            false,
        ));
    }
    OAuthCredentialReference::new(stored_connection_id, generation_id).map_err(credential_error)
}

pub fn reference_name(reference: &OAuthCredentialReference) -> String {
    format!(
        "{}:{}",
        reference.connection_id(),
        reference.generation_id()
    )
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: String,
}

impl Drop for TokenResponse {
    fn drop(&mut self) {
        self.access_token.zeroize();
        self.id_token.zeroize();
        if let Some(refresh_token) = &mut self.refresh_token {
            refresh_token.zeroize();
        }
    }
}

fn database_error(_: DatabaseError) -> ServiceError {
    ServiceError::new(
        "local_storage_failed",
        "ChatGPT connection data could not be read or saved in the local database.",
        true,
    )
}

fn credential_error(error: CredentialStoreError) -> ServiceError {
    let (code, message, retryable) = match error {
        CredentialStoreError::InvalidReference => (
            "credential_reference_invalid",
            "The saved ChatGPT credential reference is invalid. Sign in again.",
            false,
        ),
        CredentialStoreError::EmptyToken => (
            "oauth_token_empty",
            "ChatGPT returned an empty OAuth token. Sign in again.",
            false,
        ),
        CredentialStoreError::TokenTooLarge => (
            "oauth_token_too_large",
            "The ChatGPT token exceeds the Windows secure storage limit.",
            false,
        ),
        CredentialStoreError::ReferenceAlreadyUsed => (
            "credential_reference_in_use",
            "The new ChatGPT credential reference is already in use. Try again.",
            true,
        ),
        CredentialStoreError::TokenPairMissing | CredentialStoreError::IncompleteTokenPair => (
            "oauth_tokens_missing",
            "The saved ChatGPT tokens are unavailable. Sign in again.",
            false,
        ),
        CredentialStoreError::InvalidStoredToken => (
            "oauth_tokens_invalid",
            "A saved ChatGPT token could not be read. Sign in again.",
            false,
        ),
        CredentialStoreError::IncompleteWrite => (
            "credential_manager_incomplete_write",
            "Windows Credential Manager could not store the complete ChatGPT connection. Try again.",
            true,
        ),
        CredentialStoreError::StorageReadFailed => (
            "credential_manager_read_failed",
            "Windows Credential Manager could not read the ChatGPT connection. Try signing in again.",
            true,
        ),
        CredentialStoreError::StorageWriteFailed => (
            "credential_manager_write_failed",
            "Windows Credential Manager could not save the ChatGPT connection. Try again.",
            true,
        ),
        CredentialStoreError::StorageDeleteFailed => (
            "credential_manager_delete_failed",
            "Windows Credential Manager could not remove an old ChatGPT credential.",
            true,
        ),
        CredentialStoreError::StorageUnavailable | CredentialStoreError::LockUnavailable => (
            "secure_storage_unavailable",
            "Windows Credential Manager could not access the ChatGPT connection. Try again.",
            true,
        ),
    };
    ServiceError::new(code, message, retryable)
}

fn network_error() -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        "ChatGPT could not be reached. Check the internet connection and try again.",
        true,
    )
}

fn browser_error() -> ServiceError {
    ServiceError::new(
        "browser_unavailable",
        "The browser could not be opened for ChatGPT sign-in.",
        true,
    )
}

fn callback_error() -> ServiceError {
    ServiceError::new(
        "oauth_callback_invalid",
        "ChatGPT returned an invalid sign-in response. Start sign-in again.",
        false,
    )
}

fn account_info_error() -> ServiceError {
    ServiceError::new(
        "account_profile_unavailable",
        "ChatGPT did not provide a supported account profile. No connection was saved.",
        false,
    )
}
