//! Windows storage broker host with legacy SCM and isolated AppContainer transports.

use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    os::windows::ffi::OsStringExt,
    os::windows::io::{AsRawHandle, FromRawHandle},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
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
use windows_service::{
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_DATA, ERROR_PIPE_CONNECTED,
        ERROR_PIPE_NOT_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
        LocalFree,
    },
    Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    },
    Security::Isolation::GetAppContainerFolderPath,
    Security::{
        GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_APPCONTAINER_INFORMATION, TOKEN_QUERY,
        TOKEN_USER, TokenAppContainerSid, TokenIsAppContainer, TokenUser,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING,
        PIPE_ACCESS_DUPLEX,
    },
    System::{
        Com::CoTaskMemFree,
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
            ImpersonateNamedPipeClient, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
        },
        Threading::{
            GetCurrentProcess, GetCurrentThread, OpenProcess, OpenProcessToken, OpenThreadToken,
            PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
    },
};
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "OpenChatStorage";
const SERVICE_SID: &str = "S-1-5-80-3744228697-793638913-1717126734-3587791738-2419156616";
const PIPE_NAME: &str = r"\\.\pipe\OpenChat.Storage.v1";
const SDDL_REVISION_1: u32 = 1;
const PIPE_BUFFER_BYTES: u32 = 64 * 1024;
const MAX_STORAGE_CLIENTS: usize = 32;
const PROCESS_IMAGE_BUFFER_UNITS: usize = 32 * 1024;

/// Runs the protected storage broker under the Windows Service Control Manager.
pub fn run() -> Result<(), String> {
    let result = run_inner();
    if let Err(error) = &result {
        write_service_diagnostic(error);
    }
    result
}

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
    serve_profile_connection(
        &mut reader,
        &mut writer,
        &profile_root,
        "OpenChatStorage",
        &profile,
    )
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

fn run_inner() -> Result<(), String> {
    let (stop_sender, stop_receiver) = mpsc::channel();
    let status_handle = service_control_handler::register(SERVICE_NAME, move |event| match event {
        ServiceControl::Stop => {
            write_service_diagnostic("service control: stop");
            let _ = stop_sender.send(());
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Shutdown => {
            write_service_diagnostic("service control: shutdown");
            let _ = stop_sender.send(());
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })
    .map_err(|error| format!("The storage service control handler could not start: {error}"))?;

    set_service_status(
        &status_handle,
        ServiceState::StartPending,
        ServiceControlAccept::empty(),
    )?;

    let storage_root = protected_storage_root()?;
    let stop_requested = Arc::new(AtomicBool::new(false));
    let profiles = Arc::new(Mutex::new(HashMap::<String, Arc<AppStorage>>::new()));
    let active_clients = Arc::new(AtomicUsize::new(0));
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let (listener_sender, listener_receiver) = mpsc::channel();
    let server_stop = Arc::clone(&stop_requested);
    let server_profiles = Arc::clone(&profiles);
    let server_clients = Arc::clone(&active_clients);
    let listener = thread::Builder::new()
        .name("openchat-storage-pipe".to_owned())
        .spawn(move || {
            let result = accept_clients(
                storage_root,
                server_profiles,
                server_clients,
                server_stop,
                ready_sender,
            );
            let _ = listener_sender.send(result.clone());
            result
        })
        .map_err(|error| format!("The storage pipe listener could not start: {error}"))?;

    match ready_receiver.recv() {
        Ok(Ok(())) => set_service_status(
            &status_handle,
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        )?,
        Ok(Err(error)) => {
            let _ = listener.join();
            set_service_status(
                &status_handle,
                ServiceState::Stopped,
                ServiceControlAccept::empty(),
            )?;
            return Err(error);
        }
        Err(error) => {
            let _ = listener.join();
            set_service_status(
                &status_handle,
                ServiceState::Stopped,
                ServiceControlAccept::empty(),
            )?;
            return Err(format!(
                "The storage pipe readiness channel failed: {error}"
            ));
        }
    }

    loop {
        match stop_receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(()) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => match listener_receiver.try_recv() {
                Ok(Err(error)) => {
                    set_service_status(
                        &status_handle,
                        ServiceState::Stopped,
                        ServiceControlAccept::empty(),
                    )?;
                    let _ = listener.join();
                    return Err(error);
                }
                Ok(Ok(())) => {
                    set_service_status(
                        &status_handle,
                        ServiceState::Stopped,
                        ServiceControlAccept::empty(),
                    )?;
                    let _ = listener.join();
                    return Err("The storage pipe listener stopped unexpectedly.".to_owned());
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    set_service_status(
                        &status_handle,
                        ServiceState::Stopped,
                        ServiceControlAccept::empty(),
                    )?;
                    let _ = listener.join();
                    return Err("The storage pipe listener exited unexpectedly.".to_owned());
                }
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                set_service_status(
                    &status_handle,
                    ServiceState::Stopped,
                    ServiceControlAccept::empty(),
                )?;
                stop_requested.store(true, Ordering::Release);
                wake_pipe_listener();
                let _ = listener.join();
                return Err("The storage service stop channel failed.".to_owned());
            }
        }
    }

    set_service_status(
        &status_handle,
        ServiceState::StopPending,
        ServiceControlAccept::empty(),
    )?;
    stop_requested.store(true, Ordering::Release);
    wake_pipe_listener();
    listener
        .join()
        .map_err(|_| "The storage pipe listener thread failed.".to_owned())??;
    set_service_status(
        &status_handle,
        ServiceState::Stopped,
        ServiceControlAccept::empty(),
    )?;
    Ok(())
}

fn write_service_diagnostic(message: &str) {
    if let Err(error) = persist_service_diagnostic(message) {
        eprintln!("openchat-storage-service diagnostic could not be recorded: {error}");
    }
}

fn persist_service_diagnostic(message: &str) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;

    let program_data = env::var_os("PROGRAMDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "ProgramData is unavailable"))?;
    let directory = program_data.join("OpenChat").join("Storage");
    validate_directory_chain(&directory).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the protected diagnostics directory is unavailable",
        )
    })?;

    let path = directory.join("service-error.log");
    match fs::symlink_metadata(&path) {
        Ok(metadata)
            if !metadata.is_file()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 =>
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the protected diagnostic file is not a regular file",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let safe_message = message
        .chars()
        .filter(|character| !character.is_control())
        .take(512)
        .collect::<String>();
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    writeln!(file, "{timestamp} {safe_message}")?;
    file.sync_all()
}

