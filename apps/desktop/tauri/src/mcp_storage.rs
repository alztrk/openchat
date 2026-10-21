//! Stores validated global MCP server metadata and encrypted environment values.
//!
//! Server configuration is kept under the current user's local application data directory;
//! secret values are delegated to the shared encrypted secret store and exposed only as
//! saved/not-saved status.

use std::{
    collections::HashSet,
    env, fs,
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) use openchat_local_service::secrets::{SecretMap, SecretMapUpdateLock};
use serde::{Deserialize, Deserializer, Serialize};
use zeroize::Zeroizing;

use crate::{
    file_ops::replace_file_atomically,
    request_bounds::{
        deserialize_bounded_text, deserialize_bounded_texts, BoundedText, BoundedVec,
    },
};

const MAXIMUM_CONFIG_BYTES: u64 = 1024 * 1024;
const MAXIMUM_SECRET_BYTES: usize = 2500;
pub(crate) const MAXIMUM_SERVER_ID_BYTES: usize = 24;
const MAXIMUM_SERVER_NAME_BYTES: usize = 320;
const MAXIMUM_SERVER_CATEGORY_BYTES: usize = 320;
const MAXIMUM_SERVER_DESCRIPTION_BYTES: usize = 16_000;
const MAXIMUM_SERVER_COMMAND_BYTES: usize = 3;
const MAXIMUM_INSTALL_DIRECTORY_BYTES: usize = 32_768;
const MAXIMUM_SERVER_ARGUMENT_BYTES: usize = 512;
const MAXIMUM_ENVIRONMENT_NAME_BYTES: usize = 128;
const MAXIMUM_DISCOVERED_TOOL_BYTES: usize = 2_000;
const MAXIMUM_SERVER_ARGUMENTS: usize = 32;
const MAXIMUM_ENVIRONMENT_NAMES: usize = 64;
const MAXIMUM_DISCOVERED_TOOLS: usize = 500;
const SECRET_KEY_PREFIX: &str = "openchat_global_mcp";

/// Deserializes one text field while enforcing its UTF-8 byte limit.
fn deserialize_limited_text<'de, D, const MAX_BYTES: usize>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAX_BYTES>(deserializer)
}

/// Deserializes a server ID with the same bound used by path validation.
fn deserialize_server_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_text::<D, MAXIMUM_SERVER_ID_BYTES>(deserializer)
}

/// Deserializes a display name within the existing character bound's UTF-8 maximum.
fn deserialize_server_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_text::<D, MAXIMUM_SERVER_NAME_BYTES>(deserializer)
}

/// Deserializes a category within the existing character bound's UTF-8 maximum.
fn deserialize_server_category<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_text::<D, MAXIMUM_SERVER_CATEGORY_BYTES>(deserializer)
}

/// Deserializes a description within the existing character bound's UTF-8 maximum.
fn deserialize_server_description<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_text::<D, MAXIMUM_SERVER_DESCRIPTION_BYTES>(deserializer)
}

/// Deserializes a command name before the command allowlist is checked.
fn deserialize_server_command<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_text::<D, MAXIMUM_SERVER_COMMAND_BYTES>(deserializer)
}

/// Deserializes the stored installation path within the Windows path-length bound.
fn deserialize_install_directory<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_text::<D, MAXIMUM_INSTALL_DIRECTORY_BYTES>(deserializer)
}

/// Deserializes an environment name within its existing ASCII byte limit.
fn deserialize_environment_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_text::<D, MAXIMUM_ENVIRONMENT_NAME_BYTES>(deserializer)
}

/// Deserializes a secret directly into zeroizing storage after checking its byte limit.
fn deserialize_secret_value<'de, D>(deserializer: D) -> Result<SecretValue, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_text::<D, MAXIMUM_SECRET_BYTES>(deserializer)
        .map(|value| SecretValue(Zeroizing::new(value)))
}

/// Deserializes a bounded list of bounded text values.
fn deserialize_limited_texts<'de, D, const MAX_ITEMS: usize, const MAX_BYTES: usize>(
    deserializer: D,
) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_texts::<D, MAX_ITEMS, MAX_BYTES>(deserializer)
}

