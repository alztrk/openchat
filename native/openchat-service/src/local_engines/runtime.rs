use std::{
    ffi::OsString, net::SocketAddr, path::PathBuf, process::Stdio, sync::OnceLock, time::Duration,
};

use reqwest::Client;
use serde_json::{Value, json};
use tokio::{
    net::TcpListener,
    process::{Child, Command},
    sync::{Mutex, watch},
    time::{Instant, sleep},
};
use zeroize::{Zeroize, Zeroizing};

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    installer::{InstalledEngine, installed_engine},
    manifests, models, wsl,
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
    pub(crate) model_id: String,
    pub(crate) capabilities: RuntimeCapabilities,
    pub(crate) api_key: Option<Zeroizing<String>>,
}

struct RunningServer {
    engine_id: String,
    model_id: String,
    provider_model_id: String,
    port: u16,
    child: Child,
    capabilities: Option<RuntimeCapabilities>,
    api_key: Option<Zeroizing<String>>,
    session_directory: Option<PathBuf>,
}

struct RuntimeLaunch {
    arguments: Vec<OsString>,
    current_directory: PathBuf,
    use_wsl: bool,
    initial_capabilities: Option<RuntimeCapabilities>,
    provider_model_id: String,
    api_key: Option<Zeroizing<String>>,
    session_directory: Option<PathBuf>,
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
    if !model_path_matches_engine(&model)? {
        return Err(model_unavailable_error());
    }
    let installed =
        installed_engine_for(storage, &model.engine_id)?.ok_or_else(engine_not_installed_error)?;
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
            && health_check_or_cancel(
                server.port,
                server.api_key.as_deref().map(String::as_str),
                cancellation,
            )
            .await?
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
    let launch = match model.engine_id.as_str() {
        "llama_cpp" => RuntimeLaunch {
            arguments: llama_server_arguments(&model, port)?,
            current_directory: installed.root.clone(),
            use_wsl: false,
            initial_capabilities: None,
            provider_model_id: model.id.clone(),
            api_key: None,
            session_directory: None,
        },
        "exllama" => prepare_tabby_session(&model, &installed, port)?,
        "vllm" => prepare_vllm_session(&model, &installed, port).await?,
        _ => return Err(model_unavailable_error()),
    };
    let mut command = if launch.use_wsl {
        wsl::command(&installed.entrypoint, true).map_err(|_| runtime_error())?
    } else {
        Command::new(&installed.entrypoint)
    };
    let child = match command
        .args(&launch.arguments)
        .current_dir(&launch.current_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            cleanup_session_directory(launch.session_directory.as_deref()).await?;
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
        provider_model_id: launch.provider_model_id,
        port,
        child,
        capabilities: launch.initial_capabilities,
        api_key: launch.api_key,
        session_directory: launch.session_directory,
    });

    loop {
        let (port, has_exited, exited_session) = {
            let Some(server) = state.as_mut() else {
                return Err(runtime_error());
            };
            let has_exited = server
                .child
                .try_wait()
                .map_err(|_| runtime_error())?
                .is_some();
            let session = if has_exited {
                server.session_directory.take()
            } else {
                None
            };
            (server.port, has_exited, session)
        };
        if has_exited {
            *state = None;
            cleanup_session_directory(exited_session.as_deref()).await?;
            log_runtime_event(
                storage,
                "runtime_start_failed",
                Some("local_engine_start_failed"),
                Some(started_at.elapsed()),
            );
            return Err(runtime_start_failed_error());
        }
        let api_key = state
            .as_ref()
            .and_then(|server| server.api_key.as_deref())
            .map(String::as_str);
        let ready = match health_check_or_cancel(port, api_key, cancellation).await {
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
            let initial_capabilities = state.as_ref().and_then(|server| server.capabilities);
            let capabilities_result = match initial_capabilities {
                Some(capabilities) => Ok(capabilities),
                None => runtime_capabilities_or_cancel(port, api_key, cancellation).await,
            };
            let capabilities = match capabilities_result {
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
    let has_exited = server
        .child
        .try_wait()
        .map_err(|_| runtime_error())?
        .is_some();
    if has_exited {
        let session_directory = server.session_directory.take();
        *state = None;
        cleanup_session_directory(session_directory.as_deref()).await?;
        log_runtime_event(
            storage,
            "runtime_exited",
            Some("local_engine_runtime_unavailable"),
            None,
        );
        return Ok(status_json(None, "stopped"));
    }
    let status = if health_check(server.port, server.api_key.as_deref().map(String::as_str)).await {
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
        model_id: server.provider_model_id.clone(),
        capabilities,
        api_key: server.api_key.clone(),
    })
}

pub(crate) fn is_model_available(
    storage: &AppStorage,
    model: &models::RegisteredModel,
) -> Result<bool, ServiceError> {
    if !model_path_matches_engine(model)? {
        return Ok(false);
    }
    Ok(installed_engine_for(storage, &model.engine_id)?.is_some())
}

async fn health_check(port: u16, api_key: Option<&str>) -> bool {
    let client = HEALTH_CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
    });
    let Ok(client) = client else {
        return false;
    };
    let mut request = client.get(format!("http://127.0.0.1:{port}/health"));
    if let Some(api_key) = api_key {
        request = request.bearer_auth(api_key);
    }
    request
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

