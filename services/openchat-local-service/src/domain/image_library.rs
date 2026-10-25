//! Persists images created from the Design workspace in the protected local profile.
//!
//! Image metadata is indexed in SQLite while image bytes remain in a bounded, identifier-based
//! directory. Reads verify both the stored size and the image signature before returning bytes.

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    chatgpt::{
        ImageBackground, ImageGenerationRequest, ImageGenerationResult, ImageModeration,
        ImageQuality,
    },
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection},
};

const IMAGE_DIRECTORY: &str = "image-library";
const ATTACHMENT_CONTENT_PREFIX: &str = "\u{1e}openchat-attachments-v1:";
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_BATCH_BYTES: usize = 48 * 1024 * 1024;
const MAX_OFFSET: u64 = 1_000_000;
const MAX_SEARCH_CHARS: usize = 512;
const MAX_TAGS: usize = 20;
const MAX_TAG_CHARS: usize = 48;
pub(crate) const MAX_READ_CHUNK_BYTES: u64 = 1024 * 1024;
const MAX_PAGE_SIZE: u64 = 51;

/// Safe metadata for one image saved in the Design library.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageLibraryItem {
    pub(crate) id: String,
    pub(crate) generation_id: String,
    pub(crate) provider_route: String,
    pub(crate) prompt: String,
    pub(crate) model: String,
    pub(crate) size: Option<String>,
    pub(crate) quality: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) moderation: Option<String>,
    pub(crate) output_format: String,
    pub(crate) output_compression: Option<u8>,
    pub(crate) mime_type: String,
    pub(crate) size_bytes: usize,
    pub(crate) created_at: i64,
    pub(crate) is_favorite: bool,
    pub(crate) tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageGenerationRun {
    pub(crate) id: String,
    pub(crate) source_kind: String,
    pub(crate) provider_route: String,
    pub(crate) connection_id: Option<String>,
    pub(crate) workspace_id: Option<String>,
    pub(crate) conversation_id: Option<String>,
    pub(crate) message_id: Option<String>,
    pub(crate) prompt: String,
    pub(crate) model: String,
    pub(crate) size: Option<String>,
    pub(crate) quality: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) moderation: Option<String>,
    pub(crate) output_format: Option<String>,
    pub(crate) output_compression: Option<u8>,
    pub(crate) requested_count: u8,
    pub(crate) image_count: u8,
    pub(crate) status: String,
    pub(crate) error_code: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageCollection {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImagePromptTemplate {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) prompt: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

pub(crate) struct ImageLibraryFilters<'a> {
    pub(crate) query: &'a str,
    pub(crate) sort: &'a str,
    pub(crate) created_after: Option<i64>,
    pub(crate) created_before: Option<i64>,
    pub(crate) model_id: Option<&'a str>,
    pub(crate) provider_route: Option<&'a str>,
    pub(crate) favorites_only: bool,
    pub(crate) tag: Option<&'a str>,
    pub(crate) collection_id: Option<&'a str>,
}

/// Converts unfinished records from an earlier local-service process into a recoverable state.
pub(crate) fn recover_incomplete_generations(storage: &AppStorage) -> Result<(), ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    connection
        .execute(
            "UPDATE image_generation_runs
             SET status = 'failed', error_code = 'operation_interrupted',
                 updated_at_unix_ms = ?1
             WHERE status IN ('preparing', 'generating')",
            [current_time_millis()?],
        )
        .map_err(storage_error)?;
    Ok(())
}

/// Inserts the durable record before an image request starts.
pub(crate) fn begin_generation(
    storage: &AppStorage,
    request: &ImageGenerationRequest,
    source_kind: &str,
    provider_route: &str,
    connection_id: Option<&str>,
    workspace_id: Option<&str>,
    conversation_id: Option<&str>,
    message_id: Option<&str>,
) -> Result<String, ServiceError> {
    if !matches!(source_kind, "design" | "chat") || !matches!(provider_route, "oauth" | "api_key") {
        return Err(invalid_request());
    }
    let now = current_time_millis()?;
    let id = Uuid::new_v4().simple().to_string();
    let connection = storage.connect().map_err(storage_error)?;
    connection
        .execute(
            "INSERT INTO image_generation_runs (
                id, source_kind, provider_route, connection_id, workspace_id,
                conversation_id, message_id, prompt, model_id, size, quality,
                background, moderation, output_format, output_compression,
                requested_count, status, created_at_unix_ms, updated_at_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'generating', ?17, ?17)",
            params![
                id,
                source_kind,
                provider_route,
                connection_id,
                workspace_id,
                conversation_id,
                message_id,
                request.prompt,
                request.model,
                request.size,
                request.quality.map(quality_name),
                request.background.map(background_name),
                request.moderation.map(moderation_name),
                request.output_format.map(|format| format.as_str().to_owned()),
                request.output_compression,
                request.count,
                now,
            ],
        )
        .map_err(storage_error)?;
    Ok(id)
}