/// Deserializes only the supported number and size of process arguments.
fn deserialize_server_arguments<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_texts::<D, MAXIMUM_SERVER_ARGUMENTS, MAXIMUM_SERVER_ARGUMENT_BYTES>(
        deserializer,
    )
}

/// Deserializes a bounded list of declared environment variable names.
fn deserialize_environment_names<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_texts::<D, MAXIMUM_ENVIRONMENT_NAMES, MAXIMUM_ENVIRONMENT_NAME_BYTES>(
        deserializer,
    )
}

/// Deserializes only the supported number and size of discovered tool names.
fn deserialize_discovered_tools<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_limited_texts::<D, MAXIMUM_DISCOVERED_TOOLS, MAXIMUM_DISCOVERED_TOOL_BYTES>(
        deserializer,
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Safe error contract for global MCP configuration and credential operations.
pub struct McpStorageError {
    code: &'static str,
    message: &'static str,
}

impl McpStorageError {
    pub(crate) fn invalid() -> Self {
        Self {
            code: "mcp_invalid_config",
            message: "The MCP server configuration is invalid.",
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self {
            code: "mcp_storage_unavailable",
            message: "The local MCP configuration could not be accessed.",
        }
    }

    pub(crate) fn probe_failed() -> Self {
        Self {
            code: "mcp_probe_failed",
            message: "The MCP server could not return its tool list.",
        }
    }

    pub(crate) fn missing_credentials() -> Self {
        Self {
            code: "mcp_required_credentials_missing",
            message: "Save the required environment values before discovering tools.",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Persisted configuration for an installed global MCP server.
pub struct GlobalMcpServer {
    #[serde(deserialize_with = "deserialize_server_id")]
    pub(crate) id: String,
    #[serde(deserialize_with = "deserialize_server_name")]
    pub(crate) name: String,
    #[serde(deserialize_with = "deserialize_server_category")]
    pub(crate) category: String,
    #[serde(deserialize_with = "deserialize_server_description")]
    pub(crate) description: String,
    #[serde(deserialize_with = "deserialize_server_command")]
    pub(crate) command: String,
    #[serde(default, deserialize_with = "deserialize_server_arguments")]
    pub(crate) args: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_environment_names")]
    pub(crate) required_env_keys: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_environment_names")]
    pub(crate) optional_env_keys: Vec<String>,
    #[serde(default)]
    pub(crate) installed: bool,
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default, deserialize_with = "deserialize_install_directory")]
    pub(crate) install_dir: String,
    #[serde(default, deserialize_with = "deserialize_discovered_tools")]
    pub(crate) discovered_tools: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Installed servers plus the number of invalid configuration files skipped during loading.
pub struct GlobalMcpLoadResult {
    servers: Vec<GlobalMcpServer>,
    invalid_config_count: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
/// Carries one MCP secret update and zeroizes its value when the request is dropped.
pub struct GlobalMcpSecretUpdate {
    #[serde(deserialize_with = "deserialize_environment_name")]
    environment_name: String,
    #[serde(deserialize_with = "deserialize_secret_value")]
    value: SecretValue,
}

/// Owns a deserialized secret so later validation errors also clear its allocation.
struct SecretValue(Zeroizing<String>);

impl<'de> Deserialize<'de> for SecretValue {
    /// Wraps a bounded deserialized secret so later failures still zeroize its allocation.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_secret_value(deserializer)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Presence-only status for one declared MCP environment variable.
pub struct GlobalMcpSecretStatus {
    environment_name: String,
    is_saved: bool,
}

#[tauri::command]
/// Loads installed MCP server configurations after validating each bounded JSON file.
///
/// Invalid, oversized, uninstalled, or mismatched files are skipped and counted instead of
/// preventing other valid servers from loading.
///
/// # Errors
/// Returns a storage error if the MCP configuration directory cannot be read.
pub fn load_global_mcp_servers() -> Result<GlobalMcpLoadResult, McpStorageError> {
    let root = mcp_directory()?;
    fs::create_dir_all(&root).map_err(|_| McpStorageError::unavailable())?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| McpStorageError::unavailable())?;

    // Migrate any legacy *.json files in mcp_directory
    if let Ok(entries) = fs::read_dir(&canonical_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Ok(bytes) = read_bounded_config_file(&path) {
                    if let Ok(server) = serde_json::from_slice::<GlobalMcpServer>(&bytes) {
                        if validate_server(&server).is_ok() && server.installed {
                            let key = format!("mcp_server.{}", server.id);
                            let _ = crate::chat_storage::set_app_setting_raw(
                                &key,
                                &String::from_utf8_lossy(&bytes),
                            );
                        }
                    }
                }
                let _ = fs::remove_file(&path);
            }
        }
    }

    let mut servers = Vec::new();
    let mut invalid_config_count = 0;

    let settings =
        crate::chat_storage::get_all_app_settings().map_err(|_| McpStorageError::unavailable())?;
    for (key, json_val) in settings {
        if let Some(server_id) = key.strip_prefix("mcp_server.") {
            let parsed = serde_json::from_str::<GlobalMcpServer>(&json_val);
            let Some(mut server) = parsed.ok() else {
                invalid_config_count += 1;
                continue;
            };
            if validate_server(&server).is_err() || !server.installed || server.id != server_id {
                invalid_config_count += 1;
                continue;
            }
            server.install_dir = install_directory(&canonical_root, &server.id)
                .to_string_lossy()
                .into_owned();
            servers.push(server);
        }
    }

    servers.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(GlobalMcpLoadResult {
        servers,
        invalid_config_count,
    })
}

#[tauri::command]
/// Persists a validated MCP server configuration and marks it installed and enabled.
///
/// This command records a server that has already been installed; it does not invoke a package
/// manager or start the server.
///
/// # Errors
/// Returns a validation or storage error. A failed write does not return a successful result.
pub fn save_global_mcp_server(
    mut server: GlobalMcpServer,
) -> Result<GlobalMcpServer, McpStorageError> {
    validate_server(&server)?;
    let root = mcp_directory()?;
    fs::create_dir_all(&root).map_err(|_| McpStorageError::unavailable())?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| McpStorageError::unavailable())?;
    let directory = install_directory(&canonical_root, &server.id);
    fs::create_dir_all(&directory).map_err(|_| McpStorageError::unavailable())?;
    let canonical_directory =
        fs::canonicalize(&directory).map_err(|_| McpStorageError::unavailable())?;
    if !canonical_directory.starts_with(&canonical_root) {
        return Err(McpStorageError::invalid());
    }
    server.installed = true;
    server.enabled = true;
    server.install_dir = canonical_directory.to_string_lossy().into_owned();
    write_server_config(&canonical_root, &server)?;
    Ok(server)
}

#[tauri::command]
/// Changes whether an installed MCP server is enabled for use.
///
/// # Errors
/// Returns a validation or storage error if the server configuration cannot be loaded or saved.
pub fn set_global_mcp_enabled(
    server_id: BoundedText<MAXIMUM_SERVER_ID_BYTES>,
    enabled: bool,
) -> Result<GlobalMcpServer, McpStorageError> {
    let server_id = server_id.into_inner();
    let root = mcp_directory()?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| McpStorageError::unavailable())?;
    let mut server = read_server_config(&canonical_root, &server_id)?;
    server.enabled = enabled;
    write_server_config(&canonical_root, &server)?;
    Ok(server)
}

#[tauri::command]
/// Removes the selected MCP server's stored configuration and managed install directory.
///
/// The server ID and resolved paths are validated before removal.
///
/// # Errors
/// Returns a validation or storage error. Files outside the app-managed server directory are
/// not intentionally selected for removal.
pub fn remove_global_mcp_server(
    server_id: BoundedText<MAXIMUM_SERVER_ID_BYTES>,
) -> Result<(), McpStorageError> {
    let server_id = server_id.into_inner();
    validate_server_id(&server_id)?;
    let root = mcp_directory()?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| McpStorageError::unavailable())?;
    let server = read_server_config(&canonical_root, &server_id)?;
    let directory = install_directory(&canonical_root, &server.id);
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            let canonical_directory =
                fs::canonicalize(&directory).map_err(|_| McpStorageError::unavailable())?;
            if !canonical_directory.starts_with(&canonical_root) {
                return Err(McpStorageError::invalid());
            }
            fs::remove_dir_all(canonical_directory).map_err(|_| McpStorageError::unavailable())?;
        }
        Ok(_) => return Err(McpStorageError::invalid()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(McpStorageError::unavailable()),
    }
    let key = format!("mcp_server.{}", server.id);
    let _ = crate::chat_storage::delete_app_setting_raw(&key);
    let config_path = config_path(&canonical_root, &server.id);
    let _ = fs::remove_file(config_path);
    Ok(())
}

