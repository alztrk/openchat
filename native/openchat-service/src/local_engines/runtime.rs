use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    net::SocketAddr,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, OnceLock, Weak},
    time::Duration,
};

use futures_util::future::join_all;
use reqwest::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    net::TcpListener,
    process::{Child, Command},
    sync::{Mutex, RwLock, watch},
    time::{Instant, sleep},
};
use zeroize::{Zeroize, Zeroizing};

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    external_server::{self, ExternalLlamaServerCandidate},
    installer::{InstalledEngine, installed_engine},
    manifests, models, settings, wsl,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(300);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_RUNTIME_PROPERTIES_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXTERNAL_MODEL_CATALOG_BYTES: usize = 1024 * 1024;
const MAX_EXTERNAL_MODEL_COUNT: usize = 128;

static RUNNING_SERVER: OnceLock<Mutex<Option<RunningServer>>> = OnceLock::new();
static RUNNING_SERVER_OPERATION: OnceLock<Mutex<()>> = OnceLock::new();
static RUNNING_LLAMA_SERVERS: OnceLock<Mutex<HashMap<String, RunningServer>>> = OnceLock::new();
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
    capabilities: Option<RuntimeCapabilities>,
    api_key: Option<Zeroizing<String>>,
    session_directory: Option<PathBuf>,
}

struct ModelLifecycle {
    operation: Mutex<()>,
    stop_generation: watch::Sender<u64>,
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

        if model.engine_id == "llama_cpp" {
            let mut selected_server = running_llama_state().lock().await.remove(model_id);
            let result = start_model_in_slot(
                storage,
                &model,
                installed,
                &mut selected_server,
                &mut operation_cancellation,
            )
            .await;
            if let Some(server) = selected_server {
                running_llama_state()
                    .lock()
                    .await
                    .insert(model_id.to_owned(), server);
            }
            return result;
        }

        let _slot_guard = running_server_operation().lock().await;
        let mut state = running_state().lock().await.take();
        let result = start_model_in_slot(
            storage,
            &model,
            installed,
            &mut state,
            &mut operation_cancellation,
        )
        .await;
        *running_state().lock().await = state;
        result
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

