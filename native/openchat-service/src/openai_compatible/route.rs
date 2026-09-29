use rusqlite::OptionalExtension;

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    CHAT_URL, OPENAI_CHAT_URL, api_compatible_provider, authentication_required_error, model_error,
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
}

pub(super) fn resolve_chat_route(
    storage: &AppStorage,
    conversation_id: &str,
    api_key: Option<&str>,
    requested_api_key_connection_id: Option<&str>,
    stored_api_key_connection_id: Option<&str>,
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

    let (chat_url, is_free) = match provider_id.as_deref() {
        Some("opencode") => {
            if requested_api_key_connection_id.is_some() {
                return Err(route_error());
            }
            if !is_supported_free_chat_model(&model_id) && !is_supported_paid_chat_model(&model_id)
            {
                return Err(model_error());
            }
            (CHAT_URL.to_owned(), is_supported_free_chat_model(&model_id))
        }
        Some("chatgpt_api") => {
            if requested_api_key_connection_id != stored_api_key_connection_id {
                return Err(route_error());
            }
            if api_key.is_none() {
                return Err(openai_authentication_required_error());
            }
            (OPENAI_CHAT_URL.to_owned(), false)
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
            if api_key.is_none() {
                return Err(provider_authentication_required_error(id));
            }
            (format!("{}/chat/completions", provider.base_url), false)
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
    })
}