#[tauri::command]
/// Reports whether requested, declared environment values have been saved.
///
/// Secret contents are never returned. Every requested name must be declared by the selected
/// server and must occur only once in the request.
///
/// # Errors
/// Returns a validation or secure-storage error.
pub fn get_global_mcp_secret_status(
    server_id: BoundedText<MAXIMUM_SERVER_ID_BYTES>,
    environment_names: BoundedVec<
        BoundedText<MAXIMUM_ENVIRONMENT_NAME_BYTES>,
        MAXIMUM_ENVIRONMENT_NAMES,
    >,
) -> Result<Vec<GlobalMcpSecretStatus>, McpStorageError> {
    let server_id = server_id.into_inner();
    let environment_names = environment_names
        .into_inner()
        .into_iter()
        .map(BoundedText::into_inner)
        .collect::<Vec<_>>();
    let server = require_server(&server_id)?;
    let allowed_names = all_environment_names(&server);
    let mut requested = HashSet::with_capacity(environment_names.len());
    for name in &environment_names {
        validate_environment_name(name)?;
        if !allowed_names.contains(name.as_str()) || !requested.insert(name) {
            return Err(McpStorageError::invalid());
        }
    }
    let values = read_secret_map()?;
    Ok(environment_names
        .into_iter()
        .map(|environment_name| GlobalMcpSecretStatus {
            is_saved: values
                .get(&secret_key(&server.id, &environment_name))
                .is_some_and(|value| !value.is_empty()),
            environment_name,
        })
        .collect())
}

