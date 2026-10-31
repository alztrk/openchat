use super::{
    BoundedLineReader, MAX_MCP_ARGUMENTS_PER_SERVER, MAX_MCP_FRAME_BYTES, McpApprovalContext,
    McpRegistry, StdioServerSettings, connect_with_approval, load_server_configs,
    parse_server_config, parse_server_configs, save_server_catalog, server_catalog,
    server_permission_fingerprint, start_streamable_http_service, tool_definitions,
    valid_bound_permission_rule_suffix, valid_permission_rule_suffix,
};
use crate::{permissions::ToolPermissionBroker, protocol::EventSink, storage::AppStorage};
use rmcp::model::Tool;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::watch,
};

struct TestDirectory(PathBuf);

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct TestStorage {
    storage: Arc<AppStorage>,
    _directory: TestDirectory,
}

impl TestStorage {
    fn new() -> Self {
        let directory = TestDirectory(std::env::temp_dir().join(format!(
            "openchat-mcp-storage-test-{}",
            uuid::Uuid::new_v4()
        )));
        let storage = AppStorage::open_at(directory.0.clone()).expect("open isolated test storage");
        Self {
            storage: Arc::new(storage),
            _directory: directory,
        }
    }
}

fn tool(name: &str) -> Tool {
    tool_with_description(name, "Read a value from the configured server.")
}

fn tool_with_description(name: &str, description: &str) -> Tool {
    serde_json::from_value(serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": {"value": {"type": "string"}},
            "required": ["value"]
        }
    }))
    .expect("valid MCP tool fixture")
}

const TEST_SERVER_FINGERPRINT: &str = "0123456789abcdef0123456789abcdef";

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
    let (definitions, routes) = tool_definitions(
        "workspace",
        TEST_SERVER_FINGERPRINT,
        vec![tool("read-value")],
    )
    .expect("valid catalog");

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
    assert!(tool_definitions("Uppercase", TEST_SERVER_FINGERPRINT, vec![tool("read")]).is_err());
    assert!(
        tool_definitions(
            "workspace",
            TEST_SERVER_FINGERPRINT,
            vec![tool("read/value")]
        )
        .is_err()
    );
    assert!(
        tool_definitions(
            "workspace",
            TEST_SERVER_FINGERPRINT,
            vec![tool("read"), tool("read")]
        )
        .is_err()
    );
    assert!(
        tool_definitions(
            "workspace",
            TEST_SERVER_FINGERPRINT,
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

    let parsed =
        parse_server_configs(Some(&settings), working_directory).expect("valid server settings");
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
    assert!(valid_bound_permission_rule_suffix(
        "local_docs__*__0123456789abcdef0123456789abcdef"
    ));
    assert!(valid_bound_permission_rule_suffix(
        "local_docs__read-file__0123456789abcdef0123456789abcdef"
    ));
    assert!(!valid_permission_rule_suffix("local_docs__read/file"));
    assert!(!valid_permission_rule_suffix("Invalid__read"));
    assert!(!valid_permission_rule_suffix("local_docs__"));
}

#[test]
fn permission_fingerprints_change_with_server_or_tool_definitions() {
    let root = Path::new(r"C:\workspace");
    let first_server = parse_server_configs(
        Some(&json!([{
            "id": "workspace",
            "enabled": true,
            "program": r"C:\tools\server.exe",
            "arguments": ["--read-only"]
        }])),
        root,
    )
    .expect("valid first server definition");
    let changed_server = parse_server_configs(
        Some(&json!([{
            "id": "workspace",
            "enabled": true,
            "program": r"C:\tools\other-server.exe",
            "arguments": ["--read-only"]
        }])),
        root,
    )
    .expect("valid changed server definition");
    assert_ne!(
        first_server[0].permission_fingerprint,
        changed_server[0].permission_fingerprint
    );

    let (_, first_routes) = tool_definitions(
        "workspace",
        TEST_SERVER_FINGERPRINT,
        vec![tool("read-value")],
    )
    .expect("valid first tool definition");
    let (_, changed_routes) = tool_definitions(
        "workspace",
        TEST_SERVER_FINGERPRINT,
        vec![tool_with_description(
            "read-value",
            "Changed tool behavior.",
        )],
    )
    .expect("valid changed tool definition");
    assert_ne!(
        first_routes["mcp__workspace__read-value"].definition_fingerprint,
        changed_routes["mcp__workspace__read-value"].definition_fingerprint
    );
}

#[test]
fn registry_lists_only_the_selected_server_tool_names() {
    let (first_definitions, first_routes) = tool_definitions(
        "local_docs",
        TEST_SERVER_FINGERPRINT,
        vec![tool("read-file"), tool("search")],
    )
    .expect("valid local tool catalog");
    let (second_definitions, second_routes) =
        tool_definitions("other_docs", TEST_SERVER_FINGERPRINT, vec![tool("open")])
            .expect("valid other tool catalog");
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
    let project = std::env::temp_dir().join(format!("openchat-mcp-check-{}", uuid::Uuid::new_v4()));
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
    let storage = TestStorage::new();

    let registry = super::McpRegistry::connect(configs, false, &storage.storage)
        .await
        .expect("disabled servers should not be started");
    assert!(registry.is_none());
}

#[tokio::test]
async fn concurrent_mcp_startup_is_bounded_and_preserves_config_order() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak_active = Arc::new(AtomicUsize::new(0));
    let operations = (0..8)
        .map(|server_index| {
            let active = Arc::clone(&active);
            let peak_active = Arc::clone(&peak_active);
            async move {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak_active.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                server_index
            }
        })
        .collect();

    let connected = super::connect_mcp_servers_concurrently(operations).await;

    assert_eq!(connected, (0..8).collect::<Vec<_>>());
    assert!((2..=super::MAX_CONCURRENT_MCP_STARTUPS).contains(&peak_active.load(Ordering::SeqCst)));
}