    let port = available_loopback_port().await?;
    let started_at = Instant::now();
    let launch = match model.engine_id.as_str() {
        "llama_cpp" => RuntimeLaunch {
            arguments: llama_server_arguments(model, port)?,
            current_directory: installed.root.clone(),
            use_wsl: false,
            initial_capabilities: None,
            provider_model_id: model.id.clone(),
            api_key: None,
            session_directory: None,
        },
        "exllama" => prepare_tabby_session(model, &installed, port)?,
        "vllm" => prepare_vllm_session(model, &installed, port).await?,
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
                None => runtime_capabilities_or_cancel(port, api_key, cancellation).await,
            };
            let capabilities = match capabilities_result {
                Ok(capabilities) => capabilities,
                Err(error) => {
                    stop_server(storage, state).await?;
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

pub(crate) async fn stop(storage: &AppStorage) -> Result<Value, ServiceError> {
    let lifecycles = model_lifecycles().lock().await;
    for lifecycle in lifecycles.values().filter_map(Weak::upgrade) {
        advance_stop_generation(&lifecycle.stop_generation);
    }
    let runtime_guard = runtime_operation_gate().write().await;
    drop(lifecycles);

    let mut first_error = None;
    {
        let _slot_guard = running_server_operation().lock().await;
        let mut state = running_state().lock().await;
        if let Err(error) = stop_server(storage, &mut state).await {
            first_error = Some(error);
        }
    }
    let model_ids = running_llama_state()
        .lock()
        .await
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for model_id in model_ids {
        let Some(server) = running_llama_state().lock().await.remove(&model_id) else {
            continue;
        };
        let mut slot = Some(server);
        if let Err(error) = stop_server(storage, &mut slot).await {
            first_error.get_or_insert(error);
        }
        if let Some(server) = slot {
            running_llama_state().lock().await.insert(model_id, server);
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

    let managed_server = running_llama_state().lock().await.remove(model_id);
    if let Some(server) = managed_server {
        let mut slot = Some(server);
        let result = stop_server(storage, &mut slot).await;
        if let Some(server) = slot {
            running_llama_state()
                .lock()
                .await
                .insert(model_id.to_owned(), server);
        }
        result?;
    }
    {
        let _slot_guard = running_server_operation().lock().await;
        let mut state = running_state().lock().await;
        if state
            .as_ref()
            .is_some_and(|server| server.model_id == model_id)
        {
            stop_server(storage, &mut state).await?;
        }
    }
    drop(runtime_guard);
    status(storage).await
}

pub(crate) async fn status(storage: &AppStorage) -> Result<Value, ServiceError> {
    let mut servers = Vec::new();
    let mut legacy_status = None;
    {
        let mut state = running_state().lock().await;
        if let Some(server) = state.as_mut() {
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
            } else if let Some(server) = state.as_ref() {
                let status =
                    if health_check(server.port, server.api_key.as_deref().map(String::as_str))
                        .await
                    {
                        "running"
                    } else {
                        "unhealthy"
                    };
                legacy_status = Some(status);
                servers.push(server_status_json(server, status));
            }
        }
    }
    let managed_snapshots = {
        let mut managed = running_llama_state().lock().await;
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
                let mut server = managed.remove(&model_id).ok_or_else(runtime_error)?;
                let session_directory = server.session_directory.take();
                cleanup_session_directory(session_directory.as_deref()).await?;
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
                snapshots.push((model_id, process_id, server.port));
            }
        }
        snapshots
    };
    let health = join_all(
        managed_snapshots
            .iter()
            .map(|(_, _, port)| async move { health_check(*port, None).await }),
    )
    .await;
    {
        let managed = running_llama_state().lock().await;
        for ((model_id, process_id, port), healthy) in managed_snapshots.iter().zip(health) {
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
        legacy_status.unwrap_or("stopped")
    };
    Ok(runtime_status_json(servers, status))
}

pub(crate) async fn external_server_status() -> Value {
    let state = external_llama_servers_state().lock().await;
    let mut connected = state.iter().collect::<Vec<_>>();
    connected.sort_by_key(|(identity, _)| (identity.1, identity.0));
    Value::Array(
        connected
            .into_iter()
            .map(|(_, server)| external_server_status_json(server))
            .collect(),
    )
}

pub(crate) async fn find_external_server_candidates()
-> Result<Vec<ExternalLlamaServerCandidate>, ServiceError> {
    let candidates = external_server::find_running_servers().await?;
    let managed_process_ids = managed_server_process_ids().await;
    let candidates = excluding_managed_server_processes(candidates, &managed_process_ids);
    reconcile_external_server_candidates(&candidates).await;
    Ok(candidates)
}

fn excluding_managed_server_processes(
    candidates: Vec<ExternalLlamaServerCandidate>,
    managed_process_ids: &HashSet<u32>,
) -> Vec<ExternalLlamaServerCandidate> {
    candidates
        .into_iter()
        .filter(|candidate| !managed_process_ids.contains(&candidate.process_id))
        .collect()
}

async fn managed_server_process_ids() -> HashSet<u32> {
    let mut process_ids = HashSet::new();
    if let Some(process_id) = running_state()
        .lock()
        .await
        .as_ref()
        .filter(|server| server.engine_id == "llama_cpp")
        .and_then(|server| server.child.id())
    {
        process_ids.insert(process_id);
    }
    process_ids.extend(
        running_llama_state()
            .lock()
            .await
            .values()
            .filter_map(|server| server.child.id()),
    );
    process_ids
}

async fn candidate_is_external(
    candidate: ExternalLlamaServerCandidate,
) -> Result<bool, ServiceError> {
    if managed_server_process_ids()
        .await
        .contains(&candidate.process_id)
    {
        return Ok(false);
    }
    external_server::candidate_is_running(candidate).await
}

pub(crate) async fn reconcile_external_server_candidates(
    candidates: &[ExternalLlamaServerCandidate],
) {
    let _mutation = external_server_mutation_lock().lock().await;
    let mut state = external_llama_servers_state().lock().await;
    retain_detected_external_servers(&mut state, candidates);
}

pub(crate) async fn connect_external_server(
    process_id: u32,
    port: u16,
) -> Result<Value, ServiceError> {
    let _mutation = external_server_mutation_lock().lock().await;
    let candidate = ExternalLlamaServerCandidate { process_id, port };
    if !candidate_is_external(candidate).await? {
        return Err(external_server_not_found_error());
    }
    if !external_health_check(port).await? {
        return Err(external_server_not_ready_error());
    }
    let model_ids = fetch_external_model_ids(port).await?;

    let capabilities = read_external_capabilities(port).await?;
    if !candidate_is_external(candidate).await? {
        return Err(external_server_not_found_error());
    }

    let models = model_ids
        .into_iter()
        .map(|provider_model_id| ExternalLlamaModel {
            route_id: external_model_route_id(process_id, port, &provider_model_id),
            provider_model_id,
            capabilities,
        })
        .collect();
    let server = ConnectedExternalLlamaServer {
        process_id,
        port,
        models,
    };
    let response = external_server_status_json(&server);
    external_llama_servers_state()
        .lock()
        .await
        .insert((process_id, port), server);
    Ok(response)
}

pub(crate) async fn disconnect_external_server(process_id: u32, port: u16) -> Value {
    let _mutation = external_server_mutation_lock().lock().await;
    let mut servers = external_llama_servers_state().lock().await;
    remove_external_server(&mut servers, process_id, port);
    json!({"disconnected": true, "processId": process_id, "port": port})
}

pub(crate) async fn chat_model_catalog(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<Value, ServiceError> {
    if !["llama_cpp", "vllm", "exllama"].contains(&engine_id) {
        return Err(ServiceError::new(
            "local_engine_unknown",
            "The selected local inference engine is not available.",
            false,
        ));
    }

    let managed_model_groups = if engine_id == "llama_cpp" {
        status(storage)
            .await?
            .get("servers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|server| server["engineId"] == "llama_cpp" && server["status"] == "running")
            .filter_map(|server| {
                let model_id = server["modelId"].as_str()?.to_owned();
                let process_id = u32::try_from(server["processId"].as_u64()?).ok()?;
                let port = u16::try_from(server["port"].as_u64()?).ok()?;
                Some((model_id, managed_server_group_id(process_id, port)))
            })
            .collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    let mut catalog_models = models::list(storage)?
        .into_iter()
        .filter(|model| model.engine_id == engine_id)
        .map(|model| {
            let available = is_model_available(storage, &model)?;
            let mut model_json = models::to_json(&model, available)?;
            if engine_id == "llama_cpp" {
                let group_id = managed_model_groups
                    .get(&model.id)
                    .cloned()
                    .unwrap_or_else(|| "managed".to_owned());
                model_json["groupId"] = Value::String(group_id);
            }
            Ok(model_json)
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;
    if engine_id == "llama_cpp" {
        catalog_models.extend(external_chat_models().await?);
    }
    Ok(json!({"freshness": "current", "models": catalog_models}))
}

pub(crate) async fn chat_url(
    storage: &AppStorage,
    model_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<RuntimeChatEndpoint, ServiceError> {
    if let Some(endpoint) = external_chat_endpoint(model_id, cancellation).await? {
        return Ok(endpoint);
    }
    let status = start_model(storage, model_id, cancellation).await?;
    let port = status
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(runtime_error)?;
    let model = models::find(storage, model_id)?.ok_or_else(model_unavailable_error)?;
    let state = if model.engine_id == "llama_cpp" {
        let state = running_llama_state().lock().await;
        let server = state
            .get(model_id)
            .filter(|server| server.port == port)
            .ok_or_else(runtime_error)?;
        let capabilities = server.capabilities.ok_or_else(runtime_capability_error)?;
        return Ok(RuntimeChatEndpoint {
            chat_url: format!("http://127.0.0.1:{port}/v1/chat/completions"),
            model_id: server.provider_model_id.clone(),
            capabilities,
            api_key: server.api_key.clone(),
        });
    } else {
        running_state().lock().await
    };
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

async fn external_chat_models() -> Result<Vec<Value>, ServiceError> {
    let servers = external_llama_servers_state()
        .lock()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let mut models = Vec::new();
    for server in servers {
        let candidate = ExternalLlamaServerCandidate {
            process_id: server.process_id,
            port: server.port,
        };
        if !candidate_is_external(candidate).await? {
            clear_external_server_if_current(candidate).await?;
            continue;
        }
        models.extend(
            server
                .models
                .iter()
                .map(|model| external_model_json(model, server.process_id, server.port)),
        );
    }
    Ok(models)
}

async fn external_chat_endpoint(
    route_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Option<RuntimeChatEndpoint>, ServiceError> {
    let server = {
        let servers = external_llama_servers_state().lock().await;
        external_server_for_route(&servers, route_id).cloned()
    };
    let Some(server) = server else {
        return Ok(None);
    };
    let candidate = ExternalLlamaServerCandidate {
        process_id: server.process_id,
        port: server.port,
    };
    if !candidate_is_external(candidate).await? {
        clear_external_server_if_current(candidate).await?;
        return Err(external_server_not_found_error());
    }
    if !external_health_check_or_cancel(server.port, cancellation).await? {
        clear_external_server_if_current(candidate).await?;
        return Err(external_server_not_ready_error());
    }

    let current = external_llama_servers_state().lock().await;
    let Some(current) = current.get(&(server.process_id, server.port)) else {
        return Err(external_server_not_found_error());
    };
    let Some(model) = current
        .models
        .iter()
        .find(|model| model.route_id == route_id)
    else {
        return Err(external_server_not_found_error());
    };
    Ok(Some(RuntimeChatEndpoint {
        chat_url: format!("http://127.0.0.1:{}/v1/chat/completions", server.port),
        model_id: model.provider_model_id.clone(),
        capabilities: model.capabilities,
        api_key: None,
    }))
}

async fn clear_external_server_if_current(
    candidate: ExternalLlamaServerCandidate,
) -> Result<(), ServiceError> {
    let _mutation = external_server_mutation_lock().lock().await;
    if candidate_is_external(candidate).await? {
        return Ok(());
    }
    let mut servers = external_llama_servers_state().lock().await;
    remove_external_server(&mut servers, candidate.process_id, candidate.port);
    Ok(())
}

fn external_model_json(model: &ExternalLlamaModel, process_id: u32, port: u16) -> Value {
    json!({
        "id": model.route_id,
        "engineId": "llama_cpp",
        "groupId": external_server_group_id(process_id, port),
        "displayName": model.provider_model_id,
        "description": Value::Null,
        "contextWindow": model.capabilities.context_window,
        "reasoningLevels": [],
        "supportsReasoning": false,
        "supportsImages": model.capabilities.supports_images,
        "supportsTools": model.capabilities.supports_tool_calls,
        "defaultReasoningLevel": Value::Null,
        "isAvailable": true,
        "reason": Value::Null,
    })
}

fn external_server_group_id(process_id: u32, port: u16) -> String {
    format!("external-llama-server:{process_id}:{port}")
}

fn external_model_route_id(process_id: u32, port: u16, provider_model_id: &str) -> String {
    let identity = format!("{process_id}:{port}:{provider_model_id}");
    let digest = Sha256::digest(identity.as_bytes());
    let digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("external-llama-{digest}")
}

async fn external_health_check(port: u16) -> Result<bool, ServiceError> {
    let response = external_http_client()?
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .map_err(|_| external_server_not_ready_error())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(external_server_auth_error());
    }
    Ok(response.status().is_success())
}

async fn external_health_check_or_cancel(
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
        ready = external_health_check(port) => ready,
    }
}

async fn fetch_external_model_ids(port: u16) -> Result<Vec<String>, ServiceError> {
    let response = external_http_client()?
        .get(format!("http://127.0.0.1:{port}/v1/models"))
        .send()
        .await
        .map_err(|_| external_server_catalog_error())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(external_server_auth_error());
    }
    if !response.status().is_success() {
        return Err(external_server_catalog_error());
    }
    let response = read_bounded_json(
        response,
        MAX_EXTERNAL_MODEL_CATALOG_BYTES,
        external_server_catalog_error,
    )
    .await?;
    parse_external_model_ids(&response)
}

fn parse_external_model_ids(response: &Value) -> Result<Vec<String>, ServiceError> {
    let raw_models = response
        .get("data")
        .and_then(Value::as_array)
        .filter(|models| !models.is_empty() && models.len() <= MAX_EXTERNAL_MODEL_COUNT)
        .ok_or_else(external_server_catalog_error)?;
    let mut known_ids = HashSet::new();
    let mut model_ids = Vec::with_capacity(raw_models.len());
    for model in raw_models {
        let model_id = model
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| {
                !id.trim().is_empty()
                    && id.chars().count() <= 512
                    && !id.chars().any(char::is_control)
            })
            .ok_or_else(external_server_catalog_error)?;
        if known_ids.insert(model_id.to_owned()) {
            model_ids.push(model_id.to_owned());
        }
    }
    if model_ids.is_empty() {
        return Err(external_server_catalog_error());
    }
    Ok(model_ids)
}

async fn read_external_capabilities(port: u16) -> Result<RuntimeCapabilities, ServiceError> {
    let response = external_http_client()?
        .get(format!("http://127.0.0.1:{port}/props"))
        .send()
        .await
        .map_err(|_| external_server_capability_error())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(RuntimeCapabilities {
            context_window: None,
            supports_images: false,
            supports_tool_calls: None,
        });
    }
    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Ok(RuntimeCapabilities {
            context_window: None,
            supports_images: false,
            supports_tool_calls: None,
        });
    }
    if !response.status().is_success() {
        return Err(external_server_capability_error());
    }
    let properties = read_bounded_json(
        response,
        MAX_RUNTIME_PROPERTIES_BYTES,
        external_server_capability_error,
    )
    .await?;
    Ok(parse_external_capabilities(&properties))
}

fn parse_external_capabilities(properties: &Value) -> RuntimeCapabilities {
    let context_window = properties
        .pointer("/default_generation_settings/n_ctx")
        .and_then(Value::as_i64)
        .filter(|context_window| *context_window > 0);
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
    RuntimeCapabilities {
        context_window,
        supports_images,
        supports_tool_calls,
    }
}

async fn read_bounded_json(
    mut response: reqwest::Response,
    maximum_bytes: usize,
    error: fn() -> ServiceError,
) -> Result<Value, ServiceError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(error());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| error())? {
        if body.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(error());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| error())
}

fn external_http_client() -> Result<&'static Client, ServiceError> {
    EXTERNAL_CLIENT
        .get_or_init(|| {
            Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(5))
                .build()
        })
        .as_ref()
        .map_err(|_| external_server_not_ready_error())
}

fn external_llama_servers_state()
-> &'static Mutex<HashMap<(u32, u16), ConnectedExternalLlamaServer>> {
    EXTERNAL_LLAMA_SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn external_server_mutation_lock() -> &'static Mutex<()> {
    EXTERNAL_SERVER_MUTATION_LOCK.get_or_init(|| Mutex::new(()))
}

