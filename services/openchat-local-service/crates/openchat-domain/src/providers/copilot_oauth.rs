//! Manages the GitHub Copilot OAuth device flow and stored connection lifecycle.
//!
//! Polling, expiry, cancellation, credential references, and provider errors are
//! handled explicitly so transient or incomplete responses do not imply login.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use reqwest::{Client, StatusCode, header::ACCEPT, redirect::Policy};
use rusqlite::{Error as DatabaseError, OptionalExtension};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, watch};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    credentials::{
        CredentialStore, CredentialStoreError, OAuthCredentialReference, OAuthTokenPair,
    },
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection},
};

const CLIENT_ID: &str = "Ov23ctr1Udn5GokVCVJf";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const USER_URL: &str = "https://api.github.com/user";
const COPILOT_PROVIDER_ID: &str = "copilot";
const LEGACY_COPILOT_CONNECTION_ID: &str = "copilot";
const ACCESS_REFRESH_WINDOW: Duration = Duration::from_secs(5 * 60);
const MAX_DEVICE_CODE_BYTES: usize = 1024;
const MAX_USER_CODE_BYTES: usize = 32;
const MAX_TOKEN_BYTES: usize = 4096;
const MAX_OAUTH_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_GITHUB_USER_RESPONSE_BYTES: usize = 128 * 1024;
const MAX_ACTIVE_DEVICE_FLOWS: usize = 4;

macro_rules! stored_connection_from_row {
    ($row:expr) => {{
        let row = $row;
        Ok(StoredConnection {
            id: row.get(0)?,
            login: row.get(1)?,
            name: row.get(2)?,
            credential_reference: row.get(3)?,
            auth_status: row.get(4)?,
            is_selected: row.get::<_, i64>(5)? == 1,
        })
    }};
}

/// Deserializes a secret string directly into storage that clears it on drop.
fn deserialize_zeroizing_string<'de, D>(deserializer: D) -> Result<Zeroizing<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    String::deserialize(deserializer).map(Zeroizing::new)
}

fn deserialize_optional_zeroizing_string<'de, D>(
    deserializer: D,
) -> Result<Option<Zeroizing<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|value| value.map(Zeroizing::new))
}

pub struct CopilotOAuthService {
    http: Client,
    credentials: CredentialStore,
    flows: Mutex<HashMap<String, Arc<DeviceFlow>>>,
    start_lock: Mutex<()>,
    connection_lock: Mutex<()>,
}

struct DeviceFlow {
    device_code: Zeroizing<String>,
    verification_uri: String,
    expires_at: Instant,
    cancelled: AtomicBool,
    completion_lock: Mutex<()>,
    completion_started: AtomicBool,
    poll_state: Mutex<PollState>,
}

impl DeviceFlow {
    /// Claims the one-way connection-persistence step unless cancellation won first.
    async fn begin_completion(&self) -> Option<tokio::sync::MutexGuard<'_, ()>> {
        let guard = self.completion_lock.lock().await;
        if self.cancelled.load(Ordering::Acquire) {
            return None;
        }
        self.completion_started.store(true, Ordering::Release);
        Some(guard)
    }

    /// Cancels only while connection persistence has not started.
    async fn cancel(&self) -> bool {
        let _guard = self.completion_lock.lock().await;
        if self.completion_started.load(Ordering::Acquire) {
            return false;
        }
        self.cancelled.store(true, Ordering::Release);
        true
    }
}

struct PollState {
    interval: Duration,
    next_poll: Instant,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    #[serde(deserialize_with = "deserialize_zeroizing_string")]
    device_code: Zeroizing<String>,
    #[serde(deserialize_with = "deserialize_zeroizing_string")]
    user_code: Zeroizing<String>,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "default_poll_interval")]
    interval: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default, deserialize_with = "deserialize_optional_zeroizing_string")]
    access_token: Option<Zeroizing<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_zeroizing_string")]
    refresh_token: Option<Zeroizing<String>>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    refresh_token_expires_in: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct GitHubUser {
    id: u64,
    login: String,
    #[serde(default)]
    name: Option<String>,
}

impl CopilotOAuthService {
    pub fn new() -> Result<Self, ServiceError> {
        let http = Client::builder()
            .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .redirect(Policy::none())
            .build()
            .map_err(network_error_with_details)?;
        Ok(Self {
            http,
            credentials: CredentialStore,
            flows: Mutex::new(HashMap::new()),
            start_lock: Mutex::new(()),
            connection_lock: Mutex::new(()),
        })
    }

    pub async fn start(&self) -> Result<Value, ServiceError> {
        let start_guard = self
            .start_lock
            .try_lock()
            .map_err(|_| flow_start_in_progress())?;
        {
            let mut flows = self.flows.lock().await;
            flows.retain(|_, flow| {
                flow.expires_at > Instant::now() && !flow.cancelled.load(Ordering::Acquire)
            });
            ensure_device_flow_capacity(flows.len())?;
        }

        let response = self
            .http
            .post(DEVICE_CODE_URL)
            .header(ACCEPT, "application/json")
            .form(&[
                ("client_id", CLIENT_ID),
                ("scope", "read:user read:org offline_access"),
            ])
            .send()
            .await
            .map_err(network_error_with_details)?;
        if !response.status().is_success() {
            return Err(oauth_http_error(response.status()));
        }
        let body = read_oauth_body(response, MAX_OAUTH_RESPONSE_BYTES).await?;
        let data: DeviceCodeResponse = serde_json::from_slice(body.as_slice()).map_err(|_| {
            ServiceError::new(
                "invalid_oauth_response",
                "GitHub returned an unsupported device authorization response.",
                false,
            )
        })?;
        if data.device_code.is_empty()
            || data.device_code.len() > MAX_DEVICE_CODE_BYTES
            || data.device_code.chars().any(char::is_control)
            || data.user_code.is_empty()
            || data.user_code.len() > MAX_USER_CODE_BYTES
            || data.user_code.chars().any(char::is_control)
            || data.expires_in == 0
            || data.expires_in > 3600
            || !(1..=300).contains(&data.interval)
            || !valid_verification_uri(&data.verification_uri)
        {
            return Err(invalid_oauth_response());
        }

        let flow_id = Uuid::new_v4().simple().to_string();
        let expires_at = Instant::now() + Duration::from_secs(data.expires_in);
        let interval = Duration::from_secs(data.interval);
        let flow = Arc::new(DeviceFlow {
            device_code: data.device_code,
            verification_uri: data.verification_uri,
            expires_at,
            cancelled: AtomicBool::new(false),
            completion_lock: Mutex::new(()),
            completion_started: AtomicBool::new(false),
            poll_state: Mutex::new(PollState {
                interval,
                next_poll: Instant::now() + interval,
            }),
        });
        let result = json!({
            "flowId": flow_id,
            "userCode": data.user_code.as_str(),
            "verificationUri": flow.verification_uri,
            "expiresAtUnixMs": unix_time_millis()?.saturating_add(data.expires_in.saturating_mul(1000) as i64),
            "intervalSeconds": data.interval,
        });
        let mut flows = self.flows.lock().await;
        flows.retain(|_, flow| {
            flow.expires_at > Instant::now() && !flow.cancelled.load(Ordering::Acquire)
        });
        flows.insert(flow_id, Arc::clone(&flow));
        drop(flows);
        drop(start_guard);

        let verification_uri = flow.verification_uri.clone();
        let _ = tokio::task::spawn_blocking(move || webbrowser::open(&verification_uri)).await;
        Ok(result)
    }