#[tauri::command]
/// Saves or removes declared MCP environment values in the encrypted secret store.
///
/// Values are trimmed before storage; an empty or whitespace-only value removes the saved value.
/// Secret request allocations are zeroized when released.
///
/// # Errors
/// Returns a validation or secure-storage error. Invalid updates do not report success.
pub fn save_global_mcp_secrets(
    server_id: BoundedText<MAXIMUM_SERVER_ID_BYTES>,
    updates: BoundedVec<GlobalMcpSecretUpdate, MAXIMUM_ENVIRONMENT_NAMES>,
) -> Result<(), McpStorageError> {
    let server_id = server_id.into_inner();
    let updates = updates.into_inner();
    let server = require_server(&server_id)?;
    let allowed_names = all_environment_names(&server);
    let mut seen_names = HashSet::with_capacity(updates.len());
    let _update_lock = lock_secret_map()?;
    let mut values = read_secret_map()?;

    for update in updates {
        let secret = update.value.0;
        validate_environment_name(&update.environment_name)?;
        if !allowed_names.contains(update.environment_name.as_str())
            || !seen_names.insert(update.environment_name.clone())
        {
            return Err(McpStorageError::invalid());
        }
        if secret.len() > MAXIMUM_SECRET_BYTES || secret.contains('\0') {
            return Err(McpStorageError::invalid());
        }
        let key = secret_key(&server.id, &update.environment_name);
        if secret.trim().is_empty() {
            values.remove(&key);
        } else {
            values.insert(key, secret.trim().to_owned());
        }
    }

    write_secret_map(&values, &_update_lock)
}

/// Loads one installed server after validating its ID and persisted configuration.
pub(crate) fn require_server(server_id: &str) -> Result<GlobalMcpServer, McpStorageError> {
    validate_server_id(server_id)?;
    let root = mcp_directory()?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| McpStorageError::unavailable())?;
    read_server_config(&canonical_root, server_id)
}

