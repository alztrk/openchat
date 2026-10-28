//! Windows Access Control List (ACL) and securable object permissions management.

use std::{
    env, fs, io,
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
    ptr::{NonNull, null_mut},
    sync::Mutex,
    time::{Duration, Instant},
};

use windows_sys::Wdk::Storage::FileSystem::NtSetSecurityObject;
use windows_sys::Win32::{
    Foundation::{ERROR_SUCCESS, HANDLE, LocalFree, RtlNtStatusToDosError},
    Security::{
        ACL,
        Authorization::{
            EXPLICIT_ACCESS_W, GRANT_ACCESS, GetSecurityInfo, REVOKE_ACCESS, SE_FILE_OBJECT,
            SE_WINDOW_OBJECT, SET_ACCESS, SetEntriesInAclW, SetSecurityInfo, TRUSTEE_IS_SID,
            TRUSTEE_IS_UNKNOWN, TRUSTEE_IS_USER, TRUSTEE_W,
        },
        CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl,
        InitializeSecurityDescriptor, NO_INHERITANCE, OBJECT_INHERIT_ACE, PSECURITY_DESCRIPTOR,
        PSID, SE_DACL_AUTO_INHERIT_REQ, SE_DACL_AUTO_INHERITED, SE_DACL_DEFAULTED,
        SE_DACL_PROTECTED, SECURITY_DESCRIPTOR, SetSecurityDescriptorControl,
        SetSecurityDescriptorDacl,
    },
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY,
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_EXECUTE,
        FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, GetFileInformationByHandle,
        GetFinalPathNameByHandleW, READ_CONTROL, STANDARD_RIGHTS_READ, STANDARD_RIGHTS_WRITE,
        VOLUME_NAME_DOS, WRITE_DAC,
    },
    System::{
        StationsAndDesktops::{GetProcessWindowStation, GetThreadDesktop},
        SystemServices::MAXIMUM_ALLOWED,
        Threading::GetCurrentThreadId,
    },
};

use super::types::last_os_error;

/// Caps the number of file-system paths scheduled for one AppContainer ACL traversal.
pub(crate) const MAX_ACL_ENTRIES: usize = 20_000;
pub(crate) const ACL_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const ACL_LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(25);
pub(crate) static APP_CONTAINER_ACL_LOCK: Mutex<()> = Mutex::new(());

/// Returns the stable lock-file path shared by OpenChat processes for one local profile.
pub(crate) fn app_container_acl_lock_path(local_app_data: &Path) -> PathBuf {
    local_app_data
        .join("OpenChat")
        .join("appcontainer-acl.lock")
}

/// Grants minimal window station and desktop access to the AppContainer SID.
pub(crate) fn grant_user_objects_access(sid: PSID) -> io::Result<()> {
    // SAFETY: GetProcessWindowStation returns the current process window station handle.
    let winsta = unsafe { GetProcessWindowStation() };
    if !winsta.is_null() {
        let winsta_mask = STANDARD_RIGHTS_READ
            | 0x0001 /* WINSTA_ENUMDESKTOPS */
            | 0x0002 /* WINSTA_READATTRIBUTES */
            | 0x0004 /* WINSTA_ACCESSCLIPBOARD */
            | 0x0020 /* WINSTA_ACCESSGLOBALATOMS */
            | 0x0100 /* WINSTA_ENUMERATE */;
        update_user_object_acl(winsta as _, sid, true, winsta_mask)?;
    }
    // SAFETY: GetCurrentThreadId retrieves the active thread ID for the desktop query.
    let thread_id = unsafe { GetCurrentThreadId() };
    // SAFETY: GetThreadDesktop returns the desktop assigned to the calling thread.
    let desk = unsafe { GetThreadDesktop(thread_id) };
    if !desk.is_null() {
        let desk_mask = STANDARD_RIGHTS_READ
            | STANDARD_RIGHTS_WRITE
            | 0x0001 /* DESKTOP_READOBJECTS */
            | 0x0002 /* DESKTOP_CREATEWINDOW */
            | 0x0004 /* DESKTOP_CREATEMENU */
            | 0x0020 /* DESKTOP_HOOKCONTROL */
            | 0x0040 /* DESKTOP_ENUMERATE */
            | 0x0080 /* DESKTOP_WRITEOBJECTS */;
        update_user_object_acl(desk as _, sid, true, desk_mask)?;
    }
    Ok(())
}