fn external_server_status_json(server: &ConnectedExternalLlamaServer) -> Value {
    json!({
        "processId": server.process_id,
        "port": server.port,
        "modelIds": server.models.iter().map(|model| model.provider_model_id.as_str()).collect::<Vec<_>>(),
    })
}

fn external_server_for_route<'a>(
    servers: &'a HashMap<(u32, u16), ConnectedExternalLlamaServer>,
    route_id: &str,
) -> Option<&'a ConnectedExternalLlamaServer> {
    servers
        .values()
        .find(|server| server.models.iter().any(|model| model.route_id == route_id))
}

fn retain_detected_external_servers(
    servers: &mut HashMap<(u32, u16), ConnectedExternalLlamaServer>,
    candidates: &[ExternalLlamaServerCandidate],
) {
    servers.retain(|_, server| {
        candidates.contains(&ExternalLlamaServerCandidate {
            process_id: server.process_id,
            port: server.port,
        })
    });
}

fn remove_external_server(
    servers: &mut HashMap<(u32, u16), ConnectedExternalLlamaServer>,
    process_id: u32,
    port: u16,
) -> bool {
    servers.remove(&(process_id, port)).is_some()
}

fn managed_server_group_id(process_id: u32, port: u16) -> String {
    format!("managed-llama-server:{process_id}:{port}")
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
        context_window: Some(context_window),
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
    if engine_id == "llama_cpp"
        && let Some(configured_path) = settings::llama_server_executable_path(storage.root())?
    {
        let Some(entrypoint) = settings::validate_saved_executable_path(&configured_path) else {
            return Ok(None);
        };
        let root = entrypoint
            .parent()
            .ok_or_else(engine_not_installed_error)?
            .to_path_buf();
        return Ok(Some(InstalledEngine {
            engine_id: engine_id.to_owned(),
            variant_id: "user-configured".to_owned(),
            release_tag: "user-configured".to_owned(),
            root,
            entrypoint,
        }));
    }

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
            context_window: Some(
                metadata
                    .context_window
                    .ok_or_else(runtime_capability_error)?,
            ),
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

fn running_server_operation() -> &'static Mutex<()> {
    RUNNING_SERVER_OPERATION.get_or_init(|| Mutex::new(()))
}

