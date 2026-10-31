//! Stores provider OAuth token pairs and project-scoped MCP secrets in the protected local vault.
//!
//! Legacy Windows Credential Manager entries are imported on read. References contain only
//! validated identifiers, and secret values use zeroizing allocations.

use std::{
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use keyring::{Entry, Error as KeyringError};
use serde::Deserialize;
use zeroize::Zeroizing;

const CHATGPT_CREDENTIAL_SERVICE: &str = "OpenChat.ChatGPT.OAuth";
const COPILOT_CREDENTIAL_SERVICE: &str = "OpenChat.GitHubCopilot.OAuth";
const MCP_CREDENTIAL_SERVICE: &str = "OpenChat.MCP";
const MAX_CREDENTIAL_BYTES: usize = 2560;
const MAX_REFERENCE_PART_BYTES: usize = 128;
const MAX_MCP_SECRET_BYTES: usize = 2500;
static OPERATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Eq, PartialEq)]
/// A validated, non-secret key that identifies one provider OAuth token pair.
pub struct OAuthCredentialReference {
    connection_id: String,
    generation_id: String,
    service: OAuthCredentialService,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OAuthCredentialService {
    ChatGpt,
    GitHubCopilot,
}

impl OAuthCredentialService {
    fn keyring_name(self) -> &'static str {
        match self {
            Self::ChatGpt => CHATGPT_CREDENTIAL_SERVICE,
            Self::GitHubCopilot => COPILOT_CREDENTIAL_SERVICE,
        }
    }
}

impl OAuthCredentialReference {
    /// Creates a ChatGPT credential reference from bounded ASCII identifiers.
    ///
    /// Both identifiers must be non-empty and contain only ASCII letters,
    /// digits, hyphens, or underscores. Each is limited to 128 bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::InvalidReference`] if either identifier
    /// violates those constraints.
    pub fn new(
        connection_id: impl Into<String>,
        generation_id: impl Into<String>,
    ) -> Result<Self, CredentialStoreError> {
        let connection_id = connection_id.into();
        let generation_id = generation_id.into();
        if !is_valid_reference_part(&connection_id) || !is_valid_reference_part(&generation_id) {
            return Err(CredentialStoreError::InvalidReference);
        }

        Ok(Self {
            connection_id,
            generation_id,
            service: OAuthCredentialService::ChatGpt,
        })
    }

    /// Creates a GitHub Copilot credential reference with the same validation
    /// rules as [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::InvalidReference`] if either identifier
    /// violates those constraints.
    pub fn new_for_github_copilot(
        connection_id: impl Into<String>,
        generation_id: impl Into<String>,
    ) -> Result<Self, CredentialStoreError> {
        let mut reference = Self::new(connection_id, generation_id)?;
        reference.service = OAuthCredentialService::GitHubCopilot;
        Ok(reference)
    }

    fn username(&self, token_kind: &str) -> String {
        format!("{}:{}:{token_kind}", self.connection_id, self.generation_id)
    }

