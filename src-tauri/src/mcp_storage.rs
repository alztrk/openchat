use std::{
    collections::{HashMap, HashSet},
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

const MAXIMUM_CONFIG_BYTES: u64 = 1024 * 1024;
const MAXIMUM_SECRET_BYTES: usize = 2500;
const SECRET_KEY_PREFIX: &str = "openchat_global_mcp";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
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
pub struct GlobalMcpServer {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) description: String,
    pub(crate) command: String,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    #[serde(default)]
    pub(crate) required_env_keys: Vec<String>,
    #[serde(default)]
    pub(crate) optional_env_keys: Vec<String>,
    #[serde(default)]
    pub(crate) installed: bool,
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) install_dir: String,
    #[serde(default)]
    pub(crate) discovered_tools: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalMcpLoadResult {
    servers: Vec<GlobalMcpServer>,
    invalid_config_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalMcpSecretUpdate {
    environment_name: String,
    value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalMcpSecretStatus {
    environment_name: String,
    is_saved: bool,
}

#[derive(Default)]
pub(crate) struct SecretMap(pub(crate) HashMap<String, String>);

impl Drop for SecretMap {
    fn drop(&mut self) {
        for value in self.0.values_mut() {
            value.zeroize();
        }
    }
}

#[tauri::command]
pub fn load_global_mcp_servers() -> Result<GlobalMcpLoadResult, McpStorageError> {
    let root = mcp_directory()?;
    fs::create_dir_all(&root).map_err(|_| McpStorageError::unavailable())?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| McpStorageError::unavailable())?;
    let entries = fs::read_dir(&canonical_root).map_err(|_| McpStorageError::unavailable())?;
    let mut servers = Vec::new();
    let mut invalid_config_count = 0;

    for entry in entries {
        let entry = entry.map_err(|_| McpStorageError::unavailable())?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
            _ => {
                invalid_config_count += 1;
                continue;
            }
        };
        if metadata.len() > MAXIMUM_CONFIG_BYTES {
            invalid_config_count += 1;
            continue;
        }
        let parsed = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<GlobalMcpServer>(&bytes).ok());
        let Some(mut server) = parsed else {
            invalid_config_count += 1;
            continue;
        };
        if validate_server(&server).is_err() || !server.installed {
            invalid_config_count += 1;
            continue;
        }
        let Some(file_id) = path.file_stem().and_then(|value| value.to_str()) else {
            invalid_config_count += 1;
            continue;
        };
        if file_id != server.id {
            invalid_config_count += 1;
            continue;
        }
        server.install_dir = install_directory(&canonical_root, &server.id)
            .to_string_lossy()
            .into_owned();
        servers.push(server);
    }

    servers.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(GlobalMcpLoadResult {
        servers,
        invalid_config_count,
    })
}

#[tauri::command]
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
pub fn set_global_mcp_enabled(
    server_id: String,
    enabled: bool,
) -> Result<GlobalMcpServer, McpStorageError> {
    let root = mcp_directory()?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| McpStorageError::unavailable())?;
    let mut server = read_server_config(&canonical_root, &server_id)?;
    server.enabled = enabled;
    write_server_config(&canonical_root, &server)?;
    Ok(server)
}

#[tauri::command]
pub fn remove_global_mcp_server(server_id: String) -> Result<(), McpStorageError> {
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
    let config_path = config_path(&canonical_root, &server.id);
    ensure_regular_file(&config_path)?;
    fs::remove_file(config_path).map_err(|_| McpStorageError::unavailable())
}

#[tauri::command]
pub fn get_global_mcp_secret_status(
    server_id: String,
    environment_names: Vec<String>,
) -> Result<Vec<GlobalMcpSecretStatus>, McpStorageError> {
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
                .0
                .get(&secret_key(&server.id, &environment_name))
                .is_some_and(|value| !value.is_empty()),
            environment_name,
        })
        .collect())
}

