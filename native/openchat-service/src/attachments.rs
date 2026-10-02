use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use rusqlite::{Error as SqlError, types::Type};
use serde::Deserialize;

use crate::chatgpt_store::StoredAttachment;

const MESSAGE_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
const MAXIMUM_FILE_COUNT: usize = 10;
const MAXIMUM_IMAGE_COUNT: usize = 3;
const MAXIMUM_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAXIMUM_TEXT_BYTES: usize = 1024 * 1024;
const MAXIMUM_TOTAL_BYTES: usize = 14 * 1024 * 1024;

#[derive(Deserialize)]
struct MessageEnvelope {
    content: String,
    attachments: Vec<AttachmentMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentMetadata {
    id: String,
    name: String,
    mime_type: String,
    size_bytes: usize,
    kind: String,
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
