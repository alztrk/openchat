//! RPC routing for local inference engines and the Hugging Face model hub.

use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{
    hugging_face, local_engines,
    protocol::{EventSink, ServiceError},
    storage::AppStorage,
};

use super::{optional_string, required_string};

/// Dispatches local-engine, local-model, and model-hub requests.
pub(super) async fn dispatch(
    method: &str,
    params: &Value,
    request_id: &Value,
    storage: &AppStorage,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    match method {
        "local.engines.list" => local_engines::list(storage).await,
        "local.engines.runtime.status" => local_engines::runtime_status(storage).await,
        "local.engines.settings.get" => local_engines::settings(storage),
        "local.engines.settings.set" => local_engines::update_settings(storage, params),
        "local.engines.llama_server_path.set" => local_engines::set_llama_server_executable_path(
            storage,
            optional_string(params, "path")?,
        ),
        "local.engines.external_llama_server.detect" => {
            local_engines::detect_external_llama_servers().await
        }
        "local.engines.external_llama_server.connect" => {
            local_engines::connect_external_llama_server(
                required_non_zero_u32(params, "processId")?,
                required_port(params, "port")?,
            )
            .await
        }
        "local.engines.external_llama_server.disconnect" => {
            Ok(local_engines::disconnect_external_llama_server(
                required_non_zero_u32(params, "processId")?,
                required_port(params, "port")?,
            )
            .await)
        }
        "local.engines.local_servers.detect" => {
            local_engines::detect_local_servers(
                optional_port(params, "ollamaPort")?,
                optional_port(params, "lmStudioPort")?,
            )
            .await
        }
        "local.engines.local_servers.models" => {
            local_engines::fetch_local_server_models(
                required_string(params, "serverKind")?,
                required_port(params, "port")?,
            )
            .await
        }
        "local.engines.chat_models.list" => {
            local_engines::chat_model_catalog(storage, optional_string(params, "engineId")?).await
        }
        "local.models.list" => local_engines::model_catalog(storage),
        "local.models.discover" => {
            local_engines::discover_models(
                storage,
                required_string(params, "engineId")?,
                required_string(params, "modelDirectory")?,
            )
            .await
        }
        "models.hub.search" => {
            let query = match params.get("query") {
                None | Some(Value::Null) => "",
                Some(Value::String(query)) => query,
                _ => {
                    return Err(ServiceError::new(
                        "invalid_request_params",
                        "The model search query must be text.",
                        false,
                    ));
                }
            };
            let format = required_string(params, "format")?;
            let sort = required_string(params, "sort")?;
            let cursor = match params.get("cursor") {
                None | Some(Value::Null) => None,
                Some(Value::String(cursor)) => Some(cursor.as_str()),
                _ => {
                    return Err(ServiceError::new(
                        "invalid_request_params",
                        "The model search cursor must be text.",
                        false,
                    ));
                }
            };
            hugging_face::search(query, format, sort, cursor).await
        }
        "models.hub.files" => {
            let repo_id = required_string(params, "repoId")?;
            let format = required_string(params, "format")?;
            hugging_face::files(repo_id, format).await
        }
        "models.hub.download" => {
            let repo_id = required_string(params, "repoId")?;
            let revision = required_string(params, "revision")?;
            let engine_id = required_string(params, "engineId")?;
            let format = required_string(params, "format")?;
            let group_id = required_string(params, "groupId")?;
            let component_path = optional_string(params, "componentPath")?;
            let model_directory = optional_string(params, "modelDirectory")?;
            hugging_face::download(
                storage,
                hugging_face::ModelDownloadRequest {
                    repo_id,
                    revision,
                    engine_id,
                    format,
                    group_id,
                    component_path,
                    model_directory,
                    request_id,
                    events,
                    cancellation,
                },
            )
            .await
        }
        "local.models.register" => {
            local_engines::register_model(
                storage,
                required_string(params, "engineId")?,
                required_string(params, "modelPath")?,
                optional_string(params, "modelDirectory")?,
                required_string(params, "storageAction")?,
            )
            .await
        }
        "local.models.context.set" => {
            let model_id = required_string(params, "modelId")?;
            let context_length = nullable_context_length(params)?;
            local_engines::set_model_context_length(storage, model_id, context_length)
        }
        "local.models.remove" => {
            local_engines::remove_model(storage, required_string(params, "modelId")?).await
        }
        "local.engines.start" => {
            local_engines::start_model(storage, required_string(params, "modelId")?, cancellation)
                .await
        }
        "local.engines.stop" => local_engines::stop_runtime(storage).await,
        "local.engines.stop_model" => {
            local_engines::stop_model(storage, required_string(params, "modelId")?).await
        }
        "local.engines.install" => {
            let engine_id = required_string(params, "engineId")?;
            let variant_id = required_string(params, "variantId")?;
            if engine_id == "vllm" {
                return local_engines::install_vllm(
                    storage,
                    variant_id,
                    request_id,
                    events,
                    cancellation,
                )
                .await;
            }
            if matches!(engine_id, "exllama" | "sglang") {
                return local_engines::install_python_engine(
                    storage,
                    engine_id,
                    variant_id,
                    request_id,
                    events,
                    cancellation,
                )
                .await;
            }
            let (manifest, variant) = local_engines::find_variant(engine_id, variant_id).await?;
            if !local_engines::can_install(&manifest, &variant).await {
                return Err(ServiceError::new(
                    "local_engine_install_unavailable",
                    "The selected local engine package is not available for this device.",
                    false,
                ));
            }
            let installed = local_engines::installer::install_engine_variant(
                storage,
                &manifest,
                &variant,
                request_id,
                events,
                cancellation,
            )
            .await?;
            Ok(json!({
                "installed": true,
                "engineId": installed.engine_id,
                "variantId": installed.variant_id,
                "releaseTag": installed.release_tag,
            }))
        }
        "local.engines.uninstall" => {
            local_engines::uninstall_engine(storage, required_string(params, "engineId")?).await
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn required_non_zero_u32(params: &Value, name: &str) -> Result<u32, ServiceError> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(invalid_local_engine_request)
}

fn required_port(params: &Value, name: &str) -> Result<u16, ServiceError> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(invalid_local_engine_request)
}

fn optional_port(params: &Value, name: &str) -> Result<Option<u16>, ServiceError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port > 0)
            .map(Some)
            .ok_or_else(invalid_local_engine_request),
    }
}

