//! Generates bounded conversation titles through OpenAI-compatible routes.
//!
//! Title requests use explicit route credentials, cancellation, and response
//! limits so they do not alter the main chat response lifecycle.

use std::{path::Path, time::Duration};

use futures_util::StreamExt;
use reqwest::Method;
use serde_json::{Value, json};
use tokio::sync::watch;
use zeroize::Zeroizing;

use crate::response_budget::MAX_TITLE_RESPONSE_BYTES;
use crate::{
    chat_operation::TitleGenerationModel,
    chatgpt::ChatGptService,
    chatgpt_store::{self, StoredMessage},
    domain::http_headers::sensitive_api_key_header,
    protocol::{EventSink, Response as RpcResponse},
    storage::AppStorage,
};

use super::{
    CustomProviderConfig, OPENAI_RESPONSES_URL, api_compatible_provider,
    apply_opencode_free_request_shape, apply_opencode_headers, client, copilot,
    opencode_free_headers, opencode_free_transport, opencode_project_id,
    opencode_session_id_for_conversation,
    stream::{SseLine, parse_sse_line, pop_sse_line},
};

struct TitleRoute {
    model_id: String,
    url: String,
    api_key: Option<Zeroizing<String>>,
    anthropic: bool,
    copilot: bool,
    responses_api: bool,
    opencode: bool,
    is_free: bool,
}

/// Holds the selected provider route settings for one automatic title request.
pub(crate) struct TitleRouteRequest {
    pub(crate) model_id: String,
    pub(crate) url: String,
    pub(crate) api_key: Option<String>,
    pub(crate) anthropic: bool,
    pub(crate) copilot: bool,
    pub(crate) responses_api: bool,
    pub(crate) opencode: bool,
    pub(crate) is_free: bool,
}

