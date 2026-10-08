use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Write},
    iter,
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use age::{Decryptor, Encryptor, scrypt::Identity, secrecy::SecretString};
use rusqlite::Connection;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, EntryType, Header};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    attachments::{
        AttachmentMetadata, message_attachment_metadata, validate_portable_attachment_content,
    },
    protocol::ServiceError,
    storage::AppStorage,
};

const ARCHIVE_FORMAT: &str = "openchat-profile";
const ARCHIVE_VERSION: u32 = 1;
const ARCHIVE_EXTENSION: &str = "openchatprofilebackup";
const MANIFEST_PATH: &str = "manifest.json";
const DATABASE_PATH: &str = "database/openchat.sqlite3";
const MIN_PASSPHRASE_CHARACTERS: usize = 12;
const MAX_PASSPHRASE_BYTES: usize = 512;
const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_DATABASE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES: u64 = 24 * 1024 * 1024 * 1024;
const MAX_ATTACHMENTS: usize = 100_000;
const MAX_MANIFEST_BYTES: u64 = 128 * 1024 * 1024;
const MAX_AGE_SCRYPT_LOG_N: u8 = 18;
const PROFILE_ARCHIVE_ROOT: &str = "profile-archives";
const RESTORE_ROOT: &str = "profile-restore";
const RESTORE_PENDING_FILE: &str = "restore.pending";
const RESTORE_APPLYING_FILE: &str = "restore.applying.json";
const RESTORE_AWAITING_FILE: &str = "restore.awaiting.json";
const RESTORE_COMMITTED_FILE: &str = "restore.committed.json";
const RECOVERY_DIRECTORY: &str = "profile-restores";
static RESTORE_OPERATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileManifest {
    format: String,
    version: u32,
    created_at_unix_ms: i64,
    chat_schema_version: i64,
    backend_schema_version: i64,
    conversation_count: u64,
    message_count: u64,
    database: FileRecord,
    attachments: Vec<FileRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileRecord {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreMarker {
    version: u32,
    stage_id: String,
    chat_schema_version: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreJournal {
    version: u32,
    stage_id: String,
    recovery_id: String,
    phase: RestorePhase,
    had_database_directory: bool,
    had_attachments_directory: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RestorePhase {
    Applying,
    AwaitingValidation,
    Committed,
}

#[derive(Clone, Copy)]
enum ArchiveFailure {
    InvalidRequest,
    InvalidArchive,
    InvalidPassphrase,
    NotFound,
    Conflict,
    Busy,
    Storage,
    LimitExceeded,
    UnsupportedSchema,
}

impl From<rusqlite::Error> for ArchiveFailure {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}

impl From<io::Error> for ArchiveFailure {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::NotFound {
            Self::NotFound
        } else if error.kind() == io::ErrorKind::AlreadyExists {
            Self::Conflict
        } else {
            Self::Storage
        }
    }
}

struct AttachmentSource {
    record: FileRecord,
    path: PathBuf,
}

struct TemporaryDirectory {
    path: PathBuf,
    active: bool,
}

impl TemporaryDirectory {
    fn create(parent: &Path, label: &str) -> Result<Self, ArchiveFailure> {
        fs::create_dir_all(parent).map_err(|_| ArchiveFailure::Storage)?;
        let path = parent.join(format!("{label}-{}", Uuid::new_v4().simple()));
        fs::create_dir(&path).map_err(|_| ArchiveFailure::Storage)?;
        Ok(Self { path, active: true })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn preserve(&mut self) {
        self.active = false;
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

struct PartialOutput {
    path: PathBuf,
    active: bool,
}

impl PartialOutput {
    fn new(path: PathBuf) -> Self {
        Self { path, active: true }
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

fn service_error(failure: ArchiveFailure) -> ServiceError {
    match failure {
        ArchiveFailure::InvalidRequest => ServiceError::new(
            "invalid_request_params",
            "The profile backup request is invalid.",
            false,
        ),
        ArchiveFailure::InvalidArchive => ServiceError::new(
            "profile_archive_invalid",
            "The encrypted profile backup is invalid or its passphrase is incorrect.",
            false,
        ),
        ArchiveFailure::InvalidPassphrase => ServiceError::new(
            "profile_archive_passphrase_invalid",
            "Use a passphrase with at least 12 characters and no more than 512 bytes.",
            false,
        ),
        ArchiveFailure::NotFound => ServiceError::new(
            "profile_archive_not_found",
            "The selected profile backup could not be found.",
            false,
        ),
        ArchiveFailure::Conflict => ServiceError::new(
            "profile_archive_conflict",
            "A profile backup is already waiting to be restored, or the output file exists.",
            false,
        ),
        ArchiveFailure::Busy => ServiceError::new(
            "profile_archive_busy",
            "Another profile backup operation is in progress.",
            true,
        ),
        ArchiveFailure::Storage => ServiceError::new(
            "profile_archive_storage_failed",
            "The profile backup could not be read, written, or safely restored.",
            true,
        ),
        ArchiveFailure::LimitExceeded => ServiceError::new(
            "profile_archive_limit_exceeded",
            "The profile backup exceeds a supported size or item limit.",
            false,
        ),
        ArchiveFailure::UnsupportedSchema => ServiceError::new(
            "profile_archive_schema_unsupported",
            "This profile backup was created by a newer OpenChat database schema.",
            false,
        ),
    }
}

pub(crate) fn export(
    storage: &AppStorage,
    path: &str,
    passphrase: Zeroizing<String>,
) -> Result<Value, ServiceError> {
    validate_passphrase(&passphrase).map_err(service_error)?;
    export_inner(storage, Path::new(path), passphrase.as_str()).map_err(service_error)
}

pub(crate) fn prepare_restore(
    storage: &AppStorage,
    path: &str,
    passphrase: Zeroizing<String>,
    current_chat_schema_version: i64,
) -> Result<Value, ServiceError> {
    validate_passphrase(&passphrase).map_err(service_error)?;
    if !(1..=100).contains(&current_chat_schema_version) {
        return Err(service_error(ArchiveFailure::InvalidRequest));
    }
    prepare_restore_inner(
        storage,
        Path::new(path),
        passphrase.as_str(),
        current_chat_schema_version,
    )
    .map_err(service_error)
}

fn export_inner(
    storage: &AppStorage,
    target: &Path,
    passphrase: &str,
) -> Result<Value, ArchiveFailure> {
    let target = validate_output_path(target)?;
    ensure_destination_is_new(&target)?;
    let source = storage.connect().map_err(|_| ArchiveFailure::Storage)?;
    storage::verify_database_integrity(&source).map_err(|_| ArchiveFailure::Storage)?;
    let archive_temp = TemporaryDirectory::create(
        &storage.root().join("cache").join(PROFILE_ARCHIVE_ROOT),
        "export",
    )?;
    let database_snapshot_path = archive_temp.path().join("openchat.sqlite3");
    storage::create_verified_database_snapshot(&source, &database_snapshot_path)
        .map_err(|_| ArchiveFailure::Storage)?;
    drop(source);
    strip_workspace_file_changes(&database_snapshot_path)?;

    let database = Connection::open_with_flags(
        &database_snapshot_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|_| ArchiveFailure::Storage)?;
    storage::verify_database_integrity(&database).map_err(|_| ArchiveFailure::Storage)?;
    verify_foreign_keys(&database)?;
    let chat_schema_version = database
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(|_| ArchiveFailure::Storage)?;
    let backend_schema_version = read_backend_schema_version(&database)?;
    let conversation_count = count_rows(&database, "conversations")?;
    let message_count = count_rows(&database, "messages")?;
    let database_record = file_record(DATABASE_PATH, &database_snapshot_path, MAX_DATABASE_BYTES)?;
    let attachments = collect_attachments(&database, storage.root())?;
    drop(database);

    let attachment_bytes = attachments
        .iter()
        .try_fold(0_u64, |total, attachment| {
            total.checked_add(attachment.record.size_bytes)
        })
        .ok_or(ArchiveFailure::LimitExceeded)?;
    if attachments.len() > MAX_ATTACHMENTS || attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(ArchiveFailure::LimitExceeded);
    }
    let manifest = ProfileManifest {
        format: ARCHIVE_FORMAT.to_owned(),
        version: ARCHIVE_VERSION,
        created_at_unix_ms: unix_time_millis().map_err(|_| ArchiveFailure::Storage)?,
        chat_schema_version,
        backend_schema_version,
        conversation_count,
        message_count,
        database: database_record.clone(),
        attachments: attachments
            .iter()
            .map(|attachment| attachment.record.clone())
            .collect(),
    };
    validate_manifest(&manifest)?;
    let manifest_json = serde_json::to_vec(&manifest).map_err(|_| ArchiveFailure::Storage)?;
    if manifest_json.len() as u64 > MAX_MANIFEST_BYTES {
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
        &database_record,
        &database_snapshot_path,
        &attachments,
        passphrase,
    )?;
    ensure_destination_is_new(&target)?;
    fs::rename(partial.path.as_path(), &target).map_err(map_destination_error)?;
    partial.disarm();
    Ok(archive_summary(&manifest, false))
}

fn strip_workspace_file_changes(database_path: &Path) -> Result<(), ArchiveFailure> {
    let database = Connection::open(database_path).map_err(|_| ArchiveFailure::Storage)?;
    let mut present = 0;
    for table in [
        "conversation_file_changes",
        "file_change_blobs",
        "file_change_legacy_imports",
    ] {
        present += database
            .query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
                )",
                [table],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| ArchiveFailure::Storage)? as usize;
    }
    if present == 0 {
        return Ok(());
    }
    if present != 3 {
        return Err(ArchiveFailure::Storage);
    }
    database
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             BEGIN IMMEDIATE;
             DELETE FROM conversation_file_changes;
             DELETE FROM file_change_legacy_imports;
             DELETE FROM file_change_blobs;
             COMMIT;",
        )
        .map_err(|_| ArchiveFailure::Storage)
}

fn prepare_restore_inner(
    storage: &AppStorage,
    archive_path: &Path,
    passphrase: &str,
    current_chat_schema_version: i64,
) -> Result<Value, ArchiveFailure> {
    let root = storage.root();
    let restore_root = root.join("cache").join(RESTORE_ROOT);
    fs::create_dir_all(&restore_root).map_err(|_| ArchiveFailure::Storage)?;
    let _lock = RESTORE_OPERATION_LOCK
        .get_or_init(|| Mutex::new(()))
        .try_lock()
        .map_err(|_| ArchiveFailure::Busy)?;
    if restore_pending_path(&restore_root).exists()
        || restore_journal_paths(&restore_root)
            .iter()
            .any(|path| path.exists())
    {
        return Err(ArchiveFailure::Conflict);
    }
    let mut stage = TemporaryDirectory::create(&restore_root, "stage")?;
    let stage_id = stage
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("stage-"))
        .filter(|id| safe_identifier(id))
        .ok_or(ArchiveFailure::Storage)?
        .to_owned();
    let result = read_archive_to_stage(
        archive_path,
        passphrase,
        stage.path(),
        current_chat_schema_version,
    );
    let manifest = result?;
    let manifest_path = stage.path().join(MANIFEST_PATH);
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(|_| ArchiveFailure::Storage)?;
    write_new_bytes(&manifest_path, &manifest_bytes)?;
    let marker = RestoreMarker {
        version: ARCHIVE_VERSION,
        stage_id,
        chat_schema_version: current_chat_schema_version,
    };
    write_new_json(&restore_pending_path(&restore_root), &marker)?;
    stage.preserve();
    Ok(archive_summary(&manifest, true))
}

fn collect_attachments(
    database: &Connection,
    storage_root: &Path,
) -> Result<Vec<AttachmentSource>, ArchiveFailure> {
    let attachments_root = storage_root.join("attachments");
    let canonical_root = match fs::canonicalize(&attachments_root) {
        Ok(root) => root,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(ArchiveFailure::Storage),
    };
    let mut statement = database
        .prepare("SELECT conversation_id, id, content FROM messages ORDER BY rowid")
        .map_err(|_| ArchiveFailure::Storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| ArchiveFailure::Storage)?;
    let mut paths = BTreeMap::new();
    for row in rows {
        let (conversation_id, message_id, content) = row.map_err(|_| ArchiveFailure::Storage)?;
        let metadata =
            message_attachment_metadata(&content).map_err(|_| ArchiveFailure::Storage)?;
        for attachment in metadata {
            let archive_path = format!(
                "attachments/{conversation_id}/{message_id}/{}.data",
                attachment.id
            );
            if paths.contains_key(&archive_path) {
                return Err(ArchiveFailure::InvalidArchive);
            }
            let source = attachments_root
                .join(&conversation_id)
                .join(&message_id)
                .join(format!("{}.data", attachment.id));
            let file_metadata =
                fs::symlink_metadata(&source).map_err(|_| ArchiveFailure::Storage)?;
            if !file_metadata.is_file() || file_metadata.file_type().is_symlink() {
                return Err(ArchiveFailure::Storage);
            }
            let canonical = fs::canonicalize(&source).map_err(|_| ArchiveFailure::Storage)?;
            if !canonical.starts_with(&canonical_root)
                || file_metadata.len() != attachment.size_bytes as u64
            {
                return Err(ArchiveFailure::Storage);
            }
            let bytes = fs::read(&canonical).map_err(|_| ArchiveFailure::Storage)?;
            validate_portable_attachment_content(&attachment, &bytes)
                .map_err(|_| ArchiveFailure::Storage)?;
            let record = FileRecord {
                path: archive_path.clone(),
                size_bytes: file_metadata.len(),
                sha256: digest_hex(&Sha256::digest(&bytes)),
            };
            paths.insert(
                archive_path,
                AttachmentSource {
                    record,
                    path: canonical,
                },
            );
            if paths.len() > MAX_ATTACHMENTS {
                return Err(ArchiveFailure::LimitExceeded);
            }
        }
    }
    Ok(paths.into_values().collect())
}

fn write_archive(
    output: File,
    manifest_json: &[u8],
    database_record: &FileRecord,
    database_snapshot_path: &Path,
    attachments: &[AttachmentSource],
    passphrase: &str,
) -> Result<(), ArchiveFailure> {
    let secret = SecretString::from(passphrase.to_owned());
    let encrypted = Encryptor::with_user_passphrase(secret)
        .wrap_output(output)
        .map_err(|_| ArchiveFailure::Storage)?;
    let mut archive = Builder::new(encrypted);
    append_bytes(&mut archive, MANIFEST_PATH, manifest_json)?;
    append_file(&mut archive, database_record, database_snapshot_path)?;
    for attachment in attachments {
        append_file(&mut archive, &attachment.record, &attachment.path)?;
    }
    let encrypted = archive.into_inner().map_err(|_| ArchiveFailure::Storage)?;
    let output = encrypted.finish().map_err(|_| ArchiveFailure::Storage)?;
    output.sync_all().map_err(|_| ArchiveFailure::Storage)
}

fn append_bytes<W: Write>(
    archive: &mut Builder<W>,
    path: &str,
    bytes: &[u8],
) -> Result<(), ArchiveFailure> {
    let mut reader = bytes;
    append_reader(
        archive,
        path,
        u64::try_from(bytes.len()).map_err(|_| ArchiveFailure::LimitExceeded)?,
        &mut reader,
    )
}

fn append_file<W: Write>(
    archive: &mut Builder<W>,
    record: &FileRecord,
    path: &Path,
) -> Result<(), ArchiveFailure> {
    let file = File::open(path).map_err(|_| ArchiveFailure::Storage)?;
    let mut reader = HashingReader::new(BufReader::new(file));
    append_reader(archive, &record.path, record.size_bytes, &mut reader)?;
    if reader.bytes_read != record.size_bytes
        || digest_hex(&reader.hasher.finalize()) != record.sha256
    {
        return Err(ArchiveFailure::Storage);
    }
    Ok(())
}

fn append_reader<W: Write, R: Read>(
    archive: &mut Builder<W>,
    path: &str,
    size: u64,
    reader: &mut R,
) -> Result<(), ArchiveFailure> {
    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Regular);
    header.set_size(size);
    header.set_mode(0o600);
    header.set_cksum();
    archive
        .append_data(&mut header, path, reader)
        .map_err(|_| ArchiveFailure::Storage)
}

struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
    bytes_read: u64,
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
        let count = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..count]);
        self.bytes_read = self.bytes_read.saturating_add(count as u64);
        Ok(count)
    }
}

