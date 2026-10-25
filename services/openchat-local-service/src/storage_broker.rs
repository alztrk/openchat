//! Windows AppContainer storage broker over inherited standard handles.

use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    ptr::null_mut,
};

use crate::{
    storage::AppStorage,
    storage::ipc_protocol::{
        LegacyImportStatus, MAX_STORAGE_FRAME_BYTES, MAX_STORAGE_IMPORT_BYTES,
        MAX_STORAGE_SNAPSHOT_BYTES, MAX_STORAGE_TRANSFER_CHUNK_BYTES, STORAGE_PROTOCOL_VERSION,
        StorageRequest, StorageResponse, handle_request, storage_error_response,
    },
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE, LocalFree},
    Security::Authorization::ConvertSidToStringSidW,
    Security::Isolation::GetAppContainerFolderPath,
    Security::{
        GetTokenInformation, TOKEN_APPCONTAINER_INFORMATION, TOKEN_QUERY, TokenAppContainerSid,
        TokenIsAppContainer,
    },
    Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT,
    System::{
        Com::CoTaskMemFree,
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};
use zeroize::Zeroizing;

/// Runs the storage protocol over inherited standard handles from the isolated AppContainer.
pub fn run_broker_stdio() -> Result<(), String> {
    let expected_profile_root = current_appcontainer_profile_root()?;
    let profile_root = env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| "The AppContainer storage profile path is unavailable.".to_owned())?;
    if !same_path(&profile_root, &expected_profile_root) {
        return Err(format!(
            "The storage broker profile path does not match its AppContainer identity (expected_depth={}, actual_depth={}, expected_suffix={}, actual_suffix={}).",
            expected_profile_root.components().count(),
            profile_root.components().count(),
            safe_profile_path_suffix(&expected_profile_root),
            safe_profile_path_suffix(&profile_root),
        ));
    }
    let profile_root = fs::canonicalize(&profile_root)
        .map_err(|_| "The AppContainer storage profile path could not be resolved safely.")?;
    ensure_appcontainer_directory(&profile_root, &profile_root)?;
    ensure_appcontainer_directory(&profile_root, &profile_root.join("AppData"))?;
    ensure_appcontainer_directory(&profile_root, &profile_root.join("AppData").join("Roaming"))?;
    ensure_appcontainer_directory(&profile_root, &profile_root.join("Temp"))?;

    let storage_root = profile_root.join("OpenChatStorage");
    let profile = AppStorage::open_at(storage_root.clone())
        .map_err(|_| "The AppContainer storage profile could not open safely.".to_owned())?;
    profile
        .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
        .map_err(|_| "The AppContainer chat schema could not be validated.".to_owned())?;
    profile
        .initialize_backend_schema()
        .map_err(|_| "The AppContainer storage schema could not be initialized.".to_owned())?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    serve_profile_connection(&mut reader, &mut writer, &profile)
}

