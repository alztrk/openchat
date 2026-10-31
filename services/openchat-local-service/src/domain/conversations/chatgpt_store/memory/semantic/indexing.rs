//! Text chunking, backfill selection, and vector index persistence helpers.

use super::*;

pub(super) fn load_model(directory: &Path) -> Result<SemanticModel, SemanticMemoryError> {
    let config_path = directory.join("config.json");
    let config_content = fs::read_to_string(&config_path)
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    let config: BertConfig = serde_json::from_str(&config_content)
        .map_err(|error| SemanticMemoryError::Model(error.to_string()))?;

    let tokenizer_path = directory.join("tokenizer.json");
    let mut tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
        .map_err(|error| SemanticMemoryError::Model(error.to_string()))?;
    if let Some(pp) = tokenizer.get_padding_mut() {
        pp.strategy = tokenizers::PaddingStrategy::BatchLongest;
    } else {
        let params = tokenizers::PaddingParams {
            strategy: tokenizers::PaddingStrategy::BatchLongest,
            ..Default::default()
        };
        tokenizer.with_padding(Some(params));
    }
    let truncation = tokenizers::TruncationParams {
        max_length: MODEL_MAX_LENGTH,
        ..Default::default()
    };
    let _ = tokenizer.with_truncation(Some(truncation));

    let weights_path = directory.join("model.safetensors");
    // SAFETY: The model safetensors file has been verified by its SHA-256 hash
    // and is pinned read-only inside the cache directory without concurrent writers.
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[&weights_path], DType::F32, &Device::Cpu)
            .map_err(|error| SemanticMemoryError::Model(error.to_string()))?
    };
    let model = BertModel::load(vb, &config)
        .map_err(|error| SemanticMemoryError::Model(error.to_string()))?;

    Ok(SemanticModel { model, tokenizer })
}

pub(super) fn new_index() -> Result<VectorIndex, SemanticMemoryError> {
    Ok(VectorIndex::new(VECTOR_DIMENSIONS))
}

pub(super) fn chunk_text(
    tokenizer: &tokenizers::Tokenizer,
    content: &str,
) -> Result<Vec<TextChunk>, SemanticMemoryError> {
    let encoding = tokenizer
        .encode(content, false)
        .map_err(|error| SemanticMemoryError::Model(error.to_string()))?;
    let offsets = encoding.get_offsets();
    let mut chunks = Vec::new();
    let mut start_token = 0;
    while start_token < offsets.len() {
        let end_token = start_token
            .saturating_add(CHUNK_TOKEN_COUNT)
            .min(offsets.len());
        let start_byte = offsets[start_token].0;
        let end_byte = offsets[end_token - 1].1;
        if end_byte > start_byte
            && let Some(text) = content.get(start_byte..end_byte)
        {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let trimmed_start = text.len() - text.trim_start().len();
                let final_start = start_byte + trimmed_start;
                chunks.push(TextChunk {
                    start_byte: final_start,
                    end_byte: final_start + trimmed.len(),
                    content: trimmed.to_owned(),
                });
            }
        }
        if end_token == offsets.len() {
            break;
        }
        let next_start = end_token.saturating_sub(CHUNK_TOKEN_OVERLAP);
        if next_start <= start_token {
            return Err(SemanticMemoryError::Model(
                "tokenizer produced a non-advancing chunk".to_owned(),
            ));
        }
        start_token = next_start;
    }
    Ok(chunks)
}

pub(super) fn quantize_embedding(embedding: Vec<f32>) -> Result<Vec<i8>, SemanticMemoryError> {
    vector_math::quantize_embedding_unrolled(&embedding, VECTOR_DIMENSIONS)
        .map_err(SemanticMemoryError::Model)
}

