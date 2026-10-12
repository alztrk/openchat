use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Method;
use serde_json::{Value, json};
use tokio::sync::watch;
use zeroize::Zeroizing;

use crate::{
    chat_operation::TitleGenerationModel,
    chatgpt_store::{self, StoredMessage},
    protocol::{EventSink, Response as RpcResponse},
    storage::AppStorage,
};

use super::{
    CustomProviderConfig, OPENAI_CHAT_URL, RESPONSES_URL, api_compatible_provider, client,
};

struct TitleRoute {
    model_id: String,
    url: String,
    api_key: Option<Zeroizing<String>>,
    anthropic: bool,
    responses_api: bool,
}

const MAX_TITLE_RESPONSE_BYTES: usize = 64 * 1024;

pub(crate) async fn generate_title_for_target(
    storage: &AppStorage,
    conversation_id: &str,
    target: TitleGenerationModel,
    excluded_assistant_message_id: Option<&str>,
    events: EventSink,
) {
    let route = match resolve_target(storage, target).await {
        Ok(route) => route,
        Err(()) => return,
    };
    generate_title(
        storage,
        conversation_id,
        route,
        excluded_assistant_message_id,
        events,
    )
    .await;
}

pub(crate) async fn generate_title_for_route(
    storage: &AppStorage,
    conversation_id: &str,
    model_id: String,
    url: String,
    api_key: Option<String>,
    anthropic: bool,
    responses_api: bool,
    excluded_assistant_message_id: Option<&str>,
    events: EventSink,
) {
    generate_title(
        storage,
        conversation_id,
        TitleRoute {
            model_id,
            url,
            api_key: api_key.map(Zeroizing::new),
            anthropic,
            responses_api,
        },
        excluded_assistant_message_id,
        events,
    )
    .await;
}

async fn resolve_target(
    storage: &AppStorage,
    target: TitleGenerationModel,
) -> Result<TitleRoute, ()> {
    let api_key = target.api_key;
    match target.provider_id.as_str() {
        "chatgpt" => Err(()),
        "chatgpt_api" => {
            if target.connection_id.as_deref() != target.api_key_connection_id.as_deref()
                || api_key.is_none()
            {
                return Err(());
            }
            Ok(TitleRoute {
                model_id: target.model_id,
                url: OPENAI_CHAT_URL.to_owned(),
                api_key,
                anthropic: false,
                responses_api: false,
            })
        }
        "opencode" => {
            if !super::models::is_supported_free_chat_model(&target.model_id)
                && !super::models::is_supported_paid_chat_model(&target.model_id)
            {
                return Err(());
            }
            let responses_api =
                super::models::is_responses_api_model(storage, &target.model_id).map_err(|_| ())?;
            Ok(TitleRoute {
                model_id: target.model_id,
                url: if responses_api {
                    RESPONSES_URL
                } else {
                    "https://opencode.ai/zen/v1/chat/completions"
                }
                .to_owned(),
                api_key,
                anthropic: false,
                responses_api,
            })
        }
        "llama_cpp" | "vllm" | "exllama" => {
            let (_sender, mut cancellation) = watch::channel(false);
            let endpoint =
                crate::local_engines::chat_url(storage, &target.model_id, &mut cancellation)
                    .await
                    .map_err(|_| ())?;
            Ok(TitleRoute {
                model_id: endpoint.model_id,
                url: endpoint.chat_url,
                api_key: endpoint.api_key,
                anthropic: false,
                responses_api: false,
            })
        }
        provider_id if provider_id.starts_with("custom_") => {
            let provider: CustomProviderConfig = target.custom_provider.ok_or(())?;
            if provider.id != provider_id
                || target
                    .api_key_connection_id
                    .as_deref()
                    .is_some_and(|id| id != provider_id)
            {
                return Err(());
            }
            let anthropic = provider.is_anthropic();
            let suffix = if anthropic {
                "/messages"
            } else {
                "/chat/completions"
            };
            Ok(TitleRoute {
                model_id: target.model_id,
                url: format!("{}{suffix}", provider.base_url),
                api_key,
                anthropic,
                responses_api: false,
            })
        }
        provider_id => {
            let provider = api_compatible_provider(provider_id).ok_or(())?;
            if target.api_key_connection_id.as_deref() != Some(provider_id) || api_key.is_none() {
                return Err(());
            }
            Ok(TitleRoute {
                model_id: target.model_id,
                url: format!("{}/chat/completions", provider.base_url),
                api_key,
                anthropic: false,
                responses_api: false,
            })
        }
    }
}

