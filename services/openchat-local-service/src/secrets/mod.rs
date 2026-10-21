//! Shared access to OpenChat's per-user protected provider secret map.

use std::{
    collections::HashMap,
    env, fmt, fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::{
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::MapAccess, de::Visitor};
use zeroize::{Zeroize, Zeroizing};

const MAXIMUM_SECRET_MAP_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(windows)]
const SECRET_MAP_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(windows)]
const SECRET_MAP_LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(10);
const CHATGPT_API_KEY_PREFIX: &str = "openchat.chatgpt.api_key.";
const COMPATIBLE_KEY_PREFIX: &str = "openchat.compatible_provider.api_key.";
const CUSTOM_PROVIDER_KEY_PREFIX: &str = "openchat.custom_provider.api_key.";
const CUSTOM_PROVIDER_CONFIG_PREFIX: &str = "openchat.custom_provider.config.";
const OPENCODE_KEY: &str = "openchat.opencode.console_api_key";
const COMPATIBLE_PROVIDER_IDS: [&str; 16] = [
    "gemini",
    "groq",
    "cerebras",
    "openrouter",
    "mistral",
    "deepseek",
    "xai",
    "perplexity",
    "together",
    "sambanova",
    "anthropic",
    "fireworks",
    "deepinfra",
    "dashscope",
    "cohere",
    "kilo",
];

/// A validated secret-store operation error.
#[derive(Debug, Eq, PartialEq)]
pub enum SecretStoreError {
    /// The current user profile, protected file, or DPAPI operation is unavailable.
    Unavailable,
    /// A provider or connection identifier does not identify a stored API-key route.
    InvalidProviderRoute,
}

impl fmt::Display for SecretStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("The provider secret store is unavailable."),
            Self::InvalidProviderRoute => {
                formatter.write_str("The provider API-key route is invalid.")
            }
        }
    }
}

impl std::error::Error for SecretStoreError {}

/// Holds the current profile's cross-session file lock while updating the shared secret map.
#[must_use = "the lock must stay alive until the secret-map write completes"]
pub struct SecretMapUpdateLock {
    #[cfg(windows)]
    _lock_file: File,
}

impl SecretMapUpdateLock {
    /// Acquires the profile-scoped file lock shared by OpenChat secret-map writers in all sessions.
    ///
    /// The operation fails if another writer does not release the lock within 30 seconds.
    pub fn acquire() -> Result<Self, SecretStoreError> {
        #[cfg(windows)]
        {
            Self::acquire_for_path(&database_path()?)
        }
        #[cfg(not(windows))]
        {
            Err(SecretStoreError::Unavailable)
        }
    }

    fn acquire_for_path(path: &Path) -> Result<Self, SecretStoreError> {
        #[cfg(windows)]
        {
            use std::fs::TryLockError;

            let lock_path = secret_map_lock_path(path)?;
            let parent = lock_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .ok_or(SecretStoreError::Unavailable)?;
            fs::create_dir_all(parent).map_err(|_| SecretStoreError::Unavailable)?;
            let lock_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(lock_path)
                .map_err(|_| SecretStoreError::Unavailable)?;
            let deadline = Instant::now() + SECRET_MAP_LOCK_TIMEOUT;

            loop {
                match lock_file.try_lock() {
                    Ok(()) => {
                        return Ok(Self {
                            _lock_file: lock_file,
                        });
                    }
                    Err(TryLockError::WouldBlock) => {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            return Err(SecretStoreError::Unavailable);
                        }
                        thread::sleep(remaining.min(SECRET_MAP_LOCK_RETRY_INTERVAL));
                    }
                    Err(TryLockError::Error(_)) => return Err(SecretStoreError::Unavailable),
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            Err(SecretStoreError::Unavailable)
        }
    }
}

/// Owns decrypted secret values and clears them when the map is dropped.
#[derive(Default)]
pub struct SecretMap(HashMap<String, String>);

impl SecretMap {
    /// Loads the current user's protected secret map from the unified database.
    ///
    /// An absent or empty database table produces an empty map. Legacy files are automatically
    /// migrated into the database vault upon first read and removed.
    pub fn read() -> Result<Self, SecretStoreError> {
        #[cfg(windows)]
        {
            read_secret_map(&database_path()?)
        }
        #[cfg(not(windows))]
        {
            Err(SecretStoreError::Unavailable)
        }
    }

    /// Writes the current user's secret map into the unified database `secure_vault` table.
    ///
    /// Keep `update_lock` alive from the read through this write to prevent lost concurrent updates.
    pub fn write(&self, update_lock: &SecretMapUpdateLock) -> Result<(), SecretStoreError> {
        #[cfg(windows)]
        {
            write_secret_map(&database_path()?, self, update_lock)
        }
        #[cfg(not(windows))]
        {
            let _ = update_lock;
            Err(SecretStoreError::Unavailable)
        }
    }

