//! Validates ChatGPT image-generation requests and bounds response handling.
//!
//! Public and Codex authentication routes are kept distinct, cancellation is
//! observed during I/O, and decoded image bytes are checked before storage.

use std::future::Future;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{StreamExt, future::join_all};
use reqwest::{RequestBuilder, Response, StatusCode, header::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::protocol::ServiceError;

use super::{CHATGPT_CODEX_BASE, ChatGptService};

const PUBLIC_IMAGES_BASE: &str = "https://api.openai.com/v1";
const MAX_PROMPT_CHARS: usize = 10_000;
const MAX_MODEL_CHARS: usize = 128;
const MAX_API_KEY_CHARS: usize = 4_096;
const MAX_CONNECTION_ID_CHARS: usize = 256;
const MAX_IMAGE_EDGE: u32 = 3_840;
const MIN_IMAGE_PIXELS: u64 = 655_360;
const MAX_IMAGE_PIXELS: u64 = 8_294_400;
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_IMAGE_RESPONSE_BYTES: usize = 48 * 1024 * 1024;
const MAX_BASE64_IMAGE_CHARS: usize = MAX_IMAGE_BYTES.div_ceil(3) * 4 + 4;
const MAX_GENERATED_IMAGE_COUNT: u8 = 3;

#[derive(Clone, Copy)]
enum ImageRequestRoute {
    ApiKey,
    ChatGptOAuth,
}

/// Background treatment supported by the image generation API.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageBackground {
    /// Preserves transparency in supported output formats.
    Transparent,
    /// Produces an output without transparency.
    Opaque,
    /// Lets the provider select the background treatment.
    Auto,
}

impl ImageBackground {
    fn as_str(self) -> &'static str {
        match self {
            Self::Transparent => "transparent",
            Self::Opaque => "opaque",
            Self::Auto => "auto",
        }
    }
}

/// Provider quality tier for generated images.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    /// Lowest supported generation quality.
    Low,
    /// Standard generation quality.
    Medium,
    /// High generation quality.
    High,
    /// Extra-high generation quality when supported.
    XHigh,
    /// Maximum generation quality when supported.
    Max,
    /// Lets the provider select a quality tier.
    Auto,
}

impl ImageQuality {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
            Self::Auto => "auto",
        }
    }
}

/// File encoding requested for generated image bytes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageOutputFormat {
    /// PNG encoding.
    Png,
    /// WebP encoding.
    Webp,
    /// JPEG encoding.
    Jpeg,
}

impl ImageOutputFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Jpeg => "jpeg",
        }
    }
}

/// Content-moderation level for GPT Image API requests.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageModeration {
    /// Uses the provider's default moderation level.
    Auto,
    /// Requests the provider's less restrictive moderation level.
    Low,
}

impl ImageModeration {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Low => "low",
        }
    }
}

/// Prompt and generation options sent to the provider.
#[derive(Clone, Debug)]
pub struct ImageGenerationRequest {
    /// User prompt, validated against the provider request limit.
    pub prompt: String,
    /// Provider image model identifier.
    pub model: String,
    /// Optional supported image dimensions.
    pub size: Option<String>,
    /// Optional provider quality tier.
    pub quality: Option<ImageQuality>,
    /// Optional output background treatment.
    pub background: Option<ImageBackground>,
    /// Optional API content-moderation level.
    pub moderation: Option<ImageModeration>,
    /// Optional output file encoding.
    pub output_format: Option<ImageOutputFormat>,
    /// Optional output compression level for lossy API formats.
    pub output_compression: Option<u8>,
    /// Number of images requested, bounded by this provider integration.
    pub count: u8,
}

/// Authentication route for an image generation request.
#[derive(Clone, Copy)]
pub enum ImageGenerationAuth<'a> {
    /// Uses an API key supplied by the provider credential boundary.
    ApiKey(&'a str),
    /// Uses an existing ChatGPT OAuth connection and workspace.
    ChatGptOAuth {
        /// Stable local connection identifier.
        connection_id: &'a str,
        /// Provider workspace identifier associated with the connection.
        workspace_id: &'a str,
    },
}

/// Provider metadata attached to a successful image generation response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageResponseMetadata {
    /// Provider creation time in Unix seconds.
    pub created_unix_seconds: u64,
    /// Optional provider request identifier for support diagnostics.
    pub imagegen_request_id: Option<String>,
    /// Background treatment reported by the provider.
    pub background: Option<ImageBackground>,
    /// Quality tier reported by the provider.
    pub quality: Option<ImageQuality>,
    /// Output dimensions reported by the provider.
    pub size: Option<String>,
}

/// One decoded image output and its non-secret provider metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageOutput {
    /// Validated encoded image bytes.
    pub bytes: Vec<u8>,
    /// MIME type derived from the validated image signature.
    pub mime_type: String,
    /// Optional prompt text revised by the provider.
    pub revised_prompt: Option<String>,
    /// Optional generation identifier returned by the provider.
    pub generation_id: Option<String>,
}

/// Successful image-generation metadata and all validated outputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageGenerationResult {
    /// Request-level metadata returned by the provider.
    pub metadata: ImageResponseMetadata,
    /// Generated images in provider response order.
    pub outputs: Vec<ImageOutput>,
}

/// State reported for the selected ChatGPT account's normal usage quota.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatGptAccountQuotaState {
    /// The account has ordinary usage available.
    Available,
    /// The account's ordinary usage is exhausted.
    Exhausted,
    /// The provider did not report enough account usage data to determine state.
    Unknown,
    /// The account usage endpoint could not be refreshed.
    Unavailable,
    /// ChatGPT account quotas do not apply to API-key authentication.
    NotApplicable,
}

