//! Connects to configured MCP servers and exposes bounded, permission-checked tools.
//!
//! Stdio servers are launched through the Windows sandbox with restricted environment and
//! process resources. Remote transports use bounded HTTP framing, per-server approval, and
//! explicit response, schema, tool-count, and execution-time limits.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
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

#[derive(Clone)]
/// Validated process or remote transport settings for one project MCP server.
pub(crate) struct StdioServerConfig {
    pub(crate) id: String,
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

/// Returns the server-wide MCP rule, defaulting to asking the user.
pub(crate) fn server_permission_rule(
    rules: &super::ToolPermissionRules,
    server_id: &str,
) -> super::ToolPermissionRule {
    rules
        .get(&format!("mcp__{server_id}__*"))
        .copied()
        .unwrap_or(super::ToolPermissionRule::Ask)
}

/// Connects enabled servers only after applying deny rules and collecting required approvals.
///
/// # Errors
/// Returns cancellation, approval broker, or connection errors without registering partial tools.
pub(crate) async fn connect_with_approval(
    configs: Vec<StdioServerConfig>,
    rules: &super::ToolPermissionRules,
    permission_broker: &ToolPermissionBroker,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<Option<McpRegistry>, ServiceError> {
    let mut approved_configs = Vec::with_capacity(configs.len());
    for config in configs {
        if !config.enabled {
            continue;
        }

        match server_permission_rule(rules, &config.id) {
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

    McpRegistry::connect(approved_configs).await.map_err(|_| {
        ServiceError::new(
            "mcp_server_unavailable",
            "An enabled project MCP server could not be started or discovered.",
            false,
        )
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
        if !valid_server_id(&setting.id) || !seen_ids.insert(setting.id.clone()) {
            return Err(invalid_mcp_servers());
        }
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

pub(crate) struct McpToolRoute {
    pub(crate) server_id: String,
    pub(crate) tool_name: String,
}

/// Owns connected MCP clients and the validated tool definitions they expose.
pub(crate) struct McpRegistry {
    clients: HashMap<String, McpClient>,
    definitions: Vec<ToolDefinition>,
    routes: HashMap<String, McpToolRoute>,
}

impl McpRegistry {
    /// Connects the selected servers and validates each returned tool catalog.
    pub(crate) async fn connect(configs: Vec<StdioServerConfig>) -> Result<Option<Self>, String> {
        if configs.len() > MAX_MCP_SERVERS_PER_REQUEST {
            return Err("The configured MCP server count exceeds the supported limit.".to_owned());
        }

        let mut clients = HashMap::new();
        let mut definitions = Vec::new();
        let mut routes = HashMap::new();
        for config in configs.into_iter().filter(|config| config.enabled) {
            if !valid_server_id(&config.id) || clients.contains_key(&config.id) {
                return Err(
                    "The configured MCP server identifiers are invalid or duplicated.".to_owned(),
                );
            }
            let client = McpClient::connect(&config).await?;
            let tools = client.list_tools().await?;
            let (server_definitions, server_routes) = tool_definitions(&config.id, tools)?;
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
            clients.insert(config.id, client);
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

/// Converts validated MCP tool metadata into provider function definitions.
pub(crate) fn tool_definitions(
    server_id: &str,
    tools: Vec<Tool>,
) -> Result<(Vec<ToolDefinition>, HashMap<String, McpToolRoute>), String> {
    if !valid_server_id(server_id) || tools.len() > MAX_MCP_TOOLS_PER_SERVER {
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
        let name = format!("mcp__{server_id}__{}", tool.name);
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
        if routes
            .insert(
                name.clone(),
                McpToolRoute {
                    server_id: server_id.to_owned(),
                    tool_name: tool.name.into_owned(),
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
    if let Some(server_id) = value.strip_suffix("__*") {
        return valid_server_id(server_id);
    }
    value.match_indices("__").any(|(separator, _)| {
        valid_server_id(&value[..separator]) && valid_tool_name(&value[separator + 2..])
    })
}

pub(crate) struct McpClient {
    service: RunningService<RoleClient, ()>,
}

impl McpClient {
    pub(crate) async fn connect(config: &StdioServerConfig) -> Result<Self, String> {
        let service = if let Some(endpoint) = &config.endpoint {
            let client =
                crate::tools::web_search::url_fetch::pinned_public_http_client(endpoint, None)
                    .await
                    .map_err(|_| {
                        "The remote MCP endpoint could not be reached safely.".to_owned()
                    })?;
            let auth_token = if let Some(name) = &config.auth_environment_variable {
                let reference = crate::credentials::McpCredentialReference::new(
                    config.project_id.clone(),
                    config.id.clone(),
                    name.clone(),
                )
                .map_err(|_| "The remote MCP credential name is invalid.".to_owned())?;
                let secret = crate::credentials::CredentialStore
                    .load_mcp_secret(&reference)
                    .map_err(|_| "The remote MCP credential is unavailable.".to_owned())?;
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
                    let secret = credentials.load_mcp_secret(&reference).map_err(|_| {
                        format!(
                            "The MCP credential for environment variable {name} is unavailable."
                        )
                    })?;
                    Ok((name.clone(), secret))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let transport = SandboxedStdioTransport::spawn(config, &environment)?;
            timeout(MCP_STARTUP_TIMEOUT, ().serve(transport))
                .await
                .map_err(|_| {
                    "The MCP server did not initialize before the startup timeout.".to_owned()
                })?
                .map_err(|_| {
                    "The MCP server could not complete protocol initialization.".to_owned()
                })?
        };
        Ok(Self { service })
    }

    pub(crate) async fn list_tools(&self) -> Result<Vec<Tool>, String> {
        timeout(MCP_STARTUP_TIMEOUT, self.service.peer().list_all_tools())
            .await
            .map_err(|_| "The MCP server did not return its tools before the timeout.".to_owned())?
            .map_err(|_| "The MCP server returned an invalid tool list.".to_owned())
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
        .map_err(|_| "The MCP server returned a tool-call error.".to_owned())?;
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
        .map_err(|_| "The remote MCP server could not complete protocol initialization.".to_owned())
}

struct SandboxedStdioTransport {
    transport: AsyncRwTransport<RoleClient, BoundedLineReader<File>, File>,
    process: SandboxedProcess,
    frame_limit_exceeded: Arc<AtomicBool>,
}

impl SandboxedStdioTransport {
    fn spawn(
        config: &StdioServerConfig,
        environment: &[(String, zeroize::Zeroizing<String>)],
    ) -> Result<Self, String> {
        if let Some(reference) = &config.package {
            crate::connector_packages::verify_installed_package(&config.program, reference)
                .map_err(|_| "The installed MCP package failed its integrity check.".to_owned())?;
        }
        let package_read_paths = config.package_directory.iter().cloned().collect::<Vec<_>>();
        let mut process = SandboxedProcess::spawn_program_with_options(
            &config.program,
            &config.arguments,
            &config.working_directory,
            environment,
            SandboxOptions {
                additional_read_only_paths: &package_read_paths,
                job_limits: Some(SandboxJobLimits {
                    active_process_limit: MCP_ACTIVE_PROCESS_LIMIT,
                    process_memory_limit_bytes: MCP_PROCESS_MEMORY_LIMIT_BYTES,
                    job_memory_limit_bytes: MCP_JOB_MEMORY_LIMIT_BYTES,
                }),
                ..SandboxOptions::default()
            },
        )
        .map_err(|_| {
            "The MCP server could not start inside the Windows AppContainer.".to_owned()
        })?;
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

impl Transport<RoleClient> for SandboxedStdioTransport {
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

#[cfg(test)]
mod tests {
    use super::{
        BoundedLineReader, MAX_MCP_ARGUMENTS_PER_SERVER, MAX_MCP_FRAME_BYTES, McpRegistry,
        connect_with_approval, load_server_configs, parse_server_config, parse_server_configs,
        save_server_catalog, server_catalog, start_streamable_http_service, tool_definitions,
        valid_permission_rule_suffix,
    };
    use crate::{permissions::ToolPermissionBroker, protocol::EventSink};
    use rmcp::model::Tool;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::{
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
        sync::watch,
    };

    fn tool(name: &str) -> Tool {
        serde_json::from_value(serde_json::json!({
            "name": name,
            "description": "Read a value from the configured server.",
            "inputSchema": {
                "type": "object",
                "properties": {"value": {"type": "string"}},
                "required": ["value"]
            }
        }))
        .expect("valid MCP tool fixture")
    }

    #[tokio::test]
    async fn bounded_stdio_reader_accepts_a_frame_at_the_limit() {
        let (mut writer, reader) = tokio::io::duplex(MAX_MCP_FRAME_BYTES + 1);
        let mut frame = vec![b'a'; MAX_MCP_FRAME_BYTES];
        frame.push(b'\n');
        writer.write_all(&frame).await.expect("write frame");
        drop(writer);

        let exceeded = Arc::new(AtomicBool::new(false));
        let mut reader = BufReader::new(BoundedLineReader::new(reader, Arc::clone(&exceeded)));
        let mut received = Vec::new();
        let count = reader
            .read_until(b'\n', &mut received)
            .await
            .expect("read bounded frame");

        assert_eq!(count, MAX_MCP_FRAME_BYTES + 1);
        assert!(!exceeded.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn bounded_stdio_reader_rejects_a_frame_over_the_limit() {
        let (mut writer, reader) = tokio::io::duplex(MAX_MCP_FRAME_BYTES + 2);
        let mut frame = vec![b'a'; MAX_MCP_FRAME_BYTES + 1];
        frame.push(b'\n');
        writer
            .write_all(&frame)
            .await
            .expect("write oversized frame");
        drop(writer);

        let exceeded = Arc::new(AtomicBool::new(false));
        let mut reader = BufReader::new(BoundedLineReader::new(reader, Arc::clone(&exceeded)));
        let mut received = Vec::new();
        let error = reader
            .read_until(b'\n', &mut received)
            .await
            .expect_err("reject oversized frame");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(exceeded.load(Ordering::Acquire));
    }

    #[test]
    fn creates_namespaced_provider_definitions_and_routes() {
        let (definitions, routes) =
            tool_definitions("workspace", vec![tool("read-value")]).expect("valid catalog");

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "mcp__workspace__read-value");
        assert_eq!(
            definitions[0].description,
            "Read a value from the configured server."
        );
        assert_eq!(definitions[0].parameters["required"][0], "value");
        assert_eq!(routes["mcp__workspace__read-value"].server_id, "workspace");
        assert_eq!(routes["mcp__workspace__read-value"].tool_name, "read-value");
    }

    #[test]
    fn rejects_unsafe_names_duplicate_names_and_oversized_tool_catalogs() {
        assert!(tool_definitions("Uppercase", vec![tool("read")]).is_err());
        assert!(tool_definitions("workspace", vec![tool("read/value")]).is_err());
        assert!(tool_definitions("workspace", vec![tool("read"), tool("read")]).is_err());
        assert!(
            tool_definitions(
                "workspace",
                (0..=super::MAX_MCP_TOOLS_PER_SERVER)
                    .map(|index| tool(&format!("tool_{index}")))
                    .collect()
            )
            .is_err()
        );
    }

    #[test]
    fn validates_enabled_and_disabled_stdio_server_settings() {
        let working_directory = Path::new(r"C:\workspace");
        let settings = serde_json::json!([
            {
                "id": "local_docs",
                "enabled": true,
                "program": r"C:\tools\mcp-server.exe",
                "arguments": ["--mode", "read-only"],
                "environmentVariables": ["API_TOKEN", "_PRIVATE_KEY"]
            },
            {
                "id": "disabled_server",
                "enabled": false,
                "program": r"C:\tools\other-server.exe"
            }
        ]);

        let parsed = parse_server_configs(Some(&settings), working_directory)
            .expect("valid server settings");
        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].enabled);
        assert_eq!(parsed[0].arguments.len(), 2);
        assert_eq!(
            parsed[0].environment_variables,
            ["API_TOKEN", "_PRIVATE_KEY"]
        );
        assert_eq!(parsed[0].project_id, "test-project");
        assert!(!parsed[1].enabled);
        assert_eq!(parsed[0].working_directory, working_directory);
        assert!(
            parse_server_configs(None, working_directory)
                .expect("missing settings disable MCP")
                .is_empty()
        );
        assert!(
            parse_server_configs(
                Some(&serde_json::json!([{
                    "id": "invalid_environment",
                    "enabled": true,
                    "program": r"C:\tools\mcp-server.exe",
                    "environmentVariables": ["PATH"]
                }])),
                working_directory,
            )
            .is_err()
        );
        assert!(
            parse_server_configs(
                Some(&serde_json::json!([{
                    "id": "duplicate_environment",
                    "enabled": true,
                    "program": r"C:\tools\mcp-server.exe",
                    "environmentVariables": ["API_TOKEN", "API_TOKEN"]
                }])),
                working_directory,
            )
            .is_err()
        );
    }

    #[test]
    fn validates_remote_streamable_http_endpoints_and_secure_credential_names() {
        let root = Path::new(r"C:\workspace");
        let config = parse_server_configs(
            Some(&serde_json::json!([{
                "id": "remote_docs",
                "enabled": true,
                "transport": "streamableHttp",
                "endpoint": "https://mcp.example.org/v1",
                "authEnvironmentVariable": "MCP_TOKEN"
            }])),
            root,
        )
        .expect("valid remote MCP config");
        assert_eq!(
            config[0].endpoint.as_ref().map(url::Url::as_str),
            Some("https://mcp.example.org/v1")
        );
        assert_eq!(
            config[0].auth_environment_variable.as_deref(),
            Some("MCP_TOKEN")
        );

        for endpoint in [
            "http://mcp.example.org/v1",
            "https://127.0.0.1/mcp",
            "https://mcp.example.org/mcp?token=private",
            "https://user:password@mcp.example.org/mcp",
        ] {
            assert!(
                parse_server_configs(
                    Some(&serde_json::json!([{
                        "id": "remote_docs",
                        "enabled": true,
                        "transport": "streamableHttp",
                        "endpoint": endpoint
                    }])),
                    root,
                )
                .is_err(),
                "accepted unsafe endpoint {endpoint}"
            );
        }
        assert!(
            parse_server_configs(
                Some(&serde_json::json!([{
                    "id": "remote_docs",
                    "enabled": true,
                    "transport": "streamableHttp",
                    "endpoint": "https://mcp.example.org/mcp",
                    "authEnvironmentVariable": "PATH"
                }])),
                root,
            )
            .is_err()
        );
    }

    #[test]
    fn validates_per_tool_permission_namespaces() {
        assert!(valid_permission_rule_suffix("local_docs__*"));
        assert!(valid_permission_rule_suffix("local__docs__read-file"));
        assert!(!valid_permission_rule_suffix("local_docs__read/file"));
        assert!(!valid_permission_rule_suffix("Invalid__read"));
        assert!(!valid_permission_rule_suffix("local_docs__"));
    }

    #[test]
    fn registry_lists_only_the_selected_server_tool_names() {
        let (first_definitions, first_routes) =
            tool_definitions("local_docs", vec![tool("read-file"), tool("search")])
                .expect("valid local tool catalog");
        let (second_definitions, second_routes) =
            tool_definitions("other_docs", vec![tool("open")]).expect("valid other tool catalog");
        let mut definitions = first_definitions;
        definitions.extend(second_definitions);
        let mut routes = first_routes;
        routes.extend(second_routes);
        let registry = McpRegistry {
            clients: HashMap::new(),
            definitions,
            routes,
        };

        assert_eq!(
            registry.tool_names_for_server("local_docs"),
            vec!["read-file", "search"]
        );
        assert_eq!(registry.tool_names_for_server("other_docs"), vec!["open"]);
    }

    #[test]
    fn denied_servers_are_not_selected_for_chat_startup() {
        let config = parse_server_configs(
            Some(&serde_json::json!([{
                "id": "local_docs",
                "enabled": true,
                "program": r"C:\tools\mcp-server.exe"
            }])),
            Path::new(r"C:\workspace"),
        )
        .expect("valid MCP server config");
        let rules = HashMap::from([(
            "mcp__local_docs__*".to_owned(),
            super::super::ToolPermissionRule::Deny,
        )]);
        assert!(super::filter_denied_servers(&config, &rules).is_empty());
    }

    #[test]
    fn validates_unsaved_connection_check_settings_against_project_root() {
        let project =
            std::env::temp_dir().join(format!("openchat-mcp-check-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&project).expect("create temporary project");
        let program = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .into_owned();
        let config = parse_server_config(
            &project,
            "project-id",
            &serde_json::json!({
                "id": "unsaved_server",
                "enabled": true,
                "program": program.clone(),
                "arguments": ["--probe"]
            }),
        )
        .expect("validate unsaved server config");
        assert_eq!(config.id, "unsaved_server");
        assert!(config.enabled);
        assert_eq!(config.arguments, ["--probe"]);
        assert_eq!(
            config.working_directory,
            std::fs::canonicalize(&project).expect("canonicalize project")
        );
        assert!(
            parse_server_config(
                &project,
                "project-id",
                &serde_json::json!({
                    "id": "invalid_server",
                    "enabled": true,
                    "program": "relative.exe"
                })
            )
            .is_err()
        );
        std::fs::remove_dir_all(project).expect("remove temporary project");
    }

    #[tokio::test]
    async fn disabled_servers_are_not_started() {
        let configs = parse_server_configs(
            Some(&serde_json::json!([{
                "id": "disabled_server",
                "enabled": false,
                "program": r"C:\missing\server.exe"
            }])),
            Path::new(r"C:\workspace"),
        )
        .expect("valid disabled server settings");

        let registry = super::McpRegistry::connect(configs)
            .await
            .expect("disabled servers should not be started");
        assert!(registry.is_none());
    }

    #[tokio::test]
    async fn ask_requests_approval_before_start_and_denial_skips_server() {
        let configs = parse_server_configs(
            Some(&json!([{
                "id": "local_docs",
                "enabled": true,
                "program": r"C:\tools\mcp-server.exe"
            }])),
            Path::new(r"C:\workspace"),
        )
        .expect("valid MCP server config");
        let broker = ToolPermissionBroker::default();
        let rules = HashMap::new();
        let (events, mut received_events) = EventSink::test_channel();
        let request_id = json!(42);
        let (cancellation_sender, cancellation) = watch::channel(false);
        let task_broker = broker.clone();
        let task = tokio::spawn(async move {
            let mut cancellation = cancellation;
            connect_with_approval(
                configs,
                &rules,
                &task_broker,
                &request_id,
                &events,
                &mut cancellation,
            )
            .await
        });

        let event = received_events
            .recv()
            .await
            .expect("server startup requests approval before connecting");
        assert_eq!(
            event.event.as_deref(),
            Some("chat.tool.permission.requested")
        );
        let data = event.data.expect("permission event includes its request");
        assert_eq!(data["toolName"], "mcp_server_start");
        assert_eq!(
            data["targetPath"],
            format!("local_docs · {}", r"C:\tools\mcp-server.exe")
        );
        assert_eq!(data["arguments"]["transport"], "stdio");

        let approval_id = data["approvalRequestId"]
            .as_str()
            .expect("permission event includes an approval id");
        broker
            .respond(approval_id, false)
            .await
            .expect("denial is delivered to the pending request");
        assert!(
            task.await
                .expect("startup approval task does not panic")
                .expect("denial is not a service error")
                .is_none()
        );
        assert!(!*cancellation_sender.borrow());
    }

    #[tokio::test]
    async fn streamable_http_client_negotiates_lists_tools_and_sends_bearer_auth() {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind local MCP fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move {
            let mut methods = Vec::new();
            let mut auth_headers = Vec::new();
            for _ in 0..3 {
                let (stream, _) = listener.accept().await.expect("accept HTTP request");
                let mut reader = BufReader::new(stream);
                let mut request_line = String::new();
                reader
                    .read_line(&mut request_line)
                    .await
                    .expect("read request line");
                let mut content_length = 0;
                let mut authorization = None;
                loop {
                    let mut header = String::new();
                    reader
                        .read_line(&mut header)
                        .await
                        .expect("read HTTP header");
                    if header == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':') {
                        if name.eq_ignore_ascii_case("content-length") {
                            content_length = value.trim().parse::<usize>().expect("valid length");
                        }
                        if name.eq_ignore_ascii_case("authorization") {
                            authorization = Some(value.trim().to_owned());
                        }
                    }
                }
                let mut body = vec![0; content_length];
                reader
                    .read_exact(&mut body)
                    .await
                    .expect("read JSON-RPC body");
                let request: Value = serde_json::from_slice(&body).expect("valid JSON-RPC body");
                let method = request["method"].as_str().expect("request method");
                methods.push(method.to_owned());
                auth_headers.push(authorization);
                let response = match method {
                    "initialize" => Some(json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": {
                            "protocolVersion": "2025-06-18",
                            "capabilities": {"tools": {}},
                            "serverInfo": {"name": "fixture", "version": "1"}
                        }
                    })),
                    "tools/list" => Some(json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": {
                            "tools": [{
                                "name": "lookup",
                                "description": "Read one fixture value.",
                                "inputSchema": {"type": "object", "properties": {}}
                            }]
                        }
                    })),
                    "notifications/initialized" => None,
                    other => panic!("unexpected MCP request {other}"),
                };
                let body = response.map(|value| value.to_string()).unwrap_or_default();
                let status = if body.is_empty() {
                    "202 Accepted"
                } else {
                    "200 OK"
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                reader
                    .get_mut()
                    .write_all(response.as_bytes())
                    .await
                    .expect("write HTTP response");
            }
            (methods, auth_headers)
        });

        let endpoint =
            url::Url::parse(&format!("http://{address}/mcp")).expect("valid local fixture URL");
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("build fixture HTTP client");
        let service = start_streamable_http_service(
            &endpoint,
            client,
            Some(zeroize::Zeroizing::new("fixture-token".to_owned())),
        )
        .await
        .expect("initialize streamable HTTP MCP connection");
        let tools = tokio::time::timeout(Duration::from_secs(5), service.peer().list_all_tools())
            .await
            .expect("list tools before the timeout")
            .expect("valid tools response");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "lookup");

        let (methods, auth_headers) = server.await.expect("fixture server completes");
        assert!(methods.contains(&"initialize".to_owned()));
        assert!(methods.contains(&"notifications/initialized".to_owned()));
        assert!(methods.contains(&"tools/list".to_owned()));
        assert!(
            auth_headers
                .iter()
                .all(|value| { value.as_deref() == Some("Bearer fixture-token") })
        );
    }

    #[test]
    fn rejects_relative_programs_duplicate_ids_and_excessive_arguments() {
        let working_directory = Path::new(r"C:\workspace");
        let relative_program = serde_json::json!([{
            "id": "local_docs",
            "enabled": true,
            "program": "mcp-server.exe"
        }]);
        assert!(parse_server_configs(Some(&relative_program), working_directory).is_err());

        let duplicate_ids = serde_json::json!([
            {"id": "same", "enabled": true, "program": r"C:\one.exe"},
            {"id": "same", "enabled": false, "program": r"C:\two.exe"}
        ]);
        assert!(parse_server_configs(Some(&duplicate_ids), working_directory).is_err());

        let excessive_arguments = serde_json::json!([{
            "id": "local_docs",
            "enabled": true,
            "program": r"C:\tools\mcp-server.exe",
            "arguments": (0..=MAX_MCP_ARGUMENTS_PER_SERVER)
                .map(|index| format!("arg_{index}"))
                .collect::<Vec<_>>()
        }]);
        assert!(parse_server_configs(Some(&excessive_arguments), working_directory).is_err());
    }

    #[test]
    fn loads_only_regular_project_catalogs_and_treats_missing_catalog_as_empty() {
        struct ProjectDirectory(PathBuf);

        impl Drop for ProjectDirectory {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).expect("remove temporary project");
            }
        }

        let project = ProjectDirectory(
            std::env::temp_dir().join(format!("openchat-mcp-test-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir(&project.0).expect("create temporary project");
        assert!(
            load_server_configs(&project.0, "project-id")
                .expect("missing catalog disables MCP")
                .is_empty()
        );

        let catalog_directory = project.0.join(".openchat");
        std::fs::create_dir_all(&catalog_directory).expect("create catalog directory");
        std::fs::write(
            catalog_directory.join("mcp.json"),
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "servers": [{
                    "id": "workspace_docs",
                    "enabled": false,
                    "program": r"C:\tools\mcp-server.exe"
                }]
            }))
            .expect("serialize catalog"),
        )
        .expect("write catalog");

        let configs = load_server_configs(&project.0, "project-id").expect("load project catalog");
        assert_eq!(configs.len(), 1);
        assert!(!configs[0].enabled);
        assert_eq!(
            configs[0].working_directory,
            std::fs::canonicalize(&project.0).expect("canonicalize temporary project")
        );

        std::fs::write(catalog_directory.join("mcp.json"), b"{invalid")
            .expect("replace catalog with invalid data");
        assert!(load_server_configs(&project.0, "project-id").is_err());
    }

    #[test]
    fn project_catalog_can_be_read_and_atomically_replaced_after_validation() {
        struct ProjectDirectory(PathBuf);

        impl Drop for ProjectDirectory {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).expect("remove temporary project");
            }
        }

        let project = ProjectDirectory(
            std::env::temp_dir().join(format!("openchat-mcp-save-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir(&project.0).expect("create temporary project");
        assert_eq!(
            server_catalog(&project.0).expect("read empty catalog"),
            serde_json::json!({"version": 1, "servers": []})
        );
        let program = std::env::current_exe()
            .expect("current test executable")
            .to_string_lossy()
            .into_owned();
        let catalog = serde_json::json!({
            "version": 1,
            "servers": [{
                "id": "local_docs",
                "enabled": false,
                "transport": "stdio",
                "program": program.clone(),
                "arguments": ["--read-only"],
                "environmentVariables": []
            }]
        });
        save_server_catalog(&project.0, &catalog).expect("save valid catalog");
        assert_eq!(
            server_catalog(&project.0).expect("read saved catalog"),
            catalog
        );
        let updated_catalog = serde_json::json!({
            "version": 1,
            "servers": [{
                "id": "local_docs",
                "enabled": true,
                "transport": "stdio",
                "program": program.clone(),
                "arguments": ["--updated"],
                "environmentVariables": []
            }]
        });
        save_server_catalog(&project.0, &updated_catalog).expect("replace valid catalog");
        assert_eq!(
            server_catalog(&project.0).expect("read replaced catalog"),
            updated_catalog
        );

        let path = project.0.join(".openchat/mcp.json");
        let original = std::fs::read(&path).expect("read saved file");
        let invalid = serde_json::json!({
            "version": 1,
            "servers": [{"id": "invalid id", "enabled": true, "program": program}]
        });
        assert!(save_server_catalog(&project.0, &invalid).is_err());
        assert_eq!(
            std::fs::read(path).expect("read preserved catalog"),
            original
        );
    }

    #[test]
    fn save_rejects_an_external_symbolic_link_as_the_catalog_directory() {
        use std::os::windows::fs::symlink_dir;

        struct ProjectDirectories(PathBuf, PathBuf);

        impl Drop for ProjectDirectories {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).expect("remove temporary project");
                std::fs::remove_dir_all(&self.1).expect("remove temporary outside directory");
            }
        }

        let suffix = uuid::Uuid::new_v4();
        let project = std::env::temp_dir().join(format!("openchat-mcp-project-{suffix}"));
        let outside = std::env::temp_dir().join(format!("openchat-mcp-outside-{suffix}"));
        let directories = ProjectDirectories(project, outside);
        std::fs::create_dir(&directories.0).expect("create temporary project");
        std::fs::create_dir(&directories.1).expect("create temporary outside directory");
        symlink_dir(
            &directories.1,
            directories.0.join(super::MCP_CATALOG_DIRECTORY),
        )
        .expect("redirect project configuration directory");

        let result = save_server_catalog(
            &directories.0,
            &serde_json::json!({"version": 1, "servers": []}),
        );

        assert!(result.is_err());
        assert!(!directories.1.join(super::MCP_CATALOG_FILE).exists());
    }
}