    /// Returns the connection identifier used to scope this credential.
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    /// Returns the generation identifier used to distinguish credential versions.
    pub fn generation_id(&self) -> &str {
        &self.generation_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// A validated, non-secret key for one project/server/environment MCP secret.
pub struct McpCredentialReference {
    project_id: String,
    server_id: String,
    environment_name: String,
}

impl McpCredentialReference {
    /// Validates and creates a project-scoped MCP secret reference.
    ///
    /// Project IDs must be non-empty, at most 256 bytes, and contain no control
    /// characters. Server IDs use lowercase ASCII letters, digits, and
    /// underscores, up to 24 bytes. Environment names must be valid ASCII
    /// identifiers and cannot name protected process environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::InvalidReference`] if an identifier is
    /// malformed or outside its size limit.
    pub fn new(
        project_id: impl Into<String>,
        server_id: impl Into<String>,
        environment_name: impl Into<String>,
    ) -> Result<Self, CredentialStoreError> {
        let project_id = project_id.into();
        let server_id = server_id.into();
        let environment_name = environment_name.into();
        if project_id.is_empty()
            || project_id.len() > 256
            || project_id.chars().any(char::is_control)
            || !is_valid_mcp_reference_part(&server_id)
            || !is_valid_environment_name(&environment_name)
        {
            return Err(CredentialStoreError::InvalidReference);
        }
        Ok(Self {
            project_id,
            server_id,
            environment_name,
        })
    }

    fn username(&self) -> String {
        use sha2::{Digest, Sha256};

        let mut digest = Sha256::new();
        digest.update(self.project_id.as_bytes());
        digest.update([0]);
        digest.update(self.server_id.as_bytes());
        digest.update([0]);
        digest.update(self.environment_name.as_bytes());
        format!("mcp:{}", URL_SAFE_NO_PAD.encode(digest.finalize()))
    }
}

fn is_valid_mcp_reference_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 24
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

pub(crate) fn is_valid_environment_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    let syntax_is_valid = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && value.len() <= 128;
    syntax_is_valid
        && !matches!(
            value.to_ascii_uppercase().as_str(),
            "PATH"
                | "SYSTEMROOT"
                | "WINDIR"
                | "PATHEXT"
                | "COMSPEC"
                | "USERPROFILE"
                | "APPDATA"
                | "LOCALAPPDATA"
                | "TEMP"
                | "TMP"
        )
}

/// OAuth credentials held in memory with zeroizing string storage.
///
/// Expiry parsed from an access token is only a scheduling hint: this type does
/// not verify a token signature or establish its authenticity.
pub struct OAuthTokenPair {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    expires_at_unix_seconds: Option<u64>,
}

impl OAuthTokenPair {
    /// Validates a token pair and reads an optional expiry hint from the access
    /// token's JWT payload.
    ///
    /// Empty tokens and tokens larger than the supported credential limit are
    /// rejected. The JWT payload is decoded without signature verification.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::EmptyToken`] or
    /// [`CredentialStoreError::TokenTooLarge`] if either token is invalid.
    pub fn new(access_token: String, refresh_token: String) -> Result<Self, CredentialStoreError> {
        let access_token = Zeroizing::new(access_token);
        let refresh_token = Zeroizing::new(refresh_token);
        validate_token(&access_token)?;
        validate_token(&refresh_token)?;
        let expires_at_unix_seconds = access_token_expiration(&access_token);

        Ok(Self {
            access_token,
            refresh_token,
            expires_at_unix_seconds,
        })
    }

    /// Borrows the access token; callers must not log or persist this value.
    pub fn access_token(&self) -> &str {
        self.access_token.as_str()
    }

    /// Borrows the refresh token; callers must not log or persist this value.
    pub fn refresh_token(&self) -> &str {
        self.refresh_token.as_str()
    }

    /// Reports whether the known access-token expiry falls within `window` of
    /// `now`. Returns `false` when no expiry is available, and `true` if `now`
    /// predates the Unix epoch.
    pub fn expires_within(&self, now: SystemTime, window: Duration) -> bool {
        let Some(expires_at) = self.expires_at_unix_seconds else {
            return false;
        };
        let Ok(now) = now.duration_since(UNIX_EPOCH) else {
            return true;
        };
        expires_at <= now.as_secs().saturating_add(window.as_secs())
    }

    /// Returns the known access-token expiry as Unix seconds, if available.
    pub fn expires_at_unix_seconds(&self) -> Option<u64> {
        self.expires_at_unix_seconds
    }

