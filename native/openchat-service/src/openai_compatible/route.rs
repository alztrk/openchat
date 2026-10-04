use rusqlite::OptionalExtension;

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    CHAT_URL, OPENAI_CHAT_URL, RESPONSES_URL, api_compatible_provider,
    authentication_required_error, model_error,
    models::{is_supported_free_chat_model, is_supported_paid_chat_model},
    openai_authentication_required_error, provider_authentication_required_error, route_error,
    storage_error,
};

pub(super) struct ChatRoute {
    pub(super) model_id: String,
    pub(super) provider_id: Option<String>,
    pub(super) chat_url: String,
    pub(super) is_free: bool,
    pub(super) is_opencode: bool,
    pub(super) uses_responses_api: bool,
    pub(super) context_window: Option<i64>,
    pub(super) input_token_limit: Option<i64>,
    pub(super) supports_images: bool,
    pub(super) supports_tool_calls: Option<bool>,
    pub(super) connection_id: Option<String>,
}

impl ChatRoute {
    pub(super) fn request_context_limit(&self) -> Option<i64> {
        self.input_token_limit.or(self.context_window)
    }
}

pub(super) async fn resolve_chat_route(
    storage: &AppStorage,
    conversation_id: &str,
    api_key: Option<&str>,
    requested_api_key_connection_id: Option<&str>,
    stored_api_key_connection_id: Option<&str>,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<ChatRoute, ServiceError> {
    let (provider_id, model_id) = storage
        .connect()
        .map_err(|_| storage_error())?
        .query_row(
            "SELECT provider_id, model_id FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()
        .map_err(|_| storage_error())?
        .ok_or_else(route_error)?;
    let model_id = model_id
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(model_error)?;

    let (
        chat_url,
        is_free,
        uses_responses_api,
        context_window,
        input_token_limit,
        supports_images,
        supports_tool_calls,
        connection_id,
    ) = match provider_id.as_deref() {
        Some("opencode") => {
            if requested_api_key_connection_id.is_some() {
                return Err(route_error());
            }
            if !is_supported_free_chat_model(&model_id) && !is_supported_paid_chat_model(&model_id)
            {
                return Err(model_error());
            }
            let is_free = is_supported_free_chat_model(&model_id);
            let uses_responses_api = super::models::is_responses_api_model(storage, &model_id)?;
            let supports_tool_calls = super::models::supports_tool_calls(storage, &model_id)?;
            let (context_window, input_token_limit) =
                super::models::context_limits(storage, &model_id)?;
            let supports_images = super::models::supports_image_input(storage, &model_id)?;
            let chat_url = if uses_responses_api {
                RESPONSES_URL.to_owned()
            } else {
                CHAT_URL.to_owned()
            };
            (
                chat_url,
                is_free,
                uses_responses_api,
                context_window,
                input_token_limit,
                supports_images,
                supports_tool_calls,
                None,
            )
        }
        Some("llama_cpp") => {
            if requested_api_key_connection_id.is_some()
                || stored_api_key_connection_id.is_some()
                || api_key.is_some()
            {
                return Err(route_error());
            }
            let endpoint = crate::local_engines::chat_url(storage, &model_id, cancellation).await?;
            (
                endpoint.chat_url,
                true,
                false,
                Some(endpoint.capabilities.context_window),
                None,
                endpoint.capabilities.supports_images,
                endpoint.capabilities.supports_tool_calls,
                None,
            )
        }
        Some("vllm" | "exllama") => return Err(local_engine_unavailable_error()),
        Some("chatgpt_api") => {
            if requested_api_key_connection_id != stored_api_key_connection_id {
                return Err(route_error());
            }
            if api_key.is_none() {
                return Err(openai_authentication_required_error());
            }
            (
                OPENAI_CHAT_URL.to_owned(),
                false,
                false,
                None,
                None,
                crate::openai_api::supports_image_input(&model_id),
                None,
                stored_api_key_connection_id.map(str::to_owned),
            )
        }
        Some(id) => {
            let Some(provider) = api_compatible_provider(id) else {
                return Err(route_error());
            };
            if requested_api_key_connection_id != Some(id)
                || stored_api_key_connection_id != Some(id)
            {
                return Err(route_error());
            }
            let api_key = api_key.ok_or_else(|| provider_authentication_required_error(id))?;
            let (context_window, input_token_limit) =
                super::provider_models::context_limits(storage, id, api_key, &model_id)?;
            let supports_images =
                super::provider_models::supports_image_input(storage, id, api_key, &model_id)?;
            let supports_tool_calls =
                super::provider_models::supports_tool_calls(storage, id, api_key, &model_id)?;
            (
                format!("{}/chat/completions", provider.base_url),
                false,
                false,
                context_window,
                input_token_limit,
                supports_images,
                supports_tool_calls,
                Some(id.to_owned()),
            )
        }
        None => return Err(route_error()),
    };

    if !is_free && api_key.is_none() {
        return Err(authentication_required_error());
    }

    Ok(ChatRoute {
        model_id,
        is_opencode: provider_id.as_deref() == Some("opencode"),
        provider_id,
        chat_url,
        is_free,
        uses_responses_api,
        context_window,
        input_token_limit,
        supports_images,
        supports_tool_calls,
        connection_id,
    })
}

fn local_engine_unavailable_error() -> ServiceError {
    ServiceError::new(
        "local_engine_unavailable",
        "The selected local inference engine is not ready on this device.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::ChatRoute;

    fn route(context_window: Option<i64>, input_token_limit: Option<i64>) -> ChatRoute {
        ChatRoute {
            model_id: "model".to_owned(),
            provider_id: None,
            chat_url: String::new(),
            is_free: false,
            is_opencode: false,
            uses_responses_api: false,
            context_window,
            input_token_limit,
            supports_images: false,
            supports_tool_calls: None,
            connection_id: None,
        }
    }

    #[test]
    fn request_budget_prefers_explicit_input_limit_and_falls_back_to_context() {
        assert_eq!(
            route(Some(262_144), Some(131_072)).request_context_limit(),
            Some(131_072)
        );
        assert_eq!(
            route(Some(262_144), None).request_context_limit(),
            Some(262_144)
        );
        assert_eq!(
            route(None, Some(131_072)).request_context_limit(),
            Some(131_072)
        );
        assert_eq!(route(None, None).request_context_limit(), None);
    }
}
