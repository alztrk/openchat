//! Windows AppContainer storage broker over inherited standard handles.

use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    os::windows::{ffi::OsStringExt, fs::MetadataExt},
    path::{Path, PathBuf},
    ptr::null_mut,
};
use zeroize::Zeroize;

use crate::{
    storage::ipc_protocol::{
        LegacyImportStatus, MAX_STORAGE_FRAME_BYTES, MAX_STORAGE_IMPORT_BYTES,
        MAX_STORAGE_SNAPSHOT_BYTES, MAX_STORAGE_TRANSFER_CHUNK_BYTES, STORAGE_PROTOCOL_VERSION,
        StorageRequest, StorageResponse, handle_request, storage_error_response,
    },
    storage::{AppStorage, DatabaseKey},
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
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    run_broker_profile(&profile_root, &mut reader, &mut writer)
}

fn run_broker_profile<R: BufRead, W: Write>(
    profile_root: &Path,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), String> {
    let profile_root = fs::canonicalize(profile_root)
        .map_err(|_| "The AppContainer storage profile path could not be resolved safely.")?;
    ensure_appcontainer_directory(&profile_root, &profile_root)?;
    ensure_appcontainer_directory(&profile_root, &profile_root.join("AppData"))?;
    ensure_appcontainer_directory(&profile_root, &profile_root.join("AppData").join("Roaming"))?;
    ensure_appcontainer_directory(&profile_root, &profile_root.join("Temp"))?;

    let storage_root = profile_root.join("OpenChatStorage");
    ensure_appcontainer_directory(&profile_root, &storage_root)?;
    let database_directory = storage_root.join("db");
    ensure_appcontainer_directory(&profile_root, &database_directory)?;
    let key = receive_database_key(reader, writer, &database_directory.join("openchat.sqlite3"))?;
    let profile = AppStorage::open_at_with_key(storage_root.clone(), key)
        .map_err(|_| "The AppContainer storage profile could not open safely.".to_owned())?;
    profile
        .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
        .map_err(|_| "The AppContainer chat schema could not be validated.".to_owned())?;
    profile
        .initialize_backend_schema()
        .map_err(|_| "The AppContainer storage schema could not be initialized.".to_owned())?;
    clear_key_bootstrap_marker(&database_directory.join("openchat.sqlite3"))?;

    write_key_bootstrap_status(writer, 2)?;
    serve_profile_connection(reader, writer, &profile)
}

const KEY_BOOTSTRAP_MAGIC: &[u8; 4] = b"OCK1";
const KEY_BOOTSTRAP_MARKER_CONTENT: &[u8] = b"OpenChat SQLCipher initial key bootstrap v1\n";

fn receive_database_key(
    reader: &mut impl Read,
    writer: &mut impl Write,
    database_path: &Path,
) -> Result<DatabaseKey, String> {
    let mut request = [0; 5];
    reader
        .read_exact(&mut request)
        .map_err(|_| "The storage broker key initialization frame is unavailable.")?;
    if &request[..4] != KEY_BOOTSTRAP_MAGIC {
        return Err("The storage broker key initialization frame is invalid.".to_owned());
    }
    match request[4] {
        0 => {
            if !prepare_keyless_database_bootstrap(database_path)? {
                write_key_bootstrap_status(writer, 3)?;
                return Err("The existing storage database has no usable key.".to_owned());
            }
            write_key_bootstrap_status(writer, 1)?;
        }
        1 => {
            if !database_allows_existing_key(database_path)? {
                write_key_bootstrap_status(writer, 3)?;
                return Err(
                    "The saved database key has no active or recoverable storage database."
                        .to_owned(),
                );
            }
        }
        _ => return Err("The storage broker key initialization frame is invalid.".to_owned()),
    }

    let mut key_bytes = [0; 32];
    let read_result = reader.read_exact(&mut key_bytes);
    if read_result.is_err() {
        key_bytes.zeroize();
        return Err("The storage broker key initialization frame is incomplete.".to_owned());
    }
    let key_result = DatabaseKey::from_bytes(&key_bytes);
    key_bytes.zeroize();
    let key = key_result.map_err(|_| "The storage broker key initialization frame is invalid.")?;
    Ok(key)
}