fn read_archive_to_stage(
    path: &Path,
    passphrase: &str,
    stage: &Path,
    current_chat_schema_version: i64,
) -> Result<ProfileManifest, ArchiveFailure> {
    let file = validate_input_path(path)?;
    let decryptor =
        Decryptor::new(BufReader::new(file)).map_err(|_| ArchiveFailure::InvalidArchive)?;
    if !decryptor.is_scrypt() {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let mut identity = Identity::new(SecretString::from(passphrase.to_owned()));
    identity.set_max_work_factor(MAX_AGE_SCRYPT_LOG_N);
    let decrypted = decryptor
        .decrypt(iter::once(&identity as &dyn age::Identity))
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    let limited = decrypted.take(MAX_ARCHIVE_BYTES.saturating_add(1));
    let mut archive = Archive::new(limited);
    let mut entries = archive
        .entries()
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    let first = entries
        .next()
        .transpose()
        .map_err(|_| ArchiveFailure::InvalidArchive)?
        .ok_or(ArchiveFailure::InvalidArchive)?;
    validate_tar_entry(&first, MANIFEST_PATH)?;
    let manifest: ProfileManifest = read_entry_json(first, MAX_MANIFEST_BYTES)?;
    validate_manifest(&manifest)?;
    if manifest.chat_schema_version > current_chat_schema_version
        || manifest.backend_schema_version > storage::backend_schema_version()
    {
        return Err(ArchiveFailure::UnsupportedSchema);
    }
    let database_entry = entries
        .next()
        .transpose()
        .map_err(|_| ArchiveFailure::InvalidArchive)?
        .ok_or(ArchiveFailure::InvalidArchive)?;
    validate_tar_entry(&database_entry, DATABASE_PATH)?;
    if database_entry.size() != manifest.database.size_bytes {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let database_file = create_stage_file(stage, DATABASE_PATH)?;
    copy_verified_entry(database_entry, database_file, &manifest.database)?;

    let mut attachment_records = manifest
        .attachments
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    fs::create_dir(stage.join("attachments")).map_err(|_| ArchiveFailure::Storage)?;
    for entry in entries.by_ref() {
        let entry = entry.map_err(|_| ArchiveFailure::InvalidArchive)?;
        let path = tar_path(&entry)?;
        let record = attachment_records
            .remove(path.as_str())
            .ok_or(ArchiveFailure::InvalidArchive)?;
        validate_attachment_path(&path)?;
        validate_tar_entry(&entry, &record.path)?;
        if entry.size() != record.size_bytes {
            return Err(ArchiveFailure::InvalidArchive);
        }
        let file = create_stage_file(stage, &path)?;
        copy_verified_entry(entry, file, record)?;
    }
    if !attachment_records.is_empty() {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let mut decrypted = archive.into_inner();
    let mut drain = io::sink();
    let drained =
        io::copy(&mut decrypted, &mut drain).map_err(|_| ArchiveFailure::InvalidArchive)?;
    let consumed = MAX_ARCHIVE_BYTES
        .saturating_add(1)
        .saturating_sub(decrypted.limit());
    if consumed > MAX_ARCHIVE_BYTES
        || drained > MAX_ARCHIVE_BYTES
        || consumed.saturating_add(drained) > MAX_ARCHIVE_BYTES
    {
        return Err(ArchiveFailure::LimitExceeded);
    }
    validate_staged_profile(stage, &manifest, current_chat_schema_version)?;
    Ok(manifest)
}

fn validate_staged_profile(
    stage: &Path,
    manifest: &ProfileManifest,
    current_chat_schema_version: i64,
) -> Result<(), ArchiveFailure> {
    let database_path = stage.join(DATABASE_PATH);
    let metadata = fs::metadata(&database_path).map_err(|_| ArchiveFailure::Storage)?;
    if metadata.len() != manifest.database.size_bytes
        || digest_file(&database_path)? != manifest.database.sha256
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let database =
        Connection::open_with_flags(&database_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| ArchiveFailure::InvalidArchive)?;
    storage::verify_database_integrity(&database).map_err(|_| ArchiveFailure::InvalidArchive)?;
    verify_foreign_keys(&database)?;
    let chat_schema_version = database
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    let backend_schema_version = read_backend_schema_version(&database)?;
    if chat_schema_version != manifest.chat_schema_version
        || backend_schema_version != manifest.backend_schema_version
        || chat_schema_version > current_chat_schema_version
        || backend_schema_version > storage::backend_schema_version()
        || count_rows(&database, "conversations")? != manifest.conversation_count
        || count_rows(&database, "messages")? != manifest.message_count
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let expected_attachments = expected_attachments(&database)?;
    if expected_attachments.len() != manifest.attachments.len()
        || manifest
            .attachments
            .iter()
            .any(|record| !expected_attachments.contains_key(&record.path))
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    for record in &manifest.attachments {
        validate_attachment_path(&record.path)?;
        let path = stage.join(record.path.as_str());
        let file_metadata = fs::symlink_metadata(&path).map_err(|_| ArchiveFailure::Storage)?;
        if file_metadata.file_type().is_symlink()
            || !file_metadata.is_file()
            || file_metadata.len() != record.size_bytes
            || digest_file(&path)? != record.sha256
        {
            return Err(ArchiveFailure::InvalidArchive);
        }
        let attachment = expected_attachments
            .get(&record.path)
            .ok_or(ArchiveFailure::InvalidArchive)?;
        if attachment.size_bytes as u64 != record.size_bytes {
            return Err(ArchiveFailure::InvalidArchive);
        }
        let bytes = fs::read(&path).map_err(|_| ArchiveFailure::Storage)?;
        validate_portable_attachment_content(attachment, &bytes)
            .map_err(|_| ArchiveFailure::InvalidArchive)?;
    }
    Ok(())
}

fn copy_verified_entry<R: Read>(
    mut entry: tar::Entry<'_, R>,
    mut output: File,
    record: &FileRecord,
) -> Result<(), ArchiveFailure> {
    let mut hasher = Sha256::new();
    let mut bytes_written = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = entry
            .read(&mut buffer)
            .map_err(|_| ArchiveFailure::InvalidArchive)?;
        if count == 0 {
            break;
        }
        bytes_written = bytes_written
            .checked_add(count as u64)
            .ok_or(ArchiveFailure::LimitExceeded)?;
        if bytes_written > record.size_bytes {
            return Err(ArchiveFailure::InvalidArchive);
        }
        hasher.update(&buffer[..count]);
        output
            .write_all(&buffer[..count])
            .map_err(|_| ArchiveFailure::Storage)?;
    }
    if bytes_written != record.size_bytes || digest_hex(&hasher.finalize()) != record.sha256 {
        return Err(ArchiveFailure::InvalidArchive);
    }
    output.sync_all().map_err(|_| ArchiveFailure::Storage)
}

fn create_stage_file(stage: &Path, archive_path: &str) -> Result<File, ArchiveFailure> {
    let relative = safe_archive_path(archive_path)?;
    let output = stage.join(relative);
    let parent = output.parent().ok_or(ArchiveFailure::InvalidArchive)?;
    fs::create_dir_all(parent).map_err(|_| ArchiveFailure::Storage)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|_| ArchiveFailure::Storage)
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
    safe_archive_path(path)?;
    Ok(path.to_owned())
}

fn safe_archive_path(value: &str) -> Result<PathBuf, ArchiveFailure> {
    if value.is_empty() || value.contains('\\') || value.starts_with('/') {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let path = Path::new(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    Ok(path.to_path_buf())
}

fn validate_attachment_path(value: &str) -> Result<(), ArchiveFailure> {
    safe_archive_path(value)?;
    let mut parts = value.split('/');
    let valid = matches!(parts.next(), Some("attachments"))
        && parts.next().is_some_and(safe_identifier)
        && parts.next().is_some_and(safe_identifier)
        && parts.next().is_some_and(safe_attachment_file_name)
        && parts.next().is_none();
    if valid {
        Ok(())
    } else {
        Err(ArchiveFailure::InvalidArchive)
    }
}

fn read_entry_json<T: DeserializeOwned, R: Read>(
    entry: tar::Entry<'_, R>,
    maximum: u64,
) -> Result<T, ArchiveFailure> {
    if entry.size() > maximum {
        return Err(ArchiveFailure::LimitExceeded);
    }
    let size = entry.size();
    let mut bytes =
        Vec::with_capacity(usize::try_from(size).map_err(|_| ArchiveFailure::LimitExceeded)?);
    entry
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    if bytes.len() as u64 != size {
        return Err(ArchiveFailure::InvalidArchive);
    }
    serde_json::from_slice(&bytes).map_err(|_| ArchiveFailure::InvalidArchive)
}

fn validate_manifest(manifest: &ProfileManifest) -> Result<(), ArchiveFailure> {
    if manifest.format != ARCHIVE_FORMAT
        || manifest.version != ARCHIVE_VERSION
        || manifest.created_at_unix_ms <= 0
        || !(1..=100).contains(&manifest.chat_schema_version)
        || manifest.backend_schema_version < 1
        || manifest.conversation_count > 100_000
        || manifest.message_count > 2_000_000
        || manifest.attachments.len() > MAX_ATTACHMENTS
        || manifest.database.path != DATABASE_PATH
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    validate_file_record(&manifest.database, MAX_DATABASE_BYTES)?;
    let mut paths = HashSet::with_capacity(manifest.attachments.len());
    let mut total_bytes = 0_u64;
    for attachment in &manifest.attachments {
        validate_attachment_path(&attachment.path)?;
        validate_file_record(attachment, MAX_ATTACHMENT_BYTES)?;
        if !paths.insert(attachment.path.as_str()) {
            return Err(ArchiveFailure::InvalidArchive);
        }
        total_bytes = total_bytes
            .checked_add(attachment.size_bytes)
            .ok_or(ArchiveFailure::LimitExceeded)?;
        if total_bytes > MAX_ATTACHMENT_BYTES {
            return Err(ArchiveFailure::LimitExceeded);
        }
    }
    Ok(())
}

fn validate_file_record(record: &FileRecord, maximum: u64) -> Result<(), ArchiveFailure> {
    if record.size_bytes == 0
        || record.size_bytes > maximum
        || record.sha256.len() != 64
        || !record
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    Ok(())
}

fn file_record(path: &str, file_path: &Path, maximum: u64) -> Result<FileRecord, ArchiveFailure> {
    let metadata = fs::symlink_metadata(file_path).map_err(|_| ArchiveFailure::Storage)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > maximum
    {
        return Err(ArchiveFailure::LimitExceeded);
    }
    Ok(FileRecord {
        path: path.to_owned(),
        size_bytes: metadata.len(),
        sha256: digest_file(file_path)?,
    })
}

fn digest_file(path: &Path) -> Result<String, ArchiveFailure> {
    let mut file = File::open(path).map_err(|_| ArchiveFailure::Storage)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| ArchiveFailure::Storage)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(digest_hex(&hasher.finalize()))
}

fn verify_foreign_keys(database: &Connection) -> Result<(), ArchiveFailure> {
    let mut statement = database
        .prepare("PRAGMA foreign_key_check")
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    let mut rows = statement
        .query([])
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    if rows
        .next()
        .map_err(|_| ArchiveFailure::InvalidArchive)?
        .is_some()
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    Ok(())
}

fn read_backend_schema_version(database: &Connection) -> Result<i64, ArchiveFailure> {
    let exists = database
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type='table' AND name='openchat_backend_migrations')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| ArchiveFailure::Storage)?;
    if !exists {
        return Ok(0);
    }
    database
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM openchat_backend_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ArchiveFailure::Storage)
}