fn set_service_status(
    handle: &windows_service::service_control_handler::ServiceStatusHandle,
    state: ServiceState,
    controls: ServiceControlAccept,
) -> Result<(), String> {
    handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: controls,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::ZERO,
            process_id: None,
        })
        .map_err(|error| format!("The storage service status could not be updated: {error}"))
}

fn protected_storage_root() -> Result<PathBuf, String> {
    let program_data = env::var_os("PROGRAMDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| "The protected ProgramData storage root is unavailable.".to_owned())?;
    let root = program_data
        .join("OpenChat")
        .join("Storage")
        .join("Profiles");
    validate_directory_chain(&root)?;
    Ok(root)
}

fn validate_directory_chain(path: &Path) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;

    for (component_depth, directory) in path
        .ancestors()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .enumerate()
    {
        let metadata = fs::symlink_metadata(directory).map_err(|error| {
            format!(
                "The protected storage directory could not be inspected (component_depth={component_depth}, error_kind={:?}).",
                error.kind()
            )
        })?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("The protected storage path contains a reparse point.".to_owned());
        }
        if !metadata.is_dir() {
            return Err(
                "The protected storage path contains a non-directory component.".to_owned(),
            );
        }
    }
    Ok(())
}

fn ensure_storage_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(path)
                .map_err(|_| "The protected storage directory could not be created.".to_owned())?;
        }
        Err(_) => {
            return Err("The protected storage directory could not be inspected.".to_owned());
        }
    }
    validate_directory_chain(path)
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

