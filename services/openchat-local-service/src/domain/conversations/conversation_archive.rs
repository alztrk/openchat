//! Creates and restores encrypted, versioned archives of selected conversations and attachments.
//!
//! Archive entries, paths, metadata, password work factors, and decompressed sizes are bounded;
//! imports are validated before database or attachment state is committed.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Write},
    iter,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{SystemTime, UNIX_EPOCH},
};

use age::{Decryptor, Encryptor, scrypt::Identity, secrecy::SecretString};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, EntryType, Header};
use tokio::sync::watch;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    attachments::{
        AttachmentMetadata, message_attachment_metadata, validate_portable_attachment_content,
    },
    protocol::ServiceError,
    service::OperationGate,
    storage::{AppStorage, StorageConnection, StorageTransaction},
};

const ARCHIVE_FORMAT: &str = "openchat-conversations";
const ARCHIVE_VERSION: u32 = 1;
const ARCHIVE_EXTENSION: &str = "openchatbackup";
const MANIFEST_PATH: &str = "manifest.json";
const CONVERSATIONS_PATH: &str = "conversations.json";
const MAX_PASSPHRASE_BYTES: usize = 512;
const MIN_PASSPHRASE_CHARACTERS: usize = 12;
const MAX_CONVERSATION_IDS: usize = 5_000;
const MAX_CONVERSATIONS: usize = 5_000;
const MAX_MESSAGES: usize = 250_000;
const MAX_ARCHIVE_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_CONVERSATIONS_JSON_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_AGE_SCRYPT_LOG_N: u8 = 18;

static ACTIVE_RESTORE_STAGES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

type SharedArchiveOperationState = Arc<Mutex<ArchiveOperationState>>;

#[derive(Clone, Default)]
struct ArchiveOperationState {
    in_progress: bool,
    operation_id: Option<Value>,
    action: Option<ArchiveOperationKind>,
    progress: Option<ArchiveOperationProgress>,
}

#[derive(Clone, Copy)]
enum ArchiveOperationKind {
    Export,
    Inspect,
    Restore,
}

impl ArchiveOperationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Inspect => "inspect",
            Self::Restore => "restore",
        }
    }
}

#[derive(Clone)]
struct ArchiveOperationProgress {
    phase: &'static str,
    completed_items: usize,
    total_items: Option<usize>,
    completed_bytes: u64,
    total_bytes: Option<u64>,
}

struct ArchiveOperationLease {
    state: SharedArchiveOperationState,
    finished: bool,
}