fn count_rows(database: &Connection, table: &str) -> Result<u64, ArchiveFailure> {
    let query = match table {
        "conversations" => "SELECT COUNT(*) FROM conversations",
        "messages" => "SELECT COUNT(*) FROM messages",
        _ => return Err(ArchiveFailure::InvalidRequest),
    };
    let count: i64 = database
        .query_row(query, [], |row| row.get(0))
        .map_err(|_| ArchiveFailure::Storage)?;
    u64::try_from(count).map_err(|_| ArchiveFailure::Storage)
}

fn archive_summary(manifest: &ProfileManifest, requires_restart: bool) -> Value {
    json!({
        "conversationCount": manifest.conversation_count,
        "messageCount": manifest.message_count,
        "attachmentCount": manifest.attachments.len(),
        "attachmentBytes": manifest.attachments.iter().map(|item| item.size_bytes).sum::<u64>(),
        "requiresRestart": requires_restart,
    })
}

fn validate_input_path(path: &Path) -> Result<File, ArchiveFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ArchiveFailure::NotFound
        } else {
            ArchiveFailure::Storage
        }
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_ARCHIVE_BYTES
    {
        return Err(ArchiveFailure::LimitExceeded);
    }
    File::open(path).map_err(ArchiveFailure::from)
}

fn validate_output_path(path: &Path) -> Result<PathBuf, ArchiveFailure> {
    if path.extension().and_then(|value| value.to_str()) != Some(ARCHIVE_EXTENSION) {
        return Err(ArchiveFailure::InvalidRequest);
    }
    let file_name = path.file_name().ok_or(ArchiveFailure::InvalidRequest)?;
    let parent = path.parent().ok_or(ArchiveFailure::InvalidRequest)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| ArchiveFailure::NotFound)?;
    Ok(canonical_parent.join(file_name))
}

