//! Reads and updates project instruction files under the registered project.
//!
//! File access is constrained to the project root, bounded by a size limit, and
//! writes use same-directory replacement to avoid exposing partial contents.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use cap_std::{ambient_authority, fs::Dir, fs::OpenOptions};

use crate::protocol::ServiceError;

pub(crate) const MAX_PROJECT_INSTRUCTIONS_BYTES: usize = 16 * 1024;
const INSTRUCTIONS_FILE: &str = ".openchat/instructions.md";

/// Loads validated instructions from the project's `.openchat/instructions.md`.
///
/// Empty or missing instructions return `Ok(None)`; invalid encoding, oversized
/// content, or paths outside the canonical project root return an error.
pub(crate) fn load(project_root: &Path) -> Result<Option<String>, ServiceError> {
    let canonical_root = fs::canonicalize(project_root).map_err(|_| unavailable_error())?;
    let configured_path = project_root.join(INSTRUCTIONS_FILE);
    let canonical_path = match fs::canonicalize(&configured_path) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(unavailable_error()),
    };
    ensure_project_file(&canonical_root, &canonical_path)?;
    let relative_path = canonical_path
        .strip_prefix(&canonical_root)
        .map_err(|_| unavailable_error())?;

    let bytes = match crate::filesystem::read_bounded_file_under_root(
        &canonical_root,
        relative_path,
        MAX_PROJECT_INSTRUCTIONS_BYTES,
    ) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(unavailable_error()),
    };
    if bytes.len() > MAX_PROJECT_INSTRUCTIONS_BYTES {
        return Err(oversized_error());
    }

    let instructions = String::from_utf8(bytes).map_err(|_| {
        ServiceError::new(
            "project_instructions_invalid_encoding",
            "The project's instructions file must use UTF-8 encoding.",
            false,
        )
    })?;
    if instructions.trim().is_empty() {
        Ok(None)
    } else {
        validate(&instructions)?;
        Ok(Some(instructions))
    }
}