    /// Creates a token pair using an expiry supplied by a trusted provider
    /// response rather than parsing it from the access token.
    ///
    /// Both tokens are validated before the pair is returned.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::EmptyToken`] or
    /// [`CredentialStoreError::TokenTooLarge`] if either token is invalid.
    pub fn with_expiration(
        access_token: Zeroizing<String>,
        refresh_token: Zeroizing<String>,
        access_expires_at_unix_seconds: u64,
    ) -> Result<Self, CredentialStoreError> {
        validate_token(&access_token)?;
        validate_token(&refresh_token)?;
        Ok(Self {
            access_token,
            refresh_token,
            expires_at_unix_seconds: Some(access_expires_at_unix_seconds),
        })
    }
}

#[derive(Deserialize)]
struct AccessTokenClaims {
    exp: Option<u64>,
}

fn access_token_expiration(access_token: &str) -> Option<u64> {
    let mut parts = access_token.split('.');
    let (Some(header), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if header.is_empty() || payload.is_empty() || signature.is_empty() {
        return None;
    }

    let payload = Zeroizing::new(URL_SAFE_NO_PAD.decode(payload).ok()?);
    serde_json::from_slice::<AccessTokenClaims>(&payload)
        .ok()?
        .exp
}

#[derive(Default)]
/// Stores provider OAuth pairs and MCP secrets in the DPAPI-protected database vault.
///
/// Legacy Windows Credential Manager entries are imported on read and removed after migration.
pub struct CredentialStore;

impl CredentialStore {
    /// Stores a project-scoped MCP secret after enforcing its size and content limits.
    ///
    /// # Errors
    ///
    /// Returns a validation, lock, or operating-system storage error. The input
    /// remains zeroizing if validation or storage fails.
    pub fn store_mcp_secret(
        &self,
        storage: &crate::storage::AppStorage,
        reference: &McpCredentialReference,
        secret: Zeroizing<String>,
    ) -> Result<(), CredentialStoreError> {
        if secret.is_empty() {
            return Err(CredentialStoreError::EmptyToken);
        }
        if secret.len() > MAX_MCP_SECRET_BYTES || secret.contains('\0') {
            return Err(CredentialStoreError::TokenTooLarge);
        }
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let vault_key = vault_key_for_mcp(reference);
        crate::secrets::set_vault_secret_from_storage(storage, &vault_key, &secret)
            .map_err(|_| CredentialStoreError::StorageWriteFailed)?;
        if let Ok(entry) = mcp_entry(reference) {
            let _ = delete_if_present(&entry);
        }
        Ok(())
    }

    /// Loads an MCP secret into zeroizing memory.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::TokenPairMissing`] when the entry does
    /// not exist, or a lock, decoding, or storage error otherwise.
    pub fn load_mcp_secret(
        &self,
        storage: &crate::storage::AppStorage,
        reference: &McpCredentialReference,
    ) -> Result<Zeroizing<String>, CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let vault_key = vault_key_for_mcp(reference);
        match crate::secrets::get_vault_secret_from_storage(storage, &vault_key) {
            Ok(Some(secret)) => return Ok(secret),
            Ok(None) => {}
            Err(_) => return Err(CredentialStoreError::StorageReadFailed),
        }
        let entry = mcp_entry(reference)?;
        import_legacy_mcp_secret(storage, &vault_key, &entry)
    }

    /// Checks whether an MCP secret exists without returning its value.
    ///
    /// # Errors
    ///
    /// Returns a lock or operating-system storage error.
    pub fn has_mcp_secret(
        &self,
        storage: &crate::storage::AppStorage,
        reference: &McpCredentialReference,
    ) -> Result<bool, CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let vault_key = vault_key_for_mcp(reference);
        match crate::secrets::has_vault_secret_from_storage(storage, &vault_key) {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(_) => return Err(CredentialStoreError::StorageReadFailed),
        }
        credential_exists(&mcp_entry(reference)?)
    }

