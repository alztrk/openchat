//! Attachment storage, library paging, and path validation helpers.

use super::*;

pub(super) fn list_conversation_attachments(
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    let input = input_object(params)?;
    let offset = input
        .get("offset")
        .map(|value| {
            value
                .as_u64()
                .filter(|offset| *offset <= MAX_ATTACHMENT_OFFSET)
                .ok_or_else(invalid_request_params)
        })
        .transpose()?
        .unwrap_or(0);
    let limit = input
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .and_then(|limit| usize::try_from(limit).ok())
                .filter(|limit| (1..=MAX_ATTACHMENT_PAGE).contains(limit))
                .ok_or_else(invalid_request_params)
        })
        .transpose()?
        .unwrap_or(50);
    let query = match input.get("query") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.chars().count() > MAX_ATTACHMENT_QUERY_CHARS
                || value.chars().any(char::is_control)
            {
                return Err(invalid_request_params());
            }
            value.to_lowercase()
        }
        _ => return Err(invalid_request_params()),
    };
    let availability = match input.get("availability") {
        None | Some(Value::Null) => "all",
        Some(Value::String(value)) => value.as_str(),
        _ => return Err(invalid_request_params()),
    };
    if !matches!(availability, "all" | "available" | "missing") {
        return Err(invalid_request_params());
    }

    let generated_only = optional_bool(input, "generatedImagesOnly", false)?;
    let sort = match input.get("sort") {
        None | Some(Value::Null) => "newest",
        Some(Value::String(value)) if matches!(value.as_str(), "newest" | "oldest") => {
            value.as_str()
        }
        _ => return Err(invalid_request_params()),
    };
    let created_after = optional_timestamp(input, "createdAfter")?;
    let created_before = optional_timestamp(input, "createdBefore")?;
    if created_after
        .zip(created_before)
        .is_some_and(|(after, before)| after > before)
    {
        return Err(invalid_request_params());
    }
    let model_id = optional_filter_string(input, "modelId", 128)?;
    if model_id.as_deref().is_some_and(|value| {
        !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-/".contains(&byte))
    }) {
        return Err(invalid_request_params());
    }
    let provider_route = match optional_filter_string(input, "providerRoute", 16)?.as_deref() {
        None => None,
        Some("oauth") => Some("oauth".to_owned()),
        Some("api_key") => Some("api_key".to_owned()),
        Some(_) => return Err(invalid_request_params()),
    };
    let favorites_only = optional_bool(input, "favoritesOnly", false)?;
    let tag = optional_filter_string(input, "tag", 48)?.map(|tag| tag.to_lowercase());
    let collection_id = optional_filter_string(input, "collectionId", 32)?;
    if collection_id
        .as_deref()
        .is_some_and(|value| !is_safe_attachment_id(value))
    {
        return Err(invalid_request_params());
    }
    let filters = AttachmentLibraryFilters {
        generated_only,
        sort,
        created_after,
        created_before,
        model_id: model_id.as_deref(),
        provider_route: provider_route.as_deref(),
        favorites_only,
        tag: tag.as_deref(),
        collection_id: collection_id.as_deref(),
    };

    let connection = storage.connect().map_err(storage_error)?;
    let index_enabled = attachment_text_index_enabled(&connection)?;
    drop(connection);
    let mut cursor = offset;
    let mut scanned = 0_u64;
    let mut items = Vec::new();
    let mut found_extra_at = None;
    let mut reached_end = false;

    while items.len() <= limit && scanned < MAX_ATTACHMENT_SCAN as u64 {
        let batch_limit =
            ATTACHMENT_LIBRARY_BATCH_SIZE.min((MAX_ATTACHMENT_SCAN as u64).saturating_sub(scanned));
        let connection = storage.connect().map_err(storage_error)?;
        let batch = query_attachment_library_batch(
            &connection,
            cursor,
            batch_limit,
            index_enabled,
            &filters,
        )?;
        drop(connection);
        if batch.is_empty() {
            reached_end = true;
            break;
        }

        for (position, candidate) in batch.iter().enumerate() {
            let candidate_offset = cursor.saturating_add(position as u64);
            let Some(summary) = candidate.summary.as_ref() else {
                continue;
            };
            let available = attachment_is_available(
                storage.root(),
                &summary.conversation_id,
                &summary.message_id,
                &summary.attachment_id,
                summary.size_bytes,
            )?;
            if (availability == "available" && !available)
                || (availability == "missing" && available)
            {
                continue;
            }
            if !query.is_empty()
                && !summary.file_name.to_lowercase().contains(&query)
                && !summary.conversation_title.to_lowercase().contains(&query)
                && !candidate.indexed_content.to_lowercase().contains(&query)
                && !summary
                    .generation_prompt
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&query))
                && !summary
                    .generation_model
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&query))
            {
                continue;
            }
            if items.len() == limit {
                found_extra_at = Some(candidate_offset);
                break;
            }
            items.push(json!({
                "conversationId": summary.conversation_id,
                "conversationTitle": summary.conversation_title,
                "projectId": summary.project_id,
                "messageId": summary.message_id,
                "attachmentId": summary.attachment_id,
                "fileName": summary.file_name,
                "mimeType": summary.mime_type,
                "sizeBytes": summary.size_bytes,
                "createdAt": summary.created_at,
                "isAvailable": available,
                "generationId": summary.generation_id,
                "prompt": summary.generation_prompt,
                "model": summary.generation_model,
                "providerRoute": summary.generation_provider_route,
                "isFavorite": summary.is_favorite,
                "tags": summary.tags,
            }));
        }
        if found_extra_at.is_some() {
            break;
        }
        cursor = cursor.saturating_add(batch.len() as u64);
        scanned = scanned.saturating_add(batch.len() as u64);
        if batch.len() < batch_limit as usize {
            reached_end = true;
            break;
        }
    }

    let next_offset = found_extra_at.or({
        if reached_end {
            None
        } else if scanned >= MAX_ATTACHMENT_SCAN as u64 {
            Some(cursor)
        } else {
            None
        }
    });
    Ok(json!({"items": items, "nextOffset": next_offset}))
}

