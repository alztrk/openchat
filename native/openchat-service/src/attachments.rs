use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{Error as SqlError, types::Type};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::chatgpt_store::StoredAttachment;
use crate::{protocol::ServiceError, storage::AppStorage};

const MESSAGE_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
const MAXIMUM_FILE_COUNT: usize = 10;
const MAXIMUM_IMAGE_COUNT: usize = 3;
const MAXIMUM_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAXIMUM_TEXT_BYTES: usize = 1024 * 1024;
const MAXIMUM_TOTAL_BYTES: usize = 14 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
struct MessageEnvelope {
    content: String,
    attachments: Vec<AttachmentMetadata>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentMetadata {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) mime_type: String,
    pub(crate) size_bytes: usize,
    pub(crate) kind: String,
}

#[derive(Debug)]
pub(crate) struct AttachmentMetadataError(&'static str);

impl std::fmt::Display for AttachmentMetadataError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for AttachmentMetadataError {}

pub(crate) fn write_generated_images(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    images: &[(&str, &[u8])],
) -> Result<Vec<AttachmentMetadata>, ServiceError> {
    if images.is_empty() || images.len() > MAXIMUM_IMAGE_COUNT {
        return Err(image_storage_error(
            "The generated image count exceeds the supported attachment limit.",
        ));
    }
    if images
        .iter()
        .map(|(_, bytes)| bytes.len())
        .try_fold(0_usize, |total, size| total.checked_add(size))
        .is_none_or(|total| total > MAXIMUM_TOTAL_BYTES)
    {
        return Err(image_storage_error(
            "The generated image size exceeds the supported attachment limit.",
        ));
    }
    if !is_safe_identifier(conversation_id) || !is_safe_identifier(message_id) {
        return Err(image_storage_error(
            "The generated image destination is invalid.",
        ));
    }

    let attachments_root = storage_root.join("attachments");
    fs::create_dir_all(&attachments_root).map_err(|_| image_storage_unavailable())?;
    let canonical_root =
        fs::canonicalize(&attachments_root).map_err(|_| image_storage_unavailable())?;
    let message_directory = attachments_root.join(conversation_id).join(message_id);
    fs::create_dir_all(&message_directory).map_err(|_| image_storage_unavailable())?;
    let canonical_directory =
        fs::canonicalize(&message_directory).map_err(|_| image_storage_unavailable())?;
    if !canonical_directory.starts_with(&canonical_root) {
        return Err(image_storage_error(
            "The generated image destination is outside the attachment store.",
        ));
    }

    let mut metadata = Vec::with_capacity(images.len());
    let mut written_ids = Vec::with_capacity(images.len());
    for (mime_type, bytes) in images {
        let Some(extension) = image_extension(mime_type) else {
            if cleanup_generated_images(&canonical_directory, &written_ids).is_err() {
                return Err(image_storage_cleanup_unavailable());
            }
            return Err(image_storage_error(
                "The generated image format is unsupported.",
            ));
        };
        if bytes.is_empty()
            || bytes.len() > MAXIMUM_IMAGE_BYTES
            || !has_valid_image_signature(mime_type, bytes)
        {
            if cleanup_generated_images(&canonical_directory, &written_ids).is_err() {
                return Err(image_storage_cleanup_unavailable());
            }
            return Err(image_storage_error(
                "The generated image could not be validated.",
            ));
        }
        let id = Uuid::new_v4().simple().to_string();
        let file_name = format!("{id}.data");
        let path = canonical_directory.join(&file_name);
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            file.write_all(bytes)?;
            file.sync_all()
        })();
        if let Err(error) = write_result {
            let mut cleanup_ids = written_ids.clone();
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                cleanup_ids.push(id.clone());
            }
            if cleanup_generated_images(&canonical_directory, cleanup_ids).is_err() {
                return Err(image_storage_cleanup_unavailable());
            }
            return Err(image_storage_unavailable());
        }
        written_ids.push(id.clone());
        metadata.push(AttachmentMetadata {
            id,
            name: format!("generated-image.{extension}"),
            mime_type: (*mime_type).to_owned(),
            size_bytes: bytes.len(),
            kind: "image".to_owned(),
        });
    }
    Ok(metadata)
}