/// Collects the required and optional environment names declared by a server.
pub(crate) fn all_environment_names(server: &GlobalMcpServer) -> HashSet<&str> {
    server
        .required_env_keys
        .iter()
        .chain(server.optional_env_keys.iter())
        .map(String::as_str)
        .collect()
}

/// Reads the shared encrypted secret map and maps backend errors to the MCP contract.
pub(crate) fn read_secret_map() -> Result<SecretMap, McpStorageError> {
    SecretMap::read().map_err(|_| McpStorageError::unavailable())
}

/// Persists the shared encrypted secret map and maps backend errors to the MCP contract.
pub(crate) fn write_secret_map(
    values: &SecretMap,
    update_lock: &SecretMapUpdateLock,
) -> Result<(), McpStorageError> {
    values
        .write(update_lock)
        .map_err(|_| McpStorageError::unavailable())
}

/// Acquires the cross-process lock required for read-modify-write secret updates.
pub(crate) fn lock_secret_map() -> Result<SecretMapUpdateLock, McpStorageError> {
    SecretMapUpdateLock::acquire().map_err(|_| McpStorageError::unavailable())
}

/// Enforces persisted server bounds and the executable/environment allowlists.
fn validate_server(server: &GlobalMcpServer) -> Result<(), McpStorageError> {
    validate_server_id(&server.id)?;
    if server.name.trim().is_empty()
        || server.name.chars().count() > 80
        || server.category.trim().is_empty()
        || server.category.chars().count() > 80
        || server.description.chars().count() > 4000
        || server.description.chars().any(char::is_control)
        || server.args.len() > MAXIMUM_SERVER_ARGUMENTS
        || server.args.iter().any(|argument| {
            argument.is_empty()
                || argument.len() > MAXIMUM_SERVER_ARGUMENT_BYTES
                || argument.chars().any(char::is_control)
                || !argument
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"@/._-".contains(&byte))
        })
        || !matches!(server.command.as_str(), "npx" | "uvx")
        || server.required_env_keys.len() + server.optional_env_keys.len()
            > MAXIMUM_ENVIRONMENT_NAMES
        || server.discovered_tools.len() > MAXIMUM_DISCOVERED_TOOLS
        || server.discovered_tools.iter().any(|tool| {
            tool.trim().is_empty()
                || tool.chars().count() > 500
                || tool.chars().any(char::is_control)
        })
    {
        return Err(McpStorageError::invalid());
    }
    let mut names = HashSet::new();
    for name in server
        .required_env_keys
        .iter()
        .chain(server.optional_env_keys.iter())
    {
        validate_environment_name(name)?;
        if !names.insert(name) {
            return Err(McpStorageError::invalid());
        }
    }
    Ok(())
}

/// Restricts IDs used to construct config and install-directory paths.
fn validate_server_id(value: &str) -> Result<(), McpStorageError> {
    if value.is_empty()
        || value.len() > MAXIMUM_SERVER_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(McpStorageError::invalid());
    }
    Ok(())
}