pub(super) struct AttachmentLibraryCandidate {
    summary: Option<AttachmentLibrarySummary>,
    indexed_content: String,
}

struct AttachmentLibrarySummary {
    conversation_id: String,
    conversation_title: String,
    project_id: Option<String>,
    message_id: String,
    attachment_id: String,
    file_name: String,
    mime_type: String,
    size_bytes: usize,
    created_at: i64,
    generation_id: Option<String>,
    generation_prompt: Option<String>,
    generation_model: Option<String>,
    generation_provider_route: Option<String>,
    is_favorite: bool,
    tags: Vec<String>,
}

pub(super) struct AttachmentLibraryFilters<'a> {
    generated_only: bool,
    sort: &'a str,
    created_after: Option<i64>,
    created_before: Option<i64>,
    model_id: Option<&'a str>,
    provider_route: Option<&'a str>,
    favorites_only: bool,
    tag: Option<&'a str>,
    collection_id: Option<&'a str>,
}

pub(super) fn query_attachment_library_batch(
    connection: &StorageConnection<'_>,
    offset: u64,
    limit: u64,
    index_enabled: bool,
    filters: &AttachmentLibraryFilters<'_>,
) -> Result<Vec<AttachmentLibraryCandidate>, ServiceError> {
    let mut statement = connection
        .prepare(
            "SELECT c.id, c.title, c.project_id, m.id, COALESCE(m.created_at, c.updated_at),
                    json_extract(attachment.value, '$.id'),
                    json_extract(attachment.value, '$.name'),
                    json_extract(attachment.value, '$.mimeType'),
                    json_extract(attachment.value, '$.sizeBytes'),
                    json_extract(attachment.value, '$.kind'),
                    generation.id, generation.prompt, generation.model_id, generation.provider_route,
                    COALESCE(annotation.is_favorite, 0), COALESCE(annotation.tags_json, '[]'),
                    CASE WHEN ?2 = 1 THEN COALESCE((
                        SELECT indexed.content FROM attachment_text_index indexed
                        WHERE indexed.conversation_id = m.conversation_id
                          AND indexed.message_id = m.id
                          AND indexed.attachment_id = json_extract(attachment.value, '$.id')
                    ), '') ELSE '' END
             FROM messages m
             JOIN conversations c ON c.id = m.conversation_id
             CROSS JOIN json_each(
                CASE WHEN json_valid(substr(m.content, length(?1) + 1))
                     THEN json_extract(substr(m.content, length(?1) + 1), '$.attachments')
                     ELSE '[]' END
             ) AS attachment
             LEFT JOIN image_generation_outputs generated_output
               ON generated_output.conversation_id = c.id
              AND generated_output.message_id = m.id
              AND generated_output.attachment_id = json_extract(attachment.value, '$.id')
             LEFT JOIN image_generation_runs generation
               ON generation.id = generated_output.generation_id
             LEFT JOIN image_library_annotations annotation
               ON annotation.source_kind = 'chat'
              AND annotation.source_key = c.id || ':' || m.id || ':' || json_extract(attachment.value, '$.id')
             WHERE substr(m.content, 1, length(?1)) = ?1
               AND (?5 = 0 OR (m.role = 'assistant' AND lower(json_extract(attachment.value, '$.name')) IN
                    ('generated-image.png', 'generated-image.jpeg', 'generated-image.webp')))
               AND (?5 = 0 OR NOT EXISTS (
                    SELECT 1 FROM image_library_hidden_chat_images hidden
                    WHERE hidden.conversation_id = c.id AND hidden.message_id = m.id
                      AND hidden.attachment_id = json_extract(attachment.value, '$.id')
               ))
               AND (?7 IS NULL OR COALESCE(m.created_at, c.updated_at) >= ?7)
               AND (?8 IS NULL OR COALESCE(m.created_at, c.updated_at) <= ?8)
               AND (?9 IS NULL OR generation.model_id = ?9)
               AND (?10 IS NULL OR generation.provider_route = ?10)
               AND (?11 = 0 OR COALESCE(annotation.is_favorite, 0) = 1)
               AND (?12 IS NULL OR EXISTS (
                    SELECT 1 FROM json_each(COALESCE(annotation.tags_json, '[]')) tag
                    WHERE lower(tag.value) = lower(?12)
               ))
               AND (?13 IS NULL OR EXISTS (
                    SELECT 1 FROM image_collection_items collection_item
                    WHERE collection_item.collection_id = ?13
                      AND collection_item.source_kind = 'chat'
                      AND collection_item.source_key = c.id || ':' || m.id || ':' || json_extract(attachment.value, '$.id')
               ))
             ORDER BY CASE WHEN ?6 = 'oldest' THEN COALESCE(m.created_at, c.updated_at) END ASC,
                      CASE WHEN ?6 = 'newest' THEN COALESCE(m.created_at, c.updated_at) END DESC,
                      m.conversation_id ASC, m.id ASC,
                      json_extract(attachment.value, '$.id') ASC
             LIMIT ?3 OFFSET ?4",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map(
            params![
                MESSAGE_CONTENT_PREFIX,
                index_enabled,
                limit as i64,
                offset as i64,
                filters.generated_only,
                filters.sort,
                filters.created_after,
                filters.created_before,
                filters.model_id,
                filters.provider_route,
                filters.favorites_only,
                filters.tag,
                filters.collection_id,
            ],
            |row| {
                let summary = (|| -> rusqlite::Result<Option<AttachmentLibrarySummary>> {
                    let Some(conversation_id) = row.get::<_, Option<String>>(0)? else {
                        return Ok(None);
                    };
                    let Some(conversation_title) = row.get::<_, Option<String>>(1)? else {
                        return Ok(None);
                    };
                    let Some(message_id) = row.get::<_, Option<String>>(3)? else {
                        return Ok(None);
                    };
                    let Some(attachment_id) = row.get::<_, Option<String>>(5)? else {
                        return Ok(None);
                    };
                    let Some(file_name) = row.get::<_, Option<String>>(6)? else {
                        return Ok(None);
                    };
                    let Some(mime_type) = row.get::<_, Option<String>>(7)? else {
                        return Ok(None);
                    };
                    let Some(size_bytes) = row.get::<_, Option<i64>>(8)? else {
                        return Ok(None);
                    };
                    let Some(kind) = row.get::<_, Option<String>>(9)? else {
                        return Ok(None);
                    };
                    let Ok(size_bytes) = usize::try_from(size_bytes) else {
                        return Ok(None);
                    };
                    let metadata = AttachmentMetadata {
                        id: attachment_id.clone(),
                        name: file_name.clone(),
                        mime_type: mime_type.clone(),
                        size_bytes,
                        kind,
                    };
                    if validate_attachment_metadata(&metadata).is_err() {
                        return Ok(None);
                    }
                    let tags_json = row.get::<_, String>(15)?;
                    let tags =
                        serde_json::from_str::<Vec<String>>(&tags_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                15,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(Some(AttachmentLibrarySummary {
                        conversation_id,
                        conversation_title,
                        project_id: row.get(2)?,
                        message_id,
                        attachment_id,
                        file_name,
                        mime_type,
                        size_bytes,
                        created_at: row.get(4)?,
                        generation_id: row.get(10)?,
                        generation_prompt: row.get(11)?,
                        generation_model: row.get(12)?,
                        generation_provider_route: row.get(13)?,
                        is_favorite: row.get::<_, i64>(14)? != 0,
                        tags,
                    }))
                })()?;
                Ok(AttachmentLibraryCandidate {
                    summary,
                    indexed_content: row.get(16)?,
                })
            },
        )
        .map_err(storage_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage_error)
}

pub(super) fn attachment_text_index_enabled(
    connection: &StorageConnection<'_>,
) -> Result<bool, ServiceError> {
    let value = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [ATTACHMENT_TEXT_INDEX_SETTING],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    match value.as_deref() {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(_) => Err(storage_unavailable()),
    }
}

#[derive(Clone)]
pub(super) struct AttachmentInput {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) mime_type: String,
    pub(super) size_bytes: usize,
    pub(super) kind: String,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct AttachmentMetadata {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) mime_type: String,
    pub(super) size_bytes: usize,
    pub(super) kind: String,
}

