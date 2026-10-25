//! Captures project roots through a native picker and binds them to short-lived selections.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::request_bounds::BoundedText;
use serde::Serialize;
use tauri::{WebviewWindow, Wry};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

const MAXIMUM_PENDING_SELECTIONS: usize = 8;
const SELECTION_LIFETIME: Duration = Duration::from_secs(5 * 60);
const MAXIMUM_PATH_LENGTH: usize = 4096;
const MAXIMUM_DIALOG_TITLE_LENGTH: usize = 128;
const MAXIMUM_DIALOG_TITLE_BYTES: usize = MAXIMUM_DIALOG_TITLE_LENGTH * 4;
const MAXIMUM_SELECTION_ID_BYTES: usize = 128;

static PENDING_PROJECT_FOLDERS: OnceLock<Mutex<HashMap<String, PendingProjectFolder>>> =
    OnceLock::new();

struct PendingProjectFolder {
    selected_path: PathBuf,
    canonical_path: PathBuf,
    expires_at: Instant,
}

/// A folder selected in the native picker, paired with a single-use creation token.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectedProjectFolder {
    selection_id: String,
    folder_path: String,
}

/// A safe, user-facing error from native project folder selection.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectFolderError {
    code: &'static str,
    message: &'static str,
}

impl ProjectFolderError {
    fn invalid_input() -> Self {
        Self {
            code: "project_folder_invalid",
            message: "The project folder selection is invalid or has expired.",
        }
    }

    fn unavailable() -> Self {
        Self {
            code: "project_folder_unavailable",
            message: "The project folder could not be selected safely.",
        }
    }

    fn selection_limit() -> Self {
        Self {
            code: "project_folder_selection_limit",
            message: "Finish or cancel an existing project folder selection first.",
        }
    }
}

/// Opens the native directory picker and returns a one-time token for the selected folder.
#[tauri::command]
pub(crate) async fn select_project_folder(
    window: WebviewWindow<Wry>,
    title: BoundedText<MAXIMUM_DIALOG_TITLE_BYTES>,
    initial_path: Option<BoundedText<MAXIMUM_PATH_LENGTH>>,
    previous_selection_id: Option<BoundedText<MAXIMUM_SELECTION_ID_BYTES>>,
) -> Result<Option<SelectedProjectFolder>, ProjectFolderError> {
    let title = title.into_inner();
    let initial_path = initial_path.map(BoundedText::into_inner);
    let previous_selection_id = previous_selection_id.map(BoundedText::into_inner);
    let title = title.trim();
    if title.is_empty()
        || title.chars().count() > MAXIMUM_DIALOG_TITLE_LENGTH
        || title.chars().any(char::is_control)
    {
        return Err(ProjectFolderError::invalid_input());
    }

    let mut dialog = window.dialog().file().set_parent(&window).set_title(title);
    let expected_path = if let Some(initial_path) = initial_path {
        let initial_path = initial_path.trim();
        if initial_path.is_empty()
            || initial_path.len() > MAXIMUM_PATH_LENGTH
            || !Path::new(initial_path).is_absolute()
        {
            return Err(ProjectFolderError::invalid_input());
        }
        let initial_path =
            fs::canonicalize(initial_path).map_err(|_| ProjectFolderError::invalid_input())?;
        if !initial_path.is_dir() {
            return Err(ProjectFolderError::invalid_input());
        }
        dialog = dialog.set_directory(&initial_path);
        Some(initial_path)
    } else {
        None
    };

    let (sender, receiver) = tokio::sync::oneshot::channel();
    dialog.pick_folder(move |path| {
        let _ = sender.send(path);
    });
    let selected = receiver
        .await
        .map_err(|_| ProjectFolderError::unavailable())?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let selected_path = selected
        .into_path()
        .map_err(|_| ProjectFolderError::unavailable())?;
    if selected_path.to_str().is_none() || selected_path.as_os_str().len() > MAXIMUM_PATH_LENGTH {
        return Err(ProjectFolderError::invalid_input());
    }
    let canonical_path = canonical_project_folder(&selected_path)?;
    if expected_path
        .as_ref()
        .is_some_and(|expected_path| expected_path != &canonical_path)
    {
        return Err(ProjectFolderError::invalid_input());
    }
    let selection_id = store_selection(
        selected_path.clone(),
        canonical_path,
        previous_selection_id.as_deref(),
    )?;

    Ok(Some(SelectedProjectFolder {
        selection_id,
        folder_path: selected_path.to_string_lossy().into_owned(),
    }))
}