/// Revokes window station and desktop access from the AppContainer SID.
pub(crate) fn revoke_user_objects_access(sid: PSID) {
    // SAFETY: Querying the current window station and desktop to revoke the AppContainer SID ACE.
    let winsta = unsafe { GetProcessWindowStation() };
    if !winsta.is_null() {
        let _ = update_user_object_acl(winsta as _, sid, false, 0);
    }
    // SAFETY: GetCurrentThreadId retrieves the active thread ID for the desktop query.
    let thread_id = unsafe { GetCurrentThreadId() };
    // SAFETY: GetThreadDesktop returns the desktop assigned to the calling thread.
    let desk = unsafe { GetThreadDesktop(thread_id) };
    if !desk.is_null() {
        let _ = update_user_object_acl(desk as _, sid, false, 0);
    }
}

/// Updates the discretionary ACL on a window station or desktop object for the given SID.
pub(crate) fn update_user_object_acl(
    handle: HANDLE,
    sid: PSID,
    allow: bool,
    access_mask: u32,
) -> io::Result<()> {
    if handle.is_null() {
        return Ok(());
    }
    let _guard = APP_CONTAINER_ACL_LOCK
        .lock()
        .map_err(|_| io::Error::other("The AppContainer ACL lock is unavailable."))?;
    let mut p_dacl: *mut ACL = null_mut();
    let mut p_sd: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: GetSecurityInfo queries the security descriptor of the specified window object.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_WINDOW_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut p_dacl,
            null_mut(),
            &mut p_sd,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    struct LocalFreeGuard(PSECURITY_DESCRIPTOR);
    impl Drop for LocalFreeGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: Pointer was allocated by a Windows security API.
                unsafe { LocalFree(self.0.cast()) };
            }
        }
    }
    let _sd_guard = LocalFreeGuard(p_sd);

    let mode = if allow { GRANT_ACCESS } else { REVOKE_ACCESS };

    let explicit_access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: if allow { access_mask } else { 0 },
        grfAccessMode: mode,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: sid as *mut u16,
        },
    };
    let mut new_dacl: *mut ACL = null_mut();
    // SAFETY: SetEntriesInAclW modifies the DACL by adding or revoking the trustee entry.
    let status = unsafe { SetEntriesInAclW(1, &explicit_access, p_dacl, &mut new_dacl) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _dacl_guard = LocalFreeGuard(new_dacl as _);

    // SAFETY: SetSecurityInfo applies the modified DACL to the window object handle.
    let status = unsafe {
        SetSecurityInfo(
            handle,
            SE_WINDOW_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            new_dacl,
            null_mut(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

/// Adds a discovered path only while the AppContainer tree remains within its entry limit.
///
/// `scheduled_paths` includes the root and paths already processed, so the queue never needs to
/// retain more than `MAX_ACL_ENTRIES` paths for one traversal.
pub(crate) fn schedule_acl_path(
    pending: &mut Vec<PathBuf>,
    scheduled_paths: &mut usize,
    path: PathBuf,
) -> bool {
    if *scheduled_paths >= MAX_ACL_ENTRIES {
        return false;
    }

    pending.push(path);
    *scheduled_paths += 1;
    true
}

/// Adds or removes the full workspace access mask for one AppContainer SID.
pub(crate) fn set_app_container_access(
    path: &Path,
    sid: PSID,
    allow: bool,
    is_directory: bool,
) -> io::Result<()> {
    let permissions = FILE_GENERIC_READ
        | FILE_GENERIC_WRITE
        | FILE_GENERIC_EXECUTE
        | FILE_TRAVERSE
        | FILE_ADD_FILE
        | FILE_ADD_SUBDIRECTORY
        | FILE_DELETE_CHILD
        | DELETE;
    set_app_container_permissions(path, sid, allow, permissions, is_directory)
}

/// Adds or removes the read-only runtime access mask for one AppContainer SID.
pub(crate) fn set_app_container_read_access(
    path: &Path,
    sid: PSID,
    allow: bool,
    is_directory: bool,
) -> io::Result<()> {
    let permissions =
        FILE_GENERIC_READ | FILE_GENERIC_EXECUTE | FILE_TRAVERSE | FILE_READ_ATTRIBUTES;
    set_app_container_permissions(path, sid, allow, permissions, is_directory)
}

/// Applies a supplied access mask and enables inheritance only for granted directories.
pub(crate) fn set_app_container_permissions(
    path: &Path,
    sid: PSID,
    allow: bool,
    permissions: u32,
    is_directory: bool,
) -> io::Result<()> {
    let inheritance = if allow && is_directory {
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    } else {
        0
    };
    update_app_container_acl(path, sid, allow, permissions, inheritance)
}

/// Adds or removes traversal and attribute-read access without inheriting it to children.
pub(crate) fn set_app_container_traverse_access(
    path: &Path,
    sid: PSID,
    allow: bool,
) -> io::Result<()> {
    update_app_container_acl(path, sid, allow, FILE_TRAVERSE | FILE_READ_ATTRIBUTES, 0)
}

/// Performs a serialized read-modify-write update of one canonical target's discretionary ACL.
///
/// # Errors
/// Returns an error when the target changes, is a reparse point, already has multiple hard links
/// during a grant, the ACL is absent, the cross-process lock cannot be acquired, or any Windows
/// security API rejects the update.
pub(crate) fn update_app_container_acl(
    path: &Path,
    sid: PSID,
    allow: bool,
    permissions: u32,
    inheritance: u32,
) -> io::Result<()> {
    // Keep the ACL read-modify-write sequence atomic across threads and OpenChat processes.
    let _guard = APP_CONTAINER_ACL_LOCK
        .lock()
        .map_err(|_| io::Error::other("The AppContainer ACL lock is unavailable."))?;
    let _system_guard = AppContainerAclLock::acquire()?;
    let use_maximum_allowed_for_target = allow && inheritance != 0;
    let (path_handles, information) = lock_acl_path(path, use_maximum_allowed_for_target)?;
    let is_directory = information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    let file = path_handles
        .last()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "The ACL path is empty."))?;
    if allow && !is_directory && information.nNumberOfLinks > 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "A sandbox path contains a hard-linked file.",
        ));
    }
    // ACL updates use the inspected object handle. Inheritable directory grants use
    // MAXIMUM_ALLOWED with SetSecurityInfo to suppress documented propagation. Non-inheriting
    // directory updates use NtSetSecurityObject; Windows regressions verify unchanged child DACLs.
    update_acl_handle(
        file,
        sid,
        allow,
        permissions,
        inheritance,
        is_directory,
        use_maximum_allowed_for_target,
    )
}