pub(super) struct AttachmentEnvelope {
    pub(super) _content: String,
    pub(super) attachments: Vec<AttachmentMetadata>,
}

pub(super) struct StagedAttachments {
    pub(super) staging: Option<PathBuf>,
    pub(super) destination: PathBuf,
}

pub(super) fn parse_attachment_inputs(
    value: Option<&Value>,
) -> Result<Vec<AttachmentInput>, ServiceError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value.as_array().ok_or_else(invalid_request_params)?;
    if items.len() > MAX_ATTACHMENT_COUNT {
        return Err(invalid_request_params());
    }
    let mut ids = HashSet::new();
    let mut total = 0_usize;
    let mut images = 0_usize;
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let object = item.as_object().ok_or_else(invalid_request_params)?;
        let id = required_safe_id(object, "id")?.to_owned();
        if !ids.insert(id.clone()) {
            return Err(invalid_request_params());
        }
        let name = required_bounded_string(object, "name", MAX_ATTACHMENT_NAME_BYTES, true)?;
        let mime_type =
            required_bounded_string(object, "mimeType", MAX_ATTACHMENT_MIME_BYTES, true)?;
        let kind = required_bounded_string(object, "kind", MAX_ATTACHMENT_KIND_BYTES, true)?;
        if kind != "image" && kind != "text" {
            return Err(invalid_request_params());
        }
        let size_bytes = object
            .get("sizeBytes")
            .and_then(Value::as_u64)
            .and_then(|size| usize::try_from(size).ok())
            .filter(|size| *size > 0)
            .ok_or_else(invalid_request_params)?;
        let encoded = object
            .get("dataBase64")
            .and_then(Value::as_str)
            .ok_or_else(invalid_request_params)?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| invalid_request_params())?;
        if bytes.len() != size_bytes
            || size_bytes
                > if kind == "image" {
                    MAX_IMAGE_BYTES
                } else {
                    MAX_TEXT_BYTES
                }
        {
            return Err(invalid_request_params());
        }
        total = total
            .checked_add(size_bytes)
            .ok_or_else(invalid_request_params)?;
        if total > MAX_ATTACHMENT_BYTES {
            return Err(invalid_request_params());
        }
        if kind == "image" {
            images += 1;
            if images > MAX_IMAGE_COUNT {
                return Err(invalid_request_params());
            }
            if !valid_image_signature(mime_type, &bytes) {
                return Err(invalid_request_params());
            }
        } else if std::str::from_utf8(&bytes).is_err() {
            return Err(invalid_request_params());
        }
        result.push(AttachmentInput {
            id,
            name: name.to_owned(),
            mime_type: mime_type.to_owned(),
            size_bytes,
            kind: kind.to_owned(),
            bytes,
        });
    }
    Ok(result)
}