pub(crate) async fn generate_title_for_target(
    service: &ChatGptService,
    storage: &AppStorage,
    conversation_id: &str,
    target: TitleGenerationModel,
    excluded_assistant_message_id: Option<&str>,
    events: EventSink,
) {
    let route = match resolve_target(service, storage, target).await {
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
    service: &ChatGptService,
    storage: &AppStorage,
    conversation_id: &str,
    route: TitleRouteRequest,
    excluded_assistant_message_id: Option<&str>,
    events: EventSink,
) {
    let TitleRouteRequest {
        model_id,
        url,
        api_key,
        anthropic,
        copilot,
        responses_api,
        opencode,
        is_free,
    } = route;
    let api_key = if copilot {
        match service.copilot_access_token().await {
            Ok(token) => Some(token),
            Err(_) => return,
        }
    } else {
        api_key.map(Zeroizing::new)
    };
    generate_title(
        storage,
        conversation_id,
        TitleRoute {
            model_id,
            url,
            api_key,
            anthropic,
            copilot,
            responses_api,
            opencode,
            is_free,
        },
        excluded_assistant_message_id,
        events,
    )
    .await;
}

async fn resolve_target(
    service: &ChatGptService,
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
                url: OPENAI_RESPONSES_URL.to_owned(),
                api_key,
                anthropic: false,
                copilot: false,
                responses_api: true,
                opencode: false,
                is_free: false,
            })
        }
        "opencode" => {
            let metadata = super::models::route_model_metadata(storage, &target.model_id)
                .map_err(|_| ())?
                .ok_or(())?;
            if !metadata.is_free && !metadata.is_paid {
                return Err(());
            }
            let inference_base_url = metadata.inference_base_url.ok_or(())?;
            let responses_api = metadata.uses_responses_api;
            let endpoint = if responses_api {
                "responses"
            } else {
                "chat/completions"
            };
            Ok(TitleRoute {
                model_id: target.model_id,
                url: format!("{inference_base_url}/{endpoint}"),
                api_key,
                anthropic: false,
                copilot: false,
                responses_api,
                opencode: true,
                is_free: metadata.is_free,
            })
        }
        "llama_cpp" => {
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
                copilot: false,
                responses_api: false,
                opencode: false,
                is_free: false,
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
                copilot: false,
                responses_api: false,
                opencode: false,
                is_free: false,
            })
        }
        "copilot" => {
            if target.api_key_connection_id.as_deref() != Some("copilot") {
                return Err(());
            }
            let api_key = service.copilot_access_token().await.map_err(|_| ())?;
            let model_route = copilot::model_route(storage, "copilot", &target.model_id)
                .map_err(|_| ())?
                .ok_or(())?;
            let anthropic = model_route.chat_url.ends_with("/v1/messages");
            Ok(TitleRoute {
                model_id: target.model_id,
                url: model_route.chat_url,
                api_key: Some(api_key),
                anthropic,
                copilot: true,
                responses_api: model_route.uses_responses_api,
                opencode: false,
                is_free: false,
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
                copilot: false,
                responses_api: false,
                opencode: false,
                is_free: false,
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
    if chatgpt_store::is_custom_title(storage, conversation_id).unwrap_or(true) {
        return;
    }
    let Ok(messages) = chatgpt_store::conversation_messages(storage, conversation_id) else {
        return;
    };
    let prompt = title_prompt(&messages, excluded_assistant_message_id);
    if prompt.is_empty() {
        return;
    }
    // The free tier rejects the reqwest client's TLS fingerprint, non-streaming
    // requests, and requests without an environment block and a `read` tool, so
    // it needs the dedicated transport and a streamed reply.
    if route.opencode && route.is_free {
        let project_id = opencode_project_id(storage, conversation_id);
        let workspace =
            crate::workspace_settings::conversation_workspace_path(storage, conversation_id);
        if let Some(title) = generate_free_opencode_title(
            conversation_id,
            &route,
            project_id.as_deref(),
            &prompt,
            workspace.as_deref(),
        )
        .await
        {
            apply_title(storage, conversation_id, &title, events).await;
        }
        return;
    }
    let Ok(client) = (if route.copilot {
        copilot::client()
    } else {
        client()
    }) else {
        return;
    };
    let opencode_session_id = route
        .opencode
        .then(|| opencode_session_id_for_conversation(conversation_id, 0));
    let opencode_project_id = route
        .opencode
        .then(|| opencode_project_id(storage, conversation_id))
        .flatten();
    let request = if route.responses_api {
        let mut request = client.request(Method::POST, &route.url)
            .json(&json!({"model": route.model_id, "input": prompt, "stream": false, "store": false, "max_output_tokens": 64}));
        if route.copilot {
            let Some(key) = route.api_key.as_deref() else {
                return;
            };
            request =
                copilot::authorize_inference(request, key, copilot::RequestIntent::Background);
        }
        request
    } else if route.anthropic {
        let mut request = client.request(Method::POST, &route.url)
            .json(&json!({"model": route.model_id, "max_tokens": 64, "system": "Generate a concise title for the conversation. Return only the title, without quotes.", "messages": [{"role": "user", "content": prompt}]}));
        if route.copilot {
            if let Some(key) = route.api_key.as_deref() {
                request =
                    copilot::authorize_inference(request, key, copilot::RequestIntent::Background);
            } else {
                return;
            }
        } else {
            request = request.header("anthropic-version", "2023-06-01");
            if let Some(key) = route.api_key.as_deref() {
                let Ok(key) = sensitive_api_key_header(key) else {
                    return;
                };
                request = request.header("x-api-key", key);
            }
        }
        request
    } else {
        let mut request = client.request(Method::POST, &route.url)
            .json(&json!({"model": route.model_id, "messages": [
                {"role": "system", "content": "Generate a concise title for the conversation. Return only the title, without quotes."},
                {"role": "user", "content": prompt}
            ], "stream": false, "max_tokens": 64}));
        if route.copilot {
            if let Some(key) = route.api_key.as_deref() {
                request =
                    copilot::authorize_inference(request, key, copilot::RequestIntent::Background);
            } else {
                return;
            }
        } else if route.opencode {
            let Some(session_id) = opencode_session_id.as_deref() else {
                return;
            };
            request = apply_opencode_headers(request, session_id, opencode_project_id.as_deref());
            if route.is_free {
                request = request.bearer_auth("public");
            } else if let Some(key) = route.api_key.as_deref() {
                request = request.bearer_auth(key);
            }
        } else if let Some(key) = route.api_key.as_deref() {
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
    apply_title(storage, conversation_id, &title, events).await;
}

async fn apply_title(storage: &AppStorage, conversation_id: &str, title: &str, events: EventSink) {
    let Ok(true) = chatgpt_store::apply_generated_title(storage, conversation_id, title) else {
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

/// Streams a title through the OpenCode free tier and assembles its text.
async fn generate_free_opencode_title(
    conversation_id: &str,
    route: &TitleRoute,
    project_id: Option<&str>,
    prompt: &str,
    workspace: Option<&Path>,
) -> Option<String> {
    let mut body = json!({
        "model": route.model_id,
        "messages": [
            {"role": "system", "content": "Generate a concise title for the conversation. Return only the title, without quotes."},
            {"role": "user", "content": prompt}
        ],
        // Reasoning models spend part of the completion budget on hidden
        // reasoning, so a small cap can leave the visible title empty.
        "max_tokens": 512,
        "tool_choice": "none"
    });
    apply_opencode_free_request_shape(&mut body, "opencode", true, workspace);
    let body = serde_json::to_vec(&body).ok()?;
    let session_id = opencode_session_id_for_conversation(conversation_id, 0);
    let headers = opencode_free_headers(Some(&session_id), project_id);
    // Free models are served by the chat-completions endpoint even when the
    // catalog marks a model as using the Responses API.
    let url = route
        .url
        .strip_suffix("/responses")
        .map(|base| format!("{base}/chat/completions"))
        .unwrap_or_else(|| route.url.clone());
    let Ok(Ok(mut response)) = tokio::time::timeout(
        Duration::from_secs(30),
        opencode_free_transport::send(&url, headers, body),
    )
    .await
    else {
        return None;
    };
    if response.status != http::StatusCode::OK {
        return None;
    }

    let mut pending = Vec::new();
    let mut content = String::new();
    while let Some(chunk) = response.body.next().await {
        let Ok(chunk) = chunk else {
            return None;
        };
        if pending.len().saturating_add(chunk.len()) > MAX_TITLE_RESPONSE_BYTES {
            return None;
        }
        pending.extend_from_slice(&chunk);
        while let Some(line) = pop_sse_line(&mut pending, false) {
            append_title_delta(&mut content, &line);
        }
    }
    while let Some(line) = pop_sse_line(&mut pending, true) {
        append_title_delta(&mut content, &line);
    }
    normalize_title(&content)
}

fn append_title_delta(content: &mut String, line: &[u8]) {
    let Ok(SseLine::Data(value)) = parse_sse_line(line) else {
        return;
    };
    if let Some(delta) = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        content.push_str(delta);
    }
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
    normalize_title(&text)
}

fn normalize_title(text: &str) -> Option<String> {
    let title = text.trim().trim_matches(['"', '\'', '`']).trim();
    if title.is_empty() || title.chars().any(char::is_control) {
        return None;
    }
    Some(title.chars().take(120).collect())
}
