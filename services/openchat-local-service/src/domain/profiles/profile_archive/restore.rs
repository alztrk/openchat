//! Profile restore journaling, directory swap, and recovery helpers.

use super::*;

pub(super) fn write_new_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ArchiveFailure> {
    let bytes = serde_json::to_vec(value).map_err(|_| ArchiveFailure::Storage)?;
    write_new_bytes(path, &bytes)
}

pub(super) fn ensure_staged_database_encrypted(
    stage: &Path,
    manifest: &mut ProfileManifest,
    current_chat_schema_version: i64,
    database_key: &crate::storage::DatabaseKey,
) -> Result<(), ArchiveFailure> {
    if manifest.database_key.is_some() {
        return Err(ArchiveFailure::InvalidArchive);
    }
    let database_path = stage.join(DATABASE_PATH);
    let journal_path = append_path_suffix(&database_path, ".sqlcipher-migration");
    let database_exists = match fs::symlink_metadata(&database_path) {
        Ok(metadata)
            if metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() >= 16 =>
        {
            true
        }
        Ok(_) => return Err(ArchiveFailure::InvalidArchive),
        Err(error) if error.kind() == io::ErrorKind::NotFound && journal_path.exists() => false,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ArchiveFailure::InvalidArchive);
        }
        Err(_) => return Err(ArchiveFailure::Storage),
    };
    if database_exists && has_plaintext_sqlite_header(&database_path)? {
        validate_staged_profile(stage, manifest, current_chat_schema_version, None)?;
    }
    let (database, _) = database_key
        .open_or_migrate(&database_path)
        .map_err(|_| ArchiveFailure::Storage)?;
    storage::verify_database_integrity(&database).map_err(|_| ArchiveFailure::InvalidArchive)?;
    drop(database);
    manifest.database = file_record(DATABASE_PATH, &database_path, MAX_DATABASE_BYTES)?;
    Ok(())
}

pub(super) fn convert_retained_restore_databases(
    root: &Path,
    database_key: &crate::storage::DatabaseKey,
) -> rusqlite::Result<()> {
    let directory = root.join("backups").join(RECOVERY_DIRECTORY);
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(rusqlite::Error::InvalidPath(directory)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_to_sql_error(error)),
    }
    for entry in fs::read_dir(&directory).map_err(io_to_sql_error)? {
        let entry = entry.map_err(io_to_sql_error)?;
        let file_type = entry.file_type().map_err(io_to_sql_error)?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let Some(identifier) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !safe_recovery_id(&identifier) {
            continue;
        }
        let database_path = entry.path().join("db/openchat.sqlite3");
        match fs::symlink_metadata(&database_path) {
            Ok(metadata)
                if metadata.is_file()
                    && !metadata.file_type().is_symlink()
                    && metadata.len() >= 16 => {}
            Ok(_) => return Err(rusqlite::Error::InvalidPath(database_path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(io_to_sql_error(error)),
        }
        let (database, _) = database_key.open_or_migrate(&database_path)?;
        crate::storage::verify_database_integrity(&database)?;
    }
    Ok(())
}

pub(super) fn has_plaintext_sqlite_header(path: &Path) -> Result<bool, ArchiveFailure> {
    let mut file = File::open(path).map_err(|_| ArchiveFailure::Storage)?;
    let mut header = [0_u8; 16];
    file.read_exact(&mut header)
        .map_err(|_| ArchiveFailure::InvalidArchive)?;
    Ok(&header == b"SQLite format 3\0")
}

pub(super) fn append_path_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

pub(crate) fn activate_pending_restore(
    root: &Path,
    database_key: &crate::storage::DatabaseKey,
) -> rusqlite::Result<bool> {
    recover_unconfirmed_restore(root, database_key)?;
    convert_retained_restore_databases(root, database_key)?;
    let restore_root = restore_root(root);
    let marker_path = restore_pending_path(&restore_root);
    if !marker_path.exists() {
        cleanup_orphaned_stages(&restore_root, None).map_err(io_to_sql_error)?;
        return Ok(false);
    }
    let marker = read_marker(&marker_path)?;
    cleanup_orphaned_stages(&restore_root, Some(&marker.stage_id)).map_err(io_to_sql_error)?;
    let stage = restore_stage_path(&restore_root, &marker.stage_id);
    let mut manifest: ProfileManifest = read_json(&stage.join(MANIFEST_PATH), MAX_MANIFEST_BYTES)?;
    validate_manifest(&manifest).map_err(to_sql_error)?;
    ensure_staged_database_encrypted(
        &stage,
        &mut manifest,
        marker.chat_schema_version,
        database_key,
    )
    .map_err(to_sql_error)?;
    validate_staged_profile(
        &stage,
        &manifest,
        marker.chat_schema_version,
        Some(database_key),
    )
    .map_err(to_sql_error)?;

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
        version: RESTORE_STATE_VERSION,
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
        recover_unconfirmed_restore(root, database_key).map_err(|_| to_sql_error(error))?;
        return Err(to_sql_error(error));
    }
    Ok(true)
}