/// Marks a generation that did not produce a saved image as failed or cancelled.
pub(crate) fn finish_generation(
    storage: &AppStorage,
    id: &str,
    status: &str,
    error_code: Option<&str>,
) -> Result<(), ServiceError> {
    if !is_safe_id(id) || !matches!(status, "failed" | "cancelled") {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute(
            "UPDATE image_generation_runs
             SET status = ?1, error_code = ?2, updated_at_unix_ms = ?3
             WHERE id = ?4 AND status IN ('preparing', 'generating')",
            params![status, error_code, current_time_millis()?, id],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found());
    }
    Ok(())
}

/// Links successful chat outputs to their conversation attachments without copying their bytes.
pub(crate) fn complete_chat_generation(
    storage: &AppStorage,
    id: &str,
    attachments: &[crate::attachments::AttachmentMetadata],
) -> Result<(), ServiceError> {
    if !is_safe_id(id) || attachments.is_empty() || attachments.len() > 3 {
        return Err(invalid_request());
    }
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    let (conversation_id, message_id, count) = transaction
        .query_row(
            "SELECT conversation_id, message_id, requested_count
             FROM image_generation_runs
             WHERE id = ?1 AND source_kind = 'chat' AND status = 'generating'",
            [id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, u8>(2)?,
                ))
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => not_found(),
            _ => storage_error(error),
        })?;
    let conversation_id = conversation_id.ok_or_else(invalid_request)?;
    let message_id = message_id.ok_or_else(invalid_request)?;
    if attachments.len() > usize::from(count) {
        return Err(invalid_image_data());
    }
    for attachment in attachments {
        transaction
            .execute(
                "INSERT INTO image_generation_outputs (
                    generation_id, conversation_id, message_id, attachment_id
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![id, conversation_id, message_id, attachment.id],
            )
            .map_err(storage_error)?;
    }
    let changed = transaction
        .execute(
            "UPDATE image_generation_runs
             SET status = 'completed', error_code = NULL, image_count = ?1,
                 updated_at_unix_ms = ?2
             WHERE id = ?3 AND status = 'generating'",
            params![
                i64::try_from(attachments.len()).unwrap_or(i64::MAX),
                current_time_millis()?,
                id
            ],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found());
    }
    transaction.commit().map_err(storage_error)
}

/// Reverts a chat generation registration when the message could not accept its attachments.
pub(crate) fn revert_chat_generation(
    storage: &AppStorage,
    id: &str,
    error_code: &str,
) -> Result<(), ServiceError> {
    if !is_safe_id(id) || error_code.is_empty() || error_code.chars().any(char::is_control) {
        return Err(invalid_request());
    }
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM image_generation_outputs WHERE generation_id = ?1",
            [id],
        )
        .map_err(storage_error)?;
    let changed = transaction
        .execute(
            "UPDATE image_generation_runs
             SET status = 'failed', error_code = ?1, image_count = 0,
                 updated_at_unix_ms = ?2
             WHERE id = ?3 AND source_kind = 'chat' AND status = 'completed'",
            params![error_code, current_time_millis()?, id],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found());
    }
    transaction.commit().map_err(storage_error)
}

/// Returns recent incomplete or unsuccessful generation records for the main gallery.
pub(crate) fn list_generation_runs(
    storage: &AppStorage,
    limit: u64,
) -> Result<Vec<ImageGenerationRun>, ServiceError> {
    if !(1..=50).contains(&limit) {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT id, source_kind, provider_route, connection_id, workspace_id,
                    conversation_id, message_id, prompt, model_id, size, quality,
                    background, moderation, output_format, output_compression,
                    requested_count, image_count, status, error_code,
                    created_at_unix_ms, updated_at_unix_ms
             FROM image_generation_runs
             WHERE status IN ('preparing', 'generating', 'failed', 'cancelled')
             ORDER BY created_at_unix_ms DESC, id ASC LIMIT ?1",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([limit as i64], |row| {
            Ok(ImageGenerationRun {
                id: row.get(0)?,
                source_kind: row.get(1)?,
                provider_route: row.get(2)?,
                connection_id: row.get(3)?,
                workspace_id: row.get(4)?,
                conversation_id: row.get(5)?,
                message_id: row.get(6)?,
                prompt: row.get(7)?,
                model: row.get(8)?,
                size: row.get(9)?,
                quality: row.get(10)?,
                background: row.get(11)?,
                moderation: row.get(12)?,
                output_format: row.get(13)?,
                output_compression: row.get(14)?,
                requested_count: row.get(15)?,
                image_count: row.get(16)?,
                status: row.get(17)?,
                error_code: row.get(18)?,
                created_at: row.get(19)?,
                updated_at: row.get(20)?,
            })
        })
        .map_err(storage_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage_error)
}