pub(crate) fn validate_environment_name(value: &str) -> Result<(), McpStorageError> {
    let mut bytes = value.bytes();
    let valid = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_uppercase() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        && value.len() <= MAXIMUM_ENVIRONMENT_NAME_BYTES;
    if !valid
        || matches!(
            value,
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
    {
        return Err(McpStorageError::invalid());
    }
    Ok(())
}

/// Namespaces one secret-store entry by server and declared environment variable.
fn secret_key(server_id: &str, environment_name: &str) -> String {
    format!("{SECRET_KEY_PREFIX}_{server_id}_{environment_name}")
}

/// Resolves the per-user directory containing MCP metadata and managed installations.
pub(crate) fn mcp_directory() -> Result<PathBuf, McpStorageError> {
    let local_app_data = env::var_os("LOCALAPPDATA")
        .or_else(|| {
            env::var_os("USERPROFILE").map(|path| {
                PathBuf::from(path)
                    .join("AppData")
                    .join("Local")
                    .into_os_string()
            })
        })
        .ok_or_else(McpStorageError::unavailable)?;
    Ok(PathBuf::from(local_app_data).join("OpenChat").join("mcp"))
}

/// Builds the managed install path from an already validated server ID.
fn install_directory(root: &Path, server_id: &str) -> PathBuf {
    root.join(server_id)
}

/// Builds the configuration path from an already validated server ID.
fn config_path(root: &Path, server_id: &str) -> PathBuf {
    root.join(format!("{server_id}.json"))
}

/// Reads a validated MCP server config from SQLite app_settings, falling back to legacy config file if present.
fn read_server_config(root: &Path, server_id: &str) -> Result<GlobalMcpServer, McpStorageError> {
    validate_server_id(server_id)?;
    let key = format!("mcp_server.{server_id}");
    if let Ok(Some(json_str)) = crate::chat_storage::get_app_setting_raw(&key) {
        let mut server = serde_json::from_str::<GlobalMcpServer>(&json_str)
            .map_err(|_| McpStorageError::invalid())?;
        validate_server(&server)?;
        if server.id != server_id || !server.installed {
            return Err(McpStorageError::invalid());
        }
        server.install_dir = install_directory(root, server_id)
            .to_string_lossy()
            .into_owned();
        return Ok(server);
    }

    let path = config_path(root, server_id);
    if path.is_file() {
        let bytes = read_bounded_config_file(&path)?;
        let mut server = serde_json::from_slice::<GlobalMcpServer>(&bytes)
            .map_err(|_| McpStorageError::invalid())?;
        validate_server(&server)?;
        if server.id != server_id || !server.installed {
            return Err(McpStorageError::invalid());
        }
        server.install_dir = install_directory(root, server_id)
            .to_string_lossy()
            .into_owned();
        let _ = write_server_config(root, &server);
        let _ = fs::remove_file(&path);
        return Ok(server);
    }

    Err(McpStorageError::invalid())
}

/// Opens one regular config file and reads no more than the configured limit plus one byte.
fn read_bounded_config_file(path: &Path) -> Result<Vec<u8>, McpStorageError> {
    ensure_regular_file(path)?;
    let file = fs::File::open(path).map_err(|_| McpStorageError::unavailable())?;
    let metadata = file
        .metadata()
        .map_err(|_| McpStorageError::unavailable())?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_CONFIG_BYTES {
        return Err(McpStorageError::invalid());
    }
    read_bounded_config_bytes(file)
}

/// Bounds bytes read even if the file grows after its metadata has been checked.
fn read_bounded_config_bytes(reader: impl Read) -> Result<Vec<u8>, McpStorageError> {
    let mut bytes = Vec::new();
    reader
        .take(MAXIMUM_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| McpStorageError::unavailable())?;
    if bytes.len() as u64 > MAXIMUM_CONFIG_BYTES {
        return Err(McpStorageError::invalid());
    }
    Ok(bytes)
}

pub(crate) fn write_server_config(
    root: &Path,
    server: &GlobalMcpServer,
) -> Result<(), McpStorageError> {
    validate_server(server)?;
    let json_str =
        serde_json::to_string_pretty(server).map_err(|_| McpStorageError::unavailable())?;
    if json_str.len() as u64 > MAXIMUM_CONFIG_BYTES {
        return Err(McpStorageError::invalid());
    }
    let key = format!("mcp_server.{}", server.id);
    crate::chat_storage::set_app_setting_raw(&key, &json_str)
        .map_err(|_| McpStorageError::unavailable())?;
    let legacy_path = config_path(root, &server.id);
    let _ = fs::remove_file(legacy_path);
    Ok(())
}

/// Rejects missing files, symlinks, and non-regular entries before config access.
fn ensure_regular_file(path: &Path) -> Result<(), McpStorageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(McpStorageError::invalid()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(McpStorageError::unavailable())
        }
        Err(_) => Err(McpStorageError::unavailable()),
    }
}

