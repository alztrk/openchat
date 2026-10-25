//! Resolves persisted and temporary chat routes to validated provider settings.
//!
//! Credentials are obtained through the credential boundary and are not stored
//! in the route's database metadata.

use rusqlite::OptionalExtension;
use zeroize::Zeroizing;

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, sqlite_error_code},
};

use super::{
    OPENAI_RESPONSES_URL, api_compatible_provider, authentication_required_error,
    log_send_storage_failure, model_error, openai_authentication_required_error,
    provider_authentication_required_error, route_error, storage_error,
};

#[derive(Clone)]
pub(super) struct ChatRoute {
    pub(super) model_id: String,
    pub(super) provider_model_id: String,
    pub(super) provider_id: Option<String>,
    pub(super) chat_url: String,
    pub(super) is_free: bool,
    pub(super) is_opencode: bool,
    pub(super) is_copilot: bool,
    pub(super) uses_responses_api: bool,
    pub(super) reasoning_levels: Vec<String>,
    pub(super) custom_reasoning_mode: Option<String>,
    pub(super) context_window: Option<i64>,
    pub(super) input_token_limit: Option<i64>,
    pub(super) max_output_tokens: Option<i64>,
    pub(super) context_limit_override: Option<i64>,
    pub(super) supports_images: bool,
    pub(super) supports_tool_calls: Option<bool>,
    pub(super) connection_id: Option<String>,
    pub(super) local_api_key: Option<Zeroizing<String>>,
    pub(super) project_id: Option<String>,
}

impl ChatRoute {
    pub(super) fn is_anthropic(&self) -> bool {
        self.chat_url.ends_with("/messages")
    }

    pub(super) fn request_context_limit(&self) -> Option<i64> {
        self.context_limit_override
            .map(|limit| {
                self.input_token_limit
                    .or(self.context_window)
                    .map_or(limit, |max| limit.min(max))
            })
            .or(self.input_token_limit)
            .or(self.context_window)
    }
}

/// Shared authentication and context inputs for provider route resolution.
#[derive(Clone, Copy)]
pub(super) struct ChatRouteOptions<'a> {
    /// API key selected for the provider request.
    pub(super) api_key: Option<&'a str>,
    /// Connection id selected for this request, when the provider requires one.
    pub(super) requested_api_key_connection_id: Option<&'a str>,
    /// Connection id saved on the durable conversation.
    pub(super) stored_api_key_connection_id: Option<&'a str>,
    /// Validated configuration for the selected custom provider.
    pub(super) custom_provider: Option<&'a super::CustomProviderConfig>,
    /// User-selected context cap, if one is configured.
    pub(super) context_limit_override: Option<i64>,
}

/// Provider-specific values resolved before constructing a chat route.
struct ResolvedProviderRoute {
    provider_model_id: String,
    chat_url: String,
    is_free: bool,
    uses_responses_api: bool,
    limits: super::ModelContextLimits,
    supports_images: bool,
    supports_tool_calls: Option<bool>,
    connection_id: Option<String>,
    local_api_key: Option<Zeroizing<String>>,
    reasoning_levels: Vec<String>,
}

