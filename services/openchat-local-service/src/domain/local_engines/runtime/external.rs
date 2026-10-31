//! External llama.cpp server discovery, connection, and model catalog.

use super::*;

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

pub(super) fn excluding_managed_server_processes(
    candidates: Vec<ExternalLlamaServerCandidate>,
    managed_process_ids: &HashSet<u32>,
) -> Vec<ExternalLlamaServerCandidate> {
    candidates
        .into_iter()
        .filter(|candidate| !managed_process_ids.contains(&candidate.process_id))
        .collect()
}

pub(super) async fn managed_server_process_ids() -> HashSet<u32> {
    let mut process_ids = HashSet::new();
    process_ids.extend(
        running_llama_state()
            .lock()
            .await
            .values()
            .filter_map(|server| server.child.id()),
    );
    process_ids
}

pub(super) async fn candidate_is_external(
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
    if engine_id != "llama_cpp" {
        return Err(ServiceError::new(
            "local_engine_unknown",
            "The selected local inference engine is not available.",
            false,
        ));
    }

    let managed_model_groups = status(storage)
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
        .collect::<HashMap<_, _>>();
    let mut catalog_models = models::list(storage)?
        .into_iter()
        .filter(|model| model.engine_id == engine_id)
        .map(|model| {
            let available = is_model_available(storage, &model)?;
            let mut model_json = models::to_json(&model, available)?;
            let group_id = managed_model_groups
                .get(&model.id)
                .cloned()
                .unwrap_or_else(|| "managed".to_owned());
            model_json["groupId"] = Value::String(group_id);
            Ok(model_json)
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;
    catalog_models.extend(external_chat_models().await?);
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
    let state = running_llama_state().lock().await;
    let server = state
        .get(model_id)
        .filter(|server| server.port == port)
        .ok_or_else(runtime_error)?;
    let capabilities = server.capabilities.ok_or_else(runtime_capability_error)?;
    Ok(RuntimeChatEndpoint {
        chat_url: format!("http://127.0.0.1:{port}/v1/chat/completions"),
        model_id: server.provider_model_id.clone(),
        capabilities,
        api_key: server.api_key.clone(),
    })
}

pub(super) async fn external_chat_models() -> Result<Vec<Value>, ServiceError> {
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

pub(super) async fn external_chat_endpoint(
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

pub(super) async fn clear_external_server_if_current(
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

pub(super) fn external_model_json(model: &ExternalLlamaModel, process_id: u32, port: u16) -> Value {
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

pub(super) fn external_server_group_id(process_id: u32, port: u16) -> String {
    format!("external-llama-server:{process_id}:{port}")
}

pub(super) fn external_model_route_id(
    process_id: u32,
    port: u16,
    provider_model_id: &str,
) -> String {
    let identity = format!("{process_id}:{port}:{provider_model_id}");
    let digest = Sha256::digest(identity.as_bytes());
    let digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("external-llama-{digest}")
}

pub(super) async fn external_health_check(port: u16) -> Result<bool, ServiceError> {
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

pub(super) async fn external_health_check_or_cancel(
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

pub(super) async fn fetch_external_model_ids(port: u16) -> Result<Vec<String>, ServiceError> {
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

pub(super) fn parse_external_model_ids(response: &Value) -> Result<Vec<String>, ServiceError> {
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

pub(super) async fn read_external_capabilities(
    port: u16,
) -> Result<RuntimeCapabilities, ServiceError> {
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

pub(super) fn parse_external_capabilities(properties: &Value) -> RuntimeCapabilities {
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

pub(super) async fn read_bounded_json(
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

pub(super) fn external_http_client() -> Result<&'static Client, ServiceError> {
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

pub(super) fn external_llama_servers_state()
-> &'static Mutex<HashMap<(u32, u16), ConnectedExternalLlamaServer>> {
    EXTERNAL_LLAMA_SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn external_server_mutation_lock() -> &'static Mutex<()> {
    EXTERNAL_SERVER_MUTATION_LOCK.get_or_init(|| Mutex::new(()))
}

pub(super) fn external_server_status_json(server: &ConnectedExternalLlamaServer) -> Value {
    json!({
        "processId": server.process_id,
        "port": server.port,
        "modelIds": server.models.iter().map(|model| model.provider_model_id.as_str()).collect::<Vec<_>>(),
    })
}

pub(super) fn external_server_for_route<'a>(
    servers: &'a HashMap<(u32, u16), ConnectedExternalLlamaServer>,
    route_id: &str,
) -> Option<&'a ConnectedExternalLlamaServer> {
    servers
        .values()
        .find(|server| server.models.iter().any(|model| model.route_id == route_id))
}

pub(super) fn retain_detected_external_servers(
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

pub(super) fn remove_external_server(
    servers: &mut HashMap<(u32, u16), ConnectedExternalLlamaServer>,
    process_id: u32,
    port: u16,
) -> bool {
    servers.remove(&(process_id, port)).is_some()
}

pub(super) fn managed_server_group_id(process_id: u32, port: u16) -> String {
    format!("managed-llama-server:{process_id}:{port}")
}
