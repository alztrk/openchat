//! Connects to configured MCP servers and exposes bounded, permission-checked tools.
//!
//! Stdio servers use permission-mode-specific isolation with bounded process resources. Remote
//! transports use bounded HTTP framing, per-server approval, and explicit response, schema,
//! tool-count, and execution-time limits.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
    future::Future,
    io::{self, Write},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use futures_util::stream::{self, StreamExt};
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, CallToolResult, Tool},
    service::RunningService,
    transport::{
        StreamableHttpClientTransport, Transport, async_rw::AsyncRwTransport,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, ReadBuf},
    time::timeout,
};

use crate::{
    connector_packages::{self, InstalledPackageReference},
    permissions::ToolPermissionBroker,
    protocol::{EventSink, ServiceError},
};

use crate::provider_schema::ToolDefinition;

use super::terminal::windows_sandbox::{SandboxJobLimits, SandboxOptions, SandboxedProcess};

mod bounded_http;

const MCP_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_MCP_TOOLS_PER_SERVER: usize = 128;
const MAX_MCP_TOOLS_PER_REQUEST: usize = 256;
const MAX_MCP_SERVERS_PER_REQUEST: usize = 8;
const MAX_CONCURRENT_MCP_STARTUPS: usize = 4;
const MAX_MCP_ARGUMENTS_PER_SERVER: usize = 64;
const MAX_MCP_ARGUMENT_BYTES_PER_SERVER: usize = 32 * 1024;
const MAX_MCP_ARGUMENT_BYTES: usize = 16 * 1024;
const MAX_MCP_SCHEMA_BYTES: usize = 16 * 1024;
const MAX_MCP_DESCRIPTION_BYTES: usize = 4096;
const MAX_MCP_RESULT_BYTES: usize = 64 * 1024;
const MAX_MCP_FRAME_BYTES: usize = 1024 * 1024;
const MCP_ACTIVE_PROCESS_LIMIT: u32 = 32;
const MCP_PROCESS_MEMORY_LIMIT_BYTES: usize = 1024 * 1024 * 1024;
const MCP_JOB_MEMORY_LIMIT_BYTES: usize = 2 * 1024 * 1024 * 1024;
const MAX_PROVIDER_TOOL_NAME_BYTES: usize = 64;
const MAX_MCP_CALL_TIMEOUT: Duration = Duration::from_secs(120);
const MCP_CATALOG_DIRECTORY: &str = ".openchat";
const MCP_CATALOG_FILE: &str = "mcp.json";
const MAX_MCP_CATALOG_BYTES: u64 = 64 * 1024;
const MAX_MCP_PERMISSION_RULE_NAME_BYTES: usize = 128;
const MCP_PERMISSION_FINGERPRINT_HEX_BYTES: usize = 32;

#[derive(Clone)]
/// Validated process or remote transport settings for one project MCP server.
pub(crate) struct StdioServerConfig {
    pub(crate) id: String,
    pub(crate) permission_fingerprint: String,
    pub(crate) enabled: bool,
    pub(crate) program: PathBuf,
    pub(crate) package: Option<InstalledPackageReference>,
    pub(crate) package_directory: Option<PathBuf>,
    pub(crate) arguments: Vec<OsString>,
    pub(crate) environment_variables: Vec<String>,
    pub(crate) endpoint: Option<url::Url>,
    pub(crate) auth_environment_variable: Option<String>,
    pub(crate) project_id: String,
    pub(crate) working_directory: PathBuf,
}

/// Removes servers explicitly denied by the project's server-wide permission rule.
pub(crate) fn filter_denied_servers(
    configs: &[StdioServerConfig],
    rules: &super::ToolPermissionRules,
) -> Vec<StdioServerConfig> {
    configs
        .iter()
        .filter(|config| {
            !matches!(
                rules.get(&format!("mcp__{}__*", config.id)),
                Some(super::ToolPermissionRule::Deny)
            )
        })
        .cloned()
        .collect()
}

/// Returns the server rule bound to its current configuration, defaulting to asking the user.
pub(crate) fn server_permission_rule(
    rules: &super::ToolPermissionRules,
    server_id: &str,
    permission_fingerprint: &str,
) -> super::ToolPermissionRule {
    let legacy_rule = rules.get(&format!("mcp__{server_id}__*"));
    if legacy_rule == Some(&super::ToolPermissionRule::Deny) {
        return super::ToolPermissionRule::Deny;
    }
    rules
        .get(&server_permission_rule_name(
            server_id,
            permission_fingerprint,
        ))
        .copied()
        .unwrap_or(super::ToolPermissionRule::Ask)
}

pub(crate) fn server_permission_rule_name(server_id: &str, permission_fingerprint: &str) -> String {
    format!("mcpv1__{server_id}__*__{permission_fingerprint}")
}

pub(crate) fn tool_permission_rule_name(
    server_id: &str,
    tool_name: &str,
    permission_fingerprint: &str,
) -> String {
    format!("mcpv1__{server_id}__{tool_name}__{permission_fingerprint}")
}

pub(crate) fn is_mcp_permission_rule_name(name: &str) -> bool {
    name.starts_with("mcp__") || name.starts_with("mcpv1__")
}