/// Saves validated provider outputs and their generation settings as one library operation.
pub(crate) fn save_generation(
    storage: &AppStorage,
    generation_id: &str,
    provider_route: &str,
    request: &ImageGenerationRequest,
    result: &ImageGenerationResult,
) -> Result<Vec<ImageLibraryItem>, ServiceError> {
    if !is_safe_id(generation_id)
        || !matches!(provider_route, "oauth" | "api_key")
        || result.outputs.is_empty()
        || result.outputs.len() > 3
    {
        return Err(invalid_image_data());
    }
    let total_bytes = result
        .outputs
        .iter()
        .try_fold(0_usize, |total, output| {
            total.checked_add(output.bytes.len())
        })
        .filter(|total| *total <= MAX_BATCH_BYTES)
        .ok_or_else(invalid_image_data)?;
    if total_bytes == 0
        || result.outputs.iter().any(|output| {
            output.bytes.is_empty()
                || output.bytes.len() > MAX_IMAGE_BYTES
                || signature_mime(&output.bytes) != Some(output.mime_type.as_str())
        })
    {
        return Err(invalid_image_data());
    }
    let output_formats = result
        .outputs
        .iter()
        .map(|output| format_name(&output.mime_type).ok_or_else(invalid_image_data))
        .collect::<Result<Vec<_>, _>>()?;

    let directory = image_directory(storage.root(), true)?;
    let created_at = current_time_millis()?;
    let mut items = Vec::with_capacity(result.outputs.len());
    let mut paths = Vec::with_capacity(result.outputs.len());
    for (output, output_format) in result.outputs.iter().zip(output_formats) {
        let id = Uuid::new_v4().simple().to_string();
        let path = directory.join(format!("{id}.data"));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(_) => {
                cleanup_files(&paths);
                return Err(image_storage_unavailable());
            }
        };
        paths.push(path);
        if file.write_all(&output.bytes).is_err() || file.sync_all().is_err() {
            cleanup_files(&paths);
            return Err(image_storage_unavailable());
        }
        items.push(ImageLibraryItem {
            id,
            generation_id: generation_id.to_owned(),
            provider_route: provider_route.to_owned(),
            prompt: request.prompt.clone(),
            model: request.model.clone(),
            size: result
                .metadata
                .size
                .clone()
                .or_else(|| request.size.clone()),
            quality: result
                .metadata
                .quality
                .map(quality_name)
                .or_else(|| request.quality.map(quality_name)),
            background: result
                .metadata
                .background
                .map(background_name)
                .or_else(|| request.background.map(background_name)),
            moderation: request.moderation.map(moderation_name),
            output_format: output_format.to_owned(),
            output_compression: request
                .output_compression
                .filter(|_| matches!(output_format, "jpeg" | "webp")),
            mime_type: output.mime_type.clone(),
            size_bytes: output.bytes.len(),
            created_at,
            is_favorite: false,
            tags: Vec::new(),
        });
    }

    let insert_result = (|| -> Result<(), ServiceError> {
        let mut connection = storage.connect().map_err(storage_error)?;
        let transaction = connection.transaction().map_err(storage_error)?;
        for item in &items {
            transaction
                .execute(
                    "INSERT INTO generated_images (
                        id, prompt, model_id, size, quality, background, moderation, output_format,
                        output_compression, mime_type, size_bytes, created_at_unix_ms, generation_id
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        item.id,
                        item.prompt,
                        item.model,
                        item.size,
                        item.quality,
                        item.background,
                        item.moderation,
                        item.output_format,
                        item.output_compression,
                        item.mime_type,
                        i64::try_from(item.size_bytes).map_err(|_| invalid_image_data())?,
                        item.created_at,
                        item.generation_id,
                    ],
                )
                .map_err(storage_error)?;
        }
        let changed = transaction
            .execute(
                "UPDATE image_generation_runs
                 SET status = 'completed', error_code = NULL, image_count = ?1,
                     updated_at_unix_ms = ?2
                 WHERE id = ?3 AND source_kind = 'design' AND status = 'generating'",
                params![
                    i64::try_from(items.len()).unwrap_or(i64::MAX),
                    current_time_millis()?,
                    generation_id
                ],
            )
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(not_found());
        }
        transaction.commit().map_err(storage_error)
    })();
    if let Err(error) = insert_result {
        cleanup_files(&paths);
        return Err(error);
    }
    Ok(items)
}