/// Validates and atomically replaces the project's instruction file.
pub(crate) fn save(project_root: &Path, instructions: &str) -> Result<(), ServiceError> {
    validate(instructions)?;
    let root = fs::canonicalize(project_root).map_err(|_| unavailable_error())?;
    let root_directory =
        Dir::open_ambient_dir(&root, ambient_authority()).map_err(|_| unavailable_error())?;
    match root_directory.symlink_metadata(".openchat") {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(unavailable_error()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            root_directory
                .create_dir(".openchat")
                .map_err(|_| unavailable_error())?;
        }
        Err(_) => return Err(unavailable_error()),
    }
    let directory = root_directory
        .open_dir(".openchat")
        .map_err(|_| unavailable_error())?;
    let temporary = PathBuf::from(format!(".instructions-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = directory
        .open_with(&temporary, &options)
        .map_err(|_| unavailable_error())?;
    if file
        .write_all(instructions.as_bytes())
        .and_then(|()| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = directory.remove_file(&temporary);
        return Err(unavailable_error());
    }
    drop(file);
    if directory
        .rename(&temporary, &directory, Path::new("instructions.md"))
        .is_err()
    {
        let _ = directory.remove_file(&temporary);
        return Err(unavailable_error());
    }
    Ok(())
}

/// Enforces the maximum UTF-8 byte length accepted for project instructions.
pub(crate) fn validate(instructions: &str) -> Result<(), ServiceError> {
    if instructions.len() > MAX_PROJECT_INSTRUCTIONS_BYTES {
        return Err(oversized_error());
    }
    Ok(())
}

fn oversized_error() -> ServiceError {
    ServiceError::new(
        "project_instructions_too_large",
        "The project's instructions file exceeds the 16 KiB limit.",
        false,
    )
}

fn ensure_project_file(project_root: &Path, instructions_path: &Path) -> Result<(), ServiceError> {
    if instructions_path.starts_with(project_root) {
        Ok(())
    } else {
        Err(ServiceError::new(
            "project_instructions_outside_project",
            "The project's instructions file resolves outside the attached project.",
            false,
        ))
    }
}

fn unavailable_error() -> ServiceError {
    ServiceError::new(
        "project_instructions_unavailable",
        "The project's instructions file could not be read.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-project-instructions-test-{}",
                Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create temporary project directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove temporary project directory");
        }
    }

    fn write_instructions(root: &Path, bytes: &[u8]) -> PathBuf {
        let directory = root.join(".openchat");
        fs::create_dir_all(&directory).expect("create project config directory");
        let path = directory.join("instructions.md");
        fs::write(&path, bytes).expect("write project instructions");
        path
    }

    #[test]
    fn missing_and_blank_project_instructions_are_unconfigured() {
        let directory = TestDirectory::new();
        assert_eq!(load(&directory.0).expect("missing file is valid"), None);

        write_instructions(&directory.0, b" \n\t");
        assert_eq!(load(&directory.0).expect("blank file is valid"), None);
    }

    #[test]
    fn reads_utf8_project_instructions_within_the_byte_limit() {
        let directory = TestDirectory::new();
        write_instructions(&directory.0, "Use Turkish casing: İ and ı.".as_bytes());

        assert_eq!(
            load(&directory.0)
                .expect("read project instructions")
                .as_deref(),
            Some("Use Turkish casing: İ and ı.")
        );
    }

    #[test]
    fn rejects_hard_linked_project_instructions() {
        let project = TestDirectory::new();
        let outside = TestDirectory::new();
        let outside_file = outside.0.join("instructions.md");
        fs::write(&outside_file, "private external instructions")
            .expect("write external instructions");
        let project_file = project.0.join(INSTRUCTIONS_FILE);
        fs::create_dir_all(project_file.parent().expect("instruction parent"))
            .expect("create project settings directory");
        fs::hard_link(&outside_file, &project_file).expect("link external instructions");

        assert_eq!(
            load(&project.0)
                .expect_err("hard-linked instructions must be rejected")
                .code,
            "project_instructions_unavailable"
        );
    }

    #[test]
    fn saves_instructions_atomically_and_reloads_them() {
        let directory = TestDirectory::new();
        save(&directory.0, "Follow the repository conventions.")
            .expect("save project instructions");
        assert_eq!(
            load(&directory.0)
                .expect("reload project instructions")
                .as_deref(),
            Some("Follow the repository conventions.")
        );

        save(&directory.0, "Use the project's formatter.").expect("replace project instructions");
        assert_eq!(
            load(&directory.0)
                .expect("reload replaced instructions")
                .as_deref(),
            Some("Use the project's formatter.")
        );
    }

    #[test]
    fn refuses_oversized_edits_without_changing_the_saved_file() {
        let directory = TestDirectory::new();
        save(&directory.0, "Keep this guidance.").expect("save initial instructions");
        assert_eq!(
            save(
                &directory.0,
                &"x".repeat(MAX_PROJECT_INSTRUCTIONS_BYTES + 1)
            )
            .expect_err("oversized instructions must fail")
            .code,
            "project_instructions_too_large"
        );
        assert_eq!(
            load(&directory.0)
                .expect("read preserved instructions")
                .as_deref(),
            Some("Keep this guidance.")
        );
    }

    #[test]
    fn rejects_oversized_and_invalid_utf8_project_instructions() {
        let directory = TestDirectory::new();
        write_instructions(
            &directory.0,
            &vec![b'x'; MAX_PROJECT_INSTRUCTIONS_BYTES + 1],
        );
        assert_eq!(
            load(&directory.0)
                .expect_err("oversized file must fail")
                .code,
            "project_instructions_too_large"
        );

        write_instructions(&directory.0, &[0xff]);
        assert_eq!(
            load(&directory.0)
                .expect_err("invalid UTF-8 must fail")
                .code,
            "project_instructions_invalid_encoding"
        );
    }

    #[test]
    fn rejects_paths_outside_the_canonical_project_root() {
        let project = TestDirectory::new();
        let outside = TestDirectory::new();
        assert_eq!(
            ensure_project_file(&project.0, &outside.0)
                .expect_err("outside path must fail")
                .code,
            "project_instructions_outside_project"
        );
    }

    #[cfg(windows)]
    #[test]
    fn save_rejects_an_external_settings_directory_symlink() {
        use std::os::windows::fs::symlink_dir;

        let project = TestDirectory::new();
        let outside = TestDirectory::new();
        symlink_dir(&outside.0, project.0.join(".openchat"))
            .expect("link project settings to an external directory");

        assert_eq!(
            save(&project.0, "Do not write outside the project.")
                .expect_err("external settings directory must be rejected")
                .code,
            "project_instructions_unavailable"
        );
        assert!(
            !outside.0.join("instructions.md").exists(),
            "rejected saves must not create files through the external link"
        );
    }
}