pub(crate) fn is_bound_permission_rule_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("mcpv1__") else {
        return false;
    };
    valid_bound_permission_rule_suffix(suffix)
}

/// Connects enabled servers only after applying deny rules and collecting required approvals.
///
/// # Errors
/// Returns cancellation, approval broker, or connection errors without registering partial tools.
pub(crate) struct McpApprovalContext<'a> {
    pub(crate) rules: &'a super::ToolPermissionRules,
    pub(crate) full_access: bool,
    pub(crate) permission_broker: &'a ToolPermissionBroker,
    pub(crate) request_id: &'a Value,
    pub(crate) events: &'a EventSink,
    pub(crate) cancellation: &'a mut tokio::sync::watch::Receiver<bool>,
}

pub(crate) async fn connect_with_approval(
    storage: &crate::storage::AppStorage,
    configs: Vec<StdioServerConfig>,
    context: McpApprovalContext<'_>,
) -> Result<Option<McpRegistry>, ServiceError> {
    let McpApprovalContext {
        rules,
        full_access,
        permission_broker,
        request_id,
        events,
        cancellation,
    } = context;
    let mut approved_configs = Vec::with_capacity(configs.len());
    for config in configs {
        if !config.enabled {
            continue;
        }

        match server_permission_rule(rules, &config.id, &config.permission_fingerprint) {
            super::ToolPermissionRule::Deny => continue,
            super::ToolPermissionRule::Allow => approved_configs.push(config),
            super::ToolPermissionRule::Ask => {
                let transport = if config.endpoint.is_some() {
                    "streamable_http"
                } else {
                    "stdio"
                };
                let target = server_approval_target(&config);
                let approved = permission_broker
                    .request_approval(
                        request_id,
                        "mcp_server_start",
                        &target,
                        &json!({"transport": transport}),
                        events,
                        cancellation,
                    )
                    .await?;
                if approved {
                    approved_configs.push(config);
                }
            }
        }
    }

    McpRegistry::connect(approved_configs, full_access, storage)
        .await
        .map_err(|error| {
            ServiceError::new(
                "mcp_server_unavailable",
                "An enabled project MCP server could not be started or discovered.",
                false,
            )
            .with_details(error)
        })
}