    pub async fn poll(
        &self,
        storage: &AppStorage,
        flow_id: &str,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        validate_flow_id(flow_id)?;
        let flow = self
            .flows
            .lock()
            .await
            .get(flow_id)
            .cloned()
            .ok_or_else(flow_unavailable)?;
        let mut state = flow.poll_state.lock().await;
        if flow.cancelled.load(Ordering::Acquire) {
            return Err(flow_unavailable());
        }
        if Instant::now() >= flow.expires_at {
            drop(state);
            self.remove_flow(flow_id, &flow).await;
            return Err(flow_expired());
        }
        if Instant::now() < state.next_poll {
            return Ok(json!({
                "status": "pending",
                "intervalSeconds": state.interval.as_secs(),
                "retryAfterSeconds": state.next_poll.saturating_duration_since(Instant::now()).as_secs().max(1),
            }));
        }
        state.next_poll = Instant::now() + state.interval;
        let response = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Err(cancelled_error());
            }
            result = self.http.post(TOKEN_URL)
                .header(ACCEPT, "application/json")
                .form(&[
                    ("client_id", CLIENT_ID),
                    ("device_code", flow.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send() => result.map_err(network_error_with_details)?,
        };
        let status = response.status();
        let body = read_oauth_body(response, MAX_OAUTH_RESPONSE_BYTES).await?;
        let mut tokens: TokenResponse =
            serde_json::from_slice(body.as_slice()).map_err(|_| invalid_oauth_response())?;
        if let Some(error) = tokens.error.as_deref() {
            match error {
                "authorization_pending" => {
                    return Ok(json!({
                        "status": "pending",
                        "intervalSeconds": state.interval.as_secs(),
                        "retryAfterSeconds": state.interval.as_secs(),
                    }));
                }
                "slow_down" => {
                    state.interval = state.interval.saturating_add(Duration::from_secs(5));
                    state.next_poll = Instant::now() + state.interval;
                    return Ok(json!({
                        "status": "pending",
                        "intervalSeconds": state.interval.as_secs(),
                        "retryAfterSeconds": state.interval.as_secs(),
                    }));
                }
                "temporarily_unavailable" | "server_error" => {
                    state.interval = state.interval.saturating_add(Duration::from_secs(5));
                    state.next_poll = Instant::now() + state.interval;
                    return Ok(json!({
                        "status": "pending",
                        "intervalSeconds": state.interval.as_secs(),
                        "retryAfterSeconds": state.interval.as_secs(),
                    }));
                }
                "expired_token" => {
                    drop(state);
                    self.remove_flow(flow_id, &flow).await;
                    return Err(flow_expired());
                }
                "access_denied" => {
                    drop(state);
                    self.remove_flow(flow_id, &flow).await;
                    return Err(access_denied());
                }
                _ => {
                    drop(state);
                    self.remove_flow(flow_id, &flow).await;
                    return Err(oauth_http_error(status));
                }
            }
        }

        if !status.is_success() {
            drop(state);
            self.remove_flow(flow_id, &flow).await;
            return Err(oauth_http_error(status));
        }
        if flow.cancelled.load(Ordering::Acquire) {
            drop(state);
            self.remove_flow(flow_id, &flow).await;
            return Err(flow_unavailable());
        }
        let token_values = (|| {
            let access_token = tokens
                .access_token
                .take()
                .ok_or_else(invalid_oauth_response)?;
            let refresh_token = tokens.refresh_token.take().ok_or_else(|| {
                ServiceError::new(
                    "oauth_refresh_unavailable",
                    "GitHub did not issue a refresh token for this connection. Sign in again.",
                    false,
                )
            })?;
            let expires_in = tokens
                .expires_in
                .filter(|seconds| (60..=31_536_000).contains(seconds))
                .ok_or_else(invalid_oauth_response)?;
            let refresh_expires_in = tokens
                .refresh_token_expires_in
                .filter(|seconds| (60..=31_536_000).contains(seconds));
            Ok::<_, ServiceError>((access_token, refresh_token, expires_in, refresh_expires_in))
        })();
        let (access_token, refresh_token, expires_in, refresh_expires_in) = match token_values {
            Ok(values) => values,
            Err(error) => {
                drop(state);
                self.remove_flow(flow_id, &flow).await;
                return Err(error);
            }
        };
        let tokens = match expiring_tokens(access_token, refresh_token, expires_in) {
            Ok(tokens) => tokens,
            Err(error) => {
                drop(state);
                self.remove_flow(flow_id, &flow).await;
                return Err(error);
            }
        };
        let identity = match self.fetch_identity(tokens.access_token()).await {
            Ok(identity) => identity,
            Err(error) => {
                drop(state);
                self.remove_flow(flow_id, &flow).await;
                return Err(error);
            }
        };
        if flow.cancelled.load(Ordering::Acquire) {
            drop(state);
            self.remove_flow(flow_id, &flow).await;
            return Err(flow_unavailable());
        }
        let Some(_completion_guard) = flow.begin_completion().await else {
            drop(state);
            self.remove_flow(flow_id, &flow).await;
            return Err(flow_unavailable());
        };
        let persisted_connection = {
            let _connection_guard = self.connection_lock.lock().await;
            self.persist_connection(storage, tokens, refresh_expires_in, identity)
        };
        let connection = match persisted_connection {
            Ok(connection) => connection,
            Err(error) => {
                drop(state);
                self.remove_flow(flow_id, &flow).await;
                return Err(error);
            }
        };
        drop(state);
        self.remove_flow(flow_id, &flow).await;
        Ok(json!({ "status": "connected", "connection": connection }))
    }

    pub async fn cancel(&self, flow_id: &str) -> Result<Value, ServiceError> {
        validate_flow_id(flow_id)?;
        let flow = self.flows.lock().await.get(flow_id).cloned();
        let cancelled = if let Some(flow) = flow {
            let cancelled = flow.cancel().await;
            if cancelled {
                self.remove_flow(flow_id, &flow).await;
            }
            cancelled
        } else {
            false
        };
        Ok(json!({ "cancelled": cancelled }))
    }

    pub fn status(&self, storage: &AppStorage) -> Result<Value, ServiceError> {
        let connections = load_connections(storage)?;
        let selected = connections.iter().find(|connection| connection.is_selected);
        let public_connections = connections
            .iter()
            .map(|connection| connection.to_public_json())
            .collect::<Vec<_>>();
        Ok(json!({
            "connected": selected.is_some(),
            "connection": selected.map(StoredConnection::to_public_json),
            "connections": public_connections,
            "selectedConnectionId": selected.map(|connection| connection.id.as_str()),
        }))
    }

    pub fn select(&self, storage: &AppStorage, connection_id: &str) -> Result<Value, ServiceError> {
        validate_connection_id(connection_id)?;
        let mut connection = storage.connect().map_err(database_error)?;
        let tx = connection.unchecked_transaction().map_err(database_error)?;
        tx.execute(
            "UPDATE copilot_oauth_connection SET is_selected = 0 WHERE is_selected = 1",
            [],
        )
        .map_err(database_error)?;
        let changed = tx
            .execute(
                "UPDATE copilot_oauth_connection SET is_selected = 1 WHERE id = ?1",
                [connection_id],
            )
            .map_err(database_error)?;
        if changed != 1 {
            return Err(connection_not_found());
        }
        tx.commit().map_err(database_error)?;
        Ok(json!({ "selectedConnectionId": connection_id }))
    }

    pub async fn disconnect(
        &self,
        storage: &AppStorage,
        connection_id: &str,
    ) -> Result<Value, ServiceError> {
        validate_connection_id(connection_id)?;
        let _guard = self.connection_lock.lock().await;
        let mut connection = storage.connect().map_err(database_error)?;
        disconnect_connection(&mut connection, connection_id, |reference| {
            self.credentials
                .delete_token_pair(storage, reference)
                .map_err(credential_error)
        })
    }

    pub async fn access_token(
        &self,
        storage: &AppStorage,
    ) -> Result<Zeroizing<String>, ServiceError> {
        let connection = load_connection(storage)?.ok_or_else(authentication_required)?;
        if connection.auth_status != "active" {
            return Err(authentication_required());
        }
        let reference = parse_reference(&connection.credential_reference)?;
        let tokens = self
            .credentials
            .load_token_pair(storage, &reference)
            .map_err(credential_error)?;
        if !tokens.expires_within(SystemTime::now(), ACCESS_REFRESH_WINDOW) {
            return Ok(Zeroizing::new(tokens.access_token().to_owned()));
        }
        let _guard = self.connection_lock.lock().await;
        let current =
            load_connection_by_id(storage, &connection.id)?.ok_or_else(authentication_required)?;
        if current.auth_status != "active" {
            return Err(authentication_required());
        }
        let current_reference = parse_reference(&current.credential_reference)?;
        let current_tokens = self
            .credentials
            .load_token_pair(storage, &current_reference)
            .map_err(credential_error)?;
        if !current_tokens.expires_within(SystemTime::now(), ACCESS_REFRESH_WINDOW) {
            return Ok(Zeroizing::new(current_tokens.access_token().to_owned()));
        }
        self.refresh(storage, current, current_tokens).await
    }

    async fn refresh(
        &self,
        storage: &AppStorage,
        connection: StoredConnection,
        old_tokens: OAuthTokenPair,
    ) -> Result<Zeroizing<String>, ServiceError> {
        let response = self
            .http
            .post(TOKEN_URL)
            .header(ACCEPT, "application/json")
            .form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", old_tokens.refresh_token()),
            ])
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                set_auth_status(storage, &connection.id, "reauth_required")?;
                return Err(network_error_with_details(error));
            }
        };
        let status = response.status();
        let body = match read_oauth_body(response, MAX_OAUTH_RESPONSE_BYTES).await {
            Ok(body) => body,
            Err(error) => {
                set_auth_status(storage, &connection.id, "reauth_required")?;
                return Err(error);
            }
        };
        let mut result: TokenResponse = match serde_json::from_slice(body.as_slice()) {
            Ok(result) => result,
            Err(_) => {
                set_auth_status(storage, &connection.id, "reauth_required")?;
                return Err(invalid_oauth_response());
            }
        };
        if let Some(error) = result.error.as_deref() {
            if matches!(
                error,
                "invalid_grant" | "bad_refresh_token" | "expired_token"
            ) {
                set_auth_status(storage, &connection.id, "reauth_required")?;
                return Err(authentication_required());
            }
            if matches!(error, "temporarily_unavailable" | "server_error") {
                return Err(temporary_oauth_error());
            }
            set_auth_status(storage, &connection.id, "reauth_required")?;
            return Err(oauth_http_error(status));
        }
        if !status.is_success() {
            if status == StatusCode::UNAUTHORIZED
                || status == StatusCode::FORBIDDEN
                || status == StatusCode::BAD_REQUEST
            {
                set_auth_status(storage, &connection.id, "reauth_required")?;
                return Err(authentication_required());
            }
            return Err(oauth_http_error(status));
        }
        let Some(access_token) = result.access_token.take() else {
            set_auth_status(storage, &connection.id, "reauth_required")?;
            return Err(invalid_oauth_response());
        };
        let Some(refresh_token) = result.refresh_token.take() else {
            set_auth_status(storage, &connection.id, "reauth_required")?;
            return Err(ServiceError::new(
                "oauth_refresh_unavailable",
                "GitHub did not rotate the refresh token. Reconnect your account before continuing.",
                false,
            ));
        };
        let Some(expires_in) = result
            .expires_in
            .filter(|seconds| (60..=31_536_000).contains(seconds))
        else {
            set_auth_status(storage, &connection.id, "reauth_required")?;
            return Err(invalid_oauth_response());
        };
        let refresh_expires_in = result
            .refresh_token_expires_in
            .filter(|seconds| (60..=31_536_000).contains(seconds));
        let new_tokens = expiring_tokens(access_token, refresh_token, expires_in)?;
        let new_reference = new_credential_reference(&connection.id)?;
        self.credentials
            .store_new_token_pair(storage, &new_reference, &new_tokens)
            .map_err(credential_error)?;
        let previous_reference = match replace_credential_reference(
            storage,
            &connection.id,
            &connection.credential_reference,
            &new_reference,
            new_tokens
                .expires_at_unix_seconds()
                .ok_or_else(invalid_oauth_response)?,
            refresh_expires_in,
        ) {
            Ok(Some(reference)) => reference,
            Ok(None) => {
                let _ = self.credentials.delete_token_pair(storage, &new_reference);
                return Err(authentication_required());
            }
            Err(error) => {
                let _ = self.credentials.delete_token_pair(storage, &new_reference);
                return Err(error);
            }
        };
        let cleanup_result = parse_reference(&previous_reference).and_then(|reference| {
            self.credentials
                .delete_token_pair(storage, &reference)
                .map_err(credential_error)
        });
        if cleanup_result.is_err() {
            eprintln!("copilot_oauth_rotated_credential_cleanup_failed");
        }
        Ok(Zeroizing::new(new_tokens.access_token().to_owned()))
    }

    async fn fetch_identity(&self, access_token: &str) -> Result<GitHubIdentity, ServiceError> {
        let response = self
            .http
            .get(USER_URL)
            .bearer_auth(access_token)
            .header(ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(network_error_with_details)?;
        if !response.status().is_success() {
            return Err(oauth_http_error(response.status()));
        }
        let body = read_oauth_body(response, MAX_GITHUB_USER_RESPONSE_BYTES).await?;
        let mut user: GitHubUser =
            serde_json::from_slice(body.as_slice()).map_err(|_| invalid_oauth_response())?;
        if user.id == 0
            || user.login.is_empty()
            || user.login.len() > 256
            || user.login.chars().any(char::is_control)
        {
            if let Some(name) = &mut user.name {
                name.zeroize();
            }
            return Err(invalid_oauth_response());
        }
        let name = user.name.take().filter(|value| {
            !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
        });
        Ok(GitHubIdentity {
            id: user.id.to_string(),
            login: user.login,
            name,
        })
    }

    fn persist_connection(
        &self,
        storage: &AppStorage,
        tokens: OAuthTokenPair,
        refresh_expires_in: Option<u64>,
        identity: GitHubIdentity,
    ) -> Result<Value, ServiceError> {
        if tokens.access_token().len() > MAX_TOKEN_BYTES
            || tokens.refresh_token().len() > MAX_TOKEN_BYTES
        {
            return Err(invalid_oauth_response());
        }
        let reference = new_credential_reference(&identity.id)?;
        self.credentials
            .store_new_token_pair(storage, &reference, &tokens)
            .map_err(credential_error)?;
        let persisted = (|| {
            let mut connection = storage.connect().map_err(database_error)?;
            let tx = connection.unchecked_transaction().map_err(database_error)?;
            let previous_reference: Option<String> = tx
                .query_row(
                    "SELECT credential_reference FROM copilot_oauth_connection WHERE github_user_id = ?1",
                    [&identity.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(database_error)?;
            let now = unix_time_millis()?;
            let access_expires_at = tokens
                .expires_at_unix_seconds()
                .ok_or_else(invalid_oauth_response)?;
            let access_expires_at = unix_seconds_for_storage(access_expires_at)?;
            let refresh_expires_at = refresh_expiration_for_storage(refresh_expires_in)?;
            tx.execute(
                "INSERT INTO copilot_oauth_connection (
                    id, github_user_id, login, display_name, credential_reference,
                    auth_status, access_expires_at_unix_seconds, refresh_expires_at_unix_seconds,
                    connected_at_unix_ms, updated_at_unix_ms, is_selected
                 ) VALUES (?1, ?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7, ?7,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM copilot_oauth_connection WHERE is_selected = 1
                    ) THEN 0 ELSE 1 END)
                 ON CONFLICT(github_user_id) DO UPDATE SET
                    login = excluded.login,
                    display_name = excluded.display_name,
                    credential_reference = excluded.credential_reference,
                    auth_status = 'active',
                    access_expires_at_unix_seconds = excluded.access_expires_at_unix_seconds,
                    refresh_expires_at_unix_seconds = excluded.refresh_expires_at_unix_seconds,
                    is_selected = CASE WHEN EXISTS (
                        SELECT 1 FROM copilot_oauth_connection
                        WHERE is_selected = 1 AND id <> excluded.id
                    ) THEN copilot_oauth_connection.is_selected ELSE 1 END,
                    updated_at_unix_ms = excluded.updated_at_unix_ms",
                rusqlite::params![
                    identity.id,
                    identity.login,
                    identity.name,
                    reference_name(&reference),
                    access_expires_at,
                    refresh_expires_at,
                    now,
                ],
            )
            .map_err(database_error)?;
            tx.commit().map_err(database_error)?;
            Ok::<_, ServiceError>(previous_reference)
        })();
        let previous_reference = match persisted {
            Ok(reference) => reference,
            Err(error) => {
                if self
                    .credentials
                    .delete_token_pair(storage, &reference)
                    .is_err()
                {
                    eprintln!("copilot_oauth_new_credential_cleanup_failed");
                }
                return Err(error);
            }
        };
        if let Some(old_reference) = previous_reference.as_deref()
            && old_reference != reference_name(&reference)
        {
            let cleanup_result = parse_reference(old_reference).and_then(|old_reference| {
                self.credentials
                    .delete_token_pair(storage, &old_reference)
                    .map_err(credential_error)
            });
            if cleanup_result.is_err() {
                eprintln!("copilot_oauth_old_credential_cleanup_failed");
            }
        }
        Ok(json!({
            "id": identity.id,
            "login": identity.login,
            "name": identity.name,
            "authStatus": "active",
        }))
    }

    async fn remove_flow(&self, flow_id: &str, flow: &Arc<DeviceFlow>) {
        let mut flows = self.flows.lock().await;
        if flows
            .get(flow_id)
            .is_some_and(|current| Arc::ptr_eq(current, flow))
        {
            flows.remove(flow_id);
        }
    }
}