    /// Returns the stored value for a key without copying it.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Returns whether the map contains a key, including one with an empty value.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Iterates over key names and borrowed values.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    /// Inserts a value and clears the previous value when replacing a key.
    pub fn insert(&mut self, key: String, value: String) {
        if let Some(mut replaced) = self.0.insert(key, value) {
            replaced.zeroize();
        }
    }

    /// Removes a value and clears its allocation when present.
    pub fn remove(&mut self, key: &str) {
        if let Some(mut removed) = self.0.remove(key) {
            removed.zeroize();
        }
    }

    /// Returns a zeroizing copy of the API key for a validated provider route.
    ///
    /// Custom provider keys are available only while that provider's configuration
    /// record exists. Empty and whitespace-only stored values are treated as absent.
    pub fn provider_api_key(
        &self,
        provider_id: &str,
        connection_id: Option<&str>,
    ) -> Result<Option<Zeroizing<String>>, SecretStoreError> {
        let storage_key = match provider_id {
            "chatgpt_api" => {
                let connection_id = connection_id.ok_or(SecretStoreError::InvalidProviderRoute)?;
                if !is_chatgpt_connection_id(connection_id) {
                    return Err(SecretStoreError::InvalidProviderRoute);
                }
                format!("{CHATGPT_API_KEY_PREFIX}{connection_id}")
            }
            "opencode" => {
                if connection_id.is_some() {
                    return Err(SecretStoreError::InvalidProviderRoute);
                }
                OPENCODE_KEY.to_owned()
            }
            provider_id if is_custom_provider_id(provider_id) => {
                if connection_id.is_some_and(|id| id != provider_id)
                    || !self.contains_key(&format!("{CUSTOM_PROVIDER_CONFIG_PREFIX}{provider_id}"))
                {
                    return Err(SecretStoreError::InvalidProviderRoute);
                }
                format!("{CUSTOM_PROVIDER_KEY_PREFIX}{provider_id}")
            }
            provider_id if COMPATIBLE_PROVIDER_IDS.contains(&provider_id) => {
                if connection_id.is_some_and(|id| id != provider_id) {
                    return Err(SecretStoreError::InvalidProviderRoute);
                }
                format!("{COMPATIBLE_KEY_PREFIX}{provider_id}")
            }
            _ => return Err(SecretStoreError::InvalidProviderRoute),
        };

        Ok(self
            .get(&storage_key)
            .filter(|value| !value.trim().is_empty())
            .map(|value| Zeroizing::new(value.to_owned())))
    }
}

impl Serialize for SecretMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SecretMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SecretMapVisitor;

        impl<'de> Visitor<'de> for SecretMapVisitor {
            type Value = SecretMap;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map of secret names to string values")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = SecretMap::default();
                while let Some((key, value)) = access.next_entry::<String, String>()? {
                    values.insert(key, value);
                }
                Ok(values)
            }
        }

        deserializer.deserialize_map(SecretMapVisitor)
    }
}

impl Drop for SecretMap {
    fn drop(&mut self) {
        for value in self.0.values_mut() {
            value.zeroize();
        }
    }
}

/// Resolves the current user's local SQLite database path.
///
/// # Errors
/// Returns `SecretStoreError::Unavailable` if `LOCALAPPDATA` is missing or not absolute.
fn database_path() -> Result<PathBuf, SecretStoreError> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or(SecretStoreError::Unavailable)?;
    let path = PathBuf::from(local_app_data)
        .join("OpenChat")
        .join("db")
        .join("openchat.sqlite3");
    if !path.is_absolute() {
        return Err(SecretStoreError::Unavailable);
    }
    Ok(path)
}

#[cfg(windows)]
fn open_database_connection(path: &Path) -> Result<rusqlite::Connection, SecretStoreError> {
    let parent = path.parent().ok_or(SecretStoreError::Unavailable)?;
    fs::create_dir_all(parent).map_err(|_| SecretStoreError::Unavailable)?;
    let connection = rusqlite::Connection::open(path).map_err(|_| SecretStoreError::Unavailable)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|_| SecretStoreError::Unavailable)?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS secure_vault (
                 key TEXT PRIMARY KEY NOT NULL,
                 ciphertext BLOB NOT NULL,
                 description TEXT,
                 updated_at INTEGER NOT NULL
             );",
        )
        .map_err(|_| SecretStoreError::Unavailable)?;
    Ok(connection)
}

