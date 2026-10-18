//! Filesystem operations shared by Tauri commands.

use std::{io, path::Path};

#[cfg(not(windows))]
use std::fs;

/// Publishes a fully written file without replacing an existing destination.
///
/// The staging file must be in the destination directory. The caller removes the staging path
/// after publication; off Windows, hard-link creation supplies the atomic no-replace operation.
pub(crate) fn publish_file_without_replacing(staging: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

        let staging_wide = staging
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let destination_wide = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // SAFETY: Both path buffers are NUL-terminated and live through the Win32 call.
        let moved = unsafe {
            MoveFileExW(
                staging_wide.as_ptr(),
                destination_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if moved == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(not(windows))]
    {
        fs::hard_link(staging, destination)
    }
}

/// Atomically replaces a destination with a fully written staging file.
///
/// The staging file must be in the destination directory so replacement stays on one volume.
#[cfg(windows)]
pub(crate) fn replace_file_atomically(staging: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let staging_wide = staging
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: Both path buffers are NUL-terminated and live through the Win32 call.
    let moved = unsafe {
        MoveFileExW(
            staging_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Replaces a destination by renaming a staging file on non-Windows platforms.
#[cfg(not(windows))]
pub(crate) fn replace_file_atomically(staging: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(staging, destination)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-file-ops-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated file operation directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn publishing_a_file_does_not_replace_an_existing_destination() {
        let directory = TestDirectory::new();
        let target = directory.0.join("target.json");
        let staging = directory.0.join("staging.json");
        fs::write(&target, b"existing contents").expect("write existing destination");
        fs::write(&staging, b"new contents").expect("write staged file");

        let error = super::publish_file_without_replacing(&staging, &target)
            .expect_err("an existing destination must cause a conflict");

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read(&target).expect("read destination"),
            b"existing contents"
        );
        assert_eq!(
            fs::read(&staging).expect("read staged file"),
            b"new contents"
        );
    }

    #[test]
    fn publishing_a_file_preserves_the_complete_contents() {
        let directory = TestDirectory::new();
        let staging = directory.0.join("staging.json");
        let destination = directory.0.join("destination.json");
        fs::write(&staging, b"complete contents").expect("write staged file");

        super::publish_file_without_replacing(&staging, &destination)
            .expect("publish into an unused destination");

        assert_eq!(
            fs::read(&destination).expect("read destination"),
            b"complete contents"
        );
    }

    #[cfg(windows)]
    #[test]
    fn replacing_a_symlink_destination_does_not_modify_its_target() {
        use std::os::windows::fs::symlink_file;

        let directory = TestDirectory::new();
        let target = directory.0.join("target.json");
        let destination = directory.0.join("destination.json");
        let staging = directory.0.join("staging.json");
        fs::write(&target, b"target contents").expect("write symlink target");
        fs::write(&staging, b"replacement contents").expect("write staging file");
        symlink_file(&target, &destination).expect("create destination symlink");

        super::replace_file_atomically(&staging, &destination)
            .expect("replace symlink directory entry");

        assert_eq!(
            fs::read(&target).expect("read unchanged symlink target"),
            b"target contents"
        );
        assert_eq!(
            fs::read(&destination).expect("read replacement destination"),
            b"replacement contents"
        );
        assert!(!fs::symlink_metadata(destination)
            .expect("read replacement metadata")
            .file_type()
            .is_symlink());
    }

    #[cfg(windows)]
    #[test]
    fn publishing_does_not_replace_a_symlink_destination_or_its_target() {
        use std::os::windows::fs::symlink_file;

        let directory = TestDirectory::new();
        let target = directory.0.join("target.json");
        let destination = directory.0.join("destination.json");
        let staging = directory.0.join("staging.json");
        fs::write(&target, b"target contents").expect("write symlink target");
        fs::write(&staging, b"staging contents").expect("write staging file");
        symlink_file(&target, &destination).expect("create destination symlink");

        let error = super::publish_file_without_replacing(&staging, &destination)
            .expect_err("the existing symlink entry must cause a conflict");

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read(&target).expect("read symlink target"),
            b"target contents"
        );
        assert_eq!(
            fs::read(&staging).expect("read staging file"),
            b"staging contents"
        );
        assert!(fs::symlink_metadata(&destination)
            .expect("read destination metadata")
            .file_type()
            .is_symlink());
    }
}