pub(super) fn parse_attachment_envelope(content: &str) -> Result<AttachmentEnvelope, ServiceError> {
    let encoded = content
        .strip_prefix(MESSAGE_CONTENT_PREFIX)
        .ok_or_else(storage_unavailable)?;
    let value: Value = serde_json::from_str(encoded).map_err(|_| storage_unavailable())?;
    let object = value.as_object().ok_or_else(storage_unavailable)?;
    let text = object
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(storage_unavailable)?
        .to_owned();
    let items = object
        .get("attachments")
        .and_then(Value::as_array)
        .ok_or_else(storage_unavailable)?;
    if items.is_empty() || items.len() > MAX_ATTACHMENT_COUNT {
        return Err(storage_unavailable());
    }
    let mut ids = HashSet::new();
    let mut total = 0_usize;
    let mut image_count = 0_usize;
    let mut attachments = Vec::with_capacity(items.len());
    for item in items {
        let object = item.as_object().ok_or_else(storage_unavailable)?;
        let id = required_safe_id(object, "id")
            .map_err(|_| storage_unavailable())?
            .to_owned();
        if !ids.insert(id.clone()) {
            return Err(storage_unavailable());
        }
        let name = required_bounded_string(object, "name", MAX_ATTACHMENT_NAME_BYTES, true)
            .map_err(|_| storage_unavailable())?;
        let mime_type =
            required_bounded_string(object, "mimeType", MAX_ATTACHMENT_MIME_BYTES, true)
                .map_err(|_| storage_unavailable())?;
        let kind = required_bounded_string(object, "kind", MAX_ATTACHMENT_KIND_BYTES, true)
            .map_err(|_| storage_unavailable())?;
        let size_bytes = object
            .get("sizeBytes")
            .and_then(Value::as_u64)
            .and_then(|size| usize::try_from(size).ok())
            .filter(|size| *size > 0)
            .ok_or_else(storage_unavailable)?;
        if kind != "image" && kind != "text" {
            return Err(storage_unavailable());
        }
        if kind == "image" {
            image_count += 1;
            if image_count > MAX_IMAGE_COUNT || size_bytes > MAX_IMAGE_BYTES {
                return Err(storage_unavailable());
            }
        } else if size_bytes > MAX_TEXT_BYTES {
            return Err(storage_unavailable());
        }
        total = total.saturating_add(size_bytes);
        if total > MAX_ATTACHMENT_BYTES {
            return Err(storage_unavailable());
        }
        attachments.push(AttachmentMetadata {
            id,
            name: name.to_owned(),
            mime_type: mime_type.to_owned(),
            size_bytes,
            kind: kind.to_owned(),
        });
    }
    Ok(AttachmentEnvelope {
        _content: text,
        attachments,
    })
}

