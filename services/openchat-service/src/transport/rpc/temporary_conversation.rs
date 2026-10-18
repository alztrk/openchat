//! Request validation and provider routing for non-persistent conversations.

use std::{collections::HashSet, sync::Arc};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;

use crate::{
    chatgpt::{ChatGptService, TemporaryChatRequest},
    openai_compatible,
    protocol::{EventSink, ServiceError},
    provider_credentials::{self, ProviderCredentials},
    storage::AppStorage,
};

use super::{
    bounded_route_id, invalid_request_params, optional_string, required_string,
    take_optional_api_key,
};

const MAXIMUM_ATTACHMENT_MIME_TYPE_BYTES: usize = 128;

/// Validates and sends one temporary conversation turn without persisting its transcript.
pub(super) async fn dispatch(
    params: &mut Value,
    request_id: Value,
    storage: &AppStorage,
    service: &Arc<ChatGptService>,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
    events: &EventSink,
) -> Result<Value, ServiceError> {
    let _untrusted_api_key = take_optional_api_key(params, "apiKey")?;
    let conversation_id = required_string(params, "conversationId")?.to_owned();
    if !crate::temporary_conversation::is_valid_temporary_conversation_id(&conversation_id) {
        return Err(invalid_request_params());
    }
    let provider_id = bounded_route_id(params, "providerId")?.to_owned();
    let model_id = bounded_route_id(params, "modelId")?.to_owned();
    let api_key_connection_id = optional_string(params, "apiKeyConnectionId")?.map(str::to_owned);
    let (messages, has_attachments) = parse_messages(params)?;
    let api_key_connection_id = api_key_connection_id.as_deref();
    if provider_id == "chatgpt" {
        if api_key_connection_id.is_some() {
            return Err(invalid_request_params());
        }
        let connection_id = bounded_route_id(params, "connectionId")?;
        let workspace_id = bounded_route_id(params, "workspaceId")?;
        service
            .send_temporary_message(TemporaryChatRequest {
                request_id,
                conversation_id: &conversation_id,
                connection_id,
                workspace_id,
                model_id: &model_id,
                messages,
                has_attachments,
                cancellation,
                events,
            })
            .await
    } else {
        let credentials = if provider_credentials::uses_saved_api_key(&provider_id) {
            Some(ProviderCredentials::load()?)
        } else {
            None
        };
        let api_key = credentials
            .as_ref()
            .map(|credentials| credentials.api_key(&provider_id, api_key_connection_id))
            .transpose()?
            .flatten();
        if provider_credentials::requires_saved_api_key(&provider_id) && api_key.is_none() {
            return Err(provider_credentials::missing_credentials());
        }
        let custom_provider = if provider_id.starts_with("custom_") {
            Some(
                credentials
                    .as_ref()
                    .ok_or_else(provider_credentials::missing_credentials)?
                    .custom_provider_config(&provider_id)?,
            )
        } else {
            None
        };
        openai_compatible::send_temporary_message(openai_compatible::TemporaryChatContext {
            storage,
            service,
            request_id,
            conversation_id: &conversation_id,
            provider_id: &provider_id,
            model_id: &model_id,
            api_key: api_key.as_deref().map(|secret| secret.as_str()),
            api_key_connection_id,
            custom_provider,
            provider_messages: messages,
            has_attachments,
            cancellation,
            events,
        })
        .await
    }
}

