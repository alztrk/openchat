use std::{ffi::OsString, net::SocketAddr, process::Stdio, sync::OnceLock, time::Duration};

use reqwest::Client;
use serde_json::{Value, json};
use tokio::{
    net::TcpListener,
    process::{Child, Command},
    sync::{Mutex, watch},
    time::{Instant, sleep},
};

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    installer::{InstalledEngine, installed_engine},
    manifests, models,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(300);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_RUNTIME_PROPERTIES_BYTES: usize = 4 * 1024 * 1024;

static RUNNING_SERVER: OnceLock<Mutex<Option<RunningServer>>> = OnceLock::new();
static HEALTH_CLIENT: OnceLock<Result<Client, reqwest::Error>> = OnceLock::new();
static PROPERTIES_CLIENT: OnceLock<Result<Client, reqwest::Error>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeCapabilities {
    pub(crate) context_window: i64,
    pub(crate) supports_images: bool,
    pub(crate) supports_tool_calls: Option<bool>,
}

pub(crate) struct RuntimeChatEndpoint {
    pub(crate) chat_url: String,
    pub(crate) capabilities: RuntimeCapabilities,
}

struct RunningServer {
    engine_id: String,
    model_id: String,
    port: u16,
    child: Child,
    capabilities: Option<RuntimeCapabilities>,
}

pub(crate) async fn start_model(
    storage: &AppStorage,
    model_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }
    let model = models::find(storage, model_id)?.ok_or_else(model_unavailable_error)?;
    if model.engine_id != "llama_cpp" || model.path_kind != "file" || !model.path.is_file() {
        return Err(model_unavailable_error());
    }
    let installed = installed_llama_cpp(storage)?.ok_or_else(engine_not_installed_error)?;
    let state = running_state();
    let mut state = state.lock().await;

    if let Some(server) = state.as_mut() {
        let is_running = server
            .child
            .try_wait()
            .map_err(|_| runtime_error())?
            .is_none();
        if is_running
            && server.engine_id == model.engine_id
            && server.model_id == model.id
            && health_check_or_cancel(server.port, cancellation).await?
        {
            return Ok(status_json(Some(server), "running"));
        }
    }
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }
    if state.is_some() {
        stop_server(storage, &mut state).await?;
    }

    let port = available_loopback_port().await?;
    let started_at = Instant::now();
    let arguments = llama_server_arguments(&model, port)?;
    let child = match Command::new(&installed.entrypoint)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            log_runtime_event(
                storage,
                "runtime_start_failed",
                Some("local_engine_runtime_unavailable"),
                Some(started_at.elapsed()),
            );
            return Err(runtime_error());
        }
    };
    *state = Some(RunningServer {
        engine_id: model.engine_id.clone(),
        model_id: model.id.clone(),
        port,
        child,
        capabilities: None,
    });

    loop {
        let port = {
            let Some(server) = state.as_mut() else {
                return Err(runtime_error());
            };
            if let Some(exit_status) = server.child.try_wait().map_err(|_| runtime_error())? {
                let _ = exit_status;
                *state = None;
                log_runtime_event(
                    storage,
                    "runtime_start_failed",
                    Some("local_engine_start_failed"),
                    Some(started_at.elapsed()),
                );
                return Err(runtime_start_failed_error());
            }
            server.port
        };
        let ready = match health_check_or_cancel(port, cancellation).await {
            Ok(ready) => ready,
            Err(error) => {
                stop_server(storage, &mut state).await?;
                let event = if error.code == "operation_cancelled" {
                    "runtime_start_cancelled"
                } else {
                    "runtime_start_failed"
                };
                log_runtime_event(storage, event, Some(error.code), Some(started_at.elapsed()));
                return Err(error);
            }
        };
        if ready {
            let capabilities = match runtime_capabilities_or_cancel(port, cancellation).await {
                Ok(capabilities) => capabilities,
                Err(error) => {
                    stop_server(storage, &mut state).await?;
                    let event = if error.code == "operation_cancelled" {
                        "runtime_capability_check_cancelled"
                    } else {
                        "runtime_capability_check_failed"
                    };
                    log_runtime_event(storage, event, Some(error.code), Some(started_at.elapsed()));
                    return Err(error);
                }
            };
            let Some(server) = state.as_mut() else {
                return Err(runtime_error());
            };
            server.capabilities = Some(capabilities);
            log_runtime_event(storage, "runtime_started", None, Some(started_at.elapsed()));
            return Ok(status_json(state.as_ref(), "running"));
        }
        if started_at.elapsed() >= STARTUP_TIMEOUT {
            stop_server(storage, &mut state).await?;
            log_runtime_event(
                storage,
                "runtime_start_timeout",
                Some("local_engine_start_timeout"),
                Some(started_at.elapsed()),
            );
            return Err(runtime_start_timeout_error());
        }

        tokio::select! {
            changed = cancellation.changed() => {
                if changed.is_err() || *cancellation.borrow() {
                    stop_server(storage, &mut state).await?;
                    log_runtime_event(
                        storage,
                        "runtime_start_cancelled",
                        Some("operation_cancelled"),
                        Some(started_at.elapsed()),
                    );
                    return Err(cancelled_error());
                }
            }
            _ = sleep(HEALTH_POLL_INTERVAL) => {}
        }
    }
}

