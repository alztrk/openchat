//! Amazon Bedrock Converse and ConverseStream protocol support.
//!
//! Provides AWS SigV4 request signing, regional endpoint resolution,
//! Converse payload mapping, and AWS EventStream binary decoding.

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use ring::hmac;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use time::OffsetDateTime;
use url::Url;

use crate::protocol::ServiceError;

const DEFAULT_BEDROCK_REGION: &str = "us-east-1";

/// Precomputed CRC32 table for IEEE 802.3 polynomial (0xEDB88320).
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 * (crc & 1));
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Computes the IEEE 802.3 CRC-32 checksum.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        let idx = ((crc ^ u32::from(b)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    !crc
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    to_hex(&hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let s_key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&s_key, data).as_ref().to_vec()
}

/// Resolves the AWS region from a Bedrock base URL or returns default "us-east-1".
pub fn bedrock_region_from_url(base_url: &str) -> String {
    if let Ok(parsed) = Url::parse(base_url) {
        if let Some(host) = parsed.host_str() {
            let parts: Vec<&str> = host.split('.').collect();
            if parts.len() >= 4
                && parts[0].starts_with("bedrock")
                && parts[parts.len() - 2] == "amazonaws"
            {
                return parts[1].to_string();
            }
        }
    }
    DEFAULT_BEDROCK_REGION.to_string()
}

/// Resolves the chat URL for a Bedrock Converse or ConverseStream endpoint.
pub fn bedrock_chat_url(base_url: &str, model_id: &str, stream: bool) -> String {
    let trimmed = base_url.trim_end_matches('/');
    let action = if stream {
        "converse-stream"
    } else {
        "converse"
    };
    if trimmed.contains("/model/") {
        if trimmed.ends_with("/converse") || trimmed.ends_with("/converse-stream") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/{action}")
        }
    } else {
        format!("{trimmed}/model/{model_id}/{action}")
    }
}

/// Signs an HTTP request targeted at AWS Bedrock Runtime using AWS SigV4.
pub fn build_signed_bedrock_headers(
    url: &Url,
    method: &str,
    body: &[u8],
    api_key: &str,
) -> Result<HeaderMap, ServiceError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("application/json"),
    );

    // If key contains ':', treat as AccessKeyId:SecretAccessKey[:SessionToken]
    if api_key.contains(':') {
        let parts: Vec<&str> = api_key.split(':').collect();
        let access_key_id = parts[0].trim();
        let secret_access_key = parts[1].trim();
        let session_token = parts.get(2).map(|s| s.trim()).filter(|s| !s.is_empty());

        let now = OffsetDateTime::now_utc();
        let iso_date = format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        );
        let date_stamp = format!(
            "{:04}{:02}{:02}",
            now.year(),
            u8::from(now.month()),
            now.day()
        );

        let region = bedrock_region_from_url(url.as_str());
        let host = url.host_str().unwrap_or("bedrock-runtime.amazonaws.com");

        headers.insert(
            HeaderName::from_static("x-amz-date"),
            HeaderValue::from_str(&iso_date)
                .map_err(|_| ServiceError::new("invalid_request", "Invalid date header", false))?,
        );
        if let Some(token) = session_token {
            headers.insert(
                HeaderName::from_static("x-amz-security-token"),
                HeaderValue::from_str(token).map_err(|_| {
                    ServiceError::new("invalid_request", "Invalid session token header", false)
                })?,
            );
        }

        let signed_headers = if session_token.is_some() {
            "content-type;host;x-amz-date;x-amz-security-token"
        } else {
            "content-type;host;x-amz-date"
        };

        let mut canonical_headers =
            format!("content-type:application/json\nhost:{host}\nx-amz-date:{iso_date}\n");
        if let Some(token) = session_token {
            canonical_headers.push_str(&format!("x-amz-security-token:{token}\n"));
        }

        let canonical_uri = if url.path().is_empty() {
            "/"
        } else {
            url.path()
        };
        let canonical_query = url.query().unwrap_or("");
        let payload_hash = sha256_hex(body);

        let canonical_request = format!(
            "{method}\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );

        let credential_scope = format!("{date_stamp}/{region}/bedrock/aws4_request");
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{iso_date}\n{credential_scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );

        let k_date = hmac_sha256(
            format!("AWS4{secret_access_key}").as_bytes(),
            date_stamp.as_bytes(),
        );
        let k_region = hmac_sha256(&k_date, region.as_bytes());
        let k_service = hmac_sha256(&k_region, b"bedrock");
        let k_signing = hmac_sha256(&k_service, b"aws4_request");
        let signature = to_hex(&hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={access_key_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
        );

        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(&authorization)
                .map_err(|_| ServiceError::new("invalid_request", "Invalid auth header", false))?,
        );
    } else {
        // Bearer token or Bedrock API key
        let auth_val = format!("Bearer {api_key}");
        if let Ok(val) = HeaderValue::from_str(&auth_val) {
            headers.insert(HeaderName::from_static("authorization"), val);
        }
        if let Ok(val) = HeaderValue::from_str(api_key) {
            headers.insert(HeaderName::from_static("api-key"), val);
        }
    }

    Ok(headers)
}