async fn health_check_or_cancel(
    port: u16,
    api_key: Option<&str>,
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
        ready = health_check(port, api_key) => Ok(ready),
    }
}

async fn runtime_capabilities_or_cancel(
    port: u16,
    api_key: Option<&str>,
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
        capabilities = read_runtime_capabilities(port, api_key) => capabilities,
    }
}

async fn read_runtime_capabilities(
    port: u16,
    api_key: Option<&str>,
) -> Result<RuntimeCapabilities, ServiceError> {
    let client = PROPERTIES_CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(10))
            .build()
    });
    let client = client.as_ref().map_err(|_| runtime_capability_error())?;
    let mut request = client.get(format!("http://127.0.0.1:{port}/props"));
    if let Some(api_key) = api_key {
        request = request.bearer_auth(api_key);
    }
    let mut response = request
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

fn model_path_matches_engine(model: &models::RegisteredModel) -> Result<bool, ServiceError> {
    Ok(match model.engine_id.as_str() {
        "llama_cpp" => {
            model.path_kind == "file"
                && model.path.is_file()
                && model
                    .path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf"))
        }
        "exllama" => model.path_kind == "directory" && model.path.is_dir(),
        "vllm" => {
            model.path_kind == "directory"
                && model.path.is_dir()
                && models::has_transformers_model_files(&model.path)?
        }
        _ => false,
    })
}