pub(crate) async fn stop(storage: &AppStorage) -> Result<Value, ServiceError> {
    let mut state = running_state().lock().await;
    stop_server(storage, &mut state).await?;
    Ok(status_json(None, "stopped"))
}

pub(crate) async fn status(storage: &AppStorage) -> Result<Value, ServiceError> {
    let mut state = running_state().lock().await;
    let Some(server) = state.as_mut() else {
        return Ok(status_json(None, "stopped"));
    };
    if server
        .child
        .try_wait()
        .map_err(|_| runtime_error())?
        .is_some()
    {
        *state = None;
        log_runtime_event(
            storage,
            "runtime_exited",
            Some("local_engine_runtime_unavailable"),
            None,
        );
        return Ok(status_json(None, "stopped"));
    }
    let status = if health_check(server.port).await {
        "running"
    } else {
        "unhealthy"
    };
    Ok(status_json(state.as_ref(), status))
}

pub(crate) async fn chat_url(
    storage: &AppStorage,
    model_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<RuntimeChatEndpoint, ServiceError> {
    let status = start_model(storage, model_id, cancellation).await?;
    let port = status
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(runtime_error)?;
    let state = running_state().lock().await;
    let server = state
        .as_ref()
        .filter(|server| server.model_id == model_id && server.port == port)
        .ok_or_else(runtime_error)?;
    let capabilities = server.capabilities.ok_or_else(runtime_capability_error)?;
    Ok(RuntimeChatEndpoint {
        chat_url: format!("http://127.0.0.1:{port}/v1/chat/completions"),
        capabilities,
    })
}

pub(crate) fn is_model_available(
    storage: &AppStorage,
    model: &models::RegisteredModel,
) -> Result<bool, ServiceError> {
    if model.engine_id != "llama_cpp" || model.path_kind != "file" || !model.path.is_file() {
        return Ok(false);
    }
    Ok(installed_llama_cpp(storage)?.is_some())
}

async fn health_check(port: u16) -> bool {
    let client = HEALTH_CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
    });
    let Ok(client) = client else {
        return false;
    };
    client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