async fn generate_title(
    storage: &AppStorage,
    conversation_id: &str,
    route: TitleRoute,
    excluded_assistant_message_id: Option<&str>,
    events: EventSink,
) {
    let Ok(messages) = chatgpt_store::conversation_messages(storage, conversation_id) else {
        return;
    };
    let prompt = title_prompt(&messages, excluded_assistant_message_id);
    if prompt.is_empty() {
        return;
    }
    let Ok(client) = client() else { return };
    let request = if route.responses_api {
        client.request(Method::POST, &route.url)
            .json(&json!({"model": route.model_id, "input": prompt, "stream": false, "store": false, "max_output_tokens": 64}))
    } else if route.anthropic {
        let mut request = client.request(Method::POST, &route.url)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({"model": route.model_id, "max_tokens": 64, "system": "Generate a concise title for the conversation. Return only the title, without quotes.", "messages": [{"role": "user", "content": prompt}]}));
        if let Some(key) = route.api_key.as_deref() {
            request = request.header("x-api-key", key);
        }
        request
    } else {
        let mut request = client.request(Method::POST, &route.url)
            .json(&json!({"model": route.model_id, "messages": [
                {"role": "system", "content": "Generate a concise title for the conversation. Return only the title, without quotes."},
                {"role": "user", "content": prompt}
            ], "stream": false, "max_tokens": 64}));
        if let Some(key) = route.api_key.as_deref() {
            request = request.bearer_auth(key);
        }
        request
    };
    let Ok(Ok(response)) = tokio::time::timeout(Duration::from_secs(30), request.send()).await
    else {
        return;
    };
    if !response.status().is_success() {
        return;
    }
    let mut response_body = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let Ok(chunk) = chunk else { return };
        if response_body.len().saturating_add(chunk.len()) > MAX_TITLE_RESPONSE_BYTES {
            return;
        }
        response_body.extend_from_slice(&chunk);
    }
    let Ok(body) = serde_json::from_slice::<Value>(&response_body) else {
        return;
    };
    let Some(title) = extract_title(&body, route.responses_api, route.anthropic) else {
        return;
    };
    let Ok(true) = chatgpt_store::apply_generated_title(storage, conversation_id, &title) else {
        return;
    };
    let _ = events
        .send(&RpcResponse::event(
            json!(0),
            "chat.title_updated",
            json!({"conversationId": conversation_id, "title": title}),
        ))
        .await;
}

fn title_prompt(messages: &[StoredMessage], excluded_id: Option<&str>) -> String {
    let Some(latest_user_index) = messages.iter().rposition(|message| message.role == "user")
    else {
        return String::new();
    };
    let mut selected = messages
        .iter()
        .take(latest_user_index + 1)
        .filter(|message| Some(message.id.as_str()) != excluded_id)
        .filter(|message| matches!(message.role.as_str(), "user" | "assistant"))
        .rev()
        .take(8)
        .collect::<Vec<_>>();
    selected.reverse();
    selected
        .into_iter()
        .filter_map(|message| {
            let content = message.content.trim();
            if content.is_empty() {
                return None;
            }
            let content = content.chars().take(1800).collect::<String>();
            Some(format!("{}: {}", message.role, content))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn extract_title(body: &Value, responses_api: bool, anthropic: bool) -> Option<String> {
    let text = if responses_api {
        body.get("output_text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                body.get("output")?
                    .as_array()?
                    .iter()
                    .flat_map(|item| {
                        item.get("content")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                    })
                    .find_map(|part| part.get("text").and_then(Value::as_str).map(str::to_owned))
            })?
    } else if anthropic {
        body.get("content")?
            .as_array()?
            .iter()
            .find_map(|part| part.get("text").and_then(Value::as_str).map(str::to_owned))?
    } else {
        body.get("choices")?
            .as_array()?
            .first()?
            .get("message")?
            .get("content")?
            .as_str()?
            .to_owned()
    };
    let title = text.trim().trim_matches(['"', '\'', '`']).trim();
    if title.is_empty() || title.chars().any(char::is_control) {
        return None;
    }
    Some(title.chars().take(120).collect())
}