fn current_appcontainer_profile_root() -> Result<PathBuf, String> {
    let mut token = null_mut();
    // SAFETY: GetCurrentProcess returns a valid pseudo-handle and `token` is a writable output.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0
        || token.is_null()
    {
        return Err("The storage broker process identity could not be verified.".to_owned());
    }
    let _token_guard = HandleGuard(token);
    let mut is_app_container = 0u32;
    let mut returned_bytes = 0u32;
    // SAFETY: The token is live and both output buffers are sized for TokenIsAppContainer.
    if unsafe {
        GetTokenInformation(
            token,
            TokenIsAppContainer,
            (&mut is_app_container as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
            &mut returned_bytes,
        )
    } == 0
        || returned_bytes != std::mem::size_of::<u32>() as u32
        || is_app_container != 1
    {
        return Err("The storage broker must run inside its AppContainer.".to_owned());
    }
    let mut required_bytes = 0u32;
    // SAFETY: A null buffer with zero length asks Windows for the token SID buffer size.
    unsafe {
        GetTokenInformation(
            token,
            TokenAppContainerSid,
            null_mut(),
            0,
            &mut required_bytes,
        );
    }
    if required_bytes == 0
        || io::Error::last_os_error().raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32)
    {
        return Err("The storage broker AppContainer identity could not be read.".to_owned());
    }
    let word_count = (required_bytes as usize).div_ceil(std::mem::size_of::<u64>());
    let mut token_buffer = vec![0u64; word_count];
    // SAFETY: The buffer is aligned for TOKEN_APPCONTAINER_INFORMATION and sized as requested.
    if unsafe {
        GetTokenInformation(
            token,
            TokenAppContainerSid,
            token_buffer.as_mut_ptr().cast(),
            required_bytes,
            &mut required_bytes,
        )
    } == 0
    {
        return Err("The storage broker AppContainer identity could not be read.".to_owned());
    }
    // SAFETY: The successful TokenAppContainerSid query initialized this structure in the buffer.
    let app_container = unsafe {
        &*token_buffer
            .as_ptr()
            .cast::<TOKEN_APPCONTAINER_INFORMATION>()
    };
    if app_container.TokenAppContainer.is_null() {
        return Err("The storage broker AppContainer identity is empty.".to_owned());
    }

    let mut sid_string = null_mut();
    // SAFETY: The token-owned SID remains valid while `token_buffer` is alive.
    if unsafe { ConvertSidToStringSidW(app_container.TokenAppContainer, &mut sid_string) } == 0
        || sid_string.is_null()
    {
        return Err("The storage broker AppContainer identity could not be resolved.".to_owned());
    }
    let mut profile_path = null_mut();
    // SAFETY: `sid_string` is a Windows-owned NUL-terminated SID and the output pointer is writable.
    let folder_result = unsafe { GetAppContainerFolderPath(sid_string, &mut profile_path) };
    // SAFETY: `sid_string` was allocated by ConvertSidToStringSidW.
    unsafe { LocalFree(sid_string.cast()) };
    if folder_result < 0 || profile_path.is_null() {
        return Err(
            "The storage broker AppContainer data folder could not be resolved.".to_owned(),
        );
    }
    // SAFETY: Windows returned a readable NUL-terminated UTF-16 path.
    let mut length = 0usize;
    // SAFETY: `profile_path` remains allocated until CoTaskMemFree below.
    unsafe {
        while *profile_path.add(length) != 0 {
            length += 1;
        }
    }
    // SAFETY: `length` ends at the terminator just inspected above.
    let path = PathBuf::from(unsafe {
        OsString::from_wide(std::slice::from_raw_parts(profile_path, length))
    });
    // SAFETY: `profile_path` was returned by GetAppContainerFolderPath.
    unsafe { CoTaskMemFree(profile_path.cast()) };
    Ok(path)
}

/// Validates profile-owned directories without inspecting host paths the AppContainer cannot read.
fn ensure_appcontainer_directory(root: &Path, path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| {
                format!(
                    "The AppContainer profile directory could not be created (error_kind={:?}).",
                    error.kind()
                )
            })?;
        }
        Err(error) => {
            return Err(format!(
                "The AppContainer profile directory could not be inspected (error_kind={:?}).",
                error.kind()
            ));
        }
    }

    validate_appcontainer_directory_chain(root, path)
}

/// Checks the AppContainer root and each descendant, stopping before host profile ancestors.
fn validate_appcontainer_directory_chain(root: &Path, path: &Path) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;
    use std::path::Component;

    let relative = path
        .strip_prefix(root)
        .map_err(|_| "The AppContainer profile directory escaped its verified root.".to_owned())?;
    let mut directories = vec![root.to_path_buf()];
    let mut current = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(name) => current.push(name),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => {
                return Err("The AppContainer profile directory path is invalid.".to_owned());
            }
        }
        directories.push(current.clone());
    }

    for (relative_depth, directory) in directories.iter().enumerate() {
        let metadata = fs::symlink_metadata(directory).map_err(|error| {
            format!(
                "The AppContainer profile directory could not be inspected (relative_depth={relative_depth}, error_kind={:?}).",
                error.kind()
            )
        })?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("The AppContainer profile path contains a reparse point.".to_owned());
        }
        if !metadata.is_dir() {
            return Err(
                "The AppContainer profile path contains a non-directory component.".to_owned(),
            );
        }
    }

    Ok(())
}