/// Releases a selected folder token when the project dialog is cancelled.
#[tauri::command]
pub(crate) fn discard_project_folder_selection(
    selection_id: BoundedText<MAXIMUM_SELECTION_ID_BYTES>,
) -> Result<(), ProjectFolderError> {
    let selection_id = selection_id.into_inner();
    let selection_id = selection_id.trim();
    if Uuid::parse_str(selection_id).is_err() {
        return Err(ProjectFolderError::invalid_input());
    }
    let mut selections = pending_selections()
        .lock()
        .map_err(|_| ProjectFolderError::unavailable())?;
    selections.remove(selection_id);
    Ok(())
}

/// Consumes a single-use native folder selection and returns its canonical path.
#[tauri::command]
pub(crate) fn consume_project_folder_selection_command(
    selection_id: BoundedText<MAXIMUM_SELECTION_ID_BYTES>,
) -> Result<String, ProjectFolderError> {
    consume_project_folder_selection(&selection_id.into_inner())?
        .to_str()
        .map(str::to_owned)
        .ok_or_else(ProjectFolderError::invalid_input)
}

/// Consumes a selection token and revalidates that the chosen directory still exists.
pub(crate) fn consume_project_folder_selection(
    selection_id: &str,
) -> Result<PathBuf, ProjectFolderError> {
    if Uuid::parse_str(selection_id).is_err() {
        return Err(ProjectFolderError::invalid_input());
    }

    let mut selections = pending_selections()
        .lock()
        .map_err(|_| ProjectFolderError::unavailable())?;
    purge_expired_selections(&mut selections, Instant::now());
    let selection = selections
        .remove(selection_id)
        .ok_or_else(ProjectFolderError::invalid_input)?;
    if selection.expires_at <= Instant::now() {
        return Err(ProjectFolderError::invalid_input());
    }

    let current_path = canonical_project_folder(&selection.selected_path)?;
    if current_path != selection.canonical_path {
        return Err(ProjectFolderError::invalid_input());
    }
    Ok(current_path)
}

fn canonical_project_folder(path: &Path) -> Result<PathBuf, ProjectFolderError> {
    let canonical_path = fs::canonicalize(path).map_err(|_| ProjectFolderError::unavailable())?;
    if !canonical_path.is_dir()
        || canonical_path.to_str().is_none()
        || canonical_path.as_os_str().len() > MAXIMUM_PATH_LENGTH
    {
        return Err(ProjectFolderError::invalid_input());
    }
    Ok(canonical_path)
}

fn store_selection(
    selected_path: PathBuf,
    canonical_path: PathBuf,
    previous_selection_id: Option<&str>,
) -> Result<String, ProjectFolderError> {
    let now = Instant::now();
    let mut selections = pending_selections()
        .lock()
        .map_err(|_| ProjectFolderError::unavailable())?;
    purge_expired_selections(&mut selections, now);
    if let Some(previous_selection_id) = previous_selection_id {
        if Uuid::parse_str(previous_selection_id).is_err() {
            return Err(ProjectFolderError::invalid_input());
        }
        selections.remove(previous_selection_id);
    }
    if selections.len() >= MAXIMUM_PENDING_SELECTIONS {
        return Err(ProjectFolderError::selection_limit());
    }

    let selection_id = Uuid::new_v4().to_string();
    selections.insert(
        selection_id.clone(),
        PendingProjectFolder {
            selected_path,
            canonical_path,
            expires_at: now + SELECTION_LIFETIME,
        },
    );
    Ok(selection_id)
}