#[cfg(windows)]
fn migrate_legacy_file_if_present(
    connection: &mut rusqlite::Connection,
) -> Result<(), SecretStoreError> {
    let legacy_path = match secure_storage_path() {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };
    if !legacy_path.is_file() {
        return Ok(());
    }
    let legacy_map = match read_secret_map(&legacy_path) {
        Ok(map) => map,
        Err(_) => return Ok(()),
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SecretStoreError::Unavailable)?
        .as_millis();
    let now = i64::try_from(timestamp).map_err(|_| SecretStoreError::Unavailable)?;

    let transaction = connection
        .transaction()
        .map_err(|_| SecretStoreError::Unavailable)?;
    for (key, value) in legacy_map.iter() {
        let encrypted = protect_dpapi(value.as_bytes())?;
        transaction
            .execute(
                "INSERT INTO secure_vault (key, ciphertext, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
                rusqlite::params![key, encrypted, now],
            )
            .map_err(|_| SecretStoreError::Unavailable)?;
    }
    transaction
        .commit()
        .map_err(|_| SecretStoreError::Unavailable)?;

    let _ = fs::remove_file(&legacy_path);
    if let Ok(lock_path) = secret_map_lock_path(&legacy_path) {
        let _ = fs::remove_file(&lock_path);
    }
    Ok(())
}

#[cfg(windows)]
fn read_secret_map_from_db(path: &Path) -> Result<SecretMap, SecretStoreError> {
    let mut connection = open_database_connection(path)?;
    migrate_legacy_file_if_present(&mut connection)?;
    let mut statement = connection
        .prepare("SELECT key, ciphertext FROM secure_vault")
        .map_err(|_| SecretStoreError::Unavailable)?;
    let rows = statement
        .query_map([], |row| {
            let key: String = row.get(0)?;
            let ciphertext: Vec<u8> = row.get(1)?;
            Ok((key, ciphertext))
        })
        .map_err(|_| SecretStoreError::Unavailable)?;

    let mut map = SecretMap::default();
    for item in rows {
        let (key, ciphertext) = item.map_err(|_| SecretStoreError::Unavailable)?;
        if key.starts_with("keyring:") {
            continue;
        }
        let plaintext = unprotect_dpapi(&ciphertext)?;
        if let Ok(value_str) = String::from_utf8(plaintext.to_vec()) {
            map.insert(key, value_str);
        }
    }
    Ok(map)
}

#[cfg(windows)]
fn write_secret_map_to_db(
    path: &Path,
    values: &SecretMap,
    _update_lock: &SecretMapUpdateLock,
) -> Result<(), SecretStoreError> {
    let mut connection = open_database_connection(path)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SecretStoreError::Unavailable)?
        .as_millis();
    let now = i64::try_from(timestamp).map_err(|_| SecretStoreError::Unavailable)?;

    let transaction = connection
        .transaction()
        .map_err(|_| SecretStoreError::Unavailable)?;
    let existing_keys: Vec<String> = {
        let mut statement = transaction
            .prepare("SELECT key FROM secure_vault")
            .map_err(|_| SecretStoreError::Unavailable)?;
        let rows = statement
            .query_map([], |row| row.get(0))
            .map_err(|_| SecretStoreError::Unavailable)?;
        let mut keys = Vec::new();
        for row in rows {
            keys.push(row.map_err(|_| SecretStoreError::Unavailable)?);
        }
        keys
    };

    for key in existing_keys {
        if !key.starts_with("keyring:") && !values.contains_key(&key) {
            transaction
                .execute(
                    "DELETE FROM secure_vault WHERE key = ?1",
                    rusqlite::params![key],
                )
                .map_err(|_| SecretStoreError::Unavailable)?;
        }
    }

    for (key, value) in values.iter() {
        let encrypted = protect_dpapi(value.as_bytes())?;
        transaction
            .execute(
                "INSERT INTO secure_vault (key, ciphertext, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
                rusqlite::params![key, encrypted, now],
            )
            .map_err(|_| SecretStoreError::Unavailable)?;
    }
    transaction
        .commit()
        .map_err(|_| SecretStoreError::Unavailable)?;

    if let Ok(legacy_path) = secure_storage_path() {
        let _ = fs::remove_file(&legacy_path);
        if let Ok(lock_path) = secret_map_lock_path(&legacy_path) {
            let _ = fs::remove_file(&lock_path);
        }
    }
    Ok(())
}

/// Loads a single secret by key from the secure vault, decrypting it using DPAPI.
pub fn get_vault_secret(key: &str) -> Result<Option<Zeroizing<String>>, SecretStoreError> {
    #[cfg(windows)]
    {
        get_vault_secret_for_path(&database_path()?, key)
    }
    #[cfg(not(windows))]
    {
        let _ = key;
        Err(SecretStoreError::Unavailable)
    }
}

