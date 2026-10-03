use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{protocol::ServiceError, storage::AppStorage};

const MAX_MODEL_SCAN_DEPTH: usize = 5;
const MAX_MODEL_SCAN_ENTRIES: usize = 20_000;
const MAX_DISCOVERED_MODELS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredModel {
    pub(crate) id: String,
    pub(crate) engine_id: String,
    pub(crate) display_name: String,
    pub(crate) path: PathBuf,
    pub(crate) path_kind: String,
    pub(crate) created_at_unix_ms: i64,
}

pub(crate) fn list(storage: &AppStorage) -> Result<Vec<RegisteredModel>, ServiceError> {
    let connection = connect(storage)?;
    let mut statement = connection
        .prepare(
            "SELECT id, engine_id, display_name, model_path, path_kind, created_at_unix_ms
             FROM local_models ORDER BY engine_id, display_name COLLATE NOCASE, id",
        )
        .map_err(|_| database_error())?;
    let rows = statement
        .query_map([], |row| {
            Ok(RegisteredModel {
                id: row.get(0)?,
                engine_id: row.get(1)?,
                display_name: row.get(2)?,
                path: PathBuf::from(row.get::<_, String>(3)?),
                path_kind: row.get(4)?,
                created_at_unix_ms: row.get(5)?,
            })
        })
        .map_err(|_| database_error())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| database_error())
}