async fn health_check_or_cancel(
    port: u16,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<bool, ServiceError> {
    if *cancellation.borrow() || cancellation.has_changed().is_err() {
        return Err(cancelled_error());
    }
    tokio::select! {
        changed = cancellation.changed() => {
            if changed.is_err() || *cancellation.borrow() {
                Err(cancelled_error())
            } else {
                Ok(false)
            }
        }
        ready = health_check(port) => Ok(ready),
    }
}

async fn runtime_capabilities_or_cancel(
    port: u16,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<RuntimeCapabilities, ServiceError> {
    if *cancellation.borrow() || cancellation.has_changed().is_err() {
        return Err(cancelled_error());
    }
    tokio::select! {
        changed = cancellation.changed() => {
            if changed.is_err() || *cancellation.borrow() {
                Err(cancelled_error())
            } else {
                Err(runtime_capability_error())
            }
        }
        capabilities = read_runtime_capabilities(port) => capabilities,
    }
}

async fn read_runtime_capabilities(port: u16) -> Result<RuntimeCapabilities, ServiceError> {
    let client = PROPERTIES_CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(10))
            .build()
    });
    let client = client.as_ref().map_err(|_| runtime_capability_error())?;
    let mut response = client
        .get(format!("http://127.0.0.1:{port}/props"))
        .send()
        .await
        .map_err(|_| runtime_capability_error())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_RUNTIME_PROPERTIES_BYTES as u64)
    {
        return Err(runtime_capability_error());
    }

    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| runtime_capability_error())?
    {
        if body.len().saturating_add(chunk.len()) > MAX_RUNTIME_PROPERTIES_BYTES {
            return Err(runtime_capability_error());
        }
        body.extend_from_slice(&chunk);
    }
    let properties =
        serde_json::from_slice::<Value>(&body).map_err(|_| runtime_capability_error())?;
    parse_runtime_capabilities(&properties)
}

fn parse_runtime_capabilities(properties: &Value) -> Result<RuntimeCapabilities, ServiceError> {
    let context_window = properties
        .pointer("/default_generation_settings/n_ctx")
        .and_then(Value::as_i64)
        .filter(|context_window| *context_window > 0)
        .ok_or_else(runtime_capability_error)?;
    let supports_images = properties
        .pointer("/modalities/vision")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let supports_tool_calls = match (
        properties
            .pointer("/chat_template_caps/supports_tools")
            .and_then(Value::as_bool),
        properties
            .pointer("/chat_template_caps/supports_tool_calls")
            .and_then(Value::as_bool),
    ) {
        (Some(supports_tools), Some(supports_tool_calls)) => {
            Some(supports_tools && supports_tool_calls)
        }
        _ => None,
    };

    Ok(RuntimeCapabilities {
        context_window,
        supports_images,
        supports_tool_calls,
    })
}

async fn available_loopback_port() -> Result<u16, ServiceError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| runtime_error())?;
    let address: SocketAddr = listener.local_addr().map_err(|_| runtime_error())?;
    Ok(address.port())
}

fn llama_server_arguments(
    model: &models::RegisteredModel,
    port: u16,
) -> Result<Vec<OsString>, ServiceError> {
    let mut arguments = vec![
        OsString::from("--model"),
        model.path.as_os_str().to_owned(),
        OsString::from("--host"),
        OsString::from("127.0.0.1"),
        OsString::from("--port"),
        OsString::from(port.to_string()),
        OsString::from("--alias"),
        OsString::from(&model.id),
    ];
    if let Some(projector) = models::vision_projector_path(&model.path)? {
        arguments.push(OsString::from("--mmproj"));
        arguments.push(projector.into_os_string());
    }
    Ok(arguments)
}

fn installed_llama_cpp(storage: &AppStorage) -> Result<Option<InstalledEngine>, ServiceError> {
    let manifest = manifests()?
        .into_iter()
        .find(|manifest| manifest.engine_id == "llama_cpp")
        .ok_or_else(engine_not_installed_error)?;
    let variants = manifest.variants.iter().filter(|variant| {
        variant.os == std::env::consts::OS && variant.architecture == std::env::consts::ARCH
    });
    for variant in variants {
        if let Some(engine) = installed_engine(storage, &manifest, variant)? {
            return Ok(Some(engine));
        }
    }
    Ok(None)
}

fn running_state() -> &'static Mutex<Option<RunningServer>> {
    RUNNING_SERVER.get_or_init(|| Mutex::new(None))
}

