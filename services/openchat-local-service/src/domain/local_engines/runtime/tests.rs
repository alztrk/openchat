//! Tests for local runtime startup, capabilities, and external servers.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::watch,
    time::{Duration, timeout},
};
use uuid::Uuid;

use crate::storage::AppStorage;

use super::{
    ConnectedExternalLlamaServer, ExternalLlamaModel, ExternalLlamaServerCandidate,
    MAX_RUNTIME_PROPERTIES_BYTES, RuntimeCapabilities, bridge_cancellation_sources,
    configure_llama_server_api_key, excluding_managed_server_processes, external_model_json,
    external_model_route_id, external_server_for_route, external_server_group_id,
    health_check_or_cancel, installed_engine_for, llama_server_arguments, managed_server_group_id,
    model_lifecycle, parse_external_capabilities, parse_external_model_ids,
    parse_runtime_capabilities, read_runtime_capabilities, remove_external_server,
    retain_detected_external_servers, runtime_status_json, start_model,
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("openchat-local-runtime-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).expect("test directory should be created");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn configured_llama_server_binary_is_used_without_a_managed_install() {
    let temporary = TestDirectory::new();
    let executable = temporary.0.join(if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    });
    fs::write(&executable, b"test executable").expect("fixture should be written");
    let storage = AppStorage::open_at(temporary.0.clone()).expect("storage should open");
    crate::local_engines::settings::set_llama_server_executable_path(
        storage.root(),
        executable.to_str(),
    )
    .expect("the executable setting should be saved");

    let installed = installed_engine_for(&storage, "llama_cpp")
        .expect("engine lookup should succeed")
        .expect("the configured executable should be available");

    assert_eq!(installed.variant_id, "user-configured");
    assert_eq!(
        installed.entrypoint,
        executable.canonicalize().expect("path should canonicalize")
    );
}

#[test]
fn external_detection_excludes_the_openchat_managed_process() {
    let candidates = vec![
        ExternalLlamaServerCandidate {
            process_id: 4216,
            port: 8080,
        },
        ExternalLlamaServerCandidate {
            process_id: 5732,
            port: 8081,
        },
        ExternalLlamaServerCandidate {
            process_id: 9212,
            port: 8082,
        },
    ];
    let managed_process_ids = HashSet::from([4216, 9212]);

    assert_eq!(
        excluding_managed_server_processes(candidates, &managed_process_ids),
        vec![ExternalLlamaServerCandidate {
            process_id: 5732,
            port: 8081,
        }]
    );
}

#[test]
fn parses_external_model_ids_without_duplicates_or_invalid_values() {
    let parsed = parse_external_model_ids(&serde_json::json!({
        "data": [
            {"id": "qwen3-8b"},
            {"id": "qwen3-8b"},
            {"id": "llama-3.1-8b"}
        ]
    }))
    .expect("the OpenAI-compatible catalog should parse");
    assert_eq!(parsed, ["qwen3-8b", "llama-3.1-8b"]);

    for response in [
        serde_json::json!({"data": []}),
        serde_json::json!({"data": [{"id": "  "}]}),
        serde_json::json!({"data": [{"id": "qwen\n8b"}]}),
    ] {
        assert_eq!(
            parse_external_model_ids(&response)
                .expect_err("invalid external catalog values must fail")
                .code,
            "local_engine_external_server_catalog_invalid"
        );
    }
}

#[test]
fn leaves_unreported_external_capabilities_unknown() {
    assert_eq!(
        parse_external_capabilities(&serde_json::json!({
            "default_generation_settings": {"n_ctx": 32768},
            "modalities": {"vision": true}
        })),
        RuntimeCapabilities {
            context_window: Some(32_768),
            supports_images: true,
            supports_tool_calls: None,
        }
    );
    assert_eq!(
        parse_external_capabilities(&serde_json::json!({})),
        RuntimeCapabilities {
            context_window: None,
            supports_images: false,
            supports_tool_calls: None,
        }
    );
}