fn validate_passphrase(passphrase: &str) -> Result<(), ArchiveFailure> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARACTERS
        || passphrase.len() > MAX_PASSPHRASE_BYTES
        || passphrase.contains('\0')
    {
        Err(ArchiveFailure::InvalidPassphrase)
    } else {
        Ok(())
    }
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn safe_attachment_file_name(value: &str) -> bool {
    value.strip_suffix(".data").is_some_and(safe_identifier)
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

fn restore_root(root: &Path) -> PathBuf {
    root.join("cache").join(RESTORE_ROOT)
}

fn restore_pending_path(root: &Path) -> PathBuf {
    root.join(RESTORE_PENDING_FILE)
}

fn restore_journal_path(root: &Path, phase: RestorePhase) -> PathBuf {
    let file_name = match phase {
        RestorePhase::Applying => RESTORE_APPLYING_FILE,
        RestorePhase::AwaitingValidation => RESTORE_AWAITING_FILE,
        RestorePhase::Committed => RESTORE_COMMITTED_FILE,
    };
    root.join(file_name)
}

fn restore_journal_paths(root: &Path) -> [PathBuf; 3] {
    [
        restore_journal_path(root, RestorePhase::Applying),
        restore_journal_path(root, RestorePhase::AwaitingValidation),
        restore_journal_path(root, RestorePhase::Committed),
    ]
}

fn restore_stage_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("stage-{id}"))
}