fn database_allows_key_creation(database_path: &Path) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(_) => return Err("The storage database state could not be checked safely.".to_owned()),
    };
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("The storage database path is not a regular file.".to_owned());
    }
    if metadata.len() == 0 {
        return Ok(true);
    }
    if metadata.len() < 16 {
        return Ok(false);
    }
    let mut file = File::open(database_path)
        .map_err(|_| "The storage database state could not be checked safely.")?;
    let mut header = [0; 16];
    file.read_exact(&mut header)
        .map_err(|_| "The storage database state could not be checked safely.")?;
    Ok(&header == b"SQLite format 3\0")
}

fn prepare_keyless_database_bootstrap(database_path: &Path) -> Result<bool, String> {
    if !database_allows_key_creation(database_path)? {
        return Ok(false);
    }
    if database_needs_initialization_marker(database_path)? {
        write_key_bootstrap_marker(database_path)?;
    }
    Ok(true)
}

fn database_needs_initialization_marker(database_path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(database_path) {
        Ok(metadata)
            if !metadata.is_file()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 =>
        {
            Err("The storage database path is not a regular file.".to_owned())
        }
        Ok(metadata) => Ok(metadata.len() == 0),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err("The storage database state could not be checked safely.".to_owned()),
    }
}

fn database_allows_existing_key(database_path: &Path) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if key_bootstrap_marker_is_valid(database_path)? {
                return Ok(true);
            }
            return DatabaseKey::has_recoverable_plaintext_migration(database_path).map_err(|_| {
                "The interrupted storage migration could not be checked safely.".to_owned()
            });
        }
        Err(_) => return Err("The storage database state could not be checked safely.".to_owned()),
    };
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("The storage database path is not a regular file.".to_owned());
    }

    if metadata.len() < 16 {
        if key_bootstrap_marker_is_valid(database_path)? {
            return Ok(true);
        }
        return DatabaseKey::has_recoverable_plaintext_migration(database_path).map_err(|_| {
            "The interrupted storage migration could not be checked safely.".to_owned()
        });
    }

    // The keyed open below verifies SQLCipher data, while plaintext headers enter the
    // journaled migration path. Do not make a new profile when the active file is missing.
    let _ = key_bootstrap_marker_is_valid(database_path)?;
    Ok(true)
}

fn key_bootstrap_marker_path(database_path: &Path) -> PathBuf {
    let mut path = database_path.as_os_str().to_os_string();
    path.push(".sqlcipher-key-bootstrap");
    PathBuf::from(path)
}

fn write_key_bootstrap_marker(database_path: &Path) -> Result<(), String> {
    let marker_path = key_bootstrap_marker_path(database_path);
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker_path)
    {
        Ok(mut marker) => {
            let result = marker
                .write_all(KEY_BOOTSTRAP_MARKER_CONTENT)
                .and_then(|()| marker.sync_all());
            if result.is_ok() {
                return Ok(());
            }
            let cleanup = fs::remove_file(&marker_path);
            if cleanup.is_err() {
                return Err(
                    "The database key initialization state could not be saved safely.".to_owned(),
                );
            }
            Err("The database key initialization state could not be saved.".to_owned())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if key_bootstrap_marker_is_valid(database_path)? {
                Ok(())
            } else {
                Err("The database key initialization state is invalid.".to_owned())
            }
        }
        Err(_) => Err("The database key initialization state could not be saved.".to_owned()),
    }
}

fn key_bootstrap_marker_is_valid(database_path: &Path) -> Result<bool, String> {
    let marker_path = key_bootstrap_marker_path(database_path);
    let metadata = match fs::symlink_metadata(&marker_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(_) => {
            return Err("The database key initialization state could not be checked.".to_owned());
        }
    };
    if !metadata.is_file()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || metadata.len() != KEY_BOOTSTRAP_MARKER_CONTENT.len() as u64
    {
        return Err("The database key initialization state is invalid.".to_owned());
    }
    let marker = fs::read(&marker_path)
        .map_err(|_| "The database key initialization state could not be checked.")?;
    if marker == KEY_BOOTSTRAP_MARKER_CONTENT {
        Ok(true)
    } else {
        Err("The database key initialization state is invalid.".to_owned())
    }
}