#[tauri::command]
pub fn save_global_mcp_secrets(
    server_id: String,
    updates: Vec<GlobalMcpSecretUpdate>,
) -> Result<(), McpStorageError> {
    let server = require_server(&server_id)?;
    let allowed_names = all_environment_names(&server);
    let mut seen_names = HashSet::with_capacity(updates.len());
    let mut values = read_secret_map()?;

    for update in updates {
        validate_environment_name(&update.environment_name)?;
        if !allowed_names.contains(update.environment_name.as_str())
            || !seen_names.insert(update.environment_name.clone())
        {
            return Err(McpStorageError::invalid());
        }
        let secret = Zeroizing::new(update.value);
        if secret.len() > MAXIMUM_SECRET_BYTES || secret.contains('\0') {
            return Err(McpStorageError::invalid());
        }
        let key = secret_key(&server.id, &update.environment_name);
        if secret.trim().is_empty() {
            values.0.remove(&key);
        } else {
            values.0.insert(key, secret.trim().to_owned());
        }
    }

    write_secret_map(&values)
}

#[tauri::command]
pub fn get_active_global_mcp_instructions() -> Result<String, McpStorageError> {
    let loaded = load_global_mcp_servers()?;
    let active = loaded
        .servers
        .into_iter()
        .filter(|server| server.enabled)
        .collect::<Vec<_>>();
    if active.is_empty() {
        return Ok(String::new());
    }

    let mut instructions = String::from(
        "Active Model Context Protocol (MCP) Tools Available in this environment:\nThe assistant has access to the following active MCP servers. When relevant to the user request, use these tools to fetch fresh data, perform actions, or interact with external services.\n",
    );
    for server in active {
        instructions.push_str(&format!(
            "\n### Active MCP Tool: {} ({})\n{}",
            server.name, server.id, server.description
        ));
        if !server.discovered_tools.is_empty() {
            instructions.push_str("\nAvailable Tools:");
            for tool in server.discovered_tools {
                instructions.push_str("\n- ");
                instructions.push_str(&tool);
            }
        }
        instructions.push('\n');
    }
    Ok(instructions.trim().to_owned())
}

pub(crate) fn require_server(server_id: &str) -> Result<GlobalMcpServer, McpStorageError> {
    validate_server_id(server_id)?;
    let root = mcp_directory()?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| McpStorageError::unavailable())?;
    read_server_config(&canonical_root, server_id)
}

pub(crate) fn all_environment_names(server: &GlobalMcpServer) -> HashSet<&str> {
    server
        .required_env_keys
        .iter()
        .chain(server.optional_env_keys.iter())
        .map(String::as_str)
        .collect()
}

pub(crate) fn read_secret_map() -> Result<SecretMap, McpStorageError> {
    #[cfg(windows)]
    {
        let path = secure_storage_path()?;
        let encrypted = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SecretMap::default());
            }
            Err(_) => return Err(McpStorageError::unavailable()),
        };
        if encrypted.len() > 16 * 1024 * 1024 {
            return Err(McpStorageError::unavailable());
        }
        let plaintext = Zeroizing::new(unprotect_dpapi(&encrypted)?);
        let parsed: Value =
            serde_json::from_slice(&plaintext).map_err(|_| McpStorageError::unavailable())?;
        let object = parsed
            .as_object()
            .ok_or_else(McpStorageError::unavailable)?;
        let mut values = HashMap::with_capacity(object.len());
        for (key, value) in object {
            if let Some(value) = value.as_str() {
                values.insert(key.clone(), value.to_owned());
            }
        }
        Ok(SecretMap(values))
    }
    #[cfg(not(windows))]
    {
        Err(McpStorageError::unavailable())
    }
}

pub(crate) fn write_secret_map(values: &SecretMap) -> Result<(), McpStorageError> {
    #[cfg(windows)]
    {
        let json = Zeroizing::new(
            serde_json::to_vec(&values.0).map_err(|_| McpStorageError::unavailable())?,
        );
        let encrypted = protect_dpapi(&json)?;
        write_atomic_bytes(&secure_storage_path()?, &encrypted)
    }
    #[cfg(not(windows))]
    {
        let _ = values;
        Err(McpStorageError::unavailable())
    }
}

fn validate_server(server: &GlobalMcpServer) -> Result<(), McpStorageError> {
    validate_server_id(&server.id)?;
    if server.name.trim().is_empty()
        || server.name.chars().count() > 80
        || server.category.trim().is_empty()
        || server.category.chars().count() > 80
        || server.description.chars().count() > 4000
        || server.description.chars().any(char::is_control)
        || server.args.len() > 32
        || server.args.iter().any(|argument| {
            argument.is_empty()
                || argument.len() > 512
                || argument.chars().any(char::is_control)
                || !argument
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"@/._-".contains(&byte))
        })
        || !matches!(server.command.as_str(), "npx" | "uvx")
        || server.required_env_keys.len() + server.optional_env_keys.len() > 64
        || server.discovered_tools.len() > 500
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