pub(super) fn validate_attachment_metadata(
    metadata: &AttachmentMetadata,
) -> Result<(), ServiceError> {
    if !is_safe_identifier(&metadata.id)
        || metadata.name.trim().is_empty()
        || metadata.name.chars().count() > MAXIMUM_ATTACHMENT_NAME_CHARS
        || metadata.name.chars().any(char::is_control)
        || metadata.size_bytes == 0
    {
        return Err(storage_unavailable());
    }
    let Some((kind, mime_type, maximum_bytes)) = attachment_spec(&metadata.name) else {
        return Err(storage_unavailable());
    };
    if metadata.kind != kind
        || metadata.mime_type != mime_type
        || metadata.size_bytes > maximum_bytes
    {
        return Err(storage_unavailable());
    }
    Ok(())
}

pub(super) fn attachment_spec(name: &str) -> Option<(&'static str, &'static str, usize)> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let spec = match extension.as_str() {
        "png" => ("image", "image/png", MAX_IMAGE_BYTES),
        "jpg" | "jpeg" => ("image", "image/jpeg", MAX_IMAGE_BYTES),
        "webp" => ("image", "image/webp", MAX_IMAGE_BYTES),
        "gif" => ("image", "image/gif", MAX_IMAGE_BYTES),
        "txt" | "log" => ("text", "text/plain", MAX_TEXT_BYTES),
        "md" => ("text", "text/markdown", MAX_TEXT_BYTES),
        "csv" => ("text", "text/csv", MAX_TEXT_BYTES),
        "json" | "jsonl" => ("text", "application/json", MAX_TEXT_BYTES),
        "xml" => ("text", "application/xml", MAX_TEXT_BYTES),
        "html" | "htm" => ("text", "text/html", MAX_TEXT_BYTES),
        "pdf" => ("text", "application/pdf", MAX_TEXT_BYTES),
        "docx" => (
            "text",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            MAX_TEXT_BYTES,
        ),
        "xlsx" => (
            "text",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            MAX_TEXT_BYTES,
        ),
        "css" => ("text", "text/css", MAX_TEXT_BYTES),
        "js" | "jsx" => ("text", "text/javascript", MAX_TEXT_BYTES),
        "ts" | "tsx" => ("text", "text/typescript", MAX_TEXT_BYTES),
        "dart" | "rs" | "py" | "go" | "java" | "kt" | "swift" | "c" | "h" | "cpp" | "hpp"
        | "cs" | "sh" | "ps1" | "yaml" | "yml" | "toml" | "ini" | "sql" => {
            ("text", "text/plain", MAX_TEXT_BYTES)
        }
        _ => return None,
    };
    Some(spec)
}