/// Returns a bounded, newest-first page of Design images.
pub(crate) fn list(
    storage: &AppStorage,
    offset: u64,
    limit: u64,
    filters: &ImageLibraryFilters<'_>,
) -> Result<Vec<ImageLibraryItem>, ServiceError> {
    if offset > MAX_OFFSET || !(1..=MAX_PAGE_SIZE).contains(&limit) {
        return Err(invalid_request());
    }
    let query = filters.query.trim();
    if query.chars().count() > MAX_SEARCH_CHARS || query.chars().any(char::is_control) {
        return Err(invalid_request());
    }
    if !matches!(filters.sort, "newest" | "oldest")
        || filters
            .created_after
            .zip(filters.created_before)
            .is_some_and(|(after, before)| after > before)
        || filters
            .provider_route
            .is_some_and(|route| !matches!(route, "oauth" | "api_key"))
        || filters.model_id.is_some_and(|model| {
            model.len() > 128
                || !model
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._:-/".contains(&byte))
        })
        || filters.collection_id.is_some_and(|id| !is_safe_id(id))
        || filters.tag.is_some_and(|tag| {
            tag.trim().is_empty()
                || tag.chars().count() > MAX_TAG_CHARS
                || tag.chars().any(char::is_control)
        })
    {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT image.id, image.prompt, image.model_id, image.size, image.quality,
                    image.background, image.moderation, image.output_format,
                    image.output_compression, image.mime_type, image.size_bytes,
                    image.created_at_unix_ms, image.generation_id,
                    COALESCE(run.provider_route, 'unknown'),
                    COALESCE(annotation.is_favorite, 0),
                    COALESCE(annotation.tags_json, '[]')
             FROM generated_images image
             LEFT JOIN image_generation_runs run ON run.id = image.generation_id
             LEFT JOIN image_library_annotations annotation
               ON annotation.source_kind = 'design' AND annotation.source_key = image.id
             WHERE (?1 = '' OR instr(lower(image.prompt), lower(?1)) > 0
                    OR instr(lower(image.model_id), lower(?1)) > 0)
               AND (?2 IS NULL OR image.model_id = ?2)
               AND (?3 IS NULL OR image.created_at_unix_ms >= ?3)
               AND (?4 IS NULL OR image.created_at_unix_ms <= ?4)
               AND (?5 IS NULL OR COALESCE(run.provider_route, 'unknown') = ?5)
               AND (?6 = 0 OR COALESCE(annotation.is_favorite, 0) = 1)
               AND (?7 IS NULL OR EXISTS (
                    SELECT 1 FROM json_each(COALESCE(annotation.tags_json, '[]')) tag
                    WHERE lower(tag.value) = lower(?7)
               ))
               AND (?8 IS NULL OR EXISTS (
                    SELECT 1 FROM image_collection_items item
                    WHERE item.collection_id = ?8 AND item.source_kind = 'design'
                      AND item.source_key = image.id
               ))
             ORDER BY CASE WHEN ?9 = 'oldest' THEN image.created_at_unix_ms END ASC,
                      CASE WHEN ?9 = 'newest' THEN image.created_at_unix_ms END DESC,
                      image.id ASC LIMIT ?10 OFFSET ?11",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map(
            params![
                query,
                filters.model_id,
                filters.created_after,
                filters.created_before,
                filters.provider_route,
                filters.favorites_only,
                filters.tag,
                filters.collection_id,
                filters.sort,
                limit as i64,
                offset as i64,
            ],
            |row| {
                let size_bytes = row.get::<_, i64>(10)?;
                let size_bytes = usize::try_from(size_bytes)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(10, size_bytes))?;
                let tags_json = row.get::<_, String>(15)?;
                let tags = serde_json::from_str::<Vec<String>>(&tags_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        15,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(ImageLibraryItem {
                    id: row.get(0)?,
                    generation_id: row.get(12)?,
                    provider_route: row.get(13)?,
                    prompt: row.get(1)?,
                    model: row.get(2)?,
                    size: row.get(3)?,
                    quality: row.get(4)?,
                    background: row.get(5)?,
                    moderation: row.get(6)?,
                    output_format: row.get(7)?,
                    output_compression: row.get(8)?,
                    mime_type: row.get(9)?,
                    size_bytes,
                    created_at: row.get(11)?,
                    is_favorite: row.get::<_, i64>(14)? != 0,
                    tags,
                })
            },
        )
        .map_err(storage_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage_error)
}

/// Reads a bounded Design image chunk after checking its identifier, path, length, and signature.
pub(crate) fn read_chunk(
    storage: &AppStorage,
    id: &str,
    offset: u64,
    length: u64,
) -> Result<(String, u64, Vec<u8>), ServiceError> {
    if !is_safe_id(id) || length == 0 || length > MAX_READ_CHUNK_BYTES {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_error)?;
    let (mime_type, size_bytes) = connection
        .query_row(
            "SELECT mime_type, size_bytes FROM generated_images WHERE id = ?1",
            [id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => not_found(),
            _ => storage_error(error),
        })?;
    let total_bytes = u64::try_from(size_bytes).map_err(|_| image_storage_unavailable())?;
    if total_bytes == 0 || total_bytes > MAX_IMAGE_BYTES as u64 || offset > total_bytes {
        return Err(image_storage_unavailable());
    }
    let path = image_file_path(storage, id, total_bytes)?;
    let mut file = File::open(path).map_err(|_| image_storage_unavailable())?;
    let mut header = [0_u8; 12];
    let header_len = file
        .read(&mut header)
        .map_err(|_| image_storage_unavailable())?;
    if signature_mime(&header[..header_len]) != Some(mime_type.as_str()) {
        return Err(image_storage_unavailable());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| image_storage_unavailable())?;
    let chunk_len = usize::try_from(length.min(total_bytes - offset))
        .map_err(|_| image_storage_unavailable())?;
    let mut bytes = vec![0; chunk_len];
    file.read_exact(&mut bytes)
        .map_err(|_| image_storage_unavailable())?;
    Ok((mime_type, total_bytes, bytes))
}

pub(crate) fn set_annotations(
    storage: &AppStorage,
    source_kind: &str,
    source_key: &str,
    is_favorite: bool,
    tags: &[String],
) -> Result<(), ServiceError> {
    validate_source_key(source_kind, source_key)?;
    let normalized_tags = normalize_tags(tags)?;
    let tags_json = serde_json::to_string(&normalized_tags).map_err(|_| invalid_request())?;
    let connection = storage.connect().map_err(storage_error)?;
    ensure_source_exists(&connection, source_kind, source_key)?;
    connection
        .execute(
            "INSERT INTO image_library_annotations (source_kind, source_key, is_favorite, tags_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_kind, source_key) DO UPDATE SET
                is_favorite = excluded.is_favorite, tags_json = excluded.tags_json",
            params![source_kind, source_key, is_favorite, tags_json],
        )
        .map_err(storage_error)?;
    Ok(())
}