fn validate_server_id(value: &str) -> Result<(), McpStorageError> {
    if value.is_empty()
        || value.len() > 24
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
        && value.len() <= 128;
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

fn secret_key(server_id: &str, environment_name: &str) -> String {
    format!("{SECRET_KEY_PREFIX}_{server_id}_{environment_name}")
}

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

fn install_directory(root: &Path, server_id: &str) -> PathBuf {
    root.join(server_id)
}

fn config_path(root: &Path, server_id: &str) -> PathBuf {
    root.join(format!("{server_id}.json"))
}

fn read_server_config(root: &Path, server_id: &str) -> Result<GlobalMcpServer, McpStorageError> {
    validate_server_id(server_id)?;
    let path = config_path(root, server_id);
    ensure_regular_file(&path)?;
    let metadata = fs::metadata(&path).map_err(|_| McpStorageError::unavailable())?;
    if metadata.len() > MAXIMUM_CONFIG_BYTES {
        return Err(McpStorageError::invalid());
    }
    let bytes = fs::read(&path).map_err(|_| McpStorageError::unavailable())?;
    let mut server = serde_json::from_slice::<GlobalMcpServer>(&bytes)
        .map_err(|_| McpStorageError::invalid())?;
    validate_server(&server)?;
    if server.id != server_id || !server.installed {
        return Err(McpStorageError::invalid());
    }
    server.install_dir = install_directory(root, server_id)
        .to_string_lossy()
        .into_owned();
    Ok(server)
}

pub(crate) fn write_server_config(
    root: &Path,
    server: &GlobalMcpServer,
) -> Result<(), McpStorageError> {
    validate_server(server)?;
    let bytes = serde_json::to_vec_pretty(server).map_err(|_| McpStorageError::unavailable())?;
    if bytes.len() as u64 > MAXIMUM_CONFIG_BYTES {
        return Err(McpStorageError::invalid());
    }
    write_atomic_bytes(&config_path(root, &server.id), &bytes)
}

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
    if replace_file(&staging, path).is_err() {
        let _ = fs::remove_file(&staging);
        return Err(McpStorageError::unavailable());
    }
    Ok(())
}

#[cfg(windows)]
fn replace_file(staging: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
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

#[cfg(windows)]
fn secure_storage_path() -> Result<PathBuf, McpStorageError> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| {
            path.join("OpenChat contributors")
                .join("OpenChat")
                .join("flutter_secure_storage.dat")
        })
        .ok_or_else(McpStorageError::unavailable)
}

#[cfg(windows)]
fn unprotect_dpapi(encrypted: &[u8]) -> Result<Vec<u8>, McpStorageError> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB},
    };

    let length = u32::try_from(encrypted.len()).map_err(|_| McpStorageError::unavailable())?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: length,
        pbData: encrypted.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let decrypted =
        unsafe { CryptUnprotectData(&input, null_mut(), null(), null(), null(), 0, &mut output) };
    if decrypted == 0 || output.pbData.is_null() {
        return Err(McpStorageError::unavailable());
    }
    // DPAPI allocates the output buffer with LocalAlloc; release it after copying.
    let plaintext = unsafe {
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        bytes
    };
    Ok(plaintext)
}

#[cfg(windows)]
fn protect_dpapi(plaintext: &[u8]) -> Result<Vec<u8>, McpStorageError> {
    use std::ptr::null;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB},
    };

    let length = u32::try_from(plaintext.len()).map_err(|_| McpStorageError::unavailable())?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: length,
        pbData: plaintext.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let encrypted =
        unsafe { CryptProtectData(&input, null(), null(), null(), null(), 0, &mut output) };
    if encrypted == 0 || output.pbData.is_null() {
        return Err(McpStorageError::unavailable());
    }
    // DPAPI allocates the output buffer with LocalAlloc; release it after copying.
    let ciphertext = unsafe {
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        bytes
    };
    Ok(ciphertext)
}
