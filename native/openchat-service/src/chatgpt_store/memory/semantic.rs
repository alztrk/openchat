use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    time::Duration,
};

use fastembed::{
    InitOptionsUserDefined, Pooling, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel,
};
use futures_util::StreamExt;
use reqwest::{
    StatusCode,
    header::{CONTENT_RANGE, RANGE},
};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc as tokio_mpsc, watch},
    time::Instant,
};
use usearch::{
    Index, IndexOptions,
    ffi::{MetricKind, ScalarKind},
};

use super::{ArchivedMemoryExcerpt, index, search};

const MODEL_ID: &str = "multilingual-e5-small-761b726d";
const MODEL_REPOSITORY: &str = "Xenova/multilingual-e5-small";
const MODEL_REVISION: &str = "761b726dd34fb83930e26aab4e9ac3899aa1fa78";
const MODEL_DOWNLOAD_BASE_URL: &str = "https://huggingface.co";
const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(150);
const PARTIAL_SYNC_INTERVAL_BYTES: u64 = 8 * 1024 * 1024;
const VECTOR_DIMENSIONS: usize = 384;
const MODEL_MAX_LENGTH: usize = 512;
const CHUNK_TOKEN_COUNT: usize = 384;
const CHUNK_TOKEN_OVERLAP: usize = 64;
const INDEX_KEY_ID_BITS: u32 = 40;
const INDEX_KEY_ID_MASK: u64 = (1_u64 << INDEX_KEY_ID_BITS) - 1;
const MODEL_FILES: [ModelFile; 5] = [
    ModelFile {
        path: "onnx/model_quantized.onnx",
        local_name: "model_quantized.onnx",
        size: 118_308_185,
        sha256: "f80102d3f2a1229f387d3c81909990d8945513e347b0eab049f7de3c6f98c193",
    },
    ModelFile {
        path: "tokenizer.json",
        local_name: "tokenizer.json",
        size: 17_082_730,
        sha256: "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
    },
    ModelFile {
        path: "config.json",
        local_name: "config.json",
        size: 658,
        sha256: "cb99455288675345e1a4f411438d5d0adbba5fbd3a67ea4fb03c015433b996c1",
    },
    ModelFile {
        path: "special_tokens_map.json",
        local_name: "special_tokens_map.json",
        size: 167,
        sha256: "d05497f1da52c5e09554c0cd874037a083e1dc1b9cfd48034d1c717f1afc07a7",
    },
    ModelFile {
        path: "tokenizer_config.json",
        local_name: "tokenizer_config.json",
        size: 443,
        sha256: "a1d6bc8734a6f635dc158508bef000f8e2e5a759c7d92f984b2c86e5ff53425b",
    },
];

#[derive(Clone, Copy)]
struct ModelFile {
    path: &'static str,
    local_name: &'static str,
    size: u64,
    sha256: &'static str,
}

#[derive(Debug)]
pub(super) enum SemanticMemoryError {
    Io(String),
    Http(String),
    Integrity,
    Cancelled,
    Database(rusqlite::Error),
    Model(String),
    Index(String),
    Worker(String),
}

impl std::fmt::Display for SemanticMemoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "semantic model file error: {error}"),
            Self::Http(error) => write!(formatter, "semantic model download error: {error}"),
            Self::Integrity => formatter.write_str("semantic model checksum verification failed"),
            Self::Cancelled => formatter.write_str("semantic model preparation was cancelled"),
            Self::Database(error) => write!(formatter, "semantic memory database error: {error}"),
            Self::Model(error) => write!(formatter, "semantic embedding model error: {error}"),
            Self::Index(error) => write!(formatter, "semantic index error: {error}"),
            Self::Worker(error) => write!(formatter, "semantic worker error: {error}"),
        }
    }
}

impl std::error::Error for SemanticMemoryError {}

impl From<std::io::Error> for SemanticMemoryError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<rusqlite::Error> for SemanticMemoryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

enum WorkerMessage {
    Prepare {
        database_path: PathBuf,
        cache_directory: PathBuf,
        response: SyncSender<Result<(), SemanticMemoryError>>,
    },
    Search {
        database_path: PathBuf,
        cache_directory: PathBuf,
        conversation_id: String,
        through_message_id: Option<String>,
        query: String,
        result_limit: usize,
        response: SyncSender<Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError>>,
    },
    InvalidateIndex {
        database_path: PathBuf,
        cache_directory: PathBuf,
        response: SyncSender<Result<(), SemanticMemoryError>>,
    },
}

static WORKER: OnceLock<Result<Sender<WorkerMessage>, String>> = OnceLock::new();
static PREPARE_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

pub(super) async fn is_ready(cache_directory: &Path) -> bool {
    let model_directory = model_directory(cache_directory);
    let marker_path = model_directory.join(".ready");
    let Ok(marker) = tokio::fs::read_to_string(marker_path).await else {
        return false;
    };
    if marker.trim() != MODEL_ID {
        return false;
    }
    for file in MODEL_FILES {
        let Ok(metadata) = tokio::fs::metadata(model_directory.join(file.local_name)).await else {
            return false;
        };
        if metadata.len() != file.size {
            return false;
        }
    }
    true
}

pub(super) async fn prepare(
    database_path: PathBuf,
    cache_directory: PathBuf,
    cancellation: &mut watch::Receiver<bool>,
    progress: tokio_mpsc::UnboundedSender<serde_json::Value>,
) -> Result<(), SemanticMemoryError> {
    if *cancellation.borrow() {
        return Err(SemanticMemoryError::Cancelled);
    }
    let _prepare_guard = PREPARE_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    if *cancellation.borrow() {
        return Err(SemanticMemoryError::Cancelled);
    }
    let directory = model_directory(&cache_directory);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;

    match tokio::fs::remove_file(directory.join(".ready")).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(SemanticMemoryError::Io(error.to_string())),
    }

    let total_bytes = MODEL_FILES.iter().map(|file| file.size).sum::<u64>();
    let mut downloaded_bytes = 0_u64;
    report_progress(
        &progress,
        "downloading",
        downloaded_bytes,
        total_bytes,
        None,
    )?;
    for file in MODEL_FILES {
        if *cancellation.borrow() {
            return Err(SemanticMemoryError::Cancelled);
        }
        let destination = directory.join(file.local_name);
        if verify_file(destination.clone(), file).await? {
            downloaded_bytes = downloaded_bytes.saturating_add(file.size);
            report_progress(
                &progress,
                "downloading",
                downloaded_bytes,
                total_bytes,
                Some(file.local_name),
            )?;
            continue;
        }
        download_file(
            &destination,
            file,
            downloaded_bytes,
            total_bytes,
            &progress,
            cancellation,
        )
        .await?;
        downloaded_bytes = downloaded_bytes.saturating_add(file.size);
    }

    if *cancellation.borrow() {
        return Err(SemanticMemoryError::Cancelled);
    }
    report_progress(&progress, "indexing", total_bytes, total_bytes, None)?;
    run_prepare_worker(database_path, cache_directory.clone()).await?;
    tokio::fs::write(directory.join(".ready"), MODEL_ID)
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    report_progress(&progress, "ready", total_bytes, total_bytes, None)?;
    Ok(())
}