pub(crate) fn remove_from_library(
    storage: &AppStorage,
    source_kind: &str,
    source_key: &str,
) -> Result<(), ServiceError> {
    validate_source_key(source_kind, source_key)?;
    if source_kind == "chat" {
        let (conversation_id, message_id, attachment_id) = parse_chat_source_key(source_key)?;
        let connection = storage.connect().map_err(storage_error)?;
        ensure_source_exists(&connection, source_kind, source_key)?;
        connection
            .execute(
                "INSERT OR IGNORE INTO image_library_hidden_chat_images
                 (conversation_id, message_id, attachment_id) VALUES (?1, ?2, ?3)",
                params![conversation_id, message_id, attachment_id],
            )
            .map_err(storage_error)?;
        connection
            .execute(
                "DELETE FROM image_library_annotations WHERE source_kind = 'chat' AND source_key = ?1",
                [source_key],
            )
            .map_err(storage_error)?;
        connection
            .execute(
                "DELETE FROM image_collection_items WHERE source_kind = 'chat' AND source_key = ?1",
                [source_key],
            )
            .map_err(storage_error)?;
        return Ok(());
    }

    let connection = storage.connect().map_err(storage_error)?;
    let size_bytes = connection
        .query_row(
            "SELECT size_bytes FROM generated_images WHERE id = ?1",
            [source_key],
            |row| row.get::<_, i64>(0),
        )
        .map_err(map_not_found)?;
    drop(connection);
    let path = image_file_path(
        storage,
        source_key,
        u64::try_from(size_bytes).map_err(|_| image_storage_unavailable())?,
    )?;
    let tombstone = path.with_file_name(format!(
        ".deleted-{}-{}.data",
        source_key,
        Uuid::new_v4().simple()
    ));
    fs::rename(&path, &tombstone).map_err(|_| image_storage_unavailable())?;
    let deletion = (|| -> Result<(), ServiceError> {
        let mut connection = storage.connect().map_err(storage_error)?;
        let transaction = connection.transaction().map_err(storage_error)?;
        let changed = transaction
            .execute("DELETE FROM generated_images WHERE id = ?1", [source_key])
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(not_found());
        }
        transaction
            .execute(
                "DELETE FROM image_library_annotations WHERE source_kind = 'design' AND source_key = ?1",
                [source_key],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM image_collection_items WHERE source_kind = 'design' AND source_key = ?1",
                [source_key],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    })();
    if let Err(error) = deletion {
        if fs::rename(&tombstone, &path).is_err() {
            return Err(image_storage_unavailable());
        }
        return Err(error);
    }
    fs::remove_file(tombstone).map_err(|_| image_storage_unavailable())
}

pub(crate) fn clear_library(storage: &AppStorage) -> Result<u64, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let ids = {
        let mut statement = connection
            .prepare("SELECT id, size_bytes FROM generated_images ORDER BY id ASC")
            .map_err(storage_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(storage_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)?
    };
    drop(connection);
    let visible_chat_count = count_visible_chat_images(storage)?;
    let mut tombstones = Vec::with_capacity(ids.len());
    for (id, size) in &ids {
        let size = u64::try_from(*size).map_err(|_| image_storage_unavailable())?;
        let path = image_file_path(storage, id, size)?;
        let tombstone =
            path.with_file_name(format!(".deleted-{}-{}.data", id, Uuid::new_v4().simple()));
        if fs::rename(&path, &tombstone).is_err() {
            restore_renamed_files(&tombstones)?;
            return Err(image_storage_unavailable());
        }
        tombstones.push((path, tombstone));
    }
    let deletion = (|| -> Result<(), ServiceError> {
        let mut connection = storage.connect().map_err(storage_error)?;
        let transaction = connection.transaction().map_err(storage_error)?;
        transaction
            .execute("DELETE FROM generated_images", [])
            .map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM image_library_annotations WHERE source_kind = 'design'",
                [],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM image_collection_items WHERE source_kind = 'design'",
                [],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO image_library_hidden_chat_images
             (conversation_id, message_id, attachment_id)
             SELECT message.conversation_id, message.id, json_extract(attachment.value, '$.id')
             FROM messages message
             CROSS JOIN json_each(
                CASE WHEN json_valid(substr(message.content, length(?1) + 1))
                     THEN json_extract(substr(message.content, length(?1) + 1), '$.attachments')
                     ELSE '[]' END
             ) AS attachment
             WHERE substr(message.content, 1, length(?1)) = ?1
               AND message.role = 'assistant'
               AND json_extract(attachment.value, '$.kind') = 'image'
               AND json_extract(attachment.value, '$.name') IN
                   ('generated-image.png', 'generated-image.jpeg', 'generated-image.webp')",
                [ATTACHMENT_CONTENT_PREFIX],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM image_library_annotations WHERE source_kind = 'chat'",
                [],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM image_collection_items WHERE source_kind = 'chat'",
                [],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    })();
    if let Err(error) = deletion {
        restore_renamed_files(&tombstones)?;
        return Err(error);
    }
    for (_, tombstone) in &tombstones {
        fs::remove_file(tombstone).map_err(|_| image_storage_unavailable())?;
    }
    Ok((ids.len() + visible_chat_count) as u64)
}