fn accept_clients(
    storage_root: PathBuf,
    profiles: Arc<Mutex<HashMap<String, Arc<AppStorage>>>>,
    active_clients: Arc<AtomicUsize>,
    stop_requested: Arc<AtomicBool>,
    ready_sender: mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let mut first_instance = true;
    let mut ready_sender = Some(ready_sender);
    while !stop_requested.load(Ordering::Acquire) {
        let server = match create_pipe_server(first_instance) {
            Ok(server) => server,
            Err(error) => {
                if let Some(sender) = ready_sender.take() {
                    let _ = sender.send(Err(error.clone()));
                }
                return Err(error);
            }
        };
        if let Some(sender) = ready_sender.take() {
            let _ = sender.send(Ok(()));
        }
        first_instance = false;
        if !connect_pipe(server.as_raw_handle().cast())? {
            continue;
        }
        if stop_requested.load(Ordering::Acquire) {
            break;
        }
        if active_clients.fetch_add(1, Ordering::AcqRel) >= MAX_STORAGE_CLIENTS {
            active_clients.fetch_sub(1, Ordering::AcqRel);
            continue;
        }

        let root = storage_root.clone();
        let profiles = Arc::clone(&profiles);
        let active_clients = Arc::clone(&active_clients);
        let worker = thread::Builder::new()
            .name("openchat-storage-client".to_owned())
            .spawn(move || {
                let _client_guard = ActiveClientGuard(active_clients);
                if let Err(error) = serve_client(server, &root, &profiles) {
                    eprintln!("openchat-storage-service-client-error: {error}");
                }
            })
            .map_err(|error| format!("A storage client worker could not start: {error}"))?;
        drop(worker);
    }
    Ok(())
}

struct ActiveClientGuard(Arc<AtomicUsize>);