pub(super) fn apply_directory_swap(
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

pub(crate) fn recover_unconfirmed_restore(
    root: &Path,
    database_key: &crate::storage::DatabaseKey,
) -> rusqlite::Result<()> {
    let restore_root = restore_root(root);
    let Some((_, journal)) = read_restore_journal(&restore_root)? else {
        return Ok(());
    };
    if journal.version != RESTORE_STATE_VERSION
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
    let recovery_database = recovery_path(root, &journal.recovery_id)
        .join("db")
        .join("openchat.sqlite3");
    match fs::symlink_metadata(&recovery_database) {
        Ok(metadata)
            if metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() >= 16 =>
        {
            let (connection, _) = database_key.open_or_migrate(&recovery_database)?;
            crate::storage::verify_database_integrity(&connection)?;
        }
        Ok(_) => return Err(rusqlite::Error::InvalidPath(recovery_database)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_to_sql_error(error)),
    }
    rollback_directory_swap(root, &restore_root, &journal).map_err(io_to_sql_error)?;
    remove_restore_journals(&restore_root).map_err(io_to_sql_error)?;
    Ok(())
}

pub(super) fn rollback_directory_swap(
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

pub(super) fn cleanup_restore_stage(restore_root: &Path, stage_id: &str) -> io::Result<()> {
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

pub(super) fn cleanup_orphaned_stages(
    restore_root: &Path,
    retained_stage_id: Option<&str>,
) -> io::Result<()> {
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

pub(super) fn read_marker(path: &Path) -> rusqlite::Result<RestoreMarker> {
    let marker = read_json::<RestoreMarker>(path, MAX_RESTORE_STATE_BYTES)?;
    if marker.version != RESTORE_STATE_VERSION
        || !safe_identifier(&marker.stage_id)
        || !(1..=100).contains(&marker.chat_schema_version)
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(marker)
}

pub(super) fn read_json<T: DeserializeOwned>(
    path: &Path,
    maximum_bytes: u64,
) -> rusqlite::Result<T> {
    let metadata = fs::symlink_metadata(path).map_err(io_to_sql_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let file = File::open(path).map_err(io_to_sql_error)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| rusqlite::Error::InvalidQuery)?,
    );
    file.take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_to_sql_error)?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(rusqlite::Error::InvalidQuery);
    }
    serde_json::from_slice(&bytes).map_err(|_| rusqlite::Error::InvalidQuery)
}

pub(super) fn read_restore_journal(
    root: &Path,
) -> rusqlite::Result<Option<(PathBuf, RestoreJournal)>> {
    for phase in [
        RestorePhase::Committed,
        RestorePhase::Applying,
        RestorePhase::AwaitingValidation,
    ] {
        let path = restore_journal_path(root, phase);
        if !path.exists() {
            continue;
        }
        let journal = read_json::<RestoreJournal>(&path, MAX_RESTORE_STATE_BYTES)?;
        if journal.version != RESTORE_STATE_VERSION
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

pub(super) fn remove_restore_journals(root: &Path) -> io::Result<()> {
    for path in restore_journal_paths(root) {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn safe_recovery_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-'))
}

pub(super) fn to_sql_error(_: ArchiveFailure) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

pub(super) fn write_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), ArchiveFailure> {
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

pub(super) fn ensure_destination_is_new(path: &Path) -> Result<(), ArchiveFailure> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(ArchiveFailure::Conflict),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ArchiveFailure::Storage),
    }
}

pub(super) fn map_destination_error(error: io::Error) -> ArchiveFailure {
    if error.kind() == io::ErrorKind::AlreadyExists {
        ArchiveFailure::Conflict
    } else {
        ArchiveFailure::Storage
    }
}

pub(super) fn expected_attachments(
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

pub(super) fn io_to_sql_error(error: io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