fn pending_selections() -> &'static Mutex<HashMap<String, PendingProjectFolder>> {
    PENDING_PROJECT_FOLDERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn purge_expired_selections(selections: &mut HashMap<String, PendingProjectFolder>, now: Instant) {
    selections.retain(|_, selection| selection.expires_at > now);
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::Instant};

    use super::{
        canonical_project_folder, consume_project_folder_selection, pending_selections,
        store_selection, BoundedText, PendingProjectFolder, MAXIMUM_SELECTION_ID_BYTES,
    };

    struct TestFolder(PathBuf);

    impl TestFolder {
        fn create() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-project-folder-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated test folder");
            Self(path)
        }
    }

    impl Drop for TestFolder {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test folder");
        }
    }

    #[test]
    fn selected_folder_token_is_single_use() {
        let folder = TestFolder::create();
        let canonical_path = canonical_project_folder(&folder.0).expect("canonical folder");
        let selection_id = store_selection(folder.0.clone(), canonical_path.clone(), None)
            .expect("store selection");

        assert_eq!(
            consume_project_folder_selection(&selection_id).expect("consume selection"),
            folder.0
        );
        assert!(consume_project_folder_selection(&selection_id).is_err());
    }

    #[test]
    fn selecting_a_replacement_folder_invalidates_the_previous_token() {
        let first_folder = TestFolder::create();
        let second_folder = TestFolder::create();
        let first_canonical_path =
            canonical_project_folder(&first_folder.0).expect("first canonical folder");
        let second_canonical_path =
            canonical_project_folder(&second_folder.0).expect("second canonical folder");
        let first_selection_id =
            store_selection(first_folder.0.clone(), first_canonical_path, None)
                .expect("store first selection");
        let second_selection_id = store_selection(
            second_folder.0.clone(),
            second_canonical_path,
            Some(&first_selection_id),
        )
        .expect("store replacement selection");

        assert!(consume_project_folder_selection(&first_selection_id).is_err());
        assert_eq!(
            consume_project_folder_selection(&second_selection_id).expect("consume replacement"),
            second_folder.0
        );
    }

    #[test]
    fn cancelling_a_folder_selection_releases_its_token() {
        let folder = TestFolder::create();
        let canonical_path = canonical_project_folder(&folder.0).expect("canonical folder");
        let selection_id =
            store_selection(folder.0.clone(), canonical_path, None).expect("store selection");

        let selection_input = serde_json::from_str::<BoundedText<MAXIMUM_SELECTION_ID_BYTES>>(
            &serde_json::to_string(&selection_id).expect("encode selection id"),
        )
        .expect("bounded selection id");
        super::discard_project_folder_selection(selection_input).expect("discard selection");
        assert!(consume_project_folder_selection(&selection_id).is_err());
    }

    #[test]
    fn expired_folder_tokens_are_removed_and_rejected() {
        let folder = TestFolder::create();
        let canonical_path = canonical_project_folder(&folder.0).expect("canonical folder");
        let selection_id = uuid::Uuid::new_v4().to_string();
        let expired_at = Instant::now() - std::time::Duration::from_secs(1);
        let mut selections = pending_selections().lock().expect("selection lock");
        selections.insert(
            selection_id.clone(),
            PendingProjectFolder {
                selected_path: folder.0.clone(),
                canonical_path,
                expires_at: expired_at,
            },
        );
        drop(selections);

        assert!(consume_project_folder_selection(&selection_id).is_err());
    }

    #[test]
    fn selection_rejects_a_folder_removed_after_picker_confirmation() {
        let folder = TestFolder::create();
        let canonical_path = canonical_project_folder(&folder.0).expect("canonical folder");
        let selection_id =
            store_selection(folder.0.clone(), canonical_path, None).expect("store selection");
        fs::remove_dir(&folder.0).expect("remove selected folder");

        assert!(consume_project_folder_selection(&selection_id).is_err());
        fs::create_dir(&folder.0).expect("restore test folder for cleanup");
    }
}
