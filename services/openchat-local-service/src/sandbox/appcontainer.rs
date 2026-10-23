//! Windows AppContainer profile lifecycle and security capability management.

use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::MetadataExt,
    },
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::ConvertSidToStringSidW,
        DeriveCapabilitySidsFromName, FreeSid,
        Isolation::{
            CreateAppContainerProfile, DeleteAppContainerProfile, GetAppContainerFolderPath,
        },
        PSID,
    },
    Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT,
    System::Com::CoTaskMemFree,
};
use zeroize::Zeroizing;

use super::{
    security_acl::{
        MAX_ACL_ENTRIES, grant_user_objects_access, revoke_user_objects_access, schedule_acl_path,
        set_app_container_access, set_app_container_read_access, set_app_container_traverse_access,
    },
    types::{
        is_valid_additional_environment_name, is_valid_base_environment_name, last_os_error,
        shell_path, wide_null,
    },
};

/// Owns a temporary AppContainer profile and the ACL grants made for its process.
pub(crate) struct AppContainerSandbox {
    /// NUL-terminated unique profile name used to remove the profile on drop.
    pub(crate) name: Vec<u16>,
    /// Windows-allocated AppContainer SID retained until profile cleanup.
    pub(crate) sid: usize,
    /// Windows-allocated textual SID used to resolve the profile directory.
    pub(crate) sid_string: usize,
    /// Canonical directory used as the child process working directory.
    pub(crate) working_directory: PathBuf,
    /// Isolated temporary directory exposed to the child through `TEMP` and `TMP`.
    pub(crate) temp_directory: PathBuf,
    /// Paths whose AppContainer ACEs must be revoked when this sandbox is dropped.
    pub(crate) granted_paths: Vec<PathBuf>,
}