    /// Removes an MCP secret; removing a missing entry is treated as success.
    ///
    /// # Errors
    ///
    /// Returns a lock or operating-system storage error.
    pub fn delete_mcp_secret(
        &self,
        storage: &crate::storage::AppStorage,
        reference: &McpCredentialReference,
    ) -> Result<(), CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let vault_key = vault_key_for_mcp(reference);
        let _ = crate::secrets::delete_vault_secret_from_storage(storage, &vault_key);
        delete_if_present(&mcp_entry(reference)?)
    }

    /// Writes a new OAuth pair once for a reference, rolling back partial writes
    /// when possible.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::ReferenceAlreadyUsed`] if the reference
    /// already contains credentials, or a validation, lock, or storage error.
    pub fn store_new_token_pair(
        &self,
        storage: &crate::storage::AppStorage,
        reference: &OAuthCredentialReference,
        tokens: &OAuthTokenPair,
    ) -> Result<(), CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let access_key = vault_key_for_oauth(reference, "access-v2");
        let refresh_key = vault_key_for_oauth(reference, "refresh-v2");
        let expiry_key = vault_key_for_oauth(reference, "access-exp-v1");

        let access_entry = entry(reference, "access-v2")?;
        let refresh_entry = entry(reference, "refresh-v2")?;
        let expiry_entry = entry(reference, "access-exp-v1")?;

        let in_vault = crate::secrets::has_vault_secret_from_storage(storage, &access_key)
            .map_err(|_| CredentialStoreError::StorageReadFailed)?
            || crate::secrets::has_vault_secret_from_storage(storage, &refresh_key)
                .map_err(|_| CredentialStoreError::StorageReadFailed)?
            || crate::secrets::has_vault_secret_from_storage(storage, &expiry_key)
                .map_err(|_| CredentialStoreError::StorageReadFailed)?;

        if in_vault
            || credential_exists(&access_entry)?
            || credential_exists(&refresh_entry)?
            || credential_exists(&expiry_entry)?
        {
            return Err(CredentialStoreError::ReferenceAlreadyUsed);
        }

        write_token_pair_to_vault(storage, &access_key, &refresh_key, &expiry_key, tokens)?;

        let _ = delete_if_present(&access_entry);
        let _ = delete_if_present(&refresh_entry);
        let _ = delete_if_present(&expiry_entry);

        Ok(())
    }

    /// Loads and validates both OAuth tokens and their optional expiry metadata.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::TokenPairMissing`] when neither token is
    /// present, [`CredentialStoreError::IncompleteTokenPair`] when only one is
    /// present, or a validation, lock, or storage error.
    pub fn load_token_pair(
        &self,
        storage: &crate::storage::AppStorage,
        reference: &OAuthCredentialReference,
    ) -> Result<OAuthTokenPair, CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let access_key = vault_key_for_oauth(reference, "access-v2");
        let refresh_key = vault_key_for_oauth(reference, "refresh-v2");
        let expiry_key = vault_key_for_oauth(reference, "access-exp-v1");

        let vault_access = crate::secrets::get_vault_secret_from_storage(storage, &access_key)
            .map_err(|_| CredentialStoreError::StorageReadFailed)?;
        let vault_refresh = crate::secrets::get_vault_secret_from_storage(storage, &refresh_key)
            .map_err(|_| CredentialStoreError::StorageReadFailed)?;
        let vault_expiry_str = crate::secrets::get_vault_secret_from_storage(storage, &expiry_key)
            .map_err(|_| CredentialStoreError::StorageReadFailed)?;
        let vault_expiry = vault_expiry_str
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok());

        if let Some(pair) = token_pair(vault_access, vault_refresh, vault_expiry)? {
            return Ok(pair);
        }

        let access_entry = entry(reference, "access-v2")?;
        let refresh_entry = entry(reference, "refresh-v2")?;
        let expiry_entry = entry(reference, "access-exp-v1")?;
        import_legacy_token_pair(
            storage,
            &access_key,
            &refresh_key,
            &expiry_key,
            &access_entry,
            &refresh_entry,
            &expiry_entry,
        )?
        .ok_or(CredentialStoreError::TokenPairMissing)
    }

    /// Deletes all OAuth token and expiry entries for a reference.
    ///
    /// # Errors
    ///
    /// Returns a lock or operating-system storage error if any entry cannot be
    /// deleted.
    pub fn delete_token_pair(
        &self,
        storage: &crate::storage::AppStorage,
        reference: &OAuthCredentialReference,
    ) -> Result<(), CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let access_key = vault_key_for_oauth(reference, "access-v2");
        let refresh_key = vault_key_for_oauth(reference, "refresh-v2");
        let expiry_key = vault_key_for_oauth(reference, "access-exp-v1");

        let _ = crate::secrets::delete_vault_secret_from_storage(storage, &access_key);
        let _ = crate::secrets::delete_vault_secret_from_storage(storage, &refresh_key);
        let _ = crate::secrets::delete_vault_secret_from_storage(storage, &expiry_key);

        let entries = [
            entry(reference, "access-v2")?,
            entry(reference, "refresh-v2")?,
            entry(reference, "access-exp-v1")?,
        ];
        let mut deletion_failed = false;
        for entry in entries {
            if delete_if_present(&entry).is_err() {
                deletion_failed = true;
            }
        }

        if deletion_failed {
            return Err(CredentialStoreError::StorageDeleteFailed);
        }
        Ok(())
    }
}

fn import_legacy_mcp_secret(
    storage: &crate::storage::AppStorage,
    vault_key: &str,
    entry: &Entry,
) -> Result<Zeroizing<String>, CredentialStoreError> {
    let secret = entry.get_secret().map_err(|error| match error {
        KeyringError::NoEntry => CredentialStoreError::TokenPairMissing,
        _ => CredentialStoreError::StorageReadFailed,
    })?;
    let secret = Zeroizing::new(secret);
    let secret = std::str::from_utf8(secret.as_slice())
        .map_err(|_| CredentialStoreError::InvalidStoredToken)?;
    let result = Zeroizing::new(secret.to_owned());
    crate::secrets::set_vault_secret_from_storage(storage, vault_key, &result)
        .map_err(|_| CredentialStoreError::StorageWriteFailed)?;
    let _ = delete_if_present(entry);
    Ok(result)
}