/// Builds the JSON payload for a Bedrock Converse or ConverseStream request.
pub fn bedrock_converse_payload(
    messages: &[Value],
    system_prompt: Option<&str>,
    max_tokens: Option<i64>,
    temperature: Option<f64>,
) -> Value {
    let mut bedrock_messages = Vec::new();
    let mut system_blocks = Vec::new();

    if let Some(prompt) = system_prompt.filter(|p| !p.trim().is_empty()) {
        system_blocks.push(json!({ "text": prompt }));
    }

    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        let content_val = msg.get("content");

        if role == "system" {
            if let Some(text) = content_val.and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    system_blocks.push(json!({ "text": text }));
                }
            }
            continue;
        }

        let mut content_blocks = Vec::new();
        if let Some(text) = content_val.and_then(Value::as_str) {
            if !text.is_empty() {
                content_blocks.push(json!({ "text": text }));
            }
        } else if let Some(arr) = content_val.and_then(Value::as_array) {
            for item in arr {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    content_blocks.push(json!({ "text": text }));
                }
            }
        }

        if content_blocks.is_empty() {
            content_blocks.push(json!({ "text": " " }));
        }

        let bedrock_role = match role {
            "assistant" => "assistant",
            _ => "user",
        };

        bedrock_messages.push(json!({
            "role": bedrock_role,
            "content": content_blocks
        }));
    }

    let mut payload = json!({
        "messages": bedrock_messages
    });

    if !system_blocks.is_empty() {
        payload["system"] = json!(system_blocks);
    }

    let mut inference_config = serde_json::Map::new();
    if let Some(tokens) = max_tokens.filter(|t| *t > 0) {
        inference_config.insert("maxTokens".to_string(), json!(tokens));
    }
    if let Some(temp) = temperature {
        inference_config.insert("temperature".to_string(), json!(temp));
    }
    if !inference_config.is_empty() {
        payload["inferenceConfig"] = Value::Object(inference_config);
    }

    payload
}