#[tokio::test]
#[ignore = "manual MCP startup scheduling benchmark"]
async fn benchmark_mcp_startup_scheduling() {
    let startup_count = 8;
    let simulated_io_delay = Duration::from_millis(25);
    let started = Instant::now();
    for _ in 0..startup_count {
        tokio::time::sleep(simulated_io_delay).await;
    }
    let serial_elapsed = started.elapsed();

    let operations = (0..startup_count)
        .map(|server_index| async move {
            tokio::time::sleep(simulated_io_delay).await;
            server_index
        })
        .collect();
    let started = Instant::now();
    let results = super::connect_mcp_servers_concurrently(operations).await;
    let concurrent_elapsed = started.elapsed();

    assert_eq!(results.len(), startup_count);
    assert!(concurrent_elapsed < serial_elapsed);
    eprintln!(
        "mcp_startup_scheduling servers={startup_count} concurrency={} serial_ms={:.3} bounded_parallel_ms={:.3}",
        super::MAX_CONCURRENT_MCP_STARTUPS,
        serial_elapsed.as_secs_f64() * 1000.0,
        concurrent_elapsed.as_secs_f64() * 1000.0,
    );
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
    let storage = TestStorage::new();
    let task_storage = Arc::clone(&storage.storage);
    let task = tokio::spawn(async move {
        let mut cancellation = cancellation;
        connect_with_approval(
            &task_storage,
            configs,
            McpApprovalContext {
                rules: &rules,
                full_access: false,
                permission_broker: &task_broker,
                request_id: &request_id,
                events: &events,
                cancellation: &mut cancellation,
            },
        )
        .await
    });

    let mut event = received_events
        .recv()
        .await
        .expect("server startup requests approval before connecting");
    assert_eq!(
        event.event.as_deref(),
        Some("chat.tool.permission.requested")
    );
    let data = event
        .data
        .take()
        .expect("permission event includes its request");
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
            "environmentVariables": [],
            "catalogToolNames": ["search_records", "read_record"]
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
fn project_catalog_validates_optional_manifest_tool_names() {
    let program = std::env::current_exe()
        .expect("resolve test executable")
        .to_string_lossy()
        .into_owned();
    let valid = serde_json::json!([{
        "id": "records",
        "enabled": false,
        "program": program,
        "catalogToolNames": ["search_records", "read_record"]
    }]);
    assert!(parse_server_configs(Some(&valid), Path::new(".")).is_ok());

    let invalid = serde_json::json!([{
        "id": "records",
        "enabled": false,
        "program": std::env::current_exe()
            .expect("resolve test executable")
            .to_string_lossy(),
        "catalogToolNames": ["search records"]
    }]);
    assert!(parse_server_configs(Some(&invalid), Path::new(".")).is_err());
}

#[test]
fn catalog_tool_claims_do_not_change_permission_fingerprints() {
    let base: StdioServerSettings = serde_json::from_value(serde_json::json!({
        "id": "records",
        "enabled": false,
        "program": std::env::current_exe()
            .expect("resolve test executable")
            .to_string_lossy()
    }))
    .expect("parse server settings without manifest claims");
    let mut with_claims = base.clone();
    with_claims.catalog_tool_names = Some(vec!["search_records".to_owned()]);

    assert_eq!(
        server_permission_fingerprint(&base).expect("fingerprint base server"),
        server_permission_fingerprint(&with_claims).expect("fingerprint claimed server"),
        "catalog declarations are display claims, not executable server identity"
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
