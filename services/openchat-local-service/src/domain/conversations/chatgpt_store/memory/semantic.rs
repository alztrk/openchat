//! Builds and queries a local vector index for archived conversation memories.
//!
//! Model downloads, index files, embedding sizes, and retrieval results are
//! bounded; cached runtime state is released when no longer needed.

use std::{
    collections::{BinaryHeap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    time::Duration,
};

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use futures_util::StreamExt;
use reqwest::{
    StatusCode,
    header::{CONTENT_RANGE, RANGE},
};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc as tokio_mpsc, watch},
    time::Instant,
};

use super::{ArchivedMemoryExcerpt, index, search, vector_math};
use crate::storage::{DatabaseQueueHandle, StorageConnection, StorageRow, StorageTransaction};

mod indexing;
mod model_download;

use indexing::*;
use model_download::*;

const MODEL_ID: &str = "multilingual-e5-small-614241f6";
const MODEL_REPOSITORY: &str = "intfloat/multilingual-e5-small";
const MODEL_REVISION: &str = "614241f622f53c4eeff9890bdc4f31cfecc418b3";
const MODEL_DOWNLOAD_BASE_URL: &str = "https://huggingface.co";
const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const RUNTIME_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
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
        path: "model.safetensors",
        local_name: "model.safetensors",
        size: 470_641_600,
        sha256: "1a55775f53449dac10a2bcbc312469fac40b96d53198c407081a831f81c98477",
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
        size: 655,
        sha256: "69137736cab8b8903a07fe8afaafdda25aac55415a12a55d1bffa9f581abf959",
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
        database: DatabaseQueueHandle,
        cache_directory: PathBuf,
        response: SyncSender<Result<(), SemanticMemoryError>>,
    },
    Search {
        database: DatabaseQueueHandle,
        cache_directory: PathBuf,
        conversation_id: String,
        through_message_id: Option<String>,
        query: String,
        result_limit: usize,
        backfill: bool,
        response: SyncSender<Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError>>,
    },
    InvalidateIndex {
        database: DatabaseQueueHandle,
        cache_directory: PathBuf,
        response: SyncSender<Result<(), SemanticMemoryError>>,
    },
}

struct ArchiveSearchRequest {
    database: DatabaseQueueHandle,
    cache_directory: PathBuf,
    conversation_id: String,
    through_message_id: Option<String>,
    query: String,
    result_limit: usize,
    backfill: bool,
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
    database: DatabaseQueueHandle,
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
    run_prepare_worker(database, cache_directory.clone()).await?;
    tokio::fs::write(directory.join(".ready"), MODEL_ID)
        .await
        .map_err(|error| SemanticMemoryError::Io(error.to_string()))?;
    report_progress(&progress, "ready", total_bytes, total_bytes, None)?;
    Ok(())
}

pub(super) async fn search_archive(
    database: DatabaseQueueHandle,
    cache_directory: PathBuf,
    conversation_id: String,
    through_message_id: Option<String>,
    query: String,
    result_limit: usize,
) -> Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError> {
    run_search_worker(
        database,
        cache_directory,
        conversation_id,
        through_message_id,
        query,
        result_limit,
        true,
    )
    .await
}

/// Searches only embeddings already stored for a conversation, without backfilling its history.
pub(super) async fn search_archive_existing(
    database: DatabaseQueueHandle,
    cache_directory: PathBuf,
    conversation_id: String,
    query: String,
    result_limit: usize,
) -> Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError> {
    run_search_worker(
        database,
        cache_directory,
        conversation_id,
        None,
        query,
        result_limit,
        false,
    )
    .await
}