fn recovery_path(root: &Path, id: &str) -> PathBuf {
    root.join("backups").join(RECOVERY_DIRECTORY).join(id)
}

fn write_new_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ArchiveFailure> {
    let bytes = serde_json::to_vec(value).map_err(|_| ArchiveFailure::Storage)?;
    write_new_bytes(path, &bytes)
}

pub(crate) fn activate_pending_restore(root: &Path) -> rusqlite::Result<bool> {
    recover_unconfirmed_restore(root)?;
    let restore_root = restore_root(root);
    let marker_path = restore_pending_path(&restore_root);
    if !marker_path.exists() {
        cleanup_orphaned_stages(&restore_root, None).map_err(io_to_sql_error)?;
        return Ok(false);
    }
    let marker = read_marker(&marker_path)?;
    cleanup_orphaned_stages(&restore_root, Some(&marker.stage_id)).map_err(io_to_sql_error)?;
    let stage = restore_stage_path(&restore_root, &marker.stage_id);
    let manifest: ProfileManifest = read_json(&stage.join(MANIFEST_PATH))?;
    validate_manifest(&manifest).map_err(to_sql_error)?;
    validate_staged_profile(&stage, &manifest, marker.chat_schema_version).map_err(to_sql_error)?;

    let recovery_id = format!(
        "{}-{}",
        unix_time_millis().map_err(io_to_sql_error)?,
        Uuid::new_v4().simple()
    );
    let recovery = recovery_path(root, &recovery_id);
    let recovery_parent = recovery.parent().ok_or(rusqlite::Error::InvalidQuery)?;
    fs::create_dir_all(recovery_parent).map_err(io_to_sql_error)?;
    fs::create_dir(&recovery).map_err(io_to_sql_error)?;
    let database_directory = root.join("db");
    let attachments_directory = root.join("attachments");
    let journal = RestoreJournal {
        version: ARCHIVE_VERSION,
        stage_id: marker.stage_id,
        recovery_id,
        phase: RestorePhase::Applying,
        had_database_directory: database_directory.is_dir(),
        had_attachments_directory: attachments_directory.is_dir(),
    };
    write_new_json(
        &restore_journal_path(&restore_root, RestorePhase::Applying),
        &journal,
    )
    .map_err(to_sql_error)?;
    let apply_result = apply_directory_swap(root, &stage, &recovery, &journal).and_then(|()| {
        let journal = RestoreJournal {
            phase: RestorePhase::AwaitingValidation,
            ..journal
        };
        write_new_json(
            &restore_journal_path(&restore_root, RestorePhase::AwaitingValidation),
            &journal,
        )?;
        fs::remove_file(restore_journal_path(&restore_root, RestorePhase::Applying))
            .map_err(|_| ArchiveFailure::Storage)
    });
    if let Err(error) = apply_result {
        recover_unconfirmed_restore(root).map_err(|_| to_sql_error(error))?;
        return Err(to_sql_error(error));
    }
    Ok(true)
}