fn installed_engine_for(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<Option<InstalledEngine>, ServiceError> {
    let manifest = manifests()?
        .into_iter()
        .find(|manifest| manifest.engine_id == engine_id)
        .ok_or_else(engine_not_installed_error)?;
    let variants = manifest.variants.iter().filter(|variant| {
        variant.host_os.as_deref().unwrap_or(&variant.os) == std::env::consts::OS
            && variant.architecture == std::env::consts::ARCH
    });
    for variant in variants {
        if let Some(engine) = installed_engine(storage, &manifest, variant)? {
            return Ok(Some(engine));
        }
    }
    Ok(None)
}

fn prepare_tabby_session(
    model: &models::RegisteredModel,
    installed: &InstalledEngine,
    port: u16,
) -> Result<RuntimeLaunch, ServiceError> {
    let model_directory = model.path.parent().ok_or_else(model_unavailable_error)?;
    let model_name = model
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(model_unavailable_error)?;
    let model_directory = model_directory
        .to_str()
        .ok_or_else(model_unavailable_error)?;
    let session_directory = installed
        .root
        .join("sessions")
        .join(uuid::Uuid::new_v4().simple().to_string());
    let api_key = Zeroizing::new(uuid::Uuid::new_v4().simple().to_string());
    let admin_key = Zeroizing::new(uuid::Uuid::new_v4().simple().to_string());
    let configuration = json!({
        "network": {
            "host": "127.0.0.1",
            "port": port,
            "disable_auth": false,
            "allowed_origins": [],
            "disable_fetch_requests": true,
            "send_tracebacks": false,
            "api_servers": ["OAI"],
            "access_log": false,
        },
        "logging": {
            "log_prompt": false,
            "log_generation_parameters": false,
            "log_chat_completion_requests": false,
        },
        "model": {
            "model_dir": model_directory,
            "model_name": model_name,
            "backend": "exllamav3",
            "max_seq_len": -1,
        },
    });
    create_private_runtime_session(&session_directory)?;
    let write_result = (|| {
        write_private_runtime_file(
            &session_directory.join("config.yml"),
            &serde_json::to_vec(&configuration).map_err(|_| runtime_error())?,
        )?;
        let mut auth_file = serde_json::to_vec(&TabbyAuthFile {
            api_key: api_key.as_str(),
            admin_key: admin_key.as_str(),
        })
        .map_err(|_| runtime_error())?;
        let auth_result =
            write_private_runtime_file(&session_directory.join("api_tokens.yml"), &auth_file);
        auth_file.zeroize();
        auth_result?;
        Ok::<(), ServiceError>(())
    })();
    if let Err(error) = write_result {
        std::fs::remove_dir_all(&session_directory).map_err(|_| runtime_error())?;
        return Err(error);
    }

    let main_script = installed.root.join("tabbyAPI").join("main.py");
    let config_path = session_directory.join("config.yml");
    Ok(RuntimeLaunch {
        arguments: vec![
            main_script.into_os_string(),
            OsString::from("--config"),
            config_path.into_os_string(),
        ],
        current_directory: session_directory.clone(),
        use_wsl: false,
        initial_capabilities: None,
        provider_model_id: model_name.to_owned(),
        api_key: Some(api_key),
        session_directory: Some(session_directory),
    })
}

async fn prepare_vllm_session(
    model: &models::RegisteredModel,
    installed: &InstalledEngine,
    port: u16,
) -> Result<RuntimeLaunch, ServiceError> {
    let metadata = models::transformers_model_metadata(&model.path)
        .filter(|metadata| metadata.context_window.is_some())
        .ok_or_else(runtime_capability_error)?;
    let use_wsl = installed.variant_id.starts_with("windows-wsl2-");
    let model_path = wsl::path(&model.path, use_wsl)
        .await
        .map_err(|_| model_unavailable_error())?;
    let arguments = vllm_server_arguments(&model_path, &model.id, port);
    Ok(RuntimeLaunch {
        arguments,
        current_directory: installed.root.clone(),
        use_wsl,
        initial_capabilities: Some(RuntimeCapabilities {
            context_window: metadata
                .context_window
                .ok_or_else(runtime_capability_error)?,
            supports_images: metadata.supports_images,
            supports_tool_calls: None,
        }),
        provider_model_id: model.id.clone(),
        api_key: None,
        session_directory: None,
    })
}

fn vllm_server_arguments(model_path: &std::path::Path, model_id: &str, port: u16) -> Vec<OsString> {
    vec![
        OsString::from("serve"),
        model_path.as_os_str().to_owned(),
        OsString::from("--host"),
        OsString::from("127.0.0.1"),
        OsString::from("--port"),
        OsString::from(port.to_string()),
        OsString::from("--served-model-name"),
        OsString::from(model_id),
    ]
}

fn create_private_runtime_session(path: &std::path::Path) -> Result<(), ServiceError> {
    let parent = path.parent().ok_or_else(runtime_error)?;
    std::fs::create_dir_all(parent).map_err(|_| runtime_error())?;
    let parent_metadata = std::fs::symlink_metadata(parent).map_err(|_| runtime_error())?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(runtime_error());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(path).map_err(|_| runtime_error())?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir(path).map_err(|_| runtime_error())?;
    Ok(())
}

fn write_private_runtime_file(path: &std::path::Path, content: &[u8]) -> Result<(), ServiceError> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| runtime_error())?;
    use std::io::Write;
    file.write_all(content).map_err(|_| runtime_error())?;
    file.sync_all().map_err(|_| runtime_error())
}

async fn cleanup_session_directory(
    session_directory: Option<&std::path::Path>,
) -> Result<(), ServiceError> {
    if let Some(session_directory) = session_directory {
        let metadata = std::fs::symlink_metadata(session_directory).map_err(|_| runtime_error())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(runtime_error());
        }
        tokio::fs::remove_dir_all(session_directory)
            .await
            .map_err(|_| runtime_error())?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct TabbyAuthFile<'a> {
    api_key: &'a str,
    admin_key: &'a str,
}

fn running_state() -> &'static Mutex<Option<RunningServer>> {
    RUNNING_SERVER.get_or_init(|| Mutex::new(None))
}