pub(super) fn ensure_namespace(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
) -> Result<i64, SemanticMemoryError> {
    connection.execute(
        "INSERT OR IGNORE INTO conversation_memory_embedding_namespaces (conversation_id)
         VALUES (?1)",
        [conversation_id],
    )?;
    connection
        .query_row(
            "SELECT namespace FROM conversation_memory_embedding_namespaces
             WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

pub(super) fn index_key(namespace: i64, id: i64) -> Result<u64, SemanticMemoryError> {
    if !(1..=16_777_215).contains(&namespace) || id <= 0 || id as u64 > INDEX_KEY_ID_MASK {
        return Err(SemanticMemoryError::Index(
            "conversation index key is outside its supported range".to_owned(),
        ));
    }
    Ok(((namespace as u64) << INDEX_KEY_ID_BITS) | id as u64)
}

pub(super) fn pending_messages(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
    through_message_id: Option<&str>,
) -> Result<Vec<MemoryMessage>, SemanticMemoryError> {
    let mut statement = connection.prepare(
        "SELECT message.conversation_id, message.id, message.content,
                COALESCE(tool.content, ''), COALESCE(message.created_at, 0)
         FROM conversation_memory_embedding_pending AS pending
         JOIN messages AS message
           ON message.conversation_id = pending.conversation_id
          AND message.id = pending.message_id
         LEFT JOIN conversation_memory_tools_fts AS tool ON tool.rowid = message.rowid
         WHERE message.conversation_id = ?1
           AND message.role IN ('user', 'assistant') AND message.status = 'completed'
           AND NOT EXISTS (
                SELECT 1 FROM conversation_memory_archive_settings AS excluded
                WHERE excluded.conversation_id = message.conversation_id
                  AND excluded.included = 0
           )
           AND (?2 IS NULL OR (
                COALESCE(message.created_at, 0) < COALESCE((
                    SELECT boundary.created_at FROM messages AS boundary
                    WHERE boundary.conversation_id = ?1 AND boundary.id = ?2
                ), 0)
                OR (COALESCE(message.created_at, 0) = COALESCE((
                    SELECT boundary.created_at FROM messages AS boundary
                    WHERE boundary.conversation_id = ?1 AND boundary.id = ?2
                ), 0) AND message.id <= ?2)
           ))
         ORDER BY COALESCE(message.created_at, 0), message.id LIMIT 32",
    )?;
    statement
        .query_map(
            params![conversation_id, through_message_id],
            memory_message_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn next_backfill_messages(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
    cursor_created_at: Option<i64>,
    cursor_message_id: Option<&str>,
    through_message_id: Option<&str>,
) -> Result<Vec<MemoryMessage>, SemanticMemoryError> {
    let mut statement = connection.prepare(
        "SELECT message.conversation_id, message.id, message.content,
                COALESCE(tool.content, ''), COALESCE(message.created_at, 0)
         FROM messages AS message
         LEFT JOIN conversation_memory_embedding_state AS state
           ON state.conversation_id = message.conversation_id AND state.message_id = message.id
          AND state.model_id = ?5
         LEFT JOIN conversation_memory_tools_fts AS tool ON tool.rowid = message.rowid
         WHERE message.conversation_id = ?1
           AND message.role IN ('user', 'assistant') AND message.status = 'completed'
           AND state.message_id IS NULL
           AND NOT EXISTS (
                SELECT 1 FROM conversation_memory_archive_settings AS excluded
                WHERE excluded.conversation_id = message.conversation_id
                  AND excluded.included = 0
           )
           AND (?2 IS NULL OR COALESCE(message.created_at, 0) > ?2
                OR (COALESCE(message.created_at, 0) = ?2 AND message.id > ?3))
           AND (?4 IS NULL OR (
                COALESCE(message.created_at, 0) < COALESCE((
                    SELECT boundary.created_at FROM messages AS boundary
                    WHERE boundary.conversation_id = ?1 AND boundary.id = ?4
                ), 0)
                OR (COALESCE(message.created_at, 0) = COALESCE((
                    SELECT boundary.created_at FROM messages AS boundary
                    WHERE boundary.conversation_id = ?1 AND boundary.id = ?4
                ), 0) AND message.id <= ?4)
           ))
         ORDER BY COALESCE(message.created_at, 0), message.id LIMIT 32",
    )?;
    statement
        .query_map(
            params![
                conversation_id,
                cursor_created_at,
                cursor_message_id,
                through_message_id,
                MODEL_ID,
            ],
            memory_message_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn memory_message_from_row(row: &StorageRow) -> rusqlite::Result<MemoryMessage> {
    Ok(MemoryMessage {
        conversation_id: row.get(0)?,
        message_id: row.get(1)?,
        content: row.get(2)?,
        tool_content: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub(super) fn begin_embedding_write<'a>(
    connection: &'a mut StorageConnection<'_>,
) -> Result<StorageTransaction<'a>, SemanticMemoryError> {
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "UPDATE conversation_memory_semantic_index_state
         SET generation = generation WHERE id = 1",
        [],
    )?;
    Ok(transaction)
}

pub(super) fn current_embedding_source(
    connection: &StorageTransaction<'_>,
    conversation_id: &str,
    message_id: &str,
) -> rusqlite::Result<Option<String>> {
    connection
        .query_row(
            "SELECT message.content, COALESCE(tool.content, '')
             FROM messages AS message
             LEFT JOIN conversation_memory_tools_fts AS tool ON tool.rowid = message.rowid
             WHERE message.conversation_id = ?1 AND message.id = ?2
               AND message.role IN ('user', 'assistant') AND message.status = 'completed'",
            params![conversation_id, message_id],
            |row| {
                Ok(embedding_content(
                    &row.get::<_, String>(0)?,
                    &row.get::<_, String>(1)?,
                ))
            },
        )
        .optional()
}

pub(super) fn embedding_content(message_content: &str, tool_content: &str) -> String {
    match (message_content.is_empty(), tool_content.is_empty()) {
        (false, false) => format!("{message_content}\n\n{tool_content}"),
        (false, true) => message_content.to_owned(),
        (true, false) => tool_content.to_owned(),
        (true, true) => String::new(),
    }
}

pub(super) fn boundary_order(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
    through_message_id: Option<&str>,
) -> Result<Option<(i64, String)>, SemanticMemoryError> {
    through_message_id
        .map(|message_id| {
            connection
                .query_row(
                    "SELECT COALESCE(created_at, 0), id FROM messages
                     WHERE conversation_id = ?1 AND id = ?2",
                    params![conversation_id, message_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(Into::into)
        })
        .transpose()
}

pub(super) fn semantic_excerpt(
    connection: &StorageConnection<'_>,
    conversation_id: &str,
    embedding_id: i64,
    boundary: Option<&(i64, String)>,
) -> Result<Option<ArchivedMemoryExcerpt>, SemanticMemoryError> {
    connection
        .query_row(
            "SELECT message.id, message.role, message.content,
                    COALESCE(tool.content, ''), COALESCE(message.created_at, 0),
                    embedding.start_byte, embedding.end_byte
             FROM conversation_memory_embeddings AS embedding
             JOIN messages AS message
               ON message.conversation_id = embedding.conversation_id
              AND message.id = embedding.message_id
             LEFT JOIN conversation_memory_tools_fts AS tool ON tool.rowid = message.rowid
             WHERE embedding.id = ?1 AND embedding.conversation_id = ?2
               AND message.role IN ('user', 'assistant') AND message.status = 'completed'
               AND (?3 IS NULL OR (
                    COALESCE(message.created_at, 0) < ?3
                    OR (COALESCE(message.created_at, 0) = ?3 AND message.id <= ?4)
               ))",
            params![
                embedding_id,
                conversation_id,
                boundary.map(|value| value.0),
                boundary.map(|value| value.1.as_str()),
            ],
            |row| {
                let message_content: String = row.get(2)?;
                let tool_content: String = row.get(3)?;
                let content = embedding_content(&message_content, &tool_content);
                let start_byte = row.get::<_, i64>(5)? as usize;
                let end_byte = row.get::<_, i64>(6)? as usize;
                let excerpt = content.get(start_byte..end_byte).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        "semantic chunk offsets no longer match their source message".into(),
                    )
                })?;
                Ok(ArchivedMemoryExcerpt {
                    message_id: row.get(0)?,
                    role: row.get(1)?,
                    content: search::bounded_archive_excerpt(excerpt.to_owned()),
                    created_at_unix_ms: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

pub(super) fn semantic_index_generation(
    connection: &StorageConnection<'_>,
) -> Result<i64, SemanticMemoryError> {
    connection
        .query_row(
            "SELECT generation FROM conversation_memory_semantic_index_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

pub(super) fn set_indexed_generation(
    connection: &StorageConnection<'_>,
    generation: i64,
) -> Result<(), SemanticMemoryError> {
    connection.execute(
        "UPDATE conversation_memory_semantic_index_state
         SET indexed_generation = ?1 WHERE id = 1",
        [generation],
    )?;
    Ok(())
}

pub(super) fn save_index(
    connection: &StorageConnection<'_>,
    cache_directory: &Path,
    index: &VectorIndex,
    generation: i64,
) -> Result<(), SemanticMemoryError> {
    let index_directory = cache_directory.join(MODEL_ID);
    fs::create_dir_all(&index_directory)?;
    let destination = index_path(cache_directory, generation);
    let indexed_generation = connection.query_row(
        "SELECT indexed_generation FROM conversation_memory_semantic_index_state WHERE id = 1",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    if indexed_generation == Some(generation) && destination.is_file() {
        return Ok(());
    }
    let temporary = destination.with_extension("partial");
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    let _temporary_file = IndexTemporaryFile(temporary.clone());
    index.save(&path_string(&temporary))?;
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    fs::rename(&temporary, &destination)?;
    set_indexed_generation(connection, generation)?;
    prune_cached_indexes(&index_directory, Some(&destination))
}

pub(super) fn index_path(cache_directory: &Path, generation: i64) -> PathBuf {
    model_directory(cache_directory).join(format!("index-{generation}.usearch"))
}

pub(super) fn prune_cached_indexes(
    directory: &Path,
    preserved_index: Option<&Path>,
) -> Result<(), SemanticMemoryError> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(SemanticMemoryError::Index(
                "semantic index cache path is invalid".to_owned(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(SemanticMemoryError::Io(error.to_string())),
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if preserved_index.is_some_and(|preserved| path == preserved) {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.starts_with("index-")
            && (path
                .extension()
                .is_some_and(|extension| extension == "usearch")
                || file_name.ends_with(".usearch.partial"))
        {
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(SemanticMemoryError::Index(
                    "semantic index cache file is invalid".to_owned(),
                ));
            }
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub(super) fn model_directory(cache_directory: &Path) -> PathBuf {
    cache_directory.join(MODEL_ID)
}

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
