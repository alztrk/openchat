//! Markdown export generation and safe file export dialog operations.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use crate::{
    file_ops::replace_file_atomically,
    request_bounds::BoundedText,
    save_dialog::{select_save_file, SaveFileDialogError},
};

use super::{
    ChatStorageError, MAXIMUM_MARKDOWN_EXPORT_CONTENT_BYTES,
    MAXIMUM_MARKDOWN_EXPORT_FILE_NAME_BYTES, MAXIMUM_MARKDOWN_EXPORT_TITLE_BYTES,
};

#[tauri::command]
/// Prompts the user for a markdown export destination and atomically writes the conversation markdown.
///
/// # Errors
/// Returns a validation, dialog, or storage error.
pub async fn save_conversation_markdown(
    window: tauri::WebviewWindow,
    file_name: BoundedText<MAXIMUM_MARKDOWN_EXPORT_FILE_NAME_BYTES>,
    title: BoundedText<MAXIMUM_MARKDOWN_EXPORT_TITLE_BYTES>,
    content: BoundedText<MAXIMUM_MARKDOWN_EXPORT_CONTENT_BYTES>,
) -> Result<bool, ChatStorageError> {
    let file_name = file_name.into_inner();
    let title = title.into_inner();
    let content = content.into_inner();
    if !is_safe_markdown_export_file_name(&file_name)
        || title.is_empty()
        || title.len() > 128
        || title.chars().any(char::is_control)
        || content.is_empty()
        || content.len() > 20 * 1024 * 1024
    {
        return Err(ChatStorageError::invalid_input());
    }

    let Some(path) = select_save_file(&window, file_name, title, "Markdown", "md")
        .await
        .map_err(|error| match error {
            SaveFileDialogError::Unavailable => ChatStorageError::unavailable(),
        })?
    else {
        return Ok(false);
    };
    if !path.is_absolute()
        || path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("md"))
    {
        return Err(ChatStorageError::invalid_input());
    }
    write_markdown_export(&path, content.as_bytes())?;
    Ok(true)
}

/// Publishes Markdown bytes only after a complete same-directory staging write.
pub(crate) fn write_markdown_export(path: &Path, content: &[u8]) -> Result<(), ChatStorageError> {
    let parent = path.parent().ok_or_else(ChatStorageError::unavailable)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| ChatStorageError::unavailable())?;
    if !canonical_parent.is_dir() {
        return Err(ChatStorageError::unavailable());
    }
    let file_name = path
        .file_name()
        .ok_or_else(ChatStorageError::invalid_input)?;
    let destination = canonical_parent.join(file_name);
    let staging = canonical_parent.join(format!(".openchat-markdown-{}.tmp", uuid::Uuid::new_v4()));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        file.write_all(content)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&staging);
        return Err(ChatStorageError::unavailable());
    }
    if replace_file_atomically(&staging, &destination).is_err() {
        let _ = fs::remove_file(&staging);
        return Err(ChatStorageError::unavailable());
    }
    Ok(())
}

pub(crate) fn is_safe_markdown_export_file_name(file_name: &str) -> bool {
    let path = Path::new(file_name);
    !file_name.is_empty()
        && file_name.len() <= 255
        && !file_name.chars().any(char::is_control)
        && !file_name.contains(['/', '\\', ':'])
        && path.components().count() == 1
        && path.file_name().and_then(std::ffi::OsStr::to_str) == Some(file_name)
        && path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

#[cfg(test)]
mod markdown_export_tests {
    use std::{fs, path::PathBuf};

    use super::{is_safe_markdown_export_file_name, write_markdown_export};
    use crate::chat_storage::CreateProjectInput;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-markdown-export-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated Markdown export directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn markdown_export_file_name_accepts_a_simple_markdown_name() {
        assert!(is_safe_markdown_export_file_name("conversation-notes.md"));
        assert!(is_safe_markdown_export_file_name("Sohbet Özeti.MD"));
    }

    #[test]
    fn markdown_export_file_name_rejects_paths_and_other_extensions() {
        assert!(!is_safe_markdown_export_file_name(""));
        assert!(!is_safe_markdown_export_file_name("..\\outside.md"));
        assert!(!is_safe_markdown_export_file_name(
            "C:\\Users\\Public\\outside.md"
        ));
        assert!(!is_safe_markdown_export_file_name("folder/notes.md"));
        assert!(!is_safe_markdown_export_file_name("notes.txt"));
    }

    #[test]
    fn markdown_export_replaces_existing_content_only_after_staging_write() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("conversation.md");
        fs::write(&destination, b"old export").expect("write existing export");

        write_markdown_export(&destination, b"complete new export")
            .expect("publish complete staged export");

        assert_eq!(
            fs::read(destination).expect("read published Markdown"),
            b"complete new export"
        );
    }

    #[test]
    fn project_creation_rejects_renderer_supplied_folder_paths() {
        let request = r#"{
            "id": "project-1",
            "name": "Project",
            "folderSelectionId": "550e8400-e29b-41d4-a716-446655440000",
            "folderPath": "C:\\private"
        }"#;

        assert!(serde_json::from_str::<CreateProjectInput>(request).is_err());
    }
}