fn serve_profile_connection<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &AppStorage,
) -> Result<(), String> {
    let request = read_request_frame(reader)?;
    let request: StorageRequest = serde_json::from_slice(&request)
        .map_err(|_| "The storage broker handshake is malformed.".to_owned())?;
    if !matches!(request, StorageRequest::Hello { version } if version == STORAGE_PROTOCOL_VERSION)
    {
        write_response_frame(
            writer,
            &StorageResponse::Error {
                code: rusqlite::ffi::SQLITE_MISUSE,
                message: "The storage broker protocol version is unsupported.".to_owned(),
            },
        )?;
        return Err("The storage broker protocol version is unsupported.".to_owned());
    }

    write_response_frame(
        writer,
        &StorageResponse::Ready {
            version: STORAGE_PROTOCOL_VERSION,
        },
    )?;

    // Autocommit requests lease the database only while that request runs. An explicit
    // transaction keeps its lease on this broker connection until it commits, rolls back, or disconnects.
    let mut transaction_connection = None;
    let mut pending_import = None;
    let mut pending_snapshot = None;

    loop {
        let request = match read_request_frame_optional(reader)? {
            Some(request) => request,
            None => break,
        };
        let request = Zeroizing::new(request);
        let request = match serde_json::from_slice::<StorageRequest>(&request) {
            Ok(request) => request,
            Err(_) => {
                write_response_frame(
                    writer,
                    &StorageResponse::Error {
                        code: rusqlite::ffi::SQLITE_MISUSE,
                        message: "The storage request is malformed.".to_owned(),
                    },
                )?;
                break;
            }
        };

        let Some(request) = handle_legacy_import_request(
            request,
            writer,
            profile,
            &mut pending_import,
            pending_snapshot.is_some(),
            transaction_connection.is_some(),
        )?
        else {
            continue;
        };
        let Some(request) = handle_snapshot_request(
            request,
            writer,
            profile,
            &mut pending_snapshot,
            pending_import.is_some(),
            transaction_connection.is_some(),
        )?
        else {
            continue;
        };
        let Some(request) = handle_vault_request(
            request,
            writer,
            profile,
            pending_import.is_some(),
            pending_snapshot.is_some(),
            transaction_connection.is_some(),
        )?
        else {
            continue;
        };

        if transaction_connection.is_none() {
            transaction_connection = match profile.connect_owned_database() {
                Ok(connection) => Some(connection),
                Err(error) => {
                    let code = error
                        .sqlite_error_code()
                        .map_or(rusqlite::ffi::SQLITE_ERROR, |code| code as i32);
                    write_response_frame(
                        writer,
                        &StorageResponse::Error {
                            code,
                            message: "The protected database is busy.".to_owned(),
                        },
                    )?;
                    continue;
                }
            };
        }

        let (response, transaction_active) = {
            let Some(connection) = transaction_connection.as_mut() else {
                return Err("A storage request has no database lease.".to_owned());
            };
            let response = handle_request(connection, request);
            (response, !connection.is_autocommit())
        };
        write_response_frame(writer, &response)?;

        if !transaction_active {
            transaction_connection = None;
        }
    }

    if let Some(connection) = transaction_connection {
        if !connection.is_autocommit() {
            connection.execute_batch("ROLLBACK").map_err(|error| {
                format!("An abandoned database transaction could not roll back: {error}")
            })?;
        }
    }
    Ok(())
}

