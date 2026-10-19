//! Attachment validation, secure filesystem staging, and storage operations.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::{params, OptionalExtension};

use crate::request_bounds::BoundedText;

use super::{
    is_safe_identifier, open_database, storage_root, unix_time_milliseconds, ChatStorageError,
    IncomingMessageAttachment, StoredAttachmentMetadata, StoredMessageContentEnvelope,
    ValidatedAttachment, MAXIMUM_ATTACHMENT_FILE_COUNT, MAXIMUM_ATTACHMENT_IMAGE_BYTES,
    MAXIMUM_ATTACHMENT_IMAGE_COUNT, MAXIMUM_ATTACHMENT_TEXT_BYTES, MAXIMUM_ATTACHMENT_TOTAL_BYTES,
    MAXIMUM_SAFE_IDENTIFIER_BYTES, MESSAGE_CONTENT_PREFIX,
};

#[tauri::command]
/// Returns one validated attachment's bytes for a message in the specified conversation.
///
/// The lookup is scoped to the conversation/message pair and applies attachment size limits.
///
/// # Errors
/// Returns a validation or storage error when the message or attachment is unavailable.
pub fn read_message_attachment(
    conversation_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    message_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
    attachment_id: BoundedText<MAXIMUM_SAFE_IDENTIFIER_BYTES>,
) -> Result<String, ChatStorageError> {
    let conversation_id = conversation_id.into_inner();
    let message_id = message_id.into_inner();
    let attachment_id = attachment_id.into_inner();
    if !is_safe_identifier(&conversation_id)
        || !is_safe_identifier(&message_id)
        || !is_safe_identifier(&attachment_id)
    {
        return Err(ChatStorageError::invalid_input());
    }
    let connection = open_database()?;
    let stored_content = connection
        .query_row(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id, message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| ChatStorageError::unavailable())?
        .ok_or_else(ChatStorageError::unavailable)?;
    let envelope = parse_stored_message_content(&stored_content)?;
    let metadata = envelope
        .attachments
        .iter()
        .find(|attachment| attachment.id == attachment_id)
        .ok_or_else(ChatStorageError::unavailable)?;
    let storage_root = storage_root()?;
    let attachments_root =
        checked_attachment_root(&storage_root, false)?.ok_or_else(ChatStorageError::unavailable)?;
    let conversation_directory =
        checked_attachment_subdirectory(&attachments_root, &conversation_id, false)?
            .ok_or_else(ChatStorageError::unavailable)?;
    let canonical_directory =
        checked_attachment_subdirectory(&conversation_directory, &message_id, false)?
            .ok_or_else(ChatStorageError::unavailable)?;
    let attachment_path = canonical_directory.join(format!("{}.data", metadata.id));
    let attachment_metadata =
        fs::symlink_metadata(&attachment_path).map_err(|_| ChatStorageError::unavailable())?;
    if attachment_metadata.file_type().is_symlink() || !attachment_metadata.is_file() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_attachment =
        fs::canonicalize(&attachment_path).map_err(|_| ChatStorageError::unavailable())?;
    let expected_attachment = canonical_directory.join(format!("{}.data", metadata.id));
    if canonical_attachment != expected_attachment
        || !canonical_attachment.starts_with(&canonical_directory)
    {
        return Err(ChatStorageError::unavailable());
    }
    let bytes = fs::read(canonical_attachment).map_err(|_| ChatStorageError::unavailable())?;
    validate_stored_attachment(metadata, &bytes)?;
    Ok(STANDARD.encode(bytes))
}