pub(super) async fn search_archive(
    database_path: PathBuf,
    cache_directory: PathBuf,
    conversation_id: String,
    through_message_id: Option<String>,
    query: String,
    result_limit: usize,
) -> Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError> {
    run_search_worker(
        database_path,
        cache_directory,
        conversation_id,
        through_message_id,
        query,
        result_limit,
    )
    .await
}

async fn run_prepare_worker(
    database_path: PathBuf,
    cache_directory: PathBuf,
) -> Result<(), SemanticMemoryError> {
    tokio::task::spawn_blocking(move || {
        let sender = worker_sender()?;
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        sender
            .send(WorkerMessage::Prepare {
                database_path,
                cache_directory,
                response: response_sender,
            })
            .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?;
        response_receiver
            .recv()
            .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?
    })
    .await
    .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?
}

async fn run_search_worker(
    database_path: PathBuf,
    cache_directory: PathBuf,
    conversation_id: String,
    through_message_id: Option<String>,
    query: String,
    result_limit: usize,
) -> Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError> {
    tokio::task::spawn_blocking(move || {
        let sender = worker_sender()?;
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        sender
            .send(WorkerMessage::Search {
                database_path,
                cache_directory,
                conversation_id,
                through_message_id,
                query,
                result_limit,
                response: response_sender,
            })
            .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?;
        response_receiver
            .recv()
            .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?
    })
    .await
    .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?
}

fn worker_sender() -> Result<&'static Sender<WorkerMessage>, SemanticMemoryError> {
    match WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("openchat-semantic-memory".to_owned())
            .spawn(move || worker_loop(receiver))
            .map(|_| sender)
            .map_err(|error| error.to_string())
    }) {
        Ok(sender) => Ok(sender),
        Err(error) => Err(SemanticMemoryError::Worker(error.clone())),
    }
}

fn worker_loop(receiver: Receiver<WorkerMessage>) {
    let mut runtime = SemanticRuntime::default();
    while let Ok(request) = receiver.recv() {
        match request {
            WorkerMessage::Prepare {
                database_path,
                cache_directory,
                response,
            } => {
                let result = runtime.prepare(&database_path, &cache_directory);
                let _ = response.send(result);
            }
            WorkerMessage::Search {
                database_path,
                cache_directory,
                conversation_id,
                through_message_id,
                query,
                result_limit,
                response,
            } => {
                let result = runtime.search(
                    &database_path,
                    &cache_directory,
                    &conversation_id,
                    through_message_id.as_deref(),
                    &query,
                    result_limit,
                );
                let _ = response.send(result);
            }
            WorkerMessage::InvalidateIndex {
                database_path,
                cache_directory,
                response,
            } => {
                let result = runtime.invalidate_cached_index(&database_path, &cache_directory);
                let _ = response.send(result);
            }
        }
    }
}

pub(super) async fn invalidate_cached_index(
    database_path: PathBuf,
    cache_directory: PathBuf,
) -> Result<(), SemanticMemoryError> {
    tokio::task::spawn_blocking(move || {
        let sender = worker_sender()?;
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        sender
            .send(WorkerMessage::InvalidateIndex {
                database_path,
                cache_directory,
                response: response_sender,
            })
            .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?;
        response_receiver
            .recv()
            .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?
    })
    .await
    .map_err(|error| SemanticMemoryError::Worker(error.to_string()))?
}

#[derive(Default)]
struct SemanticRuntime {
    model: Option<TextEmbedding>,
    index: Option<Index>,
    model_directory: Option<PathBuf>,
    index_generation: Option<i64>,
}

impl SemanticRuntime {
    fn prepare(
        &mut self,
        database_path: &Path,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        let connection = open_database(database_path)?;
        self.load_model_and_index(&connection, cache_directory)
    }

    fn search(
        &mut self,
        database_path: &Path,
        cache_directory: &Path,
        conversation_id: &str,
        through_message_id: Option<&str>,
        query: &str,
        result_limit: usize,
    ) -> Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError> {
        let connection = open_database(database_path)?;
        if !index::archive_indexing_enabled(&connection, conversation_id)? {
            return Ok(Vec::new());
        }
        self.load_model_and_index(&connection, cache_directory)?;
        self.ensure_index_current(&connection, cache_directory)?;
        let namespace = ensure_namespace(&connection, conversation_id)?;
        self.ensure_conversation_indexed(
            &connection,
            conversation_id,
            through_message_id,
            namespace,
            cache_directory,
        )?;

        let model = self
            .model
            .as_mut()
            .ok_or_else(|| SemanticMemoryError::Model("model is not loaded".to_owned()))?;
        let query_text = format!("query: {query}");
        let query_embedding = model
            .embed([query_text.as_str()], Some(1))
            .map_err(|error| SemanticMemoryError::Model(error.to_string()))?
            .into_iter()
            .next()
            .ok_or_else(|| SemanticMemoryError::Model("query embedding is empty".to_owned()))?;
        let query_vector = quantize_embedding(query_embedding)?;
        let index = self
            .index
            .as_ref()
            .ok_or_else(|| SemanticMemoryError::Index("index is not loaded".to_owned()))?;
        if result_limit == 0 || index.size() == 0 {
            return Ok(Vec::new());
        }
        let boundary = boundary_order(&connection, conversation_id, through_message_id)?;
        let mut excerpts = Vec::with_capacity(result_limit);
        let mut seen_messages = HashSet::new();
        let mut candidate_count = (result_limit.saturating_mul(8)).max(64).min(index.size());
        loop {
            let matches = index
                .filtered_search(&query_vector, candidate_count, |key| {
                    key >> INDEX_KEY_ID_BITS == namespace as u64
                })
                .map_err(|error| SemanticMemoryError::Index(error.to_string()))?;
            for (key, distance) in matches.keys.iter().zip(matches.distances.iter()) {
                let id = key & INDEX_KEY_ID_MASK;
                let Ok(id) = i64::try_from(id) else {
                    continue;
                };
                if let Some(excerpt) =
                    semantic_excerpt(&connection, conversation_id, id, boundary.as_ref())?
                    && seen_messages.insert(excerpt.message_id.clone())
                {
                    excerpts.push((*distance, excerpt));
                    if excerpts.len() == result_limit {
                        break;
                    }
                }
            }
            if excerpts.len() == result_limit || candidate_count == index.size() {
                break;
            }
            candidate_count = candidate_count.saturating_mul(2).min(index.size());
        }
        if !index::archive_indexing_enabled(&connection, conversation_id)? {
            return Ok(Vec::new());
        }
        let indexed_generation = self.index_generation.ok_or_else(|| {
            SemanticMemoryError::Index("semantic index generation is unknown".to_owned())
        })?;
        if semantic_index_generation(&connection)? != indexed_generation {
            return Ok(Vec::new());
        }
        Ok(excerpts.into_iter().map(|(_, excerpt)| excerpt).collect())
    }