pub(crate) fn delete_generated_images(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachments: &[AttachmentMetadata],
) -> Result<(), ServiceError> {
    if !is_safe_identifier(conversation_id) || !is_safe_identifier(message_id) {
        return Err(image_storage_error(
            "The generated image destination is invalid.",
        ));
    }
    let directory = storage_root
        .join("attachments")
        .join(conversation_id)
        .join(message_id);
    let canonical_root = fs::canonicalize(storage_root.join("attachments"))
        .map_err(|_| image_storage_unavailable())?;
    let canonical_directory =
        fs::canonicalize(&directory).map_err(|_| image_storage_unavailable())?;
    if !canonical_directory.starts_with(&canonical_root) {
        return Err(image_storage_error(
            "The generated image destination is outside the attachment store.",
        ));
    }
    let ids = attachments
        .iter()
        .map(|attachment| attachment.id.as_str())
        .collect::<Vec<_>>();
    cleanup_generated_images(&canonical_directory, &ids)
        .map_err(|_| image_storage_cleanup_unavailable())
}

pub(crate) fn append_assistant_attachments(
    storage: &AppStorage,
    conversation_id: &str,
    message_id: &str,
    attachments: &[AttachmentMetadata],
) -> rusqlite::Result<()> {
    if attachments.is_empty() {
        return Ok(());
    }
    let database = storage.connect()?;
    let current_content = database.query_row(
        "SELECT content FROM messages
         WHERE conversation_id = ?1 AND id = ?2 AND role = 'assistant'",
        rusqlite::params![conversation_id, message_id],
        |row| row.get::<_, String>(0),
    )?;
    let content = append_attachment_metadata(&current_content, attachments)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let changed = database.execute(
        "UPDATE messages SET content = ?3
         WHERE conversation_id = ?1 AND id = ?2 AND role = 'assistant'",
        rusqlite::params![conversation_id, message_id, content],
    )?;
    if changed != 1 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub(crate) fn merge_content_preserving_attachments(
    existing_content: Option<&str>,
    content: &str,
) -> Result<String, AttachmentMetadataError> {
    let Some(existing_content) = existing_content else {
        return Ok(content.to_owned());
    };
    let Some(encoded) = existing_content.strip_prefix(MESSAGE_CONTENT_PREFIX) else {
        return Ok(content.to_owned());
    };
    let mut envelope = serde_json::from_str::<MessageEnvelope>(encoded)
        .map_err(|_| AttachmentMetadataError("The saved attachment envelope is invalid."))?;
    if envelope.attachments.is_empty() {
        return Err(AttachmentMetadataError(
            "The saved attachment envelope has no attachments.",
        ));
    }
    envelope.content = content.to_owned();
    encode_message_envelope(&envelope)
}

pub(crate) fn function_call_output_value(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    output: &Value,
) -> Result<Option<Value>, ServiceError> {
    let Some(value) = output.get("attachments") else {
        return Ok(None);
    };
    let attachments = serde_json::from_value::<Vec<AttachmentMetadata>>(value.clone())
        .map_err(|_| image_storage_error("The generated image metadata is invalid."))?;
    if attachments.is_empty() || attachments.len() > MAXIMUM_IMAGE_COUNT {
        return Err(image_storage_error(
            "The generated image metadata exceeds the supported limit.",
        ));
    }
    let mut content = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        validate_attachment_metadata(&attachment)
            .map_err(|_| image_storage_error("The generated image metadata is invalid."))?;
        let Some(bytes) = read_attachment(
            storage_root,
            conversation_id,
            message_id,
            &format!("{}.data", attachment.id),
            attachment.size_bytes,
            MAXIMUM_IMAGE_BYTES,
        ) else {
            return Err(ServiceError::new(
                "attachment_unavailable",
                "The generated image is no longer available for this response.",
                false,
            ));
        };
        content.push(json!({
            "type": "input_image",
            "image_url": format!(
                "data:{};base64,{}",
                attachment.mime_type,
                STANDARD.encode(bytes),
            ),
        }));
    }
    Ok(Some(Value::Array(content)))
}

