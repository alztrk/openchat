//! Detects local Ollama and LM Studio servers on loopback ports, checks health, and lists models.

use std::{collections::BTreeSet, time::Duration};

use futures_util::StreamExt;
use reqwest::{Client, Response};
use serde_json::{Value, json};

use crate::protocol::ServiceError;

use super::external_server;

const DEFAULT_OLLAMA_PORT: u16 = 11434;
const DEFAULT_LM_STUDIO_PORT: u16 = 1234;
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_PROBE_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_LOCAL_SERVER_MODELS: usize = 500;
const MAX_CANDIDATE_PORTS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalServerKind {
    Ollama,
    LmStudio,
}

impl LocalServerKind {
    pub(crate) fn parse(value: &str) -> Result<Self, ServiceError> {
        match value {
            "ollama" => Ok(Self::Ollama),
            "lm_studio" => Ok(Self::LmStudio),
            _ => Err(invalid_local_server_request()),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LmStudio => "lm_studio",
        }
    }

    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::LmStudio => "LM Studio",
        }
    }

    pub(crate) fn default_port(self) -> u16 {
        match self {
            Self::Ollama => DEFAULT_OLLAMA_PORT,
            Self::LmStudio => DEFAULT_LM_STUDIO_PORT,
        }
    }

    pub(crate) fn provider_id(self) -> &'static str {
        match self {
            Self::Ollama => "custom_ollama_local",
            Self::LmStudio => "custom_lm_studio_local",
        }
    }

    fn windows_executables(self) -> &'static [&'static str] {
        match self {
            Self::Ollama => &["ollama.exe", "ollama app.exe"],
            Self::LmStudio => &["LM Studio.exe", "lms.exe"],
        }
    }

    fn linux_executables(self) -> &'static [&'static str] {
        match self {
            Self::Ollama => &["ollama"],
            Self::LmStudio => &["lm-studio", "lms"],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalServerModel {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) context_window: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalServerHealth {
    Healthy,
    Unhealthy,
    Unreachable,
}

impl LocalServerHealth {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Unhealthy => "unhealthy",
            Self::Unreachable => "unreachable",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LocalServerProbeResult {
    pub(crate) port: u16,
    pub(crate) status: LocalServerHealth,
    pub(crate) models: Vec<LocalServerModel>,
}

pub(crate) async fn detect_local_servers(
    ollama_port: Option<u16>,
    lm_studio_port: Option<u16>,
) -> Result<Value, ServiceError> {
    let client = probe_client()?;
    let (ollama, lm_studio) = tokio::join!(
        detect_server_for_kind(&client, LocalServerKind::Ollama, ollama_port),
        detect_server_for_kind(&client, LocalServerKind::LmStudio, lm_studio_port),
    );
    Ok(json!({
        "servers": [ollama, lm_studio],
    }))
}

pub(crate) async fn fetch_local_server_models(
    server_kind: &str,
    port: u16,
) -> Result<Value, ServiceError> {
    if port == 0 {
        return Err(invalid_local_server_request());
    }
    let kind = LocalServerKind::parse(server_kind)?;
    let client = probe_client()?;
    let result = probe_server_port(&client, kind, port).await;
    Ok(json!({
        "serverKind": kind.as_str(),
        "providerId": kind.provider_id(),
        "displayName": kind.display_name(),
        "port": result.port,
        "baseUrl": open_ai_base_url(result.port),
        "status": result.status.as_str(),
        "models": result.models.iter().map(model_to_json).collect::<Vec<_>>(),
    }))
}

async fn detect_server_for_kind(
    client: &Client,
    kind: LocalServerKind,
    preferred_port: Option<u16>,
) -> Value {
    let candidate_ports = candidate_ports_for_kind(kind, preferred_port).await;
    let mut selected: Option<LocalServerProbeResult> = None;

    for port in &candidate_ports {
        let probe = probe_server_port(client, kind, *port).await;
        if probe.status == LocalServerHealth::Healthy {
            selected = Some(probe);
            break;
        }
        if selected
            .as_ref()
            .is_none_or(|current| current.status == LocalServerHealth::Unreachable)
        {
            selected = Some(probe);
        }
    }

    let result = selected.unwrap_or_else(|| LocalServerProbeResult {
        port: preferred_port.unwrap_or_else(|| kind.default_port()),
        status: LocalServerHealth::Unreachable,
        models: Vec::new(),
    });

    json!({
        "serverKind": kind.as_str(),
        "providerId": kind.provider_id(),
        "displayName": kind.display_name(),
        "defaultPort": kind.default_port(),
        "port": result.port,
        "detectedPorts": candidate_ports,
        "baseUrl": open_ai_base_url(result.port),
        "status": result.status.as_str(),
        "models": result.models.iter().map(model_to_json).collect::<Vec<_>>(),
    })
}

async fn candidate_ports_for_kind(kind: LocalServerKind, preferred_port: Option<u16>) -> Vec<u16> {
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_port = |port: u16| {
        if port > 0 && seen.insert(port) && ordered.len() < MAX_CANDIDATE_PORTS {
            ordered.push(port);
        }
    };

    if let Some(port) = preferred_port {
        push_port(port);
    }
    push_port(kind.default_port());

    if let Ok(candidates) = external_server::find_running_servers_for_executables(
        kind.windows_executables(),
        kind.linux_executables(),
    )
    .await
    {
        for candidate in candidates {
            push_port(candidate.port);
        }
    }

    ordered
}

pub(crate) async fn probe_server_port(
    client: &Client,
    kind: LocalServerKind,
    port: u16,
) -> LocalServerProbeResult {
    let v1_models_url = format!("http://127.0.0.1:{port}/v1/models");
    let v1_probe = fetch_probe_json(client, &v1_models_url).await;

    match kind {
        LocalServerKind::Ollama => {
            let tags_url = format!("http://127.0.0.1:{port}/api/tags");
            let tags_probe = fetch_probe_json(client, &tags_url).await;
            match (v1_probe, tags_probe) {
                (ProbeOutcome::Json(v1_json), ProbeOutcome::Json(tags_json)) => {
                    let mut models = parse_openai_models(&v1_json).unwrap_or_default();
                    if let Some(tag_models) = parse_ollama_tags_models(&tags_json) {
                        for tag_model in tag_models {
                            if !models.iter().any(|existing| existing.id == tag_model.id)
                                && models.len() < MAX_LOCAL_SERVER_MODELS
                            {
                                models.push(tag_model);
                            }
                        }
                    }
                    LocalServerProbeResult {
                        port,
                        status: LocalServerHealth::Healthy,
                        models,
                    }
                }
                (ProbeOutcome::Json(v1_json), _) => match parse_openai_models(&v1_json) {
                    Some(models) => LocalServerProbeResult {
                        port,
                        status: LocalServerHealth::Healthy,
                        models,
                    },
                    None => LocalServerProbeResult {
                        port,
                        status: LocalServerHealth::Unhealthy,
                        models: Vec::new(),
                    },
                },
                (_, ProbeOutcome::Json(tags_json)) => match parse_ollama_tags_models(&tags_json) {
                    Some(models) => LocalServerProbeResult {
                        port,
                        status: LocalServerHealth::Healthy,
                        models,
                    },
                    None => LocalServerProbeResult {
                        port,
                        status: LocalServerHealth::Unhealthy,
                        models: Vec::new(),
                    },
                },
                (ProbeOutcome::Unhealthy, _) | (_, ProbeOutcome::Unhealthy) => {
                    LocalServerProbeResult {
                        port,
                        status: LocalServerHealth::Unhealthy,
                        models: Vec::new(),
                    }
                }
                (ProbeOutcome::Unreachable, ProbeOutcome::Unreachable) => LocalServerProbeResult {
                    port,
                    status: LocalServerHealth::Unreachable,
                    models: Vec::new(),
                },
            }
        }
        LocalServerKind::LmStudio => match v1_probe {
            ProbeOutcome::Json(v1_json) => match parse_openai_models(&v1_json) {
                Some(models) => LocalServerProbeResult {
                    port,
                    status: LocalServerHealth::Healthy,
                    models,
                },
                None => LocalServerProbeResult {
                    port,
                    status: LocalServerHealth::Unhealthy,
                    models: Vec::new(),
                },
            },
            ProbeOutcome::Unhealthy => LocalServerProbeResult {
                port,
                status: LocalServerHealth::Unhealthy,
                models: Vec::new(),
            },
            ProbeOutcome::Unreachable => LocalServerProbeResult {
                port,
                status: LocalServerHealth::Unreachable,
                models: Vec::new(),
            },
        },
    }
}

enum ProbeOutcome {
    Json(Value),
    Unhealthy,
    Unreachable,
}

async fn fetch_probe_json(client: &Client, url: &str) -> ProbeOutcome {
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(_) => return ProbeOutcome::Unreachable,
    };
    if !response.status().is_success() {
        return ProbeOutcome::Unhealthy;
    }
    match read_bounded_json(response).await {
        Some(value) => ProbeOutcome::Json(value),
        None => ProbeOutcome::Unhealthy,
    }
}

async fn read_bounded_json(response: Response) -> Option<Value> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PROBE_RESPONSE_BYTES as u64)
    {
        return None;
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.ok()?;
        if bytes.len().saturating_add(chunk.len()) > MAX_PROBE_RESPONSE_BYTES {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).ok()
}

fn parse_openai_models(value: &Value) -> Option<Vec<LocalServerModel>> {
    let data = value.get("data")?.as_array()?;
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();

    for entry in data.iter().take(MAX_LOCAL_SERVER_MODELS) {
        let Some(id) = entry
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control))
        else {
            continue;
        };
        if !seen.insert(id.to_owned()) {
            continue;
        }
        let display_name = entry
            .get("display_name")
            .or_else(|| entry.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| {
                !name.is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
            })
            .unwrap_or(id)
            .to_owned();
        let context_window = entry
            .get("context_window")
            .or_else(|| entry.get("input_token_limit"))
            .or_else(|| entry.get("max_context_length"))
            .and_then(Value::as_i64)
            .filter(|limit| *limit > 0);
        models.push(LocalServerModel {
            id: id.to_owned(),
            display_name,
            context_window,
        });
    }
    Some(models)
}

