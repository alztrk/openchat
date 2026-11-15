//! Runtime health checks and capability probing.

use super::*;

pub(crate) fn is_model_available(
    storage: &AppStorage,
    model: &models::RegisteredModel,
) -> Result<bool, ServiceError> {
    if !model_path_matches_engine(model)? {
        return Ok(false);
    }
    Ok(installed_engine_for(storage, &model.engine_id)?.is_some())
}

pub(super) async fn health_check(port: u16, api_key: Option<&str>) -> bool {
    let client = HEALTH_CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
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

pub(super) async fn health_check_or_cancel(
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

pub(super) async fn runtime_capabilities_or_cancel(
    engine_id: &str,
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
        capabilities = async {
            match engine_id {
                "vllm" | "sglang" => read_model_list_runtime_capabilities(port, api_key).await,
                _ => read_runtime_capabilities(port, api_key).await,
            }
        } => capabilities,
    }
}

pub(super) async fn read_runtime_capabilities(
    port: u16,
    api_key: Option<&str>,
) -> Result<RuntimeCapabilities, ServiceError> {
    let properties = read_capability_endpoint(port, "/props", api_key).await?;
    parse_runtime_capabilities(&properties)
}

pub(super) async fn read_model_list_runtime_capabilities(
    port: u16,
    api_key: Option<&str>,
) -> Result<RuntimeCapabilities, ServiceError> {
    let models = read_capability_endpoint(port, "/v1/models", api_key).await?;
    parse_model_list_runtime_capabilities(&models)
}

async fn read_capability_endpoint(
    port: u16,
    endpoint: &str,
    api_key: Option<&str>,
) -> Result<Value, ServiceError> {
    let client = PROPERTIES_CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(10))
            .build()
    });
    let client = client.as_ref().map_err(|_| runtime_capability_error())?;
    let mut request = client.get(format!("http://127.0.0.1:{port}{endpoint}"));
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
    serde_json::from_slice::<Value>(&body).map_err(|_| runtime_capability_error())
}

pub(super) fn parse_model_list_runtime_capabilities(
    models: &Value,
) -> Result<RuntimeCapabilities, ServiceError> {
    let context_window = models
        .pointer("/data/0/max_model_len")
        .and_then(Value::as_i64)
        .filter(|context_window| *context_window > 0)
        .ok_or_else(runtime_capability_error)?;
    Ok(RuntimeCapabilities {
        context_window: Some(context_window),
        supports_images: false,
        supports_tool_calls: None,
    })
}

pub(super) fn parse_runtime_capabilities(
    properties: &Value,
) -> Result<RuntimeCapabilities, ServiceError> {
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