pub(crate) fn list_collections(storage: &AppStorage) -> Result<Vec<ImageCollection>, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare("SELECT id, name, created_at_unix_ms, updated_at_unix_ms FROM image_collections ORDER BY name COLLATE NOCASE ASC, id ASC")
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(ImageCollection {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })
        .map_err(storage_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage_error)
}

pub(crate) fn save_collection(
    storage: &AppStorage,
    id: Option<&str>,
    name: &str,
) -> Result<ImageCollection, ServiceError> {
    validate_name(name, 80)?;
    if id.is_some_and(|id| !is_safe_id(id)) {
        return Err(invalid_request());
    }
    let now = current_time_millis()?;
    let connection = storage.connect().map_err(storage_error)?;
    let collection_id =
        match id {
            Some(id) => {
                if !is_safe_id(id) {
                    return Err(invalid_request());
                }
                let changed = connection.execute(
                "UPDATE image_collections SET name = ?1, updated_at_unix_ms = ?2 WHERE id = ?3",
                params![name.trim(), now, id],
            ).map_err(collection_save_error)?;
                if changed != 1 {
                    return Err(not_found());
                }
                id.to_owned()
            }
            None => {
                let new_id = Uuid::new_v4().simple().to_string();
                connection.execute(
                "INSERT INTO image_collections (id, name, created_at_unix_ms, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?3)",
                params![new_id, name.trim(), now],
            ).map_err(collection_save_error)?;
                new_id
            }
        };
    connection
        .query_row(
            "SELECT id, name, created_at_unix_ms, updated_at_unix_ms FROM image_collections WHERE id = ?1",
            [&collection_id],
            |row| Ok(ImageCollection { id: row.get(0)?, name: row.get(1)?, created_at: row.get(2)?, updated_at: row.get(3)? }),
        )
        .map_err(storage_error)
}

pub(crate) fn delete_collection(storage: &AppStorage, id: &str) -> Result<(), ServiceError> {
    if !is_safe_id(id) {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute("DELETE FROM image_collections WHERE id = ?1", [id])
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found());
    }
    Ok(())
}

pub(crate) fn set_collection_item(
    storage: &AppStorage,
    collection_id: &str,
    source_kind: &str,
    source_key: &str,
    included: bool,
) -> Result<(), ServiceError> {
    if !is_safe_id(collection_id) {
        return Err(invalid_request());
    }
    validate_source_key(source_kind, source_key)?;
    let connection = storage.connect().map_err(storage_error)?;
    let collection_exists = connection
        .query_row(
            "SELECT 1 FROM image_collections WHERE id = ?1",
            [collection_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage_error)?
        .is_some();
    if !collection_exists {
        return Err(not_found());
    }
    ensure_source_exists(&connection, source_kind, source_key)?;
    if included {
        connection.execute(
            "INSERT OR IGNORE INTO image_collection_items (collection_id, source_kind, source_key)
             VALUES (?1, ?2, ?3)",
            params![collection_id, source_kind, source_key],
        ).map_err(storage_error)?;
    } else {
        connection.execute(
            "DELETE FROM image_collection_items WHERE collection_id = ?1 AND source_kind = ?2 AND source_key = ?3",
            params![collection_id, source_kind, source_key],
        ).map_err(storage_error)?;
    }
    Ok(())
}

pub(crate) fn list_prompt_templates(
    storage: &AppStorage,
) -> Result<Vec<ImagePromptTemplate>, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, prompt, created_at_unix_ms, updated_at_unix_ms
         FROM image_prompt_templates ORDER BY updated_at_unix_ms DESC, id ASC",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(ImagePromptTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                prompt: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(storage_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage_error)
}

pub(crate) fn save_prompt_template(
    storage: &AppStorage,
    id: Option<&str>,
    name: &str,
    prompt: &str,
) -> Result<ImagePromptTemplate, ServiceError> {
    validate_name(name, 80)?;
    if prompt.trim().is_empty() || prompt.len() > 10_000 || prompt.chars().any(char::is_control) {
        return Err(invalid_request());
    }
    if id.is_some_and(|id| !is_safe_id(id)) {
        return Err(invalid_request());
    }
    let now = current_time_millis()?;
    let connection = storage.connect().map_err(storage_error)?;
    let template_id = match id {
        Some(id) => {
            if !is_safe_id(id) {
                return Err(invalid_request());
            }
            let changed = connection.execute(
                "UPDATE image_prompt_templates SET name = ?1, prompt = ?2, updated_at_unix_ms = ?3 WHERE id = ?4",
                params![name.trim(), prompt.trim(), now, id],
            ).map_err(template_save_error)?;
            if changed != 1 {
                return Err(not_found());
            }
            id.to_owned()
        }
        None => {
            let new_id = Uuid::new_v4().simple().to_string();
            connection.execute(
                "INSERT INTO image_prompt_templates (id, name, prompt, created_at_unix_ms, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![new_id, name.trim(), prompt.trim(), now],
            ).map_err(template_save_error)?;
            new_id
        }
    };
    connection.query_row(
        "SELECT id, name, prompt, created_at_unix_ms, updated_at_unix_ms FROM image_prompt_templates WHERE id = ?1",
        [&template_id],
        |row| Ok(ImagePromptTemplate { id: row.get(0)?, name: row.get(1)?, prompt: row.get(2)?, created_at: row.get(3)?, updated_at: row.get(4)? }),
    ).map_err(storage_error)
}