fn running_llama_state() -> &'static Mutex<HashMap<String, RunningServer>> {
    RUNNING_LLAMA_SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn runtime_operation_gate() -> &'static RwLock<()> {
    RUNTIME_OPERATION_GATE.get_or_init(|| RwLock::new(()))
}

fn model_lifecycles() -> &'static Mutex<HashMap<String, Weak<ModelLifecycle>>> {
    MODEL_LIFECYCLES.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn model_lifecycle(model_id: &str) -> Arc<ModelLifecycle> {
    let mut lifecycles = model_lifecycles().lock().await;
    model_lifecycle_locked(model_id, &mut lifecycles)
}

fn model_lifecycle_locked(
    model_id: &str,
    lifecycles: &mut HashMap<String, Weak<ModelLifecycle>>,
) -> Arc<ModelLifecycle> {
    if lifecycles.len() > 128 {
        lifecycles.retain(|_, lifecycle| lifecycle.strong_count() > 0);
    }
    if let Some(lifecycle) = lifecycles.get(model_id).and_then(Weak::upgrade) {
        return lifecycle;
    }
    let (stop_generation, _receiver) = watch::channel(0_u64);
    let lifecycle = Arc::new(ModelLifecycle {
        operation: Mutex::new(()),
        stop_generation,
    });
    lifecycles.insert(model_id.to_owned(), Arc::downgrade(&lifecycle));
    lifecycle
}

