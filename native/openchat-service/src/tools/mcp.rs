use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
    fs::OpenOptions,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, CallToolResult, Tool},
    service::RunningService,
    transport::{Transport, async_rw::AsyncRwTransport},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::{fs::File, io::AsyncReadExt, time::timeout};

use crate::provider_schema::ToolDefinition;

use super::terminal::windows_sandbox::SandboxedProcess;

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
const MAX_PROVIDER_TOOL_NAME_BYTES: usize = 64;
const MAX_MCP_CALL_TIMEOUT: Duration = Duration::from_secs(120);
const MCP_CATALOG_DIRECTORY: &str = ".openchat";
const MCP_CATALOG_FILE: &str = "mcp.json";
const MAX_MCP_CATALOG_BYTES: u64 = 64 * 1024;

#[derive(Clone)]
pub(crate) struct StdioServerConfig {
    pub(crate) id: String,
    pub(crate) enabled: bool,
    pub(crate) program: PathBuf,
    pub(crate) arguments: Vec<OsString>,
    pub(crate) working_directory: PathBuf,
}

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

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StdioServerSettings {
    id: String,
    enabled: bool,
    program: String,
    #[serde(default)]
    arguments: Vec<String>,
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
    parse_server_settings(settings, working_directory)
}

fn parse_server_settings(
    settings: Vec<StdioServerSettings>,
    working_directory: &Path,
) -> Result<Vec<StdioServerConfig>, crate::protocol::ServiceError> {
    if settings.len() > MAX_MCP_SERVERS_PER_REQUEST {
        return Err(invalid_mcp_servers());
    }

    let mut seen_ids = HashSet::with_capacity(settings.len());
    let mut servers = Vec::with_capacity(settings.len());
    for setting in settings {
        if !valid_server_id(&setting.id)
            || !seen_ids.insert(setting.id.clone())
            || setting.program.is_empty()
            || setting.program.len() > 4096
            || setting.program.contains('\0')
            || !Path::new(&setting.program).is_absolute()
            || setting.arguments.len() > MAX_MCP_ARGUMENTS_PER_SERVER
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
        servers.push(StdioServerConfig {
            id: setting.id,
            enabled: setting.enabled,
            program: PathBuf::from(setting.program),
            arguments: setting.arguments.into_iter().map(OsString::from).collect(),
            working_directory: working_directory.to_path_buf(),
        });
    }
    Ok(servers)
}

pub(crate) fn load_server_configs(
    project_root: &Path,
) -> Result<Vec<StdioServerConfig>, crate::protocol::ServiceError> {
    let (root, catalog) = read_server_catalog(project_root)?;
    parse_server_settings(catalog.servers, &root)
}

pub(crate) fn parse_server_config(
    project_root: &Path,
    value: &Value,
) -> Result<StdioServerConfig, crate::protocol::ServiceError> {
    let root = fs::canonicalize(project_root).map_err(|_| invalid_mcp_servers())?;
    let setting: StdioServerSettings =
        serde_json::from_value(value.clone()).map_err(|_| invalid_mcp_servers())?;
    parse_server_settings(vec![setting], &root)?
        .into_iter()
        .next()
        .ok_or_else(invalid_mcp_servers)
}

pub(crate) fn server_catalog(project_root: &Path) -> Result<Value, crate::protocol::ServiceError> {
    let (_, catalog) = read_server_catalog(project_root)?;
    serde_json::to_value(catalog).map_err(|_| invalid_mcp_servers())
}

