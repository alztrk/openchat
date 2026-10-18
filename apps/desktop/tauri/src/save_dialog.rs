//! Provides the window-owned native save dialog used by export commands.

use std::path::PathBuf;

use tauri_plugin_dialog::DialogExt;

/// Failures while presenting or resolving a user-selected file destination.
#[derive(Debug)]
pub(crate) enum SaveFileDialogError {
    /// The dialog callback did not complete or its returned location was not a filesystem path.
    Unavailable,
}

/// Opens a save dialog owned by the active window and returns the selected filesystem path.
///
/// The function is asynchronous and returns `Ok(None)` when the user cancels. It does not
/// create or overwrite the selected file; the caller owns destination validation and writing.
///
/// # Errors
/// Returns `Unavailable` if the dialog callback fails or its selection is not a filesystem path.
pub(crate) async fn select_save_file(
    window: &tauri::WebviewWindow,
    file_name: String,
    title: String,
    filter_name: &'static str,
    extension: &'static str,
) -> Result<Option<PathBuf>, SaveFileDialogError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(window)
        .set_file_name(file_name)
        .set_title(title)
        .add_filter(filter_name, &[extension])
        .save_file(move |path| {
            let _ = sender.send(path);
        });

    let path = receiver
        .await
        .map_err(|_| SaveFileDialogError::Unavailable)?;
    path.map(|path| {
        path.into_path()
            .map_err(|_| SaveFileDialogError::Unavailable)
    })
    .transpose()
}