fn import_legacy_token_pair(
    storage: &crate::storage::AppStorage,
    access_key: &str,
    refresh_key: &str,
    expiry_key: &str,
    access_entry: &Entry,
    refresh_entry: &Entry,
    expiry_entry: &Entry,
) -> Result<Option<OAuthTokenPair>, CredentialStoreError> {
    let access = read_raw_optional(access_entry)?;
    let refresh = read_raw_optional(refresh_entry)?;
    let expiry = read_expiration_optional(expiry_entry)?;

    let Some(pair) = token_pair(access, refresh, expiry)? else {
        return Ok(None);
    };
    write_token_pair_to_vault(storage, access_key, refresh_key, expiry_key, &pair)?;
    let _ = delete_if_present(access_entry);
    let _ = delete_if_present(refresh_entry);
    let _ = delete_if_present(expiry_entry);
    Ok(Some(pair))
}

fn vault_key_for_mcp(reference: &McpCredentialReference) -> String {
    format!(
        "keyring:{}:{}",
        MCP_CREDENTIAL_SERVICE,
        reference.username()
    )
}

fn vault_key_for_oauth(reference: &OAuthCredentialReference, token_kind: &str) -> String {
    format!(
        "keyring:{}:{}",
        reference.service.keyring_name(),
        reference.username(token_kind)
    )
}

fn mcp_entry(reference: &McpCredentialReference) -> Result<Entry, CredentialStoreError> {
    Entry::new(MCP_CREDENTIAL_SERVICE, &reference.username())
        .map_err(|_| CredentialStoreError::StorageUnavailable)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// A safe, non-secret failure reported by credential validation or storage.
pub enum CredentialStoreError {
    /// A credential reference does not meet the identifier rules.
    InvalidReference,
    /// A credential value is empty.
    EmptyToken,
    /// A credential value exceeds the supported size limit.
    TokenTooLarge,
    /// The reference already has stored credentials and cannot be overwritten here.
    ReferenceAlreadyUsed,
    /// The requested credential is missing, or no complete OAuth pair exists.
    TokenPairMissing,
    /// Only one member of an OAuth token pair is stored.
    IncompleteTokenPair,
    /// Stored token or expiry bytes are malformed or invalid UTF-8.
    InvalidStoredToken,
    /// A multi-entry credential write failed and rollback could not restore consistency.
    IncompleteWrite,
    /// The protected local vault or legacy operating-system credential store
    /// could not be initialized.
    StorageUnavailable,
    /// The protected local vault or legacy operating-system credential store
    /// could not read an entry.
    StorageReadFailed,
    /// The protected local vault could not write an entry.
    StorageWriteFailed,
    /// The protected local vault or legacy operating-system credential store
    /// could not delete an entry.
    StorageDeleteFailed,
    /// The in-process credential operation lock is poisoned.
    LockUnavailable,
}

impl std::fmt::Display for CredentialStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidReference => "The credential reference is invalid.",
            Self::EmptyToken => "A credential cannot be empty.",
            Self::TokenTooLarge => "A credential exceeds the supported size limit.",
            Self::ReferenceAlreadyUsed => "The credential reference is already in use.",
            Self::TokenPairMissing => "The OAuth credential pair was not found.",
            Self::IncompleteTokenPair => "The OAuth credential pair is incomplete.",
            Self::InvalidStoredToken => "A stored OAuth token could not be decoded.",
            Self::IncompleteWrite => "The OAuth credential pair could not be stored completely.",
            Self::StorageUnavailable => "The protected credential store is unavailable.",
            Self::StorageReadFailed => {
                "The protected credential store could not read a credential."
            }
            Self::StorageWriteFailed => {
                "The protected credential store could not write a credential."
            }
            Self::StorageDeleteFailed => {
                "The protected credential store could not delete a credential."
            }
            Self::LockUnavailable => "Credential storage is temporarily unavailable.",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CredentialStoreError {}

fn entry(
    reference: &OAuthCredentialReference,
    token_kind: &str,
) -> Result<Entry, CredentialStoreError> {
    Entry::new(
        reference.service.keyring_name(),
        &reference.username(token_kind),
    )
    .map_err(|_| CredentialStoreError::StorageUnavailable)
}

fn credential_exists(entry: &Entry) -> Result<bool, CredentialStoreError> {
    match entry.get_secret() {
        Ok(secret) => {
            drop(Zeroizing::new(secret));
            Ok(true)
        }
        Err(KeyringError::NoEntry) => Ok(false),
        Err(_) => Err(CredentialStoreError::StorageReadFailed),
    }
}

fn read_raw_optional(entry: &Entry) -> Result<Option<Zeroizing<String>>, CredentialStoreError> {
    match entry.get_secret() {
        Ok(secret) => {
            let secret = Zeroizing::new(secret);
            let token = std::str::from_utf8(secret.as_slice())
                .map_err(|_| CredentialStoreError::InvalidStoredToken)?;
            let token = Zeroizing::new(token.to_owned());
            validate_token(&token)?;
            Ok(Some(token))
        }
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(CredentialStoreError::StorageReadFailed),
    }
}

fn token_pair(
    access: Option<Zeroizing<String>>,
    refresh: Option<Zeroizing<String>>,
    expires_at_unix_seconds: Option<u64>,
) -> Result<Option<OAuthTokenPair>, CredentialStoreError> {
    match (access, refresh) {
        (Some(access_token), Some(refresh_token)) => {
            validate_token(&access_token)?;
            validate_token(&refresh_token)?;
            let expires_at_unix_seconds =
                expires_at_unix_seconds.or_else(|| access_token_expiration(&access_token));
            Ok(Some(OAuthTokenPair {
                access_token,
                refresh_token,
                expires_at_unix_seconds,
            }))
        }
        (None, None) => Ok(None),
        _ => Err(CredentialStoreError::IncompleteTokenPair),
    }
}

fn read_expiration_optional(entry: &Entry) -> Result<Option<u64>, CredentialStoreError> {
    match entry.get_secret() {
        Ok(secret) => {
            let secret = Zeroizing::new(secret);
            let value = std::str::from_utf8(secret.as_slice())
                .map_err(|_| CredentialStoreError::InvalidStoredToken)?;
            value
                .parse::<u64>()
                .map(Some)
                .map_err(|_| CredentialStoreError::InvalidStoredToken)
        }
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(CredentialStoreError::StorageReadFailed),
    }
}

fn delete_if_present(entry: &Entry) -> Result<(), CredentialStoreError> {
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(_) => Err(CredentialStoreError::StorageDeleteFailed),
    }
}