/// Validates a temporary transcript and converts its attachments for provider requests.
fn parse_messages(
    params: &Value,
) -> Result<(Vec<crate::provider_schema::ProviderMessage>, bool), ServiceError> {
    let values = params
        .get("messages")
        .and_then(Value::as_array)
        .filter(|messages| !messages.is_empty() && messages.len() <= 100)
        .ok_or_else(invalid_request_params)?;
    let mut total_text_bytes = 0usize;
    let mut total_attachment_bytes = 0usize;
    let mut previous_role = None;
    let mut has_attachments = false;
    let mut messages = Vec::with_capacity(values.len());
    for value in values {
        let role = match value.get("role").and_then(Value::as_str) {
            Some("user") => crate::provider_schema::MessageRole::User,
            Some("assistant") => crate::provider_schema::MessageRole::Assistant,
            _ => return Err(invalid_request_params()),
        };
        if previous_role == Some(role) {
            return Err(invalid_request_params());
        }
        let original_content = value
            .get("content")
            .and_then(Value::as_str)
            .filter(|content| content.len() <= 64 * 1024)
            .ok_or_else(invalid_request_params)?;
        total_text_bytes = total_text_bytes.saturating_add(original_content.len());
        if total_text_bytes > 768 * 1024 {
            return Err(ServiceError::new(
                "temporary_transcript_too_large",
                "The temporary conversation transcript exceeds its request limit.",
                false,
            ));
        }
        let raw_attachments = match value.get("attachments") {
            None => &[][..],
            Some(Value::Array(attachments)) if attachments.len() <= 10 => attachments,
            _ => return Err(invalid_request_params()),
        };
        if role != crate::provider_schema::MessageRole::User && !raw_attachments.is_empty() {
            return Err(invalid_request_params());
        }
        if original_content.trim().is_empty() && raw_attachments.is_empty() {
            return Err(invalid_request_params());
        }
        let mut content = original_content.to_owned();
        let mut images = Vec::new();
        let mut attachment_ids = HashSet::with_capacity(raw_attachments.len());
        let mut image_count = 0usize;
        for attachment in raw_attachments {
            let metadata = parse_attachment_metadata(attachment)?;
            if !attachment_ids.insert(metadata.id.clone()) {
                return Err(invalid_request_params());
            }
            let encoded = attachment
                .get("data")
                .and_then(Value::as_str)
                .filter(|value| value.len() <= 19 * 1024 * 1024)
                .ok_or_else(invalid_request_params)?;
            let expected_encoded_length = metadata
                .size_bytes
                .div_ceil(3)
                .checked_mul(4)
                .ok_or_else(invalid_request_params)?;
            if encoded.len() != expected_encoded_length {
                return Err(invalid_request_params());
            }
            let bytes = STANDARD
                .decode(encoded)
                .map_err(|_| invalid_request_params())?;
            crate::attachments::validate_attachment(&metadata, &bytes)
                .map_err(|_| invalid_request_params())?;
            total_attachment_bytes = total_attachment_bytes.saturating_add(bytes.len());
            if total_attachment_bytes > crate::attachments::MAXIMUM_TOTAL_BYTES {
                return Err(ServiceError::new(
                    "temporary_transcript_too_large",
                    "The temporary conversation attachments exceed their request limit.",
                    false,
                ));
            }
            has_attachments = true;
            if metadata.kind == "image" {
                image_count += 1;
                if image_count > crate::attachments::MAXIMUM_IMAGE_COUNT {
                    return Err(invalid_request_params());
                }
                images.push(format!("data:{};base64,{}", metadata.mime_type, encoded));
            } else {
                let attachment_text =
                    String::from_utf8(bytes).map_err(|_| invalid_request_params())?;
                content.push_str(&format!(
                    "\n\n<openchat-user-file name={:?} mime_type={:?}>\n{}\n</openchat-user-file>",
                    metadata.name, metadata.mime_type, attachment_text
                ));
            }
        }
        messages.push(crate::provider_schema::ProviderMessage {
            role,
            content,
            images,
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
        previous_role = Some(role);
    }
    if messages
        .first()
        .is_none_or(|message| message.role != crate::provider_schema::MessageRole::User)
        || messages
            .last()
            .is_none_or(|message| message.role != crate::provider_schema::MessageRole::User)
    {
        return Err(invalid_request_params());
    }
    Ok((messages, has_attachments))
}

/// Extracts bounded attachment metadata without copying the attachment payload.
fn parse_attachment_metadata(
    value: &Value,
) -> Result<crate::attachments::AttachmentMetadata, ServiceError> {
    let id = bounded_attachment_string(value, "id", 128)?;
    let name = bounded_attachment_string(value, "name", 255 * 4)?;
    let mime_type =
        bounded_attachment_string(value, "mimeType", MAXIMUM_ATTACHMENT_MIME_TYPE_BYTES)?;
    let kind = bounded_attachment_string(value, "kind", 5)?;
    let size_bytes = value
        .get("sizeBytes")
        .and_then(Value::as_u64)
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(invalid_request_params)?;
    let metadata = crate::attachments::AttachmentMetadata {
        id: id.to_owned(),
        name: name.to_owned(),
        mime_type: mime_type.to_owned(),
        size_bytes,
        kind: kind.to_owned(),
    };
    crate::attachments::validate_attachment_metadata(&metadata)
        .map_err(|_| invalid_request_params())?;
    Ok(metadata)
}

/// Borrows one string field only when it fits the field's established byte limit.
fn bounded_attachment_string<'a>(
    value: &'a Value,
    field: &str,
    maximum_bytes: usize,
) -> Result<&'a str, ServiceError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| value.len() <= maximum_bytes)
        .ok_or_else(invalid_request_params)
}

