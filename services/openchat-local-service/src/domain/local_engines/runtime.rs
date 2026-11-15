//! Starts, stops, and inspects local model inference servers.
//!
//! Managed server processes are tracked separately from externally launched
//! servers. Lifecycle gates and cancellation state coordinate concurrent start
//! and stop requests, while health and model discovery use bounded HTTP calls.

pub(super) use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, OnceLock, Weak},
    time::Duration,
};

pub(super) use futures_util::future::join_all;
pub(super) use reqwest::Client;
pub(super) use serde_json::{Value, json};
pub(super) use sha2::{Digest, Sha256};
pub(super) use tokio::{
    io::AsyncReadExt,
    net::TcpListener,
    process::{Child, Command},
    sync::{Mutex, RwLock, watch},
    task::JoinHandle,
    time::{Instant, sleep},
};
pub(super) use zeroize::Zeroizing;

pub(super) use crate::{protocol::ServiceError, storage::AppStorage};

pub(super) use super::{
    external_server::{self, ExternalLlamaServerCandidate},
    installer::{InstalledEngine, installed_engine},
    manifests, models, settings,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(300);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_RUNTIME_PROPERTIES_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXTERNAL_MODEL_CATALOG_BYTES: usize = 1024 * 1024;
const MAX_EXTERNAL_MODEL_COUNT: usize = 128;
const MAX_RUNTIME_DIAGNOSTIC_BYTES: usize = 16 * 1024;

mod capabilities;
mod external;
mod support;

pub(crate) use capabilities::*;
pub(crate) use external::*;
use support::*;

static MANAGED_SERVERS: OnceLock<Mutex<HashMap<String, RunningServer>>> = OnceLock::new();
static RUNTIME_OPERATION_GATE: OnceLock<RwLock<()>> = OnceLock::new();
static MODEL_LIFECYCLES: OnceLock<Mutex<HashMap<String, Weak<ModelLifecycle>>>> = OnceLock::new();
static EXTERNAL_LLAMA_SERVERS: OnceLock<Mutex<HashMap<(u32, u16), ConnectedExternalLlamaServer>>> =
    OnceLock::new();
static EXTERNAL_SERVER_MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static HEALTH_CLIENT: OnceLock<Result<Client, reqwest::Error>> = OnceLock::new();
static PROPERTIES_CLIENT: OnceLock<Result<Client, reqwest::Error>> = OnceLock::new();
static EXTERNAL_CLIENT: OnceLock<Result<Client, reqwest::Error>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeCapabilities {
    pub(crate) context_window: Option<i64>,
    pub(crate) supports_images: bool,
    pub(crate) supports_tool_calls: Option<bool>,
}

pub(crate) struct RuntimeChatEndpoint {
    pub(crate) chat_url: String,
    pub(crate) model_id: String,
    pub(crate) capabilities: RuntimeCapabilities,
    pub(crate) api_key: Option<Zeroizing<String>>,
}

#[derive(Clone)]
struct ConnectedExternalLlamaServer {
    process_id: u32,
    port: u16,
    models: Vec<ExternalLlamaModel>,
}

#[derive(Clone)]
struct ExternalLlamaModel {
    route_id: String,
    provider_model_id: String,
    capabilities: RuntimeCapabilities,
}

struct RunningServer {
    engine_id: String,
    model_id: String,
    provider_model_id: String,
    port: u16,
    child: Child,
    stdout_diagnostics: Option<JoinHandle<Option<&'static str>>>,
    stderr_diagnostics: Option<JoinHandle<Option<&'static str>>>,
    capabilities: Option<RuntimeCapabilities>,
    api_key: Option<Zeroizing<String>>,
    auth_file: Option<PathBuf>,
    remote_pid_file: Option<PathBuf>,
    remote_process_marker: Option<PathBuf>,
}

struct ModelLifecycle {
    operation: Mutex<()>,
    stop_generation: watch::Sender<u64>,
}

struct RuntimeLaunch {
    arguments: Vec<std::ffi::OsString>,
    current_directory: PathBuf,
    initial_capabilities: Option<RuntimeCapabilities>,
    provider_model_id: String,
    api_key: Option<Zeroizing<String>>,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    auth_file: Option<PathBuf>,
    remote_pid_file: Option<PathBuf>,
    remote_process_marker: Option<PathBuf>,
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
    let lifecycle = model_lifecycle(model_id).await;
    let stop_generation = lifecycle.stop_generation.subscribe();
    let expected_generation = *stop_generation.borrow();
    let (mut operation_cancellation, cancellation_bridge) =
        bridge_cancellation_sources(cancellation, &stop_generation, expected_generation);
    let result = async {
        let _runtime_guard = tokio::select! {
            guard = runtime_operation_gate().read() => guard,
            _ = operation_cancellation.changed() => return Err(cancelled_error()),
        };
        if *operation_cancellation.borrow() {
            return Err(cancelled_error());
        }

        let _model_guard = tokio::select! {
            guard = lifecycle.operation.lock() => guard,
            _ = operation_cancellation.changed() => return Err(cancelled_error()),
        };
        if *operation_cancellation.borrow()
            || *lifecycle.stop_generation.subscribe().borrow() != expected_generation
        {
            return Err(cancelled_error());
        }

        if super::is_managed_provider_id(&model.engine_id) {
            let mut selected_server = managed_servers_state().lock().await.remove(model_id);
            let result = start_model_in_slot(
                storage,
                &model,
                installed,
                &mut selected_server,
                &mut operation_cancellation,
            )
            .await;
            if let Some(server) = selected_server {
                managed_servers_state()
                    .lock()
                    .await
                    .insert(model_id.to_owned(), server);
            }
            return result;
        }

        Err(model_unavailable_error())
    }
    .await;
    cancellation_bridge.abort();
    result
}

async fn start_model_in_slot(
    storage: &AppStorage,
    model: &models::RegisteredModel,
    installed: InstalledEngine,
    state: &mut Option<RunningServer>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }

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
        stop_server(storage, state).await?;
    }

    let engine_settings = settings::read_settings(storage.root())?;
    let preferred_port = match model.engine_id.as_str() {
        "vllm" => engine_settings.vllm_port,
        "exllama" => engine_settings.exllama_port,
        "sglang" => engine_settings.sglang_port,
        _ => None,
    };
    let port = available_loopback_port(preferred_port).await?;
    let started_at = Instant::now();
    let launch = match model.engine_id.as_str() {
        "llama_cpp" => RuntimeLaunch {
            arguments: llama_server_arguments(storage.root(), model, port)?,
            current_directory: installed.root.clone(),
            initial_capabilities: None,
            provider_model_id: model.id.clone(),
            api_key: Some(Zeroizing::new(uuid::Uuid::new_v4().to_string())),
            environment: Vec::new(),
            auth_file: None,
            remote_pid_file: None,
            remote_process_marker: None,
        },
        "vllm" => vllm_server_launch(storage, model, port).await?,
        "exllama" | "sglang" => python_engine_server_launch(storage, model, port).await?,
        _ => return Err(model_unavailable_error()),
    };
    let mut command = Command::new(&installed.entrypoint);
    let capture_output = matches!(model.engine_id.as_str(), "exllama" | "sglang");
    if let Some(api_key) = launch.api_key.as_ref() {
        if model.engine_id == "llama_cpp" {
            configure_llama_server_api_key(&mut command, api_key);
        }
    }
    command.envs(launch.environment.iter().map(|(key, value)| (key, value)));
    let mut child = match command
        .args(&launch.arguments)
        .current_dir(&launch.current_directory)
        .stdin(Stdio::null())
        .stdout(if capture_output {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(if capture_output {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            if model.engine_id == "vllm"
                && let Some(auth_file) = launch.auth_file.as_deref()
            {
                super::vllm::remove_auth_file(storage, auth_file).await?;
            }
            if matches!(model.engine_id.as_str(), "exllama" | "sglang")
                && let Some(auth_file) = launch.auth_file.as_deref()
            {
                super::python_engines::remove_session(storage, &model.engine_id, auth_file).await?;
            }
            log_runtime_event(
                storage,
                "runtime_start_failed",
                Some("local_engine_runtime_unavailable"),
                Some(started_at.elapsed()),
            );
            return Err(runtime_error());
        }
    };
    let stdout_diagnostics = child
        .stdout
        .take()
        .map(|stdout| tokio::spawn(async move { runtime_stream_failure_hint(stdout).await }));
    let stderr_diagnostics = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(async move { runtime_stream_failure_hint(stderr).await }));
    *state = Some(RunningServer {
        engine_id: model.engine_id.clone(),
        model_id: model.id.clone(),
        provider_model_id: launch.provider_model_id,
        port,
        child,
        stdout_diagnostics,
        stderr_diagnostics,
        capabilities: launch.initial_capabilities,
        api_key: launch.api_key,
        auth_file: launch.auth_file,
        remote_pid_file: launch.remote_pid_file,
        remote_process_marker: launch.remote_process_marker,
    });

    loop {
        let (port, has_exited) = {
            let Some(server) = state.as_mut() else {
                return Err(runtime_error());
            };
            let has_exited = server
                .child
                .try_wait()
                .map_err(|_| runtime_error())?
                .is_some();
            (server.port, has_exited)
        };
        if has_exited {
            let mut exited = state.take().ok_or_else(runtime_error)?;
            cleanup_exited_server(storage, &exited).await?;
            let stderr_failure_code = match exited.stderr_diagnostics.take() {
                Some(task) => task.await.ok().flatten(),
                None => None,
            };
            let stdout_failure_code = match exited.stdout_diagnostics.take() {
                Some(task) => task.await.ok().flatten(),
                None => None,
            };
            let failure_code = runtime_failure_code(stderr_failure_code, stdout_failure_code)
                .unwrap_or("local_engine_start_failed");
            log_runtime_event(
                storage,
                "runtime_start_failed",
                Some(failure_code),
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
                stop_server(storage, state).await?;
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
                None => {
                    runtime_capabilities_or_cancel(&model.engine_id, port, api_key, cancellation)
                        .await
                }
            };
            let capabilities = match capabilities_result {
                Ok(capabilities) => capabilities,
                Err(error) => {
                    if error.code == "operation_cancelled" {
                        stop_server(storage, state).await?;
                        log_runtime_event(
                            storage,
                            "runtime_capability_check_cancelled",
                            Some(error.code),
                            Some(started_at.elapsed()),
                        );
                        return Err(error);
                    }
                    log_runtime_event(
                        storage,
                        "runtime_capability_check_unavailable",
                        Some(error.code),
                        Some(started_at.elapsed()),
                    );
                    RuntimeCapabilities {
                        context_window: None,
                        supports_images: false,
                        supports_tool_calls: None,
                    }
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
            stop_server(storage, state).await?;
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
                    stop_server(storage, state).await?;
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

async fn runtime_stream_failure_hint<R>(mut output_stream: R) -> Option<&'static str>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut output = Vec::with_capacity(MAX_RUNTIME_DIAGNOSTIC_BYTES);
    let mut buffer = [0_u8; 4096];
    loop {
        let read = output_stream.read(&mut buffer).await.ok()?;
        if read == 0 {
            break;
        }
        output.extend_from_slice(&buffer[..read]);
        if output.len() > MAX_RUNTIME_DIAGNOSTIC_BYTES {
            let excess = output.len() - MAX_RUNTIME_DIAGNOSTIC_BYTES;
            output.drain(..excess);
        }
    }
    classify_runtime_stderr(&String::from_utf8_lossy(&output))
}

fn classify_runtime_stderr(stderr: &str) -> Option<&'static str> {
    let stderr = stderr.to_ascii_lowercase();
    if stderr.trim().is_empty() {
        return None;
    }
    if stderr.contains("out of memory") {
        Some("gpu_memory_exhausted")
    } else if (stderr.contains("nvcc")
        && (stderr.contains("not found")
            || stderr.contains("could not find")
            || stderr.contains("does not exist")
            || stderr.contains("doesn't exist")))
        || (stderr.contains("cuda_home") && stderr.contains("doesn't exist"))
    {
        Some("cuda_compiler_unavailable")
    } else if stderr.contains("no module named") || stderr.contains("modulenotfounderror") {
        Some("runtime_dependency_missing")
    } else if stderr.contains("unrecognized arguments") || stderr.contains("unknown option") {
        Some("engine_arguments_rejected")
    } else if stderr.contains("unsupported gpu architecture")
        || stderr.contains("no kernel image is available")
        || stderr.contains("invalid device function")
    {
        Some("gpu_architecture_unsupported")
    } else if stderr.contains("cuda driver version is insufficient")
        || stderr.contains("driver version is too old")
    {
        Some("cuda_driver_unsupported")
    } else if stderr.contains("cuda driver initialization failed")
        || stderr.contains("failed to initialize cuda driver")
    {
        Some("cuda_driver_unavailable")
    } else if stderr.contains("no such file or directory") || stderr.contains("filenotfounderror") {
        Some("model_files_unavailable")
    } else if stderr.contains("valueerror") {
        Some("python_value_error")
    } else if stderr.contains("runtimeerror") {
        Some("python_runtime_error")
    } else if stderr.contains("typeerror") {
        Some("python_type_error")
    } else if stderr.contains("keyerror") {
        Some("python_key_error")
    } else if stderr.contains("assertionerror") {
        Some("python_assertion_error")
    } else if stderr.contains("traceback (most recent call last):") {
        Some("python_traceback_unclassified")
    } else {
        Some("runtime_output_unclassified")
    }
}

fn runtime_failure_code(
    stderr: Option<&'static str>,
    stdout: Option<&'static str>,
) -> Option<&'static str> {
    [stderr, stdout]
        .into_iter()
        .flatten()
        .find(|code| *code != "runtime_output_unclassified")
        .or(stderr)
        .or(stdout)
}

async fn cleanup_exited_server(
    storage: &AppStorage,
    server: &RunningServer,
) -> Result<(), ServiceError> {
    if server.engine_id == "vllm" {
        if let Some(pid_file) = server.remote_pid_file.as_deref() {
            super::vllm::stop_wsl_process(storage, pid_file).await?;
        }
        if let Some(auth_file) = server.auth_file.as_deref() {
            super::vllm::remove_auth_file(storage, auth_file).await?;
        }
    } else if matches!(server.engine_id.as_str(), "exllama" | "sglang") {
        if let (Some(pid_file), Some(marker)) = (
            server.remote_pid_file.as_deref(),
            server.remote_process_marker.as_deref(),
        ) {
            super::python_engines::stop_wsl_process(storage, &server.engine_id, pid_file, marker)
                .await?;
        }
        if let Some(auth_file) = server.auth_file.as_deref() {
            super::python_engines::remove_session(storage, &server.engine_id, auth_file).await?;
        }
    }
    Ok(())
}

pub(crate) async fn stop(storage: &AppStorage) -> Result<Value, ServiceError> {
    let lifecycles = model_lifecycles().lock().await;
    for lifecycle in lifecycles.values().filter_map(Weak::upgrade) {
        advance_stop_generation(&lifecycle.stop_generation);
    }
    let runtime_guard = runtime_operation_gate().write().await;
    drop(lifecycles);

    let mut first_error = None;
    let model_ids = managed_servers_state()
        .lock()
        .await
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for model_id in model_ids {
        let Some(server) = managed_servers_state().lock().await.remove(&model_id) else {
            continue;
        };
        let mut slot = Some(server);
        if let Err(error) = stop_server(storage, &mut slot).await {
            first_error.get_or_insert(error);
        }
        if let Some(server) = slot {
            managed_servers_state()
                .lock()
                .await
                .insert(model_id, server);
        }
    }
    drop(runtime_guard);
    if let Some(error) = first_error {
        return Err(error);
    }
    status(storage).await
}

pub(crate) async fn stop_model(
    storage: &AppStorage,
    model_id: &str,
) -> Result<Value, ServiceError> {
    let mut lifecycles = model_lifecycles().lock().await;
    let lifecycle = model_lifecycle_locked(model_id, &mut lifecycles);
    advance_stop_generation(&lifecycle.stop_generation);
    let runtime_guard = runtime_operation_gate().write().await;
    drop(lifecycles);
    let _model_guard = lifecycle.operation.lock().await;

    let managed_server = managed_servers_state().lock().await.remove(model_id);
    if let Some(server) = managed_server {
        let mut slot = Some(server);
        let result = stop_server(storage, &mut slot).await;
        if let Some(server) = slot {
            managed_servers_state()
                .lock()
                .await
                .insert(model_id.to_owned(), server);
        }
        result?;
    }
    drop(runtime_guard);
    status(storage).await
}

pub(crate) async fn stop_engine(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<Value, ServiceError> {
    let mut lifecycles = model_lifecycles().lock().await;
    let model_ids = managed_servers_state()
        .lock()
        .await
        .iter()
        .filter(|(_, server)| server.engine_id == engine_id)
        .map(|(model_id, _)| model_id.clone())
        .collect::<Vec<_>>();
    for model_id in &model_ids {
        let lifecycle = model_lifecycle_locked(model_id, &mut lifecycles);
        advance_stop_generation(&lifecycle.stop_generation);
    }
    let runtime_guard = runtime_operation_gate().write().await;
    drop(lifecycles);

    let mut first_error = None;
    for model_id in model_ids {
        let Some(server) = managed_servers_state().lock().await.remove(&model_id) else {
            continue;
        };
        let mut slot = Some(server);
        if let Err(error) = stop_server(storage, &mut slot).await {
            first_error.get_or_insert(error);
        }
        if let Some(server) = slot {
            managed_servers_state()
                .lock()
                .await
                .insert(model_id, server);
        }
    }
    drop(runtime_guard);
    if let Some(error) = first_error {
        return Err(error);
    }
    status(storage).await
}

pub(crate) async fn status(storage: &AppStorage) -> Result<Value, ServiceError> {
    let mut servers = Vec::new();
    let mut stale_servers = Vec::new();
    let managed_snapshots = {
        let mut managed = managed_servers_state().lock().await;
        let model_ids = managed.keys().cloned().collect::<Vec<_>>();
        let mut snapshots = Vec::new();
        for model_id in model_ids {
            let has_exited = managed
                .get_mut(&model_id)
                .ok_or_else(runtime_error)?
                .child
                .try_wait()
                .map_err(|_| runtime_error())?
                .is_some();
            if has_exited {
                let exited = managed.remove(&model_id).ok_or_else(runtime_error)?;
                stale_servers.push(exited);
                log_runtime_event(
                    storage,
                    "runtime_exited",
                    Some("local_engine_runtime_unavailable"),
                    None,
                );
                continue;
            }
            let server = managed.get(&model_id).ok_or_else(runtime_error)?;
            if let Some(process_id) = server.child.id() {
                snapshots.push((model_id, process_id, server.port, server.api_key.clone()));
            }
        }
        snapshots
    };
    for server in stale_servers {
        cleanup_exited_server(storage, &server).await?;
    }
    let health = join_all(
        managed_snapshots
            .iter()
            .map(|(_, _, port, api_key)| async move {
                health_check(*port, api_key.as_ref().map(|key| key.as_str())).await
            }),
    )
    .await;
    {
        let managed = managed_servers_state().lock().await;
        for ((model_id, process_id, port, _), healthy) in managed_snapshots.iter().zip(health) {
            let Some(server) = managed
                .get(model_id)
                .filter(|server| server.child.id() == Some(*process_id) && server.port == *port)
            else {
                continue;
            };
            servers.push(server_status_json(
                server,
                if healthy { "running" } else { "unhealthy" },
            ));
        }
    }
    servers.sort_by(|left, right| left["modelId"].as_str().cmp(&right["modelId"].as_str()));
    let status = if servers.iter().any(|server| server["status"] == "running") {
        "running"
    } else if !servers.is_empty() {
        "unhealthy"
    } else {
        "stopped"
    };
    Ok(runtime_status_json(servers, status))
}

#[cfg(test)]
mod tests;