fn active_restore_stages() -> &'static Mutex<HashSet<PathBuf>> {
    ACTIVE_RESTORE_STAGES.get_or_init(|| Mutex::new(HashSet::new()))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveManifest {
    format: String,
    version: u32,
    created_at_unix_ms: i64,
    conversation_count: usize,
    message_count: usize,
    attachments: Vec<ArchiveAttachmentRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveAttachmentRecord {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveData {
    conversations: Vec<ArchivedConversation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchivedConversation {
    id: String,
    title: String,
    title_source: String,
    provider_id: Option<String>,
    model_id: Option<String>,
    is_pinned: bool,
    #[serde(default)]
    is_archived: bool,
    created_at: i64,
    updated_at: i64,
    memory_settings: ArchivedMemorySettings,
    messages: Vec<ArchivedMessage>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchivedMemorySettings {
    included: Option<bool>,
    excluded_tools: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchivedMessage {
    id: String,
    role: String,
    content: String,
    created_at: Option<i64>,
    output_tokens: Option<i64>,
    tokens_per_second: Option<f64>,
    elapsed_microseconds: Option<i64>,
    reasoning_summaries: String,
    tool_activities: String,
    status: String,
    failure_code: Option<String>,
}

struct AttachmentSource {
    record: ArchiveAttachmentRecord,
    path: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConflictPolicy {
    SkipExisting,
    ImportAsCopy,
}

impl ConflictPolicy {
    fn parse(value: &str) -> Result<Self, ServiceError> {
        match value {
            "skip_existing" => Ok(Self::SkipExisting),
            "import_as_copy" => Ok(Self::ImportAsCopy),
            _ => Err(invalid_request()),
        }
    }
}

struct RestoreStage {
    root: PathBuf,
    directory: PathBuf,
    active: bool,
}

struct CreatedAttachmentFiles {
    paths: Vec<PathBuf>,
    committed: bool,
}

impl CreatedAttachmentFiles {
    fn new(capacity: usize) -> Self {
        Self {
            paths: Vec::with_capacity(capacity),
            committed: false,
        }
    }

    fn push(&mut self, path: PathBuf) {
        self.paths.push(path);
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for CreatedAttachmentFiles {
    fn drop(&mut self) {
        if !self.committed {
            cleanup_created_files(&self.paths);
        }
    }
}

impl RestoreStage {
    fn create(storage: &AppStorage) -> Result<Self, ArchiveFailure> {
        let mut active_stages = active_restore_stages()
            .lock()
            .map_err(|_| ArchiveFailure::Storage)?;
        let canonical_root = restore_stage_root(storage)?;
        cleanup_abandoned_restore_stages(&canonical_root, &active_stages)?;
        let directory = canonical_root.join(format!("restore-{}", Uuid::new_v4().simple()));
        fs::create_dir(&directory).map_err(|_| ArchiveFailure::Storage)?;
        let canonical_directory =
            fs::canonicalize(&directory).map_err(|_| ArchiveFailure::Storage)?;
        if canonical_directory.parent() != Some(canonical_root.as_path()) {
            return Err(ArchiveFailure::Storage);
        }
        active_stages.insert(canonical_directory.clone());
        drop(active_stages);
        Ok(Self {
            root: canonical_root,
            directory: canonical_directory,
            active: true,
        })
    }

    fn file_path(&self, archive_path: &str) -> Result<PathBuf, ArchiveFailure> {
        let relative = Path::new(archive_path);
        if relative.is_absolute()
            || relative.components().count() != 4
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(ArchiveFailure::InvalidArchive);
        }
        let destination = self.directory.join(relative);
        let parent = destination.parent().ok_or(ArchiveFailure::InvalidArchive)?;
        fs::create_dir_all(parent).map_err(|_| ArchiveFailure::Storage)?;
        let canonical_parent = fs::canonicalize(parent).map_err(|_| ArchiveFailure::Storage)?;
        if !canonical_parent.starts_with(&self.directory) {
            return Err(ArchiveFailure::InvalidArchive);
        }
        Ok(destination)
    }

    fn cleanup(mut self) -> Result<(), ArchiveFailure> {
        fs::remove_dir_all(&self.directory).map_err(|_| ArchiveFailure::Storage)?;
        self.active = false;
        self.unregister();
        Ok(())
    }

    fn unregister(&self) {
        let Some(active_stages) = ACTIVE_RESTORE_STAGES.get() else {
            return;
        };
        if let Ok(mut active_stages) = active_stages.lock() {
            active_stages.remove(&self.directory);
        }
    }
}

impl Drop for RestoreStage {
    fn drop(&mut self) {
        if self.active
            && self.directory.parent() == Some(self.root.as_path())
            && self
                .directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("restore-"))
        {
            // The stage contains only files created by this archive operation.
            let _ = fs::remove_dir_all(&self.directory);
        }
        self.unregister();
    }
}

fn cleanup_abandoned_restore_stages(
    root: &Path,
    active_stages: &HashSet<PathBuf>,
) -> Result<(), ArchiveFailure> {
    for entry in fs::read_dir(root).map_err(|_| ArchiveFailure::Storage)? {
        let entry = entry.map_err(|_| ArchiveFailure::Storage)?;
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.starts_with("restore-") || active_stages.contains(&path) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| ArchiveFailure::Storage)?;
        if metadata.file_type().is_symlink() {
            return Err(ArchiveFailure::Storage);
        }
        if !metadata.is_dir() {
            continue;
        }
        let canonical_path = fs::canonicalize(&path).map_err(|_| ArchiveFailure::Storage)?;
        if canonical_path.parent() != Some(root) {
            return Err(ArchiveFailure::Storage);
        }
        fs::remove_dir_all(canonical_path).map_err(|_| ArchiveFailure::Storage)?;
    }
    Ok(())
}

fn restore_stage_root(storage: &AppStorage) -> Result<PathBuf, ArchiveFailure> {
    let canonical_storage =
        fs::canonicalize(storage.root()).map_err(|_| ArchiveFailure::Storage)?;
    let cache = ensure_archive_directory(&canonical_storage.join("cache"), &canonical_storage)?;
    ensure_archive_directory(&cache.join("conversation-archives"), &canonical_storage)
}

fn ensure_archive_directory(path: &Path, storage_root: &Path) -> Result<PathBuf, ArchiveFailure> {
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::create_dir(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ArchiveFailure::Storage),
        },
        Err(_) => return Err(ArchiveFailure::Storage),
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| ArchiveFailure::Storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ArchiveFailure::Storage);
    }
    let canonical_path = fs::canonicalize(path).map_err(|_| ArchiveFailure::Storage)?;
    if !canonical_path.starts_with(storage_root) {
        return Err(ArchiveFailure::Storage);
    }
    Ok(canonical_path)
}

/// Removes staging directories left behind by interrupted conversation archive restores.
///
/// # Errors
/// Returns `conversation_archive_storage_unavailable` if the staging root cannot be validated or
/// an abandoned stage cannot be removed.
pub(crate) fn recover_interrupted_restore_stages(storage: &AppStorage) -> Result<(), ServiceError> {
    let active_stages = active_restore_stages()
        .lock()
        .map_err(|_| service_error(ArchiveFailure::Storage))?;
    let root = restore_stage_root(storage).map_err(service_error)?;
    cleanup_abandoned_restore_stages(&root, &active_stages).map_err(service_error)
}

#[derive(Debug, Clone, Copy)]
enum ArchiveFailure {
    InvalidRequest,
    InvalidArchive,
    InvalidPassphrase,
    NotFound,
    Conflict,
    Busy,
    OperationBusy,
    Cancelled,
    Storage,
    LimitExceeded,
}

impl From<rusqlite::Error> for ArchiveFailure {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}

fn service_error(failure: ArchiveFailure) -> ServiceError {
    match failure {
        ArchiveFailure::InvalidRequest => invalid_request(),
        ArchiveFailure::InvalidArchive => ServiceError::new(
            "conversation_archive_invalid",
            "The encrypted conversation archive is invalid or its passphrase is incorrect.",
            false,
        ),
        ArchiveFailure::InvalidPassphrase => ServiceError::new(
            "conversation_archive_passphrase_invalid",
            "Use a passphrase with at least 12 characters and no more than 512 bytes.",
            false,
        ),
        ArchiveFailure::NotFound => ServiceError::new(
            "conversation_archive_not_found",
            "The selected archive or conversation was not found.",
            false,
        ),
        ArchiveFailure::Conflict => ServiceError::new(
            "conversation_archive_conflict",
            "The destination file or attachment already exists.",
            false,
        ),
        ArchiveFailure::Busy => ServiceError::new(
            "conversation_archive_busy",
            "A conversation has an active assistant run. Finish or stop it before creating an archive.",
            true,
        ),
        ArchiveFailure::OperationBusy => ServiceError::new(
            "conversation_archive_operation_busy",
            "A conversation archive operation is already in progress.",
            true,
        ),
        ArchiveFailure::Cancelled => ServiceError::new(
            "operation_cancelled",
            "The conversation archive operation was cancelled.",
            false,
        ),
        ArchiveFailure::Storage => ServiceError::new(
            "conversation_archive_storage_failed",
            "The archive could not be read, written, or safely restored.",
            true,
        ),
        ArchiveFailure::LimitExceeded => ServiceError::new(
            "conversation_archive_limit_exceeded",
            "The archive exceeds a supported size or item limit.",
            false,
        ),
    }
}

pub(crate) fn status(storage: &AppStorage) -> Result<Value, ServiceError> {
    let operation = operation_snapshot(storage).map_err(service_error)?;
    Ok(json!({
        "operationInProgress": operation.in_progress,
        "operationId": operation.operation_id,
        "action": operation.action.map(ArchiveOperationKind::as_str),
        "progress": operation.progress.map(|progress| json!({
            "phase": progress.phase,
            "completedItems": progress.completed_items,
            "totalItems": progress.total_items,
            "completedBytes": progress.completed_bytes,
            "totalBytes": progress.total_bytes,
        })),
    }))
}

fn archive_operation_state(
    storage: &AppStorage,
) -> Result<SharedArchiveOperationState, ArchiveFailure> {
    static STATES: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<ArchiveOperationState>>>>> =
        OnceLock::new();
    let states = STATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut states = states.lock().map_err(|_| ArchiveFailure::Storage)?;
    states.retain(|_, state| state.strong_count() > 0);
    if let Some(state) = states.get(storage.root()).and_then(Weak::upgrade) {
        return Ok(state);
    }
    let state = Arc::new(Mutex::new(ArchiveOperationState::default()));
    states.insert(storage.root().to_path_buf(), Arc::downgrade(&state));
    Ok(state)
}

fn operation_snapshot(storage: &AppStorage) -> Result<ArchiveOperationState, ArchiveFailure> {
    archive_operation_state(storage)?
        .lock()
        .map(|state| state.clone())
        .map_err(|_| ArchiveFailure::Storage)
}

fn begin_operation(
    storage: &AppStorage,
    action: ArchiveOperationKind,
    operation_id: &Value,
) -> Result<ArchiveOperationLease, ArchiveFailure> {
    let state = archive_operation_state(storage)?;
    let mut snapshot = state.lock().map_err(|_| ArchiveFailure::Storage)?;
    if snapshot.in_progress {
        return Err(ArchiveFailure::OperationBusy);
    }
    snapshot.in_progress = true;
    snapshot.operation_id = Some(operation_id.clone());
    snapshot.action = Some(action);
    snapshot.progress = None;
    drop(snapshot);
    Ok(ArchiveOperationLease {
        state,
        finished: false,
    })
}

impl ArchiveOperationLease {
    fn set_progress(
        &self,
        phase: &'static str,
        completed_items: usize,
        total_items: Option<usize>,
        completed_bytes: u64,
        total_bytes: Option<u64>,
    ) -> Result<(), ArchiveFailure> {
        let mut state = self.state.lock().map_err(|_| ArchiveFailure::Storage)?;
        state.progress = Some(ArchiveOperationProgress {
            phase,
            completed_items,
            total_items,
            completed_bytes,
            total_bytes,
        });
        Ok(())
    }

    fn finish(mut self) {
        self.release();
        self.finished = true;
    }

    fn release(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *state = ArchiveOperationState::default();
    }
}

impl Drop for ArchiveOperationLease {
    fn drop(&mut self) {
        if !self.finished {
            self.release();
        }
    }
}

fn check_cancelled(cancellation: &watch::Receiver<bool>) -> Result<(), ArchiveFailure> {
    if *cancellation.borrow() {
        Err(ArchiveFailure::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) fn export(
    storage: &AppStorage,
    operation_id: &Value,
    conversation_ids: &[String],
    passphrase: Zeroizing<String>,
    cancellation: &watch::Receiver<bool>,
    cancellation_gate: &OperationGate,
) -> Result<Value, ServiceError> {
    validate_passphrase(&passphrase).map_err(service_error)?;
    check_cancelled(cancellation).map_err(service_error)?;
    let lease = begin_operation(storage, ArchiveOperationKind::Export, operation_id)
        .map_err(service_error)?;
    let pending = crate::pending_archive_export::PendingArchiveExport::begin(
        storage.root(),
        ARCHIVE_EXTENSION,
    )?;
    let result = export_inner(
        storage,
        conversation_ids,
        pending.archive_path(),
        passphrase.as_str(),
        cancellation,
        &lease,
    )
    .map_err(service_error)?;
    check_cancelled(cancellation).map_err(service_error)?;
    if !cancellation_gate.try_begin_commit() {
        return Err(service_error(ArchiveFailure::Cancelled));
    }
    let result = pending.finish(result)?;
    lease.finish();
    Ok(result)
}

pub(crate) fn inspect(
    storage: &AppStorage,
    operation_id: &Value,
    path: &str,
    passphrase: Zeroizing<String>,
    cancellation: &watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    validate_passphrase(&passphrase).map_err(service_error)?;
    check_cancelled(cancellation).map_err(service_error)?;
    let lease = begin_operation(storage, ArchiveOperationKind::Inspect, operation_id)
        .map_err(service_error)?;
    let (manifest, data) = read_archive(
        Path::new(path),
        passphrase.as_str(),
        None,
        cancellation,
        &lease,
    )
    .map_err(service_error)?;
    check_cancelled(cancellation).map_err(service_error)?;
    let connection = storage
        .connect()
        .map_err(|_| service_error(ArchiveFailure::Storage))?;
    let duplicates =
        count_existing_conversations(&connection, &data.conversations, cancellation, &lease)
            .map_err(service_error)?;
    let result = archive_summary(&manifest, duplicates);
    lease.finish();
    Ok(result)
}

pub(crate) fn restore(
    storage: &AppStorage,
    operation_id: &Value,
    path: &str,
    passphrase: Zeroizing<String>,
    conflict_policy: &str,
    cancellation: &watch::Receiver<bool>,
    cancellation_gate: &OperationGate,
    pre_restore_cleanup: impl FnOnce() -> Result<(), ServiceError>,
) -> Result<Value, ServiceError> {
    validate_passphrase(&passphrase).map_err(service_error)?;
    let conflict_policy = ConflictPolicy::parse(conflict_policy)?;
    check_cancelled(cancellation).map_err(service_error)?;
    let lease = begin_operation(storage, ArchiveOperationKind::Restore, operation_id)
        .map_err(service_error)?;
    pre_restore_cleanup()?;
    check_cancelled(cancellation).map_err(service_error)?;
    let stage = RestoreStage::create(storage).map_err(service_error)?;
    let (manifest, data) = read_archive(
        Path::new(path),
        passphrase.as_str(),
        Some(&stage),
        cancellation,
        &lease,
    )
    .map_err(service_error)?;
    check_cancelled(cancellation).map_err(service_error)?;
    let mut connection = storage
        .connect()
        .map_err(|_| service_error(ArchiveFailure::Storage))?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| service_error(ArchiveFailure::Storage))?;
    let (result, mut created_files) = restore_transaction(
        &transaction,
        storage.root(),
        &stage,
        &manifest,
        data,
        conflict_policy,
        cancellation,
        &lease,
    )
    .map_err(service_error)?;
    check_cancelled(cancellation).map_err(service_error)?;
    stage.cleanup().map_err(service_error)?;
    check_cancelled(cancellation).map_err(service_error)?;
    if !cancellation_gate.try_begin_commit() {
        return Err(service_error(ArchiveFailure::Cancelled));
    }
    transaction
        .commit()
        .map_err(|_| service_error(ArchiveFailure::Storage))?;
    created_files.commit();
    lease.finish();
    Ok(result)
}

fn export_inner(
    storage: &AppStorage,
    conversation_ids: &[String],
    target: &Path,
    passphrase: &str,
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
) -> Result<Value, ArchiveFailure> {
    check_cancelled(cancellation)?;
    let target = validate_output_path(target)?;
    let (data, sources) = load_conversations(storage, conversation_ids, cancellation, lease)?;
    check_cancelled(cancellation)?;
    let message_count = data
        .conversations
        .iter()
        .map(|conversation| conversation.messages.len())
        .sum::<usize>();
    let attachment_bytes = sources
        .iter()
        .try_fold(0_u64, |total, source| {
            total.checked_add(source.record.size_bytes)
        })
        .ok_or(ArchiveFailure::LimitExceeded)?;
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(ArchiveFailure::LimitExceeded);
    }

    let manifest = ArchiveManifest {
        format: ARCHIVE_FORMAT.to_owned(),
        version: ARCHIVE_VERSION,
        created_at_unix_ms: unix_time_millis().map_err(|_| ArchiveFailure::Storage)?,
        conversation_count: data.conversations.len(),
        message_count,
        attachments: sources.iter().map(|source| source.record.clone()).collect(),
    };
    validate_archive_data(&manifest, &data)?;
    lease.set_progress("verifying", 0, None, 0, None)?;
    check_cancelled(cancellation)?;
    let manifest_json = serde_json::to_vec(&manifest).map_err(|_| ArchiveFailure::Storage)?;
    let conversations_json = serde_json::to_vec(&data).map_err(|_| ArchiveFailure::Storage)?;
    if manifest_json.len() as u64 > MAX_MANIFEST_BYTES
        || conversations_json.len() as u64 > MAX_CONVERSATIONS_JSON_BYTES
    {
        return Err(ArchiveFailure::LimitExceeded);
    }

    let partial_path = target.with_file_name(format!(
        ".{}.{}.partial",
        target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ArchiveFailure::InvalidRequest)?,
        Uuid::new_v4().simple()
    ));
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial_path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                ArchiveFailure::Conflict
            } else {
                ArchiveFailure::Storage
            }
        })?;
    let mut partial = PartialOutput::new(partial_path);
    write_archive(
        output,
        &manifest_json,
        &conversations_json,
        &sources,
        passphrase,
        cancellation,
        lease,
    )?;
    check_cancelled(cancellation)?;
    fs::rename(partial.path(), &target).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            ArchiveFailure::Conflict
        } else {
            ArchiveFailure::Storage
        }
    })?;
    partial.disarm();
    Ok(json!({
        "conversationCount": manifest.conversation_count,
        "messageCount": manifest.message_count,
        "attachmentCount": manifest.attachments.len(),
        "attachmentBytes": attachment_bytes,
    }))
}