#[test]
fn external_model_route_ids_are_stable_and_do_not_expose_model_text() {
    let model_id = "private/model-id";
    let route_id = external_model_route_id(4216, 8080, model_id);

    assert_eq!(route_id, external_model_route_id(4216, 8080, model_id));
    assert_ne!(route_id, external_model_route_id(5732, 8081, model_id));
    assert_ne!(
        route_id,
        external_model_route_id(4216, 8080, "different-model")
    );
    assert!(route_id.starts_with("external-llama-"));
    assert!(!route_id.contains("private"));
}

#[test]
fn external_model_catalog_does_not_invent_unknown_capabilities() {
    let model = ExternalLlamaModel {
        route_id: external_model_route_id(4216, 8080, "qwen3-8b"),
        provider_model_id: "qwen3-8b".to_owned(),
        capabilities: RuntimeCapabilities {
            context_window: None,
            supports_images: false,
            supports_tool_calls: None,
        },
    };
    let json = external_model_json(&model, 4216, 8080);

    assert_eq!(json["isAvailable"], true);
    assert_eq!(json["contextWindow"], serde_json::Value::Null);
    assert_eq!(json["supportsTools"], serde_json::Value::Null);
    assert_eq!(json["supportsImages"], false);
    assert_eq!(json["groupId"], external_server_group_id(4216, 8080));
}

#[test]
fn external_routes_resolve_to_their_own_connected_server() {
    let model = |process_id, port, model_id: &str| ExternalLlamaModel {
        route_id: external_model_route_id(process_id, port, model_id),
        provider_model_id: model_id.to_owned(),
        capabilities: RuntimeCapabilities {
            context_window: None,
            supports_images: false,
            supports_tool_calls: None,
        },
    };
    let servers = HashMap::from([
        (
            (4216, 8080),
            ConnectedExternalLlamaServer {
                process_id: 4216,
                port: 8080,
                models: vec![model(4216, 8080, "same-model")],
            },
        ),
        (
            (5732, 8081),
            ConnectedExternalLlamaServer {
                process_id: 5732,
                port: 8081,
                models: vec![model(5732, 8081, "same-model")],
            },
        ),
    ]);
    let second_route = external_model_route_id(5732, 8081, "same-model");

    let selected = external_server_for_route(&servers, &second_route)
        .expect("a route should resolve to its own server");

    assert_eq!(selected.process_id, 5732);
    assert_eq!(selected.port, 8081);
    assert_eq!(
        managed_server_group_id(5001, 51234),
        "managed-llama-server:5001:51234"
    );
}

#[test]
fn disconnect_and_detection_reconciliation_preserve_other_user_servers() {
    let model = |process_id, port, model_id: &str| ExternalLlamaModel {
        route_id: external_model_route_id(process_id, port, model_id),
        provider_model_id: model_id.to_owned(),
        capabilities: RuntimeCapabilities {
            context_window: None,
            supports_images: false,
            supports_tool_calls: None,
        },
    };
    let first = ConnectedExternalLlamaServer {
        process_id: 4216,
        port: 8080,
        models: vec![model(4216, 8080, "model-a")],
    };
    let second = ConnectedExternalLlamaServer {
        process_id: 5732,
        port: 8081,
        models: vec![model(5732, 8081, "model-b")],
    };
    let mut servers = HashMap::from([((4216, 8080), first), ((5732, 8081), second)]);

    assert!(remove_external_server(&mut servers, 4216, 8080));
    assert!(!remove_external_server(&mut servers, 4216, 8080));
    retain_detected_external_servers(
        &mut servers,
        &[ExternalLlamaServerCandidate {
            process_id: 5732,
            port: 8081,
        }],
    );

    assert_eq!(servers.len(), 1);
    assert!(
        external_server_for_route(&servers, &external_model_route_id(5732, 8081, "model-b"))
            .is_some()
    );
}

