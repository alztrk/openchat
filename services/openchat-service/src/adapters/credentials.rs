//! Stores provider OAuth token pairs and project-scoped MCP secrets in the OS credential store.
//!
//! References contain only validated identifiers. Secret values use zeroizing allocations and
//! are never included in credential-reference serialization or user-facing errors.

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
    /// Empty tokens and tokens larger than the Windows credential limit are
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
/// Stores provider OAuth pairs and MCP secrets in Windows Credential Manager.
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
        mcp_entry(reference)?
            .set_secret(secret.as_bytes())
            .map_err(write_error)
    }

    /// Loads an MCP secret into zeroizing memory.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError::TokenPairMissing`] when the entry does
    /// not exist, or a lock, decoding, or storage error otherwise.
    pub fn load_mcp_secret(
        &self,
        reference: &McpCredentialReference,
    ) -> Result<Zeroizing<String>, CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let secret = mcp_entry(reference)?
            .get_secret()
            .map_err(|error| match error {
                KeyringError::NoEntry => CredentialStoreError::TokenPairMissing,
                _ => CredentialStoreError::StorageReadFailed,
            })?;
        let secret = Zeroizing::new(secret);
        let secret = std::str::from_utf8(secret.as_slice())
            .map_err(|_| CredentialStoreError::InvalidStoredToken)?;
        Ok(Zeroizing::new(secret.to_owned()))
    }

    /// Checks whether an MCP secret exists without returning its value.
    ///
    /// # Errors
    ///
    /// Returns a lock or operating-system storage error.
    pub fn has_mcp_secret(
        &self,
        reference: &McpCredentialReference,
    ) -> Result<bool, CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        credential_exists(&mcp_entry(reference)?)
    }

    /// Removes an MCP secret; removing a missing entry is treated as success.
    ///
    /// # Errors
    ///
    /// Returns a lock or operating-system storage error.
    pub fn delete_mcp_secret(
        &self,
        reference: &McpCredentialReference,
    ) -> Result<(), CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
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
        reference: &OAuthCredentialReference,
        tokens: &OAuthTokenPair,
    ) -> Result<(), CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let access_entry = entry(reference, "access-v2")?;
        let refresh_entry = entry(reference, "refresh-v2")?;
        let expiry_entry = entry(reference, "access-exp-v1")?;

        if credential_exists(&access_entry)?
            || credential_exists(&refresh_entry)?
            || credential_exists(&expiry_entry)?
        {
            return Err(CredentialStoreError::ReferenceAlreadyUsed);
        }

        access_entry
            .set_secret(tokens.access_token().as_bytes())
            .map_err(write_error)?;

        if let Err(error) = refresh_entry.set_secret(tokens.refresh_token().as_bytes()) {
            return match delete_if_present(&access_entry) {
                Ok(()) => Err(write_error(error)),
                Err(_) => Err(CredentialStoreError::IncompleteWrite),
            };
        }

        if let Some(expires_at) = tokens.expires_at_unix_seconds()
            && let Err(error) = expiry_entry.set_secret(expires_at.to_string().as_bytes())
        {
            let refresh_delete_failed = delete_if_present(&refresh_entry).is_err();
            let access_delete_failed = delete_if_present(&access_entry).is_err();
            return if refresh_delete_failed || access_delete_failed {
                Err(CredentialStoreError::IncompleteWrite)
            } else {
                Err(write_error(error))
            };
        }

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
        reference: &OAuthCredentialReference,
    ) -> Result<OAuthTokenPair, CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let access = read_raw_optional(&entry(reference, "access-v2")?)?;
        let refresh = read_raw_optional(&entry(reference, "refresh-v2")?)?;
        let expiry = read_expiration_optional(&entry(reference, "access-exp-v1")?)?;
        token_pair(access, refresh, expiry)?.ok_or(CredentialStoreError::TokenPairMissing)
    }

    /// Deletes all OAuth token and expiry entries for a reference.
    ///
    /// # Errors
    ///
    /// Returns a lock or operating-system storage error if any entry cannot be
    /// deleted.
    pub fn delete_token_pair(
        &self,
        reference: &OAuthCredentialReference,
    ) -> Result<(), CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
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

fn mcp_entry(reference: &McpCredentialReference) -> Result<Entry, CredentialStoreError> {
    Entry::new(MCP_CREDENTIAL_SERVICE, &reference.username())
        .map_err(|_| CredentialStoreError::StorageUnavailable)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// A safe, non-secret failure reported by credential validation or storage.
pub enum CredentialStoreError {
    /// A credential reference does not meet the identifier rules.
    InvalidReference,
    /// A token or secret is empty.
    EmptyToken,
    /// A token or secret exceeds the supported size limit.
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
    /// The operating-system credential store could not be initialized.
    StorageUnavailable,
    /// The operating-system credential store could not read an entry.
    StorageReadFailed,
    /// The operating-system credential store could not write an entry.
    StorageWriteFailed,
    /// The operating-system credential store could not delete an entry.
    StorageDeleteFailed,
    /// The in-process credential operation lock is poisoned.
    LockUnavailable,
}

impl std::fmt::Display for CredentialStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidReference => "The credential reference is invalid.",
            Self::EmptyToken => "An OAuth token cannot be empty.",
            Self::TokenTooLarge => "An OAuth token exceeds the Windows credential limit.",
            Self::ReferenceAlreadyUsed => "The credential reference is already in use.",
            Self::TokenPairMissing => "The OAuth credential pair was not found.",
            Self::IncompleteTokenPair => "The OAuth credential pair is incomplete.",
            Self::InvalidStoredToken => "A stored OAuth token could not be decoded.",
            Self::IncompleteWrite => "The OAuth credential pair could not be stored completely.",
            Self::StorageUnavailable => "Windows Credential Manager is unavailable.",
            Self::StorageReadFailed => "Windows Credential Manager could not read a credential.",
            Self::StorageWriteFailed => "Windows Credential Manager could not write a credential.",
            Self::StorageDeleteFailed => {
                "Windows Credential Manager could not delete a credential."
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

fn write_error(error: KeyringError) -> CredentialStoreError {
    match error {
        KeyringError::TooLong(_, _) => CredentialStoreError::TokenTooLarge,
        _ => CredentialStoreError::StorageWriteFailed,
    }
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
    use super::{CredentialStore, McpCredentialReference, is_valid_environment_name};

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
        let reference = McpCredentialReference::new(
            uuid::Uuid::new_v4().to_string(),
            "credential_test",
            "API_TOKEN",
        )
        .expect("valid credential reference");
        let store = CredentialStore;
        let secret = format!("test-{}", uuid::Uuid::new_v4());

        assert!(!store.has_mcp_secret(&reference).expect("query credential"));
        store
            .store_mcp_secret(&reference, zeroize::Zeroizing::new(secret.clone()))
            .expect("store credential");
        assert!(store.has_mcp_secret(&reference).expect("query credential"));
        assert_eq!(
            store
                .load_mcp_secret(&reference)
                .expect("load credential")
                .as_str(),
            secret
        );
        store
            .delete_mcp_secret(&reference)
            .expect("remove credential");
        assert!(!store.has_mcp_secret(&reference).expect("query credential"));
    }
}