struct PartialOutput {
    path: PathBuf,
    active: bool,
}

impl PartialOutput {
    fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for PartialOutput {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn write_archive(
    output: File,
    manifest_json: &[u8],
    conversations_json: &[u8],
    sources: &[AttachmentSource],
    passphrase: &str,
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
) -> Result<(), ArchiveFailure> {
    check_cancelled(cancellation)?;
    let secret = SecretString::from(passphrase.to_owned());
    let encrypted = Encryptor::with_user_passphrase(secret)
        .wrap_output(output)
        .map_err(|_| ArchiveFailure::Storage)?;
    let mut archive = Builder::new(encrypted);
    let attachment_bytes = sources
        .iter()
        .try_fold(0_u64, |total, source| {
            total.checked_add(source.record.size_bytes)
        })
        .ok_or(ArchiveFailure::LimitExceeded)?;
    let total_bytes = u64::try_from(manifest_json.len())
        .ok()
        .and_then(|manifest| {
            u64::try_from(conversations_json.len())
                .ok()
                .and_then(|conversations| manifest.checked_add(conversations))
        })
        .and_then(|bytes| bytes.checked_add(attachment_bytes))
        .ok_or(ArchiveFailure::LimitExceeded)?;
    let total_items = sources.len().saturating_add(2);
    let mut completed_bytes = 0_u64;
    lease.set_progress("writing", 0, Some(total_items), 0, Some(total_bytes))?;
    append_bytes(
        &mut archive,
        MANIFEST_PATH,
        manifest_json,
        cancellation,
        lease,
        0,
        total_items,
        completed_bytes,
        total_bytes,
    )?;
    completed_bytes = completed_bytes.saturating_add(manifest_json.len() as u64);
    append_bytes(
        &mut archive,
        CONVERSATIONS_PATH,
        conversations_json,
        cancellation,
        lease,
        1,
        total_items,
        completed_bytes,
        total_bytes,
    )?;
    completed_bytes = completed_bytes.saturating_add(conversations_json.len() as u64);
    for (index, source) in sources.iter().enumerate() {
        check_cancelled(cancellation)?;
        let file = File::open(&source.path).map_err(|_| ArchiveFailure::Storage)?;
        let cancellable_reader = CancellableReader::new(
            BufReader::new(file),
            cancellation,
            lease,
            index + 2,
            total_items,
            completed_bytes,
            total_bytes,
        );
        let mut reader = HashingReader::new(cancellable_reader);
        append_reader(
            &mut archive,
            &source.record.path,
            source.record.size_bytes,
            &mut reader,
            cancellation,
        )?;
        check_cancelled(cancellation)?;
        if reader.bytes_read != source.record.size_bytes
            || digest_hex(&reader.hasher.finalize()) != source.record.sha256
        {
            return Err(ArchiveFailure::Storage);
        }
        completed_bytes = completed_bytes.saturating_add(source.record.size_bytes);
        lease.set_progress(
            "writing",
            index + 3,
            Some(total_items),
            completed_bytes,
            Some(total_bytes),
        )?;
    }
    check_cancelled(cancellation)?;
    let encrypted = archive.into_inner().map_err(|_| ArchiveFailure::Storage)?;
    let output = encrypted.finish().map_err(|_| ArchiveFailure::Storage)?;
    output.sync_all().map_err(|_| ArchiveFailure::Storage)
}

fn append_bytes<W: Write>(
    archive: &mut Builder<W>,
    path: &str,
    content: &[u8],
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
    completed_items: usize,
    total_items: usize,
    completed_bytes: u64,
    total_bytes: u64,
) -> Result<(), ArchiveFailure> {
    let mut reader = CancellableReader::new(
        content,
        cancellation,
        lease,
        completed_items,
        total_items,
        completed_bytes,
        total_bytes,
    );
    append_reader(
        archive,
        path,
        u64::try_from(content.len()).map_err(|_| ArchiveFailure::LimitExceeded)?,
        &mut reader,
        cancellation,
    )?;
    lease.set_progress(
        "writing",
        completed_items + 1,
        Some(total_items),
        completed_bytes.saturating_add(content.len() as u64),
        Some(total_bytes),
    )
}

fn append_reader<W: Write, R: Read>(
    archive: &mut Builder<W>,
    path: &str,
    size: u64,
    reader: &mut R,
    cancellation: &watch::Receiver<bool>,
) -> Result<(), ArchiveFailure> {
    check_cancelled(cancellation)?;
    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Regular);
    header.set_size(size);
    header.set_mode(0o600);
    header.set_cksum();
    if archive.append_data(&mut header, path, reader).is_err() {
        return Err(if *cancellation.borrow() {
            ArchiveFailure::Cancelled
        } else {
            ArchiveFailure::Storage
        });
    }
    check_cancelled(cancellation)
}

struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
    bytes_read: u64,
}

struct CancellableReader<'a, R> {
    inner: R,
    cancellation: &'a watch::Receiver<bool>,
    lease: &'a ArchiveOperationLease,
    completed_items: usize,
    total_items: usize,
    completed_bytes: u64,
    total_bytes: u64,
    bytes_read: u64,
    last_reported_bytes: u64,
}

impl<'a, R> CancellableReader<'a, R> {
    fn new(
        inner: R,
        cancellation: &'a watch::Receiver<bool>,
        lease: &'a ArchiveOperationLease,
        completed_items: usize,
        total_items: usize,
        completed_bytes: u64,
        total_bytes: u64,
    ) -> Self {
        Self {
            inner,
            cancellation,
            lease,
            completed_items,
            total_items,
            completed_bytes,
            total_bytes,
            bytes_read: 0,
            last_reported_bytes: 0,
        }
    }
}

impl<R: Read> Read for CancellableReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if *self.cancellation.borrow() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "archive operation cancelled",
            ));
        }
        let count = self.inner.read(buffer)?;
        self.bytes_read = self.bytes_read.saturating_add(count as u64);
        if count > 0 && self.bytes_read.saturating_sub(self.last_reported_bytes) >= 1024 * 1024 {
            self.lease
                .set_progress(
                    "writing",
                    self.completed_items,
                    Some(self.total_items),
                    self.completed_bytes.saturating_add(self.bytes_read),
                    Some(self.total_bytes),
                )
                .map_err(|_| io::Error::other("archive operation status is unavailable"))?;
            self.last_reported_bytes = self.bytes_read;
        }
        Ok(count)
    }
}

impl<R> HashingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes_read: 0,
        }
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..bytes_read]);
        self.bytes_read = self.bytes_read.saturating_add(bytes_read as u64);
        Ok(bytes_read)
    }
}

fn load_conversations(
    storage: &AppStorage,
    conversation_ids: &[String],
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
) -> Result<(ArchiveData, Vec<AttachmentSource>), ArchiveFailure> {
    check_cancelled(cancellation)?;
    if conversation_ids.is_empty() || conversation_ids.len() > MAX_CONVERSATION_IDS {
        return Err(ArchiveFailure::InvalidRequest);
    }
    let mut unique_ids = HashSet::with_capacity(conversation_ids.len());
    if conversation_ids
        .iter()
        .any(|id| !safe_identifier(id) || !unique_ids.insert(id))
    {
        return Err(ArchiveFailure::InvalidRequest);
    }
    let mut connection = storage.connect()?;
    let transaction = connection.transaction()?;
    let mut conversations = Vec::with_capacity(conversation_ids.len());
    let mut sources = Vec::new();
    let mut message_count = 0_usize;
    lease.set_progress("reading", 0, None, 0, None)?;
    for conversation_id in conversation_ids {
        check_cancelled(cancellation)?;
        let active_run = transaction.query_row(
            "SELECT EXISTS (
                SELECT 1 FROM agent_runs
                WHERE conversation_id = ?1 AND status IN ('running', 'paused')
            )",
            [conversation_id],
            |row| row.get::<_, bool>(0),
        )?;
        if active_run {
            return Err(ArchiveFailure::Busy);
        }
        let conversation = transaction
            .query_row(
                "SELECT id, title, title_source, provider_id, model_id, is_pinned,
                        is_archived,
                        created_at, updated_at
                 FROM conversations WHERE id = ?1",
                [conversation_id],
                |row| {
                    Ok(ArchivedConversation {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        title_source: row.get(2)?,
                        provider_id: row.get(3)?,
                        model_id: row.get(4)?,
                        is_pinned: row.get::<_, i64>(5)? != 0,
                        is_archived: row.get::<_, i64>(6)? != 0,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        memory_settings: ArchivedMemorySettings::default(),
                        messages: Vec::new(),
                    })
                },
            )
            .optional()?
            .ok_or(ArchiveFailure::NotFound)?;
        let mut conversation = conversation;
        conversation.memory_settings = read_memory_settings(&transaction, conversation_id)?;
        let mut statement = transaction.prepare(
            "SELECT id, role, content, created_at, output_tokens, tokens_per_second,
                    elapsed_microseconds, reasoning_summaries, tool_activities, status, failure_code
             FROM messages WHERE conversation_id = ?1 ORDER BY created_at, id",
        )?;
        let mut rows = statement.query([conversation_id])?;
        while let Some(row) = rows.next()? {
            check_cancelled(cancellation)?;
            message_count = message_count.saturating_add(1);
            if message_count > MAX_MESSAGES {
                return Err(ArchiveFailure::LimitExceeded);
            }
            let message = ArchivedMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                output_tokens: row.get(4)?,
                tokens_per_second: row.get(5)?,
                elapsed_microseconds: row.get(6)?,
                reasoning_summaries: row.get(7)?,
                tool_activities: row.get(8)?,
                status: row.get(9)?,
                failure_code: row.get(10)?,
            };
            for metadata in message_attachment_metadata(&message.content)
                .map_err(|_| ArchiveFailure::InvalidArchive)?
            {
                check_cancelled(cancellation)?;
                let path =
                    attachment_path(storage.root(), conversation_id, &message.id, &metadata.id)?;
                let bytes = read_verified_attachment(&path, &metadata, cancellation)?;
                let record = ArchiveAttachmentRecord {
                    path: archive_attachment_path(conversation_id, &message.id, &metadata.id),
                    size_bytes: bytes.len() as u64,
                    sha256: digest_hex(&Sha256::digest(&bytes)),
                };
                sources.push(AttachmentSource { record, path });
            }
            conversation.messages.push(message);
            if message_count % 100 == 0 {
                lease.set_progress("reading", message_count, None, 0, None)?;
            }
        }
        conversations.push(conversation);
        lease.set_progress("reading", message_count, None, 0, None)?;
    }
    check_cancelled(cancellation)?;
    if conversations.len() != unique_ids.len() {
        return Err(ArchiveFailure::NotFound);
    }
    if sources
        .iter()
        .map(|source| source.record.size_bytes)
        .sum::<u64>()
        > MAX_ATTACHMENT_BYTES
    {
        return Err(ArchiveFailure::LimitExceeded);
    }
    let data = ArchiveData { conversations };
    let manifest = ArchiveManifest {
        format: ARCHIVE_FORMAT.to_owned(),
        version: ARCHIVE_VERSION,
        created_at_unix_ms: 0,
        conversation_count: data.conversations.len(),
        message_count,
        attachments: sources.iter().map(|source| source.record.clone()).collect(),
    };
    validate_archive_data(&manifest, &data)?;
    drop(transaction);
    Ok((data, sources))
}