fn invalid_local_engine_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The local engine request parameters are invalid.",
        false,
    )
}

fn nullable_context_length(params: &Value) -> Result<Option<u32>, ServiceError> {
    match params.get("contextLength") {
        Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .map(Some)
            .ok_or_else(invalid_local_engine_request),
        _ => Err(invalid_local_engine_request()),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;

    use crate::{protocol::EventSink, storage::AppStorage};

    use super::dispatch;

    #[tokio::test]
    async fn local_engine_dispatch_preserves_method_and_parameter_errors() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
        let (_sender, mut cancellation) = watch::channel(false);
        let events = EventSink::new();

        let missing_engine_parameters = dispatch(
            "local.engines.external_llama_server.connect",
            &json!({}),
            &json!("request-1"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect_err("missing process and port values are rejected");
        assert_eq!(missing_engine_parameters.code, "invalid_request_params");

        let invalid_query = dispatch(
            "models.hub.search",
            &json!({"query": 1, "format": "gguf", "sort": "downloads"}),
            &json!("request-2"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect_err("non-text model search queries are rejected");
        assert_eq!(invalid_query.code, "invalid_request_params");

        let unknown_method = dispatch(
            "local.models.unknown",
            &json!({}),
            &json!("request-3"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect_err("unknown local model methods are rejected");
        assert_eq!(unknown_method.code, "method_not_found");
    }

    #[tokio::test]
    async fn local_model_context_can_be_set_and_cleared() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
        storage
            .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
            .expect("prepare current chat schema");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        let model_path = directory.0.join("model.gguf");
        fs::write(&model_path, b"model").expect("write model fixture");
        let (_sender, mut cancellation) = watch::channel(false);
        let events = EventSink::new();
        let registered = dispatch(
            "local.models.register",
            &json!({
                "engineId": "llama_cpp",
                "modelPath": model_path,
                "storageAction": "keep"
            }),
            &json!("request-register"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect("register local model");
        let model_id = registered["id"].as_str().expect("model id should exist");

        let updated = dispatch(
            "local.models.context.set",
            &json!({"modelId": model_id, "contextLength": 16384}),
            &json!("request-context-set"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect("set model context length");
        assert_eq!(updated["contextLength"], 16384);

        let cleared = dispatch(
            "local.models.context.set",
            &json!({"modelId": model_id, "contextLength": null}),
            &json!("request-context-clear"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect("clear model context length");
        assert!(cleared["contextLength"].is_null());

        let invalid = dispatch(
            "local.models.context.set",
            &json!({"modelId": model_id, "contextLength": 64}),
            &json!("request-context-invalid"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect_err("reject unsupported context length");
        assert_eq!(invalid.code, "local_model_context_length_invalid");
    }

    #[tokio::test]
    async fn local_engine_settings_rpc_gets_and_updates_configuration() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.join("data")).expect("open test storage");
        let (_sender, mut cancellation) = watch::channel(false);
        let events = EventSink::new();

        let initial = dispatch(
            "local.engines.settings.get",
            &json!({}),
            &json!("req-get-1"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect("get settings should succeed");
        assert_eq!(initial["gpuLayers"], serde_json::Value::Null);

        let updated = dispatch(
            "local.engines.settings.set",
            &json!({
                "gpuLayers": 33,
                "threads": 8,
                "contextSize": 8192,
                "flashAttention": true,
                "customArguments": "--numa",
            }),
            &json!("req-set-1"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect("set settings should succeed");
        assert_eq!(updated["gpuLayers"], 33);
        assert_eq!(updated["threads"], 8);
        assert_eq!(updated["contextSize"], 8192);
        assert_eq!(updated["flashAttention"], true);
        assert_eq!(updated["customArguments"], "--numa");

        let fetched = dispatch(
            "local.engines.settings.get",
            &json!({}),
            &json!("req-get-2"),
            &storage,
            &events,
            &mut cancellation,
        )
        .await
        .expect("get updated settings should succeed");
        assert_eq!(fetched["gpuLayers"], 33);
        assert_eq!(fetched["contextSize"], 8192);
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-local-engine-rpc-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