async fn run_prepare_worker(
    database: DatabaseQueueHandle,
    cache_directory: PathBuf,
) -> Result<(), SemanticMemoryError> {
    tokio::task::spawn_blocking(move || {
        let sender = worker_sender()?;
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        sender
            .send(WorkerMessage::Prepare {
                database,
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
    database: DatabaseQueueHandle,
    cache_directory: PathBuf,
    conversation_id: String,
    through_message_id: Option<String>,
    query: String,
    result_limit: usize,
    backfill: bool,
) -> Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError> {
    tokio::task::spawn_blocking(move || {
        let sender = worker_sender()?;
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        sender
            .send(WorkerMessage::Search {
                database,
                cache_directory,
                conversation_id,
                through_message_id,
                query,
                result_limit,
                backfill,
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
    loop {
        let request = match receiver.recv_timeout(RUNTIME_IDLE_TIMEOUT) {
            Ok(request) => request,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                runtime.release_cached_resources();
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        match request {
            WorkerMessage::Prepare {
                database,
                cache_directory,
                response,
            } => {
                let result = runtime.prepare(&database, &cache_directory);
                let _ = response.send(result);
            }
            WorkerMessage::Search {
                database,
                cache_directory,
                conversation_id,
                through_message_id,
                query,
                result_limit,
                backfill,
                response,
            } => {
                let result = runtime.search(ArchiveSearchRequest {
                    database,
                    cache_directory,
                    conversation_id,
                    through_message_id,
                    query,
                    result_limit,
                    backfill,
                });
                let _ = response.send(result);
            }
            WorkerMessage::InvalidateIndex {
                database,
                cache_directory,
                response,
            } => {
                let result = runtime.invalidate_cached_index(&database, &cache_directory);
                let _ = response.send(result);
            }
        }
    }
}

pub(super) async fn invalidate_cached_index(
    database: DatabaseQueueHandle,
    cache_directory: PathBuf,
) -> Result<(), SemanticMemoryError> {
    tokio::task::spawn_blocking(move || {
        let sender = worker_sender()?;
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        sender
            .send(WorkerMessage::InvalidateIndex {
                database,
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

const VECTOR_INDEX_MAGIC: &[u8; 4] = b"OCVI";
const VECTOR_INDEX_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct VectorMatches {
    pub keys: Vec<u64>,
    pub distances: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct VectorIndex {
    dimensions: usize,
    vectors: HashMap<u64, Vec<i8>>,
}

#[derive(PartialEq)]
struct Candidate {
    distance: f32,
    key: u64,
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then_with(|| self.key.cmp(&other.key))
    }
}

impl VectorIndex {
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            vectors: HashMap::new(),
        }
    }

    pub fn size(&self) -> usize {
        self.vectors.len()
    }

    pub fn reserve(&mut self, capacity: usize) -> Result<(), SemanticMemoryError> {
        self.vectors.reserve(capacity);
        Ok(())
    }

    pub fn add(&mut self, key: u64, vector: &[i8]) -> Result<(), SemanticMemoryError> {
        if vector.len() != self.dimensions {
            return Err(SemanticMemoryError::Index(format!(
                "vector dimension mismatch: expected {}, got {}",
                self.dimensions,
                vector.len()
            )));
        }
        self.vectors.insert(key, vector.to_vec());
        Ok(())
    }

    pub fn remove(&mut self, key: u64) -> Result<(), SemanticMemoryError> {
        self.vectors.remove(&key);
        Ok(())
    }

    pub fn filtered_search<F>(
        &self,
        query: &[i8],
        candidate_count: usize,
        filter: F,
    ) -> Result<VectorMatches, SemanticMemoryError>
    where
        F: Fn(u64) -> bool,
    {
        if query.len() != self.dimensions {
            return Err(SemanticMemoryError::Index(format!(
                "query dimension mismatch: expected {}, got {}",
                self.dimensions,
                query.len()
            )));
        }
        if candidate_count == 0 || self.vectors.is_empty() {
            return Ok(VectorMatches {
                keys: Vec::new(),
                distances: Vec::new(),
            });
        }

        let mut heap = BinaryHeap::with_capacity(candidate_count + 1);

        for (&key, vector) in &self.vectors {
            if !filter(key) {
                continue;
            }
            let similarity = vector_math::cosine_similarity_i8(query, vector);
            let distance = 1.0_f32 - similarity;

            if heap.len() < candidate_count {
                heap.push(Candidate { distance, key });
            } else if let Some(worst) = heap.peek()
                && distance < worst.distance
            {
                heap.pop();
                heap.push(Candidate { distance, key });
            }
        }

        let sorted = heap.into_sorted_vec();
        let mut keys = Vec::with_capacity(sorted.len());
        let mut distances = Vec::with_capacity(sorted.len());
        for candidate in sorted {
            keys.push(candidate.key);
            distances.push(candidate.distance);
        }

        Ok(VectorMatches { keys, distances })
    }

    pub fn save(&self, path: &str) -> Result<(), SemanticMemoryError> {
        use std::io::Write;
        let file = fs::File::create(path).map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        let mut writer = std::io::BufWriter::new(file);

        writer
            .write_all(VECTOR_INDEX_MAGIC)
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        writer
            .write_all(&VECTOR_INDEX_VERSION.to_le_bytes())
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        writer
            .write_all(&(self.dimensions as u32).to_le_bytes())
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        writer
            .write_all(&(self.vectors.len() as u64).to_le_bytes())
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;

        let mut byte_buf = vec![0_u8; self.dimensions];
        for (&key, vector) in &self.vectors {
            writer
                .write_all(&key.to_le_bytes())
                .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
            for (dst, &src) in byte_buf.iter_mut().zip(vector.iter()) {
                *dst = src as u8;
            }
            writer
                .write_all(&byte_buf)
                .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        }

        writer
            .flush()
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        Ok(())
    }

    pub fn restore(path: &str) -> Result<Self, SemanticMemoryError> {
        use std::io::Read;
        let file = fs::File::open(path).map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        let file_len = file
            .metadata()
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?
            .len();
        if file_len < 20 {
            return Err(SemanticMemoryError::Index(
                "index file is too short to contain header".to_owned(),
            ));
        }

        let mut reader = std::io::BufReader::new(file);
        let mut magic = [0_u8; 4];
        reader
            .read_exact(&mut magic)
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        if &magic != VECTOR_INDEX_MAGIC {
            return Err(SemanticMemoryError::Index(
                "invalid vector index magic signature".to_owned(),
            ));
        }

        let mut u32_buf = [0_u8; 4];
        reader
            .read_exact(&mut u32_buf)
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        let version = u32::from_le_bytes(u32_buf);
        if version != VECTOR_INDEX_VERSION {
            return Err(SemanticMemoryError::Index(format!(
                "unsupported index version: {version}"
            )));
        }

        reader
            .read_exact(&mut u32_buf)
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        let dimensions = u32::from_le_bytes(u32_buf) as usize;
        if dimensions != VECTOR_DIMENSIONS {
            return Err(SemanticMemoryError::Index(format!(
                "index dimension mismatch: expected {VECTOR_DIMENSIONS}, got {dimensions}"
            )));
        }

        let mut u64_buf = [0_u8; 8];
        reader
            .read_exact(&mut u64_buf)
            .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
        let entry_count = u64::from_le_bytes(u64_buf) as usize;

        let entry_bytes = 8 + dimensions;
        let expected_bytes =
            20_u64.saturating_add((entry_count as u64).saturating_mul(entry_bytes as u64));
        if file_len < expected_bytes {
            return Err(SemanticMemoryError::Index(
                "index file is truncated".to_owned(),
            ));
        }

        let mut vectors = HashMap::with_capacity(entry_count);
        let mut key_buf = [0_u8; 8];
        let mut vec_buf = vec![0_u8; dimensions];

        for _ in 0..entry_count {
            reader
                .read_exact(&mut key_buf)
                .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
            let key = u64::from_le_bytes(key_buf);
            reader
                .read_exact(&mut vec_buf)
                .map_err(|e| SemanticMemoryError::Io(e.to_string()))?;
            let vector = vec_buf.iter().map(|&b| b as i8).collect();
            vectors.insert(key, vector);
        }

        Ok(Self {
            dimensions,
            vectors,
        })
    }
}

struct SemanticModel {
    model: BertModel,
    tokenizer: tokenizers::Tokenizer,
}

impl SemanticModel {
    fn embed<S: AsRef<str>>(
        &self,
        texts: &[S],
        batch_size: Option<usize>,
    ) -> Result<Vec<Vec<f32>>, SemanticMemoryError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let batch_size = batch_size.unwrap_or(32).max(1);
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(batch_size) {
            let chunk_strs: Vec<&str> = chunk.iter().map(|s| s.as_ref()).collect();
            let encodings = self
                .tokenizer
                .encode_batch(chunk_strs, true)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;

            let mut all_ids = Vec::with_capacity(encodings.len());
            let mut all_masks = Vec::with_capacity(encodings.len());
            for encoding in encodings {
                all_ids.push(encoding.get_ids().to_vec());
                all_masks.push(encoding.get_attention_mask().to_vec());
            }

            let input_ids = Tensor::new(all_ids, &Device::Cpu)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;
            let attention_mask = Tensor::new(all_masks, &Device::Cpu)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;
            let token_type_ids = input_ids
                .zeros_like()
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;

            let embeddings = self
                .model
                .forward(&input_ids, &token_type_ids, Some(&attention_mask))
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;

            // Mean pooling: sum(embeddings * mask) / sum(mask)
            let mask = attention_mask
                .unsqueeze(2)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?
                .to_dtype(DType::F32)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?
                .broadcast_as(embeddings.shape())
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;

            let masked_embeddings = embeddings
                .broadcast_mul(&mask)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;
            let sum = masked_embeddings
                .sum(1)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;
            let mask_sum = mask
                .sum(1)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?
                .clamp(1e-9, f64::INFINITY)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;
            let pooled = sum
                .broadcast_div(&mask_sum)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;

            // L2 normalize
            let norm = pooled
                .sqr()
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?
                .sum_keepdim(1)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?
                .sqrt()
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;
            let normalized = pooled
                .broadcast_div(&norm)
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;

            let batch_vectors = normalized
                .to_vec2::<f32>()
                .map_err(|e| SemanticMemoryError::Model(e.to_string()))?;
            all_embeddings.extend(batch_vectors);
        }

        Ok(all_embeddings)
    }
}

#[derive(Default)]
struct SemanticRuntime {
    model: Option<SemanticModel>,
    index: Option<VectorIndex>,
    model_directory: Option<PathBuf>,
    index_generation: Option<i64>,
}

impl SemanticRuntime {
    fn release_cached_resources(&mut self) {
        self.model = None;
        self.index = None;
        self.model_directory = None;
        self.index_generation = None;
    }

    fn prepare(
        &mut self,
        database: &DatabaseQueueHandle,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        let connection = database.connect()?;
        self.load_model_and_index(&connection, cache_directory)
    }

    fn search(
        &mut self,
        request: ArchiveSearchRequest,
    ) -> Result<Vec<ArchivedMemoryExcerpt>, SemanticMemoryError> {
        let ArchiveSearchRequest {
            database,
            cache_directory,
            conversation_id,
            through_message_id,
            query,
            result_limit,
            backfill,
        } = request;
        let mut connection = database.connect()?;
        if !index::archive_indexing_enabled(&connection, &conversation_id)? {
            return Ok(Vec::new());
        }
        self.load_model_and_index(&connection, &cache_directory)?;
        self.ensure_index_current(&connection, &cache_directory)?;
        let namespace = ensure_namespace(&connection, &conversation_id)?;
        if backfill {
            self.ensure_conversation_indexed(
                &mut connection,
                &conversation_id,
                through_message_id.as_deref(),
                namespace,
                &cache_directory,
            )?;
        }

        let model = self
            .model
            .as_ref()
            .ok_or_else(|| SemanticMemoryError::Model("model is not loaded".to_owned()))?;
        let query_text = format!("query: {query}");
        let query_embedding = model
            .embed(&[query_text.as_str()], Some(1))?
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
        let boundary =
            boundary_order(&connection, &conversation_id, through_message_id.as_deref())?;
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
                    semantic_excerpt(&connection, &conversation_id, id, boundary.as_ref())?
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
        if !index::archive_indexing_enabled(&connection, &conversation_id)? {
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
        connection: &StorageConnection<'_>,
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
        connection: &StorageConnection<'_>,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        let generation = semantic_index_generation(connection)?;
        if self.index_generation == Some(generation) && self.index.is_some() {
            return Ok(());
        }
        let index_path = index_path(cache_directory, generation);
        if self.index.is_none()
            && index_path.is_file()
            && let Ok(index) = VectorIndex::restore(&path_string(&index_path))
        {
            let directory = index_path.parent().ok_or_else(|| {
                SemanticMemoryError::Index("semantic index cache path is invalid".to_owned())
            })?;
            prune_cached_indexes(directory, Some(&index_path))?;
            set_indexed_generation(connection, generation)?;
            self.index = Some(index);
            self.index_generation = Some(generation);
            return Ok(());
        }

        let mut index = new_index()?;
        let vector_count = connection.query_row(
            "SELECT COUNT(*) FROM conversation_memory_embeddings WHERE model_id = ?1",
            [MODEL_ID],
            |row| row.get::<_, i64>(0),
        )?;
        let vector_count = usize::try_from(vector_count).map_err(|_| {
            SemanticMemoryError::Index("semantic vector count is out of range".to_owned())
        })?;
        if vector_count > 0 {
            index.reserve(vector_count)?;
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
            index.add(key, &vector)?;
        }
        save_index(connection, cache_directory, &index, generation)?;
        self.index = Some(index);
        self.index_generation = Some(generation);
        Ok(())
    }

    fn ensure_conversation_indexed(
        &mut self,
        connection: &mut StorageConnection<'_>,
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
        database: &DatabaseQueueHandle,
        cache_directory: &Path,
    ) -> Result<(), SemanticMemoryError> {
        let connection = database.connect()?;
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
        prune_cached_indexes(&directory, None)
    }

    fn embed_messages(
        &mut self,
        connection: &mut StorageConnection<'_>,
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
                .as_ref()
                .ok_or_else(|| SemanticMemoryError::Model("model is not loaded".to_owned()))?;
            let chunks = chunk_text(&model.tokenizer, &source_content)?;
            let chunk_texts = chunks
                .iter()
                .map(|chunk| format!("passage: {}", chunk.content))
                .collect::<Vec<_>>();
            let embeddings = if chunk_texts.is_empty() {
                Vec::new()
            } else {
                model.embed(&chunk_texts, Some(32))?
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
                new_ids.push(transaction.last_insert_rowid()?);
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
                .as_mut()
                .ok_or_else(|| SemanticMemoryError::Index("index is not loaded".to_owned()))?;
            for old_id in old_ids {
                index.remove(index_key(namespace, old_id)?)?;
            }
            if !new_ids.is_empty() {
                let current_size = index.size();
                let required_capacity = current_size.saturating_add(new_ids.len());
                let geometric_capacity = current_size.saturating_mul(2).max(64);
                index.reserve(required_capacity.max(geometric_capacity))?;
            }
            for (id, vector) in new_ids.into_iter().zip(vectors.iter()) {
                index.add(index_key(namespace, id)?, vector)?;
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

struct IndexTemporaryFile(PathBuf);

impl Drop for IndexTemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests;