fn read_memory_settings(
    connection: &StorageTransaction<'_>,
    conversation_id: &str,
) -> Result<ArchivedMemorySettings, ArchiveFailure> {
    let included = connection
        .query_row(
            "SELECT included FROM conversation_memory_archive_settings WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?;
    let mut statement = connection.prepare(
        "SELECT tool_name FROM conversation_memory_excluded_tools
         WHERE conversation_id = ?1 ORDER BY tool_name",
    )?;
    let excluded_tools = statement
        .query_map([conversation_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ArchivedMemorySettings {
        included,
        excluded_tools,
    })
}

fn validate_output_path(path: &Path) -> Result<PathBuf, ArchiveFailure> {
    if !path.is_absolute()
        || !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("openchatbackup"))
    {
        return Err(ArchiveFailure::InvalidRequest);
    }
    if fs::symlink_metadata(path).is_ok() {
        return Err(ArchiveFailure::Conflict);
    }
    let parent = path.parent().ok_or(ArchiveFailure::InvalidRequest)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| ArchiveFailure::Storage)?;
    if !canonical_parent.is_dir() {
        return Err(ArchiveFailure::Storage);
    }
    let file_name = path.file_name().ok_or(ArchiveFailure::InvalidRequest)?;
    Ok(canonical_parent.join(file_name))
}

fn validate_input_path(path: &Path) -> Result<File, ArchiveFailure> {
    if !path.is_absolute()
        || !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("openchatbackup"))
    {
        return Err(ArchiveFailure::InvalidRequest);
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ArchiveFailure::NotFound
        } else {
            ArchiveFailure::Storage
        }
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_ARCHIVE_FILE_BYTES
    {
        return Err(ArchiveFailure::LimitExceeded);
    }
    File::open(path).map_err(|_| ArchiveFailure::Storage)
}

fn read_archive(
    path: &Path,
    passphrase: &str,
    stage: Option<&RestoreStage>,
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
) -> Result<(ArchiveManifest, ArchiveData), ArchiveFailure> {
    check_cancelled(cancellation)?;
    let file = validate_input_path(path)?;
    let archive_bytes = file.metadata().map_err(|_| ArchiveFailure::Storage)?.len();
    lease.set_progress("reading", 0, None, 0, Some(archive_bytes))?;
    let decryptor =
        Decryptor::new(BufReader::new(file)).map_err(|_| ArchiveFailure::InvalidArchive)?;
    if !decryptor.is_scrypt() {
        return Err(ArchiveFailure::InvalidArchive);
    }
    check_cancelled(cancellation)?;
    let mut identity = Identity::new(SecretString::from(passphrase.to_owned()));
    identity.set_max_work_factor(MAX_AGE_SCRYPT_LOG_N);
    let decrypted = decryptor
        .decrypt(iter::once(&identity as &dyn age::Identity))
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    check_cancelled(cancellation)?;
    let limited = decrypted.take(MAX_ARCHIVE_FILE_BYTES.saturating_add(1));
    let mut archive = Archive::new(limited);
    let mut entries = archive
        .entries()
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    check_cancelled(cancellation)?;
    let first = entries
        .next()
        .transpose()
        .map_err(|_| ArchiveFailure::InvalidArchive)?
        .ok_or(ArchiveFailure::InvalidArchive)?;
    validate_tar_entry(&first, MANIFEST_PATH)?;
    if first.size() > MAX_MANIFEST_BYTES {
        return Err(ArchiveFailure::LimitExceeded);
    }
    let manifest = read_entry_json::<ArchiveManifest, _>(
        first,
        MAX_MANIFEST_BYTES,
        cancellation,
        lease,
        None,
    )?;
    validate_manifest(&manifest)?;

    check_cancelled(cancellation)?;
    let second = entries
        .next()
        .transpose()
        .map_err(|_| ArchiveFailure::InvalidArchive)?
        .ok_or(ArchiveFailure::InvalidArchive)?;
    validate_tar_entry(&second, CONVERSATIONS_PATH)?;
    if second.size() > MAX_CONVERSATIONS_JSON_BYTES {
        return Err(ArchiveFailure::LimitExceeded);
    }
    let data = read_entry_json::<ArchiveData, _>(
        second,
        MAX_CONVERSATIONS_JSON_BYTES,
        cancellation,
        lease,
        None,
    )?;
    validate_archive_data(&manifest, &data)?;

    let mut attachment_records = manifest
        .attachments
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect::<HashMap<_, _>>();
    let mut attachment_metadata = expected_attachment_metadata(&data)?;
    let mut attachment_bytes = 0_u64;
    let total_attachment_bytes = manifest
        .attachments
        .iter()
        .try_fold(0_u64, |total, record| total.checked_add(record.size_bytes))
        .ok_or(ArchiveFailure::LimitExceeded)?;
    let mut attachments_read = 0_usize;
    for entry in entries.by_ref() {
        check_cancelled(cancellation)?;
        let mut entry = entry.map_err(|_| {
            if *cancellation.borrow() {
                ArchiveFailure::Cancelled
            } else {
                ArchiveFailure::InvalidArchive
            }
        })?;
        let path = tar_path(&entry)?;
        let record = attachment_records
            .remove(path.as_str())
            .ok_or(ArchiveFailure::InvalidArchive)?;
        validate_tar_entry(&entry, &record.path)?;
        if entry.size() != record.size_bytes {
            return Err(ArchiveFailure::InvalidArchive);
        }
        let metadata = attachment_metadata
            .remove(path.as_str())
            .ok_or(ArchiveFailure::InvalidArchive)?;
        let staged_path = stage
            .map(|stage| stage.file_path(path.as_str()))
            .transpose()?;
        let mut staged_file = staged_path
            .as_ref()
            .map(|path| {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|_| ArchiveFailure::Storage)
            })
            .transpose()?;
        let mut bytes = Vec::with_capacity(metadata.size_bytes);
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            check_cancelled(cancellation)?;
            let count = entry.read(&mut buffer).map_err(|_| {
                if *cancellation.borrow() {
                    ArchiveFailure::Cancelled
                } else {
                    ArchiveFailure::InvalidArchive
                }
            })?;
            if count == 0 {
                break;
            }
            let total_after = (bytes.len() as u64)
                .checked_add(count as u64)
                .ok_or(ArchiveFailure::LimitExceeded)?;
            if total_after > record.size_bytes || total_after > MAX_ATTACHMENT_BYTES {
                return Err(ArchiveFailure::InvalidArchive);
            }
            hasher.update(&buffer[..count]);
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(file) = staged_file.as_mut() {
                file.write_all(&buffer[..count])
                    .map_err(|_| ArchiveFailure::Storage)?;
            }
            lease.set_progress(
                "reading",
                attachments_read,
                Some(manifest.attachments.len()),
                attachment_bytes.saturating_add(bytes.len() as u64),
                Some(total_attachment_bytes),
            )?;
        }
        if bytes.len() != metadata.size_bytes || digest_hex(&hasher.finalize()) != record.sha256 {
            return Err(ArchiveFailure::InvalidArchive);
        }
        validate_portable_attachment_content(&metadata, &bytes)
            .map_err(|_| ArchiveFailure::InvalidArchive)?;
        if let Some(file) = staged_file.as_mut() {
            file.sync_all().map_err(|_| ArchiveFailure::Storage)?;
        }
        attachment_bytes = attachment_bytes
            .checked_add(record.size_bytes)
            .ok_or(ArchiveFailure::LimitExceeded)?;
        attachments_read += 1;
        lease.set_progress(
            "reading",
            attachments_read,
            Some(manifest.attachments.len()),
            attachment_bytes,
            Some(total_attachment_bytes),
        )?;
        if attachment_bytes > MAX_ATTACHMENT_BYTES {
            return Err(ArchiveFailure::LimitExceeded);
        }
    }
    check_cancelled(cancellation)?;
    if !attachment_records.is_empty() || !attachment_metadata.is_empty() {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let mut decrypted = archive.into_inner();
    let mut drain_buffer = [0_u8; 64 * 1024];
    let mut drained = 0_u64;
    loop {
        check_cancelled(cancellation)?;
        let count = decrypted.read(&mut drain_buffer).map_err(|_| {
            if *cancellation.borrow() {
                ArchiveFailure::Cancelled
            } else {
                ArchiveFailure::InvalidArchive
            }
        })?;
        if count == 0 {
            break;
        }
        drained = drained
            .checked_add(count as u64)
            .ok_or(ArchiveFailure::LimitExceeded)?;
    }
    let consumed = MAX_ARCHIVE_FILE_BYTES
        .saturating_add(1)
        .saturating_sub(decrypted.limit());
    if consumed > MAX_ARCHIVE_FILE_BYTES
        || drained > MAX_ARCHIVE_FILE_BYTES
        || consumed.saturating_add(drained) > MAX_ARCHIVE_FILE_BYTES
    {
        return Err(ArchiveFailure::LimitExceeded);
    }
    Ok((manifest, data))
}

fn validate_tar_entry<R: Read>(
    entry: &tar::Entry<'_, R>,
    expected: &str,
) -> Result<(), ArchiveFailure> {
    if !entry.header().entry_type().is_file() || tar_path(entry)? != expected {
        return Err(ArchiveFailure::InvalidArchive);
    }
    Ok(())
}

fn tar_path<R: Read>(entry: &tar::Entry<'_, R>) -> Result<String, ArchiveFailure> {
    let path = entry.path().map_err(|_| ArchiveFailure::InvalidArchive)?;
    let path = path.to_str().ok_or(ArchiveFailure::InvalidArchive)?;
    if path.is_empty() || path.contains('\\') || path.starts_with('/') {
        return Err(ArchiveFailure::InvalidArchive);
    }
    Ok(path.to_owned())
}

