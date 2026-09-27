use std::sync::Mutex;

use keyring::{Entry, Error as KeyringError};
use zeroize::Zeroizing;

const CREDENTIAL_SERVICE: &str = "Zihora.ChatGPT.OAuth";
const MAX_CREDENTIAL_BYTES: usize = 2560;
const MAX_REFERENCE_PART_BYTES: usize = 128;
static OPERATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthCredentialReference {
    connection_id: String,
    generation_id: String,
}

impl OAuthCredentialReference {
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
        })
    }

    fn username(&self, token_kind: &str) -> String {
        format!("{}:{}:{token_kind}", self.connection_id, self.generation_id)
    }

    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    pub fn generation_id(&self) -> &str {
        &self.generation_id
    }
}

pub struct OAuthTokenPair {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
}

impl OAuthTokenPair {
    pub fn new(access_token: String, refresh_token: String) -> Result<Self, CredentialStoreError> {
        let access_token = Zeroizing::new(access_token);
        let refresh_token = Zeroizing::new(refresh_token);
        validate_token(&access_token)?;
        validate_token(&refresh_token)?;

        Ok(Self {
            access_token,
            refresh_token,
        })
    }

    pub fn access_token(&self) -> &str {
        self.access_token.as_str()
    }

    pub fn refresh_token(&self) -> &str {
        self.refresh_token.as_str()
    }
}

#[derive(Default)]
pub struct CredentialStore;

impl CredentialStore {
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

        if credential_exists(&access_entry)? || credential_exists(&refresh_entry)? {
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

        Ok(())
    }

    pub fn load_token_pair(
        &self,
        reference: &OAuthCredentialReference,
    ) -> Result<OAuthTokenPair, CredentialStoreError> {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| CredentialStoreError::LockUnavailable)?;
        let access = read_raw_optional(&entry(reference, "access-v2")?)?;
        let refresh = read_raw_optional(&entry(reference, "refresh-v2")?)?;
        token_pair(access, refresh)?.ok_or(CredentialStoreError::TokenPairMissing)
    }

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialStoreError {
    InvalidReference,
    EmptyToken,
    TokenTooLarge,
    ReferenceAlreadyUsed,
    TokenPairMissing,
    IncompleteTokenPair,
    InvalidStoredToken,
    IncompleteWrite,
    StorageUnavailable,
    StorageReadFailed,
    StorageWriteFailed,
    StorageDeleteFailed,
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
    Entry::new(CREDENTIAL_SERVICE, &reference.username(token_kind))
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
) -> Result<Option<OAuthTokenPair>, CredentialStoreError> {
    match (access, refresh) {
        (Some(access_token), Some(refresh_token)) => {
            validate_token(&access_token)?;
            validate_token(&refresh_token)?;
            Ok(Some(OAuthTokenPair {
                access_token,
                refresh_token,
            }))
        }
        (None, None) => Ok(None),
        _ => Err(CredentialStoreError::IncompleteTokenPair),
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
