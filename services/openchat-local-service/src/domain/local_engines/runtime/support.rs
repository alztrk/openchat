//! Runtime launch helpers, lifecycle state, logging, and error constructors.

use super::*;

pub(super) async fn available_loopback_port(preferred: Option<u16>) -> Result<u16, ServiceError> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], preferred.unwrap_or(0))))
        .await
        .map_err(|_| runtime_error())?;
    let address: SocketAddr = listener.local_addr().map_err(|_| runtime_error())?;
    Ok(address.port())
}

pub(super) fn append_context_length(
    arguments: &mut Vec<OsString>,
    option: &str,
    context_length: Option<u32>,
) {
    if let Some(context_length) = context_length {
        arguments.extend([
            OsString::from(option),
            OsString::from(context_length.to_string()),
        ]);
    }
}

pub(super) fn llama_server_arguments(
    storage_root: &Path,
    model: &models::RegisteredModel,
    port: u16,
) -> Result<Vec<OsString>, ServiceError> {
    let settings = settings::read_settings(storage_root)?;
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

    if let Some(gpu_layers) = settings.gpu_layers {
        arguments.push(OsString::from("-ngl"));
        arguments.push(OsString::from(gpu_layers.to_string()));
    }

    if let Some(threads) = settings.threads {
        arguments.push(OsString::from("-t"));
        arguments.push(OsString::from(threads.to_string()));
    }

    if let Some(threads_batch) = settings.threads_batch {
        arguments.push(OsString::from("-tb"));
        arguments.push(OsString::from(threads_batch.to_string()));
    }

    append_context_length(
        &mut arguments,
        "-c",
        model.context_length.or(settings.context_size),
    );

    if let Some(batch_size) = settings.batch_size {
        arguments.push(OsString::from("-b"));
        arguments.push(OsString::from(batch_size.to_string()));
    }

    if let Some(ubatch_size) = settings.ubatch_size {
        arguments.push(OsString::from("-ub"));
        arguments.push(OsString::from(ubatch_size.to_string()));
    }

    if settings.flash_attention == Some(true) {
        arguments.push(OsString::from("--flash-attn"));
    }

    if settings.mlock == Some(true) {
        arguments.push(OsString::from("--mlock"));
    }

    if settings.no_mmap == Some(true) {
        arguments.push(OsString::from("--no-mmap"));
    }

    if let Some(parallel) = settings.parallel {
        arguments.push(OsString::from("-np"));
        arguments.push(OsString::from(parallel.to_string()));
    }

    if let Some(custom) = settings.custom_arguments.as_deref() {
        for arg in custom.split_whitespace() {
            if !arg.is_empty() {
                arguments.push(OsString::from(arg));
            }
        }
    }

    if let Some(projector) = models::vision_projector_path(&model.path)? {
        arguments.push(OsString::from("--mmproj"));
        arguments.push(projector.into_os_string());
    }
    Ok(arguments)
}