    fn load_model_and_index(
        &mut self,
        connection: &Connection,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        let model_directory = model_directory(cache_directory);
        if self.model_directory.as_deref() != Some(model_directory.as_path()) {
            self.model = Some(load_model(&model_directory)?);
            self.model_directory = Some(model_directory);
            self.index = None;
            self.index_generation = None;
        }
        self.ensure_index_current(connection, cache_directory)
    }

    fn ensure_index_current(
        &mut self,
        connection: &Connection,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        let generation = semantic_index_generation(connection)?;
        if self.index_generation == Some(generation) && self.index.is_some() {
            return Ok(());
        }
        let index_path = index_path(cache_directory, generation);
        if self.index.is_none()
            && index_path.is_file()
            && let Ok(index) = Index::restore(&path_string(&index_path))
        {
            self.index = Some(index);
            self.index_generation = Some(generation);
            set_indexed_generation(connection, generation)?;
            return Ok(());
        }

        let index = new_index()?;
        let vector_count = connection.query_row(
            "SELECT COUNT(*) FROM conversation_memory_embeddings WHERE model_id = ?1",
            [MODEL_ID],
            |row| row.get::<_, i64>(0),
        )?;
        let vector_count = usize::try_from(vector_count).map_err(|_| {
            SemanticMemoryError::Index("semantic vector count is out of range".to_owned())
        })?;
        if vector_count > 0 {
            index
                .reserve(vector_count)
                .map_err(|error| SemanticMemoryError::Index(error.to_string()))?;
        }
        let mut statement = connection.prepare(
            "SELECT embedding.id, namespace.namespace, embedding.vector
             FROM conversation_memory_embeddings AS embedding
             JOIN conversation_memory_embedding_namespaces AS namespace
               ON namespace.conversation_id = embedding.conversation_id
             WHERE embedding.model_id = ?1
             ORDER BY embedding.id",
        )?;
        let mut rows = statement.query([MODEL_ID])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let namespace: i64 = row.get(1)?;
            let bytes: Vec<u8> = row.get(2)?;
            let vector = bytes.into_iter().map(|byte| byte as i8).collect::<Vec<_>>();
            let key = index_key(namespace, id)?;
            index
                .add(key, &vector)
                .map_err(|error| SemanticMemoryError::Index(error.to_string()))?;
        }
        save_index(connection, cache_directory, &index, generation)?;
        self.index = Some(index);
        self.index_generation = Some(generation);
        Ok(())
    }

    fn ensure_conversation_indexed(
        &mut self,
        connection: &Connection,
        conversation_id: &str,
        through_message_id: Option<&str>,
        namespace: i64,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        if !index::archive_indexing_enabled(connection, conversation_id)? {
            return Ok(());
        }
        connection.execute(
            "INSERT OR IGNORE INTO conversation_memory_embedding_backfill (conversation_id)
             VALUES (?1)",
            [conversation_id],
        )?;

        loop {
            if !index::archive_indexing_enabled(connection, conversation_id)? {
                return Ok(());
            }
            let pending = pending_messages(connection, conversation_id, through_message_id)?;
            if !pending.is_empty() {
                self.embed_messages(connection, &pending, namespace, cache_directory)?;
                continue;
            }

            let cursor = connection.query_row(
                "SELECT cursor_created_at, cursor_message_id
                 FROM conversation_memory_embedding_backfill WHERE conversation_id = ?1",
                [conversation_id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )?;
            let batch = next_backfill_messages(
                connection,
                conversation_id,
                cursor.0,
                cursor.1.as_deref(),
                through_message_id,
            )?;
            let Some(last) = batch.last() else {
                break;
            };
            self.embed_messages(connection, &batch, namespace, cache_directory)?;
            connection.execute(
                "UPDATE conversation_memory_embedding_backfill
                 SET cursor_created_at = ?2, cursor_message_id = ?3
                 WHERE conversation_id = ?1",
                params![conversation_id, last.created_at, last.message_id],
            )?;
        }
        if !index::archive_indexing_enabled(connection, conversation_id)? {
            return Ok(());
        }
        let generation = semantic_index_generation(connection)?;
        let index = self
            .index
            .as_ref()
            .ok_or_else(|| SemanticMemoryError::Index("index is not loaded".to_owned()))?;
        save_index(connection, cache_directory, index, generation)?;
        self.index_generation = Some(generation);
        Ok(())
    }

    fn invalidate_cached_index(
        &mut self,
        database_path: &Path,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        let connection = open_database(database_path)?;
        connection.execute(
            "UPDATE conversation_memory_semantic_index_state
             SET indexed_generation = NULL WHERE id = 1",
            [],
        )?;
        self.index = None;
        self.index_generation = None;

        let directory = model_directory(cache_directory);
        match fs::symlink_metadata(&directory) {
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

    fn embed_messages(
        &mut self,
        connection: &Connection,
        messages: &[MemoryMessage],
        namespace: i64,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        for message in messages {
            let source_content = embedding_content(&message.content, &message.tool_content);
            let content_hash = sha256(source_content.as_bytes());
            let current_hash = connection
                .query_row(
                    "SELECT content_hash FROM conversation_memory_embedding_state
                     WHERE conversation_id = ?1 AND message_id = ?2 AND model_id = ?3",
                    params![message.conversation_id, message.message_id, MODEL_ID],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if current_hash.as_deref() == Some(content_hash.as_str()) {
                let transaction = begin_embedding_write(connection)?;
                if !index::archive_indexing_enabled(&transaction, &message.conversation_id)? {
                    transaction.rollback()?;
                    return Ok(());
                }
                if current_embedding_source(
                    &transaction,
                    &message.conversation_id,
                    &message.message_id,
                )?
                .as_deref()
                    != Some(source_content.as_str())
                {
                    transaction.rollback()?;
                    continue;
                }
                transaction.execute(
                    "DELETE FROM conversation_memory_embedding_pending
                     WHERE conversation_id = ?1 AND message_id = ?2",
                    params![message.conversation_id, message.message_id],
                )?;
                transaction.commit()?;
                continue;
            }

            let model = self
                .model
                .as_mut()
                .ok_or_else(|| SemanticMemoryError::Model("model is not loaded".to_owned()))?;
            let chunks = chunk_text(&model.tokenizer, &source_content)?;
            let chunk_texts = chunks
                .iter()
                .map(|chunk| format!("passage: {}", chunk.content))
                .collect::<Vec<_>>();
            let embeddings = if chunk_texts.is_empty() {
                Vec::new()
            } else {
                model
                    .embed(&chunk_texts, Some(32))
                    .map_err(|error| SemanticMemoryError::Model(error.to_string()))?
            };
            if embeddings.len() != chunks.len() {
                return Err(SemanticMemoryError::Model(
                    "model returned an unexpected embedding count".to_owned(),
                ));
            }
            let vectors = embeddings
                .into_iter()
                .map(quantize_embedding)
                .collect::<Result<Vec<_>, _>>()?;
            let transaction = begin_embedding_write(connection)?;
            if !index::archive_indexing_enabled(&transaction, &message.conversation_id)? {
                transaction.rollback()?;
                return Ok(());
            }
            if current_embedding_source(
                &transaction,
                &message.conversation_id,
                &message.message_id,
            )?
            .as_deref()
                != Some(source_content.as_str())
            {
                transaction.rollback()?;
                continue;
            }
            let old_ids = transaction
                .prepare(
                    "SELECT id FROM conversation_memory_embeddings
                     WHERE conversation_id = ?1 AND message_id = ?2",
                )?
                .query_map(
                    params![message.conversation_id, message.message_id],
                    |row| row.get::<_, i64>(0),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            transaction.execute(
                "DELETE FROM conversation_memory_embeddings
                 WHERE conversation_id = ?1 AND message_id = ?2",
                params![message.conversation_id, message.message_id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_embedding_state
                 WHERE conversation_id = ?1 AND message_id = ?2",
                params![message.conversation_id, message.message_id],
            )?;
            let mut new_ids = Vec::with_capacity(chunks.len());
            for ((chunk_index, chunk), vector) in chunks.iter().enumerate().zip(vectors.iter()) {
                let vector_bytes = vector.iter().map(|value| *value as u8).collect::<Vec<_>>();
                transaction.execute(
                    "INSERT INTO conversation_memory_embeddings (
                        conversation_id, message_id, chunk_index, start_byte, end_byte,
                        model_id, content_hash, vector
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        message.conversation_id,
                        message.message_id,
                        i64::try_from(chunk_index).map_err(|_| SemanticMemoryError::Index(
                            "message has too many semantic chunks".to_owned(),
                        ))?,
                        i64::try_from(chunk.start_byte).map_err(|_| SemanticMemoryError::Index(
                            "semantic chunk offset is too large".to_owned(),
                        ))?,
                        i64::try_from(chunk.end_byte).map_err(|_| SemanticMemoryError::Index(
                            "semantic chunk offset is too large".to_owned(),
                        ))?,
                        MODEL_ID,
                        content_hash,
                        vector_bytes,
                    ],
                )?;
                new_ids.push(transaction.last_insert_rowid());
            }
            transaction.execute(
                "INSERT INTO conversation_memory_embedding_state (
                    conversation_id, message_id, model_id, content_hash, chunk_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    message.conversation_id,
                    message.message_id,
                    MODEL_ID,
                    content_hash,
                    i64::try_from(chunks.len()).map_err(|_| SemanticMemoryError::Index(
                        "message has too many semantic chunks".to_owned(),
                    ))?,
                ],
            )?;
            transaction.execute(
                "DELETE FROM conversation_memory_embedding_pending
                 WHERE conversation_id = ?1 AND message_id = ?2",
                params![message.conversation_id, message.message_id],
            )?;
            transaction.commit()?;

            let index = self
                .index
                .as_ref()
                .ok_or_else(|| SemanticMemoryError::Index("index is not loaded".to_owned()))?;
            for old_id in old_ids {
                index
                    .remove(index_key(namespace, old_id)?)
                    .map_err(|error| SemanticMemoryError::Index(error.to_string()))?;
            }
            if !new_ids.is_empty() {
                let current_size = index.size();
                let required_capacity = current_size.saturating_add(new_ids.len());
                let geometric_capacity = current_size.saturating_mul(2).max(64);
                index
                    .reserve(required_capacity.max(geometric_capacity))
                    .map_err(|error| SemanticMemoryError::Index(error.to_string()))?;
            }
            for (id, vector) in new_ids.into_iter().zip(vectors.iter()) {
                index
                    .add(index_key(namespace, id)?, vector)
                    .map_err(|error| SemanticMemoryError::Index(error.to_string()))?;
            }
            self.index_generation = Some(semantic_index_generation(connection)?);
        }
        let generation = semantic_index_generation(connection)?;
        let index = self
            .index
            .as_ref()
            .ok_or_else(|| SemanticMemoryError::Index("index is not loaded".to_owned()))?;
        save_index(connection, cache_directory, index, generation)?;
        self.index_generation = Some(generation);
        Ok(())
    }
}

#[derive(Debug)]
struct MemoryMessage {
    conversation_id: String,
    message_id: String,
    content: String,
    tool_content: String,
    created_at: i64,
}

struct TextChunk {
    start_byte: usize,
    end_byte: usize,
    content: String,
}

fn open_database(path: &Path) -> Result<Connection, SemanticMemoryError> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(connection)
}

fn load_model(directory: &Path) -> Result<TextEmbedding, SemanticMemoryError> {
    let read = |name: &str| {
        fs::read(directory.join(name)).map_err(|error| SemanticMemoryError::Io(error.to_string()))
    };
    let model = UserDefinedEmbeddingModel::new(
        read("model_quantized.onnx")?,
        TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        },
    )
    .with_pooling(Pooling::Mean);
    TextEmbedding::try_new_from_user_defined(
        model,
        InitOptionsUserDefined::default()
            .with_max_length(MODEL_MAX_LENGTH)
            .with_intra_threads(2),
    )
    .map_err(|error| SemanticMemoryError::Model(error.to_string()))
}

fn new_index() -> Result<Index, SemanticMemoryError> {
    let options = IndexOptions {
        dimensions: VECTOR_DIMENSIONS,
        metric: MetricKind::Cos,
        quantization: ScalarKind::I8,
        connectivity: 16,
        expansion_add: 128,
        expansion_search: 64,
        multi: false,
    };
    Index::new(&options).map_err(|error| SemanticMemoryError::Index(error.to_string()))
}

fn chunk_text(
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

fn quantize_embedding(embedding: Vec<f32>) -> Result<Vec<i8>, SemanticMemoryError> {
    if embedding.len() != VECTOR_DIMENSIONS {
        return Err(SemanticMemoryError::Model(format!(
            "expected {VECTOR_DIMENSIONS} dimensions, received {}",
            embedding.len()
        )));
    }
    if embedding.iter().any(|value| !value.is_finite()) {
        return Err(SemanticMemoryError::Model(
            "embedding contains a non-finite value".to_owned(),
        ));
    }
    let max_abs = embedding
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f32, f32::max);
    if !max_abs.is_finite() || max_abs == 0.0 {
        return Err(SemanticMemoryError::Model(
            "embedding is not a finite non-zero vector".to_owned(),
        ));
    }
    Ok(embedding
        .iter()
        .map(|value| (value / max_abs * i8::MAX as f32).round() as i8)
        .collect())
}

fn ensure_namespace(
    connection: &Connection,
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

fn index_key(namespace: i64, id: i64) -> Result<u64, SemanticMemoryError> {
    if !(1..=16_777_215).contains(&namespace) || id <= 0 || id as u64 > INDEX_KEY_ID_MASK {
        return Err(SemanticMemoryError::Index(
            "conversation index key is outside its supported range".to_owned(),
        ));
    }
    Ok(((namespace as u64) << INDEX_KEY_ID_BITS) | id as u64)
}

fn pending_messages(
    connection: &Connection,
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

fn next_backfill_messages(
    connection: &Connection,
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

fn memory_message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryMessage> {
    Ok(MemoryMessage {
        conversation_id: row.get(0)?,
        message_id: row.get(1)?,
        content: row.get(2)?,
        tool_content: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn begin_embedding_write(
    connection: &Connection,
) -> Result<rusqlite::Transaction<'_>, SemanticMemoryError> {
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "UPDATE conversation_memory_semantic_index_state
         SET generation = generation WHERE id = 1",
        [],
    )?;
    Ok(transaction)
}

fn current_embedding_source(
    connection: &Connection,
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

fn embedding_content(message_content: &str, tool_content: &str) -> String {
    match (message_content.is_empty(), tool_content.is_empty()) {
        (false, false) => format!("{message_content}\n\n{tool_content}"),
        (false, true) => message_content.to_owned(),
        (true, false) => tool_content.to_owned(),
        (true, true) => String::new(),
    }
}

fn boundary_order(
    connection: &Connection,
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

fn semantic_excerpt(
    connection: &Connection,
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

fn semantic_index_generation(connection: &Connection) -> Result<i64, SemanticMemoryError> {
    connection
        .query_row(
            "SELECT generation FROM conversation_memory_semantic_index_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn set_indexed_generation(
    connection: &Connection,
    generation: i64,
) -> Result<(), SemanticMemoryError> {
    connection.execute(
        "UPDATE conversation_memory_semantic_index_state
         SET indexed_generation = ?1 WHERE id = 1",
        [generation],
    )?;
    Ok(())
}

fn save_index(
    connection: &Connection,
    cache_directory: &Path,
    index: &Index,
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
    index
        .save(&path_string(&temporary))
        .map_err(|error| SemanticMemoryError::Index(error.to_string()))?;
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    fs::rename(&temporary, &destination)?;
    set_indexed_generation(connection, generation)?;
    for entry in fs::read_dir(&index_directory)? {
        let path = entry?.path();
        if path.file_name() != destination.file_name()
            && path
                .extension()
                .is_some_and(|extension| extension == "usearch")
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn index_path(cache_directory: &Path, generation: i64) -> PathBuf {
    model_directory(cache_directory).join(format!("index-{generation}.usearch"))
}

fn model_directory(cache_directory: &Path) -> PathBuf {
    cache_directory.join(MODEL_ID)
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

async fn verify_file(path: PathBuf, asset: ModelFile) -> Result<bool, SemanticMemoryError> {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(SemanticMemoryError::Io(error.to_string())),
    };
    if file
        .metadata()
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?
        .len()
        != asset.size
    {
        return Ok(false);
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = file
            .read(&mut buffer)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        if length == 0 {
            break;
        }
        hasher.update(&buffer[..length]);
    }
    Ok(format!("{:x}", hasher.finalize()) == asset.sha256)
}

async fn download_file(
    destination: &Path,
    asset: ModelFile,
    completed_bytes: u64,
    total_bytes: u64,
    progress: &tokio_mpsc::UnboundedSender<serde_json::Value>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), SemanticMemoryError> {
    let url = format!(
        "{MODEL_DOWNLOAD_BASE_URL}/{MODEL_REPOSITORY}/resolve/{MODEL_REVISION}/{}?download=true",
        asset.path,
    );
    download_file_from_url(
        &url,
        destination,
        asset,
        completed_bytes,
        total_bytes,
        progress,
        cancellation,
    )
    .await
}

async fn download_file_from_url(
    url: &str,
    destination: &Path,
    asset: ModelFile,
    completed_bytes: u64,
    total_bytes: u64,
    progress: &tokio_mpsc::UnboundedSender<serde_json::Value>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), SemanticMemoryError> {
    if *cancellation.borrow() {
        return Err(SemanticMemoryError::Cancelled);
    }

    let temporary = destination.with_extension("partial");
    let mut resume_offset = match tokio::fs::metadata(&temporary).await {
        Ok(metadata) if metadata.len() <= asset.size => metadata.len(),
        Ok(_) => {
            tokio::fs::remove_file(&temporary)
                .await
                .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
            0
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(SemanticMemoryError::Io(error.to_string())),
    };
    if resume_offset == asset.size {
        if verify_file(temporary.clone(), asset).await? {
            if destination.exists() {
                fs::remove_file(destination)?;
            }
            tokio::fs::rename(&temporary, destination)
                .await
                .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
            report_progress(
                progress,
                "downloading",
                completed_bytes.saturating_add(asset.size),
                total_bytes,
                Some(asset.local_name),
            )?;
            return Ok(());
        }
        tokio::fs::remove_file(&temporary)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        resume_offset = 0;
    }

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|error| SemanticMemoryError::Http(error.to_string()))?;
    let mut request = client.get(url);
    if resume_offset > 0 {
        request = request.header(RANGE, format!("bytes={resume_offset}-"));
    }
    let response = request
        .send()
        .await
        .map_err(|error| SemanticMemoryError::Http(error.to_string()))?;
    let (response_offset, response_length) = match response.status() {
        StatusCode::OK => (0, asset.size),
        StatusCode::PARTIAL_CONTENT => {
            let content_range = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| parse_content_range(value, resume_offset, asset.size))
                .ok_or(SemanticMemoryError::Integrity)?;
            (resume_offset, content_range)
        }
        status => {
            return Err(SemanticMemoryError::Http(format!(
                "model host returned status {status}"
            )));
        }
    };
    if response
        .content_length()
        .is_some_and(|length| length != response_length)
    {
        return Err(SemanticMemoryError::Integrity);
    }

    let mut hasher = Sha256::new();
    if response_offset > 0 {
        let mut prefix = tokio::fs::File::open(&temporary)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        let mut hashed_bytes = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let length = prefix
                .read(&mut buffer)
                .await
                .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
            if length == 0 {
                break;
            }
            hashed_bytes = hashed_bytes.saturating_add(length as u64);
            hasher.update(&buffer[..length]);
        }
        if hashed_bytes != response_offset {
            tokio::fs::remove_file(&temporary)
                .await
                .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
            return Err(SemanticMemoryError::Integrity);
        }
    }

    let mut output_options = tokio::fs::OpenOptions::new();
    output_options.create(true).write(true);
    if response_offset > 0 {
        output_options.append(true);
    } else {
        output_options.truncate(true);
    }
    let mut output = output_options
        .open(&temporary)
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    report_progress(
        progress,
        "downloading",
        completed_bytes.saturating_add(response_offset),
        total_bytes,
        Some(asset.local_name),
    )?;
    let mut stream = response.bytes_stream();
    let mut received_bytes = 0_u64;
    let mut bytes_since_sync = 0_u64;
    let mut last_progress_at = Instant::now() - PROGRESS_MIN_INTERVAL;
    loop {
        if *cancellation.borrow() {
            persist_partial_download(&mut output).await?;
            return Err(SemanticMemoryError::Cancelled);
        }
        let next = tokio::select! {
            item = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, stream.next()) => item,
            changed = cancellation.changed() => {
                if changed.is_err() || *cancellation.borrow() {
                    persist_partial_download(&mut output).await?;
                    return Err(SemanticMemoryError::Cancelled);
                }
                continue;
            }
        };
        let chunk = match next {
            Ok(Some(Ok(chunk))) => chunk,
            Ok(Some(Err(error))) => {
                persist_partial_download(&mut output).await?;
                return Err(SemanticMemoryError::Http(error.to_string()));
            }
            Ok(None) => break,
            Err(_) => {
                persist_partial_download(&mut output).await?;
                return Err(SemanticMemoryError::Http(
                    "download stopped receiving data for 60 seconds".into(),
                ));
            }
        };
        received_bytes = received_bytes
            .checked_add(chunk.len() as u64)
            .filter(|size| *size <= response_length)
            .ok_or(SemanticMemoryError::Integrity)?;
        hasher.update(&chunk);
        output
            .write_all(&chunk)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        bytes_since_sync = bytes_since_sync.saturating_add(chunk.len() as u64);
        if bytes_since_sync >= PARTIAL_SYNC_INTERVAL_BYTES {
            persist_partial_download(&mut output).await?;
            bytes_since_sync = 0;
        }

        if received_bytes == response_length || last_progress_at.elapsed() >= PROGRESS_MIN_INTERVAL
        {
            if let Err(error) = report_progress(
                progress,
                "downloading",
                completed_bytes
                    .saturating_add(response_offset)
                    .saturating_add(received_bytes),
                total_bytes,
                Some(asset.local_name),
            ) {
                persist_partial_download(&mut output).await?;
                return Err(error);
            }
            last_progress_at = Instant::now();
        }
    }
    persist_partial_download(&mut output).await?;
    drop(output);
    if received_bytes != response_length
        || response_offset.saturating_add(received_bytes) != asset.size
    {
        return Err(SemanticMemoryError::Http(
            "model host ended the download before the file was complete".to_owned(),
        ));
    }
    if format!("{:x}", hasher.finalize()) != asset.sha256 {
        tokio::fs::remove_file(&temporary)
            .await
            .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
        return Err(SemanticMemoryError::Integrity);
    }
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    tokio::fs::rename(&temporary, destination)
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    Ok(())
}

async fn persist_partial_download(output: &mut tokio::fs::File) -> Result<(), SemanticMemoryError> {
    output
        .flush()
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    output
        .sync_data()
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))
}

fn parse_content_range(value: &str, expected_start: u64, expected_total: u64) -> Option<u64> {
    let (unit, range) = value.split_once(' ')?;
    if unit != "bytes" {
        return None;
    }
    let (positions, total) = range.split_once('/')?;
    if total.parse::<u64>().ok()? != expected_total {
        return None;
    }
    let (start, end) = positions.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = end.parse::<u64>().ok()?;
    if start != expected_start || end < start || end >= expected_total {
        return None;
    }
    Some(end - start + 1)
}

fn report_progress(
    progress: &tokio_mpsc::UnboundedSender<serde_json::Value>,
    phase: &str,
    downloaded_bytes: u64,
    total_bytes: u64,
    file_name: Option<&str>,
) -> Result<(), SemanticMemoryError> {
    let mut data = serde_json::json!({
        "phase": phase,
        "downloadedBytes": downloaded_bytes,
        "totalBytes": total_bytes,
    });
    if let Some(file_name) = file_name {
        data["fileName"] = serde_json::json!(file_name);
    }
    progress
        .send(data)
        .map_err(|_| SemanticMemoryError::Worker("semantic progress receiver closed".to_owned()))
}

struct IndexTemporaryFile(PathBuf);

impl Drop for IndexTemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::{mpsc as tokio_mpsc, watch},
    };

    use super::{
        INDEX_KEY_ID_BITS, ModelFile, SemanticMemoryError, download_file_from_url,
        embedding_content, index_key, new_index, next_backfill_messages, parse_content_range,
        pending_messages, quantize_embedding, semantic_excerpt,
    };

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-semantic-download-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).expect("create isolated download test directory");
            Self(path)
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn model_file_for(payload: &[u8]) -> ModelFile {
        use sha2::{Digest, Sha256};

        let sha256 = format!("{:x}", Sha256::digest(payload));
        ModelFile {
            path: "model.bin",
            local_name: "model.bin",
            size: payload.len() as u64,
            sha256: Box::leak(sha256.into_boxed_str()),
        }
    }

    async fn spawn_download_server(
        payload: Vec<u8>,
        ignore_range: bool,
        truncate_after: Option<usize>,
    ) -> (String, tokio::task::JoinHandle<Option<u64>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local model download test server");
        let address = listener
            .local_addr()
            .expect("read local model test server address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("accept local model download request");
            let mut request = Vec::new();
            loop {
                let mut buffer = [0_u8; 1024];
                let length = stream
                    .read(&mut buffer)
                    .await
                    .expect("read local model download request");
                if length == 0 {
                    return None;
                }
                request.extend_from_slice(&buffer[..length]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request);
            let requested_start = request.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("range")
                    .then(|| {
                        value
                            .trim()
                            .strip_prefix("bytes=")?
                            .strip_suffix('-')?
                            .parse()
                            .ok()
                    })
                    .flatten()
            });
            let resume_from = if ignore_range {
                0
            } else {
                requested_start.unwrap_or(0) as usize
            };
            let is_partial = requested_start.is_some() && !ignore_range;
            let response_body = &payload[resume_from..];
            let body_to_send = truncate_after
                .map(|length| response_body.len().min(length))
                .unwrap_or(response_body.len());
            let status = if is_partial {
                "206 Partial Content"
            } else {
                "200 OK"
            };
            let mut headers = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
                response_body.len()
            );
            if is_partial {
                headers.push_str(&format!(
                    "Content-Range: bytes {}-{}/{}\r\n",
                    resume_from,
                    payload.len() - 1,
                    payload.len()
                ));
            }
            headers.push_str("\r\n");
            stream
                .write_all(headers.as_bytes())
                .await
                .expect("write local model download headers");
            stream
                .write_all(&response_body[..body_to_send])
                .await
                .expect("write local model download body");
            requested_start
        });
        (format!("http://{address}/model"), server)
    }

    async fn download_for_test(
        url: &str,
        destination: &std::path::Path,
        asset: ModelFile,
        cancellation: &mut watch::Receiver<bool>,
        progress: &tokio_mpsc::UnboundedSender<serde_json::Value>,
    ) -> Result<(), SemanticMemoryError> {
        download_file_from_url(
            url,
            destination,
            asset,
            0,
            asset.size,
            progress,
            cancellation,
        )
        .await
    }

    #[tokio::test]
    async fn interrupted_model_download_resumes_from_the_saved_partial_file() {
        let payload = vec![b'x'; 512 * 1024];
        let asset = model_file_for(&payload);
        let temporary_directory = TemporaryDirectory::new();
        let destination = temporary_directory.0.join(asset.local_name);
        let (progress_sender, mut progress_receiver) = tokio_mpsc::unbounded_channel();
        let (_cancellation_sender, mut cancellation) = watch::channel(false);
        let (first_url, first_server) =
            spawn_download_server(payload.clone(), false, Some(128 * 1024)).await;

        let first_result = download_for_test(
            &first_url,
            &destination,
            asset,
            &mut cancellation,
            &progress_sender,
        )
        .await;
        assert!(matches!(first_result, Err(SemanticMemoryError::Http(_))));
        assert_eq!(first_server.await.expect("join interrupted download"), None);
        let partial_path = destination.with_extension("partial");
        let partial_size = fs::metadata(&partial_path)
            .expect("partial download remains after the connection ends")
            .len();
        assert_eq!(partial_size, 128 * 1024);

        let (resume_url, resume_server) = spawn_download_server(payload.clone(), false, None).await;
        download_for_test(
            &resume_url,
            &destination,
            asset,
            &mut cancellation,
            &progress_sender,
        )
        .await
        .expect("resume the model download");
        assert_eq!(
            resume_server.await.expect("join resumed download"),
            Some(partial_size)
        );
        assert_eq!(
            fs::read(&destination).expect("read verified model"),
            payload
        );
        let mut saw_complete_progress = false;
        while let Ok(data) = progress_receiver.try_recv() {
            if data["downloadedBytes"].as_u64() == Some(asset.size) {
                saw_complete_progress = true;
            }
        }
        assert!(saw_complete_progress);
    }

    #[tokio::test]
    async fn model_download_restarts_safely_when_host_ignores_range() {
        let payload = vec![b'y'; 256 * 1024];
        let asset = model_file_for(&payload);
        let temporary_directory = TemporaryDirectory::new();
        let destination = temporary_directory.0.join(asset.local_name);
        let partial_path = destination.with_extension("partial");
        fs::write(&partial_path, &payload[..64 * 1024]).expect("create partial model file");
        let (url, server) = spawn_download_server(payload.clone(), true, None).await;
        let (progress_sender, _progress_receiver) = tokio_mpsc::unbounded_channel();
        let (_cancellation_sender, mut cancellation) = watch::channel(false);

        download_for_test(
            &url,
            &destination,
            asset,
            &mut cancellation,
            &progress_sender,
        )
        .await
        .expect("restart from the beginning after a full response");

        assert_eq!(
            server.await.expect("join ignored-range download"),
            Some(64 * 1024)
        );
        assert_eq!(
            fs::read(destination).expect("read downloaded model"),
            payload
        );
    }

    #[test]
    fn content_range_must_match_the_requested_offset_and_pinned_file_size() {
        assert_eq!(parse_content_range("bytes 20-99/100", 20, 100), Some(80));
        assert_eq!(parse_content_range("bytes 19-99/100", 20, 100), None);
        assert_eq!(parse_content_range("bytes 20-99/101", 20, 100), None);
        assert_eq!(parse_content_range("bytes 20-100/100", 20, 100), None);
        assert_eq!(parse_content_range("items 20-99/100", 20, 100), None);
    }

    #[test]
    fn semantic_source_keeps_message_text_and_archived_tool_details_together() {
        assert_eq!(
            embedding_content("assistant summary", "read config.json feature flags"),
            "assistant summary\n\nread config.json feature flags"
        );
        assert_eq!(
            embedding_content("assistant summary", ""),
            "assistant summary"
        );
        assert_eq!(embedding_content("", "tool result"), "tool result");
        assert_eq!(embedding_content("", ""), "");
    }

    #[test]
    fn semantic_pending_and_backfill_queries_skip_excluded_conversations() {
        let connection = rusqlite::Connection::open_in_memory()
            .expect("open semantic archive preference fixture");
        connection
            .execute_batch(
                "CREATE TABLE messages (
                    conversation_id TEXT NOT NULL,
                    id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    PRIMARY KEY (conversation_id, id)
                );
                CREATE TABLE conversation_memory_embedding_pending (
                    conversation_id TEXT NOT NULL,
                    message_id TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, message_id)
                );
                CREATE TABLE conversation_memory_archive_settings (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    included INTEGER NOT NULL
                );
                CREATE TABLE conversation_memory_embedding_state (
                    conversation_id TEXT NOT NULL,
                    message_id TEXT NOT NULL,
                    model_id TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, message_id, model_id)
                );
                CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                    conversation_id UNINDEXED, message_id UNINDEXED, role UNINDEXED,
                    scope_token, content
                );
                INSERT INTO messages VALUES
                    ('excluded', 'pending', 'user', 'should not embed', 'completed', 1),
                    ('included', 'pending', 'user', 'can embed', 'completed', 1);
                INSERT INTO conversation_memory_archive_settings VALUES ('excluded', 0);
                INSERT INTO conversation_memory_embedding_pending VALUES ('excluded', 'pending');
                INSERT INTO conversation_memory_embedding_pending VALUES ('included', 'pending');",
            )
            .expect("create pending and backfill archive fixture");

        assert!(
            pending_messages(&connection, "excluded", None)
                .expect("read excluded pending messages")
                .is_empty()
        );
        assert_eq!(
            pending_messages(&connection, "included", None)
                .expect("read included pending messages")
                .len(),
            1
        );
        assert!(
            next_backfill_messages(&connection, "excluded", None, None, None)
                .expect("read excluded backfill messages")
                .is_empty()
        );
        assert_eq!(
            next_backfill_messages(&connection, "included", None, None, None)
                .expect("read included backfill messages")
                .len(),
            1
        );
    }

    #[test]
    fn semantic_archive_rebuilds_tool_excerpts_from_the_filtered_tool_index() {
        let connection =
            rusqlite::Connection::open_in_memory().expect("open in-memory semantic archive");
        connection
            .execute_batch(
                "CREATE TABLE messages (
                    conversation_id TEXT NOT NULL,
                    id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER,
                    PRIMARY KEY (conversation_id, id)
                );
                CREATE TABLE conversation_memory_embedding_pending (
                    conversation_id TEXT NOT NULL,
                    message_id TEXT NOT NULL,
                    PRIMARY KEY (conversation_id, message_id)
                );
                CREATE TABLE conversation_memory_archive_settings (
                    conversation_id TEXT PRIMARY KEY NOT NULL,
                    included INTEGER NOT NULL
                );
                CREATE TABLE conversation_memory_embeddings (
                    id INTEGER PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    message_id TEXT NOT NULL,
                    start_byte INTEGER NOT NULL,
                    end_byte INTEGER NOT NULL
                );
                CREATE VIRTUAL TABLE conversation_memory_tools_fts USING fts5(
                    conversation_id UNINDEXED,
                    message_id UNINDEXED,
                    role UNINDEXED,
                    scope_token,
                    content
                );
                INSERT INTO messages
                    (conversation_id, id, role, content, status, created_at)
                VALUES ('conversation', 'message', 'assistant', 'I checked the settings.', 'completed', 1);
                INSERT INTO conversation_memory_embedding_pending
                    (conversation_id, message_id) VALUES ('conversation', 'message');",
            )
            .expect("create semantic archive fixture");
        let message_rowid = connection
            .query_row(
                "SELECT rowid FROM messages WHERE conversation_id = 'conversation' AND id = 'message'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read semantic archive source rowid");
        let tool_content = "read settings.json semantic memory enabled";
        connection
            .execute(
                "INSERT INTO conversation_memory_tools_fts (
                    rowid, conversation_id, message_id, role, scope_token, content
                 ) VALUES (?1, 'conversation', 'message', 'assistant', 'scope', ?2)",
                rusqlite::params![message_rowid, tool_content],
            )
            .expect("insert filtered tool archive content");

        let pending = pending_messages(&connection, "conversation", None)
            .expect("read pending semantic archive message");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].tool_content, tool_content);

        let source = embedding_content(&pending[0].content, &pending[0].tool_content);
        let start_byte = source
            .find(tool_content)
            .expect("find tool archive excerpt") as i64;
        let end_byte = start_byte + tool_content.len() as i64;
        connection
            .execute(
                "INSERT INTO conversation_memory_embeddings (
                    conversation_id, message_id, start_byte, end_byte
                 ) VALUES ('conversation', 'message', ?1, ?2)",
                rusqlite::params![start_byte, end_byte],
            )
            .expect("insert tool excerpt offsets");

        let excerpt = semantic_excerpt(
            &connection,
            "conversation",
            connection.last_insert_rowid(),
            None,
        )
        .expect("read semantic tool excerpt")
        .expect("semantic tool excerpt exists");
        assert_eq!(excerpt.content, tool_content);
    }

    #[test]
    fn semantic_index_filters_hits_by_conversation_namespace() {
        let index = new_index().expect("create semantic test index");
        index.reserve(2).expect("reserve semantic test vectors");
        let mut conversation_vector = vec![0_i8; super::VECTOR_DIMENSIONS];
        conversation_vector[0] = 100;
        let mut other_vector = vec![0_i8; super::VECTOR_DIMENSIONS];
        other_vector[1] = 100;
        let conversation_key = index_key(3, 1).expect("encode current conversation key");
        index
            .add(conversation_key, &conversation_vector)
            .expect("add current conversation vector");
        index
            .add(
                index_key(4, 2).expect("encode other conversation key"),
                &other_vector,
            )
            .expect("add other conversation vector");

        let matches = index
            .filtered_search(&conversation_vector, 2, |key| key >> INDEX_KEY_ID_BITS == 3)
            .expect("search within current conversation namespace");

        assert_eq!(matches.keys, [conversation_key]);
    }

    #[test]
    fn quantization_scales_each_embedding_and_rejects_invalid_vectors() {
        let mut embedding = vec![0.0; super::VECTOR_DIMENSIONS];
        embedding[0] = 0.25;
        embedding[1] = -0.125;

        let quantized = quantize_embedding(embedding).expect("quantize finite embedding");

        assert_eq!(quantized[0], i8::MAX);
        assert_eq!(quantized[1], -64);
        assert!(matches!(
            quantize_embedding(vec![0.0; super::VECTOR_DIMENSIONS]),
            Err(SemanticMemoryError::Model(_))
        ));
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut invalid_embedding = vec![0.0; super::VECTOR_DIMENSIONS];
            invalid_embedding[super::VECTOR_DIMENSIONS - 1] = invalid;
            assert!(matches!(
                quantize_embedding(invalid_embedding),
                Err(SemanticMemoryError::Model(_))
            ));
        }
    }

    #[test]
    fn embedding_ids_are_packed_with_their_conversation_namespace() {
        let first = index_key(1, 9).expect("encode first key");
        let second = index_key(2, 9).expect("encode second key");

        assert_ne!(first, second);
        assert_eq!(first >> INDEX_KEY_ID_BITS, 1);
        assert!(index_key(16_777_216, 9).is_err());
        assert!(index_key(1, 0).is_err());
    }
}