pub(crate) fn delete_prompt_template(storage: &AppStorage, id: &str) -> Result<(), ServiceError> {
    if !is_safe_id(id) {
        return Err(invalid_request());
    }
    let connection = storage.connect().map_err(storage_error)?;
    let changed = connection
        .execute("DELETE FROM image_prompt_templates WHERE id = ?1", [id])
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(not_found());
    }
    Ok(())
}

fn validate_source_key(source_kind: &str, source_key: &str) -> Result<(), ServiceError> {
    match source_kind {
        "design" if is_safe_id(source_key) => Ok(()),
        "chat" => parse_chat_source_key(source_key).map(|_| ()),
        _ => Err(invalid_request()),
    }
}

fn parse_chat_source_key(source_key: &str) -> Result<(&str, &str, &str), ServiceError> {
    let mut parts = source_key.split(':');
    let conversation_id = parts.next().ok_or_else(invalid_request)?;
    let message_id = parts.next().ok_or_else(invalid_request)?;
    let attachment_id = parts.next().ok_or_else(invalid_request)?;
    if parts.next().is_some()
        || !is_safe_id(conversation_id)
        || !is_safe_id(message_id)
        || !is_safe_id(attachment_id)
    {
        return Err(invalid_request());
    }
    Ok((conversation_id, message_id, attachment_id))
}

fn ensure_source_exists(
    connection: &StorageConnection<'_>,
    source_kind: &str,
    source_key: &str,
) -> Result<(), ServiceError> {
    let exists = match source_kind {
        "design" => connection.query_row(
            "SELECT 1 FROM generated_images WHERE id = ?1",
            [source_key],
            |row| row.get::<_, i64>(0),
        ),
        "chat" => {
            let (conversation_id, message_id, attachment_id) = parse_chat_source_key(source_key)?;
            connection.query_row(
                "SELECT 1 FROM messages message
                 CROSS JOIN json_each(
                    CASE WHEN json_valid(substr(message.content, length(?1) + 1))
                         THEN json_extract(substr(message.content, length(?1) + 1), '$.attachments')
                         ELSE '[]' END
                 ) AS attachment
                 WHERE message.id = ?2 AND message.conversation_id = ?3
                   AND substr(message.content, 1, length(?1)) = ?1
                   AND message.role = 'assistant'
                   AND json_extract(attachment.value, '$.id') = ?4
                   AND json_extract(attachment.value, '$.kind') = 'image'
                   AND json_extract(attachment.value, '$.name') IN
                       ('generated-image.png', 'generated-image.jpeg', 'generated-image.webp')
                   AND NOT EXISTS (
                       SELECT 1 FROM image_library_hidden_chat_images hidden
                       WHERE hidden.conversation_id = ?3 AND hidden.message_id = ?2
                         AND hidden.attachment_id = ?4
                   )",
                params![
                    ATTACHMENT_CONTENT_PREFIX,
                    message_id,
                    conversation_id,
                    attachment_id
                ],
                |row| row.get::<_, i64>(0),
            )
        }
        _ => return Err(invalid_request()),
    }
    .optional()
    .map_err(storage_error)?;
    if exists.is_some() {
        Ok(())
    } else {
        Err(not_found())
    }
}

fn normalize_tags(tags: &[String]) -> Result<Vec<String>, ServiceError> {
    if tags.len() > MAX_TAGS {
        return Err(invalid_request());
    }
    let mut normalized = HashSet::with_capacity(tags.len());
    for tag in tags {
        let tag = tag.trim().to_lowercase();
        if tag.is_empty()
            || tag.chars().count() > MAX_TAG_CHARS
            || tag.chars().any(char::is_control)
        {
            return Err(invalid_request());
        }
        normalized.insert(tag);
    }
    let mut result = normalized.into_iter().collect::<Vec<_>>();
    result.sort();
    Ok(result)
}

fn validate_name(value: &str, max_chars: usize) -> Result<(), ServiceError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars || value.chars().any(char::is_control)
    {
        return Err(invalid_request());
    }
    Ok(())
}

fn restore_renamed_files(files: &[(PathBuf, PathBuf)]) -> Result<(), ServiceError> {
    for (original, tombstone) in files.iter().rev() {
        if fs::rename(tombstone, original).is_err() {
            return Err(image_storage_unavailable());
        }
    }
    Ok(())
}