/// Safe account usage information reported by ChatGPT OAuth.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptAccountQuotaStatus {
    /// Current account quota state.
    pub state: ChatGptAccountQuotaState,
    /// Highest usage percentage reported for an account bucket.
    pub used_percent: Option<f64>,
    /// Provider reset time in Unix milliseconds, when reported.
    pub reset_at_unix_ms: Option<i64>,
    /// Safe machine-readable reason when account status is unknown or unavailable.
    pub reason: Option<String>,
}

/// Generated image outputs together with the account quota snapshot used before generation.
pub struct ImageGenerationWithAccountQuotaResult {
    /// Validated image response.
    pub result: ImageGenerationResult,
    /// ChatGPT account quota information for the authentication route.
    pub account_quota: ChatGptAccountQuotaStatus,
    /// Number of requested images that could not be returned after partial success.
    pub failed_count: u8,
    /// First safe error from a partially successful multi-image request.
    pub failure: Option<ServiceError>,
}

impl ImageGenerationRequest {
    fn validate(&self) -> Result<(), ServiceError> {
        validate_prompt(&self.prompt)?;
        validate_model(&self.model)?;
        validate_size(self.size.as_deref())?;
        validate_count(self.count)?;
        if matches!(self.quality, Some(ImageQuality::XHigh | ImageQuality::Max))
            && !supports_extended_quality(&self.model)
        {
            return Err(invalid_image_request("quality"));
        }
        if self
            .size
            .as_deref()
            .is_some_and(|size| !is_standard_size(size))
            && !supports_custom_size(&self.model)
        {
            return Err(invalid_image_request("size"));
        }
        if self.output_compression.is_some_and(|value| value > 100)
            || (self.output_compression.is_some()
                && !matches!(
                    self.output_format,
                    Some(ImageOutputFormat::Jpeg | ImageOutputFormat::Webp)
                ))
            || (self.background == Some(ImageBackground::Transparent)
                && self.output_format == Some(ImageOutputFormat::Jpeg))
        {
            return Err(invalid_image_request("output_compression"));
        }
        Ok(())
    }
}