pub(crate) fn save_server_catalog(
    project_root: &Path,
    value: &Value,
) -> Result<Value, crate::protocol::ServiceError> {
    let root = fs::canonicalize(project_root).map_err(|_| invalid_mcp_servers())?;
    let catalog: StdioServerCatalog =
        serde_json::from_value(value.clone()).map_err(|_| invalid_mcp_servers())?;
    if catalog.version != 1 {
        return Err(invalid_mcp_servers());
    }
    parse_server_settings(catalog.servers.clone(), &root)?;

    let directory = root.join(MCP_CATALOG_DIRECTORY);
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(invalid_mcp_servers());
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&directory).map_err(|_| invalid_mcp_servers())?;
        }
        Err(_) => return Err(invalid_mcp_servers()),
    }
    let canonical_directory = fs::canonicalize(&directory).map_err(|_| invalid_mcp_servers())?;
    if !canonical_directory.starts_with(&root) {
        return Err(invalid_mcp_servers());
    }
    let catalog_path = canonical_directory.join(MCP_CATALOG_FILE);
    match fs::symlink_metadata(&catalog_path) {
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() > MAX_MCP_CATALOG_BYTES =>
        {
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
    let temporary = canonical_directory.join(format!(".mcp-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| invalid_mcp_servers())?;
        file.write_all(&contents)
            .map_err(|_| invalid_mcp_servers())?;
        file.sync_all().map_err(|_| invalid_mcp_servers())?;
        replace_catalog_file(&temporary, &catalog_path)?;
        Ok::<(), crate::protocol::ServiceError>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    server_catalog(&root)
}

#[cfg(windows)]
fn replace_catalog_file(
    source: &Path,
    destination: &Path,
) -> Result<(), crate::protocol::ServiceError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replaced = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        return Err(invalid_mcp_servers());
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_catalog_file(
    source: &Path,
    destination: &Path,
) -> Result<(), crate::protocol::ServiceError> {
    fs::rename(source, destination).map_err(|_| invalid_mcp_servers())
}

fn read_server_catalog(
    project_root: &Path,
) -> Result<(PathBuf, StdioServerCatalog), crate::protocol::ServiceError> {
    let root = fs::canonicalize(project_root).map_err(|_| invalid_mcp_servers())?;
    let directory = root.join(MCP_CATALOG_DIRECTORY);
    let directory_metadata = match fs::symlink_metadata(&directory) {
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
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Err(invalid_mcp_servers());
    }
    let canonical_directory = fs::canonicalize(&directory).map_err(|_| invalid_mcp_servers())?;
    if !canonical_directory.starts_with(&root) {
        return Err(invalid_mcp_servers());
    }
    let catalog_path = canonical_directory.join(MCP_CATALOG_FILE);
    let metadata = match fs::symlink_metadata(&catalog_path) {
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
    let canonical_catalog = fs::canonicalize(&catalog_path).map_err(|_| invalid_mcp_servers())?;
    if !canonical_catalog.starts_with(&root) {
        return Err(invalid_mcp_servers());
    }
    let mut source = Vec::with_capacity(MAX_MCP_CATALOG_BYTES as usize);
    fs::File::open(canonical_catalog)
        .map_err(|_| invalid_mcp_servers())?
        .take(MAX_MCP_CATALOG_BYTES + 1)
        .read_to_end(&mut source)
        .map_err(|_| invalid_mcp_servers())?;
    if source.len() as u64 > MAX_MCP_CATALOG_BYTES {
        return Err(invalid_mcp_servers());
    }
    let catalog: StdioServerCatalog =
        serde_json::from_slice(&source).map_err(|_| invalid_mcp_servers())?;
    if catalog.version != 1 {
        return Err(invalid_mcp_servers());
    }
    parse_server_settings(catalog.servers.clone(), &root)?;
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

pub(crate) struct McpRegistry {
    clients: HashMap<String, StdioClient>,
    definitions: Vec<ToolDefinition>,
    routes: HashMap<String, McpToolRoute>,
}

impl McpRegistry {
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
            let client = StdioClient::connect(&config).await?;
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

    pub(crate) fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    pub(crate) fn server_id_for_tool(&self, provider_tool_name: &str) -> Option<&str> {
        self.routes
            .get(provider_tool_name)
            .map(|route| route.server_id.as_str())
    }

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

pub(crate) fn valid_server_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 24
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

pub(crate) fn valid_tool_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(crate) struct StdioClient {
    service: RunningService<RoleClient, ()>,
}

impl StdioClient {
    pub(crate) async fn connect(config: &StdioServerConfig) -> Result<Self, String> {
        let transport = SandboxedStdioTransport::spawn(config)?;
        let service = timeout(MCP_STARTUP_TIMEOUT, ().serve(transport))
            .await
            .map_err(|_| {
                "The MCP server did not initialize before the startup timeout.".to_owned()
            })?
            .map_err(|_| "The MCP server could not complete protocol initialization.".to_owned())?;
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

struct SandboxedStdioTransport {
    transport: AsyncRwTransport<RoleClient, File, File>,
    process: SandboxedProcess,
}

impl SandboxedStdioTransport {
    fn spawn(config: &StdioServerConfig) -> Result<Self, String> {
        let mut process = SandboxedProcess::spawn_program(
            &config.program,
            &config.arguments,
            &config.working_directory,
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
        if let Some(mut stderr) = process.take_stderr() {
            tokio::spawn(async move {
                let mut buffer = [0_u8; 4096];
                while matches!(stderr.read(&mut buffer).await, Ok(count) if count > 0) {}
            });
        }
        Ok(Self {
            transport: AsyncRwTransport::new_client(stdout, stdin),
            process,
        })
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

    fn receive(
        &mut self,
    ) -> impl std::future::Future<Output = Option<rmcp::service::RxJsonRpcMessage<RoleClient>>> + Send
    {
        self.transport.receive()
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
        MAX_MCP_ARGUMENTS_PER_SERVER, load_server_configs, parse_server_config,
        parse_server_configs, save_server_catalog, server_catalog, tool_definitions,
    };
    use rmcp::model::Tool;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

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
                "arguments": ["--mode", "read-only"]
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
        assert!(!parsed[1].enabled);
        assert_eq!(parsed[0].working_directory, working_directory);
        assert!(
            parse_server_configs(None, working_directory)
                .expect("missing settings disable MCP")
                .is_empty()
        );
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
            load_server_configs(&project.0)
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

        let configs = load_server_configs(&project.0).expect("load project catalog");
        assert_eq!(configs.len(), 1);
        assert!(!configs[0].enabled);
        assert_eq!(
            configs[0].working_directory,
            std::fs::canonicalize(&project.0).expect("canonicalize temporary project")
        );

        std::fs::write(catalog_directory.join("mcp.json"), b"{invalid")
            .expect("replace catalog with invalid data");
        assert!(load_server_configs(&project.0).is_err());
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
                "program": program.clone(),
                "arguments": ["--read-only"]
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
                "program": program.clone(),
                "arguments": ["--updated"]
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
}