struct GitHubIdentity {
    id: String,
    login: String,
    name: Option<String>,
}

struct StoredConnection {
    id: String,
    login: String,
    name: Option<String>,
    credential_reference: String,
    auth_status: String,
    is_selected: bool,
}

fn load_connection(storage: &AppStorage) -> Result<Option<StoredConnection>, ServiceError> {
    storage
        .connect()
        .map_err(database_error)?
        .query_row(
            "SELECT id, login, display_name, credential_reference,
                    auth_status, is_selected
             FROM copilot_oauth_connection WHERE is_selected = 1",
            [],
            |row| stored_connection_from_row!(row),
        )
        .optional()
        .map_err(database_error)
}

fn load_connection_by_id(
    storage: &AppStorage,
    connection_id: &str,
) -> Result<Option<StoredConnection>, ServiceError> {
    storage
        .connect()
        .map_err(database_error)?
        .query_row(
            "SELECT id, login, display_name, credential_reference,
                    auth_status, is_selected
             FROM copilot_oauth_connection WHERE id = ?1",
            [connection_id],
            |row| stored_connection_from_row!(row),
        )
        .optional()
        .map_err(database_error)
}

fn load_connections(storage: &AppStorage) -> Result<Vec<StoredConnection>, ServiceError> {
    let connection = storage.connect().map_err(database_error)?;
    let mut statement = connection
        .prepare(
            "SELECT id, login, display_name, credential_reference,
                    auth_status, is_selected
             FROM copilot_oauth_connection
             ORDER BY connected_at_unix_ms DESC, id",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([], |row| stored_connection_from_row!(row))
        .map_err(database_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(database_error)
}

impl StoredConnection {
    fn to_public_json(&self) -> Value {
        json!({
            "id": self.id,
            "login": self.login,
            "name": self.name,
            "authStatus": self.auth_status,
        })
    }
}

/// Removes the keyring pair before committing its database reference deletion.
///
/// Keeping the reference row until credential cleanup succeeds lets a later disconnect retry
/// partially completed Windows Credential Manager cleanup.
fn disconnect_connection(
    connection: &mut StorageConnection<'_>,
    connection_id: &str,
    remove_credentials: impl FnOnce(&OAuthCredentialReference) -> Result<(), ServiceError>,
) -> Result<Value, ServiceError> {
    let transaction = connection.transaction().map_err(database_error)?;
    let stored_account: Option<(String, String, bool)> = transaction
        .query_row(
            "SELECT credential_reference, github_user_id, is_selected
             FROM copilot_oauth_connection WHERE id = ?1",
            [connection_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? == 1)),
        )
        .optional()
        .map_err(database_error)?;
    let reference = stored_account
        .as_ref()
        .map(|(reference, _, _)| reference.as_str())
        .map(parse_reference)
        .transpose()?;

    if let Some(reference) = reference.as_ref() {
        remove_credentials(reference)?;
    }

    let removed = if let Some((stored_reference, github_user_id, was_selected)) = stored_account {
        let deleted_reference: Option<String> = transaction
            .query_row(
                "DELETE FROM copilot_oauth_connection
                 WHERE id = ?1 AND credential_reference = ?2
                 RETURNING credential_reference",
                rusqlite::params![connection_id, stored_reference],
                |row| row.get(0),
            )
            .optional()
            .map_err(database_error)?;
        if deleted_reference.is_none() {
            return Err(ServiceError::new(
                "connection_changed",
                "The GitHub Copilot connection changed while it was being removed. Try again.",
                true,
            ));
        }
        if was_selected {
            transaction
                .execute(
                    "UPDATE copilot_oauth_connection SET is_selected = 1
                     WHERE id = (
                        SELECT id FROM copilot_oauth_connection
                        ORDER BY (auth_status = 'active') DESC, connected_at_unix_ms DESC, id
                        LIMIT 1
                     )",
                    [],
                )
                .map_err(database_error)?;
        }
        transaction
            .execute(
                "DELETE FROM compatible_provider_model_catalog
                 WHERE provider_id = ?1 AND api_key_hash = ?2",
                rusqlite::params![COPILOT_PROVIDER_ID, account_hash(&github_user_id)],
            )
            .map_err(database_error)?;
        true
    } else {
        false
    };
    transaction.commit().map_err(database_error)?;
    Ok(json!({ "removed": removed }))
}

fn replace_credential_reference(
    storage: &AppStorage,
    connection_id: &str,
    old_reference: &str,
    new_reference: &OAuthCredentialReference,
    access_expires_at: u64,
    refresh_expires_in: Option<u64>,
) -> Result<Option<String>, ServiceError> {
    let mut connection = storage.connect().map_err(database_error)?;
    let tx = connection.unchecked_transaction().map_err(database_error)?;
    let previous: Option<String> = tx
        .query_row(
            "SELECT credential_reference FROM copilot_oauth_connection WHERE id = ?1 AND credential_reference = ?2 AND auth_status = 'active'",
            rusqlite::params![connection_id, old_reference],
            |row| row.get(0),
        )
        .optional()
        .map_err(database_error)?;
    let Some(previous) = previous else {
        return Ok(None);
    };
    let access_expires_at = unix_seconds_for_storage(access_expires_at)?;
    let refresh_expires_at = refresh_expiration_for_storage(refresh_expires_in)?;
    let changed = tx
        .execute(
            "UPDATE copilot_oauth_connection SET credential_reference = ?1,
            access_expires_at_unix_seconds = ?2, refresh_expires_at_unix_seconds = ?3,
            updated_at_unix_ms = ?4
         WHERE id = ?5 AND credential_reference = ?6 AND auth_status = 'active'",
            rusqlite::params![
                reference_name(new_reference),
                access_expires_at,
                refresh_expires_at,
                unix_time_millis()?,
                connection_id,
                old_reference,
            ],
        )
        .map_err(database_error)?;
    tx.commit().map_err(database_error)?;
    if changed == 1 {
        Ok(Some(previous))
    } else {
        Ok(None)
    }
}

fn set_auth_status(
    storage: &AppStorage,
    connection_id: &str,
    status: &str,
) -> Result<(), ServiceError> {
    storage
        .connect()
        .map_err(database_error)?
        .execute(
            "UPDATE copilot_oauth_connection SET auth_status = ?1, updated_at_unix_ms = ?2 WHERE id = ?3",
            rusqlite::params![status, unix_time_millis()?, connection_id],
        )
        .map_err(database_error)?;
    Ok(())
}

fn parse_reference(value: &str) -> Result<OAuthCredentialReference, ServiceError> {
    let Some((connection_id, generation_id)) = value.split_once(':') else {
        return Err(authentication_required());
    };
    if !valid_credential_connection_id(connection_id) || generation_id.contains(':') {
        return Err(authentication_required());
    }
    OAuthCredentialReference::new_for_github_copilot(connection_id, generation_id)
        .map_err(credential_error)
}

fn valid_credential_connection_id(value: &str) -> bool {
    value == LEGACY_COPILOT_CONNECTION_ID
        || (!value.is_empty()
            && value.len() <= 20
            && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn validate_connection_id(value: &str) -> Result<(), ServiceError> {
    if !value.is_empty() && value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(())
    } else {
        Err(connection_not_found())
    }
}

fn connection_not_found() -> ServiceError {
    ServiceError::new(
        "copilot_connection_not_found",
        "The selected GitHub Copilot account is no longer connected.",
        false,
    )
}

fn account_hash(github_user_id: &str) -> String {
    let digest = Sha256::digest(github_user_id.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn reference_name(reference: &OAuthCredentialReference) -> String {
    format!(
        "{}:{}",
        reference.connection_id(),
        reference.generation_id()
    )
}

fn new_credential_reference(connection_id: &str) -> Result<OAuthCredentialReference, ServiceError> {
    OAuthCredentialReference::new_for_github_copilot(
        connection_id,
        Uuid::new_v4().simple().to_string(),
    )
    .map_err(credential_error)
}

fn expiring_tokens(
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    expires_in: u64,
) -> Result<OAuthTokenPair, ServiceError> {
    let expires_at = unix_time_seconds()?
        .checked_add(expires_in)
        .ok_or_else(invalid_oauth_response)?;
    OAuthTokenPair::with_expiration(access_token, refresh_token, expires_at)
        .map_err(credential_error)
}

fn unix_time_seconds() -> Result<u64, ServiceError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .map_err(|_| {
            ServiceError::new(
                "clock_unavailable",
                "The local clock is unavailable.",
                false,
            )
        })
}

fn unix_seconds_for_storage(seconds: u64) -> Result<i64, ServiceError> {
    i64::try_from(seconds).map_err(|_| invalid_oauth_response())
}

fn refresh_expiration_for_storage(expires_in: Option<u64>) -> Result<Option<i64>, ServiceError> {
    expires_in
        .map(|seconds| {
            let expiration = unix_time_seconds()?
                .checked_add(seconds)
                .ok_or_else(invalid_oauth_response)?;
            unix_seconds_for_storage(expiration)
        })
        .transpose()
}

fn unix_time_millis() -> Result<i64, ServiceError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis().min(i64::MAX as u128) as i64)
        .map_err(|_| {
            ServiceError::new(
                "clock_unavailable",
                "The local clock is unavailable.",
                false,
            )
        })
}

fn default_poll_interval() -> u64 {
    5
}

async fn read_oauth_body(
    mut response: reqwest::Response,
    maximum_bytes: usize,
) -> Result<Zeroizing<Vec<u8>>, ServiceError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(invalid_oauth_response());
    }
    let capacity = response
        .content_length()
        .map(|length| (length as usize).min(maximum_bytes))
        .unwrap_or_default();
    let mut body = Zeroizing::new(Vec::with_capacity(capacity));
    while let Some(chunk) = response.chunk().await.map_err(network_error_with_details)? {
        if body.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(invalid_oauth_response());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn valid_verification_uri(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url.path() == "/login/device"
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

fn validate_flow_id(value: &str) -> Result<(), ServiceError> {
    if value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(flow_unavailable())
    }
}

fn ensure_device_flow_capacity(active_flows: usize) -> Result<(), ServiceError> {
    if active_flows < MAX_ACTIVE_DEVICE_FLOWS {
        Ok(())
    } else {
        Err(flow_limit_reached())
    }
}

fn network_error() -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        "GitHub could not be reached. Check your connection and try again.",
        true,
    )
}