pub(super) async fn vllm_server_launch(
    storage: &AppStorage,
    model: &models::RegisteredModel,
    port: u16,
) -> Result<RuntimeLaunch, ServiceError> {
    let context = super::super::vllm::launch_context(storage)?;
    let model_path = super::super::vllm::model_path_for_runtime(&context, &model.path).await?;
    let api_key = Zeroizing::new(uuid::Uuid::new_v4().to_string());
    let auth_file = super::super::vllm::write_auth_file(&context, api_key.as_str()).await?;
    let runtime_python = super::super::vllm::runtime_child_path(
        &context.root,
        ".venv/bin/vllm",
        context.distro.is_some(),
    );
    let remote_pid_file = context.distro.as_ref().map(|_| {
        super::super::vllm::runtime_child_path(
            &context.root,
            &format!(".session-{}.pid", uuid::Uuid::new_v4()),
            true,
        )
    });
    let auth_path = auth_file.to_str().ok_or_else(runtime_error)?;
    let root_path = context.root.to_str().ok_or_else(runtime_error)?;
    let mut arguments = Vec::<OsString>::new();
    let mut environment = Vec::new();

    if let Some(distro) = context.distro.as_deref() {
        let pid_file = remote_pid_file
            .as_deref()
            .and_then(Path::to_str)
            .ok_or_else(runtime_error)?;
        arguments.extend([
            OsString::from("--distribution"),
            OsString::from(distro),
            OsString::from("--exec"),
            OsString::from("env"),
            OsString::from(format!("OPENCHAT_VLLM_AUTH_FILE={auth_path}")),
            OsString::from(format!("PYTHONPATH={root_path}")),
            OsString::from("VLLM_USE_FLASHINFER_SAMPLER=0"),
            OsString::from("sh"),
            OsString::from("-c"),
            OsString::from("printf '%s' \"$$\" > \"$1\"; shift; exec \"$@\""),
            OsString::from("openchat-vllm"),
            OsString::from(pid_file),
            runtime_python.into_os_string(),
        ]);
    } else {
        environment.push((
            OsString::from("OPENCHAT_VLLM_AUTH_FILE"),
            OsString::from(auth_path),
        ));
        environment.push((OsString::from("PYTHONPATH"), OsString::from(root_path)));
        environment.push((
            OsString::from("VLLM_USE_FLASHINFER_SAMPLER"),
            OsString::from("0"),
        ));
    }
    arguments.extend([
        OsString::from("serve"),
        OsString::from(model_path),
        OsString::from("--host"),
        OsString::from("127.0.0.1"),
        OsString::from("--port"),
        OsString::from(port.to_string()),
        OsString::from("--served-model-name"),
        OsString::from(&model.id),
        OsString::from("--middleware"),
        OsString::from("openchat_vllm_auth.OpenChatAuthMiddleware"),
    ]);

    let settings = settings::read_settings(storage.root())?;
    append_context_length(
        &mut arguments,
        "--max-model-len",
        model.context_length.or(settings.vllm_max_model_len),
    );
    if let Some(value) = settings.vllm_gpu_memory_utilization_milli {
        arguments.extend([
            OsString::from("--gpu-memory-utilization"),
            OsString::from(format!("{:.3}", f64::from(value) / 1000.0)),
        ]);
    }
    if let Some(value) = settings.vllm_tensor_parallel_size {
        arguments.extend([
            OsString::from("--tensor-parallel-size"),
            OsString::from(value.to_string()),
        ]);
    }
    if let Some(value) = settings.vllm_max_num_seqs {
        arguments.extend([
            OsString::from("--max-num-seqs"),
            OsString::from(value.to_string()),
        ]);
    }
    if let Some(value) = settings.vllm_dtype {
        arguments.extend([OsString::from("--dtype"), OsString::from(value)]);
    }
    if settings.vllm_enforce_eager == Some(true) {
        arguments.push(OsString::from("--enforce-eager"));
    }
    if let Some(custom_arguments) = settings.vllm_custom_arguments.as_deref() {
        arguments.extend(custom_arguments.split_whitespace().map(OsString::from));
    }

    Ok(RuntimeLaunch {
        arguments,
        current_directory: if context.distro.is_some() {
            storage.root().to_path_buf()
        } else {
            context.root
        },
        initial_capabilities: None,
        provider_model_id: model.id.clone(),
        api_key: Some(api_key),
        environment,
        auth_file: Some(auth_file),
        remote_pid_file,
        remote_process_marker: None,
    })
}