fn handle_vault_request(
    request: StorageRequest,
    writer: &mut impl Write,
    profile: &AppStorage,
    import_active: bool,
    snapshot_active: bool,
    transaction_active: bool,
) -> Result<Option<StorageRequest>, String> {
    let blocked = import_active || snapshot_active || transaction_active;
    let response = match request {
        StorageRequest::VaultReadMap => {
            if blocked {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                match crate::secrets::SecretMap::read_from_storage(profile) {
                    Ok(secrets) if secrets.is_within_limits() => {
                        StorageResponse::VaultMap { secrets }
                    }
                    _ => storage_error_response(rusqlite::Error::InvalidQuery),
                }
            }
        }
        StorageRequest::VaultWriteMap { secrets } => {
            if blocked || !secrets.is_within_limits() {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                match secrets.write_to_storage(profile) {
                    Ok(()) => StorageResponse::VaultUpdated,
                    Err(_) => storage_error_response(rusqlite::Error::InvalidQuery),
                }
            }
        }
        StorageRequest::VaultGet { key } => {
            if blocked || !valid_vault_key(&key) {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                match crate::secrets::get_vault_secret_from_storage(profile, &key) {
                    Ok(value) => {
                        let mut secrets = crate::secrets::SecretMap::default();
                        if let Some(value) = value {
                            secrets.insert(key, value.to_string());
                        }
                        StorageResponse::VaultMap { secrets }
                    }
                    Err(_) => storage_error_response(rusqlite::Error::InvalidQuery),
                }
            }
        }
        StorageRequest::VaultSet { secrets } => {
            let mut entries = secrets.iter();
            let first = entries.next();
            if blocked || !secrets.is_within_limits() || entries.next().is_some() {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else if let Some((key, value)) = first.filter(|(key, _)| valid_vault_key(key)) {
                match crate::secrets::set_vault_secret_from_storage(profile, key, value) {
                    Ok(()) => StorageResponse::VaultUpdated,
                    Err(_) => storage_error_response(rusqlite::Error::InvalidQuery),
                }
            } else {
                storage_error_response(rusqlite::Error::InvalidQuery)
            }
        }
        StorageRequest::VaultDelete { key } => {
            if blocked || !valid_vault_key(&key) {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                match crate::secrets::delete_vault_secret_from_storage(profile, &key) {
                    Ok(()) => StorageResponse::VaultUpdated,
                    Err(_) => storage_error_response(rusqlite::Error::InvalidQuery),
                }
            }
        }
        StorageRequest::VaultHas { key } => {
            if blocked || !valid_vault_key(&key) {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                match crate::secrets::has_vault_secret_from_storage(profile, &key) {
                    Ok(exists) => StorageResponse::VaultContains { exists },
                    Err(_) => storage_error_response(rusqlite::Error::InvalidQuery),
                }
            }
        }
        request => return Ok(Some(request)),
    };

    write_response_frame(writer, &response)?;
    Ok(None)
}

fn valid_vault_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 1024 && !key.chars().any(char::is_control)
}

struct PendingLegacyImport {
    import_id: String,
    path: PathBuf,
    file: Option<File>,
    total_bytes: u64,
    next_offset: u64,
    expected_sha256: String,
    hasher: Sha256,
}

struct PendingStorageSnapshot {
    snapshot_id: String,
    path: PathBuf,
    file: Option<File>,
    total_bytes: u64,
    next_offset: u64,
    sha256: String,
}