fn server_approval_target(config: &StdioServerConfig) -> String {
    let target = match &config.endpoint {
        Some(endpoint) => endpoint.origin().ascii_serialization(),
        None => config.program.to_string_lossy().into_owned(),
    };
    format!("{} · {target}", config.id)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StdioServerSettings {
    id: String,
    enabled: bool,
    #[serde(default = "stdio_transport")]
    transport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    package: Option<InstalledPackageReference>,
    #[serde(default)]
    arguments: Vec<String>,
    #[serde(default)]
    environment_variables: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auth_environment_variable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    catalog_tool_names: Option<Vec<String>>,
}

fn stdio_transport() -> String {
    "stdio".to_owned()
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StdioServerCatalog {
    version: u32,
    servers: Vec<StdioServerSettings>,
}

#[cfg(test)]
fn parse_server_configs(
    value: Option<&Value>,
    working_directory: &Path,
) -> Result<Vec<StdioServerConfig>, crate::protocol::ServiceError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let settings: Vec<StdioServerSettings> =
        serde_json::from_value(value.clone()).map_err(|_| {
            crate::protocol::ServiceError::new(
                "invalid_mcp_servers",
                "One or more MCP server settings are invalid.",
                false,
            )
        })?;
    parse_server_settings(settings, working_directory, "test-project", Path::new("."))
}

fn parse_server_settings(
    settings: Vec<StdioServerSettings>,
    working_directory: &Path,
    project_id: &str,
    storage_root: &Path,
) -> Result<Vec<StdioServerConfig>, crate::protocol::ServiceError> {
    if settings.len() > MAX_MCP_SERVERS_PER_REQUEST {
        return Err(invalid_mcp_servers());
    }

    let mut seen_ids = HashSet::with_capacity(settings.len());
    let mut servers = Vec::with_capacity(settings.len());
    for setting in settings {
        if !valid_server_id(&setting.id)
            || !seen_ids.insert(setting.id.clone())
            || !valid_catalog_tool_names(setting.catalog_tool_names.as_deref())
        {
            return Err(invalid_mcp_servers());
        }
        let permission_fingerprint = server_permission_fingerprint(&setting)?;
        let (program, package_directory, endpoint, auth_environment_variable) =
            match setting.transport.as_str() {
                "stdio" => {
                    if setting.endpoint.is_some() || setting.auth_environment_variable.is_some() {
                        return Err(invalid_mcp_servers());
                    }
                    validate_stdio_settings(&setting)?;
                    match (setting.program.as_deref(), setting.package.as_ref()) {
                        (Some(program), None)
                            if !program.is_empty()
                                && program.len() <= 4096
                                && !program.contains('\0')
                                && Path::new(program).is_absolute() =>
                        {
                            (PathBuf::from(program), None, None, None)
                        }
                        (None, Some(package)) => {
                            let (program, package_directory) =
                                connector_packages::resolve_installed_package(
                                    storage_root,
                                    working_directory,
                                    &setting.id,
                                    package,
                                )
                                .map_err(|_| invalid_mcp_servers())?;
                            (program, Some(package_directory), None, None)
                        }
                        _ => return Err(invalid_mcp_servers()),
                    }
                }
                "streamableHttp" => {
                    if setting.program.is_some()
                        || setting.package.is_some()
                        || !setting.arguments.is_empty()
                        || !setting.environment_variables.is_empty()
                    {
                        return Err(invalid_mcp_servers());
                    }
                    let endpoint = setting
                        .endpoint
                        .as_deref()
                        .ok_or_else(invalid_mcp_servers)?;
                    let endpoint =
                        validate_remote_endpoint(endpoint).map_err(|_| invalid_mcp_servers())?;
                    if setting
                        .auth_environment_variable
                        .as_deref()
                        .is_some_and(|name| !crate::credentials::is_valid_environment_name(name))
                    {
                        return Err(invalid_mcp_servers());
                    }
                    (
                        PathBuf::new(),
                        None,
                        Some(endpoint),
                        setting.auth_environment_variable,
                    )
                }
                _ => return Err(invalid_mcp_servers()),
            };
        servers.push(StdioServerConfig {
            id: setting.id,
            permission_fingerprint,
            enabled: setting.enabled,
            program,
            package: setting.package,
            package_directory,
            arguments: setting.arguments.into_iter().map(OsString::from).collect(),
            environment_variables: setting.environment_variables,
            endpoint,
            auth_environment_variable,
            project_id: project_id.to_owned(),
            working_directory: working_directory.to_path_buf(),
        });
    }
    Ok(servers)
}

fn validate_stdio_settings(
    setting: &StdioServerSettings,
) -> Result<(), crate::protocol::ServiceError> {
    if setting.arguments.len() > MAX_MCP_ARGUMENTS_PER_SERVER
        || setting.environment_variables.len() > 32
    {
        return Err(invalid_mcp_servers());
    }
    let arguments_within_limit = setting
        .arguments
        .iter()
        .try_fold(0usize, |total, argument| {
            (!argument.contains('\0')).then(|| total.saturating_add(argument.len()))
        })
        .is_some_and(|total| total <= MAX_MCP_ARGUMENT_BYTES_PER_SERVER);
    if !arguments_within_limit {
        return Err(invalid_mcp_servers());
    }
    let mut environment_names = HashSet::with_capacity(setting.environment_variables.len());
    if setting.environment_variables.iter().any(|name| {
        !crate::credentials::is_valid_environment_name(name)
            || !environment_names.insert(name.clone())
    }) {
        return Err(invalid_mcp_servers());
    }
    Ok(())
}

/// Accepts only validated remote MCP endpoint URLs supported by the configured transport.
pub(crate) fn validate_remote_endpoint(value: &str) -> Result<url::Url, String> {
    let endpoint =
        url::Url::parse(value).map_err(|_| "The remote MCP endpoint URL is invalid.".to_owned())?;
    if endpoint.scheme() != "https" || endpoint.query().is_some() || endpoint.fragment().is_some() {
        return Err(
            "Remote MCP endpoints must use HTTPS without query or fragment data.".to_owned(),
        );
    }
    crate::tools::web_search::url_fetch::validate_http_url(endpoint)
}

/// Loads project MCP configurations and returns only validated server entries.
#[cfg(test)]
pub(crate) fn load_server_configs(
    project_root: &Path,
    project_id: &str,
) -> Result<Vec<StdioServerConfig>, crate::protocol::ServiceError> {
    load_server_configs_with_storage_root(project_root, project_id, project_root)
}

pub(crate) fn load_server_configs_with_storage_root(
    project_root: &Path,
    project_id: &str,
    storage_root: &Path,
) -> Result<Vec<StdioServerConfig>, crate::protocol::ServiceError> {
    let (root, catalog) = read_server_catalog(project_root, storage_root)?;
    parse_server_settings(catalog.servers, &root, project_id, storage_root)
}

/// Parses and validates a single project MCP server configuration.
#[cfg(test)]
pub(crate) fn parse_server_config(
    project_root: &Path,
    project_id: &str,
    value: &Value,
) -> Result<StdioServerConfig, crate::protocol::ServiceError> {
    parse_server_config_with_storage_root(project_root, project_id, value, project_root)
}

pub(crate) fn parse_server_config_with_storage_root(
    project_root: &Path,
    project_id: &str,
    value: &Value,
    storage_root: &Path,
) -> Result<StdioServerConfig, crate::protocol::ServiceError> {
    let root = fs::canonicalize(project_root).map_err(|_| invalid_mcp_servers())?;
    let setting: StdioServerSettings =
        serde_json::from_value(value.clone()).map_err(|_| invalid_mcp_servers())?;
    parse_server_settings(vec![setting], &root, project_id, storage_root)?
        .into_iter()
        .next()
        .ok_or_else(invalid_mcp_servers)
}

/// Reads the bounded project catalog used to render available MCP server definitions.
#[cfg(test)]
pub(crate) fn server_catalog(project_root: &Path) -> Result<Value, crate::protocol::ServiceError> {
    server_catalog_with_storage_root(project_root, project_root)
}

pub(crate) fn server_catalog_with_storage_root(
    project_root: &Path,
    storage_root: &Path,
) -> Result<Value, crate::protocol::ServiceError> {
    let (_, catalog) = read_server_catalog(project_root, storage_root)?;
    serde_json::to_value(catalog).map_err(|_| invalid_mcp_servers())
}

/// Validates and atomically saves the bounded project MCP catalog.
#[cfg(test)]
pub(crate) fn save_server_catalog(
    project_root: &Path,
    value: &Value,
) -> Result<Value, crate::protocol::ServiceError> {
    save_server_catalog_with_storage_root(project_root, value, project_root)
}

pub(crate) fn save_server_catalog_with_storage_root(
    project_root: &Path,
    value: &Value,
    storage_root: &Path,
) -> Result<Value, crate::protocol::ServiceError> {
    let root = fs::canonicalize(project_root).map_err(|_| invalid_mcp_servers())?;
    let root_directory =
        Dir::open_ambient_dir(&root, ambient_authority()).map_err(|_| invalid_mcp_servers())?;
    let catalog: StdioServerCatalog =
        serde_json::from_value(value.clone()).map_err(|_| invalid_mcp_servers())?;
    if catalog.version != 1 {
        return Err(invalid_mcp_servers());
    }
    parse_server_settings(catalog.servers.clone(), &root, "validation", storage_root)?;

    let directory = match project_mcp_directory(&root_directory)? {
        Some(directory) => directory,
        None => {
            root_directory
                .create_dir(MCP_CATALOG_DIRECTORY)
                .map_err(|_| invalid_mcp_servers())?;
            project_mcp_directory(&root_directory)?.ok_or_else(invalid_mcp_servers)?
        }
    };
    let catalog_path = Path::new(MCP_CATALOG_FILE);
    match directory.symlink_metadata(catalog_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(invalid_mcp_servers());
        }
        Ok(metadata) if !metadata.is_file() || metadata.len() > MAX_MCP_CATALOG_BYTES => {
            return Err(invalid_mcp_servers());
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(invalid_mcp_servers()),
    }
    let contents = serde_json::to_vec_pretty(&catalog).map_err(|_| invalid_mcp_servers())?;
    if contents.len() as u64 > MAX_MCP_CATALOG_BYTES {
        return Err(invalid_mcp_servers());
    }
    let temporary = format!(".mcp-{}.tmp", uuid::Uuid::new_v4());
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = directory
            .open_with(&temporary, &options)
            .map_err(|_| invalid_mcp_servers())?;
        file.write_all(&contents)
            .map_err(|_| invalid_mcp_servers())?;
        file.sync_all().map_err(|_| invalid_mcp_servers())?;
        directory
            .rename(&temporary, &directory, catalog_path)
            .map_err(|_| invalid_mcp_servers())
    })();
    if result.is_err() {
        let _ = directory.remove_file(&temporary);
    }
    result?;
    server_catalog_with_storage_root(&root, storage_root)
}