#[test]
fn runtime_status_lists_multiple_managed_servers() {
    let status = runtime_status_json(
        vec![
            serde_json::json!({"modelId": "model-a", "port": 51234, "status": "running"}),
            serde_json::json!({"modelId": "model-b", "port": 51235, "status": "running"}),
        ],
        "running",
    );

    assert_eq!(status["status"], "running");
    assert_eq!(status["servers"].as_array().map(Vec::len), Some(2));
    assert_eq!(status["servers"][1]["modelId"], "model-b");
}

#[test]
fn llama_server_arguments_pass_a_single_matching_vision_projector() {
    let temporary = TestDirectory::new();
    let model_path = temporary.0.join("vision-model.gguf");
    let projector_path = temporary.0.join("mmproj-vision-model.gguf");
    fs::write(&model_path, b"model").expect("model fixture should be written");
    fs::write(&projector_path, b"projector").expect("projector fixture should be written");
    let model = crate::local_engines::models::RegisteredModel {
        id: "model-id".to_owned(),
        engine_id: "llama_cpp".to_owned(),
        display_name: "vision-model".to_owned(),
        path: model_path,
        path_kind: "file".to_owned(),
        created_at_unix_ms: 0,
    };

    let arguments = llama_server_arguments(&model, 12345).expect("arguments should build");
    let projector_argument = arguments
        .iter()
        .position(|argument| argument == "--mmproj")
        .expect("projector argument should be included");

    assert_eq!(
        arguments[projector_argument + 1],
        projector_path.as_os_str()
    );
    assert_eq!(arguments[5], "12345");
    assert!(arguments.iter().any(|argument| argument == "--no-ui"));
}

#[test]
fn managed_llama_server_api_key_is_given_through_its_environment() {
    let api_key = zeroize::Zeroizing::new("fixture-token".to_owned());
    let mut command = tokio::process::Command::new("llama-server");
    command.arg("--model").arg("fixture-model.gguf");
    configure_llama_server_api_key(&mut command, &api_key);

    let environment_value = command
        .as_std()
        .get_envs()
        .find_map(|(name, value)| {
            (name == std::ffi::OsStr::new("LLAMA_API_KEY"))
                .then_some(value)
                .flatten()
        })
        .expect("managed server key should be set in the child environment");

    assert_eq!(environment_value, std::ffi::OsStr::new("fixture-token"));
    assert!(
        command
            .as_std()
            .get_args()
            .all(|argument| argument != std::ffi::OsStr::new("fixture-token"))
    );
}

#[test]
fn llama_server_arguments_omit_projector_for_text_only_models() {
    let temporary = TestDirectory::new();
    let model_path = temporary.0.join("text-model.gguf");
    fs::write(&model_path, b"model").expect("model fixture should be written");
    let model = crate::local_engines::models::RegisteredModel {
        id: "model-id".to_owned(),
        engine_id: "llama_cpp".to_owned(),
        display_name: "text-model".to_owned(),
        path: model_path,
        path_kind: "file".to_owned(),
        created_at_unix_ms: 0,
    };

    let arguments = llama_server_arguments(&model, 12345).expect("arguments should build");

    assert!(!arguments.iter().any(|argument| argument == "--mmproj"));
}

#[tokio::test]
async fn honors_cancellation_before_looking_up_or_starting_a_model() {
    let temporary = TestDirectory::new();
    let storage = AppStorage::open_at(temporary.0.clone()).expect("storage should open");
    let (_sender, mut cancellation) = watch::channel(true);

    let error = start_model(&storage, "missing-model", &mut cancellation)
        .await
        .expect_err("a cancelled start should not continue");

    assert_eq!(error.code, "operation_cancelled");
}

#[tokio::test]
async fn readiness_wait_is_interrupted_by_cancel_or_operation_shutdown() {
    let (sender, mut cancellation) = watch::channel(false);
    sender
        .send(true)
        .expect("the active readiness request should receive cancellation");
    let error = health_check_or_cancel(0, None, &mut cancellation)
        .await
        .expect_err("a cancelled readiness wait should stop");
    assert_eq!(error.code, "operation_cancelled");

    let (sender, mut cancellation) = watch::channel(false);
    drop(sender);
    let error = health_check_or_cancel(0, None, &mut cancellation)
        .await
        .expect_err("a closed operation should stop its readiness wait");
    assert_eq!(error.code, "operation_cancelled");
}