impl Drop for PendingStorageSnapshot {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

impl Drop for PendingLegacyImport {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

fn handle_snapshot_request(
    request: StorageRequest,
    writer: &mut impl Write,
    profile: &AppStorage,
    pending: &mut Option<PendingStorageSnapshot>,
    import_active: bool,
    transaction_active: bool,
) -> Result<Option<StorageRequest>, String> {
    let response = match request {
        StorageRequest::BeginSnapshot => {
            if transaction_active || import_active || pending.is_some() {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                let directory = profile.root().join("cache");
                ensure_appcontainer_directory(profile.root(), &directory)?;
                let snapshot_id = Uuid::new_v4().simple().to_string();
                let path = directory.join(format!("snapshot-{snapshot_id}.sqlite3"));
                profile.create_owned_database_snapshot(&path).map_err(|_| {
                    "The verified database snapshot could not be created.".to_owned()
                })?;
                let mut snapshot = PendingStorageSnapshot {
                    snapshot_id: snapshot_id.clone(),
                    path: path.clone(),
                    file: None,
                    total_bytes: 0,
                    next_offset: 0,
                    sha256: String::new(),
                };
                let mut file = File::open(&path)
                    .map_err(|_| "The verified database snapshot could not be read.".to_owned())?;
                let total_bytes = file
                    .metadata()
                    .map_err(|_| {
                        "The verified database snapshot could not be inspected.".to_owned()
                    })?
                    .len();
                if total_bytes == 0 || total_bytes > MAX_STORAGE_SNAPSHOT_BYTES {
                    drop(file);
                    drop(snapshot);
                    storage_error_response(rusqlite::Error::InvalidQuery)
                } else {
                    let sha256 = hash_snapshot(&mut file)?;
                    snapshot.file = Some(file);
                    snapshot.total_bytes = total_bytes;
                    snapshot.sha256 = sha256.clone();
                    *pending = Some(snapshot);
                    StorageResponse::SnapshotStarted {
                        snapshot_id,
                        total_bytes,
                        sha256,
                    }
                }
            }
        }
        StorageRequest::ReadSnapshotChunk {
            snapshot_id,
            offset,
        } => {
            let Some(snapshot) = pending.as_mut() else {
                return write_migration_error(writer, rusqlite::Error::InvalidQuery);
            };
            if transaction_active
                || import_active
                || snapshot.snapshot_id != snapshot_id
                || offset != snapshot.next_offset
                || offset >= snapshot.total_bytes
            {
                pending.take();
                return write_migration_error(writer, rusqlite::Error::InvalidQuery);
            }
            let remaining = snapshot.total_bytes.saturating_sub(snapshot.next_offset);
            let chunk_length = usize::try_from(remaining)
                .unwrap_or(MAX_STORAGE_TRANSFER_CHUNK_BYTES)
                .min(MAX_STORAGE_TRANSFER_CHUNK_BYTES);
            let mut buffer = vec![0; chunk_length];
            let file = snapshot.file.as_mut().ok_or_else(|| {
                "The verified database snapshot stream is unavailable.".to_owned()
            })?;
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|_| "The verified database snapshot could not be read.".to_owned())?;
            if bytes_read == 0 {
                pending.take();
                return write_migration_error(writer, rusqlite::Error::InvalidQuery);
            }
            buffer.truncate(bytes_read);
            let chunk_offset = snapshot.next_offset;
            snapshot.next_offset = snapshot
                .next_offset
                .checked_add(bytes_read as u64)
                .filter(|next| *next <= snapshot.total_bytes)
                .ok_or_else(|| "The verified database snapshot stream is invalid.".to_owned())?;
            StorageResponse::SnapshotChunk {
                offset: chunk_offset,
                data: STANDARD.encode(buffer),
                complete: snapshot.next_offset == snapshot.total_bytes,
            }
        }
        StorageRequest::EndSnapshot { snapshot_id } => {
            if pending
                .as_ref()
                .is_some_and(|snapshot| snapshot.snapshot_id == snapshot_id)
            {
                pending.take();
                StorageResponse::SnapshotEnded
            } else {
                storage_error_response(rusqlite::Error::InvalidQuery)
            }
        }
        request => return Ok(Some(request)),
    };