pub(crate) fn discover(
    engine_id: &str,
    model_directory: &str,
    registered_paths: &[PathBuf],
) -> Result<Value, ServiceError> {
    if model_folder_name(engine_id).is_none() {
        return Err(invalid_model_error());
    }
    let directory = Path::new(model_directory);
    if !directory.is_absolute() {
        return Err(model_discovery_path_error());
    }
    let directory = directory
        .canonicalize()
        .map_err(|_| model_discovery_path_error())?;
    if !directory.is_dir() {
        return Err(model_discovery_path_error());
    }

    let registered_paths = registered_paths
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect::<HashSet<_>>();
    let (candidates, truncated) = scan_model_directory(engine_id, &directory, &registered_paths)?;
    let models = candidates
        .into_iter()
        .map(|model| {
            let path = model.path.to_str().ok_or_else(model_discovery_path_error)?;
            Ok(json!({
                "engineId": engine_id,
                "displayName": model.display_name,
                "path": path,
                "pathKind": model.path_kind,
            }))
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;
    Ok(json!({"models": models, "truncated": truncated}))
}

struct DiscoveredModel {
    display_name: String,
    path: PathBuf,
    path_kind: &'static str,
}

fn scan_model_directory(
    engine_id: &str,
    model_directory: &Path,
    registered_paths: &HashSet<PathBuf>,
) -> Result<(Vec<DiscoveredModel>, bool), ServiceError> {
    let mut directories = vec![(model_directory.to_path_buf(), 0usize)];
    let mut candidates = Vec::new();
    let mut entries_scanned = 0usize;
    let mut truncated = false;

    while let Some((directory, depth)) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(|_| model_discovery_error())? {
            if entries_scanned >= MAX_MODEL_SCAN_ENTRIES {
                truncated = true;
                break;
            }
            entries_scanned += 1;
            let entry = entry.map_err(|_| model_discovery_error())?;
            let entry_type = entry.file_type().map_err(|_| model_discovery_error())?;
            if entry_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            if entry_type.is_file() {
                if engine_id == "llama_cpp"
                    && path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf"))
                    && !registered_paths.contains(&path)
                {
                    let display_name = path
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .filter(|name| !name.trim().is_empty())
                        .ok_or_else(model_discovery_error)?
                        .to_owned();
                    candidates.push(DiscoveredModel {
                        display_name,
                        path,
                        path_kind: "file",
                    });
                }
                continue;
            }
            if !entry_type.is_dir() {
                continue;
            }

            if engine_id != "llama_cpp"
                && !registered_paths.contains(&path)
                && has_transformers_model_files(&path)?
            {
                let display_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| !name.trim().is_empty())
                    .ok_or_else(model_discovery_error)?
                    .to_owned();
                candidates.push(DiscoveredModel {
                    display_name,
                    path,
                    path_kind: "directory",
                });
                continue;
            }
            if depth < MAX_MODEL_SCAN_DEPTH {
                directories.push((path, depth + 1));
            }
        }
        if truncated {
            break;
        }
    }

    candidates.sort_by(|left, right| {
        left.display_name
            .to_lowercase()
            .cmp(&right.display_name.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    if candidates.len() > MAX_DISCOVERED_MODELS {
        candidates.truncate(MAX_DISCOVERED_MODELS);
        truncated = true;
    }
    Ok((candidates, truncated))
}

pub(crate) fn find(
    storage: &AppStorage,
    model_id: &str,
) -> Result<Option<RegisteredModel>, ServiceError> {
    let connection = connect(storage)?;
    connection
        .query_row(
            "SELECT id, engine_id, display_name, model_path, path_kind, created_at_unix_ms
             FROM local_models WHERE id = ?1",
            [model_id],
            |row| {
                Ok(RegisteredModel {
                    id: row.get(0)?,
                    engine_id: row.get(1)?,
                    display_name: row.get(2)?,
                    path: PathBuf::from(row.get::<_, String>(3)?),
                    path_kind: row.get(4)?,
                    created_at_unix_ms: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|_| database_error())
}

pub(crate) fn ensure_storage_directories(root: &Path) -> Result<(), ServiceError> {
    for directory in ["llama", "exllama", "vllm"] {
        fs::create_dir_all(root.join("models").join(directory))
            .map_err(|_| model_storage_path_error())?;
    }
    Ok(())
}

pub(crate) fn default_model_directory(
    storage_root: &Path,
    engine_id: &str,
) -> Result<PathBuf, ServiceError> {
    let engine_folder = model_folder_name(engine_id).ok_or_else(invalid_model_error)?;
    Ok(storage_root.join("models").join(engine_folder))
}

pub(crate) fn resolve_model_directory(
    storage_root: &Path,
    engine_id: &str,
    model_directory: Option<&str>,
) -> Result<PathBuf, ServiceError> {
    if model_folder_name(engine_id).is_none() {
        return Err(invalid_model_error());
    }
    let Some(model_directory) = model_directory else {
        ensure_storage_directories(storage_root)?;
        return default_model_directory(storage_root, engine_id);
    };

    let selected_directory = Path::new(model_directory);
    if !selected_directory.is_absolute() {
        return Err(model_discovery_path_error());
    }
    let selected_directory = selected_directory
        .canonicalize()
        .map_err(|_| model_discovery_path_error())?;
    if !selected_directory.is_dir() {
        return Err(model_discovery_path_error());
    }
    Ok(selected_directory)
}

pub(crate) fn register_with_storage_action(
    database_root: &Path,
    engine_id: &str,
    model_path: &str,
    model_directory: Option<&str>,
    storage_action: &str,
) -> Result<RegisteredModel, ServiceError> {
    let action = match storage_action {
        "move" => StorageAction::Move,
        "copy" => StorageAction::Copy,
        "keep" => StorageAction::Keep,
        _ => return Err(invalid_storage_action_error()),
    };

    let (source_path, path_kind) = validate_model_path(engine_id, model_path)?;
    if action == StorageAction::Keep {
        return register_validated(database_root, engine_id, source_path, path_kind);
    }

    let target_directory = resolve_model_directory(database_root, engine_id, model_directory)?;

    if action == StorageAction::Move && source_path.parent() == Some(&target_directory) {
        return register_validated(database_root, engine_id, source_path, path_kind);
    }

    let destination = unique_destination(&target_directory, &source_path, path_kind)?;
    let transfer = transfer_model(&source_path, &destination, path_kind, action)?;
    let destination = match destination.canonicalize() {
        Ok(destination) => destination,
        Err(_) => {
            if rollback_transfer(&transfer, path_kind).is_err() {
                return Err(transfer_recovery_error());
            }
            return Err(model_storage_path_error());
        }
    };
    match register_validated(database_root, engine_id, destination, path_kind) {
        Ok(model) => Ok(model),
        Err(error) => {
            if rollback_transfer(&transfer, path_kind).is_err() {
                return Err(transfer_recovery_error());
            }
            Err(error)
        }
    }
}

fn register_validated(
    storage_root: &Path,
    engine_id: &str,
    model_path: PathBuf,
    path_kind: &'static str,
) -> Result<RegisteredModel, ServiceError> {
    let path_string = model_path
        .to_str()
        .ok_or_else(invalid_model_path_error)?
        .to_owned();
    let display_name = if path_kind == "file" {
        model_path.file_stem()
    } else {
        model_path.file_name()
    }
    .and_then(|name| name.to_str())
    .filter(|name| !name.trim().is_empty())
    .ok_or_else(invalid_model_path_error)?
    .to_owned();
    let connection = connect_at_root(storage_root)?;

    if let Some(model) = connection
        .query_row(
            "SELECT id, engine_id, display_name, model_path, path_kind, created_at_unix_ms
             FROM local_models WHERE engine_id = ?1 AND model_path = ?2",
            params![engine_id, path_string],
            |row| {
                Ok(RegisteredModel {
                    id: row.get(0)?,
                    engine_id: row.get(1)?,
                    display_name: row.get(2)?,
                    path: PathBuf::from(row.get::<_, String>(3)?),
                    path_kind: row.get(4)?,
                    created_at_unix_ms: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|_| database_error())?
    {
        return Ok(model);
    }

    let model = RegisteredModel {
        id: Uuid::new_v4().to_string(),
        engine_id: engine_id.to_owned(),
        display_name,
        path: model_path,
        path_kind: path_kind.to_owned(),
        created_at_unix_ms: current_time_millis()?,
    };
    connection
        .execute(
            "INSERT INTO local_models (
                id, engine_id, display_name, model_path, path_kind, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                model.id,
                model.engine_id,
                model.display_name,
                model.path.to_str().ok_or_else(invalid_model_path_error)?,
                model.path_kind,
                model.created_at_unix_ms,
            ],
        )
        .map_err(|_| database_error())?;
    Ok(model)
}

fn validate_model_path(
    engine_id: &str,
    model_path: &str,
) -> Result<(PathBuf, &'static str), ServiceError> {
    if !matches!(engine_id, "llama_cpp" | "vllm" | "exllama") {
        return Err(invalid_model_error());
    }

    let selected_path = selected_entry_path(Path::new(model_path))?;
    let metadata = fs::metadata(&selected_path).map_err(|_| invalid_model_path_error())?;
    let path_kind = if engine_id == "llama_cpp" {
        if !metadata.is_file()
            || selected_path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("gguf"))
        {
            return Err(invalid_model_error());
        }
        "file"
    } else {
        if !metadata.is_dir() || !has_transformers_model_files(&selected_path)? {
            return Err(invalid_model_error());
        }
        "directory"
    };
    Ok((selected_path, path_kind))
}

fn selected_entry_path(path: &Path) -> Result<PathBuf, ServiceError> {
    let file_name = path.file_name().ok_or_else(invalid_model_path_error)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let absolute_parent = if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| invalid_model_path_error())?
            .join(parent)
    };
    let canonical_parent = absolute_parent
        .canonicalize()
        .map_err(|_| invalid_model_path_error())?;
    Ok(canonical_parent.join(file_name))
}

pub(crate) fn remove(storage: &AppStorage, model_id: &str) -> Result<bool, ServiceError> {
    let connection = connect(storage)?;
    connection
        .execute("DELETE FROM local_models WHERE id = ?1", [model_id])
        .map(|rows| rows > 0)
        .map_err(|_| database_error())
}

pub(crate) fn to_json(model: &RegisteredModel, available: bool) -> Value {
    let path_exists = fs::metadata(&model.path).is_ok_and(|metadata| {
        if model.path_kind == "file" {
            metadata.is_file()
        } else {
            metadata.is_dir()
        }
    });
    json!({
        "id": model.id,
        "engineId": model.engine_id,
        "displayName": model.display_name,
        "description": Value::Null,
        "contextWindow": Value::Null,
        "reasoningLevels": [],
        "supportsReasoning": false,
        "supportsImages": false,
        "defaultReasoningLevel": Value::Null,
        "path": model.path,
        "pathKind": model.path_kind,
        "pathExists": path_exists,
        "isAvailable": path_exists && available,
        "reason": if !path_exists {
            json!("model_file_missing")
        } else if available {
            Value::Null
        } else {
            json!("local_engine_not_ready")
        },
        "createdAtUnixMs": model.created_at_unix_ms,
    })
}

fn connect(storage: &AppStorage) -> Result<Connection, ServiceError> {
    connect_at_root(storage.root())
}

fn connect_at_root(root: &Path) -> Result<Connection, ServiceError> {
    let database_path = root.join("db").join("local_models.sqlite3");
    let connection = Connection::open(database_path).map_err(|_| database_error())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| database_error())?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS local_models (
                id TEXT PRIMARY KEY NOT NULL,
                engine_id TEXT NOT NULL CHECK (engine_id IN ('llama_cpp', 'vllm', 'exllama')),
                display_name TEXT NOT NULL,
                model_path TEXT NOT NULL,
                path_kind TEXT NOT NULL CHECK (path_kind IN ('file', 'directory')),
                created_at_unix_ms INTEGER NOT NULL,
                UNIQUE (engine_id, model_path)
             );",
        )
        .map_err(|_| database_error())?;
    Ok(connection)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StorageAction {
    Move,
    Copy,
    Keep,
}

struct ModelTransfer {
    source: PathBuf,
    destination: PathBuf,
    action: StorageAction,
}

fn model_folder_name(engine_id: &str) -> Option<&'static str> {
    match engine_id {
        "llama_cpp" => Some("llama"),
        "exllama" => Some("exllama"),
        "vllm" => Some("vllm"),
        _ => None,
    }
}

fn unique_destination(
    target_directory: &Path,
    source: &Path,
    path_kind: &str,
) -> Result<PathBuf, ServiceError> {
    let file_name = source
        .file_name()
        .ok_or_else(invalid_model_path_error)?
        .to_str()
        .ok_or_else(invalid_model_path_error)?;
    let original = target_directory.join(file_name);
    if !path_entry_exists(&original)? {
        return Ok(original);
    }

    let source_name = Path::new(file_name);
    for suffix in 2_u64.. {
        let candidate_name = if path_kind == "file" {
            let stem = source_name
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or_else(invalid_model_path_error)?;
            match source_name
                .extension()
                .and_then(|extension| extension.to_str())
            {
                Some(extension) => format!("{stem} ({suffix}).{extension}"),
                None => format!("{stem} ({suffix})"),
            }
        } else {
            format!("{file_name} ({suffix})")
        };
        let candidate = target_directory.join(candidate_name);
        if !path_entry_exists(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(model_storage_path_error())
}

fn path_entry_exists(path: &Path) -> Result<bool, ServiceError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(model_storage_path_error()),
    }
}

fn transfer_model(
    source: &Path,
    destination: &Path,
    path_kind: &str,
    action: StorageAction,
) -> Result<ModelTransfer, ServiceError> {
    match action {
        StorageAction::Copy => copy_to_destination(source, destination, path_kind)?,
        StorageAction::Move => {
            let has_links = contains_symbolic_link(source)?;
            if has_links || fs::rename(source, destination).is_err() {
                copy_to_destination(source, destination, path_kind)?;
                if remove_model_path(source, path_kind).is_err() {
                    return Err(transfer_recovery_error());
                }
            }
        }
        StorageAction::Keep => return Err(invalid_storage_action_error()),
    }
    Ok(ModelTransfer {
        source: source.to_path_buf(),
        destination: destination.to_path_buf(),
        action,
    })
}

fn copy_to_destination(
    source: &Path,
    destination: &Path,
    path_kind: &str,
) -> Result<(), ServiceError> {
    let parent = destination.parent().ok_or_else(model_storage_path_error)?;
    let staging_path = parent.join(format!(".openchat-model-{}.tmp", Uuid::new_v4()));
    let copy_result = if path_kind == "file" {
        fs::copy(source, &staging_path)
            .map(|_| ())
            .map_err(|_| transfer_error())
    } else {
        copy_model_directory(source, &staging_path).map_err(|_| transfer_error())
    };
    if copy_result.is_err() {
        let _ = remove_model_path(&staging_path, path_kind);
        return copy_result;
    }
    if fs::rename(&staging_path, destination).is_err() {
        let _ = remove_model_path(&staging_path, path_kind);
        return Err(transfer_error());
    }
    Ok(())
}

fn copy_model_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let entry_type = entry.file_type()?;
        if entry_type.is_symlink() {
            let target_metadata = fs::metadata(&source_path)?;
            if target_metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Directory symbolic links are not supported in model folders.",
                ));
            }
            if !target_metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Unsupported symbolic link in model folder.",
                ));
            }
            fs::copy(&source_path, &destination_path)?;
        } else if entry_type.is_dir() {
            copy_model_directory(&source_path, &destination_path)?;
        } else if entry_type.is_file() {
            fs::copy(&source_path, &destination_path)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Unsupported file type in model folder.",
            ));
        }
    }
    Ok(())
}