/// Updates an inspected securable object through its existing handle.
pub(super) fn update_acl_handle(
    file: &fs::File,
    sid: PSID,
    allow: bool,
    permissions: u32,
    inheritance: u32,
    is_directory: bool,
    use_maximum_allowed_for_target: bool,
) -> io::Result<()> {
    let mut current_dacl = null_mut();
    let mut source_descriptor = null_mut();
    // SAFETY: `file` owns the live target handle and every requested output pointer is writable.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle() as HANDLE,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut current_dacl,
            null_mut(),
            &mut source_descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    if current_dacl.is_null() {
        // SAFETY: The descriptor pointer was returned by GetSecurityInfo; LocalFree accepts null.
        unsafe { LocalFree(source_descriptor) };
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "A null or absent DACL cannot be safely scoped to an AppContainer.",
        ));
    }
    let mut original_control = 0;
    let mut descriptor_revision = 0;
    // SAFETY: `source_descriptor` is the descriptor returned by the successful GetSecurityInfo call.
    if unsafe {
        GetSecurityDescriptorControl(
            source_descriptor,
            &mut original_control,
            &mut descriptor_revision,
        )
    } == 0
    {
        let error = last_os_error();
        // SAFETY: The descriptor was allocated by GetSecurityInfo and has not yet been freed.
        unsafe { LocalFree(source_descriptor) };
        return Err(error);
    }
    let access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: permissions,
        grfAccessMode: if allow { SET_ACCESS } else { REVOKE_ACCESS },
        grfInheritance: inheritance,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.cast(),
        },
    };
    let mut updated_dacl = null_mut();
    // SAFETY: `current_dacl` came from the successful query, `access` is initialized, and the output pointer is writable.
    let acl_status = unsafe { SetEntriesInAclW(1, &access, current_dacl, &mut updated_dacl) };
    if acl_status != 0 {
        // SAFETY: The descriptor was allocated by GetSecurityInfo and has not yet been freed.
        unsafe { LocalFree(source_descriptor) };
        return Err(io::Error::from_raw_os_error(acl_status as i32));
    }
    // SAFETY: The descriptor was allocated by GetSecurityInfo and has not yet been freed.
    unsafe { LocalFree(source_descriptor) };
    let updated_dacl = require_non_null_acl(updated_dacl)?;
    let set_result = if is_directory && !use_maximum_allowed_for_target {
        let mut descriptor_storage = std::mem::MaybeUninit::<SECURITY_DESCRIPTOR>::uninit();
        let descriptor = descriptor_storage.as_mut_ptr();
        // SAFETY: `descriptor` points to writable storage large enough for a SECURITY_DESCRIPTOR.
        let initialized = unsafe { InitializeSecurityDescriptor(descriptor.cast(), 1) };
        let dacl_attached = if initialized != 0 {
            // SAFETY: Initialization succeeded and `updated_dacl` is a valid API-produced ACL.
            (unsafe {
                SetSecurityDescriptorDacl(
                    descriptor.cast(),
                    1,
                    updated_dacl.as_ptr(),
                    i32::from(original_control & SE_DACL_DEFAULTED != 0),
                )
            }) != 0
        } else {
            false
        };
        let inheritance_control_mask =
            SE_DACL_AUTO_INHERIT_REQ | SE_DACL_AUTO_INHERITED | SE_DACL_PROTECTED;
        // This operation is intentionally non-inheriting: retaining the request bit could ask a
        // filesystem provider to propagate the changed DACL to existing children.
        let inheritance_control_value = noninheriting_directory_control_value(original_control);
        let inheritance_control_restored = if initialized != 0 && dacl_attached {
            // SAFETY: `descriptor` is initialized and the mask contains only control bits supported
            // by SetSecurityDescriptorControl.
            (unsafe {
                SetSecurityDescriptorControl(
                    descriptor.cast(),
                    inheritance_control_mask,
                    inheritance_control_value,
                )
            }) != 0
        } else {
            false
        };
        if initialized == 0 || !dacl_attached || !inheritance_control_restored {
            Err(last_os_error())
        } else {
            // SAFETY: `file` is the inspected target handle and `descriptor` remains valid for the
            // call. NtSetSecurityObject requires WRITE_DAC, which the path was opened with.
            let status = unsafe {
                NtSetSecurityObject(
                    file.as_raw_handle() as HANDLE,
                    DACL_SECURITY_INFORMATION,
                    descriptor.cast(),
                )
            };
            if status == 0 {
                Ok(())
            } else {
                // SAFETY: The native ACL update returned this NTSTATUS value.
                let error = unsafe { RtlNtStatusToDosError(status) };
                Err(io::Error::from_raw_os_error(error as i32))
            }
        }
    } else {
        // SAFETY: `file` owns the live target handle and `updated_dacl` is a valid API-produced ACL.
        // MAXIMUM_ALLOWED suppresses propagation to existing children for inheritable grants.
        let status = unsafe {
            SetSecurityInfo(
                file.as_raw_handle() as HANDLE,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                updated_dacl.as_ptr(),
                null_mut(),
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status as i32))
        }
    };
    // SAFETY: `updated_dacl` was allocated by SetEntriesInAclW and has no remaining users.
    unsafe { LocalFree(updated_dacl.as_ptr().cast()) };
    set_result
}