    write_response_frame(writer, &response)?;
    Ok(None)
}

fn hash_snapshot(file: &mut File) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|_| "The verified database snapshot could not be read.".to_owned())?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "The verified database snapshot stream could not be reset.".to_owned())?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn handle_legacy_import_request(
    request: StorageRequest,
    writer: &mut impl Write,
    profile: &AppStorage,
    pending: &mut Option<PendingLegacyImport>,
    snapshot_active: bool,
    transaction_active: bool,
) -> Result<Option<StorageRequest>, String> {
    let response = match request {
        StorageRequest::LegacyImportStatus => {
            if transaction_active {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                match profile.legacy_import_status() {
                    Ok(status) => StorageResponse::LegacyImportStatus { status },
                    Err(error) => storage_error_response(error),
                }
            }
        }
        StorageRequest::CompleteEmptyLegacyImport {
            secrets,
            preserve_legacy_source,
        } => {
            if transaction_active || pending.is_some() || snapshot_active {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                match profile.complete_empty_legacy_import(secrets, preserve_legacy_source) {
                    Ok(()) => StorageResponse::LegacyImportCompleted,
                    Err(error) => storage_error_response(error),
                }
            }
        }
        StorageRequest::BeginLegacyImport {
            total_bytes,
            sha256,
        } => {
            if transaction_active
                || pending.is_some()
                || snapshot_active
                || total_bytes == 0
                || total_bytes > MAX_STORAGE_IMPORT_BYTES
                || !valid_sha256(&sha256)
            {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else if profile.legacy_import_status().ok() != Some(LegacyImportStatus::PendingEmpty)
            {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                let directory = profile.root().join("db").join("imports");
                ensure_appcontainer_directory(profile.root(), &directory)?;
                let import_id = Uuid::new_v4().simple().to_string();
                let path = directory.join(format!("legacy-{import_id}.partial"));
                let file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|_| "The storage import could not be staged safely.".to_owned())?;
                *pending = Some(PendingLegacyImport {
                    import_id: import_id.clone(),
                    path,
                    file: Some(file),
                    total_bytes,
                    next_offset: 0,
                    expected_sha256: sha256,
                    hasher: Sha256::new(),
                });
                StorageResponse::LegacyImportStarted { import_id }
            }
        }
        StorageRequest::WriteLegacyImportChunk {
            import_id,
            offset,
            data,
        } => {
            let Some(import) = pending.as_mut() else {
                return write_migration_error(writer, rusqlite::Error::InvalidQuery);
            };
            let decoded = match STANDARD.decode(data) {
                Ok(decoded) => decoded,
                Err(_) => {
                    pending.take();
                    return write_migration_error(writer, rusqlite::Error::InvalidQuery);
                }
            };
            if transaction_active
                || import.import_id != import_id
                || offset != import.next_offset
                || decoded.is_empty()
                || decoded.len() > MAX_STORAGE_TRANSFER_CHUNK_BYTES
                || import.next_offset.saturating_add(decoded.len() as u64) > import.total_bytes
            {
                pending.take();
                return write_migration_error(writer, rusqlite::Error::InvalidQuery);
            }
            let file = import
                .file
                .as_mut()
                .ok_or_else(|| "The storage import stream is unavailable.".to_owned())?;
            file.write_all(&decoded)
                .map_err(|_| "The storage import could not be written safely.".to_owned())?;
            import.hasher.update(&decoded);
            import.next_offset = import.next_offset.saturating_add(decoded.len() as u64);
            StorageResponse::LegacyImportChunkWritten {
                next_offset: import.next_offset,
            }
        }
        StorageRequest::CommitLegacyImport { import_id, secrets } => {
            if transaction_active
                || pending
                    .as_ref()
                    .is_none_or(|import| import.import_id != import_id)
            {
                pending.take();
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                let mut import = pending
                    .take()
                    .ok_or_else(|| "The storage import stream is unavailable.".to_owned())?;
                let digest = format!("{:x}", import.hasher.clone().finalize());
                let is_complete = import.next_offset == import.total_bytes
                    && digest == import.expected_sha256
                    && import
                        .file
                        .as_ref()
                        .is_some_and(|file| file.sync_all().is_ok());
                if !is_complete {
                    drop(import);
                    storage_error_response(rusqlite::Error::InvalidQuery)
                } else {
                    drop(import.file.take());
                    let result = profile.import_legacy_snapshot(&import.path, secrets);
                    drop(import);
                    match result {
                        Ok(()) => StorageResponse::LegacyImportCompleted,
                        Err(error) => storage_error_response(error),
                    }
                }
            }
        }
        request => return Ok(Some(request)),
    };

    write_response_frame(writer, &response)?;
    Ok(None)
}