fn network_error_with_details(error: reqwest::Error) -> ServiceError {
    network_error().with_details(error.without_url().to_string())
}

fn temporary_oauth_error() -> ServiceError {
    ServiceError::new(
        "oauth_service_unavailable",
        "GitHub authorization is temporarily unavailable. Try again later.",
        true,
    )
}

fn oauth_http_error(status: StatusCode) -> ServiceError {
    let (code, message, retryable) = if status == StatusCode::UNAUTHORIZED
        || status == StatusCode::FORBIDDEN
        || status == StatusCode::BAD_REQUEST
    {
        (
            "oauth_authorization_rejected",
            "GitHub rejected this authorization. Start a new sign-in attempt.",
            false,
        )
    } else if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        (
            "oauth_service_unavailable",
            "GitHub authorization is temporarily unavailable. Try again later.",
            true,
        )
    } else {
        (
            "oauth_request_failed",
            "GitHub authorization could not be completed.",
            false,
        )
    };
    ServiceError::new(code, message, retryable)
        .with_details(format!("HTTP status {}", status.as_u16()))
}

fn invalid_oauth_response() -> ServiceError {
    ServiceError::new(
        "invalid_oauth_response",
        "GitHub returned an unsupported authorization response.",
        false,
    )
}

fn flow_unavailable() -> ServiceError {
    ServiceError::new(
        "oauth_flow_unavailable",
        "The GitHub sign-in request is no longer available. Start again.",
        false,
    )
}

