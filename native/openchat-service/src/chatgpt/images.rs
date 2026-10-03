use std::future::Future;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
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
const MAX_BASE64_IMAGE_CHARS: usize = ((MAX_IMAGE_BYTES + 2) / 3) * 4 + 4;
const MAX_GENERATED_IMAGE_COUNT: u8 = 3;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageBackground {
    Transparent,
    Opaque,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    Low,
    Medium,
    High,
    XHigh,
    Max,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageOutputFormat {
    Png,
    Webp,
    Jpeg,
}

impl ImageOutputFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Jpeg => "jpeg",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub model: String,
    pub size: Option<String>,
    pub quality: Option<ImageQuality>,
    pub background: Option<ImageBackground>,
    pub output_format: Option<ImageOutputFormat>,
    pub count: u8,
}

#[derive(Clone, Copy)]
pub enum ImageGenerationAuth<'a> {
    ApiKey(&'a str),
    ChatGptOAuth {
        connection_id: &'a str,
        workspace_id: &'a str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageResponseMetadata {
    pub created_unix_seconds: u64,
    pub imagegen_request_id: Option<String>,
    pub background: Option<ImageBackground>,
    pub quality: Option<ImageQuality>,
    pub size: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageOutput {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub revised_prompt: Option<String>,
    pub generation_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageGenerationResult {
    pub metadata: ImageResponseMetadata,
    pub outputs: Vec<ImageOutput>,
}

impl ImageGenerationRequest {
    fn validate(&self) -> Result<(), ServiceError> {
        validate_prompt(&self.prompt)?;
        validate_model(&self.model)?;
        validate_size(self.size.as_deref())?;
        validate_count(self.count)?;
        Ok(())
    }
}

impl ChatGptService {
    pub async fn generate_image(
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
                validate_api_quality(request.quality, &request.model)?;
                self.request_public_json(
                    public_endpoint("images/generations"),
                    api_key,
                    generation_body(request, true, true),
                    cancellation,
                )
                .await?
            }
            ImageGenerationAuth::ChatGptOAuth {
                connection_id,
                workspace_id,
            } => {
                validate_codex_quality(request.quality)?;
                self.request_codex_json(
                    codex_endpoint("images/generations"),
                    connection_id,
                    workspace_id,
                    generation_body(request, false, false),
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
        response_json_bounded(response, cancellation).await
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
        response_json_bounded(response, cancellation).await
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
    if include_output_format {
        if let Some(output_format) = request.output_format {
            body["output_format"] = Value::String(output_format.as_str().to_owned());
        }
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
) -> Result<(Value, Option<String>), ServiceError> {
    let status = response.status();
    if !status.is_success() {
        return Err(provider_status_error(status));
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

fn validate_api_quality(quality: Option<ImageQuality>, model: &str) -> Result<(), ServiceError> {
    if matches!(quality, Some(ImageQuality::XHigh | ImageQuality::Max))
        && !supports_extended_quality(model)
    {
        return Err(invalid_image_request("quality"));
    }
    Ok(())
}

fn supports_extended_quality(model: &str) -> bool {
    ["gpt-image-2.5-sunburst", "gpt-image-2.5-flare"]
        .iter()
        .any(|base| {
            model == *base
                || model
                    .strip_prefix(base)
                    .is_some_and(|suffix| suffix.starts_with('-'))
        })
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

fn provider_status_error(status: StatusCode) -> ServiceError {
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
        StatusCode::TOO_MANY_REQUESTS => ServiceError::new(
            "image_rate_limited",
            "The image provider rate limit was reached. Try again later.",
            true,
        ),
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
            model: "gpt-image-2".to_owned(),
            size: Some("1024x1024".to_owned()),
            quality: Some(ImageQuality::High),
            background: Some(ImageBackground::Auto),
            output_format: Some(ImageOutputFormat::Png),
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
        let value = generation_body(&generation_request(), true, true);
        assert_eq!(value["output_format"], "png");
        assert_eq!(value["n"], 2);
        assert_eq!(value["quality"], "high");

        let mut xhigh_request = generation_request();
        xhigh_request.model = "gpt-image-2.5-sunburst".to_owned();
        xhigh_request.quality = Some(ImageQuality::XHigh);
        assert_eq!(
            generation_body(&xhigh_request, true, true)["quality"],
            "xhigh"
        );
    }

    #[test]
    fn codex_body_does_not_receive_public_output_format() {
        let value = generation_body(&generation_request(), false, false);
        assert!(value.get("output_format").is_none());
        assert!(value.get("n").is_none());
        assert_eq!(value["background"], "auto");
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
    fn api_image_quality_matches_the_selected_image_model() {
        assert!(validate_api_quality(Some(ImageQuality::High), "gpt-image-2").is_ok());
        assert!(validate_api_quality(Some(ImageQuality::Auto), "gpt-image-2").is_ok());
        assert!(validate_api_quality(Some(ImageQuality::XHigh), "gpt-image-2").is_err());
        assert!(validate_api_quality(Some(ImageQuality::Max), "gpt-image-2").is_err());
        assert!(validate_api_quality(Some(ImageQuality::XHigh), "gpt-image-2.5-sunburst").is_ok());
        assert!(
            validate_api_quality(Some(ImageQuality::Max), "gpt-image-2.5-flare-2026-09-08").is_ok()
        );
        assert!(
            validate_api_quality(Some(ImageQuality::Max), "gpt-image-2.5-unsupported").is_err()
        );
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
            .json(&generation_body(&generation_request(), true, true))
            .send()
            .await
            .expect("mock image request should succeed");
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let (value, request_id) = response_json_bounded(response, &mut cancellation)
            .await
            .expect("mock image JSON should be accepted");
        assert_eq!(request_id.as_deref(), Some("mock-image-1"));
        let result = parse_provider_response(value, request_id).expect("mock image should decode");
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.outputs[0].mime_type, "image/png");
        server.await.expect("mock image endpoint should finish");
    }
}