pub(crate) fn validate_incoming_attachments(
    attachments: &[IncomingMessageAttachment],
) -> Result<Vec<ValidatedAttachment>, ChatStorageError> {
    if attachments.len() > MAXIMUM_ATTACHMENT_FILE_COUNT {
        return Err(ChatStorageError::invalid_input());
    }
    let mut ids = std::collections::HashSet::with_capacity(attachments.len());
    let mut total_bytes = 0_usize;
    let mut image_count = 0_usize;
    let mut validated = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let metadata = StoredAttachmentMetadata {
            id: attachment.id.clone(),
            name: attachment.name.clone(),
            mime_type: attachment.mime_type.clone(),
            size_bytes: attachment.size_bytes,
            kind: attachment.kind.clone(),
        };
        validate_attachment_metadata(&metadata)?;
        if !ids.insert(metadata.id.clone()) {
            return Err(ChatStorageError::invalid_input());
        }
        let (_, _, maximum_bytes) =
            attachment_spec(&metadata.name).ok_or_else(ChatStorageError::invalid_input)?;
        let maximum_base64_length = maximum_bytes.div_ceil(3).saturating_mul(4);
        if attachment.data_base64.len() > maximum_base64_length {
            return Err(ChatStorageError::invalid_input());
        }
        let bytes = STANDARD
            .decode(&attachment.data_base64)
            .map_err(|_| ChatStorageError::invalid_input())?;
        validate_stored_attachment(&metadata, &bytes)?;
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or_else(ChatStorageError::invalid_input)?;
        if total_bytes > MAXIMUM_ATTACHMENT_TOTAL_BYTES {
            return Err(ChatStorageError::invalid_input());
        }
        if metadata.kind == "image" {
            image_count += 1;
            if image_count > MAXIMUM_ATTACHMENT_IMAGE_COUNT {
                return Err(ChatStorageError::invalid_input());
            }
        }
        validated.push(ValidatedAttachment { metadata, bytes });
    }
    Ok(validated)
}