fn count_visible_chat_images(storage: &AppStorage) -> Result<usize, ServiceError> {
    let connection = storage.connect().map_err(storage_error)?;
    let count = connection
        .query_row(
            "SELECT COUNT(*) FROM messages message
         JOIN conversations conversation ON conversation.id = message.conversation_id
         CROSS JOIN json_each(
            CASE WHEN json_valid(substr(message.content, length(?1) + 1))
                 THEN json_extract(substr(message.content, length(?1) + 1), '$.attachments')
                 ELSE '[]' END
         ) AS attachment
         WHERE substr(message.content, 1, length(?1)) = ?1
           AND message.role = 'assistant'
           AND json_extract(attachment.value, '$.kind') = 'image'
           AND json_extract(attachment.value, '$.name') IN
               ('generated-image.png', 'generated-image.jpeg', 'generated-image.webp')
           AND NOT EXISTS (
               SELECT 1 FROM image_library_hidden_chat_images hidden
               WHERE hidden.conversation_id = conversation.id AND hidden.message_id = message.id
                 AND hidden.attachment_id = json_extract(attachment.value, '$.id')
           )",
            [ATTACHMENT_CONTENT_PREFIX],
            |row| row.get::<_, i64>(0),
        )
        .map_err(storage_error)?;
    usize::try_from(count).map_err(|_| image_storage_unavailable())
}

fn map_not_found(error: rusqlite::Error) -> ServiceError {
    match error {
        rusqlite::Error::QueryReturnedNoRows => not_found(),
        other => storage_error(other),
    }
}

fn collection_save_error(error: rusqlite::Error) -> ServiceError {
    match &error {
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            ServiceError::new(
                "image_collection_name_exists",
                "An image collection with this name already exists.",
                false,
            )
        }
        _ => storage_error(error),
    }
}

fn template_save_error(error: rusqlite::Error) -> ServiceError {
    storage_error(error)
}

fn image_file_path(
    storage: &AppStorage,
    id: &str,
    size_bytes: u64,
) -> Result<PathBuf, ServiceError> {
    let directory = image_directory(storage.root(), false)?;
    let path = directory.join(format!("{id}.data"));
    let metadata = fs::symlink_metadata(&path).map_err(|_| image_storage_unavailable())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() != size_bytes {
        return Err(image_storage_unavailable());
    }
    let canonical = fs::canonicalize(&path).map_err(|_| image_storage_unavailable())?;
    if canonical != path || !canonical.starts_with(&directory) {
        return Err(image_storage_unavailable());
    }
    Ok(canonical)
}

fn image_directory(root: &Path, create: bool) -> Result<PathBuf, ServiceError> {
    let root = fs::canonicalize(root).map_err(|_| image_storage_unavailable())?;
    let path = root.join(IMAGE_DIRECTORY);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(image_storage_unavailable());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
            match fs::create_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let metadata =
                        fs::symlink_metadata(&path).map_err(|_| image_storage_unavailable())?;
                    if metadata.file_type().is_symlink() || !metadata.is_dir() {
                        return Err(image_storage_unavailable());
                    }
                }
                Err(_) => return Err(image_storage_unavailable()),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(image_storage_unavailable());
        }
        Err(_) => return Err(image_storage_unavailable()),
    }
    let canonical = fs::canonicalize(&path).map_err(|_| image_storage_unavailable())?;
    if canonical != path || !canonical.starts_with(&root) {
        return Err(image_storage_unavailable());
    }
    Ok(canonical)
}

fn cleanup_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn is_safe_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn format_name(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpeg"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

fn signature_mime(bytes: &[u8]) -> Option<&'static str> {
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

fn quality_name(value: ImageQuality) -> String {
    match value {
        ImageQuality::Low => "low",
        ImageQuality::Medium => "medium",
        ImageQuality::High => "high",
        ImageQuality::XHigh => "xhigh",
        ImageQuality::Max => "max",
        ImageQuality::Auto => "auto",
    }
    .to_owned()
}

fn background_name(value: ImageBackground) -> String {
    match value {
        ImageBackground::Transparent => "transparent",
        ImageBackground::Opaque => "opaque",
        ImageBackground::Auto => "auto",
    }
    .to_owned()
}

fn moderation_name(value: ImageModeration) -> String {
    match value {
        ImageModeration::Auto => "auto",
        ImageModeration::Low => "low",
    }
    .to_owned()
}

fn current_time_millis() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| image_storage_unavailable())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| image_storage_unavailable())
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The image library request is invalid.",
        false,
    )
}

fn invalid_image_data() -> ServiceError {
    ServiceError::new(
        "image_output_invalid",
        "The generated image could not be saved to the Design library.",
        false,
    )
}

fn not_found() -> ServiceError {
    ServiceError::new(
        "image_not_found",
        "The requested image is unavailable.",
        false,
    )
}

fn storage_error(error: rusqlite::Error) -> ServiceError {
    ServiceError::new(
        "image_library_storage_failed",
        "The Design image library could not be read or updated.",
        true,
    )
    .with_details(crate::storage::sqlite_error_code(&error))
}

fn image_storage_unavailable() -> ServiceError {
    ServiceError::new(
        "image_library_storage_failed",
        "The Design image could not be saved or read from local storage.",
        true,
    )
}