fn apply_directory_swap(
    root: &Path,
    stage: &Path,
    recovery: &Path,
    journal: &RestoreJournal,
) -> Result<(), ArchiveFailure> {
    let database_directory = root.join("db");
    let attachments_directory = root.join("attachments");
    if journal.had_database_directory {
        fs::rename(&database_directory, recovery.join("db"))
            .map_err(|_| ArchiveFailure::Storage)?;
    }
    if journal.had_attachments_directory {
        fs::rename(&attachments_directory, recovery.join("attachments"))
            .map_err(|_| ArchiveFailure::Storage)?;
    }
    fs::create_dir(&database_directory).map_err(|_| ArchiveFailure::Storage)?;
    fs::rename(
        stage.join(DATABASE_PATH),
        database_directory.join("openchat.sqlite3"),
    )
    .map_err(|_| ArchiveFailure::Storage)?;
    fs::rename(stage.join("attachments"), &attachments_directory)
        .map_err(|_| ArchiveFailure::Storage)?;
    Ok(())
}

pub(crate) fn recover_unconfirmed_restore(root: &Path) -> rusqlite::Result<()> {
    let restore_root = restore_root(root);
    let Some((_, journal)) = read_restore_journal(&restore_root)? else {
        return Ok(());
    };
    if journal.version != ARCHIVE_VERSION
        || !safe_identifier(&journal.stage_id)
        || !safe_recovery_id(&journal.recovery_id)
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if journal.phase == RestorePhase::Committed {
        cleanup_restore_stage(&restore_root, &journal.stage_id).map_err(io_to_sql_error)?;
        remove_restore_journals(&restore_root).map_err(io_to_sql_error)?;
        return Ok(());
    }
    rollback_directory_swap(root, &restore_root, &journal).map_err(io_to_sql_error)?;
    remove_restore_journals(&restore_root).map_err(io_to_sql_error)?;
    Ok(())
}

fn rollback_directory_swap(
    root: &Path,
    restore_root: &Path,
    journal: &RestoreJournal,
) -> io::Result<()> {
    let recovery = recovery_path(root, &journal.recovery_id);
    let database_directory = root.join("db");
    let attachments_directory = root.join("attachments");
    let had_recovered_database = recovery.join("db").is_dir();
    if had_recovered_database {
        if database_directory.exists() {
            fs::remove_dir_all(&database_directory)?;
        }
        fs::rename(recovery.join("db"), &database_directory)?;
    } else if !journal.had_database_directory && database_directory.exists() {
        fs::remove_dir_all(&database_directory)?;
    }
    let had_recovered_attachments = recovery.join("attachments").is_dir();
    if had_recovered_attachments {
        if attachments_directory.exists() {
            fs::remove_dir_all(&attachments_directory)?;
        }
        fs::rename(recovery.join("attachments"), &attachments_directory)?;
    } else if !journal.had_attachments_directory && attachments_directory.exists() {
        fs::remove_dir_all(&attachments_directory)?;
    }
    cleanup_restore_stage(restore_root, &journal.stage_id)?;
    let _ = fs::remove_dir(recovery);
    Ok(())
}

fn cleanup_restore_stage(restore_root: &Path, stage_id: &str) -> io::Result<()> {
    let stage = restore_stage_path(restore_root, stage_id);
    if stage.exists() {
        fs::remove_dir_all(stage)?;
    }
    let marker = restore_pending_path(restore_root);
    if marker.exists() {
        fs::remove_file(marker)?;
    }
    Ok(())
}