fn write_migration_error(
    writer: &mut impl Write,
    error: rusqlite::Error,
) -> Result<Option<StorageRequest>, String> {
    write_response_frame(writer, &storage_error_response(error))?;
    Ok(None)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn same_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    let (Ok(left), Ok(right)) = (fs::canonicalize(left), fs::canonicalize(right)) else {
        return false;
    };
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(right.as_os_str().to_string_lossy().as_ref())
}

fn safe_profile_path_suffix(path: &Path) -> String {
    use std::path::Component;

    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => None,
        })
        .collect::<Vec<_>>();
    let suffix_start = components.len().saturating_sub(6);
    let suffix = components
        .iter()
        .skip(suffix_start)
        .map(|component| {
            if matches!(
                component.to_ascii_lowercase().as_str(),
                "openchatstoragebrokerv1"
                    | "ac"
                    | "local"
                    | "temp"
                    | "appdata"
                    | "roaming"
                    | "openchatstorage"
                    | "packages"
            ) {
                component.as_str()
            } else {
                "<redacted>"
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    if suffix.is_empty() {
        "unknown".to_owned()
    } else {
        suffix
    }
}

fn read_request_frame(reader: &mut impl BufRead) -> Result<Vec<u8>, String> {
    read_request_frame_optional(reader)?
        .ok_or_else(|| "The storage broker handshake is empty.".to_owned())
}

fn read_request_frame_optional(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut frame = Vec::new();
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|error| format!("The storage broker request could not be read: {error}"))?;
        if buffer.is_empty() {
            if frame.is_empty() {
                return Ok(None);
            }
            return Err("The storage broker ended during a request frame.".to_owned());
        }

        if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            if frame.len().saturating_add(newline) > MAX_STORAGE_FRAME_BYTES {
                return Err("The storage broker request exceeds its size limit.".to_owned());
            }
            frame.extend_from_slice(&buffer[..newline]);
            reader.consume(newline.saturating_add(1));
            if frame.last() == Some(&b'\r') {
                frame.pop();
            }
            return Ok(Some(frame));
        }

        if frame.len().saturating_add(buffer.len()) > MAX_STORAGE_FRAME_BYTES {
            return Err("The storage broker request exceeds its size limit.".to_owned());
        }
        let consumed = buffer.len();
        frame.extend_from_slice(buffer);
        reader.consume(consumed);
    }
}

fn write_response_frame(writer: &mut impl Write, response: &StorageResponse) -> Result<(), String> {
    let mut frame = Zeroizing::new(
        serde_json::to_vec(response)
            .map_err(|_| "The storage broker response could not be encoded.".to_owned())?,
    );
    if frame.len() > MAX_STORAGE_FRAME_BYTES {
        return Err("The storage broker response exceeds its size limit.".to_owned());
    }
    frame.push(b'\n');
    writer
        .write_all(&frame)
        .and_then(|()| writer.flush())
        .map_err(|error| format!("The storage broker response could not be written: {error}"))
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: The guard owns the handle returned by a successful Windows API call.
            unsafe { CloseHandle(self.0) };
        }
    }
}