fn advance_stop_generation(stop_generation: &watch::Sender<u64>) {
    stop_generation.send_modify(|generation| {
        *generation = generation.wrapping_add(1);
    });
}

fn bridge_cancellation_sources(
    caller_cancellation: &watch::Receiver<bool>,
    stop_generation: &watch::Receiver<u64>,
    expected_generation: u64,
) -> (watch::Receiver<bool>, tokio::task::JoinHandle<()>) {
    let mut caller_cancellation = caller_cancellation.clone();
    let mut stop_generation = stop_generation.clone();
    let (cancellation_sender, cancellation_receiver) = watch::channel(false);
    let bridge = tokio::spawn(async move {
        loop {
            if *caller_cancellation.borrow() || *stop_generation.borrow() != expected_generation {
                cancellation_sender.send_replace(true);
                return;
            }
            tokio::select! {
                changed = caller_cancellation.changed() => {
                    if changed.is_err() || *caller_cancellation.borrow() {
                        cancellation_sender.send_replace(true);
                        return;
                    }
                }
                changed = stop_generation.changed() => {
                    if changed.is_err() || *stop_generation.borrow() != expected_generation {
                        cancellation_sender.send_replace(true);
                        return;
                    }
                }
            }
        }
    });
    (cancellation_receiver, bridge)
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
    let servers = server
        .map(|server| vec![server_status_json(server, status)])
        .unwrap_or_default();
    runtime_status_json(servers, status)
}