fn append_attachment_metadata(
    existing_content: &str,
    attachments: &[AttachmentMetadata],
) -> Result<String, AttachmentMetadataError> {
    let mut envelope = match existing_content.strip_prefix(MESSAGE_CONTENT_PREFIX) {
        Some(encoded) => serde_json::from_str::<MessageEnvelope>(encoded)
            .map_err(|_| AttachmentMetadataError("The saved attachment envelope is invalid."))?,
        None => MessageEnvelope {
            content: existing_content.to_owned(),
            attachments: Vec::new(),
        },
    };
    if attachments.is_empty()
        || envelope.attachments.len().saturating_add(attachments.len()) > MAXIMUM_FILE_COUNT
    {
        return Err(AttachmentMetadataError(
            "The attachment count exceeds the supported limit.",
        ));
    }
    let mut total_bytes = envelope
        .attachments
        .iter()
        .map(|attachment| attachment.size_bytes)
        .sum::<usize>();
    let mut image_count = envelope
        .attachments
        .iter()
        .filter(|attachment| attachment.kind == "image")
        .count();
    for attachment in attachments {
        validate_attachment_metadata(attachment)?;
        if envelope
            .attachments
            .iter()
            .any(|existing| existing.id == attachment.id)
        {
            return Err(AttachmentMetadataError(
                "The attachment identifier is already in use.",
            ));
        }
        total_bytes = total_bytes.saturating_add(attachment.size_bytes);
        if total_bytes > MAXIMUM_TOTAL_BYTES {
            return Err(AttachmentMetadataError(
                "The attachment size limit was exceeded.",
            ));
        }
        if attachment.kind == "image" {
            image_count = image_count.saturating_add(1);
            if image_count > MAXIMUM_IMAGE_COUNT {
                return Err(AttachmentMetadataError(
                    "The image attachment limit was exceeded.",
                ));
            }
        }
        envelope.attachments.push(attachment.clone());
    }
    encode_message_envelope(&envelope)
}

fn encode_message_envelope(envelope: &MessageEnvelope) -> Result<String, AttachmentMetadataError> {
    serde_json::to_string(envelope)
        .map(|encoded| format!("{MESSAGE_CONTENT_PREFIX}{encoded}"))
        .map_err(|_| AttachmentMetadataError("The attachment envelope could not be encoded."))
}

fn validate_attachment_metadata(
    metadata: &AttachmentMetadata,
) -> Result<(), AttachmentMetadataError> {
    if !is_safe_identifier(&metadata.id)
        || metadata.name.trim().is_empty()
        || metadata.size_bytes == 0
        || metadata.kind != "image"
        || !matches!(
            metadata.mime_type.as_str(),
            "image/png" | "image/jpeg" | "image/webp"
        )
        || metadata.size_bytes > MAXIMUM_IMAGE_BYTES
    {
        return Err(AttachmentMetadataError(
            "Invalid image attachment metadata.",
        ));
    }
    Ok(())
}