pub(super) async fn python_engine_server_launch(
    storage: &AppStorage,
    model: &models::RegisteredModel,
    port: u16,
) -> Result<RuntimeLaunch, ServiceError> {
    let engine = super::super::python_engines::Engine::parse(&model.engine_id)?;
    let paths = super::super::python_engines::launch_context(storage, &model.engine_id).await?;
    let settings = settings::read_settings(storage.root())?;
    let model_name = model
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(model_unavailable_error)?;
    let model_path =
        super::super::python_engines::model_path_for_runtime(&paths, &model.path).await?;
    let session =
        super::super::python_engines::create_session(&paths, engine, model, port, &settings)
            .await?;
    let python = super::super::python_engines::python_path(&paths);
    let mut arguments = Vec::<OsString>::new();
    let mut environment = Vec::new();
    let provider_model_id;

    match engine {
        super::super::python_engines::Engine::Exllama => {
            arguments.push(session.runner.as_os_str().to_owned());
            provider_model_id = model_name.to_owned();
        }
        super::super::python_engines::Engine::Sglang => {
            arguments.extend([
                session.runner.as_os_str().to_owned(),
                OsString::from("--model-path"),
                OsString::from(model_path),
                OsString::from("--served-model-name"),
                OsString::from(&model.id),
                OsString::from("--host"),
                OsString::from("127.0.0.1"),
                OsString::from("--port"),
                OsString::from(port.to_string()),
            ]);
            append_context_length(
                &mut arguments,
                "--context-length",
                model.context_length.or(settings.sglang_context_length),
            );
            if let Some(memory_fraction) = settings.sglang_mem_fraction_static_milli {
                arguments.extend([
                    OsString::from("--mem-fraction-static"),
                    OsString::from(format!("{:.3}", f64::from(memory_fraction) / 1000.0)),
                ]);
            }
            if let Some(max_requests) = settings.sglang_max_running_requests {
                arguments.extend([
                    OsString::from("--max-running-requests"),
                    OsString::from(max_requests.to_string()),
                ]);
            }
            provider_model_id = model.id.clone();
            if !paths.wsl {
                environment.push((
                    OsString::from("OPENCHAT_SGLANG_TOKEN_FILE"),
                    session.auth_file.as_os_str().to_owned(),
                ));
            }
        }
    }

    if paths.wsl {
        let distro = paths.distro.as_deref().ok_or_else(runtime_error)?;
        let pid_file = session
            .remote_pid_file
            .as_deref()
            .and_then(Path::to_str)
            .ok_or_else(runtime_error)?;
        let token_file = session.auth_file.to_str().ok_or_else(runtime_error)?;
        let python = python.to_str().ok_or_else(runtime_error)?;
        let runner = session.runner.to_str().ok_or_else(runtime_error)?;
        let mut wsl_arguments = vec![
            OsString::from("--distribution"),
            OsString::from(distro),
            OsString::from("--exec"),
            OsString::from("env"),
            OsString::from(format!("OPENCHAT_SGLANG_TOKEN_FILE={token_file}")),
            OsString::from("sh"),
            OsString::from("-c"),
            OsString::from("printf '%s' \"$$\" > \"$1\"; shift; exec \"$@\""),
            OsString::from("openchat-python-engine"),
            OsString::from(pid_file),
            OsString::from(python),
            OsString::from(runner),
        ];
        wsl_arguments.extend(arguments.iter().skip(1).cloned());
        arguments = wsl_arguments;
    }

    Ok(RuntimeLaunch {
        arguments,
        current_directory: if paths.wsl {
            storage.root().to_path_buf()
        } else {
            session.directory
        },
        initial_capabilities: None,
        provider_model_id,
        api_key: Some(session.api_key),
        environment,
        auth_file: Some(session.auth_file),
        remote_pid_file: session.remote_pid_file,
        remote_process_marker: paths.wsl.then_some(session.runner),
    })
}

pub(super) fn configure_llama_server_api_key(command: &mut Command, api_key: &Zeroizing<String>) {
    command.env("LLAMA_API_KEY", api_key.as_str());
}

pub(super) fn model_path_matches_engine(
    model: &models::RegisteredModel,
) -> Result<bool, ServiceError> {
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
        "vllm" => {
            model.path_kind == "directory"
                && model.path.is_dir()
                && model.path.join("config.json").is_file()
        }
        "exllama" | "sglang" => {
            model.path_kind == "directory"
                && model.path.is_dir()
                && model.path.join("config.json").is_file()
        }
        _ => false,
    })
}

pub(super) fn installed_engine_for(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<Option<InstalledEngine>, ServiceError> {
    if engine_id == "vllm" {
        return super::super::vllm::installed_engine(storage);
    }
    if matches!(engine_id, "exllama" | "sglang") {
        return super::super::python_engines::installed_engine(storage, engine_id);
    }
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

pub(super) fn managed_servers_state() -> &'static Mutex<HashMap<String, RunningServer>> {
    MANAGED_SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
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
    if matches!(server.engine_id.as_str(), "vllm" | "exllama" | "sglang")
        && let Some(pid_file) = server.remote_pid_file.as_deref()
    {
        if server.engine_id == "vllm" {
            super::super::vllm::stop_wsl_process(storage, pid_file).await?;
        } else if let Some(marker) = server.remote_process_marker.as_deref() {
            super::super::python_engines::stop_wsl_process(
                storage,
                &server.engine_id,
                pid_file,
                marker,
            )
            .await?;
        }
    }
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
    if let Some(task) = server.stderr_diagnostics.take() {
        let _ = task.await;
    }
    if let Some(task) = server.stdout_diagnostics.take() {
        let _ = task.await;
    }
    if matches!(server.engine_id.as_str(), "vllm" | "exllama" | "sglang")
        && let Some(auth_file) = server.auth_file.as_deref()
    {
        if server.engine_id == "vllm" {
            super::super::vllm::remove_auth_file(storage, auth_file).await?;
        } else {
            super::super::python_engines::remove_session(storage, &server.engine_id, auth_file)
                .await?;
        }
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
        "contextWindow": server.capabilities.and_then(|capabilities| capabilities.context_window),
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