fn parse_ollama_tags_models(value: &Value) -> Option<Vec<LocalServerModel>> {
    let items = value.get("models")?.as_array()?;
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();

    for entry in items.iter().take(MAX_LOCAL_SERVER_MODELS) {
        let Some(id) = entry
            .get("model")
            .or_else(|| entry.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control))
        else {
            continue;
        };
        if !seen.insert(id.to_owned()) {
            continue;
        }
        let display_name = entry
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| {
                !name.is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
            })
            .unwrap_or(id)
            .to_owned();
        models.push(LocalServerModel {
            id: id.to_owned(),
            display_name,
            context_window: None,
        });
    }
    Some(models)
}

fn model_to_json(model: &LocalServerModel) -> Value {
    json!({
        "id": model.id,
        "displayName": model.display_name,
        "contextWindow": model.context_window,
    })
}

fn open_ai_base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/v1")
}

pub(crate) fn probe_client() -> Result<Client, ServiceError> {
    Client::builder()
        .user_agent("OpenChat local server detector")
        .connect_timeout(PROBE_TIMEOUT)
        .timeout(PROBE_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| {
            ServiceError::new(
                "local_server_probe_unavailable",
                "Local server detection could not initialize an HTTP client.",
                true,
            )
        })
}

fn invalid_local_server_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The local server request parameters are invalid.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    use super::{
        LocalServerHealth, LocalServerKind, fetch_local_server_models, probe_client,
        probe_server_port,
    };

    #[tokio::test]
    async fn probes_ollama_health_and_merges_v1_models_with_api_tags() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local ollama fixture");
        let port = listener.local_addr().expect("read fixture addr").port();

        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.expect("accept probe request");
                let request_line = read_request_line(&mut stream).await;
                let body = if request_line.contains("GET /v1/models ") {
                    r#"{"object":"list","data":[{"id":"qwen2.5-coder:7b","context_window":32768}]}"#
                } else if request_line.contains("GET /api/tags ") {
                    r#"{"models":[{"name":"qwen2.5-coder:7b","model":"qwen2.5-coder:7b"},{"name":"llama3.2:3b","model":"llama3.2:3b"}]}"#
                } else {
                    "{}"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write fixture response");
            }
        });

        let result = fetch_local_server_models("ollama", port)
            .await
            .expect("ollama probe should succeed");
        assert_eq!(result["serverKind"], "ollama");
        assert_eq!(result["providerId"], "custom_ollama_local");
        assert_eq!(result["port"], port);
        assert_eq!(result["baseUrl"], format!("http://127.0.0.1:{port}/v1"));
        assert_eq!(result["status"], "healthy");
        let models = result["models"].as_array().expect("models array");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "qwen2.5-coder:7b");
        assert_eq!(models[0]["contextWindow"], 32768);
        assert_eq!(models[1]["id"], "llama3.2:3b");

        server.await.expect("fixture server completes");
    }

    #[tokio::test]
    async fn probes_lm_studio_health_and_reports_unhealthy_or_unreachable_states() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local lm studio fixture");
        let port = listener.local_addr().expect("read fixture addr").port();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept first probe");
            let _ = read_request_line(&mut stream).await;
            let body =
                r#"{"data":[{"id":"deepseek-r1-distill-qwen-7b","max_context_length":65536}]}"#;
            let ok_response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(ok_response.as_bytes())
                .await
                .expect("write healthy response");

            let (mut stream, _) = listener.accept().await.expect("accept second probe");
            let _ = read_request_line(&mut stream).await;
            stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .expect("write unhealthy response");
        });

        let client = probe_client().expect("build probe client");
        let healthy = probe_server_port(&client, LocalServerKind::LmStudio, port).await;
        assert_eq!(healthy.status, LocalServerHealth::Healthy);
        assert_eq!(healthy.models.len(), 1);
        assert_eq!(healthy.models[0].id, "deepseek-r1-distill-qwen-7b");
        assert_eq!(healthy.models[0].context_window, Some(65536));

        let unhealthy = probe_server_port(&client, LocalServerKind::LmStudio, port).await;
        assert_eq!(unhealthy.status, LocalServerHealth::Unhealthy);
        assert!(unhealthy.models.is_empty());

        server.await.expect("fixture server completes");

        let closed_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let closed_port = closed_listener.local_addr().expect("read addr").port();
        drop(closed_listener);

        let unreachable = probe_server_port(&client, LocalServerKind::LmStudio, closed_port).await;
        assert_eq!(unreachable.status, LocalServerHealth::Unreachable);
    }

    async fn read_request_line(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut byte).await.expect("read request byte");
            if read == 0 {
                break;
            }
            request.push(byte[0]);
        }
        String::from_utf8_lossy(&request).into_owned()
    }
}