fn write_token_pair_to_vault(
    storage: &crate::storage::AppStorage,
    access_key: &str,
    refresh_key: &str,
    expiry_key: &str,
    tokens: &OAuthTokenPair,
) -> Result<(), CredentialStoreError> {
    crate::secrets::set_vault_secret_from_storage(storage, access_key, tokens.access_token())
        .map_err(|_| CredentialStoreError::StorageWriteFailed)?;

    if crate::secrets::set_vault_secret_from_storage(storage, refresh_key, tokens.refresh_token())
        .is_err()
    {
        let _ = crate::secrets::delete_vault_secret_from_storage(storage, access_key);
        return Err(CredentialStoreError::IncompleteWrite);
    }

    if let Some(expires_at) = tokens.expires_at_unix_seconds()
        && crate::secrets::set_vault_secret_from_storage(
            storage,
            expiry_key,
            &expires_at.to_string(),
        )
        .is_err()
    {
        let _ = crate::secrets::delete_vault_secret_from_storage(storage, refresh_key);
        let _ = crate::secrets::delete_vault_secret_from_storage(storage, access_key);
        return Err(CredentialStoreError::IncompleteWrite);
    }

    Ok(())
}

fn validate_token(token: &str) -> Result<(), CredentialStoreError> {
    if token.is_empty() {
        return Err(CredentialStoreError::EmptyToken);
    }
    if token.len() > MAX_CREDENTIAL_BYTES {
        return Err(CredentialStoreError::TokenTooLarge);
    }
    Ok(())
}