pub(crate) fn package_is_referenced(
    project_root: &Path,
    id: &str,
    reference: &InstalledPackageReference,
    storage_root: &Path,
) -> Result<bool, crate::protocol::ServiceError> {
    let (_, catalog) = read_server_catalog(project_root, storage_root)?;
    Ok(catalog.servers.iter().any(|server| {
        server.id == id
            && server
                .package
                .as_ref()
                .is_some_and(|package| package == reference)
    }))
}

fn project_mcp_directory(
    root_directory: &Dir,
) -> Result<Option<Dir>, crate::protocol::ServiceError> {
    match root_directory.symlink_metadata(MCP_CATALOG_DIRECTORY) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(invalid_mcp_servers())
        }
        Ok(_) => root_directory
            .open_dir(MCP_CATALOG_DIRECTORY)
            .map(Some)
            .map_err(|_| invalid_mcp_servers()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(invalid_mcp_servers()),
    }
}

fn read_server_catalog(
    project_root: &Path,
    storage_root: &Path,
) -> Result<(PathBuf, StdioServerCatalog), crate::protocol::ServiceError> {
    let root = fs::canonicalize(project_root).map_err(|_| invalid_mcp_servers())?;
    let root_directory =
        Dir::open_ambient_dir(&root, ambient_authority()).map_err(|_| invalid_mcp_servers())?;
    let Some(directory) = project_mcp_directory(&root_directory)? else {
        return Ok((
            root,
            StdioServerCatalog {
                version: 1,
                servers: Vec::new(),
            },
        ));
    };
    let catalog_path = Path::new(MCP_CATALOG_FILE);
    let metadata = match directory.symlink_metadata(catalog_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok((
                root,
                StdioServerCatalog {
                    version: 1,
                    servers: Vec::new(),
                },
            ));
        }
        Err(_) => return Err(invalid_mcp_servers()),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_MCP_CATALOG_BYTES
    {
        return Err(invalid_mcp_servers());
    }
    let source = super::read_bounded_file_from_directory(
        &directory,
        catalog_path,
        MAX_MCP_CATALOG_BYTES as usize,
    )
    .map_err(|_| invalid_mcp_servers())?;
    if source.len() as u64 > MAX_MCP_CATALOG_BYTES {
        return Err(invalid_mcp_servers());
    }
    let catalog: StdioServerCatalog =
        serde_json::from_slice(&source).map_err(|_| invalid_mcp_servers())?;
    if catalog.version != 1 {
        return Err(invalid_mcp_servers());
    }
    parse_server_settings(catalog.servers.clone(), &root, "validation", storage_root)?;
    Ok((root, catalog))
}