pub(super) fn stage_attachments(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachments: &[AttachmentInput],
) -> Result<StagedAttachments, ServiceError> {
    let root = checked_directory(storage_root, "attachments", true)?;
    let conversation = checked_child_directory(&root, conversation_id, true)?;
    let destination_path = conversation.join(message_id);
    match fs::symlink_metadata(&destination_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(storage_unavailable());
            }
            let destination = checked_child_directory(&conversation, message_id, false)?;
            if !attachment_directory_matches(&destination, attachments)? {
                return Err(storage_unavailable());
            }
            return Ok(StagedAttachments {
                staging: None,
                destination,
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(storage_unavailable()),
    }
    let staging = conversation.join(format!(".stage-{message_id}-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|_| storage_unavailable())?;
    for attachment in attachments {
        let path = staging.join(format!("{}.data", attachment.id));
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.write_all(&attachment.bytes)?;
            file.sync_all()
        })();
        if write_result.is_err() {
            let _ = remove_directory_if_present(&staging);
            return Err(storage_unavailable());
        }
    }
    Ok(StagedAttachments {
        staging: Some(staging),
        destination: destination_path,
    })
}

pub(super) fn attachment_directory_matches(
    directory: &Path,
    attachments: &[AttachmentInput],
) -> Result<bool, ServiceError> {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(storage_unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(storage_unavailable());
    }
    let canonical_directory = fs::canonicalize(directory).map_err(|_| storage_unavailable())?;
    if canonical_directory != directory {
        return Err(storage_unavailable());
    }

    let mut expected = attachments
        .iter()
        .map(|attachment| format!("{}.data", attachment.id))
        .collect::<HashSet<_>>();
    for entry in fs::read_dir(&canonical_directory).map_err(|_| storage_unavailable())? {
        let entry = entry.map_err(|_| storage_unavailable())?;
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            return Ok(false);
        };
        if !expected.remove(&file_name) {
            return Ok(false);
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| storage_unavailable())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(storage_unavailable());
        }
        let Some(attachment) = attachments
            .iter()
            .find(|attachment| file_name == format!("{}.data", attachment.id))
        else {
            return Ok(false);
        };
        if metadata.len() != attachment.bytes.len() as u64 {
            return Ok(false);
        }
        let content = fs::read(path).map_err(|_| storage_unavailable())?;
        if content != attachment.bytes {
            return Ok(false);
        }
    }
    Ok(expected.is_empty())
}