/// Stores a single secret by key in the secure vault, encrypting it using DPAPI.
pub fn set_vault_secret(key: &str, value: &str) -> Result<(), SecretStoreError> {
    #[cfg(windows)]
    {
        set_vault_secret_for_path(&database_path()?, key, value)
    }
    #[cfg(not(windows))]
    {
        let _ = (key, value);
        Err(SecretStoreError::Unavailable)
    }
}

/// Removes a single secret by key from the secure vault.
pub fn delete_vault_secret(key: &str) -> Result<(), SecretStoreError> {
    #[cfg(windows)]
    {
        delete_vault_secret_for_path(&database_path()?, key)
    }
    #[cfg(not(windows))]
    {
        let _ = key;
        Err(SecretStoreError::Unavailable)
    }
}

/// Checks whether a secret exists in the secure vault by key.
pub fn has_vault_secret(key: &str) -> Result<bool, SecretStoreError> {
    #[cfg(windows)]
    {
        has_vault_secret_for_path(&database_path()?, key)
    }
    #[cfg(not(windows))]
    {
        let _ = key;
        Err(SecretStoreError::Unavailable)
    }
}

#[cfg(windows)]
pub(crate) fn get_vault_secret_for_path(
    path: &Path,
    key: &str,
) -> Result<Option<Zeroizing<String>>, SecretStoreError> {
    let connection = open_database_connection(path)?;
    let mut statement = connection
        .prepare("SELECT ciphertext FROM secure_vault WHERE key = ?1")
        .map_err(|_| SecretStoreError::Unavailable)?;
    let mut rows = statement
        .query([key])
        .map_err(|_| SecretStoreError::Unavailable)?;
    if let Some(row) = rows.next().map_err(|_| SecretStoreError::Unavailable)? {
        let ciphertext: Vec<u8> = row.get(0).map_err(|_| SecretStoreError::Unavailable)?;
        let plaintext = unprotect_dpapi(&ciphertext)?;
        let string_val =
            String::from_utf8(plaintext.to_vec()).map_err(|_| SecretStoreError::Unavailable)?;
        Ok(Some(Zeroizing::new(string_val)))
    } else {
        Ok(None)
    }
}

#[cfg(windows)]
pub(crate) fn set_vault_secret_for_path(
    path: &Path,
    key: &str,
    value: &str,
) -> Result<(), SecretStoreError> {
    let connection = open_database_connection(path)?;
    let encrypted = protect_dpapi(value.as_bytes())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SecretStoreError::Unavailable)?
        .as_millis();
    let now = i64::try_from(timestamp).map_err(|_| SecretStoreError::Unavailable)?;
    connection
        .execute(
            "INSERT INTO secure_vault (key, ciphertext, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
            rusqlite::params![key, encrypted, now],
        )
        .map_err(|_| SecretStoreError::Unavailable)?;
    Ok(())
}

#[cfg(windows)]
pub(crate) fn delete_vault_secret_for_path(path: &Path, key: &str) -> Result<(), SecretStoreError> {
    let connection = open_database_connection(path)?;
    connection
        .execute(
            "DELETE FROM secure_vault WHERE key = ?1",
            rusqlite::params![key],
        )
        .map_err(|_| SecretStoreError::Unavailable)?;
    Ok(())
}

#[cfg(windows)]
pub(crate) fn has_vault_secret_for_path(path: &Path, key: &str) -> Result<bool, SecretStoreError> {
    let connection = open_database_connection(path)?;
    let mut statement = connection
        .prepare("SELECT 1 FROM secure_vault WHERE key = ?1 LIMIT 1")
        .map_err(|_| SecretStoreError::Unavailable)?;
    let exists = statement
        .exists([key])
        .map_err(|_| SecretStoreError::Unavailable)?;
    Ok(exists)
}

/// Resolves the legacy per-user DPAPI secret-map file path.
///
/// # Errors
/// Returns `SecretStoreError::Unavailable` if `APPDATA` is missing or not absolute.
fn secure_storage_path() -> Result<PathBuf, SecretStoreError> {
    let app_data = env::var_os("APPDATA").ok_or(SecretStoreError::Unavailable)?;
    secret_map_path_from_app_data(&PathBuf::from(app_data))
}

/// Builds the secret-map path from an absolute application-data directory.
///
/// # Errors
/// Returns `SecretStoreError::Unavailable` when the supplied directory is not absolute.
fn secret_map_path_from_app_data(app_data: &Path) -> Result<PathBuf, SecretStoreError> {
    if !app_data.is_absolute() {
        return Err(SecretStoreError::Unavailable);
    }

    Ok(app_data
        .join("OpenChat contributors")
        .join("OpenChat")
        .join("flutter_secure_storage.dat"))
}

