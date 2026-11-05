//! Runtime launch helpers, lifecycle state, logging, and error constructors.

use super::*;

pub(super) async fn available_loopback_port() -> Result<u16, ServiceError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| runtime_error())?;
    let address: SocketAddr = listener.local_addr().map_err(|_| runtime_error())?;
    Ok(address.port())
}

pub(super) fn llama_server_arguments(
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
        OsString::from("--no-ui"),
    ];
    if let Some(projector) = models::vision_projector_path(&model.path)? {
        arguments.push(OsString::from("--mmproj"));
        arguments.push(projector.into_os_string());
    }
    Ok(arguments)
}

pub(super) fn configure_llama_server_api_key(command: &mut Command, api_key: &Zeroizing<String>) {
    command.env("LLAMA_API_KEY", api_key.as_str());
}

pub(super) fn model_path_matches_engine(
    model: &models::RegisteredModel,
) -> Result<bool, ServiceError> {
    Ok(model.engine_id == "llama_cpp"
        && model.path_kind == "file"
        && model.path.is_file()
        && model
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf")))
}

pub(super) fn installed_engine_for(
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

pub(super) fn running_llama_state() -> &'static Mutex<HashMap<String, RunningServer>> {
    RUNNING_LLAMA_SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn runtime_operation_gate() -> &'static RwLock<()> {
    RUNTIME_OPERATION_GATE.get_or_init(|| RwLock::new(()))
}

pub(super) fn model_lifecycles() -> &'static Mutex<HashMap<String, Weak<ModelLifecycle>>> {
    MODEL_LIFECYCLES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) async fn model_lifecycle(model_id: &str) -> Arc<ModelLifecycle> {
    let mut lifecycles = model_lifecycles().lock().await;
    model_lifecycle_locked(model_id, &mut lifecycles)
}

pub(super) fn model_lifecycle_locked(
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

pub(super) fn advance_stop_generation(stop_generation: &watch::Sender<u64>) {
    stop_generation.send_modify(|generation| {
        *generation = generation.wrapping_add(1);
    });
}

pub(super) fn bridge_cancellation_sources(
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

pub(super) async fn stop_server(
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
    let Some(_server) = state.take() else {
        return Err(runtime_error());
    };
    log_runtime_event(storage, "runtime_stopped", None, None);
    Ok(())
}

pub(super) fn log_runtime_event(
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

pub(super) fn status_json(server: Option<&RunningServer>, status: &str) -> Value {
    let servers = server
        .map(|server| vec![server_status_json(server, status)])
        .unwrap_or_default();
    runtime_status_json(servers, status)
}

pub(super) fn server_status_json(server: &RunningServer, status: &str) -> Value {
    json!({
        "engineId": server.engine_id,
        "modelId": server.model_id,
        "processId": server.child.id(),
        "port": server.port,
        "status": status,
    })
}

pub(super) fn runtime_status_json(servers: Vec<Value>, status: &str) -> Value {
    let primary = servers.first();
    json!({
        "status": status,
        "engineId": primary.and_then(|server| server["engineId"].as_str()),
        "modelId": primary.and_then(|server| server["modelId"].as_str()),
        "port": primary.and_then(|server| server["port"].as_u64()),
        "servers": servers,
    })
}

pub(super) fn engine_not_installed_error() -> ServiceError {
    ServiceError::new(
        "local_engine_not_installed",
        "The selected local inference engine is not installed.",
        false,
    )
}

pub(super) fn model_unavailable_error() -> ServiceError {
    ServiceError::new(
        "local_model_unavailable",
        "The selected local model file is unavailable or unsupported.",
        false,
    )
}

pub(super) fn runtime_start_failed_error() -> ServiceError {
    ServiceError::new(
        "local_engine_start_failed",
        "The local inference engine exited before its health check passed.",
        true,
    )
}

pub(super) fn runtime_start_timeout_error() -> ServiceError {
    ServiceError::new(
        "local_engine_start_timeout",
        "The local inference engine did not become ready before the startup limit.",
        true,
    )
}

pub(super) fn runtime_error() -> ServiceError {
    ServiceError::new(
        "local_engine_runtime_unavailable",
        "The local inference engine could not be controlled.",
        true,
    )
}

pub(super) fn runtime_capability_error() -> ServiceError {
    ServiceError::new(
        "local_engine_capability_unavailable",
        "The local inference server did not report a valid context window and capability status.",
        true,
    )
}

pub(super) fn external_server_not_found_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_not_found",
        "The selected llama-server process is no longer running on that port.",
        true,
    )
}

pub(super) fn external_server_not_ready_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_not_ready",
        "The selected llama-server did not pass its local health check.",
        true,
    )
}

pub(super) fn external_server_auth_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_auth_required",
        "The selected llama-server requires authentication that OpenChat has not been configured to provide.",
        false,
    )
}

pub(super) fn external_server_catalog_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_catalog_invalid",
        "The selected llama-server did not return a supported local model catalog.",
        true,
    )
}

pub(super) fn external_server_capability_error() -> ServiceError {
    ServiceError::new(
        "local_engine_external_server_capabilities_invalid",
        "The selected llama-server returned invalid capability information.",
        true,
    )
}

pub(super) fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "Starting the local model was cancelled.",
        false,
    )
}