pub(super) async fn resolve_chat_route(
    storage: &AppStorage,
    conversation_id: &str,
    options: ChatRouteOptions<'_>,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<ChatRoute, ServiceError> {
    let (provider_id, model_id, project_id) = {
        let connection = storage.connect().map_err(|error| {
            log_send_storage_failure(
                storage,
                "chat.send.compatible_route_open",
                sqlite_error_code(&error),
            );
            storage_error()
        })?;
        connection
            .query_row(
                "SELECT provider_id, model_id, project_id FROM conversations WHERE id = ?1",
                [conversation_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| {
                log_send_storage_failure(
                    storage,
                    "chat.send.compatible_route_read",
                    sqlite_error_code(&error),
                );
                storage_error()
            })?
            .ok_or_else(route_error)?
    };
    resolve_chat_route_for_model(
        storage,
        provider_id,
        model_id,
        project_id.as_deref(),
        options,
        cancellation,
    )
    .await
    .map_err(|error| {
        if error.code == "storage_unavailable" {
            log_send_storage_failure(storage, "chat.send.compatible_route_metadata", error.code);
        }
        error
    })
}

pub(super) async fn resolve_temporary_chat_route(
    storage: &AppStorage,
    provider_id: &str,
    model_id: &str,
    options: ChatRouteOptions<'_>,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<ChatRoute, ServiceError> {
    if provider_id == "chatgpt" {
        return Err(ServiceError::new(
            "temporary_provider_unsupported",
            "Temporary conversations are not available for this provider yet.",
            false,
        ));
    }
    resolve_chat_route_for_model(
        storage,
        Some(provider_id.to_owned()),
        Some(model_id.to_owned()),
        None,
        options,
        cancellation,
    )
    .await
    .map_err(|error| {
        if error.code == "storage_unavailable" {
            log_send_storage_failure(
                storage,
                "chat.temporary.send.compatible_route_metadata",
                error.code,
            );
        }
        error
    })
}

async fn resolve_chat_route_for_model(
    storage: &AppStorage,
    provider_id: Option<String>,
    model_id: Option<String>,
    project_id: Option<&str>,
    options: ChatRouteOptions<'_>,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<ChatRoute, ServiceError> {
    let ChatRouteOptions {
        api_key,
        requested_api_key_connection_id,
        stored_api_key_connection_id,
        custom_provider,
        context_limit_override,
    } = options;
    let model_id = model_id
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(model_error)?;

    let resolved = match provider_id.as_deref() {
        Some("opencode") => {
            if requested_api_key_connection_id.is_some() {
                return Err(route_error());
            }
            let is_free = super::models::is_free_model(storage, &model_id)?;
            if !is_free && !super::models::is_paid_model(storage, &model_id)? {
                return Err(model_error());
            }
            let uses_responses_api = super::models::is_responses_api_model(storage, &model_id)?;
            let chat_url = super::models::inference_url(storage, &model_id, uses_responses_api)?;
            let supports_tool_calls = super::models::supports_tool_calls(storage, &model_id)?;
            let limits = super::models::context_limits(storage, &model_id)?;
            let supports_images = super::models::supports_image_input(storage, &model_id)?;
            ResolvedProviderRoute {
                provider_model_id: model_id.clone(),
                chat_url,
                is_free,
                uses_responses_api,
                limits,
                supports_images,
                supports_tool_calls,
                connection_id: None,
                local_api_key: None,
                reasoning_levels: Vec::new(),
            }
        }
        Some("llama_cpp") => {
            if requested_api_key_connection_id.is_some()
                || stored_api_key_connection_id.is_some()
                || api_key.is_some()
            {
                return Err(route_error());
            }
            let endpoint = crate::local_engines::chat_url(storage, &model_id, cancellation).await?;
            ResolvedProviderRoute {
                provider_model_id: endpoint.model_id,
                chat_url: endpoint.chat_url,
                is_free: true,
                uses_responses_api: false,
                limits: super::ModelContextLimits {
                    context_window: endpoint.capabilities.context_window,
                    input_token_limit: None,
                    max_output_tokens: None,
                },
                supports_images: endpoint.capabilities.supports_images,
                supports_tool_calls: endpoint.capabilities.supports_tool_calls,
                connection_id: None,
                local_api_key: endpoint.api_key,
                reasoning_levels: Vec::new(),
            }
        }
        Some("chatgpt_api") => {
            if requested_api_key_connection_id != stored_api_key_connection_id {
                return Err(route_error());
            }
            if api_key.is_none() {
                return Err(openai_authentication_required_error());
            }
            ResolvedProviderRoute {
                provider_model_id: model_id.clone(),
                chat_url: OPENAI_RESPONSES_URL.to_owned(),
                is_free: false,
                uses_responses_api: true,
                limits: super::ModelContextLimits::default(),
                supports_images: crate::openai_api::supports_image_input(
                    storage, api_key, &model_id,
                ),
                supports_tool_calls: None,
                connection_id: stored_api_key_connection_id.map(str::to_owned),
                local_api_key: None,
                reasoning_levels: Vec::new(),
            }
        }
        Some("copilot") => {
            if requested_api_key_connection_id != Some("copilot")
                || stored_api_key_connection_id != Some("copilot")
            {
                return Err(route_error());
            }
            if api_key.is_none() {
                return Err(provider_authentication_required_error("copilot"));
            }
            let Some(copilot_route) = super::copilot::model_route(storage, "copilot", &model_id)?
            else {
                return Err(model_error());
            };
            ResolvedProviderRoute {
                provider_model_id: model_id.clone(),
                chat_url: copilot_route.chat_url,
                is_free: false,
                uses_responses_api: copilot_route.uses_responses_api,
                limits: super::ModelContextLimits {
                    context_window: copilot_route.context_window,
                    input_token_limit: copilot_route.input_token_limit,
                    max_output_tokens: copilot_route.max_output_tokens,
                },
                supports_images: copilot_route.supports_images,
                supports_tool_calls: copilot_route.supports_tool_calls,
                connection_id: Some("copilot".to_owned()),
                local_api_key: None,
                reasoning_levels: copilot_route.reasoning_levels,
            }
        }
        Some(id) if id.starts_with("custom_") => {
            let Some(provider) = custom_provider.filter(|provider| provider.id == id) else {
                return Err(route_error());
            };
            if requested_api_key_connection_id != Some(id)
                || stored_api_key_connection_id != Some(id)
            {
                return Err(route_error());
            }
            let limits = super::provider_models::custom_context_limits(
                storage, provider, api_key, &model_id,
            )?;
            ResolvedProviderRoute {
                provider_model_id: model_id.clone(),
                chat_url: if provider.is_anthropic() {
                    format!("{}/messages", provider.base_url)
                } else {
                    format!("{}/chat/completions", provider.base_url)
                },
                is_free: false,
                uses_responses_api: false,
                limits,
                supports_images: false,
                supports_tool_calls: None,
                connection_id: Some(id.to_owned()),
                local_api_key: None,
                reasoning_levels: match provider.reasoning_mode.as_str() {
                    "reasoningEffort" => provider.reasoning_levels.clone(),
                    "chatTemplateThinking" => vec!["enabled".to_owned()],
                    _ => Vec::new(),
                },
            }
        }
        Some(id) => {
            let Some(provider) = api_compatible_provider(id) else {
                return Err(route_error());
            };
            let is_kilo_free = id == "kilo"
                && (model_id.ends_with(":free")
                    || model_id.contains("/free")
                    || model_id == "kilo-auto/free");
            if !is_kilo_free
                && (requested_api_key_connection_id != Some(id)
                    || stored_api_key_connection_id != Some(id))
            {
                return Err(route_error());
            }
            if !is_kilo_free && api_key.is_none() {
                return Err(provider_authentication_required_error(id));
            }
            let key = api_key.unwrap_or("");
            let limits = super::provider_models::context_limits(storage, id, key, &model_id)?;
            let supports_images =
                super::provider_models::supports_image_input(storage, id, key, &model_id)?;
            let supports_tool_calls =
                super::provider_models::supports_tool_calls(storage, id, key, &model_id)?;
            ResolvedProviderRoute {
                provider_model_id: model_id.clone(),
                chat_url: if provider.is_anthropic() {
                    format!("{}/messages", provider.base_url)
                } else {
                    format!("{}/chat/completions", provider.base_url)
                },
                is_free: is_kilo_free,
                uses_responses_api: false,
                limits,
                supports_images,
                supports_tool_calls,
                connection_id: Some(id.to_owned()),
                local_api_key: None,
                reasoning_levels: Vec::new(),
            }
        }
        None => return Err(route_error()),
    };

    if !resolved.is_free
        && api_key.is_none()
        && !provider_id
            .as_deref()
            .is_some_and(|provider_id| provider_id.starts_with("custom_"))
    {
        return Err(authentication_required_error());
    }

    let is_copilot = provider_id.as_deref() == Some("copilot");
    let custom_reasoning_mode = custom_provider
        .filter(|provider| provider_id.as_deref() == Some(provider.id.as_str()))
        .map(|provider| provider.reasoning_mode.clone());

    Ok(ChatRoute {
        model_id,
        provider_model_id: resolved.provider_model_id,
        is_opencode: provider_id.as_deref() == Some("opencode"),
        provider_id,
        chat_url: resolved.chat_url,
        is_free: resolved.is_free,
        uses_responses_api: resolved.uses_responses_api,
        is_copilot,
        context_window: resolved.limits.context_window,
        reasoning_levels: resolved.reasoning_levels,
        custom_reasoning_mode,
        input_token_limit: resolved.limits.input_token_limit,
        max_output_tokens: resolved.limits.max_output_tokens,
        context_limit_override,
        supports_images: resolved.supports_images,
        supports_tool_calls: resolved.supports_tool_calls,
        connection_id: resolved.connection_id,
        local_api_key: resolved.local_api_key,
        project_id: project_id.map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::ChatRoute;

    fn route(
        context_window: Option<i64>,
        input_token_limit: Option<i64>,
        context_limit_override: Option<i64>,
    ) -> ChatRoute {
        ChatRoute {
            model_id: "model".to_owned(),
            provider_model_id: "model".to_owned(),
            provider_id: None,
            chat_url: String::new(),
            is_free: false,
            is_opencode: false,
            is_copilot: false,
            uses_responses_api: false,
            reasoning_levels: Vec::new(),
            custom_reasoning_mode: None,
            context_window,
            input_token_limit,
            max_output_tokens: None,
            context_limit_override,
            supports_images: false,
            supports_tool_calls: None,
            connection_id: None,
            local_api_key: None,
            project_id: None,
        }
    }

    #[test]
    fn request_budget_prefers_explicit_input_limit_and_falls_back_to_context() {
        assert_eq!(
            route(Some(262_144), Some(131_072), None).request_context_limit(),
            Some(131_072)
        );
        assert_eq!(
            route(Some(262_144), None, None).request_context_limit(),
            Some(262_144)
        );
        assert_eq!(
            route(None, Some(131_072), None).request_context_limit(),
            Some(131_072)
        );
        assert_eq!(route(None, None, None).request_context_limit(), None);
        assert_eq!(
            route(Some(262_144), Some(250_000), Some(180_000)).request_context_limit(),
            Some(180_000)
        );
        assert_eq!(
            route(Some(262_144), Some(131_072), Some(180_000)).request_context_limit(),
            Some(131_072)
        );
    }
}