#[cfg(windows)]
fn secret_map_lock_path(path: &Path) -> Result<PathBuf, SecretStoreError> {
    let mut lock_file_name = path
        .file_name()
        .ok_or(SecretStoreError::Unavailable)?
        .to_os_string();
    lock_file_name.push(".lock");
    Ok(path.with_file_name(lock_file_name))
}

fn read_secret_map(path: &Path) -> Result<SecretMap, SecretStoreError> {
    #[cfg(windows)]
    {
        if path
            .extension()
            .is_some_and(|ext| ext == "sqlite3" || ext == "db")
        {
            return read_secret_map_from_db(path);
        }
        let encrypted = match read_bounded_file(path, MAXIMUM_SECRET_MAP_BYTES) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(SecretMap::default()),
            Err(_) => return Err(SecretStoreError::Unavailable),
        };
        let plaintext = unprotect_dpapi(&encrypted)?;
        serde_json::from_slice(&plaintext).map_err(|_| SecretStoreError::Unavailable)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(SecretStoreError::Unavailable)
    }
}

fn read_bounded_file(path: &Path, maximum_bytes: u64) -> std::io::Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "secret map exceeds the configured size limit",
        ));
    }
    Ok(Some(bytes))
}

fn write_secret_map(
    path: &Path,
    values: &SecretMap,
    update_lock: &SecretMapUpdateLock,
) -> Result<(), SecretStoreError> {
    #[cfg(windows)]
    {
        if path
            .extension()
            .is_some_and(|ext| ext == "sqlite3" || ext == "db")
        {
            return write_secret_map_to_db(path, values, update_lock);
        }
        let plaintext =
            Zeroizing::new(serde_json::to_vec(values).map_err(|_| SecretStoreError::Unavailable)?);
        if plaintext.len() as u64 > MAXIMUM_SECRET_MAP_BYTES {
            return Err(SecretStoreError::Unavailable);
        }
        let encrypted = protect_dpapi(&plaintext)?;
        if encrypted.len() as u64 > MAXIMUM_SECRET_MAP_BYTES {
            return Err(SecretStoreError::Unavailable);
        }
        write_atomic_bytes(path, &encrypted)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, values, update_lock);
        Err(SecretStoreError::Unavailable)
    }
}

fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), SecretStoreError> {
    let parent = path.parent().ok_or(SecretStoreError::Unavailable)?;
    fs::create_dir_all(parent).map_err(|_| SecretStoreError::Unavailable)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SecretStoreError::Unavailable)?
        .as_nanos();
    let file_name = path.file_name().ok_or(SecretStoreError::Unavailable)?;
    let staging = parent.join(format!(
        ".{}.{}.{}.tmp",
        file_name.to_string_lossy(),
        std::process::id(),
        timestamp
    ));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&staging);
        return Err(SecretStoreError::Unavailable);
    }
    if replace_file(&staging, path).is_err() {
        let _ = fs::remove_file(&staging);
        return Err(SecretStoreError::Unavailable);
    }
    Ok(())
}

#[cfg(windows)]
fn replace_file(staging: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let staging_wide = staging
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: Both path buffers are NUL-terminated and live through the Win32 call.
    let moved = unsafe {
        MoveFileExW(
            staging_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(staging: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(staging, destination)
}

/// Copies decrypted bytes into zeroizing storage and clears the DPAPI-owned source buffer.
fn copy_and_zeroize_dpapi_buffer(buffer: &mut [u8]) -> Zeroizing<Vec<u8>> {
    let plaintext = Zeroizing::new(buffer.to_vec());
    buffer.zeroize();
    plaintext
}

#[cfg(windows)]
/// Decrypts a DPAPI blob and clears the native plaintext buffer before returning.
fn unprotect_dpapi(encrypted: &[u8]) -> Result<Zeroizing<Vec<u8>>, SecretStoreError> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptUnprotectData},
    };

    let length = u32::try_from(encrypted.len()).map_err(|_| SecretStoreError::Unavailable)?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: length,
        pbData: encrypted.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` points to `encrypted` for its checked length and `output` is writable.
    let decrypted =
        unsafe { CryptUnprotectData(&input, null_mut(), null(), null(), null(), 0, &mut output) };
    if decrypted == 0 {
        if !output.pbData.is_null() {
            // SAFETY: A non-null failure buffer is owned by DPAPI and uses LocalAlloc.
            unsafe { LocalFree(output.pbData.cast()) };
        }
        return Err(SecretStoreError::Unavailable);
    }
    if output.pbData.is_null() {
        return Err(SecretStoreError::Unavailable);
    }
    if u64::from(output.cbData) > MAXIMUM_SECRET_MAP_BYTES {
        // SAFETY: DPAPI returned a non-null allocation with `cbData` writable bytes.
        unsafe {
            std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize).zeroize();
            LocalFree(output.pbData.cast());
        }
        return Err(SecretStoreError::Unavailable);
    }
    // Clear the DPAPI-owned plaintext before releasing its LocalAlloc buffer.
    // SAFETY: DPAPI returned a non-null allocation with `cbData` readable bytes.
    let plaintext = unsafe {
        let buffer = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let bytes = copy_and_zeroize_dpapi_buffer(buffer);
        LocalFree(output.pbData.cast());
        bytes
    };
    Ok(plaintext)
}

#[cfg(windows)]
fn protect_dpapi(plaintext: &[u8]) -> Result<Vec<u8>, SecretStoreError> {
    use std::ptr::null;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData},
    };

    let length = u32::try_from(plaintext.len()).map_err(|_| SecretStoreError::Unavailable)?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: length,
        pbData: plaintext.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` points to `plaintext` for its checked length and `output` is writable.
    let encrypted =
        unsafe { CryptProtectData(&input, null(), null(), null(), null(), 0, &mut output) };
    if encrypted == 0 || output.pbData.is_null() {
        if !output.pbData.is_null() {
            // SAFETY: Any non-null output buffer returned by DPAPI is owned by LocalAlloc.
            unsafe { LocalFree(output.pbData.cast()) };
        }
        return Err(SecretStoreError::Unavailable);
    }
    // DPAPI allocates the output buffer with LocalAlloc; release it after copying.
    // SAFETY: The non-null DPAPI output is readable for the returned `cbData` length.
    let ciphertext = unsafe {
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        bytes
    };
    Ok(ciphertext)
}