pub(super) fn attachment_path(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
) -> Result<PathBuf, ServiceError> {
    let root = checked_directory(storage_root, "attachments", false)?;
    let conversation = checked_child_directory(&root, conversation_id, false)?;
    let message = checked_child_directory(&conversation, message_id, false)?;
    let path = message.join(format!("{attachment_id}.data"));
    let metadata = fs::symlink_metadata(&path).map_err(|_| storage_unavailable())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(storage_unavailable());
    }
    Ok(path)
}

pub(super) fn attachment_is_available(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
    expected_size: usize,
) -> Result<bool, ServiceError> {
    if !is_safe_identifier(conversation_id)
        || !is_safe_identifier(message_id)
        || !is_safe_identifier(attachment_id)
    {
        return Err(invalid_request_params());
    }
    let canonical_storage_root =
        fs::canonicalize(storage_root).map_err(|_| storage_unavailable())?;
    let Some(attachments_root) =
        optional_checked_directory(&canonical_storage_root, "attachments")?
    else {
        return Ok(false);
    };
    let Some(conversation_directory) =
        optional_checked_directory(&attachments_root, conversation_id)?
    else {
        return Ok(false);
    };
    let Some(message_directory) = optional_checked_directory(&conversation_directory, message_id)?
    else {
        return Ok(false);
    };
    let path = message_directory.join(format!("{attachment_id}.data"));
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(storage_unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(false);
    }
    if metadata.len() != expected_size as u64 {
        return Ok(false);
    }
    let canonical_path = fs::canonicalize(&path).map_err(|_| storage_unavailable())?;
    if canonical_path != path || !canonical_path.starts_with(&message_directory) {
        return Err(storage_unavailable());
    }
    Ok(true)
}

pub(super) fn optional_checked_directory(
    parent: &Path,
    name: &str,
) -> Result<Option<PathBuf>, ServiceError> {
    if !is_safe_identifier(name) {
        return Err(invalid_request_params());
    }
    let parent = fs::canonicalize(parent).map_err(|_| storage_unavailable())?;
    let expected_directory = parent.join(name);
    let metadata = match fs::symlink_metadata(&expected_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage_unavailable()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(storage_unavailable());
    }
    let canonical_directory =
        fs::canonicalize(&expected_directory).map_err(|_| storage_unavailable())?;
    if canonical_directory != expected_directory || !canonical_directory.starts_with(&parent) {
        return Err(storage_unavailable());
    }
    Ok(Some(canonical_directory))
}