fn flow_start_in_progress() -> ServiceError {
    ServiceError::new(
        "oauth_flow_start_in_progress",
        "A GitHub sign-in request is already starting. Try again in a moment.",
        true,
    )
}

fn flow_limit_reached() -> ServiceError {
    ServiceError::new(
        "oauth_flow_limit_reached",
        "Too many GitHub sign-in requests are active. Cancel one before starting another.",
        false,
    )
}

fn flow_expired() -> ServiceError {
    ServiceError::new(
        "oauth_flow_expired",
        "The GitHub device code expired. Start sign-in again.",
        false,
    )
}

fn access_denied() -> ServiceError {
    ServiceError::new(
        "oauth_access_denied",
        "GitHub sign-in was declined. Start again if you want to connect this account.",
        false,
    )
}

fn authentication_required() -> ServiceError {
    ServiceError::new(
        "copilot_authentication_required",
        "Connect or reconnect your GitHub account to use Copilot.",
        false,
    )
}

fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "The sign-in request was cancelled.",
        false,
    )
}

fn database_error(_: DatabaseError) -> ServiceError {
    ServiceError::new(
        "local_storage_failed",
        "The GitHub Copilot connection could not be read or saved locally.",
        true,
    )
}

fn credential_error(error: CredentialStoreError) -> ServiceError {
    let (code, message, retryable) = match error {
        CredentialStoreError::InvalidReference => (
            "credential_reference_invalid",
            "The saved GitHub Copilot credential reference is invalid. Reconnect the account.",
            false,
        ),
        CredentialStoreError::EmptyToken
        | CredentialStoreError::TokenTooLarge
        | CredentialStoreError::InvalidStoredToken => (
            "oauth_credential_invalid",
            "The saved GitHub Copilot credentials are invalid. Reconnect the account.",
            false,
        ),
        CredentialStoreError::ReferenceAlreadyUsed => (
            "credential_reference_in_use",
            "A new GitHub Copilot credential reference is already in use. Try again.",
            true,
        ),
        CredentialStoreError::TokenPairMissing | CredentialStoreError::IncompleteTokenPair => (
            "oauth_tokens_missing",
            "The saved GitHub Copilot tokens are unavailable. Reconnect the account.",
            false,
        ),
        CredentialStoreError::IncompleteWrite => (
            "oauth_credential_write_incomplete",
            "The GitHub Copilot credentials could not be stored completely.",
            true,
        ),
        CredentialStoreError::StorageUnavailable
        | CredentialStoreError::StorageReadFailed
        | CredentialStoreError::StorageWriteFailed
        | CredentialStoreError::StorageDeleteFailed
        | CredentialStoreError::LockUnavailable => (
            "secure_storage_unavailable",
            "The protected credential storage is unavailable.",
            true,
        ),
    };
    ServiceError::new(code, message, retryable)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, atomic::AtomicBool},
        time::{Duration, Instant},
    };

    use rusqlite::Connection;
    use tokio::sync::{Barrier, Mutex};
    use zeroize::Zeroizing;

    use crate::protocol::ServiceError;
    use crate::storage::StorageConnection;

    use super::{
        COPILOT_PROVIDER_ID, DeviceCodeResponse, DeviceFlow, MAX_ACTIVE_DEVICE_FLOWS, PollState,
        TokenResponse, account_hash, disconnect_connection, ensure_device_flow_capacity,
        invalid_oauth_response, refresh_expiration_for_storage, unix_seconds_for_storage,
    };

    fn test_connection() -> StorageConnection<'static> {
        let connection = Connection::open_in_memory().expect("open in-memory test database");
        connection
            .execute_batch(
                "CREATE TABLE copilot_oauth_connection (
                    id TEXT PRIMARY KEY NOT NULL,
                    github_user_id TEXT NOT NULL,
                    credential_reference TEXT NOT NULL UNIQUE,
                    is_selected INTEGER NOT NULL DEFAULT 0,
                    auth_status TEXT NOT NULL DEFAULT 'active',
                    connected_at_unix_ms INTEGER NOT NULL DEFAULT 1
                 );
                 CREATE TABLE compatible_provider_model_catalog (
                    provider_id TEXT NOT NULL,
                    api_key_hash TEXT NOT NULL,
                    fetched_at_unix_ms INTEGER NOT NULL,
                    models_json TEXT NOT NULL,
                    PRIMARY KEY (provider_id, api_key_hash)
                 );",
            )
            .expect("create test connection tables");
        StorageConnection::standalone(connection)
    }

    fn insert_connection(connection: &StorageConnection<'_>, credential_reference: &str) {
        connection
            .execute(
                "INSERT INTO copilot_oauth_connection (
                    id, github_user_id, credential_reference, is_selected
                ) VALUES ('123', '123', ?1, 1)",
                [credential_reference],
            )
            .expect("insert test Copilot connection");
        connection
            .execute(
                "INSERT INTO compatible_provider_model_catalog (
                    provider_id, api_key_hash, fetched_at_unix_ms, models_json
                ) VALUES (?1, ?2, 1, '[]')",
                rusqlite::params![COPILOT_PROVIDER_ID, account_hash("123")],
            )
            .expect("insert test model catalog");
    }

    fn row_count(connection: &StorageConnection<'_>, table: &str) -> i64 {
        let query = format!("SELECT COUNT(*) FROM {table}");
        connection
            .query_row(&query, [], |row| row.get(0))
            .expect("count test rows")
    }

    #[test]
    fn disconnect_keeps_connection_and_catalog_when_credential_removal_fails() {
        let mut connection = test_connection();
        insert_connection(&connection, "123:0123456789abcdef0123456789abcdef");

        let error = disconnect_connection(&mut connection, "123", |_| {
            Err(ServiceError::new(
                "secure_storage_unavailable",
                "Credential cleanup failed.",
                true,
            ))
        })
        .expect_err("credential cleanup failure should abort disconnect");

        assert_eq!(error.code, "secure_storage_unavailable");
        assert_eq!(row_count(&connection, "copilot_oauth_connection"), 1);
        assert_eq!(
            row_count(&connection, "compatible_provider_model_catalog"),
            1
        );

        let result = disconnect_connection(&mut connection, "123", |_| Ok(()))
            .expect("a later disconnect should retry credential cleanup");

        assert_eq!(result["removed"], true);
        assert_eq!(row_count(&connection, "copilot_oauth_connection"), 0);
        assert_eq!(
            row_count(&connection, "compatible_provider_model_catalog"),
            0
        );
    }

    #[test]
    fn disconnect_rejects_invalid_reference_without_removing_connection() {
        let mut connection = test_connection();
        insert_connection(&connection, "invalid-reference");

        let error = disconnect_connection(&mut connection, "123", |_| {
            Err(ServiceError::new(
                "unexpected_cleanup",
                "Credential cleanup should not run for an invalid reference.",
                false,
            ))
        })
        .expect_err("invalid stored reference should abort disconnect");

        assert_eq!(error.code, "copilot_authentication_required");
        assert_eq!(row_count(&connection, "copilot_oauth_connection"), 1);
        assert_eq!(
            row_count(&connection, "compatible_provider_model_catalog"),
            1
        );
    }

    #[test]
    fn disconnect_removes_connection_and_catalog_after_credential_cleanup() {
        let mut connection = test_connection();
        insert_connection(&connection, "123:0123456789abcdef0123456789abcdef");

        let result = disconnect_connection(&mut connection, "123", |reference| {
            assert_eq!(reference.connection_id(), "123");
            Ok(())
        })
        .expect("successful credential cleanup should permit disconnect");

        assert_eq!(result["removed"], true);
        assert_eq!(row_count(&connection, "copilot_oauth_connection"), 0);
        assert_eq!(
            row_count(&connection, "compatible_provider_model_catalog"),
            0
        );
    }

    fn test_device_flow() -> DeviceFlow {
        let now = Instant::now();
        DeviceFlow {
            device_code: Zeroizing::new("device-code".to_owned()),
            verification_uri: "https://github.com/login/device".to_owned(),
            expires_at: now + Duration::from_secs(600),
            cancelled: AtomicBool::new(false),
            completion_lock: Mutex::new(()),
            completion_started: AtomicBool::new(false),
            poll_state: Mutex::new(PollState {
                interval: Duration::from_secs(5),
                next_poll: now,
            }),
        }
    }

    #[test]
    fn device_code_response_keeps_codes_in_zeroizing_storage() {
        let response: DeviceCodeResponse = serde_json::from_str(
            r#"{"device_code":"device-secret","user_code":"user-secret","verification_uri":"https://github.com/login/device","expires_in":600,"interval":5}"#,
        )
        .expect("GitHub device authorization response should deserialize");

        assert_eq!(response.device_code.as_str(), "device-secret");
        assert_eq!(response.user_code.as_str(), "user-secret");
    }

    #[test]
    fn token_response_deserializes_secrets_into_zeroizing_storage() {
        let response: TokenResponse = serde_json::from_str(
            r#"{"access_token":"access-secret","refresh_token":"refresh-secret","expires_in":3600}"#,
        )
        .expect("GitHub token response should deserialize");

        assert_eq!(
            response.access_token.as_deref().map(String::as_str),
            Some("access-secret")
        );
        assert_eq!(
            response.refresh_token.as_deref().map(String::as_str),
            Some("refresh-secret")
        );
    }

    #[test]
    fn device_flow_capacity_accepts_the_limit_and_rejects_one_more() {
        assert!(ensure_device_flow_capacity(MAX_ACTIVE_DEVICE_FLOWS - 1).is_ok());
        let error = ensure_device_flow_capacity(MAX_ACTIVE_DEVICE_FLOWS)
            .expect_err("the active sign-in flow limit must be enforced");

        assert_eq!(error.code, "oauth_flow_limit_reached");
    }

    #[test]
    fn sqlite_expiration_seconds_reject_values_outside_signed_integer_range() {
        assert_eq!(
            unix_seconds_for_storage(1_900_000_000).unwrap(),
            1_900_000_000
        );
        assert_eq!(
            unix_seconds_for_storage(u64::MAX).unwrap_err().code,
            invalid_oauth_response().code
        );
    }

    #[test]
    fn refresh_expiration_remains_absent_when_github_does_not_supply_it() {
        assert_eq!(refresh_expiration_for_storage(None).unwrap(), None);
    }

    #[tokio::test]
    async fn cancellation_and_connection_persistence_have_exactly_one_winner() {
        for _ in 0..64 {
            let flow = Arc::new(test_device_flow());
            let barrier = Arc::new(Barrier::new(3));

            let cancel_flow = Arc::clone(&flow);
            let cancel_barrier = Arc::clone(&barrier);
            let cancel = tokio::spawn(async move {
                cancel_barrier.wait().await;
                cancel_flow.cancel().await
            });

            let completion_flow = Arc::clone(&flow);
            let completion_barrier = Arc::clone(&barrier);
            let completion = tokio::spawn(async move {
                completion_barrier.wait().await;
                completion_flow.begin_completion().await.is_some()
            });

            barrier.wait().await;
            let cancelled = cancel.await.expect("cancellation task should finish");
            let completion_started = completion
                .await
                .expect("connection completion task should finish");

            assert_ne!(
                cancelled, completion_started,
                "cancellation and persistence must not both win or lose"
            );
        }
    }
}