impl ChatGptService {
    /// Reads the normal account quota from the selected ChatGPT usage endpoint.
    pub async fn chatgpt_account_quota(
        &self,
        connection_id: &str,
        workspace_id: &str,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<ChatGptAccountQuotaStatus, ServiceError> {
        match self.usage(connection_id, workspace_id, cancellation).await {
            Ok(snapshot) => Ok(chatgpt_account_quota_status(&snapshot)),
            Err(error) if *cancellation.borrow() || error.code == "request_cancelled" => Err(error),
            Err(error) if error.retryable => Ok(ChatGptAccountQuotaStatus {
                state: ChatGptAccountQuotaState::Unavailable,
                used_percent: None,
                reset_at_unix_ms: None,
                reason: Some(error.code.to_owned()),
            }),
            Err(error) => Err(error),
        }
    }

    /// Checks the ChatGPT account quota and generates through the shared image service.
    ///
    /// API-key authentication skips this check because OpenAI applies API limits when
    /// the generation request is submitted. OAuth batches use concurrent single-image
    /// requests after one account-quota check because that route does not accept `n`.
    pub async fn generate_image_with_account_quota(
        &self,
        request: &ImageGenerationRequest,
        auth: ImageGenerationAuth<'_>,
        turn_id: Option<&str>,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<ImageGenerationWithAccountQuotaResult, ServiceError> {
        request.validate()?;
        if matches!(auth, ImageGenerationAuth::ChatGptOAuth { .. }) {
            let mut single_image_request = request.clone();
            single_image_request.count = 1;
            validate_codex_image_request(&single_image_request)?;
        }
        let account_quota = match auth {
            ImageGenerationAuth::ApiKey(_) => ChatGptAccountQuotaStatus {
                state: ChatGptAccountQuotaState::NotApplicable,
                used_percent: None,
                reset_at_unix_ms: None,
                reason: None,
            },
            ImageGenerationAuth::ChatGptOAuth {
                connection_id,
                workspace_id,
            } => {
                self.chatgpt_account_quota(connection_id, workspace_id, cancellation)
                    .await?
            }
        };
        if account_quota.state == ChatGptAccountQuotaState::Exhausted {
            return Err(ServiceError::new(
                "image_rate_limited",
                "ChatGPT reports that the selected account's ordinary usage limit has been reached.",
                true,
            ));
        }
        let (result, failed_count, failure) = match auth {
            ImageGenerationAuth::ApiKey(_) => {
                let result = self
                    .generate_image(request, auth, turn_id, cancellation)
                    .await?;
                let returned_count =
                    u8::try_from(result.outputs.len()).map_err(|_| invalid_image_response())?;
                if returned_count > request.count {
                    return Err(invalid_image_response());
                }
                let failed_count = request.count - returned_count;
                let failure = (failed_count > 0).then(|| {
                    ServiceError::new(
                        "image_generation_incomplete",
                        "The image provider returned fewer images than requested.",
                        true,
                    )
                });
                (result, failed_count, failure)
            }
            ImageGenerationAuth::ChatGptOAuth { .. } if request.count == 1 => {
                let result = self
                    .generate_image(request, auth, turn_id, cancellation)
                    .await?;
                if result.outputs.len() != 1 {
                    return Err(invalid_image_response());
                }
                (result, 0, None)
            }
            ImageGenerationAuth::ChatGptOAuth { .. } => {
                self.generate_oauth_image_batch(request, auth, turn_id, cancellation)
                    .await?
            }
        };
        Ok(ImageGenerationWithAccountQuotaResult {
            result,
            account_quota,
            failed_count,
            failure,
        })
    }

    /// Runs a bounded batch of concurrent OAuth image requests and preserves partial successes.
    async fn generate_oauth_image_batch(
        &self,
        request: &ImageGenerationRequest,
        auth: ImageGenerationAuth<'_>,
        turn_id: Option<&str>,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<(ImageGenerationResult, u8, Option<ServiceError>), ServiceError> {
        let mut single_image_request = request.clone();
        single_image_request.count = 1;
        let single_image_request = &single_image_request;
        let cancellation_receivers = (0..request.count)
            .map(|_| cancellation.clone())
            .collect::<Vec<_>>();
        let responses = join_all(cancellation_receivers.into_iter().map(
            |mut request_cancellation| async move {
                self.generate_image(
                    single_image_request,
                    auth,
                    turn_id,
                    &mut request_cancellation,
                )
                .await
            },
        ))
        .await;

        let mut outputs = Vec::new();
        let mut metadata = None;
        let mut failed_count = 0_u8;
        let mut first_failure = None;
        for response in responses {
            match response {
                Ok(mut result) if result.outputs.len() == 1 => {
                    if metadata.is_none() {
                        metadata = Some(result.metadata.clone());
                    }
                    outputs.append(&mut result.outputs);
                }
                Ok(_) => {
                    failed_count = failed_count.saturating_add(1);
                    first_failure.get_or_insert_with(invalid_image_response);
                }
                Err(error) => {
                    failed_count = failed_count.saturating_add(1);
                    first_failure.get_or_insert(error);
                }
            }
        }
        if outputs.is_empty() {
            return match first_failure {
                Some(error) => Err(error),
                None => Err(ServiceError::new(
                    "image_generation_failed",
                    "The image provider did not return an image.",
                    true,
                )),
            };
        }
        let Some(metadata) = metadata else {
            return Err(ServiceError::new(
                "image_generation_failed",
                "The image provider did not return valid image metadata.",
                true,
            ));
        };
        Ok((
            ImageGenerationResult { metadata, outputs },
            failed_count,
            first_failure,
        ))
    }

    /// Sends an image-generation request and returns validated image bytes.
    ///
    /// The request is validated before network access. Cancellation, rejected
    /// provider responses, oversized payloads, and invalid image signatures are
    /// returned as service errors; this method does not persist the outputs.
    async fn generate_image(
        &self,
        request: &ImageGenerationRequest,
        auth: ImageGenerationAuth<'_>,
        turn_id: Option<&str>,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<ImageGenerationResult, ServiceError> {
        request.validate()?;
        let (value, image_request_id) = match auth {
            ImageGenerationAuth::ApiKey(api_key) => {
                validate_api_key(api_key)?;
                self.request_public_json(
                    public_endpoint("images/generations"),
                    api_key,
                    generation_body(request, true, true, true, true),
                    cancellation,
                )
                .await?
            }
            ImageGenerationAuth::ChatGptOAuth {
                connection_id,
                workspace_id,
            } => {
                validate_codex_image_request(request)?;
                self.request_codex_json(
                    codex_endpoint("images/generations"),
                    connection_id,
                    workspace_id,
                    generation_body(request, false, false, false, false),
                    turn_id,
                    cancellation,
                )
                .await?
            }
        };
        parse_provider_response(value, image_request_id)
    }

    async fn request_public_json(
        &self,
        url: String,
        api_key: &str,
        body: Value,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<(Value, Option<String>), ServiceError> {
        let response = send_with_cancellation(
            self.http.post(url).bearer_auth(api_key).json(&body),
            cancellation,
        )
        .await?;
        response_json_bounded(response, cancellation, ImageRequestRoute::ApiKey).await
    }

    async fn request_codex_json(
        &self,
        url: String,
        connection_id: &str,
        workspace_id: &str,
        body: Value,
        turn_id: Option<&str>,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<(Value, Option<String>), ServiceError> {
        validate_connection_identifier(connection_id, "connection")?;
        validate_connection_identifier(workspace_id, "workspace")?;
        if let Some(turn_id) = turn_id {
            validate_connection_identifier(turn_id, "turn")?;
        }
        self.ensure_active_connection(connection_id)?;
        let external_workspace_id = self.external_workspace_id(connection_id, workspace_id)?;
        let response = await_with_cancellation(
            self.authorized_image_request(
                reqwest::Method::POST,
                url,
                connection_id,
                &external_workspace_id,
                Some(body),
                turn_id,
            ),
            cancellation,
        )
        .await?;
        response_json_bounded(response, cancellation, ImageRequestRoute::ChatGptOAuth).await
    }
}

fn chatgpt_account_quota_status(snapshot: &Value) -> ChatGptAccountQuotaStatus {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX);
    let buckets = snapshot
        .get("buckets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|bucket| {
            bucket
                .get("limitId")
                .and_then(Value::as_str)
                .is_some_and(|id| id.starts_with("codex:"))
                && bucket
                    .get("resetAtUnixMs")
                    .and_then(Value::as_i64)
                    .is_none_or(|reset_at| reset_at > now)
        })
        .collect::<Vec<_>>();
    let exhausted_buckets = buckets
        .iter()
        .filter(|bucket| {
            bucket
                .get("usedPercent")
                .and_then(Value::as_f64)
                .is_some_and(|used| used >= 100.0)
        })
        .collect::<Vec<_>>();
    let ordinary_usage_allowed = snapshot
        .get("ordinaryUsageAllowed")
        .and_then(Value::as_bool);
    let exhausted = ordinary_usage_allowed == Some(false) || !exhausted_buckets.is_empty();
    let used_percent = buckets
        .iter()
        .filter_map(|bucket| bucket.get("usedPercent").and_then(Value::as_f64))
        .max_by(f64::total_cmp);
    let has_usage = used_percent.is_some();
    let reset_at_unix_ms = exhausted_buckets
        .iter()
        .filter_map(|bucket| bucket.get("resetAtUnixMs").and_then(Value::as_i64))
        .max()
        .or_else(|| {
            if ordinary_usage_allowed == Some(false) {
                buckets
                    .iter()
                    .filter_map(|bucket| bucket.get("resetAtUnixMs").and_then(Value::as_i64))
                    .max()
            } else {
                None
            }
        });
    let state = if exhausted {
        ChatGptAccountQuotaState::Exhausted
    } else if ordinary_usage_allowed == Some(true) || has_usage {
        ChatGptAccountQuotaState::Available
    } else {
        ChatGptAccountQuotaState::Unknown
    };
    ChatGptAccountQuotaStatus {
        state,
        used_percent,
        reset_at_unix_ms,
        reason: (state == ChatGptAccountQuotaState::Unknown)
            .then(|| "account_quota_not_reported".to_owned()),
    }
}

fn public_endpoint(path: &str) -> String {
    format!("{PUBLIC_IMAGES_BASE}/{path}")
}

fn codex_endpoint(path: &str) -> String {
    format!("{CHATGPT_CODEX_BASE}/{path}")
}

fn generation_body(
    request: &ImageGenerationRequest,
    include_output_format: bool,
    include_count: bool,
    include_compression: bool,
    include_moderation: bool,
) -> Value {
    let mut body = json!({
        "model": &request.model,
        "prompt": &request.prompt,
    });
    if include_count {
        body["n"] = json!(request.count);
    }
    add_optional_fields(
        &mut body,
        request.size.as_deref(),
        request.quality,
        request.background,
    );
    if include_output_format && let Some(output_format) = request.output_format {
        body["output_format"] = Value::String(output_format.as_str().to_owned());
    }
    if include_compression && let Some(output_compression) = request.output_compression {
        body["output_compression"] = json!(output_compression);
    }
    if include_moderation && let Some(moderation) = request.moderation {
        body["moderation"] = Value::String(moderation.as_str().to_owned());
    }
    body
}

fn add_optional_fields(
    body: &mut Value,
    size: Option<&str>,
    quality: Option<ImageQuality>,
    background: Option<ImageBackground>,
) {
    if let Some(size) = size {
        body["size"] = Value::String(size.to_owned());
    }
    if let Some(quality) = quality {
        body["quality"] = Value::String(quality.as_str().to_owned());
    }
    if let Some(background) = background {
        body["background"] = Value::String(background.as_str().to_owned());
    }
}

fn image_request_id_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-codex-imagegen-request-id")
        .or_else(|| headers.get("x-request-id"))
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn image_transport_error(is_timeout: bool) -> ServiceError {
    if is_timeout {
        ServiceError::new(
            "provider_timeout",
            "The image provider did not return an image in time.",
            true,
        )
    } else {
        super::network_error()
    }
}

async fn send_with_cancellation(
    request: RequestBuilder,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Response, ServiceError> {
    await_with_cancellation(
        async move {
            request
                .send()
                .await
                .map_err(|error| image_transport_error(error.is_timeout()))
        },
        cancellation,
    )
    .await
}

async fn response_json_bounded(
    response: Response,
    cancellation: &mut watch::Receiver<bool>,
    route: ImageRequestRoute,
) -> Result<(Value, Option<String>), ServiceError> {
    let status = response.status();
    if !status.is_success() {
        return Err(provider_status_error_for_route(status, route));
    }
    let image_request_id = image_request_id_from_headers(response.headers());
    if response
        .content_length()
        .is_some_and(|length| length > MAX_IMAGE_RESPONSE_BYTES as u64)
    {
        return Err(image_response_too_large());
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    loop {
        let next = tokio::select! {
            changed = cancellation.changed() => {
                let _ = changed;
                return Err(image_request_cancelled());
            }
            next = stream.next() => next,
        };
        let Some(next) = next else {
            break;
        };
        let chunk = next.map_err(|error| image_transport_error(error.is_timeout()))?;
        if body.len().saturating_add(chunk.len()) > MAX_IMAGE_RESPONSE_BYTES {
            return Err(image_response_too_large());
        }
        body.extend_from_slice(&chunk);
    }
    let value = serde_json::from_slice(&body).map_err(|_| invalid_image_response())?;
    Ok((value, image_request_id))
}

async fn await_with_cancellation<F, T>(
    future: F,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<T, ServiceError>
where
    F: Future<Output = Result<T, ServiceError>>,
{
    if *cancellation.borrow() {
        return Err(image_request_cancelled());
    }
    tokio::pin!(future);
    tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            Err(image_request_cancelled())
        }
        result = &mut future => result,
    }
}

#[derive(Deserialize)]
struct ProviderImageResponse {
    created: u64,
    data: Vec<ProviderImageData>,
    background: Option<String>,
    quality: Option<String>,
    size: Option<String>,
}

#[derive(Deserialize)]
struct ProviderImageData {
    b64_json: String,
    #[serde(alias = "id")]
    generation_id: Option<String>,
    revised_prompt: Option<String>,
}

fn parse_provider_response(
    value: Value,
    image_request_id: Option<String>,
) -> Result<ImageGenerationResult, ServiceError> {
    let response: ProviderImageResponse =
        serde_json::from_value(value).map_err(|_| invalid_image_response())?;
    if response.data.is_empty() {
        return Err(invalid_image_response());
    }
    let outputs = response
        .data
        .into_iter()
        .map(parse_image_output)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ImageGenerationResult {
        metadata: ImageResponseMetadata {
            created_unix_seconds: response.created,
            imagegen_request_id: image_request_id,
            background: parse_background(response.background.as_deref()),
            quality: parse_quality(response.quality.as_deref()),
            size: response.size,
        },
        outputs,
    })
}

fn parse_image_output(data: ProviderImageData) -> Result<ImageOutput, ServiceError> {
    if data.b64_json.len() > MAX_BASE64_IMAGE_CHARS {
        return Err(image_output_too_large());
    }
    let bytes = STANDARD
        .decode(data.b64_json.as_bytes())
        .map_err(|_| invalid_image_output())?;
    let mime_type = image_mime_for_signature(&bytes).ok_or_else(invalid_image_output)?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(image_output_too_large());
    }
    Ok(ImageOutput {
        bytes,
        mime_type: mime_type.to_owned(),
        revised_prompt: data.revised_prompt,
        generation_id: data.generation_id,
    })
}

fn parse_background(value: Option<&str>) -> Option<ImageBackground> {
    match value {
        Some("transparent") => Some(ImageBackground::Transparent),
        Some("opaque") => Some(ImageBackground::Opaque),
        Some("auto") => Some(ImageBackground::Auto),
        _ => None,
    }
}

fn parse_quality(value: Option<&str>) -> Option<ImageQuality> {
    match value {
        Some("low") => Some(ImageQuality::Low),
        Some("medium") => Some(ImageQuality::Medium),
        Some("high") => Some(ImageQuality::High),
        Some("xhigh") => Some(ImageQuality::XHigh),
        Some("max") => Some(ImageQuality::Max),
        Some("auto") => Some(ImageQuality::Auto),
        _ => None,
    }
}

fn validate_prompt(prompt: &str) -> Result<(), ServiceError> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(invalid_image_request("prompt"));
    }
    Ok(())
}

fn validate_model(model: &str) -> Result<(), ServiceError> {
    if model.is_empty()
        || model.chars().count() > MAX_MODEL_CHARS
        || !model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-/".contains(&byte))
    {
        return Err(invalid_image_request("model"));
    }
    Ok(())
}

fn validate_size(size: Option<&str>) -> Result<(), ServiceError> {
    let Some(size) = size else {
        return Ok(());
    };
    if size == "auto" {
        return Ok(());
    }
    let Some((width, height)) = size.split_once('x') else {
        return Err(invalid_image_request("size"));
    };
    let Ok(width) = width.parse::<u32>() else {
        return Err(invalid_image_request("size"));
    };
    let Ok(height) = height.parse::<u32>() else {
        return Err(invalid_image_request("size"));
    };
    let long_edge = u32::max(width, height);
    let short_edge = u32::min(width, height);
    if !(256..=MAX_IMAGE_EDGE).contains(&width)
        || !(256..=MAX_IMAGE_EDGE).contains(&height)
        || width % 16 != 0
        || height % 16 != 0
        || u64::from(width) * u64::from(height) < MIN_IMAGE_PIXELS
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
        || long_edge > short_edge.saturating_mul(3)
    {
        return Err(invalid_image_request("size"));
    }
    Ok(())
}

fn validate_count(count: u8) -> Result<(), ServiceError> {
    if (1..=MAX_GENERATED_IMAGE_COUNT).contains(&count) {
        Ok(())
    } else {
        Err(invalid_image_request("count"))
    }
}

fn validate_api_key(api_key: &str) -> Result<(), ServiceError> {
    if api_key.trim().is_empty()
        || api_key.chars().count() > MAX_API_KEY_CHARS
        || api_key.chars().any(char::is_control)
    {
        return Err(ServiceError::new(
            "image_auth_invalid",
            "The OpenAI API key is missing or invalid.",
            false,
        ));
    }
    Ok(())
}

fn validate_connection_identifier(value: &str, kind: &str) -> Result<(), ServiceError> {
    if value.trim().is_empty()
        || value.chars().count() > MAX_CONNECTION_ID_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(ServiceError::new(
            "image_auth_invalid",
            format!("The ChatGPT {kind} identifier is missing or invalid."),
            false,
        ));
    }
    Ok(())
}