#[cfg(test)]
mod tests {
    use std::{hint::black_box, time::Instant};

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde_json::json;

    use super::{parse_attachment_metadata, parse_messages};

    #[test]
    fn transcripts_accept_only_bounded_user_assistant_turns() {
        let messages = parse_messages(&json!({
            "messages": [
                {"role": "user", "content": "Question"},
                {"role": "assistant", "content": "Answer"},
                {"role": "user", "content": "Follow-up"}
            ]
        }))
        .expect("valid temporary transcript");
        assert_eq!(messages.0.len(), 3);
        assert!(
            parse_messages(&json!({
                "messages": [{"role": "tool", "content": "not allowed"}]
            }))
            .is_err()
        );
        assert!(
            parse_messages(&json!({
                "messages": [
                    {"role": "user", "content": "Question"},
                    {"role": "user", "content": "Repeated role"}
                ]
            }))
            .is_err()
        );
        assert!(
            parse_messages(&json!({
                "messages": [{"role": "user", "content": "x".repeat(768 * 1024 + 1)}]
            }))
            .is_err()
        );
    }

    #[test]
    fn attachment_metadata_is_bounded_before_owned_values_are_created() {
        let valid = json!({
            "id": "file-1",
            "name": "notes.txt",
            "mimeType": "text/plain",
            "sizeBytes": 4,
            "kind": "text",
            "data": "not copied into metadata",
        });
        assert_eq!(
            parse_attachment_metadata(&valid)
                .expect("valid metadata")
                .name,
            "notes.txt"
        );

        let oversized_mime_type = json!({
            "id": "file-1",
            "name": "notes.txt",
            "mimeType": format!("text/{}", "a".repeat(128)),
            "sizeBytes": 4,
            "kind": "text",
        });
        assert!(parse_attachment_metadata(&oversized_mime_type).is_err());
    }

    #[test]
    fn temporary_image_attachments_preserve_provider_payload() {
        let image = b"\x89PNG\r\n\x1a\n";
        let encoded = STANDARD.encode(image);
        let messages = parse_messages(&json!({
            "messages": [{
                "role": "user",
                "content": "Describe this image.",
                "attachments": [{
                    "id": "image-1",
                    "name": "image.png",
                    "mimeType": "image/png",
                    "sizeBytes": image.len(),
                    "kind": "image",
                    "data": encoded,
                }],
            }],
        }))
        .expect("valid image attachment");

        assert!(messages.1);
        assert_eq!(
            messages.0[0].images,
            [format!("data:image/png;base64,{encoded}")]
        );
    }

    #[test]
    fn temporary_attachments_reject_base64_size_mismatch_before_decoding() {
        let image = b"\x89PNG\r\n\x1a\n";
        let result = parse_messages(&json!({
            "messages": [{
                "role": "user",
                "content": "Describe this image.",
                "attachments": [{
                    "id": "image-1",
                    "name": "image.png",
                    "mimeType": "image/png",
                    "sizeBytes": 2,
                    "kind": "image",
                    "data": STANDARD.encode(image),
                }],
            }],
        }));

        assert!(result.is_err());
    }

    #[test]
    #[ignore = "manual comparison of cloned and bounded image metadata parsing"]
    fn benchmark_attachment_metadata_parsing() {
        const IMAGE_BYTES: usize = crate::attachments::MAXIMUM_IMAGE_BYTES;
        let mut image = vec![0; IMAGE_BYTES];
        image[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        let encoded = STANDARD.encode(image);
        let attachment = json!({
            "id": "image-1",
            "name": "image.png",
            "mimeType": "image/png",
            "sizeBytes": IMAGE_BYTES,
            "kind": "image",
            "data": encoded,
        });

        black_box(
            serde_json::from_value::<crate::attachments::AttachmentMetadata>(attachment.clone())
                .expect("valid cloned metadata"),
        );
        black_box(parse_attachment_metadata(&attachment).expect("valid bounded metadata"));
        for sample in 1..=5 {
            let started = Instant::now();
            let cloned = serde_json::from_value::<crate::attachments::AttachmentMetadata>(
                attachment.clone(),
            )
            .expect("valid cloned metadata");
            crate::attachments::validate_attachment_metadata(&cloned)
                .expect("valid cloned metadata");
            black_box(cloned);
            let cloned_elapsed = started.elapsed().as_micros();

            let started = Instant::now();
            black_box(parse_attachment_metadata(&attachment).expect("valid bounded metadata"));
            println!(
                "attachment_metadata sample={sample} cloned_us={cloned_elapsed} bounded_us={}",
                started.elapsed().as_micros(),
            );
        }
    }
}