impl AppContainerSandbox {
    /// Creates a temporary profile and grants only workspace and selected runtime access.
    ///
    /// # Errors
    /// Returns an error if Windows profile setup, path validation, ACL updates, or directory
    /// traversal fails. Any ACLs already granted are revoked during cleanup.
    pub(crate) fn create(
        working_directory: &Path,
        additional_workspace_paths: &[PathBuf],
        additional_read_only_paths: &[PathBuf],
    ) -> io::Result<Self> {
        let acl_working_directory = fs::canonicalize(working_directory)?;
        let working_directory = shell_path(&acl_working_directory);
        if !working_directory.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The sandbox working directory is unavailable.",
            ));
        }
        let name = wide_null(&format!("OpenChatProcess{}", uuid::Uuid::new_v4().simple()));
        let display_name = wide_null("OpenChat restricted process");
        let description = wide_null("Temporary AppContainer for a restricted OpenChat process.");
        let mut sid = null_mut();
        // SAFETY: The input strings are NUL-terminated and `sid` is a writable output pointer.
        let status = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                display_name.as_ptr(),
                description.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        if status < 0 || sid.is_null() {
            // SAFETY: Any non-null SID was returned by the failed profile call; the profile name remains live and terminated.
            unsafe {
                if !sid.is_null() {
                    FreeSid(sid);
                }
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(io::Error::other(
                "Windows could not create the AppContainer sandbox.",
            ));
        }

        let mut sid_string = null_mut();
        // SAFETY: `sid` was returned by Windows and `sid_string` is a writable output pointer.
        if unsafe { ConvertSidToStringSidW(sid, &mut sid_string) } == 0 || sid_string.is_null() {
            let error = last_os_error();
            // SAFETY: Any non-null string was allocated by the conversion call; `sid` and profile name are still valid.
            unsafe {
                if !sid_string.is_null() {
                    LocalFree(sid_string.cast());
                }
                FreeSid(sid);
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(error);
        }
        let sid_string = sid_string as usize;
        let mut app_container_data = null_mut();
        // SAFETY: `sid_string` points to the converted NUL-terminated SID and output is writable.
        let folder_result = unsafe {
            GetAppContainerFolderPath(
                sid_string as windows_sys::core::PCWSTR,
                &mut app_container_data,
            )
        };
        if folder_result < 0 || app_container_data.is_null() {
            // SAFETY: These are the still-owned SID/string/profile resources allocated above.
            unsafe {
                FreeSid(sid);
                LocalFree(sid_string as _);
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(io::Error::other(
                "Windows could not locate the AppContainer profile folder.",
            ));
        }
        // SAFETY: Windows returned a readable NUL-terminated AppContainer folder path.
        let package_root = PathBuf::from(unsafe { wide_os_string(app_container_data) });
        // SAFETY: `app_container_data` is the CoTaskMem allocation returned by the folder API.
        unsafe { CoTaskMemFree(app_container_data.cast()) };
        let temp_directory = package_root.join("OpenChatProcessTemp");
        if let Err(error) = fs::create_dir_all(&temp_directory) {
            // SAFETY: These are the still-owned SID/string/profile resources allocated above.
            unsafe {
                FreeSid(sid);
                LocalFree(sid_string as _);
                DeleteAppContainerProfile(name.as_ptr());
            }
            return Err(error);
        }

        let mut sandbox = Self {
            name,
            sid: sid as usize,
            sid_string,
            working_directory: working_directory.clone(),
            temp_directory,
            granted_paths: Vec::new(),
        };
        if let Err(error) = grant_user_objects_access(sid) {
            sandbox.cleanup();
            return Err(error);
        }
        if let Err(error) = sandbox.grant_working_directory(acl_working_directory) {
            sandbox.cleanup();
            return Err(error);
        }
        for path in additional_workspace_paths {
            let canonical_path = match fs::canonicalize(path) {
                Ok(path) => path,
                Err(error) => {
                    sandbox.cleanup();
                    return Err(error);
                }
            };
            if shell_path(&canonical_path) == sandbox.working_directory {
                continue;
            }
            if let Err(error) = sandbox.grant_working_directory(canonical_path) {
                sandbox.cleanup();
                return Err(error);
            }
        }
        for path in additional_read_only_paths {
            if let Err(error) = sandbox.grant_read_only_path(path) {
                sandbox.cleanup();
                return Err(error);
            }
        }
        Ok(sandbox)
    }

    /// Builds a sorted, NUL-terminated environment containing only the allowed base variables.
    ///
    /// # Errors
    /// Returns an error for invalid variable names, embedded NUL characters, temporary-directory
    /// failures, or an environment block larger than the Windows process limit.
    pub(crate) fn environment_block(
        &self,
        working_directory: &Path,
        additional_environment: &[(String, Zeroizing<String>)],
        base_environment: Option<&[(String, Zeroizing<String>)]>,
    ) -> io::Result<Vec<u16>> {
        let mut variables = BTreeMap::new();
        if let Some(base_environment) = base_environment {
            for (key, value) in base_environment {
                if !is_valid_base_environment_name(key) || value.contains('\0') {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "The sandbox base environment is invalid.",
                    ));
                }
                variables.insert(
                    key.to_ascii_uppercase(),
                    Zeroizing::new(value.encode_utf16().collect::<Vec<_>>()),
                );
            }
        } else {
            for (key, value) in std::env::vars_os() {
                let Some(key_text) = key.to_str() else {
                    continue;
                };
                if matches!(
                    key_text.to_ascii_uppercase().as_str(),
                    "PATH"
                        | "SYSTEMROOT"
                        | "WINDIR"
                        | "PATHEXT"
                        | "COMSPEC"
                        | "USERPROFILE"
                        | "APPDATA"
                        | "LOCALAPPDATA"
                ) {
                    variables.insert(
                        key_text.to_ascii_uppercase(),
                        Zeroizing::new(value.encode_wide().collect::<Vec<_>>()),
                    );
                }
            }
        }
        let working_directory = shell_path(working_directory);
        let working_directory_text = working_directory.to_string_lossy();
        let mut drive_characters = working_directory_text.chars();
        if let (Some(drive), Some(':')) = (drive_characters.next(), drive_characters.next())
            && drive.is_ascii_alphabetic()
        {
            let drive = drive.to_ascii_uppercase();
            variables.insert(
                format!("={drive}:"),
                Zeroizing::new(
                    working_directory
                        .as_os_str()
                        .encode_wide()
                        .collect::<Vec<_>>(),
                ),
            );
        }
        fs::create_dir_all(&self.temp_directory)?;
        let isolated_profile = self.temp_directory.parent().unwrap_or(&self.temp_directory);
        let isolated_appdata = isolated_profile.join("AppData").join("Roaming");
        let isolated_localappdata = isolated_profile.join("AppData").join("Local");
        let _ = fs::create_dir_all(&isolated_appdata);
        let _ = fs::create_dir_all(&isolated_localappdata);

        variables.insert(
            "USERPROFILE".to_owned(),
            Zeroizing::new(
                isolated_profile
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
            ),
        );
        variables.insert(
            "APPDATA".to_owned(),
            Zeroizing::new(
                isolated_appdata
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
            ),
        );
        variables.insert(
            "LOCALAPPDATA".to_owned(),
            Zeroizing::new(
                isolated_localappdata
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
            ),
        );

        variables.insert(
            "TEMP".to_owned(),
            Zeroizing::new(
                self.temp_directory
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
            ),
        );
        variables.insert(
            "TMP".to_owned(),
            Zeroizing::new(
                self.temp_directory
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
            ),
        );
        for (key, value) in additional_environment {
            let normalized_key = key.to_ascii_uppercase();
            if !is_valid_additional_environment_name(key)
                || value.contains('\0')
                || matches!(
                    normalized_key.as_str(),
                    "PATH"
                        | "SYSTEMROOT"
                        | "WINDIR"
                        | "PATHEXT"
                        | "COMSPEC"
                        | "USERPROFILE"
                        | "APPDATA"
                        | "LOCALAPPDATA"
                        | "TEMP"
                        | "TMP"
                )
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The sandbox environment variable is invalid.",
                ));
            }
            variables.insert(
                normalized_key,
                Zeroizing::new(value.encode_utf16().collect::<Vec<_>>()),
            );
        }

        let mut block = Vec::new();
        for (key, value) in variables {
            block.extend(std::ffi::OsStr::new(&key).encode_wide());
            block.push(b'=' as u16);
            block.extend(value.iter().copied());
            block.push(0);
        }
        block.push(0);
        if block.len() > 32_767 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The sandbox environment exceeds the Windows process limit.",
            ));
        }
        Ok(block)
    }

    /// Grants read and execute access to a runtime file tree without following reparse points.
    ///
    /// Hard-linked files are rejected because they could alias data outside the granted tree.
    ///
    /// # Errors
    /// Returns an error when the root is invalid, contains an unsafe file, exceeds the traversal
    /// bound, or Windows cannot inspect or update one of its paths.
    fn grant_read_only_path(&mut self, root: &Path) -> io::Result<()> {
        let root = fs::canonicalize(root)?;
        let root_metadata = fs::symlink_metadata(&root)?;
        if root_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox runtime path cannot be a reparse point.",
            ));
        }
        if !root_metadata.is_dir() && !root_metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A sandbox runtime path must be a file or directory.",
            ));
        }

        let access_root = if root_metadata.is_dir() {
            root.clone()
        } else {
            root.parent()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "A sandbox runtime file must have a parent directory.",
                    )
                })?
                .to_path_buf()
        };
        let mut ancestor = access_root.parent();
        while let Some(path) = ancestor {
            if path.parent().is_none() {
                break;
            }
            set_app_container_traverse_access(path, self.sid as PSID, true)?;
            self.granted_paths.push(path.to_path_buf());
            ancestor = path.parent();
        }

        let mut pending = vec![root];
        let mut scheduled_paths = 1;
        let mut visited = HashSet::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if visited.len() > MAX_ACL_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The runtime path contains too many entries for a bounded AppContainer.",
                ));
            }
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                continue;
            }
            let is_directory = metadata.is_dir();
            set_app_container_read_access(&path, self.sid as PSID, true, is_directory)?;
            self.granted_paths.push(path.clone());
            if is_directory {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let child_path = entry.path();
                    let child_metadata = fs::symlink_metadata(&child_path)?;
                    if child_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
                        && !schedule_acl_path(&mut pending, &mut scheduled_paths, child_path)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "The runtime path contains too many entries for a bounded AppContainer.",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Grants read, write, and execute access to the selected workspace tree.
    ///
    /// Reparse points are not traversed, and hard-linked files are rejected to keep access within
    /// the workspace boundary.
    ///
    /// # Errors
    /// Returns an error when traversal exceeds its bound, a hard-linked file is found, or a file
    /// system or Windows ACL operation fails.
    fn grant_working_directory(&mut self, root: PathBuf) -> io::Result<()> {
        let mut ancestor = root.parent();
        while let Some(path) = ancestor {
            if path.parent().is_none() {
                break;
            }
            set_app_container_traverse_access(path, self.sid as PSID, true)?;
            self.granted_paths.push(path.to_path_buf());
            ancestor = path.parent();
        }

        let mut pending = vec![root.clone()];
        let mut scheduled_paths = 1;
        let mut visited = HashSet::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if visited.len() > MAX_ACL_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "The working directory contains too many entries for a bounded AppContainer sandbox.",
                ));
            }
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                continue;
            }
            let is_directory = metadata.is_dir();
            set_app_container_access(&path, self.sid as PSID, true, is_directory)?;
            self.granted_paths.push(path.clone());
            if is_directory {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let child_path = entry.path();
                    let child_metadata = fs::symlink_metadata(&child_path)?;
                    if child_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        continue;
                    }
                    if !schedule_acl_path(&mut pending, &mut scheduled_paths, child_path) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "The working directory contains too many entries for a bounded AppContainer sandbox.",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Revokes this profile's ACE from every path recorded during sandbox setup.
    fn cleanup(&mut self) {
        revoke_user_objects_access(self.sid as PSID);
        for path in self.granted_paths.drain(..).rev() {
            if set_app_container_access(&path, self.sid as PSID, false, false).is_err() {
                eprintln!("app_container_acl_cleanup_failed");
            }
        }
    }
}