pub(super) fn checked_directory(
    parent: &Path,
    name: &str,
    create: bool,
) -> Result<PathBuf, ServiceError> {
    let parent = fs::canonicalize(parent).map_err(|_| storage_unavailable())?;
    let path = parent.join(name);
    if create {
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(storage_unavailable()),
        }
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| storage_unavailable())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(storage_unavailable());
    }
    let canonical = fs::canonicalize(&path).map_err(|_| storage_unavailable())?;
    if canonical != path || !canonical.starts_with(&parent) {
        return Err(storage_unavailable());
    }
    Ok(canonical)
}

pub(super) fn checked_child_directory(
    parent: &Path,
    name: &str,
    create: bool,
) -> Result<PathBuf, ServiceError> {
    if !is_safe_identifier(name) {
        return Err(invalid_request_params());
    }
    checked_directory(parent, name, create)
}

pub(crate) fn remove_attachment_directory(
    storage_root: &Path,
    conversation_id: &str,
) -> Result<(), ServiceError> {
    if !is_safe_identifier(conversation_id) {
        return Err(invalid_request_params());
    }
    let Some(attachments) = optional_checked_directory(storage_root, "attachments")? else {
        return Ok(());
    };
    let Some(path) = optional_checked_directory(&attachments, conversation_id)? else {
        return Ok(());
    };
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(storage_unavailable()),
    }
}

pub(super) fn remove_all_attachment_directories(storage_root: &Path) -> Result<(), ServiceError> {
    let Some(attachments) = optional_checked_directory(storage_root, "attachments")? else {
        return Ok(());
    };
    for entry in fs::read_dir(&attachments).map_err(|_| storage_unavailable())? {
        let entry = entry.map_err(|_| storage_unavailable())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| storage_unavailable())?;
        let directory = checked_child_directory(&attachments, &name, false)?;
        fs::remove_dir_all(directory).map_err(|_| storage_unavailable())?;
    }
    Ok(())
}

pub(crate) fn cleanup_orphaned_attachment_directories(
    storage: &AppStorage,
) -> Result<(), ServiceError> {
    let Some(attachments) = optional_checked_directory(storage.root(), "attachments")? else {
        return Ok(());
    };
    let mut connection = storage.connect().map_err(storage_error)?;
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(storage_error)?;
    let mut statement = transaction
        .prepare("SELECT EXISTS (SELECT 1 FROM conversations WHERE id = ?1)")
        .map_err(storage_error)?;
    let mut cleanup_failed = false;

    for entry in fs::read_dir(&attachments).map_err(|_| storage_unavailable())? {
        let entry = entry.map_err(|_| storage_unavailable())?;
        let name = match entry.file_name().into_string() {
            Ok(name) if is_safe_identifier(&name) => name,
            _ => {
                cleanup_failed = true;
                continue;
            }
        };
        let directory = match checked_child_directory(&attachments, &name, false) {
            Ok(directory) => directory,
            Err(_) => {
                cleanup_failed = true;
                continue;
            }
        };
        let conversation_exists = statement
            .query_row([name.as_str()], |row| row.get::<_, bool>(0))
            .map_err(storage_error)?;
        if conversation_exists {
            continue;
        }
        match fs::remove_dir_all(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => cleanup_failed = true,
        }
    }

    drop(statement);
    transaction.commit().map_err(storage_error)?;
    if cleanup_failed {
        Err(storage_unavailable())
    } else {
        Ok(())
    }
}

pub(super) fn remove_directory_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(super) fn valid_image_signature(mime_type: &str, bytes: &[u8]) -> bool {
    match mime_type {
        "image/png" => bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "image/gif" => {
            bytes.len() >= 6
                && &bytes[..4] == b"GIF8"
                && (bytes[4] == b'7' || bytes[4] == b'9')
                && bytes[5] == b'a'
        }
        _ => false,
    }
}