async fn stop_server(
    storage: &AppStorage,
    state: &mut Option<RunningServer>,
) -> Result<(), ServiceError> {
    let Some(server) = state.as_mut() else {
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
    let Some(server) = state.take() else {
        return Err(runtime_error());
    };
    cleanup_session_directory(server.session_directory.as_deref()).await?;
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
        InstalledEngine, MAX_RUNTIME_PROPERTIES_BYTES, RuntimeCapabilities, health_check_or_cancel,
        llama_server_arguments, parse_runtime_capabilities, prepare_tabby_session,
        read_runtime_capabilities, start_model, vllm_server_arguments,
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
    fn tabby_session_is_authenticated_loopback_only_and_uses_model_folder_name() {
        let temporary = TestDirectory::new();
        let model_directory = temporary.0.join("models").join("exllama-model");
        let models_root = model_directory
            .parent()
            .expect("model directory should have a parent");
        fs::create_dir_all(models_root).expect("model parent should be created");
        fs::create_dir_all(&model_directory).expect("model directory should be created");
        let installed_root = temporary.0.join("engine");
        fs::create_dir_all(&installed_root).expect("engine directory should be created");

        let model = crate::local_engines::models::RegisteredModel {
            id: "local-model-id".to_owned(),
            engine_id: "exllama".to_owned(),
            display_name: "ExLlama model".to_owned(),
            path: model_directory.clone(),
            path_kind: "directory".to_owned(),
            created_at_unix_ms: 0,
        };
        let installed = InstalledEngine {
            engine_id: "exllama".to_owned(),
            variant_id: "windows-x86_64-cuda12.8-python3.12-torch2.9".to_owned(),
            release_tag: "v1.5.2".to_owned(),
            root: installed_root.clone(),
            entrypoint: installed_root.join("python/environment/.venv/Scripts/python.exe"),
        };

        let launch = prepare_tabby_session(&model, &installed, 49183)
            .expect("the TabbyAPI session should be created");
        let session_directory = launch
            .session_directory
            .as_ref()
            .expect("the session directory should be retained for cleanup");
        let configuration: serde_json::Value = serde_json::from_slice(
            &fs::read(session_directory.join("config.yml"))
                .expect("the private TabbyAPI configuration should exist"),
        )
        .expect("JSON configuration should also parse as YAML");
        assert_eq!(configuration["network"]["host"], "127.0.0.1");
        assert_eq!(configuration["network"]["port"], 49183);
        assert_eq!(configuration["network"]["disable_auth"], false);
        assert_eq!(
            configuration["network"]["allowed_origins"],
            serde_json::json!([])
        );
        assert_eq!(configuration["network"]["disable_fetch_requests"], true);
        assert_eq!(configuration["logging"]["log_prompt"], false);
        assert_eq!(configuration["model"]["model_name"], "exllama-model");
        assert_eq!(
            configuration["model"]["model_dir"],
            models_root.to_string_lossy().as_ref()
        );
        assert!(configuration.get("api_key").is_none());

        let auth: serde_json::Value = serde_json::from_slice(
            &fs::read(session_directory.join("api_tokens.yml"))
                .expect("the TabbyAPI auth file should exist"),
        )
        .expect("the TabbyAPI auth file should parse");
        let api_key = launch
            .api_key
            .as_deref()
            .map(String::as_str)
            .expect("the runtime should retain the generated API key");
        let file_api_key = auth["api_key"]
            .as_str()
            .expect("the auth file should include an API key");
        let admin_key = auth["admin_key"]
            .as_str()
            .expect("the auth file should include an admin key");
        assert!(file_api_key == api_key);
        assert!(!file_api_key.is_empty());
        assert!(!admin_key.is_empty());
        assert!(file_api_key != admin_key);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(session_directory)
                    .expect("the session metadata should be readable")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(session_directory.join("api_tokens.yml"))
                    .expect("the auth file metadata should be readable")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        assert_eq!(launch.provider_model_id, "exllama-model");
        assert_eq!(
            launch.current_directory.as_path(),
            session_directory.as_path()
        );
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

    #[test]
    fn vllm_arguments_use_the_local_model_and_loopback_only() {
        let arguments = vllm_server_arguments(
            std::path::Path::new("/mnt/d/models/local-transformer"),
            "registered-model-id",
            49183,
        );

        assert_eq!(arguments[0], "serve");
        assert_eq!(arguments[1], "/mnt/d/models/local-transformer");
        assert_eq!(arguments[2], "--host");
        assert_eq!(arguments[3], "127.0.0.1");
        assert_eq!(arguments[4], "--port");
        assert_eq!(arguments[5], "49183");
        assert_eq!(arguments[6], "--served-model-name");
        assert_eq!(arguments[7], "registered-model-id");
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
            tokio::spawn(
                async move { health_check_or_cancel(port, None, &mut cancellation).await },
            );
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
            read_runtime_capabilities(port, None)
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