impl Drop for AppContainerSandbox {
    /// Revokes granted ACLs, then releases the profile SID and deletes the profile.
    fn drop(&mut self) {
        self.cleanup();
        // SAFETY: These pointers were allocated by the paired Windows APIs and are still owned by this sandbox.
        unsafe {
            FreeSid(self.sid as PSID);
            LocalFree(self.sid_string as _);
            if DeleteAppContainerProfile(self.name.as_ptr()) < 0 {
                eprintln!("app_container_profile_cleanup_failed");
            }
        }
    }
}

/// Owns SID arrays allocated while resolving the optional `internetClient` capability.
pub(crate) struct DerivedCapabilitySids {
    group_sids: *mut PSID,
    group_sid_count: u32,
    capability_sids: *mut PSID,
    capability_sid_count: u32,
}

impl DerivedCapabilitySids {
    /// Resolves the single SID required to grant the AppContainer internet capability.
    ///
    /// # Errors
    /// Returns an error if Windows cannot derive the SIDs or returns an unexpected capability set.
    pub(crate) fn internet_client() -> io::Result<Self> {
        let name = wide_null("internetClient");
        let mut sids = Self {
            group_sids: null_mut(),
            group_sid_count: 0,
            capability_sids: null_mut(),
            capability_sid_count: 0,
        };
        // SAFETY: `name` is NUL-terminated and the SID arrays/counts are writable output locations.
        let derived = unsafe {
            DeriveCapabilitySidsFromName(
                name.as_ptr(),
                &mut sids.group_sids,
                &mut sids.group_sid_count,
                &mut sids.capability_sids,
                &mut sids.capability_sid_count,
            )
        };
        if derived == 0 {
            return Err(last_os_error());
        }
        if sids.capability_sid_count != 1 || sids.capability_sids.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows returned an invalid AppContainer capability SID set.",
            ));
        }
        Ok(sids)
    }

    /// Returns the validated sole capability SID for `SECURITY_CAPABILITIES`.
    pub(crate) fn capability_sid(&self) -> PSID {
        // SAFETY: `internet_client` validates exactly one non-null capability SID before this accessor is used.
        unsafe { *self.capability_sids }
    }
}