fn clear_key_bootstrap_marker(database_path: &Path) -> Result<(), String> {
    if !key_bootstrap_marker_is_valid(database_path)? {
        return Ok(());
    }
    fs::remove_file(key_bootstrap_marker_path(database_path))
        .map_err(|_| "The database key initialization state could not be cleared.".to_owned())
}

fn write_key_bootstrap_status(writer: &mut impl Write, status: u8) -> Result<(), String> {
    let mut response = [0; 5];
    response[..4].copy_from_slice(KEY_BOOTSTRAP_MAGIC);
    response[4] = status;
    writer
        .write_all(&response)
        .and_then(|()| writer.flush())
        .map_err(|_| "The storage broker key initialization response failed.".to_owned())
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

    while let Some(request) = read_request_frame_optional(reader)? {
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

        if matches!(&request, StorageRequest::CommitPendingProfileRestore) {
            let response = if pending_import.is_some()
                || pending_snapshot.is_some()
                || transaction_connection.is_some()
            {
                storage_error_response(rusqlite::Error::InvalidQuery)
            } else {
                match profile.commit_pending_profile_restore() {
                    Ok(()) => StorageResponse::Unit,
                    Err(error) => storage_error_response(error),
                }
            };
            write_response_frame(writer, &response)?;
            continue;
        }

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

    if let Some(connection) = transaction_connection
        && !connection.is_autocommit()
    {
        connection.execute_batch("ROLLBACK").map_err(|error| {
            format!("An abandoned database transaction could not roll back: {error}")
        })?;
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
                || profile.legacy_import_status().ok() != Some(LegacyImportStatus::PendingEmpty)
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

#[cfg(test)]
mod tests {
    use std::{fs, io::Cursor, path::PathBuf};

    use rusqlite::{Connection, OpenFlags};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::{
        KEY_BOOTSTRAP_MAGIC, KEY_BOOTSTRAP_MARKER_CONTENT, database_allows_existing_key,
        key_bootstrap_marker_is_valid, key_bootstrap_marker_path,
        prepare_keyless_database_bootstrap, receive_database_key, run_broker_profile,
    };
    use crate::storage::{DatabaseKey, ipc_protocol::STORAGE_PROTOCOL_VERSION};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-storage-broker-{}",
                Uuid::new_v4().simple()
            ));
            fs::create_dir(&path).expect("create isolated storage broker profile");
            Self(path)
        }

        fn database_path(&self) -> PathBuf {
            self.0.join("openchat.sqlite3")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated storage broker profile");
        }
    }

    #[test]
    fn first_run_marker_allows_database_key_bootstrap_until_initialization_finishes() {
        let directory = TestDirectory::new();
        let database_path = directory.database_path();

        assert!(
            prepare_keyless_database_bootstrap(&database_path)
                .expect("prepare first-run key bootstrap")
        );
        assert!(key_bootstrap_marker_is_valid(&database_path).expect("validate first-run marker"));
        assert!(
            database_allows_existing_key(&database_path)
                .expect("recognize interrupted first-run bootstrap")
        );

        let key = DatabaseKey::from_bytes(&[0xA5; 32]).expect("create test database key");
        let database = key
            .open(
                &database_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .expect("create encrypted test database");
        database
            .execute_batch("CREATE TABLE probe (value TEXT NOT NULL);")
            .expect("initialize encrypted test database");
        drop(database);

        super::clear_key_bootstrap_marker(&database_path)
            .expect("clear completed first-run marker");
        assert!(!key_bootstrap_marker_path(&database_path).exists());
        assert!(
            database_allows_existing_key(&database_path)
                .expect("accept initialized encrypted database")
        );
    }

    #[test]
    fn saved_key_with_missing_database_fails_closed_without_recovery_evidence() {
        let directory = TestDirectory::new();

        assert!(
            !database_allows_existing_key(&directory.database_path())
                .expect("inspect missing isolated database")
        );
    }

    #[test]
    fn broker_protocol_rejects_saved_key_when_active_database_is_missing() {
        let directory = TestDirectory::new();
        let mut input = Vec::from(*KEY_BOOTSTRAP_MAGIC);
        input.push(1);
        input.extend_from_slice(&[0xA5; 32]);
        let mut reader = Cursor::new(input);
        let mut output = Vec::new();

        let error = match receive_database_key(&mut reader, &mut output, &directory.database_path())
        {
            Ok(_) => panic!("missing active database must not initialize an empty profile"),
            Err(error) => error,
        };

        assert!(error.contains("no active or recoverable storage database"));
        let mut expected = Vec::from(*KEY_BOOTSTRAP_MAGIC);
        expected.push(3);
        assert_eq!(output, expected);
    }

    #[test]
    fn broker_protocol_creates_recovery_marker_before_requesting_first_key() {
        let directory = TestDirectory::new();
        let database_path = directory.database_path();
        let mut input = Vec::from(*KEY_BOOTSTRAP_MAGIC);
        input.push(0);
        input.extend_from_slice(&[0xA5; 32]);
        let mut reader = Cursor::new(input);
        let mut output = Vec::new();

        let key = receive_database_key(&mut reader, &mut output, &database_path)
            .expect("initialize first-run database key");

        assert_eq!(key.as_bytes(), &[0xA5; 32]);
        let mut expected = Vec::from(*KEY_BOOTSTRAP_MAGIC);
        expected.push(1);
        assert_eq!(output, expected);
        assert!(
            key_bootstrap_marker_is_valid(&database_path)
                .expect("validate marker before database creation")
        );
    }

    #[test]
    fn interrupted_plaintext_migration_requires_a_valid_journal_and_source() {
        let directory = TestDirectory::new();
        let database_path = directory.database_path();
        let source_path = directory
            .0
            .join("openchat.sqlite3.plaintext-migration-source");
        let journal_path = directory.0.join("openchat.sqlite3.sqlcipher-migration");
        let source = Connection::open(&source_path).expect("create plaintext recovery source");
        source
            .execute_batch("CREATE TABLE probe (value TEXT NOT NULL);")
            .expect("write plaintext recovery source");
        drop(source);
        fs::write(&journal_path, b"OpenChat SQLCipher migration v1\n")
            .expect("write migration recovery journal");

        assert!(
            database_allows_existing_key(&database_path)
                .expect("accept valid interrupted migration")
        );

        fs::write(&journal_path, b"invalid\n").expect("corrupt migration recovery journal");
        assert!(database_allows_existing_key(&database_path).is_err());
    }

    #[test]
    fn keyless_bootstrap_rejects_encrypted_or_incomplete_existing_databases() {
        let directory = TestDirectory::new();
        let database_path = directory.database_path();
        let key = DatabaseKey::from_bytes(&[0xA5; 32]).expect("create test database key");
        let database = key
            .open(
                &database_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .expect("create encrypted test database");
        database
            .execute_batch("CREATE TABLE probe (value TEXT NOT NULL);")
            .expect("initialize encrypted test database");
        drop(database);

        assert!(
            !prepare_keyless_database_bootstrap(&database_path)
                .expect("reject keyless encrypted database")
        );

        fs::remove_file(&database_path).expect("remove only the isolated encrypted database");
        fs::write(&database_path, b"partial").expect("create incomplete isolated database");
        assert!(
            !prepare_keyless_database_bootstrap(&database_path)
                .expect("reject keyless incomplete database")
        );
    }

    #[test]
    fn malformed_bootstrap_marker_is_never_accepted() {
        let directory = TestDirectory::new();
        let database_path = directory.database_path();
        let marker_path = key_bootstrap_marker_path(&database_path);
        fs::write(&marker_path, KEY_BOOTSTRAP_MARKER_CONTENT)
            .expect("write isolated bootstrap marker");
        fs::write(&marker_path, b"invalid\n").expect("corrupt isolated bootstrap marker");

        assert!(key_bootstrap_marker_is_valid(&database_path).is_err());
        assert!(database_allows_existing_key(&database_path).is_err());
    }

    #[test]
    fn broker_stdio_persists_encrypted_data_in_an_isolated_profile() {
        let directory = TestDirectory::new();
        let profile_root = directory.0.join("profile");
        fs::create_dir(&profile_root).expect("create isolated broker profile");
        let key = [0xA5; 32];
        let hello = json!({
            "operation": "hello",
            "version": STORAGE_PROTOCOL_VERSION,
        });
        let create_and_insert = json!({
            "operation": "execute_batch",
            "sql": "CREATE TABLE broker_probe (value TEXT NOT NULL); INSERT INTO broker_probe VALUES ('persisted');",
        });
        let query = json!({
            "operation": "query",
            "sql": "SELECT value FROM broker_probe",
            "parameters": [],
        });

        let first_output = run_test_broker(
            &profile_root,
            0,
            &key,
            &[hello.clone(), create_and_insert, query.clone()],
        );
        assert_eq!(&first_output[..5], b"OCK1\x01");
        assert_eq!(&first_output[5..10], b"OCK1\x02");
        let first_responses = parse_broker_responses(&first_output, 10);
        assert_eq!(first_responses[0]["result"], "ready");
        assert_eq!(first_responses[1]["result"], "unit");
        assert_eq!(first_responses[2]["rows"][0][0]["value"], "persisted");

        let database_path = profile_root
            .join("OpenChatStorage")
            .join("db")
            .join("openchat.sqlite3");
        let encrypted_database =
            fs::read(&database_path).expect("read isolated encrypted database");
        assert!(!encrypted_database.starts_with(b"SQLite format 3\0"));

        let second_output = run_test_broker(&profile_root, 1, &key, &[hello, query]);
        assert_eq!(&second_output[..5], b"OCK1\x02");
        let second_responses = parse_broker_responses(&second_output, 5);
        assert_eq!(second_responses[0]["result"], "ready");
        assert_eq!(second_responses[1]["rows"][0][0]["value"], "persisted");
    }

    #[test]
    fn broker_protocol_accepts_profile_restore_commit_requests() {
        let directory = TestDirectory::new();
        let profile_root = directory.0.join("restore-profile");
        fs::create_dir(&profile_root).expect("create isolated broker profile");
        let hello = json!({
            "operation": "hello",
            "version": STORAGE_PROTOCOL_VERSION,
        });
        let commit_restore = json!({
            "operation": "commit_pending_profile_restore",
        });

        let output = run_test_broker(&profile_root, 0, &[0xA5; 32], &[hello, commit_restore]);

        assert_eq!(&output[..5], b"OCK1\x01");
        assert_eq!(&output[5..10], b"OCK1\x02");
        let responses = parse_broker_responses(&output, 10);
        assert_eq!(responses[0]["result"], "ready");
        assert_eq!(responses[1]["result"], "unit");
    }

    fn run_test_broker(
        profile_root: &std::path::Path,
        bootstrap_mode: u8,
        key: &[u8; 32],
        requests: &[Value],
    ) -> Vec<u8> {
        let mut input = Vec::from(*KEY_BOOTSTRAP_MAGIC);
        input.push(bootstrap_mode);
        input.extend_from_slice(key);
        for request in requests {
            input.extend_from_slice(
                &serde_json::to_vec(request).expect("encode broker test request"),
            );
            input.push(b'\n');
        }
        let mut reader = Cursor::new(input);
        let mut output = Vec::new();
        run_broker_profile(profile_root, &mut reader, &mut output)
            .expect("run broker against isolated profile");
        output
    }

    fn parse_broker_responses(output: &[u8], status_bytes: usize) -> Vec<Value> {
        output[status_bytes..]
            .split(|byte| *byte == b'\n')
            .filter(|frame| !frame.is_empty())
            .map(|frame| serde_json::from_slice(frame).expect("decode broker test response"))
            .collect()
    }
}