fn read_entry_json<T: DeserializeOwned, R: Read>(
    entry: tar::Entry<'_, R>,
    maximum: u64,
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
    total_items: Option<usize>,
) -> Result<T, ArchiveFailure> {
    let size = entry.size();
    if size > maximum {
        return Err(ArchiveFailure::LimitExceeded);
    }
    let mut bytes =
        Vec::with_capacity(usize::try_from(size).map_err(|_| ArchiveFailure::LimitExceeded)?);
    let mut entry = entry.take(maximum.saturating_add(1));
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancelled(cancellation)?;
        let count = entry.read(&mut buffer).map_err(|_| {
            if *cancellation.borrow() {
                ArchiveFailure::Cancelled
            } else {
                ArchiveFailure::InvalidArchive
            }
        })?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        lease.set_progress("reading", 0, total_items, bytes.len() as u64, Some(size))?;
    }
    if bytes.len() as u64 != size {
        return Err(ArchiveFailure::InvalidArchive);
    }
    serde_json::from_slice(&bytes).map_err(|_| ArchiveFailure::InvalidArchive)
}

fn validate_manifest(manifest: &ArchiveManifest) -> Result<(), ArchiveFailure> {
    if manifest.format != ARCHIVE_FORMAT
        || manifest.version != ARCHIVE_VERSION
        || manifest.conversation_count == 0
        || manifest.conversation_count > MAX_CONVERSATIONS
        || manifest.message_count > MAX_MESSAGES
        || manifest.attachments.len() > MAX_MESSAGES.saturating_mul(10)
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let mut paths = HashSet::with_capacity(manifest.attachments.len());
    let mut total = 0_u64;
    for attachment in &manifest.attachments {
        if !paths.insert(attachment.path.as_str())
            || attachment.sha256.len() != 64
            || !attachment
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ArchiveFailure::InvalidArchive);
        }
        total = total
            .checked_add(attachment.size_bytes)
            .ok_or(ArchiveFailure::LimitExceeded)?;
        if total > MAX_ATTACHMENT_BYTES {
            return Err(ArchiveFailure::LimitExceeded);
        }
    }
    Ok(())
}

fn validate_archive_data(
    manifest: &ArchiveManifest,
    data: &ArchiveData,
) -> Result<(), ArchiveFailure> {
    if manifest.conversation_count != data.conversations.len()
        || data.conversations.is_empty()
        || data.conversations.len() > MAX_CONVERSATIONS
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let mut conversation_ids = HashSet::with_capacity(data.conversations.len());
    let mut message_count = 0_usize;
    let mut expected_attachments = HashMap::new();
    for conversation in &data.conversations {
        if !safe_identifier(&conversation.id)
            || !conversation_ids.insert(conversation.id.as_str())
            || conversation.title.trim().is_empty()
            || conversation.title.len() > 4096
            || !matches!(conversation.title_source.as_str(), "automatic" | "manual")
            || conversation
                .provider_id
                .as_ref()
                .is_some_and(|value| value.len() > 128)
            || conversation
                .model_id
                .as_ref()
                .is_some_and(|value| value.len() > 512)
            || conversation.memory_settings.excluded_tools.len() > 512
        {
            return Err(ArchiveFailure::InvalidArchive);
        }
        let mut message_ids = HashSet::with_capacity(conversation.messages.len());
        for message in &conversation.messages {
            message_count = message_count.saturating_add(1);
            if message_count > MAX_MESSAGES
                || !safe_identifier(&message.id)
                || !message_ids.insert(message.id.as_str())
                || !matches!(message.role.as_str(), "user" | "assistant")
                || message.content.len() > 16 * 1024 * 1024
                || !matches!(
                    message.status.as_str(),
                    "streaming" | "completed" | "failed" | "stopped"
                )
                || message.output_tokens.is_some_and(|tokens| tokens < 0)
                || message
                    .tokens_per_second
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
                || message.elapsed_microseconds.is_some_and(|value| value < 0)
            {
                return Err(ArchiveFailure::InvalidArchive);
            }
            validate_reasoning_summaries(&message.reasoning_summaries)?;
            validate_tool_activities(&message.tool_activities)?;
            for metadata in message_attachment_metadata(&message.content)
                .map_err(|_| ArchiveFailure::InvalidArchive)?
            {
                let path = archive_attachment_path(&conversation.id, &message.id, &metadata.id);
                if expected_attachments.insert(path, metadata).is_some() {
                    return Err(ArchiveFailure::InvalidArchive);
                }
            }
        }
        for name in &conversation.memory_settings.excluded_tools {
            if name.trim().is_empty() || name.len() > 128 {
                return Err(ArchiveFailure::InvalidArchive);
            }
        }
    }
    if message_count != manifest.message_count
        || expected_attachments.len() != manifest.attachments.len()
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    for attachment in &manifest.attachments {
        let metadata = expected_attachments
            .get(&attachment.path)
            .ok_or(ArchiveFailure::InvalidArchive)?;
        if u64::try_from(metadata.size_bytes).map_err(|_| ArchiveFailure::InvalidArchive)?
            != attachment.size_bytes
        {
            return Err(ArchiveFailure::InvalidArchive);
        }
    }
    Ok(())
}

fn validate_reasoning_summaries(encoded: &str) -> Result<(), ArchiveFailure> {
    let Value::Array(summaries) =
        serde_json::from_str::<Value>(encoded).map_err(|_| ArchiveFailure::InvalidArchive)?
    else {
        return Err(ArchiveFailure::InvalidArchive);
    };
    if summaries.len() > 10_000 {
        return Err(ArchiveFailure::LimitExceeded);
    }
    for summary in summaries {
        let Some(fields) = summary.as_object() else {
            return Err(ArchiveFailure::InvalidArchive);
        };
        if fields
            .get("id")
            .and_then(Value::as_str)
            .is_none_or(|value| value.is_empty() || value.len() > 512)
            || fields.get("content").and_then(Value::as_str).is_none()
            || fields.get("isComplete").and_then(Value::as_bool).is_none()
            || fields.get("elapsedMicroseconds").is_some_and(|value| {
                !value.is_null() && value.as_i64().is_none_or(|duration| duration < 0)
            })
        {
            return Err(ArchiveFailure::InvalidArchive);
        }
    }
    Ok(())
}

fn validate_tool_activities(encoded: &str) -> Result<(), ArchiveFailure> {
    let Value::Array(activities) =
        serde_json::from_str::<Value>(encoded).map_err(|_| ArchiveFailure::InvalidArchive)?
    else {
        return Err(ArchiveFailure::InvalidArchive);
    };
    if activities.len() > 10_000 {
        return Err(ArchiveFailure::LimitExceeded);
    }
    for activity in activities {
        let Some(fields) = activity.as_object() else {
            return Err(ArchiveFailure::InvalidArchive);
        };
        let call_id = fields.get("callId").and_then(Value::as_str);
        let name = fields.get("name").and_then(Value::as_str);
        let status = fields.get("status").and_then(Value::as_str);
        let Some(status) = status else {
            return Err(ArchiveFailure::InvalidArchive);
        };
        let requires_output = matches!(status, "completed" | "failed" | "denied" | "cancelled");
        if call_id.is_none_or(|value| value.is_empty() || value.len() > 512)
            || name.is_none_or(|value| value.is_empty() || value.len() > 512)
            || !fields.contains_key("arguments")
            || !matches!(
                status,
                "awaitingApproval"
                    | "waitingForUser"
                    | "running"
                    | "completed"
                    | "failed"
                    | "denied"
                    | "cancelled"
            )
            || fields.contains_key("output") != requires_output
            || fields
                .get("roundId")
                .is_some_and(|value| !value.is_null() && value.as_str().is_none_or(str::is_empty))
            || fields
                .get("targetPath")
                .is_some_and(|value| !value.is_null() && value.as_str().is_none())
            || fields
                .get("assistantTextBeforeByteOffset")
                .is_some_and(|value| {
                    !value.is_null() && value.as_i64().is_none_or(|offset| offset < 0)
                })
        {
            return Err(ArchiveFailure::InvalidArchive);
        }
    }
    Ok(())
}

fn expected_attachment_metadata(
    data: &ArchiveData,
) -> Result<HashMap<String, AttachmentMetadata>, ArchiveFailure> {
    let mut result = HashMap::new();
    for conversation in &data.conversations {
        for message in &conversation.messages {
            for metadata in message_attachment_metadata(&message.content)
                .map_err(|_| ArchiveFailure::InvalidArchive)?
            {
                let path = archive_attachment_path(&conversation.id, &message.id, &metadata.id);
                if result.insert(path, metadata).is_some() {
                    return Err(ArchiveFailure::InvalidArchive);
                }
            }
        }
    }
    Ok(result)
}

fn count_existing_conversations(
    connection: &StorageConnection<'_>,
    conversations: &[ArchivedConversation],
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
) -> Result<usize, ArchiveFailure> {
    let mut statement =
        connection.prepare("SELECT EXISTS (SELECT 1 FROM conversations WHERE id = ?1)")?;
    let mut count = 0;
    for (index, conversation) in conversations.iter().enumerate() {
        check_cancelled(cancellation)?;
        let exists = statement.query_row([&conversation.id], |row| row.get::<_, bool>(0))?;
        count += usize::from(exists);
        lease.set_progress("verifying", index + 1, Some(conversations.len()), 0, None)?;
    }
    check_cancelled(cancellation)?;
    Ok(count)
}

fn archive_summary(manifest: &ArchiveManifest, duplicate_count: usize) -> Value {
    json!({
        "formatVersion": manifest.version,
        "conversationCount": manifest.conversation_count,
        "messageCount": manifest.message_count,
        "attachmentCount": manifest.attachments.len(),
        "attachmentBytes": manifest.attachments.iter().map(|record| record.size_bytes).sum::<u64>(),
        "duplicateConversationCount": duplicate_count,
        "createdAtUnixMs": manifest.created_at_unix_ms,
    })
}