impl Drop for DerivedCapabilitySids {
    /// Releases the API-allocated SID entries and their containing arrays.
    fn drop(&mut self) {
        // SAFETY: Both arrays and counts were returned by DeriveCapabilitySidsFromName and are freed once here.
        unsafe {
            free_sid_array(self.group_sids, self.group_sid_count);
            free_sid_array(self.capability_sids, self.capability_sid_count);
        }
    }
}

/// Releases the SID pointers returned by `DeriveCapabilitySidsFromName`.
///
/// # Safety
/// `sids` and `count` must be the matching pointer and count returned by that API, and the array must not have been freed.
unsafe fn free_sid_array(sids: *mut PSID, count: u32) {
    if sids.is_null() {
        return;
    }
    for index in 0..count as usize {
        // SAFETY: The caller guarantees this API-owned array has `count` initialized SID pointers.
        unsafe {
            LocalFree(*sids.add(index) as _);
        }
    }
    // SAFETY: The array itself was allocated by the Windows API and has not been freed yet.
    unsafe {
        LocalFree(sids as _);
    }
}

/// Converts a Windows-owned NUL-terminated UTF-16 path to an `OsString`.
///
/// # Safety
/// `value` must be non-null and point to a readable NUL-terminated UTF-16 string.
pub(crate) unsafe fn wide_os_string(value: windows_sys::core::PWSTR) -> std::ffi::OsString {
    let mut length = 0usize;
    // SAFETY: The caller guarantees that `value` points to a readable NUL-terminated string.
    while unsafe { *value.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: The scan found the terminator, so `length` code units are readable before it.
    std::ffi::OsString::from_wide(unsafe { std::slice::from_raw_parts(value, length) })
}