fn validate_codex_quality(quality: Option<ImageQuality>) -> Result<(), ServiceError> {
    if matches!(quality, Some(ImageQuality::XHigh | ImageQuality::Max)) {
        return Err(invalid_image_request("quality"));
    }
    Ok(())
}

fn validate_codex_image_request(request: &ImageGenerationRequest) -> Result<(), ServiceError> {
    validate_codex_quality(request.quality)?;
    if request.count != 1
        || request.output_format.is_some()
        || request.output_compression.is_some()
        || request.moderation.is_some()
    {
        return Err(invalid_image_request("unsupported_oauth_option"));
    }
    Ok(())
}

fn supports_extended_quality(model: &str) -> bool {
    matches!(
        model,
        "gpt-image-2.5-flare"
            | "gpt-image-2.5-flare-2026-09-08"
            | "gpt-image-2.5-sunburst"
            | "gpt-image-2.5-sunburst-2026-09-08"
    )
}

fn supports_custom_size(model: &str) -> bool {
    matches!(model, "gpt-image-2" | "gpt-image-2-2026-04-21") || supports_extended_quality(model)
}

fn is_standard_size(size: &str) -> bool {
    matches!(size, "auto" | "1024x1024" | "1024x1536" | "1536x1024")
}

fn image_mime_for_signature(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn invalid_image_request(field: &str) -> ServiceError {
    ServiceError::new(
        "image_request_invalid",
        format!("The image request field `{field}` is invalid."),
        false,
    )
}

fn invalid_image_response() -> ServiceError {
    ServiceError::new(
        "image_response_invalid",
        "The image provider returned data that OpenChat could not read.",
        false,
    )
}

fn invalid_image_output() -> ServiceError {
    ServiceError::new(
        "image_output_invalid",
        "The image provider returned invalid or unsupported image bytes.",
        false,
    )
}

fn image_output_too_large() -> ServiceError {
    ServiceError::new(
        "image_output_too_large",
        "The generated image exceeds OpenChat's supported size limit.",
        false,
    )
}

fn image_response_too_large() -> ServiceError {
    ServiceError::new(
        "image_response_too_large",
        "The image provider response exceeds OpenChat's supported size limit.",
        false,
    )
}

fn image_request_cancelled() -> ServiceError {
    ServiceError::new(
        "request_cancelled",
        "The image request was cancelled.",
        true,
    )
}

#[cfg(test)]
fn provider_status_error(status: StatusCode) -> ServiceError {
    provider_status_error_for_route(status, ImageRequestRoute::ChatGptOAuth)
}

fn provider_status_error_for_route(status: StatusCode, route: ImageRequestRoute) -> ServiceError {
    match status {
        StatusCode::UNAUTHORIZED => ServiceError::new(
            "image_auth_rejected",
            "The image provider rejected the selected credentials.",
            false,
        ),
        StatusCode::FORBIDDEN => ServiceError::new(
            "image_entitlement_unavailable",
            "The selected account does not have access to image generation.",
            false,
        ),
        StatusCode::TOO_MANY_REQUESTS => match route {
            ImageRequestRoute::ApiKey => ServiceError::new(
                "image_api_rate_limited",
                "The OpenAI API rate limit was reached. Check API usage limits and try again later.",
                true,
            ),
            ImageRequestRoute::ChatGptOAuth => ServiceError::new(
                "image_rate_limited",
                "The ChatGPT image-generation limit was reached. Try again after it resets.",
                true,
            ),
        },
        status if status.is_server_error() => ServiceError::new(
            "image_provider_unavailable",
            "The image provider is temporarily unavailable.",
            true,
        ),
        _ => ServiceError::new(
            "image_provider_rejected",
            "The image provider rejected the image request.",
            false,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nopenchat";
    const JPEG: &[u8] = b"\xff\xd8\xffopenchat";

    fn generation_request() -> ImageGenerationRequest {
        ImageGenerationRequest {
            prompt: "A small lighthouse at sunrise".to_owned(),
            model: "configured-image-model".to_owned(),
            size: Some("1024x1024".to_owned()),
            quality: Some(ImageQuality::High),
            background: Some(ImageBackground::Auto),
            moderation: None,
            output_format: Some(ImageOutputFormat::Png),
            output_compression: None,
            count: 2,
        }
    }

    #[test]
    fn public_and_codex_routes_are_separate() {
        assert_eq!(
            public_endpoint("images/generations"),
            "https://api.openai.com/v1/images/generations"
        );
        assert_eq!(
            codex_endpoint("images/generations"),
            "https://chatgpt.com/backend-api/codex/images/generations"
        );
        assert_ne!(
            public_endpoint("images/generations"),
            codex_endpoint("images/generations")
        );
    }

    #[test]
    fn api_key_body_has_public_only_fields() {
        let value = generation_body(&generation_request(), true, true, true, true);
        assert_eq!(value["output_format"], "png");
        assert_eq!(value["n"], 2);
        assert_eq!(value["quality"], "high");

        let mut xhigh_request = generation_request();
        xhigh_request.model = "configured-image-model-with-extended-quality".to_owned();
        xhigh_request.quality = Some(ImageQuality::XHigh);
        assert_eq!(
            generation_body(&xhigh_request, true, true, true, true)["quality"],
            "xhigh"
        );
    }

    #[test]
    fn codex_body_does_not_receive_public_output_format() {
        let value = generation_body(&generation_request(), false, false, false, false);
        assert!(value.get("output_format").is_none());
        assert!(value.get("n").is_none());
        assert_eq!(value["background"], "auto");
    }

    #[test]
    fn account_quota_uses_the_chatgpt_account_usage_buckets() {
        let status = chatgpt_account_quota_status(&json!({
            "ordinaryUsageAllowed": false,
            "buckets": [{
                "limitId": "codex:primary",
                "usedPercent": 100.0,
                "resetAtUnixMs": i64::MAX
            }]
        }));

        assert_eq!(status.state, ChatGptAccountQuotaState::Exhausted);
        assert_eq!(status.used_percent, Some(100.0));
        assert_eq!(status.reset_at_unix_ms, Some(i64::MAX));
    }

    #[test]
    fn account_quota_reads_the_normal_primary_bucket_from_oauth_usage_payload() {
        let snapshot = super::super::response_parser::parse_usage(
            &json!({
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 42.5,
                        "reset_at": 4_102_444_800_i64
                    }
                },
                "additional_rate_limits": [{
                    "limit_name": "image_generation",
                    "rate_limit": {
                        "primary_window": {
                            "used_percent": 100.0,
                            "reset_at": 4_102_444_800_i64
                        }
                    }
                }]
            }),
            "connection",
            "workspace",
            Vec::new(),
            None,
            "omitted",
        )
        .expect("OAuth usage response should parse");
        let buckets = serde_json::to_value(snapshot.buckets)
            .expect("usage buckets should serialize for quota evaluation");
        let snapshot = json!({
            "ordinaryUsageAllowed": snapshot.ordinary_usage_allowed,
            "buckets": buckets,
        });

        let status = chatgpt_account_quota_status(&snapshot);

        assert_eq!(status.state, ChatGptAccountQuotaState::Available);
        assert_eq!(status.used_percent, Some(42.5));
        assert_eq!(status.reset_at_unix_ms, Some(4_102_444_800_000));
    }

    #[test]
    fn account_quota_reports_exhaustion_and_ignores_expired_windows() {
        let exhausted = chatgpt_account_quota_status(&json!({
            "ordinaryUsageAllowed": true,
            "buckets": [{
                "limitId": "codex:primary",
                "usedPercent": 100.0,
                "resetAtUnixMs": i64::MAX
            }]
        }));
        assert_eq!(exhausted.state, ChatGptAccountQuotaState::Exhausted);
        assert_eq!(exhausted.used_percent, Some(100.0));

        let expired = chatgpt_account_quota_status(&json!({
            "ordinaryUsageAllowed": true,
            "buckets": [{
                "limitId": "codex:primary",
                "usedPercent": 100.0,
                "resetAtUnixMs": 1
            }]
        }));
        assert_eq!(expired.state, ChatGptAccountQuotaState::Available);
        assert_eq!(expired.used_percent, None);
        assert_eq!(expired.reason, None);
    }

    #[test]
    fn account_quota_is_unknown_when_usage_is_not_reported() {
        let status = chatgpt_account_quota_status(&json!({
            "buckets": [{
                "limitId": "codex:secondary",
                "usedPercent": null,
                "resetAtUnixMs": null
            }]
        }));

        assert_eq!(status.state, ChatGptAccountQuotaState::Unknown);
        assert_eq!(status.used_percent, None);
        assert_eq!(status.reason.as_deref(), Some("account_quota_not_reported"));
    }

    #[test]
    fn rate_limit_errors_keep_oauth_and_api_key_routes_distinct() {
        let oauth = provider_status_error_for_route(
            StatusCode::TOO_MANY_REQUESTS,
            ImageRequestRoute::ChatGptOAuth,
        );
        let api_key = provider_status_error_for_route(
            StatusCode::TOO_MANY_REQUESTS,
            ImageRequestRoute::ApiKey,
        );

        assert_eq!(oauth.code, "image_rate_limited");
        assert_eq!(api_key.code, "image_api_rate_limited");
        assert!(oauth.retryable);
        assert!(api_key.retryable);
    }

    #[test]
    fn validation_rejects_invalid_request_fields() {
        let mut request = generation_request();
        request.prompt = "   ".to_owned();
        assert_eq!(
            request.validate().unwrap_err().code,
            "image_request_invalid"
        );
        request.prompt = "valid".to_owned();
        request.size = Some("not-a-size".to_owned());
        assert_eq!(
            request.validate().unwrap_err().code,
            "image_request_invalid"
        );
        request.size = Some("256x256".to_owned());
        assert_eq!(
            request.validate().unwrap_err().code,
            "image_request_invalid"
        );
        request.size = Some("1600x512".to_owned());
        assert_eq!(
            request.validate().unwrap_err().code,
            "image_request_invalid"
        );
        request.size = Some("1024x1024".to_owned());
        request.count = 0;
        assert_eq!(
            request.validate().unwrap_err().code,
            "image_request_invalid"
        );
    }

    #[test]
    fn response_parser_decodes_and_validates_signatures() {
        let value = json!({
            "created": 1_700_000_000_u64,
            "data": [{
                "b64_json": STANDARD.encode(PNG),
                "generation_id": "gen_test",
                "revised_prompt": "revised"
            }],
            "quality": "high",
            "size": "1024x1024"
        });
        let result = parse_provider_response(value, None).expect("valid image response");
        assert_eq!(result.metadata.created_unix_seconds, 1_700_000_000);
        assert_eq!(result.outputs[0].mime_type, "image/png");
        assert_eq!(result.outputs[0].generation_id.as_deref(), Some("gen_test"));

        let invalid = json!({
            "created": 1,
            "data": [{"b64_json": STANDARD.encode(b"not an image")}]
        });
        assert_eq!(
            parse_provider_response(invalid, None).unwrap_err().code,
            "image_output_invalid"
        );
    }

    #[test]
    fn jpeg_signature_is_supported() {
        let value = json!({
            "created": 1,
            "data": [{"b64_json": STANDARD.encode(JPEG)}]
        });
        let result = parse_provider_response(value, None).expect("valid jpeg response");
        assert_eq!(result.outputs[0].mime_type, "image/jpeg");
    }

    #[test]
    fn provider_statuses_have_typed_retryability() {
        assert_eq!(
            provider_status_error(StatusCode::UNAUTHORIZED).code,
            "image_auth_rejected"
        );
        assert_eq!(
            provider_status_error(StatusCode::FORBIDDEN).code,
            "image_entitlement_unavailable"
        );
        assert!(!provider_status_error(StatusCode::UNAUTHORIZED).retryable);
        assert!(!provider_status_error(StatusCode::FORBIDDEN).retryable);
        assert!(provider_status_error(StatusCode::TOO_MANY_REQUESTS).retryable);
        assert!(provider_status_error(StatusCode::BAD_GATEWAY).retryable);
        assert!(!provider_status_error(StatusCode::BAD_REQUEST).retryable);
    }

    #[test]
    fn oauth_quality_rejects_values_not_supported_by_codex() {
        assert_eq!(
            validate_codex_quality(Some(ImageQuality::XHigh))
                .unwrap_err()
                .code,
            "image_request_invalid"
        );
        assert_eq!(
            validate_codex_quality(Some(ImageQuality::Max))
                .unwrap_err()
                .code,
            "image_request_invalid"
        );
        assert!(validate_codex_quality(Some(ImageQuality::High)).is_ok());
        assert!(validate_codex_quality(Some(ImageQuality::Auto)).is_ok());
        assert!(validate_codex_quality(None).is_ok());
    }

    #[test]
    fn generated_image_count_respects_attachment_store_limit() {
        assert!(validate_count(1).is_ok());
        assert!(validate_count(MAX_GENERATED_IMAGE_COUNT).is_ok());
        assert!(validate_count(MAX_GENERATED_IMAGE_COUNT + 1).is_err());
    }

    #[test]
    fn credentials_and_turn_identifiers_reject_whitespace_only_values() {
        assert_eq!(
            validate_api_key(" \t ").unwrap_err().code,
            "image_auth_invalid"
        );
        assert_eq!(
            validate_connection_identifier(" \t ", "connection")
                .unwrap_err()
                .code,
            "image_auth_invalid"
        );
        assert!(validate_connection_identifier("turn-123", "turn").is_ok());
    }

    #[test]
    fn response_headers_preserve_image_request_id() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-codex-imagegen-request-id",
            reqwest::header::HeaderValue::from_static("image-request-123"),
        );
        assert_eq!(
            image_request_id_from_headers(&headers).as_deref(),
            Some("image-request-123")
        );

        let mut fallback_headers = HeaderMap::new();
        fallback_headers.insert(
            "x-request-id",
            reqwest::header::HeaderValue::from_static("request-456"),
        );
        assert_eq!(
            image_request_id_from_headers(&fallback_headers).as_deref(),
            Some("request-456")
        );
    }

    #[test]
    fn response_parser_keeps_image_request_id() {
        let value = json!({
            "created": 1,
            "data": [{"b64_json": STANDARD.encode(PNG)}]
        });
        let result = parse_provider_response(value, Some("image-request-789".to_owned()))
            .expect("valid image response");
        assert_eq!(
            result.metadata.imagegen_request_id.as_deref(),
            Some("image-request-789")
        );
    }

    #[test]
    fn transport_errors_distinguish_timeout_from_connection_failure() {
        let timeout = image_transport_error(true);
        assert_eq!(timeout.code, "provider_timeout");
        assert!(timeout.retryable);

        let connection = image_transport_error(false);
        assert_eq!(connection.code, "network_unavailable");
        assert!(connection.retryable);
    }

    #[tokio::test]
    async fn cancellation_interrupts_pending_request() {
        let (sender, mut receiver) = watch::channel(false);
        let pending = await_with_cancellation(
            std::future::pending::<Result<(), ServiceError>>(),
            &mut receiver,
        );
        sender.send(true).expect("receiver is alive");
        let result = tokio::time::timeout(Duration::from_millis(100), pending)
            .await
            .expect("cancellation should interrupt the pending request")
            .expect_err("cancelled request should fail");
        assert_eq!(result.code, "request_cancelled");
    }

    #[tokio::test]
    async fn mock_image_endpoint_response_is_bounded_and_decoded() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock image endpoint should bind");
        let address = listener
            .local_addr()
            .expect("mock image endpoint should expose an address");
        let body = json!({
            "created": 1_700_000_000_u64,
            "data": [{"b64_json": STANDARD.encode(PNG)}]
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Codex-ImageGen-Request-Id: mock-image-1\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("mock image endpoint should accept");
            let mut request = [0_u8; 4096];
            let size = stream
                .read(&mut request)
                .await
                .expect("mock request should be readable");
            assert!(
                String::from_utf8_lossy(&request[..size]).contains("POST /v1/images/generations ")
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("mock response should be writable");
        });

        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/v1/images/generations"))
            .bearer_auth("test-api-key")
            .json(&generation_body(
                &generation_request(),
                true,
                true,
                true,
                true,
            ))
            .send()
            .await
            .expect("mock image request should succeed");
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let (value, request_id) =
            response_json_bounded(response, &mut cancellation, ImageRequestRoute::ApiKey)
                .await
                .expect("mock image JSON should be accepted");
        assert_eq!(request_id.as_deref(), Some("mock-image-1"));
        let result = parse_provider_response(value, request_id).expect("mock image should decode");
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.outputs[0].mime_type, "image/png");
        server.await.expect("mock image endpoint should finish");
    }
}