pub(super) fn noninheriting_directory_control_value(original_control: u16) -> u16 {
    original_control & (SE_DACL_AUTO_INHERITED | SE_DACL_PROTECTED)
}

/// Converts a successful ACL API result into a non-null pointer before applying it.
///
/// A null ACL would grant unrestricted access when applied, so the sandbox rejects it instead of
/// treating it as an empty ACL.
pub(crate) fn require_non_null_acl(acl: *mut ACL) -> io::Result<NonNull<ACL>> {
    NonNull::new(acl).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows returned a null ACL after a successful ACL update.",
        )
    })
}

/// Holds the stable per-profile file lock while an ACL update is in progress.
#[must_use = "the file lock must remain held until the ACL update completes"]
pub(crate) struct AppContainerAclLock {
    _lock_file: fs::File,
}

impl AppContainerAclLock {
    /// Acquires the per-profile file lock within the configured timeout.
    ///
    /// # Errors
    /// Returns an error if the profile path is unavailable, the lock file cannot be opened, or
    /// another process keeps the lock for the full timeout.
    pub(crate) fn acquire() -> io::Result<Self> {
        let local_app_data = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "The current Windows user's local application-data path is unavailable.",
                )
            })?;
        if !local_app_data.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The current Windows user's local application-data path is not absolute.",
            ));
        }

        Self::acquire_for_path(&app_container_acl_lock_path(&local_app_data))
    }

    /// Acquires a file lock at the supplied path; tests pass a temporary workspace path.
    pub(crate) fn acquire_for_path(lock_path: &Path) -> io::Result<Self> {
        use std::fs::TryLockError;

        let parent = lock_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The AppContainer ACL lock path has no parent directory.",
                )
            })?;
        fs::create_dir_all(parent)?;
        let lock_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        let deadline = Instant::now() + ACL_LOCK_TIMEOUT;

        loop {
            match lock_file.try_lock() {
                Ok(()) => {
                    return Ok(Self {
                        _lock_file: lock_file,
                    });
                }
                Err(TryLockError::WouldBlock) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Timed out while acquiring the AppContainer ACL lock.",
                        ));
                    }
                    std::thread::sleep(remaining.min(ACL_LOCK_RETRY_INTERVAL));
                }
                Err(TryLockError::Error(error)) => return Err(error),
            }
        }
    }
}