#[tokio::test]
async fn stop_generation_interrupts_an_active_start_cancellation_bridge() {
    let (caller_sender, caller_cancellation) = watch::channel(false);
    let (stop_sender, stop_generation) = watch::channel(7_u64);
    let (mut operation_cancellation, bridge) =
        bridge_cancellation_sources(&caller_cancellation, &stop_generation, 7);

    stop_sender.send_replace(8);
    timeout(Duration::from_millis(250), operation_cancellation.changed())
        .await
        .expect("stopping the model should interrupt its start operation")
        .expect("the cancellation bridge should remain available");
    assert!(*operation_cancellation.borrow());
    bridge.abort();
    drop(caller_sender);
}

#[tokio::test]
async fn model_lifecycle_locks_are_shared_per_model_only() {
    let model_id = format!("test-model-{}", Uuid::new_v4());
    let other_model_id = format!("test-model-{}", Uuid::new_v4());
    let first = model_lifecycle(&model_id).await;
    let same_model = model_lifecycle(&model_id).await;
    let other_model = model_lifecycle(&other_model_id).await;

    assert!(std::sync::Arc::ptr_eq(&first, &same_model));
    assert!(!std::sync::Arc::ptr_eq(&first, &other_model));

    let _first_guard = first.operation.lock().await;
    let other_guard = timeout(Duration::from_millis(250), other_model.operation.lock())
        .await
        .expect("different model starts should not wait on this model");
    drop(other_guard);
    assert!(
        timeout(Duration::from_millis(25), same_model.operation.lock())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cancellation_interrupts_an_in_flight_health_request() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the test health endpoint should bind");
    let port = listener
        .local_addr()
        .expect("the test health endpoint should have an address")
        .port();
    let (sender, mut cancellation) = watch::channel(false);
    let readiness =
        tokio::spawn(async move { health_check_or_cancel(port, None, &mut cancellation).await });
    let (_connection, _) = listener
        .accept()
        .await
        .expect("the health request should reach the test endpoint");

    sender
        .send(true)
        .expect("the active readiness request should receive cancellation");
    let result = timeout(Duration::from_millis(250), readiness)
        .await
        .expect("cancellation should interrupt the pending health request")
        .expect("the readiness task should finish");
    let error = result.expect_err("a cancelled readiness wait should stop");

    assert_eq!(error.code, "operation_cancelled");
}

#[tokio::test]
async fn managed_health_check_does_not_follow_redirects() {
    let target_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the redirect target should bind");
    let target_address = target_listener
        .local_addr()
        .expect("the redirect target should have an address");
    let target = tokio::spawn(async move {
        let accepted = timeout(Duration::from_secs(1), target_listener.accept()).await;
        let Ok(Ok((mut stream, _))) = accepted else {
            return false;
        };
        let _ = read_request_headers(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .expect("the redirect target response should be written");
        true
    });

    let source_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the health endpoint should bind");
    let port = source_listener
        .local_addr()
        .expect("the health endpoint should have an address")
        .port();
    let source = tokio::spawn(async move {
        let (mut stream, _) = source_listener
            .accept()
            .await
            .expect("the health request should reach its endpoint");
        let _ = read_request_headers(&mut stream).await;
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/health\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("the health redirect should be written");
    });

    assert!(!super::health_check(port, Some("fixture-token")).await);
    source
        .await
        .expect("the health endpoint fixture should finish");
    assert!(
        !target
            .await
            .expect("the redirect target fixture should finish")
    );
}

/// Reads a local HTTP fixture request before the fixture sends its response.
async fn read_request_headers(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut byte = [0; 1];
    while !request.ends_with(b"\r\n\r\n") {
        let read = stream.read(&mut byte).await.expect("read request headers");
        assert_ne!(read, 0, "the client closed before sending request headers");
        request.push(byte[0]);
    }
    request
}

#[tokio::test]
async fn reads_reported_capabilities_from_the_local_props_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the test properties endpoint should bind");
    let port = listener
        .local_addr()
        .expect("the test properties endpoint should have an address")
        .port();
    let responder = tokio::spawn(async move {
        let (mut stream, _) = listener
            .accept()
            .await
            .expect("the properties request should reach the test endpoint");
        let request = read_request_headers(&mut stream).await;
        let body = r#"{"default_generation_settings":{"n_ctx":32768},"modalities":{"vision":true},"chat_template_caps":{"supports_tools":true,"supports_tool_calls":true},"model_path":"private"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("the properties response should be written");
        request
    });

    assert_eq!(
        read_runtime_capabilities(port, Some("fixture-token"))
            .await
            .expect("reported properties should be read"),
        RuntimeCapabilities {
            context_window: Some(32_768),
            supports_images: true,
            supports_tool_calls: Some(true),
        }
    );
    let request = responder
        .await
        .expect("the properties server should finish");
    assert!(
        String::from_utf8_lossy(&request)
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-token")
    );
}

#[tokio::test]
async fn rejects_a_props_response_over_the_body_limit() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the test properties endpoint should bind");
    let port = listener
        .local_addr()
        .expect("the test properties endpoint should have an address")
        .port();
    let responder = tokio::spawn(async move {
        let (mut stream, _) = listener
            .accept()
            .await
            .expect("the properties request should reach the test endpoint");
        let mut request = [0; 1024];
        let _ = stream
            .read(&mut request)
            .await
            .expect("the HTTP request should be readable");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MAX_RUNTIME_PROPERTIES_BYTES + 1
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("the oversized response header should be written");
    });

    assert_eq!(
        read_runtime_capabilities(port, None)
            .await
            .expect_err("an oversized response must be rejected")
            .code,
        "local_engine_capability_unavailable"
    );
    responder
        .await
        .expect("the properties server should finish");
}