fn contains_symbolic_link(path: &Path) -> Result<bool, ServiceError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| invalid_model_path_error())?;
    if metadata.file_type().is_symlink() {
        return Ok(true);
    }
    if !metadata.is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(path).map_err(|_| invalid_model_path_error())? {
        let entry = entry.map_err(|_| invalid_model_path_error())?;
        let child = entry.path();
        if entry
            .file_type()
            .map_err(|_| invalid_model_path_error())?
            .is_symlink()
            || contains_symbolic_link(&child)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rollback_transfer(transfer: &ModelTransfer, path_kind: &str) -> Result<(), ()> {
    match transfer.action {
        StorageAction::Copy => remove_model_path(&transfer.destination, path_kind).map_err(|_| ()),
        StorageAction::Move => {
            if path_entry_exists(&transfer.source).map_err(|_| ())? {
                return Err(());
            }
            if fs::rename(&transfer.destination, &transfer.source).is_ok() {
                return Ok(());
            }
            copy_to_destination(&transfer.destination, &transfer.source, path_kind)
                .map_err(|_| ())?;
            remove_model_path(&transfer.destination, path_kind).map_err(|_| ())
        }
        StorageAction::Keep => Ok(()),
    }
}

fn remove_model_path(path: &Path, path_kind: &str) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        #[cfg(windows)]
        {
            return fs::remove_file(path).or_else(|_| fs::remove_dir(path));
        }
        #[cfg(not(windows))]
        {
            return fs::remove_file(path);
        }
    }
    if path_kind == "file" {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    }
}