fn restore_transaction(
    transaction: &StorageTransaction<'_>,
    storage_root: &Path,
    stage: &RestoreStage,
    manifest: &ArchiveManifest,
    data: ArchiveData,
    policy: ConflictPolicy,
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
) -> Result<(Value, CreatedAttachmentFiles), ArchiveFailure> {
    check_cancelled(cancellation)?;
    validate_archive_data(manifest, &data)?;
    let mut target_ids = HashMap::with_capacity(data.conversations.len());
    let mut duplicate_count = 0_usize;
    for conversation in &data.conversations {
        check_cancelled(cancellation)?;
        let exists = transaction.query_row(
            "SELECT EXISTS (SELECT 1 FROM conversations WHERE id = ?1)",
            [&conversation.id],
            |row| row.get::<_, bool>(0),
        )?;
        if exists {
            duplicate_count += 1;
            match policy {
                ConflictPolicy::SkipExisting => {
                    target_ids.insert(conversation.id.clone(), None);
                    continue;
                }
                ConflictPolicy::ImportAsCopy => {
                    let copied_id = loop {
                        let candidate = Uuid::new_v4().simple().to_string();
                        let candidate_exists = transaction.query_row(
                            "SELECT EXISTS (SELECT 1 FROM conversations WHERE id = ?1)",
                            [&candidate],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !candidate_exists {
                            break candidate;
                        }
                    };
                    target_ids.insert(conversation.id.clone(), Some(copied_id));
                }
            }
        } else {
            target_ids.insert(conversation.id.clone(), Some(conversation.id.clone()));
        }
    }
    let mut created_files = CreatedAttachmentFiles::new(manifest.attachments.len());
    let total_attachment_bytes = manifest
        .attachments
        .iter()
        .try_fold(0_u64, |total, record| total.checked_add(record.size_bytes))
        .ok_or(ArchiveFailure::LimitExceeded)?;
    let mut copied_attachment_bytes = 0_u64;
    for (index, record) in manifest.attachments.iter().enumerate() {
        check_cancelled(cancellation)?;
        let (source_conversation_id, message_id, _, file_name) =
            parse_attachment_archive_path(&record.path)?;
        let Some(target_conversation_id) = target_ids
            .get(source_conversation_id)
            .ok_or(ArchiveFailure::InvalidArchive)?
        else {
            continue;
        };
        let staged_path = stage.directory.join(&record.path);
        let target = create_attachment_destination(
            storage_root,
            target_conversation_id,
            message_id,
            file_name,
        )?;
        copy_new_file(
            &staged_path,
            &target,
            &mut created_files,
            cancellation,
            lease,
            index,
            manifest.attachments.len(),
            copied_attachment_bytes,
            total_attachment_bytes,
        )?;
        copied_attachment_bytes = copied_attachment_bytes
            .checked_add(record.size_bytes)
            .ok_or(ArchiveFailure::LimitExceeded)?;
        lease.set_progress(
            "restoring",
            index + 1,
            Some(manifest.attachments.len()),
            copied_attachment_bytes,
            Some(total_attachment_bytes),
        )?;
    }

    let mut inserted_count = 0_usize;
    let mut inserted_messages = 0_usize;
    for conversation in &data.conversations {
        check_cancelled(cancellation)?;
        let target_id = target_ids
            .get(&conversation.id)
            .ok_or(ArchiveFailure::InvalidArchive)?;
        let Some(target_id) = target_id else {
            continue;
        };
        transaction.execute(
            "INSERT INTO conversations (
                id, title, title_source, connection_id, workspace_id, api_key_connection_id,
                provider_id, model_id, project_id, is_pinned, is_archived,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, NULL, NULL, NULL, ?4, ?5, NULL, ?6, ?7, ?8, ?9)",
            params![
                target_id,
                conversation.title,
                conversation.title_source,
                conversation.provider_id,
                conversation.model_id,
                conversation.is_pinned,
                conversation.is_archived,
                conversation.created_at,
                conversation.updated_at,
            ],
        )?;
        if let Some(included) = conversation.memory_settings.included {
            transaction.execute(
                "INSERT INTO conversation_memory_archive_settings (conversation_id, included)
                 VALUES (?1, ?2)",
                params![target_id, included],
            )?;
        }
        for tool_name in &conversation.memory_settings.excluded_tools {
            transaction.execute(
                "INSERT INTO conversation_memory_excluded_tools (conversation_id, tool_name)
                 VALUES (?1, ?2)",
                params![target_id, tool_name],
            )?;
        }
        for message in &conversation.messages {
            check_cancelled(cancellation)?;
            transaction.execute(
                "INSERT INTO messages (
                    id, conversation_id, role, content, created_at, output_tokens,
                    tokens_per_second, elapsed_microseconds, reasoning_summaries,
                    tool_activities, status, failure_code
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    message.id,
                    target_id,
                    message.role,
                    message.content,
                    message.created_at,
                    message.output_tokens,
                    message.tokens_per_second,
                    message.elapsed_microseconds,
                    message.reasoning_summaries,
                    message.tool_activities,
                    message.status,
                    message.failure_code,
                ],
            )?;
            inserted_messages += 1;
            if inserted_messages % 100 == 0 {
                lease.set_progress(
                    "restoring",
                    inserted_messages,
                    Some(manifest.message_count),
                    copied_attachment_bytes,
                    Some(total_attachment_bytes),
                )?;
            }
        }
        inserted_count += 1;
    }
    check_cancelled(cancellation)?;
    lease.set_progress(
        "committing",
        inserted_messages,
        Some(manifest.message_count),
        copied_attachment_bytes,
        Some(total_attachment_bytes),
    )?;
    Ok((
        json!({
            "restoredConversationCount": inserted_count,
            "restoredMessageCount": inserted_messages,
            "skippedConversationCount": data.conversations.len().saturating_sub(inserted_count),
            "duplicateConversationCount": duplicate_count,
            "restoredAttachmentCount": created_files.paths.len(),
        }),
        created_files,
    ))
}

fn create_attachment_destination(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    file_name: &str,
) -> Result<PathBuf, ArchiveFailure> {
    if !safe_identifier(conversation_id)
        || !safe_identifier(message_id)
        || !safe_attachment_file_name(file_name)
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let attachments_root = storage_root.join("attachments");
    fs::create_dir_all(&attachments_root).map_err(|_| ArchiveFailure::Storage)?;
    let canonical_root =
        fs::canonicalize(&attachments_root).map_err(|_| ArchiveFailure::Storage)?;
    let canonical_storage = fs::canonicalize(storage_root).map_err(|_| ArchiveFailure::Storage)?;
    if !canonical_root.starts_with(&canonical_storage) {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let conversation = attachments_root.join(conversation_id);
    ensure_directory(&conversation)?;
    let message = conversation.join(message_id);
    ensure_directory(&message)?;
    let canonical_message = fs::canonicalize(&message).map_err(|_| ArchiveFailure::Storage)?;
    if !canonical_message.starts_with(&canonical_root) {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let destination = canonical_message.join(file_name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(ArchiveFailure::Conflict);
    }
    Ok(destination)
}

fn ensure_directory(path: &Path) -> Result<(), ArchiveFailure> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|_| ArchiveFailure::Storage)?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                Ok(())
            } else {
                Err(ArchiveFailure::InvalidArchive)
            }
        }
        Err(_) => Err(ArchiveFailure::Storage),
    }
}

fn copy_new_file(
    source: &Path,
    destination: &Path,
    created_files: &mut CreatedAttachmentFiles,
    cancellation: &watch::Receiver<bool>,
    lease: &ArchiveOperationLease,
    completed_items: usize,
    total_items: usize,
    completed_bytes: u64,
    total_bytes: u64,
) -> Result<(), ArchiveFailure> {
    check_cancelled(cancellation)?;
    let mut source = File::open(source).map_err(|_| ArchiveFailure::Storage)?;
    let file_bytes = source
        .metadata()
        .map_err(|_| ArchiveFailure::Storage)?
        .len();
    let mut destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                ArchiveFailure::Conflict
            } else {
                ArchiveFailure::Storage
            }
        })?;
    created_files.push(destination.to_path_buf());
    let mut copied_bytes = 0_u64;
    let mut last_reported_bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancelled(cancellation)?;
        let count = source
            .read(&mut buffer)
            .map_err(|_| ArchiveFailure::Storage)?;
        if count == 0 {
            break;
        }
        destination_file
            .write_all(&buffer[..count])
            .map_err(|_| ArchiveFailure::Storage)?;
        copied_bytes = copied_bytes
            .checked_add(count as u64)
            .ok_or(ArchiveFailure::LimitExceeded)?;
        if copied_bytes.saturating_sub(last_reported_bytes) >= 1024 * 1024 {
            lease.set_progress(
                "restoring",
                completed_items,
                Some(total_items),
                completed_bytes.saturating_add(copied_bytes),
                Some(total_bytes),
            )?;
            last_reported_bytes = copied_bytes;
        }
    }
    if copied_bytes != file_bytes {
        return Err(ArchiveFailure::Storage);
    }
    check_cancelled(cancellation)?;
    destination_file
        .sync_all()
        .map_err(|_| ArchiveFailure::Storage)
}

fn cleanup_created_files(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        let _ = fs::remove_file(path);
    }
}

fn attachment_path(
    storage_root: &Path,
    conversation_id: &str,
    message_id: &str,
    attachment_id: &str,
) -> Result<PathBuf, ArchiveFailure> {
    if !safe_identifier(conversation_id)
        || !safe_identifier(message_id)
        || !safe_identifier(attachment_id)
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let root = storage_root.join("attachments");
    let conversation = root.join(conversation_id);
    let message = conversation.join(message_id);
    let file_path = message.join(format!("{attachment_id}.data"));
    let canonical_root = fs::canonicalize(root).map_err(|_| ArchiveFailure::Storage)?;
    let canonical_storage = fs::canonicalize(storage_root).map_err(|_| ArchiveFailure::Storage)?;
    let canonical_conversation =
        fs::canonicalize(conversation).map_err(|_| ArchiveFailure::Storage)?;
    let canonical_message = fs::canonicalize(message).map_err(|_| ArchiveFailure::Storage)?;
    let canonical_file = fs::canonicalize(file_path).map_err(|_| ArchiveFailure::Storage)?;
    if !canonical_root.starts_with(&canonical_storage)
        || !canonical_conversation.starts_with(&canonical_root)
        || !canonical_message.starts_with(&canonical_conversation)
        || !canonical_file.starts_with(&canonical_message)
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    Ok(canonical_file)
}

fn read_verified_attachment(
    path: &Path,
    metadata: &AttachmentMetadata,
    cancellation: &watch::Receiver<bool>,
) -> Result<Vec<u8>, ArchiveFailure> {
    check_cancelled(cancellation)?;
    let file = File::open(path).map_err(|_| ArchiveFailure::Storage)?;
    let file_metadata = file.metadata().map_err(|_| ArchiveFailure::Storage)?;
    if file_metadata.len() != metadata.size_bytes as u64 {
        return Err(ArchiveFailure::Storage);
    }
    let mut bytes = Vec::with_capacity(metadata.size_bytes);
    let mut reader = file.take(file_metadata.len().saturating_add(1));
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancelled(cancellation)?;
        let count = reader
            .read(&mut buffer)
            .map_err(|_| ArchiveFailure::Storage)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    check_cancelled(cancellation)?;
    validate_portable_attachment_content(metadata, &bytes).map_err(|_| ArchiveFailure::Storage)?;
    Ok(bytes)
}