#[test]
fn parses_only_reported_runtime_capabilities() {
    let properties = serde_json::json!({
        "default_generation_settings": {"n_ctx": 32_768},
        "modalities": {"vision": true},
        "chat_template_caps": {
            "supports_tools": true,
            "supports_tool_calls": true
        },
        "model_path": "private path must not be returned"
    });

    assert_eq!(
        parse_runtime_capabilities(&properties).expect("capabilities should parse"),
        RuntimeCapabilities {
            context_window: Some(32_768),
            supports_images: true,
            supports_tool_calls: Some(true),
        }
    );
}

#[test]
fn refuses_missing_or_invalid_effective_context() {
    for properties in [
        serde_json::json!({"modalities": {"vision": true}}),
        serde_json::json!({"default_generation_settings": {"n_ctx": 0}}),
        serde_json::json!({"default_generation_settings": {"n_ctx": "32768"}}),
    ] {
        assert_eq!(
            parse_runtime_capabilities(&properties)
                .expect_err("invalid context metadata should fail")
                .code,
            "local_engine_capability_unavailable"
        );
    }
}

#[test]
fn unknown_tool_and_vision_capabilities_are_not_inferred() {
    let properties = serde_json::json!({
        "default_generation_settings": {"n_ctx": 4_096},
        "chat_template_caps": {"supports_tools": true}
    });

    assert_eq!(
        parse_runtime_capabilities(&properties).expect("context should parse"),
        RuntimeCapabilities {
            context_window: Some(4_096),
            supports_images: false,
            supports_tool_calls: None,
        }
    );

    for chat_template_caps in [
        serde_json::json!({
            "supports_tools": false,
            "supports_tool_calls": true
        }),
        serde_json::json!({
            "supports_tools": true,
            "supports_tool_calls": false
        }),
    ] {
        let properties = serde_json::json!({
            "default_generation_settings": {"n_ctx": 4_096},
            "chat_template_caps": chat_template_caps
        });
        assert_eq!(
            parse_runtime_capabilities(&properties)
                .expect("known context should parse")
                .supports_tool_calls,
            Some(false)
        );
    }
}