fn has_transformers_model_files(path: &Path) -> Result<bool, ServiceError> {
    let config_path = path.join("config.json");
    if !config_path.is_file() {
        return Ok(false);
    }
    let entries = fs::read_dir(path).map_err(|_| invalid_model_path_error())?;
    for entry in entries {
        let entry = entry.map_err(|_| invalid_model_path_error())?;
        if !entry
            .file_type()
            .map_err(|_| invalid_model_path_error())?
            .is_file()
        {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let name = name.to_ascii_lowercase();
        if name.ends_with(".safetensors")
            || name.ends_with(".safetensors.index.json")
            || name.ends_with(".bin")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn current_time_millis() -> Result<i64, ServiceError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| database_error())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| database_error())
}

fn invalid_model_error() -> ServiceError {
    ServiceError::new(
        "local_model_format_invalid",
        "The selected path is not a supported model for this engine.",
        false,
    )
}

fn invalid_model_path_error() -> ServiceError {
    ServiceError::new(
        "local_model_path_unavailable",
        "The selected model path could not be accessed.",
        false,
    )
}

fn database_error() -> ServiceError {
    ServiceError::new(
        "local_model_storage_unavailable",
        "Registered local models could not be read or saved.",
        true,
    )
}

fn invalid_storage_action_error() -> ServiceError {
    ServiceError::new(
        "local_model_storage_action_invalid",
        "The selected model storage action is invalid.",
        false,
    )
}