/// Decodes one AWS EventStream frame from `pending` bytes.
///
/// Returns `Ok(Some(payload_json))` when a complete valid frame is decoded,
/// `Ok(None)` when more data is needed, or an error on corrupt CRC/framing.
pub fn pop_eventstream_frame(pending: &mut Vec<u8>) -> Result<Option<Value>, ServiceError> {
    if pending.len() < 12 {
        return Ok(None);
    }

    let total_len = u32::from_be_bytes([pending[0], pending[1], pending[2], pending[3]]) as usize;
    let headers_len = u32::from_be_bytes([pending[4], pending[5], pending[6], pending[7]]) as usize;
    let prelude_crc = u32::from_be_bytes([pending[8], pending[9], pending[10], pending[11]]);

    if total_len < 16 || total_len > 16 * 1024 * 1024 || headers_len > total_len - 16 {
        return Err(ServiceError::new(
            "invalid_provider_response",
            "Bedrock event-stream frame length is invalid.",
            true,
        ));
    }

    let computed_prelude_crc = crc32(&pending[0..8]);
    if computed_prelude_crc != prelude_crc {
        return Err(ServiceError::new(
            "invalid_provider_response",
            "Bedrock event-stream prelude CRC check failed.",
            true,
        ));
    }

    if pending.len() < total_len {
        return Ok(None);
    }

    let expected_msg_crc = u32::from_be_bytes([
        pending[total_len - 4],
        pending[total_len - 3],
        pending[total_len - 2],
        pending[total_len - 1],
    ]);
    let computed_msg_crc = crc32(&pending[0..total_len - 4]);
    if computed_msg_crc != expected_msg_crc {
        return Err(ServiceError::new(
            "invalid_provider_response",
            "Bedrock event-stream message CRC check failed.",
            true,
        ));
    }

    let payload_start = 12 + headers_len;
    let payload_end = total_len - 4;
    let payload_bytes = &pending[payload_start..payload_end];

    let value = if payload_bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice::<Value>(payload_bytes).unwrap_or(Value::Null)
    };

    pending.drain(0..total_len);
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_standard_check_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn region_resolution_extracts_from_standard_and_custom_urls() {
        assert_eq!(
            bedrock_region_from_url("https://bedrock-runtime.us-east-1.amazonaws.com"),
            "us-east-1"
        );
        assert_eq!(
            bedrock_region_from_url("https://bedrock-runtime.eu-west-1.amazonaws.com/model/test"),
            "eu-west-1"
        );
        assert_eq!(
            bedrock_region_from_url("https://custom-gateway.local/bedrock"),
            "us-east-1"
        );
    }

    #[test]
    fn chat_url_formats_converse_and_stream_endpoints() {
        assert_eq!(
            bedrock_chat_url(
                "https://bedrock-runtime.us-east-1.amazonaws.com",
                "anthropic.claude-3-5-sonnet-20241022-v2:0",
                true
            ),
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-5-sonnet-20241022-v2:0/converse-stream"
        );
        assert_eq!(
            bedrock_chat_url(
                "https://bedrock-runtime.us-east-1.amazonaws.com",
                "meta.llama3-3-70b-instruct-v1:0",
                false
            ),
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/meta.llama3-3-70b-instruct-v1:0/converse"
        );
    }

    #[test]
    fn eventstream_frame_roundtrip_decoding() {
        let payload = br#"{"contentBlockDelta":{"delta":{"text":"hello"}}}"#;
        let headers_len = 0u32;
        let total_len = (16 + payload.len()) as u32;

        let mut frame = Vec::new();
        frame.extend_from_slice(&total_len.to_be_bytes());
        frame.extend_from_slice(&headers_len.to_be_bytes());
        let prelude_crc = crc32(&frame[0..8]);
        frame.extend_from_slice(&prelude_crc.to_be_bytes());
        frame.extend_from_slice(payload);
        let msg_crc = crc32(&frame);
        frame.extend_from_slice(&msg_crc.to_be_bytes());

        let mut buffer = frame;
        let result = pop_eventstream_frame(&mut buffer).expect("Frame should decode successfully");
        assert!(buffer.is_empty());
        let val = result.expect("Should return decoded JSON value");
        assert_eq!(
            val.pointer("/contentBlockDelta/delta/text")
                .and_then(Value::as_str),
            Some("hello")
        );
    }

    #[test]
    fn signed_bedrock_headers_include_sigv4_authorization() {
        let url = Url::parse("https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-5-sonnet-20241022-v2:0/converse-stream").unwrap();
        let body = b"{}";
        let api_key = "AKIAIOSFODNN7EXAMPLE:wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let headers = build_signed_bedrock_headers(&url, "POST", body, api_key).unwrap();

        assert!(headers.contains_key("authorization"));
        assert!(headers.contains_key("x-amz-date"));
        let auth = headers.get("authorization").unwrap().to_str().unwrap();
        assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/"));
        assert!(auth.contains("/us-east-1/bedrock/aws4_request"));
        assert!(auth.contains("SignedHeaders=content-type;host;x-amz-date"));
        assert!(auth.contains("Signature="));
    }
}