async fn stop_server(
    storage: &AppStorage,
    state: &mut Option<RunningServer>,
) -> Result<(), ServiceError> {
    let Some(mut server) = state.take() else {
        return Ok(());
    };
    if server
        .child
        .try_wait()
        .map_err(|_| runtime_error())?
        .is_none()
        && (server.child.kill().await.is_err() || server.child.wait().await.is_err())
    {
        log_runtime_event(
            storage,
            "runtime_stop_failed",
            Some("local_engine_runtime_unavailable"),
            None,
        );
        return Err(runtime_error());
    }
    log_runtime_event(storage, "runtime_stopped", None, None);
    Ok(())
}

fn log_runtime_event(
    storage: &AppStorage,
    event: &'static str,
    code: Option<&'static str>,
    duration: Option<Duration>,
) {
    if storage
        .log_local_engine_event(event, code, duration)
        .is_err()
    {
        eprintln!("local_engine_diagnostic_write_failed");
    }
}

fn status_json(server: Option<&RunningServer>, status: &str) -> Value {
    json!({
        "status": status,
        "engineId": server.map(|server| server.engine_id.as_str()),
        "modelId": server.map(|server| server.model_id.as_str()),
        "port": server.map(|server| server.port),
    })
}

fn engine_not_installed_error() -> ServiceError {
    ServiceError::new(
        "local_engine_not_installed",
        "The selected local inference engine is not installed.",
        false,
    )
}

fn model_unavailable_error() -> ServiceError {
    ServiceError::new(
        "local_model_unavailable",
        "The selected local model file is unavailable or unsupported.",
        false,
    )
}

fn runtime_start_failed_error() -> ServiceError {
    ServiceError::new(
        "local_engine_start_failed",
        "The local inference engine exited before its health check passed.",
        true,
    )
}

fn runtime_start_timeout_error() -> ServiceError {
    ServiceError::new(
        "local_engine_start_timeout",
        "The local inference engine did not become ready before the startup limit.",
        true,
    )
}

fn runtime_error() -> ServiceError {
    ServiceError::new(
        "local_engine_runtime_unavailable",
        "The local inference engine could not be controlled.",
        true,
    )
}

fn runtime_capability_error() -> ServiceError {
    ServiceError::new(
        "local_engine_capability_unavailable",
        "The local inference server did not report a valid context window and capability status.",
        true,
    )
}

fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "Starting the local model was cancelled.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::watch,
        time::{Duration, timeout},
    };
    use uuid::Uuid;

    use crate::storage::AppStorage;

    use super::{
        MAX_RUNTIME_PROPERTIES_BYTES, RuntimeCapabilities, health_check_or_cancel,
        llama_server_arguments, parse_runtime_capabilities, read_runtime_capabilities, start_model,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-local-runtime-test-{}", Uuid::new_v4()));
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
        let error = health_check_or_cancel(0, &mut cancellation)
            .await
            .expect_err("a cancelled readiness wait should stop");
        assert_eq!(error.code, "operation_cancelled");

        let (sender, mut cancellation) = watch::channel(false);
        drop(sender);
        let error = health_check_or_cancel(0, &mut cancellation)
            .await
            .expect_err("a closed operation should stop its readiness wait");
        assert_eq!(error.code, "operation_cancelled");
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
            tokio::spawn(async move { health_check_or_cancel(port, &mut cancellation).await });
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
            let mut request = [0; 1024];
            let _ = stream
                .read(&mut request)
                .await
                .expect("the HTTP request should be readable");
            let body = r#"{"default_generation_settings":{"n_ctx":32768},"modalities":{"vision":true},"chat_template_caps":{"supports_tools":true,"supports_tool_calls":true},"model_path":"private"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("the properties response should be written");
        });

        assert_eq!(
            read_runtime_capabilities(port)
                .await
                .expect("reported properties should be read"),
            RuntimeCapabilities {
                context_window: 32_768,
                supports_images: true,
                supports_tool_calls: Some(true),
            }
        );
        responder
            .await
            .expect("the properties server should finish");
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
            read_runtime_capabilities(port)
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
                context_window: 32_768,
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
                context_window: 4_096,
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
}