pub(crate) fn validate_attachment_metadata(
    metadata: &StoredAttachmentMetadata,
) -> Result<(), ChatStorageError> {
    if !is_safe_identifier(&metadata.id)
        || metadata.name.trim().is_empty()
        || metadata.name.chars().count() > 255
        || metadata.name.chars().any(char::is_control)
        || metadata.size_bytes == 0
    {
        return Err(ChatStorageError::invalid_input());
    }
    let (kind, mime_type, maximum_bytes) =
        attachment_spec(&metadata.name).ok_or_else(ChatStorageError::invalid_input)?;
    if metadata.kind != kind
        || metadata.mime_type != mime_type
        || metadata.size_bytes > maximum_bytes
    {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

pub(crate) fn validate_stored_attachment(
    metadata: &StoredAttachmentMetadata,
    bytes: &[u8],
) -> Result<(), ChatStorageError> {
    validate_attachment_metadata(metadata)?;
    if bytes.is_empty() || bytes.len() != metadata.size_bytes {
        return Err(ChatStorageError::invalid_input());
    }
    let valid_content = match metadata.kind.as_str() {
        "image" => has_valid_image_signature(&metadata.mime_type, bytes),
        "text" => std::str::from_utf8(bytes).is_ok(),
        _ => false,
    };
    if !valid_content {
        return Err(ChatStorageError::invalid_input());
    }
    Ok(())
}

pub(crate) fn attachment_spec(name: &str) -> Option<(&'static str, &'static str, usize)> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let spec = match extension.as_str() {
        "png" => ("image", "image/png", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "jpg" | "jpeg" => ("image", "image/jpeg", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "webp" => ("image", "image/webp", MAXIMUM_ATTACHMENT_IMAGE_BYTES),
        "txt" | "log" => ("text", "text/plain", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "md" => ("text", "text/markdown", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "csv" => ("text", "text/csv", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "json" | "jsonl" => ("text", "application/json", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "xml" => ("text", "application/xml", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "html" => ("text", "text/html", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "css" => ("text", "text/css", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "js" | "jsx" => ("text", "text/javascript", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "ts" | "tsx" => ("text", "text/typescript", MAXIMUM_ATTACHMENT_TEXT_BYTES),
        "dart" | "rs" | "py" | "go" | "java" | "kt" | "swift" | "c" | "h" | "cpp" | "hpp"
        | "cs" | "sh" | "ps1" | "yaml" | "yml" | "toml" | "ini" | "sql" => {
            ("text", "text/plain", MAXIMUM_ATTACHMENT_TEXT_BYTES)
        }
        _ => return None,
    };
    Some(spec)
}

pub(crate) fn has_valid_image_signature(mime_type: &str, bytes: &[u8]) -> bool {
    match mime_type {
        "image/png" => bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

pub(crate) fn parse_stored_message_content(
    stored_content: &str,
) -> Result<StoredMessageContentEnvelope, ChatStorageError> {
    let encoded = stored_content
        .strip_prefix(MESSAGE_CONTENT_PREFIX)
        .ok_or_else(ChatStorageError::unavailable)?;
    let envelope = serde_json::from_str::<StoredMessageContentEnvelope>(encoded)
        .map_err(|_| ChatStorageError::unavailable())?;
    if envelope.attachments.is_empty() || envelope.attachments.len() > MAXIMUM_ATTACHMENT_FILE_COUNT
    {
        return Err(ChatStorageError::unavailable());
    }
    let mut ids = std::collections::HashSet::with_capacity(envelope.attachments.len());
    let mut total_bytes = 0_usize;
    let mut image_count = 0_usize;
    for attachment in &envelope.attachments {
        validate_attachment_metadata(attachment).map_err(|_| ChatStorageError::unavailable())?;
        if !ids.insert(attachment.id.as_str()) {
            return Err(ChatStorageError::unavailable());
        }
        total_bytes = total_bytes.saturating_add(attachment.size_bytes);
        if attachment.kind == "image" {
            image_count += 1;
        }
    }
    if total_bytes > MAXIMUM_ATTACHMENT_TOTAL_BYTES || image_count > MAXIMUM_ATTACHMENT_IMAGE_COUNT
    {
        return Err(ChatStorageError::unavailable());
    }
    Ok(envelope)
}

pub(crate) fn stage_message_attachments(
    conversation_id: &str,
    message_id: &str,
    attachments: &[ValidatedAttachment],
) -> Result<(PathBuf, PathBuf), ChatStorageError> {
    if !is_safe_identifier(conversation_id) || !is_safe_identifier(message_id) {
        return Err(ChatStorageError::invalid_input());
    }
    let root = storage_root()?;
    let canonical_root =
        checked_attachment_root(&root, true)?.ok_or_else(ChatStorageError::unavailable)?;
    let canonical_conversation =
        checked_attachment_subdirectory(&canonical_root, conversation_id, true)?
            .ok_or_else(ChatStorageError::unavailable)?;
    let destination = canonical_conversation.join(message_id);
    if destination.exists() {
        return Err(ChatStorageError::unavailable());
    }
    let staging =
        canonical_conversation.join(format!(".stage-{message_id}-{}", unix_time_milliseconds()?));
    fs::create_dir(&staging).map_err(|_| ChatStorageError::unavailable())?;
    for attachment in attachments {
        let path = staging.join(format!("{}.data", attachment.metadata.id));
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.write_all(&attachment.bytes)?;
            file.sync_all()
        })();
        if write_result.is_err() {
            cleanup_attachment_directory(&staging)?;
            return Err(ChatStorageError::unavailable());
        }
    }
    Ok((staging, destination))
}

/// Resolves the attachment root only when it is a real directory under application storage.
///
/// When `create` is true, the directory is created if absent; an absent directory otherwise
/// remains an explicit `None` result so deletion can be idempotent.
pub(crate) fn checked_attachment_root(
    storage_root: &Path,
    create: bool,
) -> Result<Option<PathBuf>, ChatStorageError> {
    let canonical_storage_root =
        fs::canonicalize(storage_root).map_err(|_| ChatStorageError::unavailable())?;
    let expected_root = canonical_storage_root.join("attachments");
    if create {
        match fs::create_dir(&expected_root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ChatStorageError::unavailable()),
        }
    }
    let metadata = match fs::symlink_metadata(&expected_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_root =
        fs::canonicalize(&expected_root).map_err(|_| ChatStorageError::unavailable())?;
    if canonical_root != expected_root || !canonical_root.starts_with(&canonical_storage_root) {
        return Err(ChatStorageError::unavailable());
    }
    Ok(Some(canonical_root))
}

/// Resolves one identifier-named attachment directory without following path redirections.
///
/// When `create` is true, a missing directory is created beneath the already checked parent.
pub(crate) fn checked_attachment_subdirectory(
    parent: &Path,
    identifier: &str,
    create: bool,
) -> Result<Option<PathBuf>, ChatStorageError> {
    if !is_safe_identifier(identifier) {
        return Err(ChatStorageError::invalid_input());
    }
    let expected_directory = parent.join(identifier);
    if create {
        match fs::create_dir(&expected_directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ChatStorageError::unavailable()),
        }
    }
    let metadata = match fs::symlink_metadata(&expected_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ChatStorageError::unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let canonical_directory =
        fs::canonicalize(&expected_directory).map_err(|_| ChatStorageError::unavailable())?;
    if canonical_directory != expected_directory || !canonical_directory.starts_with(parent) {
        return Err(ChatStorageError::unavailable());
    }
    Ok(Some(canonical_directory))
}

pub(crate) fn cleanup_attachment_directory(path: &std::path::Path) -> Result<(), ChatStorageError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ChatStorageError::unavailable()),
    }
}

pub(crate) fn delete_conversation_attachments(
    conversation_id: &str,
) -> Result<(), ChatStorageError> {
    let storage_root = storage_root()?;
    let Some(attachments_root) = checked_attachment_root(&storage_root, false)? else {
        return Ok(());
    };
    let Some(canonical_directory) =
        checked_attachment_subdirectory(&attachments_root, conversation_id, false)?
    else {
        return Ok(());
    };
    cleanup_attachment_directory(&canonical_directory)
}

#[cfg(test)]
mod attachment_path_tests {
    use std::{fs, path::PathBuf};

    use super::{checked_attachment_root, checked_attachment_subdirectory};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-attachment-paths-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated attachment path directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    fn create_directory_symlink(source: &std::path::Path, destination: &std::path::Path) {
        std::os::windows::fs::symlink_dir(source, destination)
            .expect("create isolated directory symlink");
    }

    #[cfg(unix)]
    fn create_directory_symlink(source: &std::path::Path, destination: &std::path::Path) {
        std::os::unix::fs::symlink(source, destination).expect("create isolated directory symlink");
    }

    #[cfg(any(windows, unix))]
    #[test]
    fn attachment_root_rejects_a_symlink_to_an_external_directory() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        let external_directory = directory.0.join("external");
        fs::create_dir(&storage_root).expect("create storage root");
        fs::create_dir(&external_directory).expect("create external directory");
        fs::write(external_directory.join("keep.txt"), b"preserve")
            .expect("write external sentinel");
        create_directory_symlink(&external_directory, &storage_root.join("attachments"));

        assert!(checked_attachment_root(&storage_root, true).is_err());
        assert_eq!(
            fs::read(external_directory.join("keep.txt")).expect("read external sentinel"),
            b"preserve"
        );
    }

    #[cfg(any(windows, unix))]
    #[test]
    fn attachment_subdirectory_rejects_a_symlink_even_when_it_stays_inside_the_root() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        fs::create_dir(&storage_root).expect("create storage root");
        let attachments_root = checked_attachment_root(&storage_root, true)
            .expect("create attachment root")
            .expect("attachment root exists");
        let legitimate_directory = attachments_root.join("conversation-legitimate");
        fs::create_dir(&legitimate_directory).expect("create legitimate conversation directory");
        fs::write(legitimate_directory.join("keep.txt"), b"preserve")
            .expect("write legitimate sentinel");
        create_directory_symlink(
            &legitimate_directory,
            &attachments_root.join("conversation-symlink"),
        );

        assert!(
            checked_attachment_subdirectory(&attachments_root, "conversation-symlink", false)
                .is_err()
        );
        assert_eq!(
            fs::read(legitimate_directory.join("keep.txt")).expect("read legitimate sentinel"),
            b"preserve"
        );
    }

    #[test]
    fn attachment_root_is_created_only_inside_the_application_storage_root() {
        let directory = TestDirectory::new();
        let storage_root = directory.0.join("storage");
        fs::create_dir(&storage_root).expect("create storage root");

        let attachment_root = checked_attachment_root(&storage_root, true)
            .expect("create checked attachment root")
            .expect("created attachment root");

        assert_eq!(
            attachment_root,
            fs::canonicalize(storage_root.join("attachments"))
                .expect("canonicalize created attachment root")
        );
        assert!(attachment_root.is_dir());
    }
}