/// Opens each component of one canonical ACL path without following reparse points.
///
/// Each opened handle is checked against its canonical component to reject redirection while the
/// path is prepared. Retaining handles preserves the inspected objects, but does not prevent a
/// process with delete-child access to a parent directory from renaming a directory entry.
pub(crate) fn lock_acl_path(
    path: &Path,
    use_maximum_allowed_for_target: bool,
) -> io::Result<(Vec<fs::File>, BY_HANDLE_FILE_INFORMATION)> {
    let mut components: Vec<PathBuf> = path
        .ancestors()
        .filter(|component| !component.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect();
    components.reverse();
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The ACL path is empty.",
        ));
    }

    let mut handles = Vec::with_capacity(components.len());
    let mut target_information = BY_HANDLE_FILE_INFORMATION::default();
    for (index, component) in components.iter().enumerate() {
        let is_target = index + 1 == components.len();
        let access_mode = if is_target && use_maximum_allowed_for_target {
            MAXIMUM_ALLOWED
        } else {
            FILE_READ_ATTRIBUTES
                | if is_target {
                    READ_CONTROL | WRITE_DAC
                } else {
                    0
                }
        };
        let mut options = fs::OpenOptions::new();
        options
            .access_mode(access_mode)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
        let file = options.open(component)?;
        let information = verify_acl_handle_path(&file, component)?;
        if !is_target && information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox path ancestor is not a directory.",
            ));
        }

        target_information = information;
        handles.push(file);
    }

    Ok((handles, target_information))
}

fn verify_acl_handle_path(file: &fs::File, path: &Path) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` owns a live target handle and `information` is a writable output structure.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut information) } == 0
    {
        return Err(last_os_error());
    }
    if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "A sandbox path cannot contain a reparse point.",
        ));
    }
    let mut final_path = vec![0u16; 32_768];
    // SAFETY: `file` is live and `final_path` is writable for its declared capacity.
    let final_path_length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle() as HANDLE,
            final_path.as_mut_ptr(),
            final_path.len() as u32,
            FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
        )
    };
    if final_path_length == 0 {
        return Err(last_os_error());
    }
    if final_path_length as usize >= final_path.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "A sandbox path exceeds the Windows path limit.",
        ));
    }
    final_path.truncate(final_path_length as usize);
    if !path
        .as_os_str()
        .encode_wide()
        .eq(final_path.iter().copied())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "A sandbox path changed while its ACL was being prepared.",
        ));
    }
    Ok(information)
}