fn is_valid_reference_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_PART_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod mcp_credential_tests {
    use super::{
        CredentialStore, McpCredentialReference, OAuthCredentialReference, OAuthTokenPair,
        is_valid_environment_name,
    };
    use crate::storage::AppStorage;
    use std::{fs, path::PathBuf, sync::Once};

    fn install_in_memory_credential_store() {
        static INSTALL_MOCK: Once = Once::new();
        INSTALL_MOCK.call_once(|| {
            // Keep adapter tests from reading or writing the current user's credential store.
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    struct TestStorage {
        storage: AppStorage,
        // Drop storage before removing the directory that contains its open database.
        _directory: TestDirectory,
    }

    impl TestStorage {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "openchat-credential-store-test-{}",
                uuid::Uuid::new_v4()
            ));
            let directory = TestDirectory(root);
            let storage =
                AppStorage::open_at(directory.0.clone()).expect("open isolated test storage");
            storage
                .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
                .expect("prepare credential vault schema");
            Self {
                storage,
                _directory: directory,
            }
        }
    }

    struct TestDirectory(PathBuf);

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test storage");
        }
    }

    #[test]
    fn mcp_credentials_are_scoped_to_project_server_and_environment_name() {
        let reference = McpCredentialReference::new("project-1", "private_api", "API_TOKEN")
            .expect("valid credential reference");
        let same_reference = McpCredentialReference::new("project-1", "private_api", "API_TOKEN")
            .expect("valid credential reference");
        let other_project = McpCredentialReference::new("project-2", "private_api", "API_TOKEN")
            .expect("valid credential reference");
        let other_server = McpCredentialReference::new("project-1", "other_server", "API_TOKEN")
            .expect("valid credential reference");
        let other_name = McpCredentialReference::new("project-1", "private_api", "OTHER_TOKEN")
            .expect("valid credential reference");

        assert_eq!(reference.username(), same_reference.username());
        assert_ne!(reference.username(), other_project.username());
        assert_ne!(reference.username(), other_server.username());
        assert_ne!(reference.username(), other_name.username());
        assert!(McpCredentialReference::new("project-1", "private-api", "API_TOKEN").is_err());
        assert!(McpCredentialReference::new("project-1", "private_api", "1_TOKEN").is_err());
    }

    #[test]
    fn mcp_environment_names_are_bounded_and_cannot_replace_sandbox_variables() {
        assert!(is_valid_environment_name("API_TOKEN"));
        assert!(is_valid_environment_name("_PRIVATE_KEY"));
        assert!(!is_valid_environment_name("1_API_TOKEN"));
        assert!(!is_valid_environment_name("API-TOKEN"));
        assert!(!is_valid_environment_name("PATH"));
        assert!(!is_valid_environment_name("systemroot"));
        assert!(!is_valid_environment_name(&"A".repeat(129)));
    }

    #[test]
    fn mcp_credential_store_round_trips_and_removes_a_secret() {
        install_in_memory_credential_store();
        let reference = McpCredentialReference::new(
            uuid::Uuid::new_v4().to_string(),
            "credential_test",
            "API_TOKEN",
        )
        .expect("valid credential reference");
        let store = CredentialStore;
        let storage = TestStorage::new();
        let secret = format!("test-{}", uuid::Uuid::new_v4());

        assert!(
            !store
                .has_mcp_secret(&storage.storage, &reference)
                .expect("query credential")
        );
        store
            .store_mcp_secret(
                &storage.storage,
                &reference,
                zeroize::Zeroizing::new(secret.clone()),
            )
            .expect("store credential");
        assert!(
            store
                .has_mcp_secret(&storage.storage, &reference)
                .expect("query credential")
        );
        assert_eq!(
            store
                .load_mcp_secret(&storage.storage, &reference)
                .expect("load credential")
                .as_str(),
            secret
        );
        store
            .delete_mcp_secret(&storage.storage, &reference)
            .expect("remove credential");
        assert!(
            !store
                .has_mcp_secret(&storage.storage, &reference)
                .expect("query credential")
        );
    }

    #[test]
    fn oauth_token_pair_round_trips_and_removes_in_vault() {
        install_in_memory_credential_store();
        let conn_id = format!("conn_{}", uuid::Uuid::new_v4().simple());
        let gen_id = format!("gen_{}", uuid::Uuid::new_v4().simple());
        let reference = OAuthCredentialReference::new(conn_id, gen_id).expect("valid reference");
        let store = CredentialStore;
        let storage = TestStorage::new();
        let tokens = OAuthTokenPair::with_expiration(
            zeroize::Zeroizing::new("access_token_val".to_string()),
            zeroize::Zeroizing::new("refresh_token_val".to_string()),
            1_800_000_000,
        )
        .expect("token pair");

        store
            .store_new_token_pair(&storage.storage, &reference, &tokens)
            .expect("store new token pair");
        let loaded = store
            .load_token_pair(&storage.storage, &reference)
            .expect("load token pair");
        assert_eq!(loaded.access_token(), "access_token_val");
        assert_eq!(loaded.refresh_token(), "refresh_token_val");
        assert_eq!(loaded.expires_at_unix_seconds(), Some(1_800_000_000));

        store
            .delete_token_pair(&storage.storage, &reference)
            .expect("delete token pair");
        assert!(store.load_token_pair(&storage.storage, &reference).is_err());
    }

    #[test]
    fn legacy_credentials_remain_when_vault_import_fails() {
        install_in_memory_credential_store();
        let storage = TestStorage::new();
        {
            let connection = storage.storage.connect().expect("connect test storage");
            connection
                .execute_batch(
                    "CREATE TRIGGER reject_credential_migration
                     BEFORE INSERT ON secure_vault
                     BEGIN SELECT RAISE(FAIL, 'injected migration failure'); END;",
                )
                .expect("inject vault write failure");
        }

        let mcp_reference = McpCredentialReference::new(
            uuid::Uuid::new_v4().to_string(),
            "migration_test",
            "API_TOKEN",
        )
        .expect("valid MCP credential reference");
        let mcp_legacy = super::mcp_entry(&mcp_reference).expect("create legacy MCP entry");
        mcp_legacy
            .set_secret(b"legacy-mcp-value")
            .expect("store mock legacy MCP credential");
        assert_eq!(
            super::import_legacy_mcp_secret(
                &storage.storage,
                &super::vault_key_for_mcp(&mcp_reference),
                &mcp_legacy,
            )
            .err(),
            Some(super::CredentialStoreError::StorageWriteFailed)
        );
        assert!(mcp_legacy.get_secret().is_ok());
        let _ = super::delete_if_present(&mcp_legacy);

        let oauth_reference = OAuthCredentialReference::new(
            format!("conn_{}", uuid::Uuid::new_v4().simple()),
            format!("gen_{}", uuid::Uuid::new_v4().simple()),
        )
        .expect("valid OAuth credential reference");
        let access_legacy =
            super::entry(&oauth_reference, "access-v2").expect("create legacy OAuth access entry");
        let refresh_legacy = super::entry(&oauth_reference, "refresh-v2")
            .expect("create legacy OAuth refresh entry");
        access_legacy
            .set_secret(b"legacy-access-value")
            .expect("store mock legacy access token");
        refresh_legacy
            .set_secret(b"legacy-refresh-value")
            .expect("store mock legacy refresh token");
        let access_key = super::vault_key_for_oauth(&oauth_reference, "access-v2");
        let refresh_key = super::vault_key_for_oauth(&oauth_reference, "refresh-v2");
        let expiry_key = super::vault_key_for_oauth(&oauth_reference, "access-exp-v1");
        assert_eq!(
            super::import_legacy_token_pair(
                &storage.storage,
                &access_key,
                &refresh_key,
                &expiry_key,
                &access_legacy,
                &refresh_legacy,
                &super::entry(&oauth_reference, "access-exp-v1")
                    .expect("create legacy OAuth expiry entry"),
            )
            .err(),
            Some(super::CredentialStoreError::StorageWriteFailed)
        );
        assert!(access_legacy.get_secret().is_ok());
        assert!(refresh_legacy.get_secret().is_ok());
        let _ = super::delete_if_present(&access_legacy);
        let _ = super::delete_if_present(&refresh_legacy);
    }

    #[test]
    fn mcp_credential_read_failure_does_not_fall_back_to_legacy_store() {
        install_in_memory_credential_store();
        let storage = TestStorage::new();
        let reference = McpCredentialReference::new(
            uuid::Uuid::new_v4().to_string(),
            "vault_read_test",
            "API_TOKEN",
        )
        .expect("valid MCP credential reference");
        let legacy = super::mcp_entry(&reference).expect("create legacy MCP entry");
        legacy
            .set_secret(b"legacy-value")
            .expect("store mock legacy credential");
        storage
            .storage
            .connect()
            .expect("connect to isolated storage")
            .execute("DROP TABLE secure_vault", [])
            .expect("inject vault read failure");

        let store = CredentialStore;
        assert_eq!(
            store.load_mcp_secret(&storage.storage, &reference).err(),
            Some(super::CredentialStoreError::StorageReadFailed)
        );
        assert_eq!(
            store.has_mcp_secret(&storage.storage, &reference).err(),
            Some(super::CredentialStoreError::StorageReadFailed)
        );
        assert!(legacy.get_secret().is_ok());
        let _ = super::delete_if_present(&legacy);

        let oauth_reference = OAuthCredentialReference::new(
            format!("conn_{}", uuid::Uuid::new_v4().simple()),
            format!("gen_{}", uuid::Uuid::new_v4().simple()),
        )
        .expect("valid OAuth credential reference");
        let token_pair = OAuthTokenPair::with_expiration(
            zeroize::Zeroizing::new("access-value".to_owned()),
            zeroize::Zeroizing::new("refresh-value".to_owned()),
            1_800_000_000,
        )
        .expect("valid OAuth token pair");
        assert_eq!(
            store
                .store_new_token_pair(&storage.storage, &oauth_reference, &token_pair)
                .err(),
            Some(super::CredentialStoreError::StorageReadFailed)
        );
    }
}