fn model_storage_path_error() -> ServiceError {
    ServiceError::new(
        "local_model_storage_path_unavailable",
        "The OpenChat model storage folder could not be accessed.",
        false,
    )
}

fn model_discovery_path_error() -> ServiceError {
    ServiceError::new(
        "local_model_directory_unavailable",
        "The selected model directory could not be accessed.",
        false,
    )
}

fn model_discovery_error() -> ServiceError {
    ServiceError::new(
        "local_model_discovery_failed",
        "The selected model directory could not be scanned.",
        false,
    )
}

fn transfer_error() -> ServiceError {
    ServiceError::new(
        "local_model_transfer_failed",
        "The selected model could not be copied or moved.",
        false,
    )
}

fn transfer_recovery_error() -> ServiceError {
    ServiceError::new(
        "local_model_transfer_recovery_needed",
        "The model transfer finished, but the model could not be registered or restored. It remains in the OpenChat model folder.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fs, path::PathBuf};

    use uuid::Uuid;

    use crate::storage::AppStorage;

    use super::{
        MAX_DISCOVERED_MODELS, discover, register_with_storage_action, scan_model_directory,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-local-model-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("test directory should be created");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovers_nested_gguf_files_without_matching_other_extensions() {
        let temporary = TestDirectory::new();
        let model_directory = temporary.0.join("models");
        let nested = model_directory.join("Qwen").join("quantized");
        fs::create_dir_all(&nested).expect("model directory should be created");
        let model = nested.join("qwen-7b.gguf");
        fs::write(&model, b"model").expect("model fixture should be written");
        fs::write(nested.join("readme.md"), b"not a model")
            .expect("non-model fixture should be written");

        let (found, truncated) =
            scan_model_directory("llama_cpp", &model_directory, &HashSet::new())
                .expect("scan should succeed");

        assert!(!truncated);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, model);
        assert_eq!(found[0].path_kind, "file");
    }

    #[test]
    fn discovers_transformers_models_for_the_selected_engine() {
        let temporary = TestDirectory::new();
        let model_directory = temporary.0.join("models");
        let model = model_directory.join("Qwen-7B");
        fs::create_dir_all(&model).expect("model directory should be created");
        fs::write(model.join("config.json"), b"{}")
            .expect("model config fixture should be written");
        fs::write(model.join("model.safetensors"), b"weights")
            .expect("model weights fixture should be written");

        for engine_id in ["vllm", "exllama"] {
            let (found, truncated) =
                scan_model_directory(engine_id, &model_directory, &HashSet::new())
                    .expect("scan should succeed");

            assert!(!truncated);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].path, model);
            assert_eq!(found[0].path_kind, "directory");
        }
    }

    #[test]
    fn discovery_stops_after_the_candidate_limit() {
        let temporary = TestDirectory::new();
        for index in 0..=MAX_DISCOVERED_MODELS {
            fs::write(temporary.0.join(format!("model-{index}.gguf")), b"model")
                .expect("model fixture should be written");
        }

        let (found, truncated) = scan_model_directory("llama_cpp", &temporary.0, &HashSet::new())
            .expect("scan should succeed");

        assert_eq!(found.len(), MAX_DISCOVERED_MODELS);
        assert!(truncated);
    }

    #[test]
    fn registered_models_do_not_consume_the_discovery_result_limit() {
        let temporary = TestDirectory::new();
        let mut registered_paths = HashSet::new();
        for index in 0..=MAX_DISCOVERED_MODELS {
            let model_path = temporary.0.join(format!("model-{index}.gguf"));
            fs::write(&model_path, b"model").expect("model fixture should be written");
            if index == 0 {
                registered_paths.insert(model_path);
            }
        }

        let (found, truncated) = scan_model_directory("llama_cpp", &temporary.0, &registered_paths)
            .expect("scan should succeed");

        assert_eq!(found.len(), MAX_DISCOVERED_MODELS);
        assert!(!truncated);
    }

    #[test]
    fn discovery_omits_models_that_are_already_registered() {
        let temporary = TestDirectory::new();
        let database_root = temporary.0.join("app");
        let storage = AppStorage::open_at(database_root).expect("test storage should open");
        let model_directory = temporary.0.join("models");
        fs::create_dir_all(&model_directory).expect("model directory should be created");
        let model_path = model_directory.join("qwen-7b.gguf");
        fs::write(&model_path, b"model").expect("model fixture should be written");
        let model_path_string = model_path.to_str().expect("fixture path should be UTF-8");
        let model_directory_string = model_directory
            .to_str()
            .expect("fixture path should be UTF-8");
        let registered = register_with_storage_action(
            storage.root(),
            "llama_cpp",
            model_path_string,
            None,
            "keep",
        )
        .expect("model should be registered");

        let discovery = discover("llama_cpp", model_directory_string, &[registered.path])
            .expect("discovery should succeed");

        assert_eq!(discovery["models"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn copies_registered_models_into_the_selected_engine_directory() {
        let temporary = TestDirectory::new();
        let database_root = temporary.0.join("app");
        let storage = AppStorage::open_at(database_root).expect("test storage should open");
        let source_directory = temporary.0.join("source");
        let model_directory = temporary.0.join("chosen-models");
        fs::create_dir_all(&source_directory).expect("source directory should be created");
        fs::create_dir_all(&model_directory).expect("model directory should be created");
        let source_path = source_directory.join("qwen-7b.gguf");
        fs::write(&source_path, b"model").expect("model fixture should be written");

        let model = register_with_storage_action(
            storage.root(),
            "llama_cpp",
            source_path.to_str().expect("fixture path should be UTF-8"),
            Some(
                model_directory
                    .to_str()
                    .expect("fixture path should be UTF-8"),
            ),
            "copy",
        )
        .expect("model should be copied and registered");

        assert!(
            model.path.starts_with(
                model_directory
                    .canonicalize()
                    .expect("folder should resolve")
            )
        );
        assert!(model.path.is_file());
        assert!(source_path.is_file());
    }
}