fn invalid_mcp_servers() -> crate::protocol::ServiceError {
    crate::protocol::ServiceError::new(
        "invalid_mcp_servers",
        "One or more MCP server settings are invalid.",
        false,
    )
}

fn server_permission_fingerprint(
    setting: &StdioServerSettings,
) -> Result<String, crate::protocol::ServiceError> {
    let mut definition = setting.clone();
    definition.enabled = true;
    definition.catalog_tool_names = None;
    let value = serde_json::to_value(definition).map_err(|_| invalid_mcp_servers())?;
    permission_fingerprint(&value).map_err(|_| invalid_mcp_servers())
}

fn valid_catalog_tool_names(names: Option<&[String]>) -> bool {
    let Some(names) = names else {
        return true;
    };
    if names.len() > MAX_MCP_TOOLS_PER_SERVER {
        return false;
    }

    let mut seen = HashSet::with_capacity(names.len());
    names.iter().all(|name| {
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            && seen.insert(name.to_ascii_lowercase())
    })
}

fn permission_fingerprint(value: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| "The MCP definition could not be fingerprinted.".to_owned())?;
    let digest = Sha256::digest(bytes);
    Ok(digest[..MCP_PERMISSION_FINGERPRINT_HEX_BYTES / 2]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn valid_permission_fingerprint(value: &str) -> bool {
    value.len() == MCP_PERMISSION_FINGERPRINT_HEX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) struct McpToolRoute {
    pub(crate) server_id: String,
    pub(crate) server_permission_fingerprint: String,
    pub(crate) tool_name: String,
    pub(crate) definition_fingerprint: String,
}

/// Owns connected MCP clients and the validated tool definitions they expose.
pub(crate) struct McpRegistry {
    clients: HashMap<String, McpClient>,
    definitions: Vec<ToolDefinition>,
    routes: HashMap<String, McpToolRoute>,
}

impl McpRegistry {
    /// Connects the selected servers and validates each returned tool catalog.
    pub(crate) async fn connect(
        configs: Vec<StdioServerConfig>,
        full_access: bool,
        storage: &crate::storage::AppStorage,
    ) -> Result<Option<Self>, String> {
        if configs.len() > MAX_MCP_SERVERS_PER_REQUEST {
            return Err("The configured MCP server count exceeds the supported limit.".to_owned());
        }

        let enabled_configs = configs
            .into_iter()
            .filter(|config| config.enabled)
            .collect::<Vec<_>>();
        let mut server_ids = HashSet::with_capacity(enabled_configs.len());
        if enabled_configs
            .iter()
            .any(|config| !valid_server_id(&config.id) || !server_ids.insert(&config.id))
        {
            return Err(
                "The configured MCP server identifiers are invalid or duplicated.".to_owned(),
            );
        }

        let operations = enabled_configs
            .into_iter()
            .map(|config| async move {
                let client = McpClient::connect(&config, full_access, storage).await?;
                let tools = client.list_tools().await?;
                Ok::<_, String>((config.id, config.permission_fingerprint, client, tools))
            })
            .collect::<Vec<_>>();
        let connected_servers = connect_mcp_servers_concurrently(operations).await;

        let mut clients = HashMap::new();
        let mut definitions = Vec::new();
        let mut routes = HashMap::new();
        for connected_server in connected_servers {
            let (server_id, server_permission_fingerprint, client, tools) = connected_server?;
            let (server_definitions, server_routes) =
                tool_definitions(&server_id, &server_permission_fingerprint, tools)?;
            if definitions.len().saturating_add(server_definitions.len())
                > MAX_MCP_TOOLS_PER_REQUEST
                || server_routes.keys().any(|name| routes.contains_key(name))
            {
                return Err(
                    "The MCP tool catalog exceeds request limits or contains duplicate names."
                        .to_owned(),
                );
            }
            definitions.extend(server_definitions);
            routes.extend(server_routes);
            clients.insert(server_id, client);
        }

        if clients.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            clients,
            definitions,
            routes,
        }))
    }

    /// Returns the validated provider-facing tool definitions for connected servers.
    pub(crate) fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    pub(crate) fn tool_names_for_server(&self, server_id: &str) -> Vec<String> {
        let prefix = format!("mcp__{server_id}__");
        self.definitions
            .iter()
            .filter_map(|tool| tool.name.strip_prefix(&prefix))
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn permission_bindings_for_server(&self, server_id: &str) -> Vec<(String, String)> {
        let mut bindings = self
            .routes
            .values()
            .filter(|route| route.server_id == server_id)
            .map(|route| {
                (
                    route.tool_name.clone(),
                    tool_permission_rule_name(
                        server_id,
                        &route.tool_name,
                        &route.definition_fingerprint,
                    ),
                )
            })
            .collect::<Vec<_>>();
        bindings.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        bindings
    }

    pub(crate) fn permission_fingerprints_for_tool(
        &self,
        provider_tool_name: &str,
    ) -> Option<(&str, &str)> {
        self.routes.get(provider_tool_name).map(|route| {
            (
                route.server_permission_fingerprint.as_str(),
                route.definition_fingerprint.as_str(),
            )
        })
    }

    pub(crate) fn server_id_for_tool(&self, provider_tool_name: &str) -> Option<&str> {
        self.routes
            .get(provider_tool_name)
            .map(|route| route.server_id.as_str())
    }

    pub(crate) fn tool_name_for_tool(&self, provider_tool_name: &str) -> Option<&str> {
        self.routes
            .get(provider_tool_name)
            .map(|route| route.tool_name.as_str())
    }

    /// Calls a server tool with the per-call input and output bounds enforced.
    pub(crate) async fn call_tool(
        &self,
        provider_tool_name: &str,
        arguments: Map<String, Value>,
        timeout_duration: Duration,
    ) -> Result<Value, String> {
        let route = self
            .routes
            .get(provider_tool_name)
            .ok_or_else(|| "The requested MCP tool is not in the active registry.".to_owned())?;
        let client = self
            .clients
            .get(&route.server_id)
            .ok_or_else(|| "The MCP server for this tool is unavailable.".to_owned())?;
        let result = client
            .call_tool(route.tool_name.clone(), arguments, timeout_duration)
            .await?;
        serde_json::to_value(result)
            .map_err(|_| "The MCP server returned an invalid tool result.".to_owned())
    }
}