fn parse_attachment_archive_path(path: &str) -> Result<(&str, &str, &str, &str), ArchiveFailure> {
    let mut components = path.split('/');
    let prefix = components.next().ok_or(ArchiveFailure::InvalidArchive)?;
    let conversation_id = components.next().ok_or(ArchiveFailure::InvalidArchive)?;
    let message_id = components.next().ok_or(ArchiveFailure::InvalidArchive)?;
    let file_name = components.next().ok_or(ArchiveFailure::InvalidArchive)?;
    let attachment_id = file_name
        .strip_suffix(".data")
        .ok_or(ArchiveFailure::InvalidArchive)?;
    if prefix != "attachments"
        || !safe_identifier(conversation_id)
        || !safe_identifier(message_id)
        || !safe_identifier(attachment_id)
        || !safe_attachment_file_name(file_name)
        || components.next().is_some()
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    Ok((conversation_id, message_id, attachment_id, file_name))
}

fn archive_attachment_path(conversation_id: &str, message_id: &str, attachment_id: &str) -> String {
    format!("attachments/{conversation_id}/{message_id}/{attachment_id}.data")
}

fn safe_attachment_file_name(name: &str) -> bool {
    name.strip_suffix(".data").is_some_and(safe_identifier)
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_passphrase(passphrase: &str) -> Result<(), ArchiveFailure> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARACTERS
        || passphrase.len() > MAX_PASSPHRASE_BYTES
        || passphrase.contains('\0')
    {
        return Err(ArchiveFailure::InvalidPassphrase);
    }
    Ok(())
}