/// Writes bounded config bytes to a unique staging file before replacing the destination.
#[allow(dead_code)]
fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), McpStorageError> {
    let parent = path.parent().ok_or_else(McpStorageError::unavailable)?;
    fs::create_dir_all(parent).map_err(|_| McpStorageError::unavailable())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| McpStorageError::unavailable())?
        .as_nanos();
    let file_name = path.file_name().ok_or_else(McpStorageError::unavailable)?;
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
        return Err(McpStorageError::unavailable());
    }
    if replace_file_atomically(&staging, path).is_err() {
        let _ = fs::remove_file(&staging);
        return Err(McpStorageError::unavailable());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        read_bounded_config_bytes, GlobalMcpServer, SecretValue, MAXIMUM_CONFIG_BYTES,
        MAXIMUM_SECRET_BYTES,
    };
    use crate::request_bounds::{BoundedText, BoundedVec};
    use std::io::{self, Cursor, Read};

    struct RepeatingReader {
        bytes_read: usize,
    }

    impl Read for RepeatingReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            buffer.fill(b'x');
            self.bytes_read += buffer.len();
            Ok(buffer.len())
        }
    }

    #[test]
    fn bounded_config_reader_accepts_the_limit_and_stops_after_one_extra_byte() {
        let exact_limit = Cursor::new(vec![b'x'; MAXIMUM_CONFIG_BYTES as usize]);
        let bytes = read_bounded_config_bytes(exact_limit)
            .expect("config bytes at the limit should be accepted");
        assert_eq!(bytes.len() as u64, MAXIMUM_CONFIG_BYTES);

        let mut repeating = RepeatingReader { bytes_read: 0 };
        assert!(read_bounded_config_bytes(&mut repeating).is_err());
        assert_eq!(repeating.bytes_read, MAXIMUM_CONFIG_BYTES as usize + 1);
    }

    #[test]
    fn secret_update_deserializes_into_zeroizing_storage() {
        let update = serde_json::from_str::<super::GlobalMcpSecretUpdate>(
            r#"{"environmentName":"API_TOKEN","value":"test-secret"}"#,
        )
        .expect("secret update should deserialize");

        assert_eq!(update.value.0.as_str(), "test-secret");
    }

    #[test]
    fn bounded_sequence_accepts_limit_and_rejects_extra_items() {
        let accepted = serde_json::from_str::<BoundedVec<String, 2>>(r#"["first","second"]"#)
            .expect("a sequence at its limit should deserialize");
        assert_eq!(accepted.into_inner(), ["first", "second"]);

        let rejected = serde_json::from_str::<BoundedVec<String, 2>>(r#"["first","second",{}]"#);
        assert!(rejected.is_err());
    }

    #[test]
    fn bounded_text_accepts_its_limit_and_rejects_longer_values() {
        let accepted = serde_json::from_str::<BoundedText<4>>(r#""abcd""#)
            .expect("text at its byte limit should deserialize");
        assert_eq!(accepted.into_inner(), "abcd");

        assert!(serde_json::from_str::<BoundedText<4>>(r#""abcde""#).is_err());
    }

    #[test]
    fn secret_deserialization_enforces_the_existing_byte_limit() {
        let accepted_json = serde_json::to_string(&"s".repeat(MAXIMUM_SECRET_BYTES))
            .expect("bounded secret should serialize for the test");
        let accepted = serde_json::from_str::<SecretValue>(&accepted_json)
            .expect("a secret at its byte limit should deserialize");
        assert_eq!(accepted.0.len(), MAXIMUM_SECRET_BYTES);

        let rejected_json = serde_json::to_string(&"s".repeat(MAXIMUM_SECRET_BYTES + 1))
            .expect("oversized secret should serialize for the test");
        assert!(serde_json::from_str::<SecretValue>(&rejected_json).is_err());
    }

    #[test]
    fn server_deserialization_enforces_collection_limits() {
        let server = serde_json::json!({
            "id": "valid_server",
            "name": "Valid server",
            "category": "Tools",
            "description": "",
            "command": "npx",
            "args": vec!["--yes"; 32],
            "installed": true,
            "enabled": false,
        });
        serde_json::from_value::<GlobalMcpServer>(server.clone())
            .expect("server with the maximum supported arguments should deserialize");

        let mut oversized_server = server;
        oversized_server["args"] = serde_json::json!(vec!["--yes"; 33]);
        assert!(serde_json::from_value::<GlobalMcpServer>(oversized_server).is_err());
    }
}