async fn connect_mcp_servers_concurrently<F>(operations: Vec<F>) -> Vec<F::Output>
where
    F: Future,
{
    let mut completed = stream::iter(
        operations
            .into_iter()
            .enumerate()
            .map(|(index, operation)| async move { (index, operation.await) }),
    )
    .buffer_unordered(MAX_CONCURRENT_MCP_STARTUPS)
    .collect::<Vec<_>>()
    .await;
    completed.sort_unstable_by_key(|(index, _)| *index);
    completed
        .into_iter()
        .map(|(_, operation)| operation)
        .collect()
}

/// Converts validated MCP tool metadata into provider function definitions.
pub(crate) fn tool_definitions(
    server_id: &str,
    server_permission_fingerprint: &str,
    tools: Vec<Tool>,
) -> Result<(Vec<ToolDefinition>, HashMap<String, McpToolRoute>), String> {
    if !valid_server_id(server_id)
        || !valid_permission_fingerprint(server_permission_fingerprint)
        || tools.len() > MAX_MCP_TOOLS_PER_SERVER
    {
        return Err("The MCP server returned an unsupported tool catalog.".to_owned());
    }

    let mut definitions = Vec::with_capacity(tools.len());
    let mut routes = HashMap::with_capacity(tools.len());
    for tool in tools {
        if !valid_tool_name(&tool.name) {
            return Err(
                "The MCP server returned a tool name that cannot be routed safely.".to_owned(),
            );
        }
        let tool_name = tool.name.into_owned();
        let name = format!("mcp__{server_id}__{tool_name}");
        if name.len() > MAX_PROVIDER_TOOL_NAME_BYTES {
            return Err(
                "The MCP server returned a tool name that exceeds the supported length.".to_owned(),
            );
        }
        let parameters = serde_json::to_value(tool.input_schema.as_ref())
            .map_err(|_| "The MCP server returned an invalid tool schema.".to_owned())?;
        let schema_size = serde_json::to_vec(&parameters)
            .map_err(|_| "The MCP server returned an invalid tool schema.".to_owned())?
            .len();
        if schema_size > MAX_MCP_SCHEMA_BYTES {
            return Err(
                "The MCP server returned a tool schema that exceeds the supported size.".to_owned(),
            );
        }
        let description = tool
            .description
            .map(|value| value.into_owned())
            .unwrap_or_default();
        if description.len() > MAX_MCP_DESCRIPTION_BYTES {
            return Err(
                "The MCP server returned a tool description that exceeds the supported size."
                    .to_owned(),
            );
        }
        let definition = json!({
            "serverFingerprint": server_permission_fingerprint,
            "serverId": server_id,
            "toolName": tool_name,
            "description": description,
            "inputSchema": parameters,
        });
        let definition_fingerprint = permission_fingerprint(&definition)?;
        if routes
            .insert(
                name.clone(),
                McpToolRoute {
                    server_id: server_id.to_owned(),
                    server_permission_fingerprint: server_permission_fingerprint.to_owned(),
                    tool_name,
                    definition_fingerprint,
                },
            )
            .is_some()
        {
            return Err("The MCP server returned duplicate tool names.".to_owned());
        }
        definitions.push(ToolDefinition {
            name,
            description,
            parameters,
        });
    }
    Ok((definitions, routes))
}

/// Checks whether a server ID is safe to use in permission keys and managed paths.
pub(crate) fn valid_server_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 24
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Checks whether a server-provided tool name can be safely exposed through the tool schema.
pub(crate) fn valid_tool_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// Checks whether a permission-rule suffix uses the supported bounded identifier syntax.
pub(crate) fn valid_permission_rule_suffix(value: &str) -> bool {
    if value.len().saturating_add("mcp__".len()) > MAX_PROVIDER_TOOL_NAME_BYTES {
        return false;
    }
    valid_mcp_permission_rule_suffix(value, false)
}