fn image_extension(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

fn cleanup_generated_images<I, S>(directory: &Path, ids: I) -> std::io::Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut first_error = None;
    for id in ids {
        let id = id.as_ref();
        if is_safe_identifier(id) {
            match fs::remove_file(directory.join(format!("{id}.data"))) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) if first_error.is_none() => first_error = Some(error),
                Err(_) => {}
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

fn image_storage_error(message: &'static str) -> ServiceError {
    ServiceError::new("image_storage_unavailable", message, true)
}

fn image_storage_unavailable() -> ServiceError {
    image_storage_error("The generated image could not be saved locally.")
}

fn image_storage_cleanup_unavailable() -> ServiceError {
    image_storage_error("The generated image could not be safely cleaned up locally.")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nopenchat-test-image";

    fn test_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("openchat-generated-image-{}", Uuid::new_v4()))
    }

    #[test]
    fn generated_images_are_written_before_structured_follow_up_output() {
        let root = test_root();
        let attachments = write_generated_images(
            &root,
            "conversation-1",
            "assistant-1",
            &[("image/png", PNG)],
        )
        .expect("generated image should be stored");
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].kind, "image");
        assert_eq!(attachments[0].mime_type, "image/png");
        assert_eq!(attachments[0].size_bytes, PNG.len());
        assert_eq!(attachments[0].name, "generated-image.png");

        let stored_path = root
            .join("attachments")
            .join("conversation-1")
            .join("assistant-1")
            .join(format!("{}.data", attachments[0].id));
        assert_eq!(fs::read(&stored_path).expect("stored bytes"), PNG);

        let envelope = append_attachment_metadata("", &attachments)
            .expect("generated metadata should use the existing envelope");
        let (content, hydrated) =
            decode_message_content(envelope, &root, "conversation-1", "assistant-1")
                .expect("stored attachment metadata should hydrate");
        assert!(content.contains("[Attached image: generated-image.png]"));
        assert_eq!(hydrated.len(), 1);
        assert_eq!(hydrated[0].mime_type, "image/png");
        assert_eq!(hydrated[0].content.as_deref(), Some(PNG));

        let tool_output = json!({"attachments": attachments});
        let follow_up =
            function_call_output_value(&root, "conversation-1", "assistant-1", &tool_output)
                .expect("follow-up image should be readable")
                .expect("image attachment should become input content");
        assert_eq!(follow_up[0]["type"], "input_image");
        assert!(
            follow_up[0]["image_url"]
                .as_str()
                .is_some_and(|value| value.starts_with("data:image/png;base64,"))
        );
        assert!(!tool_output.to_string().contains("base64"));

        let metadata =
            serde_json::from_value::<Vec<AttachmentMetadata>>(tool_output["attachments"].clone())
                .expect("metadata should round-trip");
        delete_generated_images(&root, "conversation-1", "assistant-1", &metadata)
            .expect("generated image should be cleaned up");
        assert!(!stored_path.exists());
        fs::remove_dir_all(root).expect("temporary image root should be removed");
    }

    #[test]
    fn assistant_content_merge_keeps_generated_attachment_metadata() {
        let metadata = AttachmentMetadata {
            id: "image-1".to_owned(),
            name: "generated-image.png".to_owned(),
            mime_type: "image/png".to_owned(),
            size_bytes: PNG.len(),
            kind: "image".to_owned(),
        };
        let envelope = append_attachment_metadata("partial response", &[metadata])
            .expect("attachment envelope should encode");
        let merged = merge_content_preserving_attachments(Some(&envelope), "final response")
            .expect("assistant content should merge");
        assert!(merged.starts_with(MESSAGE_CONTENT_PREFIX));
        let decoded = serde_json::from_str::<MessageEnvelope>(
            merged
                .strip_prefix(MESSAGE_CONTENT_PREFIX)
                .expect("attachment envelope prefix"),
        )
        .expect("attachment envelope should decode");
        assert_eq!(decoded.content, "final response");
        assert_eq!(decoded.attachments.len(), 1);
        assert_eq!(decoded.attachments[0].id, "image-1");
    }
}