fn server_status_json(server: &RunningServer, status: &str) -> Value {
    json!({
        "engineId": server.engine_id,
        "modelId": server.model_id,
        "processId": server.child.id(),
        "port": server.port,
        "status": status,
    })
}

fn runtime_status_json(servers: Vec<Value>, status: &str) -> Value {
    let primary = servers.first();
    json!({
        "status": status,
        "engineId": primary.and_then(|server| server["engineId"].as_str()),
        "modelId": primary.and_then(|server| server["modelId"].as_str()),
        "port": primary.and_then(|server| server["port"].as_u64()),
        "servers": servers,
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

fn external_server_not_found_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_not_found",
        "The selected llama-server process is no longer running on that port.",
        true,
    )
}

fn external_server_not_ready_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_not_ready",
        "The selected llama-server did not pass its local health check.",
        true,
    )
}

fn external_server_auth_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_auth_required",
        "The selected llama-server requires authentication that OpenChat has not been configured to provide.",
        false,
    )
}

fn external_server_catalog_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_catalog_invalid",
        "The selected llama-server did not return a supported local model catalog.",
        true,
    )
}

fn external_server_capability_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_capabilities_invalid",
        "The selected llama-server returned invalid capability information.",
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
        InstalledEngine, MAX_RUNTIME_PROPERTIES_BYTES, RuntimeCapabilities,
        bridge_cancellation_sources, excluding_managed_server_processes, external_model_json,
        external_model_route_id, external_server_for_route, external_server_group_id,
        health_check_or_cancel, installed_engine_for, llama_server_arguments,
        managed_server_group_id, model_lifecycle, parse_external_capabilities,
        parse_external_model_ids, parse_runtime_capabilities, prepare_tabby_session,
        read_runtime_capabilities, remove_external_server, retain_detected_external_servers,
        runtime_status_json, start_model, vllm_server_arguments,
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
                context_window: Some(32_768),
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
}