impl Drop for ActiveClientGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn create_pipe_server(first_instance: bool) -> Result<File, String> {
    let descriptor = pipe_security_descriptor()?;
    let security_attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let pipe_name = wide(PIPE_NAME);
    let open_mode = PIPE_ACCESS_DUPLEX
        | if first_instance {
            FILE_FLAG_FIRST_PIPE_INSTANCE
        } else {
            0
        };
    // SAFETY: `security_attributes` and its descriptor remain live for the duration of
    // CreateNamedPipeW, and `pipe_name` is a NUL-terminated UTF-16 string.
    let handle = unsafe {
        CreateNamedPipeW(
            pipe_name.as_ptr(),
            open_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            PIPE_BUFFER_BYTES,
            PIPE_BUFFER_BYTES,
            0,
            &security_attributes,
        )
    };
    // SAFETY: The descriptor was allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW.
    unsafe { LocalFree(descriptor.cast()) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!(
            "The storage named pipe could not be created: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: CreateNamedPipeW returned a valid, owned pipe handle.
    Ok(unsafe { File::from_raw_handle(handle.cast()) })
}

fn pipe_security_descriptor() -> Result<*mut std::ffi::c_void, String> {
    // The restricted service token needs its service SID on the pipe ACL to wake its own listener.
    let sddl = wide(&format!(
        "D:P(D;;GA;;;NU)(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;LS)(A;;GA;;;{SERVICE_SID})(A;;GRGW;;;AU)"
    ));
    let mut descriptor = null_mut();
    // SAFETY: `sddl` is NUL-terminated and `descriptor` is a writable output pointer.
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    };
    if converted == 0 || descriptor.is_null() {
        return Err(format!(
            "The storage named pipe ACL could not be created: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(descriptor)
}

fn connect_pipe(pipe: HANDLE) -> Result<bool, String> {
    // SAFETY: `pipe` is a live named-pipe server handle created by CreateNamedPipeW.
    let connected = unsafe { ConnectNamedPipe(pipe, null_mut()) };
    if connected != 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(code) if code == ERROR_PIPE_CONNECTED as i32 => Ok(true),
        Some(code) if code == ERROR_NO_DATA as i32 || code == ERROR_PIPE_NOT_CONNECTED as i32 => {
            // A client can disconnect before the server starts servicing this instance. Drop it
            // and create a fresh pipe instance instead of taking down the service.
            Ok(false)
        }
        _ => Err(format!("A storage pipe client could not connect: {error}")),
    }
}

fn serve_client(
    mut pipe: File,
    storage_root: &std::path::Path,
    profiles: &Mutex<HashMap<String, Arc<AppStorage>>>,
) -> Result<(), String> {
    let client_pid = pipe_client_process_id(pipe.as_raw_handle().cast())?;
    verify_client_executable(client_pid)?;
    let profile_sid = impersonated_client_sid(pipe.as_raw_handle().cast())?;
    if !valid_sid_component(&profile_sid) {
        return Err("The storage pipe client has an invalid Windows user SID.".to_owned());
    }

    let mut reader = BufReader::new(pipe.try_clone().map_err(|error| error.to_string())?);
    let profile = storage_for_user(storage_root, profiles, &profile_sid)?;
    serve_profile_connection(&mut reader, &mut pipe, storage_root, &profile_sid, &profile)
}

fn serve_profile_connection<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    storage_root: &Path,
    profile_sid: &str,
    profile: &AppStorage,
) -> Result<(), String> {
    let request = read_request_frame(reader)?;
    let request: StorageRequest = serde_json::from_slice(&request)
        .map_err(|_| "The storage pipe handshake is malformed.".to_owned())?;
    if !matches!(request, StorageRequest::Hello { version } if version == STORAGE_PROTOCOL_VERSION)
    {
        write_response_frame(
            writer,
            &StorageResponse::Error {
                code: rusqlite::ffi::SQLITE_MISUSE,
                message: "The storage pipe protocol version is unsupported.".to_owned(),
            },
        )?;
        return Err("The storage pipe protocol version is unsupported.".to_owned());
    }

    write_response_frame(
        writer,
        &StorageResponse::Ready {
            version: STORAGE_PROTOCOL_VERSION,
        },
    )?;

    // Autocommit requests lease the database only while that request runs. An explicit
    // transaction keeps its lease on this client pipe until it commits, rolls back, or disconnects.
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
            storage_root,
            profile_sid,
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
            storage_root,
            profile_sid,
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
    storage_root: &Path,
    profile_sid: &str,
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
                let directory = storage_root.join(profile_sid).join("cache");
                ensure_storage_directory(&directory)?;
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
    storage_root: &Path,
    profile_sid: &str,
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
                let directory = storage_root.join(profile_sid).join("db").join("imports");
                ensure_storage_directory(&directory)?;
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

fn storage_for_user(
    storage_root: &std::path::Path,
    profiles: &Mutex<HashMap<String, Arc<AppStorage>>>,
    profile_sid: &str,
) -> Result<Arc<AppStorage>, String> {
    let mut profiles = profiles
        .lock()
        .map_err(|_| "The storage profile map is unavailable.".to_owned())?;
    if let Some(storage) = profiles.get(profile_sid) {
        return Ok(Arc::clone(storage));
    }

    validate_directory_chain(storage_root)?;
    let root = storage_root.join(profile_sid);
    ensure_storage_directory(&root)?;
    for child in ["db", "logs", "cache"] {
        ensure_storage_directory(&root.join(child))?;
    }
    let storage = AppStorage::open_at(root)
        .map_err(|_| "The protected storage profile could not open safely.".to_owned())?;
    storage
        .prepare_chat_schema_migration(crate::CHAT_SCHEMA_VERSION)
        .map_err(|_| {
            "The protected storage schema could not be validated or repaired.".to_owned()
        })?;
    storage
        .initialize_backend_schema()
        .map_err(|_| "The protected storage schema could not be initialized.".to_owned())?;
    let storage = Arc::new(storage);
    profiles.insert(profile_sid.to_owned(), Arc::clone(&storage));
    Ok(storage)
}

fn pipe_client_process_id(pipe: HANDLE) -> Result<u32, String> {
    let mut process_id = 0;
    // SAFETY: `pipe` is a live named-pipe server handle and `process_id` is writable.
    let success = unsafe { GetNamedPipeClientProcessId(pipe, &mut process_id) };
    if success == 0 || process_id == 0 {
        return Err(format!(
            "The storage pipe client process could not be identified: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(process_id)
}

fn verify_client_executable(process_id: u32) -> Result<(), String> {
    // SAFETY: OpenProcess requests query-only access to a live client process.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return Err(
            "The storage pipe client process could not be opened for identity checks.".to_owned(),
        );
    }
    let _process_guard = HandleGuard(process);
    let mut image = vec![0u16; PROCESS_IMAGE_BUFFER_UNITS];
    let mut image_length = image.len() as u32;
    // SAFETY: `image` is a writable UTF-16 buffer and `image_length` contains its capacity.
    let queried =
        unsafe { QueryFullProcessImageNameW(process, 0, image.as_mut_ptr(), &mut image_length) };
    if queried == 0 {
        return Err("The storage pipe client image path could not be verified.".to_owned());
    }
    let actual = std::fs::canonicalize(PathBuf::from(OsString::from_wide(
        &image[..image_length as usize],
    )))
    .map_err(|_| "The storage pipe client image path could not be canonicalized.".to_owned())?;
    let service_image = std::env::current_exe()
        .map_err(|_| "The storage service image path is unavailable.".to_owned())?;
    let install_directory = service_image
        .parent()
        .ok_or_else(|| "The storage service has no installation directory.".to_owned())?;
    let expected_client =
        std::fs::canonicalize(install_directory.join("openchat-local-service.exe"))
            .map_err(|_| "The installed local service executable is unavailable.".to_owned())?;
    if !same_path(&actual, &expected_client) {
        return Err(
            "The storage pipe client is not the installed OpenChat local service.".to_owned(),
        );
    }

    let parent_id = client_parent_process_id(process_id)?;
    // SAFETY: The parent PID came from the operating system's process snapshot and receives query-only access.
    let parent = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, parent_id) };
    if parent.is_null() {
        return Err("The storage pipe client parent process could not be verified.".to_owned());
    }
    let _parent_guard = HandleGuard(parent);
    let mut parent_image = vec![0u16; PROCESS_IMAGE_BUFFER_UNITS];
    let mut parent_image_length = parent_image.len() as u32;
    // SAFETY: `parent_image` is a writable UTF-16 buffer and its length is its capacity.
    let queried = unsafe {
        QueryFullProcessImageNameW(
            parent,
            0,
            parent_image.as_mut_ptr(),
            &mut parent_image_length,
        )
    };
    if queried == 0 {
        return Err("The storage pipe client parent image path could not be verified.".to_owned());
    }
    let actual_parent = std::fs::canonicalize(PathBuf::from(OsString::from_wide(
        &parent_image[..parent_image_length as usize],
    )))
    .map_err(|_| {
        "The storage pipe client parent image path could not be canonicalized.".to_owned()
    })?;
    let expected_parent = std::fs::canonicalize(install_directory.join("openchat-desktop.exe"))
        .map_err(|_| "The installed OpenChat desktop executable is unavailable.".to_owned())?;
    if !same_path(&actual_parent, &expected_parent) {
        return Err("The storage pipe client was not started by the installed OpenChat desktop application.".to_owned());
    }

    Ok(())
}

fn client_parent_process_id(process_id: u32) -> Result<u32, String> {
    // SAFETY: A process snapshot with a process-only flag and zero PID is valid.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
        return Err("The storage pipe client parent process could not be located.".to_owned());
    }
    let _snapshot_guard = HandleGuard(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: `entry` is initialized with the required structure size and writable fields.
    if unsafe { Process32FirstW(snapshot, &mut entry) } == 0 {
        return Err("The storage pipe client process list could not be read.".to_owned());
    }
    loop {
        if entry.th32ProcessID == process_id {
            return Ok(entry.th32ParentProcessID);
        }
        // SAFETY: `entry` is writable and the snapshot remains open for the full enumeration.
        if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
            break;
        }
    }
    Err("The storage pipe client process disappeared before authentication.".to_owned())
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

fn impersonated_client_sid(pipe: HANDLE) -> Result<String, String> {
    // SAFETY: The live pipe handle has an authenticated client attached.
    if unsafe { ImpersonateNamedPipeClient(pipe) } == 0 {
        return Err(format!(
            "The storage pipe client could not be impersonated: {}",
            io::Error::last_os_error()
        ));
    }
    let _impersonation_guard = ImpersonationGuard;
    let mut token = null_mut();
    // SAFETY: GetCurrentThread returns a valid pseudo handle; token is a writable output pointer.
    if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) } == 0 {
        return Err(format!(
            "The storage pipe client token could not be opened: {}",
            io::Error::last_os_error()
        ));
    }
    let _token_guard = HandleGuard(token);
    let mut required_bytes = 0;
    // SAFETY: A null output buffer with length zero requests the required token information size.
    unsafe {
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut required_bytes);
    }
    if required_bytes == 0
        || io::Error::last_os_error().raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32)
    {
        return Err("The storage pipe client user SID could not be read.".to_owned());
    }
    let word_count = (required_bytes as usize).div_ceil(std::mem::size_of::<u64>());
    let mut token_buffer = vec![0u64; word_count];
    // SAFETY: `token_buffer` is aligned for TOKEN_USER and has the size returned by Windows.
    if unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            token_buffer.as_mut_ptr().cast(),
            required_bytes,
            &mut required_bytes,
        )
    } == 0
    {
        return Err(format!(
            "The storage pipe client user SID could not be read: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: The successful TokenUser query initialized a TOKEN_USER at the start of the buffer.
    let user = unsafe { &*token_buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut sid_string = null_mut();
    // SAFETY: The token-owned SID remains valid while `token` and `token_buffer` are alive.
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid_string) } == 0
        || sid_string.is_null()
    {
        return Err(format!(
            "The storage pipe client SID could not be formatted: {}",
            io::Error::last_os_error()
        ));
    }
    let sid = unsafe {
        let mut length = 0;
        while *sid_string.add(length) != 0 {
            length += 1;
        }
        OsString::from_wide(std::slice::from_raw_parts(sid_string, length))
    };
    // SAFETY: The SID string was allocated by ConvertSidToStringSidW.
    unsafe { LocalFree(sid_string.cast()) };
    Ok(sid.to_string_lossy().into_owned())
}

fn valid_sid_component(sid: &str) -> bool {
    sid.starts_with("S-1-")
        && sid.len() <= 184
        && sid
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'-')
}