pub(crate) fn valid_bound_permission_rule_suffix(value: &str) -> bool {
    value.len().saturating_add("mcpv1__".len()) <= MAX_MCP_PERMISSION_RULE_NAME_BYTES
        && valid_mcp_permission_rule_suffix(value, true)
}

fn valid_mcp_permission_rule_suffix(value: &str, bound_only: bool) -> bool {
    value.match_indices("__").any(|(separator, _)| {
        let server_id = &value[..separator];
        let remainder = &value[separator + 2..];
        if !valid_server_id(server_id) {
            return false;
        }
        if remainder == "*" {
            return !bound_only;
        }
        if let Some(fingerprint) = remainder.strip_prefix("*__") {
            return valid_permission_fingerprint(fingerprint);
        }
        if let Some((tool_name, fingerprint)) = remainder.rsplit_once("__")
            && valid_tool_name(tool_name)
            && valid_permission_fingerprint(fingerprint)
        {
            return true;
        }
        !bound_only && valid_tool_name(remainder)
    })
}

pub(crate) struct McpClient {
    service: RunningService<RoleClient, ()>,
}

impl McpClient {
    pub(crate) async fn connect(
        config: &StdioServerConfig,
        full_access: bool,
        storage: &crate::storage::AppStorage,
    ) -> Result<Self, String> {
        let service = if let Some(endpoint) = &config.endpoint {
            let client =
                crate::tools::web_search::url_fetch::pinned_public_http_client(endpoint, None)
                    .await
                    .map_err(|error| {
                        format!("The remote MCP endpoint could not be reached safely: {error}")
                    })?;
            let auth_token = if let Some(name) = &config.auth_environment_variable {
                let reference = crate::credentials::McpCredentialReference::new(
                    config.project_id.clone(),
                    config.id.clone(),
                    name.clone(),
                )
                .map_err(|_| "The remote MCP credential name is invalid.".to_owned())?;
                let secret = crate::credentials::CredentialStore
                    .load_mcp_secret(storage, &reference)
                    .map_err(|error| {
                        format!("The remote MCP credential is unavailable: {error}")
                    })?;
                Some(secret)
            } else {
                None
            };
            start_streamable_http_service(endpoint, client, auth_token).await?
        } else {
            let credentials = crate::credentials::CredentialStore;
            let environment = config
                .environment_variables
                .iter()
                .map(|name| {
                    let reference = crate::credentials::McpCredentialReference::new(
                        config.project_id.clone(),
                        config.id.clone(),
                        name.clone(),
                    )
                    .map_err(|_| "An MCP environment variable name is invalid.".to_owned())?;
                    let secret = credentials.load_mcp_secret(storage, &reference).map_err(|error| {
                        format!(
                            "The MCP credential for environment variable {name} is unavailable: {error}"
                        )
                    })?;
                    Ok((name.clone(), secret))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let transport = StdioTransport::spawn(config, &environment, full_access)?;
            timeout(MCP_STARTUP_TIMEOUT, ().serve(transport))
                .await
                .map_err(|_| {
                    "The MCP server did not initialize before the startup timeout.".to_owned()
                })?
                .map_err(|error| {
                    format!("The MCP server could not complete protocol initialization: {error}")
                })?
        };
        Ok(Self { service })
    }

    pub(crate) async fn list_tools(&self) -> Result<Vec<Tool>, String> {
        timeout(MCP_STARTUP_TIMEOUT, self.service.peer().list_all_tools())
            .await
            .map_err(|_| "The MCP server did not return its tools before the timeout.".to_owned())?
            .map_err(|error| format!("The MCP server could not return its tool list: {error}"))
    }

    pub(crate) async fn call_tool(
        &self,
        name: String,
        arguments: Map<String, Value>,
        timeout_duration: Duration,
    ) -> Result<CallToolResult, String> {
        let argument_size = serde_json::to_vec(&arguments)
            .map_err(|_| "The MCP tool arguments are invalid.".to_owned())?
            .len();
        if argument_size > MAX_MCP_ARGUMENT_BYTES
            || timeout_duration.is_zero()
            || timeout_duration > MAX_MCP_CALL_TIMEOUT
        {
            return Err("The MCP tool call exceeds supported limits.".to_owned());
        }
        let result = timeout(
            timeout_duration,
            self.service
                .peer()
                .call_tool(CallToolRequestParams::new(name).with_arguments(arguments)),
        )
        .await
        .map_err(|_| "The MCP tool did not finish before its timeout.".to_owned())?
        .map_err(|error| format!("The MCP server returned a tool-call error: {error}"))?;
        let result_size = serde_json::to_vec(&result)
            .map_err(|_| "The MCP server returned an invalid tool result.".to_owned())?
            .len();
        if result_size > MAX_MCP_RESULT_BYTES {
            return Err(
                "The MCP server returned a tool result that exceeds the supported size.".to_owned(),
            );
        }
        Ok(result)
    }
}

async fn start_streamable_http_service(
    endpoint: &url::Url,
    client: reqwest::Client,
    auth_token: Option<zeroize::Zeroizing<String>>,
) -> Result<RunningService<RoleClient, ()>, String> {
    let transport_config = StreamableHttpClientTransportConfig::with_uri(endpoint.as_str())
        .max_sse_event_size(MAX_MCP_RESULT_BYTES);
    let transport = StreamableHttpClientTransport::with_client(
        bounded_http::BoundedMcpHttpClient::new(client, auth_token),
        transport_config,
    );
    timeout(MCP_STARTUP_TIMEOUT, ().serve(transport))
        .await
        .map_err(|_| {
            "The remote MCP server did not initialize before the startup timeout.".to_owned()
        })?
        .map_err(|error| {
            format!("The remote MCP server could not complete protocol initialization: {error}")
        })
}

struct StdioTransport {
    transport: AsyncRwTransport<RoleClient, BoundedLineReader<File>, File>,
    process: SandboxedProcess,
    frame_limit_exceeded: Arc<AtomicBool>,
}

impl StdioTransport {
    fn spawn(
        config: &StdioServerConfig,
        environment: &[(String, zeroize::Zeroizing<String>)],
        full_access: bool,
    ) -> Result<Self, String> {
        let verified_package_file = config
            .package
            .as_ref()
            .map(|reference| {
                crate::connector_packages::open_verified_package(&config.program, reference)
            })
            .transpose()
            .map_err(|_| "The installed MCP package failed its integrity check.".to_owned())?;
        let package_read_paths = config.package_directory.iter().cloned().collect::<Vec<_>>();
        let options = SandboxOptions {
            additional_read_only_paths: &package_read_paths,
            job_limits: Some(SandboxJobLimits {
                active_process_limit: MCP_ACTIVE_PROCESS_LIMIT,
                process_memory_limit_bytes: MCP_PROCESS_MEMORY_LIMIT_BYTES,
                job_memory_limit_bytes: MCP_JOB_MEMORY_LIMIT_BYTES,
            }),
            ..SandboxOptions::default()
        };
        let mut process = if full_access {
            SandboxedProcess::spawn_program_without_sandbox(
                &config.program,
                &config.arguments,
                &config.working_directory,
                environment,
                options,
            )
        } else {
            SandboxedProcess::spawn_program_with_options(
                &config.program,
                &config.arguments,
                &config.working_directory,
                environment,
                options,
            )
        }
        .map_err(|error| format!("The MCP server could not start: {error}"))?;
        drop(verified_package_file);
        let stdin = process
            .take_stdin()
            .ok_or_else(|| "The MCP server stdin pipe is unavailable.".to_owned())?;
        let stdout = process
            .take_stdout()
            .ok_or_else(|| "The MCP server stdout pipe is unavailable.".to_owned())?;
        let frame_limit_exceeded = Arc::new(AtomicBool::new(false));
        let bounded_stdout = BoundedLineReader::new(stdout, Arc::clone(&frame_limit_exceeded));
        if let Some(mut stderr) = process.take_stderr() {
            tokio::spawn(async move {
                let mut buffer = [0_u8; 4096];
                while matches!(stderr.read(&mut buffer).await, Ok(count) if count > 0) {}
            });
        }
        Ok(Self {
            transport: AsyncRwTransport::new_client(bounded_stdout, stdin),
            process,
            frame_limit_exceeded,
        })
    }
}

struct BoundedLineReader<R> {
    reader: R,
    current_line_bytes: usize,
    frame_limit_exceeded: Arc<AtomicBool>,
}

impl<R> BoundedLineReader<R> {
    fn new(reader: R, frame_limit_exceeded: Arc<AtomicBool>) -> Self {
        Self {
            reader,
            current_line_bytes: 0,
            frame_limit_exceeded,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for BoundedLineReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let previous_length = buffer.filled().len();
        match Pin::new(&mut this.reader).poll_read(context, buffer) {
            Poll::Ready(Ok(())) => {
                for byte in &buffer.filled()[previous_length..] {
                    if *byte == b'\n' {
                        this.current_line_bytes = 0;
                        continue;
                    }
                    this.current_line_bytes = this.current_line_bytes.saturating_add(1);
                    if this.current_line_bytes > MAX_MCP_FRAME_BYTES {
                        this.frame_limit_exceeded.store(true, Ordering::Release);
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "MCP stdout frame exceeds the supported size.",
                        )));
                    }
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

impl Transport<RoleClient> for StdioTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        item: rmcp::service::TxJsonRpcMessage<RoleClient>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        self.transport.send(item)
    }

    async fn receive(&mut self) -> Option<rmcp::service::RxJsonRpcMessage<RoleClient>> {
        let message = self.transport.receive().await;
        if self.frame_limit_exceeded.load(Ordering::Acquire) {
            if self.process.start_kill().is_err() {
                eprintln!("mcp_frame_limit_process_termination_failed");
            }
            return None;
        }
        message
    }

    fn close(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        let result = self.process.start_kill();
        let close = self.transport.close();
        async move {
            result?;
            close.await
        }
    }
}

#[path = "mcp/tests.rs"]
#[cfg(test)]
mod tests;
