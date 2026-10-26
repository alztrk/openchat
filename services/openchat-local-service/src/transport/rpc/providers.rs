//! RPC routing for provider accounts, authentication, and model catalogs.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{
    chatgpt::{
        ChatGptService, ImageBackground, ImageGenerationAuth, ImageGenerationRequest,
        ImageModeration, ImageOutputFormat, ImageQuality,
    },
    image_library, openai_api, openai_compatible,
    protocol::ServiceError,
    provider_credentials::ProviderCredentials,
    service::OperationGate,
    storage::AppStorage,
};

use super::{bounded_route_id, optional_string, required_string, take_optional_api_key};

/// Dispatches provider authentication, account, and model-catalog requests.
pub(super) async fn dispatch(
    method: &str,
    params: &mut Value,
    storage: &AppStorage,
    service: &Arc<ChatGptService>,
    cancellation: &mut watch::Receiver<bool>,
    cancellation_gate: &OperationGate,
) -> Result<Value, ServiceError> {
    match method {
        "chatgpt.oauth.start" => service.oauth_sign_in(cancellation, cancellation_gate).await,
        "copilot.oauth.start" => service.copilot_oauth_start().await,
        "copilot.oauth.poll" => {
            service
                .copilot_oauth_poll(required_string(params, "flowId")?, cancellation)
                .await
        }
        "copilot.oauth.cancel" => {
            service
                .copilot_oauth_cancel(required_string(params, "flowId")?)
                .await
        }
        "copilot.oauth.status" => service.copilot_oauth_status(),
        "copilot.oauth.select" => {
            service.copilot_oauth_select(required_string(params, "connectionId")?)
        }
        "copilot.oauth.disconnect" => {
            service
                .copilot_oauth_disconnect(required_string(params, "connectionId")?)
                .await
        }
        "copilot.usage.get" => service.copilot_usage(cancellation).await,
        "chatgpt.connections.list" => service.list_connections(),
        "chatgpt.connections.refresh_plans" => service.refresh_connection_plans(cancellation).await,
        "chatgpt.connections.delete" => {
            service
                .delete_connection(
                    required_string(params, "connectionId")?,
                    cancellation,
                    cancellation_gate,
                )
                .await
        }
        "chatgpt.titles.get" => service.title_preference(),
        "chatgpt.titles.select" => {
            let connection_id = optional_string(params, "connectionId")?;
            let workspace_id = optional_string(params, "workspaceId")?;
            service.select_title_target(connection_id, workspace_id)
        }
        "chatgpt.connections.select" => {
            service.select_connection(required_string(params, "connectionId")?)
        }
        "chatgpt.workspaces.select" => {
            let connection_id = required_string(params, "connectionId")?;
            let workspace_id = required_string(params, "workspaceId")?;
            service.select_workspace(connection_id, workspace_id)
        }
        "chatgpt.models.list" => {
            let connection_id = required_string(params, "connectionId")?;
            let workspace_id = required_string(params, "workspaceId")?;
            let force_refresh = params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            service
                .models(connection_id, workspace_id, force_refresh, cancellation)
                .await
        }
        "chatgpt.api.models.list" => {
            let _untrusted_api_key = take_optional_api_key(params, "apiKey")?;
            let connection_id = bounded_route_id(params, "connectionId")?;
            let credentials = ProviderCredentials::load(storage)?;
            let api_key = credentials.required_api_key("chatgpt_api", Some(connection_id))?;
            let force_refresh = params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            openai_api::models(storage, api_key.as_str(), force_refresh, cancellation).await
        }
        "chatgpt.usage.get" => {
            let connection_id = required_string(params, "connectionId")?;
            let workspace_id = required_string(params, "workspaceId")?;
            service
                .usage(connection_id, workspace_id, cancellation)
                .await
        }
        "chatgpt.images.generate" => {
            generate_design_image(params, storage, service, cancellation).await
        }
        "chatgpt.reset_credits.consume" => {
            let connection_id = required_string(params, "connectionId")?;
            let workspace_id = required_string(params, "workspaceId")?;
            let credit_id = required_string(params, "creditId")?;
            service
                .consume_reset_credit(connection_id, workspace_id, credit_id, cancellation)
                .await
        }
        "opencode.models.list" => {
            let _untrusted_api_key = take_optional_api_key(params, "apiKey")?;
            let credentials = ProviderCredentials::load(storage)?;
            let api_key = credentials.api_key("opencode", None)?;
            let force_refresh = params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            openai_compatible::models(
                storage,
                api_key.as_ref().map(|secret| secret.as_str()),
                force_refresh,
                cancellation,
            )
            .await
        }
        "compatible.models.list" => {
            let _untrusted_api_key = take_optional_api_key(params, "apiKey")?;
            let provider_id = required_string(params, "providerId")?;
            let force_refresh = params
                .get("forceRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if provider_id.starts_with("custom_") {
                let credentials = ProviderCredentials::load(storage)?;
                let config = credentials.custom_provider_config(provider_id)?;
                let api_key = credentials.api_key(provider_id, Some(provider_id))?;
                openai_compatible::models_for_custom_provider(
                    storage,
                    &config,
                    api_key.as_deref().map(|secret| secret.as_str()),
                    force_refresh,
                    cancellation,
                )
                .await
            } else {
                let copilot_token = if provider_id == "copilot" {
                    Some(service.copilot_access_token().await?)
                } else {
                    None
                };
                let api_key = match copilot_token {
                    Some(token) => token,
                    None => {
                        let credentials = ProviderCredentials::load(storage)?;
                        if provider_id == "kilo" {
                            credentials
                                .api_key(provider_id, Some(provider_id))?
                                .unwrap_or_else(|| zeroize::Zeroizing::new(String::new()))
                        } else {
                            credentials.required_api_key(provider_id, Some(provider_id))?
                        }
                    }
                };
                openai_compatible::models_for_provider(
                    storage,
                    provider_id,
                    api_key.as_str(),
                    force_refresh,
                    cancellation,
                )
                .await
            }
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DesignImageRequest {
    prompt: String,
    model: String,
    size: Option<String>,
    quality: Option<ImageQuality>,
    background: Option<ImageBackground>,
    moderation: Option<ImageModeration>,
    output_format: Option<ImageOutputFormat>,
    output_compression: Option<u8>,
    #[serde(default = "one_image")]
    count: u8,
    auth: DesignImageAuth,
}

#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum DesignImageAuth {
    #[serde(rename = "oauth")]
    OAuth {
        connection_id: String,
        workspace_id: String,
    },
    ApiKey {
        connection_id: String,
    },
}

fn one_image() -> u8 {
    1
}

async fn generate_design_image(
    params: &mut Value,
    storage: &AppStorage,
    service: &ChatGptService,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let input = serde_json::from_value::<DesignImageRequest>(params.clone())
        .map_err(|_| invalid_image_request())?;
    *params = Value::Null;
    let request = ImageGenerationRequest {
        prompt: input.prompt,
        model: input.model,
        size: input.size,
        quality: input.quality,
        background: input.background,
        moderation: input.moderation,
        output_format: input.output_format,
        output_compression: input.output_compression,
        count: input.count,
    };

    let api_key = match &input.auth {
        DesignImageAuth::ApiKey { connection_id } => {
            let credentials = ProviderCredentials::load(storage)?;
            Some(credentials.required_api_key("chatgpt_api", Some(connection_id))?)
        }
        DesignImageAuth::OAuth { .. } => None,
    };
    let (provider_route, connection_id, workspace_id) = match &input.auth {
        DesignImageAuth::OAuth {
            connection_id,
            workspace_id,
        } => (
            "oauth",
            Some(connection_id.as_str()),
            Some(workspace_id.as_str()),
        ),
        DesignImageAuth::ApiKey { connection_id } => {
            ("api_key", Some(connection_id.as_str()), None)
        }
    };
    let generation_id = image_library::begin_generation(
        storage,
        &request,
        "design",
        provider_route,
        connection_id,
        workspace_id,
        None,
        None,
    )?;

    let generated_result = match input.auth {
        DesignImageAuth::OAuth {
            connection_id,
            workspace_id,
        } => {
            service
                .generate_image_with_account_quota(
                    &request,
                    ImageGenerationAuth::ChatGptOAuth {
                        connection_id: &connection_id,
                        workspace_id: &workspace_id,
                    },
                    None,
                    cancellation,
                )
                .await
        }
        DesignImageAuth::ApiKey { .. } => match api_key.as_deref() {
            Some(api_key) => {
                service
                    .generate_image_with_account_quota(
                        &request,
                        ImageGenerationAuth::ApiKey(api_key.as_str()),
                        None,
                        cancellation,
                    )
                    .await
            }
            None => Err(invalid_image_request()),
        },
    };
    let generated = match generated_result {
        Ok(generated) => generated,
        Err(error) => {
            let status = if matches!(error.code, "request_cancelled" | "operation_cancelled") {
                "cancelled"
            } else {
                "failed"
            };
            image_library::finish_generation(storage, &generation_id, status, Some(error.code))?;
            return Err(error);
        }
    };

    let images = match image_library::save_generation(
        storage,
        &generation_id,
        provider_route,
        &request,
        &generated.result,
    ) {
        Ok(images) => images,
        Err(error) => {
            image_library::finish_generation(storage, &generation_id, "failed", Some(error.code))?;
            return Err(error);
        }
    };
    Ok(json!({
        "images": images,
        "accountQuota": generated.account_quota,
        "failedCount": generated.failed_count,
        "failureCode": generated.failure.as_ref().map(|failure| failure.code),
        "generationId": generation_id,
    }))
}

fn invalid_image_request() -> ServiceError {
    ServiceError::new(
        "image_request_invalid",
        "The Design image request is invalid. Check the prompt and selected options.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc};

    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;

    use crate::{chatgpt::ChatGptService, service::OperationGate, storage::AppStorage};

    use super::{DesignImageAuth, DesignImageRequest, dispatch};

    #[test]
    fn design_image_request_accepts_frontend_oauth_payload() {
        let request = json!({
            "prompt": "test image",
            "model": "chatgpt-image-latest",
            "size": "1024x1024",
            "quality": "auto",
            "background": "auto",
            "auth": {
                "kind": "oauth",
                "connectionId": "connection-id",
                "workspaceId": "workspace-id"
            }
        });

        let parsed = serde_json::from_value::<DesignImageRequest>(request)
            .expect("frontend OAuth image request should deserialize");
        let DesignImageAuth::OAuth {
            connection_id,
            workspace_id,
        } = parsed.auth
        else {
            panic!("frontend OAuth image request should use OAuth auth");
        };

        assert_eq!(connection_id, "connection-id");
        assert_eq!(workspace_id, "workspace-id");
    }

    #[tokio::test]
    async fn provider_dispatch_preserves_error_contract_and_removes_renderer_keys() {
        let directory = TestDirectory::new();
        let storage =
            Arc::new(AppStorage::open_at(directory.0.join("data")).expect("open test storage"));
        initialize_test_schema(&storage);
        let service = Arc::new(
            ChatGptService::new(Arc::clone(&storage)).expect("initialize provider service"),
        );
        let (_sender, mut cancellation) = watch::channel(false);

        let mut params = json!({"apiKey": "renderer-only-untrusted"});
        let error = dispatch(
            "chatgpt.api.models.list",
            &mut params,
            &storage,
            &service,
            &mut cancellation,
            &OperationGate::default(),
        )
        .await
        .expect_err("a missing saved connection id is rejected");
        assert_eq!(error.code, "invalid_request_params");
        assert!(params.get("apiKey").is_none());

        let mut params = json!({});
        let error = dispatch(
            "copilot.oauth.poll",
            &mut params,
            &storage,
            &service,
            &mut cancellation,
            &OperationGate::default(),
        )
        .await
        .expect_err("missing flow ids are rejected");
        assert_eq!(error.code, "invalid_request_params");

        let mut params = json!({});
        let error = dispatch(
            "chatgpt.unknown.method",
            &mut params,
            &storage,
            &service,
            &mut cancellation,
            &OperationGate::default(),
        )
        .await
        .expect_err("unknown provider methods are rejected");
        assert_eq!(error.code, "method_not_found");
    }

    fn initialize_test_schema(storage: &AppStorage) {
        let connection = storage.connect().expect("connect test storage");
        connection
            .execute_batch(
                "CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                 CREATE TABLE messages (
                     id TEXT NOT NULL,
                     conversation_id TEXT NOT NULL REFERENCES conversations(id),
                     role TEXT NOT NULL,
                     content TEXT NOT NULL,
                     status TEXT NOT NULL,
                     created_at INTEGER,
                     output_tokens INTEGER,
                     tool_activities TEXT NOT NULL DEFAULT '[]',
                     PRIMARY KEY (conversation_id, id)
                 );",
            )
            .expect("create host conversation schema");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-provider-rpc-test-{}", Uuid::new_v4()));
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