#[cfg(windows)]
/// Encrypts plaintext bytes using Windows DPAPI.
///
/// # Errors
/// Returns `SecretStoreError::Unavailable` if DPAPI encryption fails.
pub fn protect_secret(plaintext: &[u8]) -> Result<Vec<u8>, SecretStoreError> {
    protect_dpapi(plaintext)
}

#[cfg(windows)]
/// Decrypts ciphertext bytes using Windows DPAPI and returns zeroizing memory.
///
/// # Errors
/// Returns `SecretStoreError::Unavailable` if DPAPI decryption fails.
pub fn unprotect_secret(ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, SecretStoreError> {
    unprotect_dpapi(ciphertext)
}

fn is_chatgpt_connection_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn is_custom_provider_id(value: &str) -> bool {
    (19..=64).contains(&value.len())
        && value.starts_with("custom_")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use std::{
        fs::{self, OpenOptions, TryLockError},
        path::{Path, PathBuf},
        sync::{Arc, Barrier},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{SecretMap, SecretStoreError, copy_and_zeroize_dpapi_buffer};
    #[cfg(windows)]
    use super::{
        SecretMapUpdateLock, delete_vault_secret_for_path, get_vault_secret_for_path,
        has_vault_secret_for_path, read_bounded_file, read_secret_map, secret_map_lock_path,
        secret_map_path_from_app_data, set_vault_secret_for_path, write_secret_map,
    };

    #[test]
    fn secret_map_accepts_only_string_values() {
        let secrets = serde_json::from_str::<SecretMap>(r#"{"token":"value"}"#)
            .expect("string-valued secrets should deserialize");
        assert_eq!(secrets.get("token"), Some("value"));

        assert!(serde_json::from_str::<SecretMap>(r#"{"token":42}"#).is_err());
    }

    /// Verifies that path construction rejects empty and relative profile roots.
    #[cfg(windows)]
    #[test]
    fn secret_map_path_requires_absolute_app_data() {
        let app_data = Path::new(r"C:\Users\Profile\AppData\Roaming");
        let path = secret_map_path_from_app_data(app_data).expect("absolute APPDATA path");
        assert_eq!(
            path,
            app_data
                .join("OpenChat contributors")
                .join("OpenChat")
                .join("flutter_secure_storage.dat")
        );

        assert_eq!(
            secret_map_path_from_app_data(Path::new(r"AppData\Roaming")),
            Err(SecretStoreError::Unavailable)
        );
        assert_eq!(
            secret_map_path_from_app_data(Path::new("")),
            Err(SecretStoreError::Unavailable)
        );
    }

    #[test]
    fn duplicate_secret_keys_keep_the_last_value() {
        let secrets = serde_json::from_str::<SecretMap>(r#"{"token":"old","token":"new"}"#)
            .expect("duplicate secret keys should deserialize safely");
        assert_eq!(secrets.get("token"), Some("new"));
    }

    #[test]
    fn replacing_and_removing_secrets_updates_only_the_requested_key() {
        let mut secrets = SecretMap::default();
        secrets.insert("first".to_owned(), "old-secret".to_owned());
        secrets.insert("second".to_owned(), "other-secret".to_owned());

        secrets.insert("first".to_owned(), "new-secret".to_owned());
        assert_eq!(secrets.get("first"), Some("new-secret"));
        assert_eq!(secrets.get("second"), Some("other-secret"));

        secrets.remove("first");
        assert!(!secrets.contains_key("first"));
        assert_eq!(secrets.get("second"), Some("other-secret"));
    }

    #[test]
    fn api_keys_are_scoped_to_valid_provider_routes() {
        let mut secrets = SecretMap::default();
        secrets.insert(
            "openchat.compatible_provider.api_key.gemini".to_owned(),
            "gemini-secret".to_owned(),
        );
        secrets.insert(
            "openchat.custom_provider.config.custom_provider_123456".to_owned(),
            "{}".to_owned(),
        );
        secrets.insert(
            "openchat.custom_provider.api_key.custom_provider_123456".to_owned(),
            "custom-secret".to_owned(),
        );

        assert_eq!(
            secrets
                .provider_api_key("gemini", Some("gemini"))
                .expect("compatible provider route")
                .as_deref()
                .map(String::as_str),
            Some("gemini-secret")
        );
        assert_eq!(
            secrets
                .provider_api_key("custom_provider_123456", Some("custom_provider_123456"))
                .expect("custom provider route")
                .as_deref()
                .map(String::as_str),
            Some("custom-secret")
        );
        assert_eq!(
            secrets.provider_api_key("copilot", Some("copilot")),
            Err(SecretStoreError::InvalidProviderRoute)
        );
        assert_eq!(
            secrets.provider_api_key("custom_provider_123456", Some("other_custom_provider")),
            Err(SecretStoreError::InvalidProviderRoute)
        );
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_secret_map_round_trips_without_using_the_user_store_path() {
        let directory = TestDirectory::new();
        let path = directory.0.join("secrets.dat");
        let mut secrets = SecretMap::default();
        secrets.insert(
            "openchat.compatible_provider.api_key.gemini".to_owned(),
            "local-test-secret".to_owned(),
        );

        let update_lock = SecretMapUpdateLock::acquire_for_path(path.as_path())
            .expect("test secret map update lock should be available");
        write_secret_map(&path, &secrets, &update_lock).expect("write test secret map");
        drop(secrets);

        let restored = read_secret_map(&path).expect("read test secret map");
        assert_eq!(
            restored.get("openchat.compatible_provider.api_key.gemini"),
            Some("local-test-secret")
        );
    }

    #[cfg(windows)]
    #[test]
    fn bounded_file_reader_rejects_data_past_the_configured_limit() {
        let directory = TestDirectory::new();
        let path = directory.0.join("oversized.dat");
        fs::write(&path, b"123456789").expect("write oversized test data");

        let error = read_bounded_file(&path, 8).expect_err("oversized input should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[cfg(windows)]
    #[test]
    fn concurrent_locked_updates_preserve_every_secret_map_entry() {
        let directory = TestDirectory::new();
        let path = Arc::new(directory.0.join("concurrent-secrets.dat"));
        let start = Arc::new(Barrier::new(8));
        let writers = (0..8)
            .map(|index| {
                let path = Arc::clone(&path);
                let start = Arc::clone(&start);
                thread::spawn(move || {
                    start.wait();
                    let _update_lock = SecretMapUpdateLock::acquire_for_path(path.as_path())
                        .expect("secret map update lock should be available");
                    let mut values = read_secret_map(&path).expect("read current secret map");
                    values.insert(format!("entry_{index}"), format!("value_{index}"));
                    write_secret_map(&path, &values, &_update_lock)
                        .expect("write updated secret map");
                })
            })
            .collect::<Vec<_>>();

        for writer in writers {
            writer.join().expect("secret map writer should complete");
        }

        let values = read_secret_map(&path).expect("read all concurrent updates");
        for index in 0..8 {
            assert_eq!(
                values.get(&format!("entry_{index}")),
                Some(format!("value_{index}").as_str())
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn secret_map_file_lock_excludes_a_second_handle() {
        let directory = TestDirectory::new();
        let path = directory.0.join("secrets.dat");
        let lock_path = secret_map_lock_path(&path).expect("secret map lock path");
        let first = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .expect("create first secret map lock handle");
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .expect("create second secret map lock handle");

        first.try_lock().expect("acquire first secret map lock");
        assert!(matches!(second.try_lock(), Err(TryLockError::WouldBlock)));
    }

    #[cfg(windows)]
    #[test]
    fn secret_map_lock_path_is_scoped_to_the_storage_path() {
        let profile_path = Path::new(r"C:\Users\ProfileOne\AppData\Roaming\OpenChat\secrets.dat");
        let same_profile_path =
            Path::new(r"C:\Users\ProfileOne\AppData\Roaming\OpenChat\secrets.dat");
        let other_profile_path =
            Path::new(r"C:\Users\AnotherUser\AppData\Roaming\OpenChat\secrets.dat");

        let profile_lock_path = secret_map_lock_path(profile_path).expect("profile lock path");
        let same_profile_lock_path =
            secret_map_lock_path(same_profile_path).expect("same profile lock path");
        let other_profile_lock_path =
            secret_map_lock_path(other_profile_path).expect("other profile lock path");

        assert_eq!(profile_lock_path, same_profile_lock_path);
        assert_ne!(profile_lock_path, other_profile_lock_path);
        assert_eq!(
            profile_lock_path.file_name().and_then(|name| name.to_str()),
            Some("secrets.dat.lock")
        );
    }

    #[cfg(windows)]
    #[test]
    fn missing_dpapi_file_is_an_empty_secret_map() {
        let directory = TestDirectory::new();
        let secrets = read_secret_map(&directory.0.join("missing.dat"))
            .expect("missing secret map should be empty");
        assert!(secrets.iter().next().is_none());
    }

    #[test]
    fn copying_dpapi_plaintext_zeroizes_the_source_buffer() {
        let mut buffer = b"secret payload".to_vec();
        let plaintext = copy_and_zeroize_dpapi_buffer(&mut buffer);

        assert_eq!(plaintext.as_slice(), b"secret payload");
        assert!(buffer.iter().all(|byte| *byte == 0));
    }

    #[cfg(windows)]
    #[test]
    fn sqlite_vault_round_trips_and_encrypts_data() {
        let directory = TestDirectory::new();
        let db_path = directory.0.join("vault.sqlite3");
        let lock = SecretMapUpdateLock::acquire_for_path(&db_path).expect("acquire lock");
        let mut map = SecretMap::default();
        map.insert("test_key".to_string(), "super_secret_value".to_string());
        write_secret_map(&db_path, &map, &lock).expect("write to sqlite vault");

        // Verify read_secret_map decrypts it correctly
        let read = read_secret_map(&db_path).expect("read from sqlite vault");
        assert_eq!(read.get("test_key"), Some("super_secret_value"));

        // Direct inspect raw DB to ensure value is NOT plaintext in the database
        let conn = rusqlite::Connection::open(&db_path).expect("open raw db");
        let raw_blob: Vec<u8> = conn
            .query_row(
                "SELECT ciphertext FROM secure_vault WHERE key = 'test_key'",
                [],
                |row| row.get(0),
            )
            .expect("read raw ciphertext");
        assert_ne!(raw_blob, b"super_secret_value");
        assert!(!String::from_utf8_lossy(&raw_blob).contains("super_secret_value"));
    }

    #[cfg(windows)]
    #[test]
    fn vault_secret_crud_and_keyring_preservation() {
        let directory = TestDirectory::new();
        let db_path = directory.0.join("vault_crud.sqlite3");

        // 1. Set secret directly
        set_vault_secret_for_path(&db_path, "keyring:test_oauth", "token_12345")
            .expect("set vault secret");
        assert!(has_vault_secret_for_path(&db_path, "keyring:test_oauth").expect("has secret"));

        // 2. Read back
        let loaded = get_vault_secret_for_path(&db_path, "keyring:test_oauth").expect("get secret");
        assert_eq!(loaded.as_deref().map(|s| s.as_str()), Some("token_12345"));

        // 3. Ensure write_secret_map does NOT delete keyring keys
        let lock = SecretMapUpdateLock::acquire_for_path(&db_path).expect("acquire lock");
        let mut map = SecretMap::default();
        map.insert("provider_key".to_string(), "api_val".to_string());
        write_secret_map(&db_path, &map, &lock).expect("write secret map");

        // keyring key must still be intact
        let loaded_after =
            get_vault_secret_for_path(&db_path, "keyring:test_oauth").expect("get secret after");
        assert_eq!(
            loaded_after.as_deref().map(|s| s.as_str()),
            Some("token_12345")
        );

        // 4. Delete secret
        delete_vault_secret_for_path(&db_path, "keyring:test_oauth").expect("delete secret");
        assert!(
            !has_vault_secret_for_path(&db_path, "keyring:test_oauth")
                .expect("has secret after delete")
        );
        assert_eq!(
            get_vault_secret_for_path(&db_path, "keyring:test_oauth").expect("get deleted"),
            None
        );
    }

    #[cfg(windows)]
    struct TestDirectory(PathBuf);

    #[cfg(windows)]
    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after the Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "openchat-provider-secret-store-{}-{timestamp}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    #[cfg(windows)]
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