fn read_request_frame(reader: &mut impl BufRead) -> Result<Vec<u8>, String> {
    read_request_frame_optional(reader)?
        .ok_or_else(|| "The storage pipe handshake is empty.".to_owned())
}

fn read_request_frame_optional(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut frame = Vec::new();
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|error| format!("The storage pipe request could not be read: {error}"))?;
        if buffer.is_empty() {
            if frame.is_empty() {
                return Ok(None);
            }
            return Err("The storage pipe ended during a request frame.".to_owned());
        }

        if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            if frame.len().saturating_add(newline) > MAX_STORAGE_FRAME_BYTES {
                return Err("The storage pipe request exceeds its size limit.".to_owned());
            }
            frame.extend_from_slice(&buffer[..newline]);
            reader.consume(newline.saturating_add(1));
            if frame.last() == Some(&b'\r') {
                frame.pop();
            }
            return Ok(Some(frame));
        }

        if frame.len().saturating_add(buffer.len()) > MAX_STORAGE_FRAME_BYTES {
            return Err("The storage pipe request exceeds its size limit.".to_owned());
        }
        let consumed = buffer.len();
        frame.extend_from_slice(buffer);
        reader.consume(consumed);
    }
}

fn write_response_frame(writer: &mut impl Write, response: &StorageResponse) -> Result<(), String> {
    let mut frame = Zeroizing::new(
        serde_json::to_vec(response)
            .map_err(|_| "The storage pipe response could not be encoded.".to_owned())?,
    );
    if frame.len() > MAX_STORAGE_FRAME_BYTES {
        return Err("The storage pipe response exceeds its size limit.".to_owned());
    }
    frame.push(b'\n');
    writer
        .write_all(&frame)
        .and_then(|()| writer.flush())
        .map_err(|error| format!("The storage pipe response could not be written: {error}"))
}

fn wake_pipe_listener() {
    let pipe_name = wide(PIPE_NAME);
    // SAFETY: The pipe path is a fixed local named pipe and the returned handle is closed immediately.
    let handle = unsafe {
        CreateFileW(
            pipe_name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
        // SAFETY: CreateFileW returned an owned wake-up handle.
        unsafe { CloseHandle(handle) };
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
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

struct ImpersonationGuard;

impl Drop for ImpersonationGuard {
    fn drop(&mut self) {
        // SAFETY: This guard is constructed only after a successful pipe-client impersonation.
        let _ = unsafe { windows_sys::Win32::Security::RevertToSelf() };
    }
}