fn digest_hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unix_time_millis() -> io::Result<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?;
    i64::try_from(duration.as_millis()).map_err(io::Error::other)
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The conversation archive request is invalid.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;
    use zeroize::Zeroizing;

    use crate::service::OperationGate;
    use crate::storage::AppStorage;

    use super::{
        ArchiveFailure, ArchiveOperationKind, RestoreStage, begin_operation, export, export_inner,
        inspect, restore, status, validate_output_path,
    };

    struct TemporaryProfiles(PathBuf);

    impl TemporaryProfiles {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-conversation-archive-{}",
                Uuid::new_v4().simple()
            ));
            fs::create_dir_all(&path).expect("temporary test root should be created");
            Self(path)
        }

        fn storage(&self, name: &str) -> AppStorage {
            let root = self.0.join(name);
            let storage = AppStorage::open_at(root).expect("test storage should open");
            let connection = storage.connect().expect("test database should connect");
            connection
                .execute_batch(
                    "CREATE TABLE projects (
                        id TEXT PRIMARY KEY NOT NULL,
                        name TEXT NOT NULL,
                        folder_path TEXT NOT NULL,
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL
                    );
                    CREATE TABLE conversations (
                        id TEXT PRIMARY KEY NOT NULL,
                        title TEXT NOT NULL,
                        title_source TEXT NOT NULL DEFAULT 'automatic',
                        connection_id TEXT,
                        workspace_id TEXT,
                        api_key_connection_id TEXT,
                        provider_id TEXT,
                        model_id TEXT,
                        project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                        is_pinned INTEGER NOT NULL DEFAULT 0,
                        is_archived INTEGER NOT NULL DEFAULT 0,
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL
                    );
                    CREATE TABLE messages (
                        rowid INTEGER PRIMARY KEY,
                        id TEXT NOT NULL,
                        conversation_id TEXT NOT NULL
                            REFERENCES conversations(id) ON DELETE CASCADE,
                        role TEXT NOT NULL,
                        content TEXT NOT NULL,
                        created_at INTEGER,
                        output_tokens INTEGER,
                        tokens_per_second REAL,
                        elapsed_microseconds INTEGER,
                        reasoning_summaries TEXT NOT NULL DEFAULT '[]',
                        tool_activities TEXT NOT NULL DEFAULT '[]',
                        status TEXT NOT NULL,
                        failure_code TEXT,
                        UNIQUE (conversation_id, id)
                    );",
                )
                .expect("desktop chat tables should be created");
            drop(connection);
            storage
                .initialize_backend_schema()
                .expect("backend schema should initialize");
            storage
        }
    }

    impl Drop for TemporaryProfiles {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn add_conversation_with_attachment(storage: &AppStorage) -> Vec<u8> {
        let attachment = b"a portable attachment".to_vec();
        let content = format!(
            "\u{1e}openchat-attachments-v1:{}",
            json!({
                "content": "See attached file",
                "attachments": [{
                    "id": "attachment-1",
                    "name": "notes.txt",
                    "mimeType": "text/plain",
                    "sizeBytes": attachment.len(),
                    "kind": "text"
                }]
            })
        );
        let connection = storage.connect().expect("database should connect");
        connection
            .execute(
                "INSERT INTO conversations (
                    id, title, title_source, provider_id, model_id, is_pinned,
                    is_archived,
                    created_at, updated_at
                 ) VALUES ('conversation-1', 'Archive test', 'manual', 'opencode',
                           'test-model', 1, 1, 10, 20)",
                [],
            )
            .expect("conversation should be inserted");
        connection
            .execute(
                "INSERT INTO messages (
                    id, conversation_id, role, content, created_at, output_tokens,
                    tokens_per_second, elapsed_microseconds, reasoning_summaries,
                    tool_activities, status, failure_code
                 ) VALUES ('message-1', 'conversation-1', 'user', ?1, 11, NULL,
                           NULL, NULL, '[]', '[]', 'completed', NULL)",
                [content],
            )
            .expect("message should be inserted");
        connection
            .execute(
                "INSERT INTO conversation_memory_archive_settings (conversation_id, included)
                 VALUES ('conversation-1', 0)",
                [],
            )
            .expect("memory inclusion setting should be inserted");
        connection
            .execute(
                "INSERT INTO conversation_memory_excluded_tools (conversation_id, tool_name)
                 VALUES ('conversation-1', 'web_search')",
                [],
            )
            .expect("memory tool setting should be inserted");
        let attachment_directory = storage
            .root()
            .join("attachments")
            .join("conversation-1")
            .join("message-1");
        fs::create_dir_all(&attachment_directory).expect("attachment directory should be created");
        fs::write(attachment_directory.join("attachment-1.data"), &attachment)
            .expect("attachment should be stored");
        attachment
    }

    fn fresh_passphrase() -> Zeroizing<String> {
        Zeroizing::new("archive-passphrase-2026".to_owned())
    }

    #[test]
    fn archive_operation_status_exposes_single_flight_and_clears_after_drop() {
        let temporary = TemporaryProfiles::new();
        let storage = temporary.storage("status");
        let lease = begin_operation(&storage, ArchiveOperationKind::Export, &json!(41))
            .expect("first archive operation should start");

        let active = status(&storage).expect("active archive status should load");
        assert_eq!(active["operationInProgress"], true);
        assert_eq!(active["operationId"], json!(41));
        assert_eq!(active["action"], "export");

        let second = begin_operation(&storage, ArchiveOperationKind::Inspect, &json!(42));
        assert!(matches!(second, Err(ArchiveFailure::OperationBusy)));

        drop(lease);
        let idle = status(&storage).expect("idle archive status should load");
        assert_eq!(idle["operationInProgress"], false);
        assert!(idle["operationId"].is_null());
        assert!(idle["action"].is_null());
        assert!(idle["progress"].is_null());
    }

    #[test]
    fn restore_cancellation_during_preflight_releases_the_archive_operation() {
        let temporary = TemporaryProfiles::new();
        let storage = temporary.storage("restore-cancel");
        let (cancel_sender, cancellation) = watch::channel(false);

        let error = restore(
            &storage,
            &json!(43),
            "missing.openchatbackup",
            fresh_passphrase(),
            "skip_existing",
            &cancellation,
            &OperationGate::default(),
            || {
                let active = status(&storage).expect("preflight should see active status");
                assert_eq!(active["operationInProgress"], true);
                assert_eq!(active["action"], "restore");
                cancel_sender
                    .send(true)
                    .expect("restore cancellation receiver should be active");
                Ok(())
            },
        )
        .expect_err("cancellation after preflight should stop before reading an archive");

        assert_eq!(error.code, "operation_cancelled");
        assert_eq!(
            status(&storage).expect("archive status should reset after cancellation")["operationInProgress"],
            false
        );
    }

    #[test]
    fn restore_cancellation_winning_the_commit_gate_rolls_back_database_and_files() {
        let temporary = TemporaryProfiles::new();
        let source = temporary.storage("commit-source");
        add_conversation_with_attachment(&source);
        let (_cancel_sender, cancellation) = watch::channel(false);
        let exported = export(
            &source,
            &json!(44),
            &["conversation-1".to_owned()],
            fresh_passphrase(),
            &cancellation,
            &OperationGate::default(),
        )
        .expect("source archive should export");
        let export_id = exported["exportId"]
            .as_str()
            .expect("source export should return its identifier");
        let archive_path = source
            .root()
            .join("cache/archive-exports")
            .join(format!("{export_id}.openchatbackup"));

        let destination = temporary.storage("commit-destination");
        let cancellation_gate = OperationGate::default();
        let error = restore(
            &destination,
            &json!(45),
            archive_path.to_str().expect("archive path should be UTF-8"),
            fresh_passphrase(),
            "skip_existing",
            &cancellation,
            &cancellation_gate,
            || {
                assert!(cancellation_gate.request_cancellation());
                Ok(())
            },
        )
        .expect_err("a cancellation that wins the commit gate must stop restore");

        assert_eq!(error.code, "operation_cancelled");
        assert!(read_conversation_ids(&destination).is_empty());
        assert!(
            !destination
                .root()
                .join("attachments/conversation-1/message-1/attachment-1.data")
                .exists()
        );
    }

    #[test]
    fn export_cancellation_winning_the_commit_gate_removes_the_pending_archive() {
        let temporary = TemporaryProfiles::new();
        let storage = temporary.storage("export-cancel");
        add_conversation_with_attachment(&storage);
        let (_cancel_sender, cancellation) = watch::channel(false);
        let cancellation_gate = OperationGate::default();
        assert!(cancellation_gate.request_cancellation());

        let error = export(
            &storage,
            &json!(46),
            &["conversation-1".to_owned()],
            fresh_passphrase(),
            &cancellation,
            &cancellation_gate,
        )
        .expect_err("a cancellation that wins export publication must stop the export");

        assert_eq!(error.code, "operation_cancelled");
        let export_directory = storage.root().join("cache/archive-exports");
        let leftovers = fs::read_dir(export_directory)
            .expect("export directory should exist")
            .collect::<Result<Vec<_>, _>>()
            .expect("export directory contents should be readable");
        assert!(leftovers.is_empty());
    }

    fn read_conversation_ids(storage: &AppStorage) -> Vec<String> {
        let connection = storage.connect().expect("database should connect");
        let mut statement = connection
            .prepare("SELECT id FROM conversations ORDER BY id")
            .expect("conversation query should prepare");
        statement
            .query_map([], |row| row.get(0))
            .expect("conversation query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("conversation ids should load")
    }

    #[test]
    fn encrypted_archive_round_trips_selected_chat_and_attachments_safely() {
        let temporary = TemporaryProfiles::new();
        let source = temporary.storage("source");
        let attachment = add_conversation_with_attachment(&source);
        let (_cancel_sender, cancellation) = watch::channel(false);
        let exported = export(
            &source,
            &json!(1),
            &["conversation-1".to_owned()],
            fresh_passphrase(),
            &cancellation,
            &OperationGate::default(),
        )
        .expect("archive should export");
        let export_id = exported["exportId"]
            .as_str()
            .expect("service should return a pending archive identifier");
        let archive_path = source
            .root()
            .join("cache/archive-exports")
            .join(format!("{export_id}.openchatbackup"));
        assert_eq!(exported["conversationCount"], 1);
        assert_eq!(exported["messageCount"], 1);
        assert_eq!(exported["attachmentCount"], 1);
        assert_eq!(export_id.len(), 32);

        let destination = temporary.storage("destination");
        let preview = inspect(
            &destination,
            &json!(2),
            archive_path.to_str().expect("archive path should be UTF-8"),
            fresh_passphrase(),
            &cancellation,
        )
        .expect("archive should be inspected before restore");
        assert_eq!(preview["duplicateConversationCount"], 0);
        assert!(preview.get("providerCredentials").is_none());

        let restored = restore(
            &destination,
            &json!(3),
            archive_path.to_str().expect("archive path should be UTF-8"),
            fresh_passphrase(),
            "skip_existing",
            &cancellation,
            &OperationGate::default(),
            || Ok(()),
        )
        .expect("archive should restore");
        assert_eq!(restored["restoredConversationCount"], 1);
        assert_eq!(restored["restoredMessageCount"], 1);
        assert_eq!(restored["restoredAttachmentCount"], 1);
        assert_eq!(read_conversation_ids(&destination), ["conversation-1"]);

        let connection = destination.connect().expect("database should connect");
        let (
            provider_id,
            connection_id,
            is_archived,
            message_content,
            memory_included,
            excluded_tool,
        ): (Option<String>, Option<String>, bool, String, bool, String) = connection
            .query_row(
                "SELECT c.provider_id, c.connection_id, c.is_archived, m.content,
                        s.included, e.tool_name
                 FROM conversations c
                 JOIN messages m ON m.conversation_id = c.id
                 JOIN conversation_memory_archive_settings s ON s.conversation_id = c.id
                 JOIN conversation_memory_excluded_tools e ON e.conversation_id = c.id
                 WHERE c.id = 'conversation-1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get::<_, i64>(2)? != 0,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("restored conversation should retain safe data");
        assert_eq!(provider_id.as_deref(), Some("opencode"));
        assert_eq!(connection_id, None);
        assert!(is_archived);
        assert!(!memory_included);
        assert_eq!(excluded_tool, "web_search");
        assert!(message_content.starts_with("\u{1e}openchat-attachments-v1:"));
        assert_eq!(
            fs::read(
                destination
                    .root()
                    .join("attachments/conversation-1/message-1/attachment-1.data")
            )
            .expect("restored attachment should exist"),
            attachment
        );
        drop(connection);

        let second_restore = restore(
            &destination,
            &json!(4),
            archive_path.to_str().expect("archive path should be UTF-8"),
            fresh_passphrase(),
            "skip_existing",
            &cancellation,
            &OperationGate::default(),
            || Ok(()),
        )
        .expect("existing conversation should be skipped");
        assert_eq!(second_restore["restoredConversationCount"], 0);
        assert_eq!(second_restore["restoredAttachmentCount"], 0);

        let copied = restore(
            &destination,
            &json!(5),
            archive_path.to_str().expect("archive path should be UTF-8"),
            fresh_passphrase(),
            "import_as_copy",
            &cancellation,
            &OperationGate::default(),
            || Ok(()),
        )
        .expect("existing conversation should import as a copy");
        assert_eq!(copied["restoredConversationCount"], 1);
        assert_eq!(copied["restoredAttachmentCount"], 1);
        let copied_id = read_conversation_ids(&destination)
            .into_iter()
            .find(|id| id != "conversation-1")
            .expect("the imported copy should have a new id");
        assert_eq!(
            fs::read(
                destination
                    .root()
                    .join("attachments")
                    .join(copied_id)
                    .join("message-1/attachment-1.data")
            )
            .expect("copy attachment should follow the copied conversation"),
            attachment
        );
    }

    #[test]
    fn restore_stage_recovery_removes_abandoned_stages_and_preserves_active_ones() {
        let temporary = TemporaryProfiles::new();
        let storage = temporary.storage("stage-recovery");
        let active = RestoreStage::create(&storage).expect("create active restore stage");
        fs::write(active.directory.join("active.txt"), b"active restore")
            .expect("write active stage marker");

        let recovery_root = storage.root().join("cache/conversation-archives");
        let abandoned = recovery_root.join("restore-interrupted-process");
        fs::create_dir_all(&abandoned).expect("create abandoned restore stage");
        fs::write(abandoned.join("attachment.data"), b"abandoned stage")
            .expect("write abandoned stage marker");

        let next = RestoreStage::create(&storage).expect("create next restore stage");

        assert!(active.directory.join("active.txt").exists());
        assert!(!abandoned.exists());
        drop(next);
        drop(active);
    }

    #[test]
    fn invalid_passphrase_and_invalid_output_target_do_not_publish_partial_archives() {
        let temporary = TemporaryProfiles::new();
        let source = temporary.storage("source");
        add_conversation_with_attachment(&source);
        let archive_path = temporary.0.join("selected.openchatbackup");
        let (_cancel_sender, cancellation) = watch::channel(false);
        let lease = begin_operation(&source, ArchiveOperationKind::Export, &json!(1))
            .expect("archive export operation should begin");
        export_inner(
            &source,
            &["conversation-1".to_owned()],
            &archive_path,
            fresh_passphrase().as_str(),
            &cancellation,
            &lease,
        )
        .expect("archive should export");
        lease.finish();

        let destination = temporary.storage("destination");
        let wrong_password = inspect(
            &destination,
            &json!(2),
            archive_path.to_str().expect("archive path should be UTF-8"),
            Zeroizing::new("not-the-correct-passphrase".to_owned()),
            &cancellation,
        )
        .expect_err("wrong password must not inspect the archive");
        assert_eq!(wrong_password.code, "conversation_archive_invalid");

        let invalid_output = temporary.0.join("selected.tar");
        let error = validate_output_path(&invalid_output)
            .expect_err("unsupported archive extension should be rejected");
        assert!(matches!(error, ArchiveFailure::InvalidRequest));
        assert!(!invalid_output.exists());
    }

    #[test]
    fn restore_rolls_back_copied_attachments_without_deleting_existing_files() {
        let temporary = TemporaryProfiles::new();
        let source = temporary.storage("source");
        let first_attachment = b"first attachment";
        let second_attachment = b"second attachment";
        let content = format!(
            "\u{1e}openchat-attachments-v1:{}",
            json!({
                "content": "Two files",
                "attachments": [
                    {
                        "id": "attachment-1",
                        "name": "first.txt",
                        "mimeType": "text/plain",
                        "sizeBytes": first_attachment.len(),
                        "kind": "text"
                    },
                    {
                        "id": "attachment-2",
                        "name": "second.txt",
                        "mimeType": "text/plain",
                        "sizeBytes": second_attachment.len(),
                        "kind": "text"
                    }
                ]
            })
        );
        let connection = source.connect().expect("source database should connect");
        connection
            .execute(
                "INSERT INTO conversations (id, title, title_source, created_at, updated_at)
                 VALUES ('conversation-1', 'Archive rollback', 'manual', 10, 20)",
                [],
            )
            .expect("source conversation should be inserted");
        connection
            .execute(
                "INSERT INTO messages (id, conversation_id, role, content, status)
                 VALUES ('message-1', 'conversation-1', 'user', ?1, 'completed')",
                [content],
            )
            .expect("source message should be inserted");
        let source_attachment_directory =
            source.root().join("attachments/conversation-1/message-1");
        fs::create_dir_all(&source_attachment_directory)
            .expect("source attachment directory should be created");
        fs::write(
            source_attachment_directory.join("attachment-1.data"),
            first_attachment,
        )
        .expect("first source attachment should be written");
        fs::write(
            source_attachment_directory.join("attachment-2.data"),
            second_attachment,
        )
        .expect("second source attachment should be written");
        drop(connection);

        let archive_path = temporary.0.join("rollback.openchatbackup");
        let (_cancel_sender, cancellation) = watch::channel(false);
        let lease = begin_operation(&source, ArchiveOperationKind::Export, &json!(1))
            .expect("archive export operation should begin");
        export_inner(
            &source,
            &["conversation-1".to_owned()],
            &archive_path,
            fresh_passphrase().as_str(),
            &cancellation,
            &lease,
        )
        .expect("source archive should export");
        lease.finish();

        let destination = temporary.storage("destination");
        let existing_attachment_directory = destination
            .root()
            .join("attachments/conversation-1/message-1");
        fs::create_dir_all(&existing_attachment_directory)
            .expect("conflicting attachment directory should be created");
        let protected_bytes = b"pre-existing user file";
        fs::write(
            existing_attachment_directory.join("attachment-2.data"),
            protected_bytes,
        )
        .expect("pre-existing attachment file should be written");

        let error = restore(
            &destination,
            &json!(2),
            archive_path.to_str().expect("archive path should be UTF-8"),
            fresh_passphrase(),
            "skip_existing",
            &cancellation,
            &OperationGate::default(),
            || Ok(()),
        )
        .expect_err("the existing attachment should cause a conflict");
        assert_eq!(error.code, "conversation_archive_conflict");
        assert!(read_conversation_ids(&destination).is_empty());
        assert!(
            !existing_attachment_directory
                .join("attachment-1.data")
                .exists()
        );
        assert_eq!(
            fs::read(existing_attachment_directory.join("attachment-2.data"))
                .expect("pre-existing file should remain intact"),
            protected_bytes
        );
    }

    #[test]
    fn attachment_archive_path_rejects_traversal_and_backslash_paths() {
        assert!(super::parse_attachment_archive_path("attachments/c1/m1/a1.data").is_ok());
        assert!(super::parse_attachment_archive_path("attachments/../m1/a1.data").is_err());
        assert!(super::parse_attachment_archive_path("attachments/c1/m1\\a1.data").is_err());
    }
}