fn cleanup_orphaned_stages(restore_root: &Path, retained_stage_id: Option<&str>) -> io::Result<()> {
    let entries = match fs::read_dir(restore_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(stage_id) = file_name
            .to_str()
            .and_then(|name| name.strip_prefix("stage-"))
        else {
            continue;
        };
        if !safe_identifier(stage_id) || Some(stage_id) == retained_stage_id {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() && !file_type.is_symlink() {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

pub(crate) fn commit_pending_restore(root: &Path) -> rusqlite::Result<()> {
    let restore_root = restore_root(root);
    let Some((journal_path, journal)) = read_restore_journal(&restore_root)? else {
        return Ok(());
    };
    if journal.phase != RestorePhase::AwaitingValidation {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let stage_id = journal.stage_id.clone();
    write_new_json(
        &restore_journal_path(&restore_root, RestorePhase::Committed),
        &RestoreJournal {
            phase: RestorePhase::Committed,
            ..journal
        },
    )
    .map_err(to_sql_error)?;
    fs::remove_file(journal_path).map_err(io_to_sql_error)?;
    cleanup_restore_stage(&restore_root, &stage_id).map_err(io_to_sql_error)?;
    remove_restore_journals(&restore_root).map_err(io_to_sql_error)
}

fn read_marker(path: &Path) -> rusqlite::Result<RestoreMarker> {
    let marker = read_json::<RestoreMarker>(path)?;
    if marker.version != ARCHIVE_VERSION
        || !safe_identifier(&marker.stage_id)
        || !(1..=100).contains(&marker.chat_schema_version)
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(marker)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> rusqlite::Result<T> {
    let metadata = fs::symlink_metadata(path).map_err(io_to_sql_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let bytes = fs::read(path).map_err(io_to_sql_error)?;
    serde_json::from_slice(&bytes).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn read_restore_journal(root: &Path) -> rusqlite::Result<Option<(PathBuf, RestoreJournal)>> {
    for phase in [
        RestorePhase::Committed,
        RestorePhase::Applying,
        RestorePhase::AwaitingValidation,
    ] {
        let path = restore_journal_path(root, phase);
        if !path.exists() {
            continue;
        }
        let journal = read_json::<RestoreJournal>(&path)?;
        if journal.version != ARCHIVE_VERSION
            || journal.phase != phase
            || !safe_identifier(&journal.stage_id)
            || !safe_recovery_id(&journal.recovery_id)
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        return Ok(Some((path, journal)));
    }
    Ok(None)
}

fn remove_restore_journals(root: &Path) -> io::Result<()> {
    for path in restore_journal_paths(root) {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn safe_recovery_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-'))
}

fn to_sql_error(_: ArchiveFailure) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn write_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), ArchiveFailure> {
    ensure_destination_is_new(path)?;
    let parent = path.parent().ok_or(ArchiveFailure::Storage)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ArchiveFailure::Storage)?;
    let temporary_path = parent.join(format!(".{name}.{}.tmp", Uuid::new_v4().simple()));
    let mut temporary = PartialOutput::new(temporary_path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary.path)
        .map_err(ArchiveFailure::from)?;
    file.write_all(bytes).map_err(|_| ArchiveFailure::Storage)?;
    file.sync_all().map_err(|_| ArchiveFailure::Storage)?;
    drop(file);
    ensure_destination_is_new(path)?;
    fs::rename(&temporary.path, path).map_err(map_destination_error)?;
    temporary.disarm();
    Ok(())
}

fn ensure_destination_is_new(path: &Path) -> Result<(), ArchiveFailure> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(ArchiveFailure::Conflict),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ArchiveFailure::Storage),
    }
}

fn map_destination_error(error: io::Error) -> ArchiveFailure {
    if error.kind() == io::ErrorKind::AlreadyExists {
        ArchiveFailure::Conflict
    } else {
        ArchiveFailure::Storage
    }
}

fn expected_attachments(
    database: &Connection,
) -> Result<BTreeMap<String, AttachmentMetadata>, ArchiveFailure> {
    let mut statement = database
        .prepare("SELECT conversation_id, id, content FROM messages ORDER BY rowid")
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    let mut paths = BTreeMap::new();
    for row in rows {
        let (conversation_id, message_id, content) =
            row.map_err(|_| ArchiveFailure::InvalidArchive)?;
        let metadata =
            message_attachment_metadata(&content).map_err(|_| ArchiveFailure::InvalidArchive)?;
        for attachment in metadata {
            let path = format!(
                "attachments/{conversation_id}/{message_id}/{}.data",
                attachment.id
            );
            if paths.insert(path, attachment).is_some() {
                return Err(ArchiveFailure::InvalidArchive);
            }
        }
    }
    Ok(paths)
}

fn io_to_sql_error(error: io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

mod storage {
    pub(super) use crate::storage::{
        backend_schema_version, create_verified_database_snapshot, verify_database_integrity,
    };
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use rusqlite::{Connection, params};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::sync::Mutex;
    use uuid::Uuid;

    use super::{export, prepare_restore};
    use crate::storage::AppStorage;

    const CHAT_SCHEMA_VERSION: i64 = 11;
    const PASSPHRASE: &str = "profile-archive-test-passphrase";
    static RESTORE_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn encrypted_profile_backup_restores_database_and_attachments_safely() {
        let _guard = RESTORE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let source_directory = TestDirectory::new("source");
        let source = seed_profile(&source_directory.0, "source-chat", true);
        let local_snapshot_path = add_workspace_file_change(&source);
        assert!(local_snapshot_path.is_file());
        let archive_directory = source_directory.0.join("exports");
        fs::create_dir(&archive_directory).expect("create export directory");
        let archive_path = archive_directory.join("profile.openchatprofilebackup");
        let export_result = export(
            &source,
            archive_path.to_str().expect("archive path is UTF-8"),
            PASSPHRASE.to_owned().into(),
        )
        .expect("export encrypted profile");
        assert_eq!(export_result["conversationCount"], 1);
        assert_eq!(export_result["attachmentCount"], 1);
        assert_eq!(export_result["requiresRestart"], false);
        assert_eq!(workspace_file_change_count(&source), 1);
        assert_eq!(workspace_file_blob_count(&source), 1);
        assert!(local_snapshot_path.is_file());

        let destination_directory = TestDirectory::new("destination");
        let destination = seed_profile(&destination_directory.0, "existing-chat", true);
        let invalid_passphrase = prepare_restore(
            &destination,
            archive_path.to_str().expect("archive path is UTF-8"),
            "wrong-profile-passphrase".to_owned().into(),
            CHAT_SCHEMA_VERSION,
        )
        .expect_err("reject an incorrect passphrase");
        assert_eq!(invalid_passphrase.code, "profile_archive_invalid");
        assert_eq!(read_chat_count(&destination), 1);

        let restore_result = prepare_restore(
            &destination,
            archive_path.to_str().expect("archive path is UTF-8"),
            PASSPHRASE.to_owned().into(),
            CHAT_SCHEMA_VERSION,
        )
        .expect("validate and stage profile restore");
        assert_eq!(restore_result["requiresRestart"], true);
        assert_eq!(read_chat_count(&destination), 1);
        drop(destination);

        let restored = AppStorage::open_at(destination_directory.0.clone())
            .expect("activate staged profile during startup");
        assert_eq!(read_chat_count(&restored), 1);
        assert!(chat_exists(&restored, "source-chat"));
        assert!(!chat_exists(&restored, "existing-chat"));
        let attachment_path = destination_directory
            .0
            .join("attachments/source-chat/source-message/source-attachment.data");
        assert_eq!(
            fs::read(&attachment_path).expect("read restored attachment"),
            b"attachment payload"
        );
        restored
            .initialize_backend_schema()
            .expect("validate restored backend schema");
        assert_eq!(workspace_file_change_count(&restored), 0);
        assert_eq!(workspace_file_blob_count(&restored), 0);
        assert!(
            !destination_directory
                .0
                .join("attachments/snapshots")
                .exists()
        );
        restored
            .commit_pending_profile_restore()
            .expect("commit verified restore");

        let recovery_root = destination_directory.0.join("backups/profile-restores");
        let recovery = fs::read_dir(recovery_root)
            .expect("list preserved profiles")
            .next()
            .expect("the replaced profile is retained")
            .expect("read recovery entry")
            .path();
        assert!(recovery.join("db/openchat.sqlite3").is_file());
        assert!(
            recovery
                .join("attachments/existing-chat/source-message/source-attachment.data")
                .is_file()
        );
    }

    #[test]
    fn unconfirmed_profile_restore_rolls_back_on_the_next_startup() {
        let _guard = RESTORE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let source_directory = TestDirectory::new("rollback-source");
        let source = seed_profile(&source_directory.0, "source-chat", false);
        let archive_directory = source_directory.0.join("exports");
        fs::create_dir(&archive_directory).expect("create export directory");
        let archive_path = archive_directory.join("profile.openchatprofilebackup");
        export(
            &source,
            archive_path.to_str().expect("archive path is UTF-8"),
            PASSPHRASE.to_owned().into(),
        )
        .expect("export profile");

        let destination_directory = TestDirectory::new("rollback-destination");
        let destination = seed_profile(&destination_directory.0, "existing-chat", false);
        prepare_restore(
            &destination,
            archive_path.to_str().expect("archive path is UTF-8"),
            PASSPHRASE.to_owned().into(),
            CHAT_SCHEMA_VERSION,
        )
        .expect("stage profile restore");
        drop(destination);

        let unconfirmed =
            AppStorage::open_at(destination_directory.0.clone()).expect("activate staged profile");
        assert!(chat_exists(&unconfirmed, "source-chat"));
        drop(unconfirmed);

        let rolled_back = AppStorage::open_at(destination_directory.0.clone())
            .expect("roll back the unconfirmed restore");
        assert!(chat_exists(&rolled_back, "existing-chat"));
        assert!(!chat_exists(&rolled_back, "source-chat"));
        assert!(
            !destination_directory
                .0
                .join("cache/profile-restore/restore.pending")
                .exists()
        );
    }

    #[test]
    fn profile_export_does_not_overwrite_an_existing_archive() {
        let source_directory = TestDirectory::new("no-overwrite");
        let source = seed_profile(&source_directory.0, "source-chat", false);
        let archive_directory = source_directory.0.join("exports");
        fs::create_dir(&archive_directory).expect("create export directory");
        let archive_path = archive_directory.join("profile.openchatprofilebackup");
        fs::write(&archive_path, b"preserve existing data").expect("create existing file");

        let error = export(
            &source,
            archive_path.to_str().expect("archive path is UTF-8"),
            PASSPHRASE.to_owned().into(),
        )
        .expect_err("reject an existing archive path");

        assert_eq!(error.code, "profile_archive_conflict");
        assert_eq!(
            fs::read(&archive_path).expect("read existing archive"),
            b"preserve existing data"
        );
        assert_eq!(
            fs::read_dir(&archive_directory)
                .expect("list export directory")
                .count(),
            1,
            "failed exports should not leave partial archives"
        );
    }

    #[test]
    fn startup_removes_orphaned_restore_staging_directories() {
        let directory = TestDirectory::new("orphan-stage");
        let orphan = directory
            .0
            .join("cache/profile-restore/stage-abandoned-stage");
        fs::create_dir_all(&orphan).expect("create orphaned stage");
        fs::write(orphan.join("partial.bin"), b"incomplete").expect("write partial stage");

        let storage = AppStorage::open_at(directory.0.clone()).expect("open local storage");

        assert!(
            !orphan.exists(),
            "orphaned restore data should be reclaimed"
        );
        assert!(storage.database_path().is_file());
    }

    fn seed_profile(
        root: &std::path::Path,
        conversation_id: &str,
        with_attachment: bool,
    ) -> AppStorage {
        let database_directory = root.join("db");
        fs::create_dir_all(&database_directory).expect("create database directory");
        let database_path = database_directory.join("openchat.sqlite3");
        let connection = Connection::open(&database_path).expect("create database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE projects (
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
                    id TEXT NOT NULL,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
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
                    PRIMARY KEY (conversation_id, id)
                 );
                 PRAGMA user_version = 11;",
            )
            .expect("create chat schema");
        connection
            .execute(
                "INSERT INTO conversations (id, title, created_at, updated_at)
                 VALUES (?1, ?1, 1, 1)",
                [conversation_id],
            )
            .expect("insert conversation");
        let attachment_bytes = b"attachment payload";
        let content = if with_attachment {
            let attachment = json!({
                "id": "source-attachment",
                "name": "notes.txt",
                "mimeType": "text/plain",
                "sizeBytes": attachment_bytes.len(),
                "kind": "text"
            });
            let envelope = json!({ "content": "attached", "attachments": [attachment] });
            let content = format!("\u{1e}openchat-attachments-v1:{envelope}");
            let attachment_directory = root
                .join("attachments")
                .join(conversation_id)
                .join("source-message");
            fs::create_dir_all(&attachment_directory).expect("create attachment directory");
            fs::write(
                attachment_directory.join("source-attachment.data"),
                attachment_bytes,
            )
            .expect("write attachment fixture");
            content
        } else {
            "plain message".to_owned()
        };
        connection
            .execute(
                "INSERT INTO messages (conversation_id, id, role, content, status, created_at)
                 VALUES (?1, ?2, 'user', ?3, 'completed', 1)",
                params![conversation_id, "source-message", content],
            )
            .expect("insert message");
        drop(connection);

        let storage = AppStorage::open_at(root.to_path_buf()).expect("open test storage");
        storage
            .initialize_backend_schema()
            .expect("initialize current backend schema");
        storage
    }

    fn read_chat_count(storage: &AppStorage) -> i64 {
        storage
            .connect()
            .expect("connect to chat database")
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .expect("count conversations")
    }

    fn chat_exists(storage: &AppStorage, conversation_id: &str) -> bool {
        storage
            .connect()
            .expect("connect to chat database")
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM conversations WHERE id = ?1)",
                [conversation_id],
                |row| row.get(0),
            )
            .expect("look up conversation")
    }

    fn add_workspace_file_change(storage: &AppStorage) -> PathBuf {
        let original = b"workspace source snapshot";
        let hash = format!("{:x}", Sha256::digest(original));
        let snapshot_path = storage
            .root()
            .join("attachments/snapshots")
            .join(&hash[..2])
            .join(format!("{hash}.blob"));
        fs::create_dir_all(snapshot_path.parent().expect("snapshot parent"))
            .expect("create snapshot storage");
        fs::write(&snapshot_path, original).expect("write local workspace snapshot");
        let connection = storage.connect().expect("connect to source profile");
        connection
            .execute(
                "INSERT INTO file_change_blobs (
                    hash, size_bytes, state, created_at_unix_ms
                 ) VALUES (?1, ?2, 'ready', 1)",
                params![hash, original.len() as i64],
            )
            .expect("insert local snapshot metadata");
        connection
            .execute(
                "INSERT INTO conversation_file_changes (
                    conversation_id, id, root, path, baseline_exists, baseline_hash,
                    baseline_blob_hash, expected_exists, expected_hash, added_lines,
                    removed_lines, is_binary, diff_available, status
                 ) VALUES (
                    'source-chat', ?1, 'C:/workspace', 'private.rs', 1, ?2,
                    ?2, 0, NULL, NULL, NULL, 0, 0, 'active'
                 )",
                params!["a".repeat(64), hash],
            )
            .expect("insert local workspace change");
        connection
            .execute(
                "INSERT INTO file_change_legacy_imports (conversation_id, imported_at_unix_ms)
                 VALUES ('source-chat', 1)",
                [],
            )
            .expect("insert migration marker");
        snapshot_path
    }

    fn workspace_file_change_count(storage: &AppStorage) -> i64 {
        storage
            .connect()
            .expect("connect to profile database")
            .query_row(
                "SELECT COUNT(*) FROM conversation_file_changes",
                [],
                |row| row.get(0),
            )
            .expect("count workspace file changes")
    }

    fn workspace_file_blob_count(storage: &AppStorage) -> i64 {
        storage
            .connect()
            .expect("connect to profile database")
            .query_row("SELECT COUNT(*) FROM file_change_blobs", [], |row| {
                row.get(0)
            })
            .expect("count workspace snapshots")
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-profile-archive-{label}-{}",
                Uuid::new_v4().simple()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