pub(crate) fn decode_message_content(
    value: String,
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
) -> Result<(String, Vec<StoredAttachment>), SqlError> {
    let Some(encoded) = value.strip_prefix(MESSAGE_CONTENT_PREFIX) else {
        return Ok((value, Vec::new()));
    };
    let envelope = serde_json::from_str::<MessageEnvelope>(encoded)
        .map_err(|error| SqlError::FromSqlConversionFailure(2, Type::Text, Box::new(error)))?;
    if envelope.attachments.is_empty()
        || envelope.attachments.len() > MAXIMUM_FILE_COUNT
        || !is_safe_identifier(conversation_id)
        || !is_safe_identifier(message_id)
    {
        return Err(SqlError::FromSqlConversionFailure(
            2,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid attachment metadata.",
            )),
        ));
    }

    let mut total_bytes = 0_usize;
    let mut image_count = 0_usize;
    let mut effective_content = envelope.content;
    let mut attachments = Vec::with_capacity(envelope.attachments.len());
    for metadata in envelope.attachments {
        let max_size = match metadata.kind.as_str() {
            "image"
                if matches!(
                    metadata.mime_type.as_str(),
                    "image/png" | "image/jpeg" | "image/webp"
                ) =>
            {
                image_count = image_count.saturating_add(1);
                MAXIMUM_IMAGE_BYTES
            }
            "text" if is_text_mime_type(&metadata.mime_type) => MAXIMUM_TEXT_BYTES,
            _ => {
                return Err(SqlError::FromSqlConversionFailure(
                    2,
                    Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid attachment type.",
                    )),
                ));
            }
        };
        if !is_safe_identifier(&metadata.id)
            || metadata.name.trim().is_empty()
            || metadata.size_bytes == 0
            || metadata.size_bytes > max_size
        {
            return Err(SqlError::FromSqlConversionFailure(
                2,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid attachment metadata.",
                )),
            ));
        }
        total_bytes = total_bytes.saturating_add(metadata.size_bytes);
        if image_count > MAXIMUM_IMAGE_COUNT || total_bytes > MAXIMUM_TOTAL_BYTES {
            return Err(SqlError::FromSqlConversionFailure(
                2,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Attachment limits were exceeded.",
                )),
            ));
        }
        let mut attachment_content = read_attachment(
            storage_root,
            conversation_id,
            message_id,
            &format!("{}.data", metadata.id),
            metadata.size_bytes,
            max_size,
        );
        if metadata.kind == "image"
            && attachment_content
                .as_deref()
                .is_some_and(|bytes| !has_valid_image_signature(&metadata.mime_type, bytes))
        {
            attachment_content = None;
        }
        if metadata.kind == "text"
            && let Some(bytes) = attachment_content.as_deref()
            && let Ok(text) = std::str::from_utf8(bytes)
        {
            effective_content.push_str(&format!(
                "\n\n[Attached text file: {}]\n{}",
                metadata.name, text
            ));
        } else if metadata.kind == "image" {
            effective_content.push_str(&format!("\n\n[Attached image: {}]", metadata.name));
        }
        attachments.push(StoredAttachment {
            mime_type: metadata.mime_type,
            kind: metadata.kind,
            content: attachment_content,
        });
    }

    Ok((effective_content, attachments))
}

fn read_attachment(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    file_name: &str,
    expected_size: usize,
    maximum_size: usize,
) -> Option<Vec<u8>> {
    let attachments_root = storage_root.join("attachments");
    let message_directory = attachments_root.join(conversation_id).join(message_id);
    let file_path = message_directory.join(file_name);
    let canonical_root = fs::canonicalize(&attachments_root).ok()?;
    let canonical_directory = fs::canonicalize(&message_directory).ok()?;
    let canonical_file = fs::canonicalize(file_path).ok()?;
    // Windows may redirect app-local data outside the requested storage root.
    if !canonical_directory.starts_with(&canonical_root)
        || !canonical_file.starts_with(&canonical_directory)
    {
        return None;
    }
    let metadata = fs::metadata(&canonical_file).ok()?;
    if metadata.len() != u64::try_from(expected_size).ok()?
        || metadata.len() > u64::try_from(maximum_size).ok()?
    {
        return None;
    }
    let file = File::open(canonical_file).ok()?;
    let limit = u64::try_from(maximum_size).ok()?.saturating_add(1);
    let mut bytes = Vec::with_capacity(expected_size);
    file.take(limit).read_to_end(&mut bytes).ok()?;
    (bytes.len() == expected_size).then_some(bytes)
}

fn is_text_mime_type(mime_type: &str) -> bool {
    mime_type.starts_with("text/") || matches!(mime_type, "application/json" | "application/xml")
}

fn has_valid_image_signature(mime_type: &str, bytes: &[u8]) -> bool {
    match mime_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
