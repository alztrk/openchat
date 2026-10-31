//! Profile archive tar writing, staged archive reading, and staged validation.

use super::*;

pub(super) fn write_archive(
    output: File,
    manifest_json: &[u8],
    database_record: &FileRecord,
    database_snapshot_path: &Path,
    attachments: &[AttachmentSource],
    passphrase: &str,
) -> Result<(), ArchiveFailure> {
    let secret = SecretString::from(passphrase.to_owned());
    let mut recipient = age::scrypt::Recipient::new(secret);
    // Keep new archives within the same bounded work factor that restore accepts.
    recipient.set_work_factor(DEFAULT_AGE_SCRYPT_LOG_N);
    let encrypted = Encryptor::with_recipients(iter::once(&recipient as &dyn age::Recipient))
        .map_err(|_| ArchiveFailure::Storage)?
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

pub(super) fn append_bytes<W: Write>(
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

pub(super) fn append_file<W: Write>(
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

pub(super) fn append_reader<W: Write, R: Read>(
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

pub(super) fn read_archive_to_stage(
    path: &Path,
    passphrase: &str,
    stage: &Path,
    current_chat_schema_version: i64,
    local_database_key: &crate::storage::DatabaseKey,
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
    let mut manifest: ProfileManifest = read_entry_json(first, MAX_MANIFEST_BYTES)?;
    validate_manifest(&manifest)?;
    let archive_database_key = match manifest.version {
        1 if manifest.database_key.is_none() => None,
        2 => Some(
            crate::storage::DatabaseKey::from_bytes(
                manifest
                    .database_key
                    .as_deref()
                    .ok_or(ArchiveFailure::InvalidArchive)?,
            )
            .map_err(|_| ArchiveFailure::InvalidArchive)?,
        ),
        _ => return Err(ArchiveFailure::InvalidArchive),
    };
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
    validate_staged_profile(
        stage,
        &manifest,
        current_chat_schema_version,
        archive_database_key.as_ref(),
    )?;

    let database_path = stage.join(DATABASE_PATH);
    let converted_database_path = stage.join("database/openchat.sqlite3.local-encrypted-partial");
    match archive_database_key.as_ref() {
        Some(archive_key) => archive_key
            .export_encrypted_database(&database_path, &converted_database_path, local_database_key)
            .map_err(|_| ArchiveFailure::Storage)?,
        None => local_database_key
            .export_plaintext_database(&database_path, &converted_database_path)
            .map_err(|_| ArchiveFailure::Storage)?,
    }
    drop(archive_database_key);
    fs::remove_file(&database_path).map_err(|_| ArchiveFailure::Storage)?;
    fs::rename(&converted_database_path, &database_path).map_err(|_| ArchiveFailure::Storage)?;
    if let Some(key) = &mut manifest.database_key {
        key.zeroize();
    }
    manifest.database_key = None;
    manifest.database = file_record(DATABASE_PATH, &database_path, MAX_DATABASE_BYTES)?;
    validate_staged_profile(
        stage,
        &manifest,
        current_chat_schema_version,
        Some(local_database_key),
    )?;
    Ok(manifest)
}

pub(super) fn validate_staged_profile(
    stage: &Path,
    manifest: &ProfileManifest,
    current_chat_schema_version: i64,
    database_key: Option<&crate::storage::DatabaseKey>,
) -> Result<(), ArchiveFailure> {
    let database_path = stage.join(DATABASE_PATH);
    let metadata = fs::metadata(&database_path).map_err(|_| ArchiveFailure::Storage)?;
    if metadata.len() != manifest.database.size_bytes
        || digest_file(&database_path)? != manifest.database.sha256
    {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let database = match database_key {
        Some(key) => key
            .open(&database_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| ArchiveFailure::InvalidArchive)?,
        None => {
            Connection::open_with_flags(&database_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|_| ArchiveFailure::InvalidArchive)?
        }
    };
    crate::storage::disable_trusted_schema(&database)
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    match database_key {
        Some(_) => storage::verify_database_integrity(&database),
        None => storage::verify_plaintext_database_integrity(&database),
    }
    .map_err(|_| ArchiveFailure::InvalidArchive)?;
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

pub(super) fn copy_verified_entry<R: Read>(
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

pub(super) fn create_stage_file(stage: &Path, archive_path: &str) -> Result<File, ArchiveFailure> {
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

pub(super) fn validate_tar_entry<R: Read>(
    entry: &tar::Entry<'_, R>,
    expected: &str,
) -> Result<(), ArchiveFailure> {
    if !entry.header().entry_type().is_file() || tar_path(entry)? != expected {
        return Err(ArchiveFailure::InvalidArchive);
    }
    Ok(())
}

pub(super) fn tar_path<R: Read>(entry: &tar::Entry<'_, R>) -> Result<String, ArchiveFailure> {
    let path = entry.path().map_err(|_| ArchiveFailure::InvalidArchive)?;
    let path = path.to_str().ok_or(ArchiveFailure::InvalidArchive)?;
    safe_archive_path(path)?;
    Ok(path.to_owned())
}

pub(super) fn safe_archive_path(value: &str) -> Result<PathBuf, ArchiveFailure> {
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

pub(super) fn validate_attachment_path(value: &str) -> Result<(), ArchiveFailure> {
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

pub(super) fn read_entry_json<T: DeserializeOwned, R: Read>(
    entry: tar::Entry<'_, R>,
    maximum: u64,
) -> Result<T, ArchiveFailure> {
    if entry.size() > maximum {
        return Err(ArchiveFailure::LimitExceeded);
    }
    let size = entry.size();
    let mut bytes = Zeroizing::new(Vec::with_capacity(
        usize::try_from(size).map_err(|_| ArchiveFailure::LimitExceeded)?,
    ));
    entry
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    if bytes.len() as u64 != size {
        return Err(ArchiveFailure::InvalidArchive);
    }
    serde_json::from_slice(&bytes).map_err(|_| ArchiveFailure::InvalidArchive)
}

pub(super) fn validate_manifest(manifest: &ProfileManifest) -> Result<(), ArchiveFailure> {
    if manifest.format != ARCHIVE_FORMAT
        || !(1..=ARCHIVE_VERSION).contains(&manifest.version)
        || manifest.created_at_unix_ms <= 0
        || !(1..=100).contains(&manifest.chat_schema_version)
        || manifest.backend_schema_version < 1
        || manifest.conversation_count > 100_000
        || manifest.message_count > 2_000_000
        || manifest.attachments.len() > MAX_ATTACHMENTS
        || manifest.database.path != DATABASE_PATH
        || (manifest.version == 1 && manifest.database_key.is_some())
        || manifest
            .database_key
            .as_ref()
            .is_some_and(|key| key.len() != 32)
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

pub(super) fn validate_file_record(
    record: &FileRecord,
    maximum: u64,
) -> Result<(), ArchiveFailure> {
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

pub(super) fn file_record(
    path: &str,
    file_path: &Path,
    maximum: u64,
) -> Result<FileRecord, ArchiveFailure> {
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

pub(super) fn digest_file(path: &Path) -> Result<String, ArchiveFailure> {
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

pub(super) fn verify_foreign_keys(database: &Connection) -> Result<(), ArchiveFailure> {
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

pub(super) fn read_backend_schema_version(database: &Connection) -> Result<i64, ArchiveFailure> {
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

pub(super) fn count_rows(database: &Connection, table: &str) -> Result<u64, ArchiveFailure> {
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

pub(super) fn archive_summary(manifest: &ProfileManifest, requires_restart: bool) -> Value {
    json!({
        "conversationCount": manifest.conversation_count,
        "messageCount": manifest.message_count,
        "attachmentCount": manifest.attachments.len(),
        "attachmentBytes": manifest.attachments.iter().map(|item| item.size_bytes).sum::<u64>(),
        "requiresRestart": requires_restart,
    })
}

pub(super) fn validate_input_path(path: &Path) -> Result<File, ArchiveFailure> {
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
    let file = open_profile_archive(path).map_err(ArchiveFailure::from)?;
    let opened_metadata = file.metadata().map_err(|_| ArchiveFailure::Storage)?;
    if !opened_metadata.is_file()
        || opened_metadata.len() > MAX_ARCHIVE_BYTES
        || profile_archive_handle_is_reparse_point(&opened_metadata)
    {
        return Err(ArchiveFailure::LimitExceeded);
    }
    Ok(file)
}

#[cfg(windows)]
pub(super) fn open_profile_archive(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(windows))]
pub(super) fn open_profile_archive(path: &Path) -> io::Result<File> {
    File::open(path)
}

#[cfg(windows)]
pub(super) fn profile_archive_handle_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(super) fn profile_archive_handle_is_reparse_point(_: &fs::Metadata) -> bool {
    false
}

pub(super) fn validate_output_path(path: &Path) -> Result<PathBuf, ArchiveFailure> {
    if path.extension().and_then(|value| value.to_str()) != Some(ARCHIVE_EXTENSION) {
        return Err(ArchiveFailure::InvalidRequest);
    }
    let file_name = path.file_name().ok_or(ArchiveFailure::InvalidRequest)?;
    let parent = path.parent().ok_or(ArchiveFailure::InvalidRequest)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| ArchiveFailure::NotFound)?;
    Ok(canonical_parent.join(file_name))
}

pub(super) fn validate_passphrase(passphrase: &str) -> Result<(), ArchiveFailure> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARACTERS
        || passphrase.len() > MAX_PASSPHRASE_BYTES
        || passphrase.contains('\0')
    {
        Err(ArchiveFailure::InvalidPassphrase)
    } else {
        Ok(())
    }
}

pub(super) fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(super) fn safe_attachment_file_name(value: &str) -> bool {
    value.strip_suffix(".data").is_some_and(safe_identifier)
}

pub(super) fn digest_hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn unix_time_millis() -> io::Result<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?;
    i64::try_from(duration.as_millis()).map_err(io::Error::other)
}

pub(super) fn restore_root(root: &Path) -> PathBuf {
    root.join("cache").join(RESTORE_ROOT)
}

pub(super) fn restore_pending_path(root: &Path) -> PathBuf {
    root.join(RESTORE_PENDING_FILE)
}

pub(super) fn restore_journal_path(root: &Path, phase: RestorePhase) -> PathBuf {
    let file_name = match phase {
        RestorePhase::Applying => RESTORE_APPLYING_FILE,
        RestorePhase::AwaitingValidation => RESTORE_AWAITING_FILE,
        RestorePhase::Committed => RESTORE_COMMITTED_FILE,
    };
    root.join(file_name)
}

pub(super) fn restore_journal_paths(root: &Path) -> [PathBuf; 3] {
    [
        restore_journal_path(root, RestorePhase::Applying),
        restore_journal_path(root, RestorePhase::AwaitingValidation),
        restore_journal_path(root, RestorePhase::Committed),
    ]
}

pub(super) fn restore_stage_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("stage-{id}"))
}

pub(super) fn recovery_path(root: &Path, id: &str) -> PathBuf {
    root.join("backups").join(RECOVERY_DIRECTORY).join(id)
}
