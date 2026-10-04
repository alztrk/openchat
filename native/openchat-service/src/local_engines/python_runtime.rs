//! Installs Python-based local engines into their app-managed runtime roots.

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::{fs, process::Command, sync::watch, time::timeout};

use crate::{local_engines::catalog::EngineVariant, protocol::ServiceError};

const EXLLAMA_VERSION: &str = "1.5.2";
const PYTHON_VERSION: &str = "3.12.13";
const UV_CACHE_DIRECTORY: &str = ".uv-cache";

struct ExllamaRuntimeFiles {
    project: &'static [u8],
    lock: &'static [u8],
    uv_executable: &'static str,
    wheel_name: &'static str,
    python_executable: &'static str,
}

/// Resolve Python lock files and runtime paths for a supported ExLlama variant.
pub(crate) async fn install_exllama(
    runtime_root: &Path,
    variant: &EngineVariant,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let files = exllama_runtime_files(variant)?;
    let uv = runtime_root.join(files.uv_executable);
    let wheel = runtime_root.join("wheels").join(files.wheel_name);
    let python_root = runtime_root.join("python");
    let python_install_dir = python_root.join("interpreters");
    let project_root = python_root.join("environment");
    let venv_python = project_root.join(files.python_executable);

    require_regular_file(&uv)?;
    require_regular_file(&wheel)?;
    require_regular_file(&runtime_root.join("tabbyAPI").join("main.py"))?;
    fs::create_dir_all(&project_root)
        .await
        .map_err(|_| runtime_install_error())?;
    write_bytes(&project_root.join("pyproject.toml"), files.project).await?;
    write_bytes(&project_root.join("uv.lock"), files.lock).await?;

    check_cancelled(cancellation)?;
    let cache_dir = runtime_root.join(UV_CACHE_DIRECTORY);
    let install_dir_arg = python_install_dir.as_os_str().to_owned();
    run_uv(
        &uv,
        runtime_root,
        &python_install_dir,
        &cache_dir,
        [
            OsString::from("python"),
            OsString::from("install"),
            OsString::from("--no-bin"),
            OsString::from("--install-dir"),
            install_dir_arg,
            OsString::from(PYTHON_VERSION),
        ],
        cancellation,
    )
    .await?;

    let interpreter = find_managed_python(
        &uv,
        runtime_root,
        &python_install_dir,
        &cache_dir,
        cancellation,
    )
    .await?;
    if !interpreter.starts_with(&python_install_dir) {
        return Err(runtime_install_error());
    }

    run_uv(
        &uv,
        runtime_root,
        &python_install_dir,
        &cache_dir,
        [
            OsString::from("sync"),
            OsString::from("--locked"),
            OsString::from("--no-install-project"),
            OsString::from("--no-build"),
            OsString::from("--python"),
            interpreter.as_os_str().to_owned(),
            OsString::from("--project"),
            project_root.as_os_str().to_owned(),
        ],
        cancellation,
    )
    .await?;

    require_regular_file(&venv_python)?;
    run_uv(
        &uv,
        runtime_root,
        &python_install_dir,
        &cache_dir,
        [
            OsString::from("pip"),
            OsString::from("install"),
            OsString::from("--python"),
            venv_python.as_os_str().to_owned(),
            OsString::from("--no-deps"),
            wheel.as_os_str().to_owned(),
        ],
        cancellation,
    )
    .await?;

    verify_exllama_imports(&venv_python, runtime_root, cancellation).await?;
    fs::remove_file(&wheel)
        .await
        .map_err(|_| runtime_install_error())?;
    if fs::try_exists(&cache_dir)
        .await
        .map_err(|_| runtime_install_error())?
    {
        fs::remove_dir_all(&cache_dir)
            .await
            .map_err(|_| runtime_install_error())?;
    }
    Ok(())
}

fn exllama_runtime_files(variant: &EngineVariant) -> Result<ExllamaRuntimeFiles, ServiceError> {
    match variant.variant_id.as_str() {
        "windows-x86_64-cuda12.8-python3.12-torch2.9" => Ok(ExllamaRuntimeFiles {
            project: include_bytes!(
                "../../resources/local-engines/python/exllama-windows-cuda128/pyproject.toml"
            ),
            lock: include_bytes!(
                "../../resources/local-engines/python/exllama-windows-cuda128/uv.lock"
            ),
            uv_executable: "uv.exe",
            wheel_name: "exllamav3-1.5.2+cu128.torch2.9.0-cp312-cp312-win_amd64.whl",
            python_executable: "Scripts/python.exe",
        }),
        "linux-x86_64-cuda12.8-python3.12-torch2.9" => Ok(ExllamaRuntimeFiles {
            project: include_bytes!(
                "../../resources/local-engines/python/exllama-linux-cuda128/pyproject.toml"
            ),
            lock: include_bytes!(
                "../../resources/local-engines/python/exllama-linux-cuda128/uv.lock"
            ),
            uv_executable: "uv-x86_64-unknown-linux-gnu/uv",
            wheel_name: "exllamav3-1.5.2+cu128.torch2.9.0-cp312-cp312-linux_x86_64.whl",
            python_executable: "bin/python",
        }),
        _ => Err(runtime_install_error()),
    }
}

async fn verify_exllama_imports(
    python: &Path,
    runtime_root: &Path,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let code = format!(
        "import importlib.metadata as m; import torch; from exllamav3.ext import exllamav3_ext; assert m.version('exllamav3') == '{EXLLAMA_VERSION}'; assert torch.cuda.is_available()"
    );
    let mut command = Command::new(python);
    command
        .args([OsStr::new("-c"), OsStr::new(&code)])
        .current_dir(runtime_root.join("tabbyAPI"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    run_command(&mut command, cancellation).await
}

async fn find_managed_python(
    uv: &Path,
    runtime_root: &Path,
    python_install_dir: &Path,
    cache_dir: &Path,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<PathBuf, ServiceError> {
    check_cancelled(cancellation)?;
    let mut command = uv_command(uv, runtime_root, python_install_dir, cache_dir);
    command
        .args([
            OsStr::new("--managed-python"),
            OsStr::new("python"),
            OsStr::new("find"),
            OsStr::new(PYTHON_VERSION),
            OsStr::new("--no-project"),
        ])
        .stdout(Stdio::piped());
    let output = tokio::select! {
        result = timeout(Duration::from_secs(30), command.output()) => {
            result
                .map_err(|_| runtime_install_error())?
                .map_err(|_| runtime_install_error())?
        }
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(cancelled_error());
        }
    };
    if !output.status.success() || output.stdout.len() > 4096 {
        return Err(runtime_install_error());
    }
    let value = std::str::from_utf8(&output.stdout)
        .map_err(|_| runtime_install_error())?
        .trim();
    let interpreter = PathBuf::from(value);
    require_regular_file(&interpreter)?;
    Ok(interpreter)
}

async fn run_uv<I, S>(
    uv: &Path,
    runtime_root: &Path,
    python_install_dir: &Path,
    cache_dir: &Path,
    arguments: I,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = uv_command(uv, runtime_root, python_install_dir, cache_dir);
    command
        .arg("--no-config")
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    run_command(&mut command, cancellation).await
}

fn uv_command<'a>(
    uv: &'a Path,
    runtime_root: &'a Path,
    python_install_dir: &'a Path,
    cache_dir: &'a Path,
) -> Command {
    let mut command = Command::new(uv);
    command.current_dir(runtime_root).kill_on_drop(true);
    for (key, _) in std::env::vars_os() {
        let normalized = key.to_string_lossy().to_ascii_uppercase();
        if normalized.starts_with("UV_") || normalized.starts_with("PIP_") {
            command.env_remove(key);
        }
    }
    command
        .env("UV_PYTHON_INSTALL_DIR", python_install_dir)
        .env("UV_CACHE_DIR", cache_dir);
    command
}

async fn run_command(
    command: &mut Command,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    check_cancelled(cancellation)?;
    let mut child = command.spawn().map_err(|_| runtime_install_error())?;
    let status = tokio::select! {
        result = child.wait() => result.map_err(|_| runtime_install_error())?,
        changed = cancellation.changed() => {
            let _ = changed;
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(cancelled_error());
        }
    };
    if status.success() {
        Ok(())
    } else {
        Err(runtime_install_error())
    }
}

async fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), ServiceError> {
    let mut file = fs::File::create(path)
        .await
        .map_err(|_| runtime_install_error())?;
    use tokio::io::AsyncWriteExt;
    file.write_all(bytes)
        .await
        .map_err(|_| runtime_install_error())?;
    file.sync_all().await.map_err(|_| runtime_install_error())
}

fn require_regular_file(path: &Path) -> Result<(), ServiceError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        _ => Err(runtime_install_error()),
    }
}

fn check_cancelled(cancellation: &watch::Receiver<bool>) -> Result<(), ServiceError> {
    if *cancellation.borrow() || cancellation.has_changed().is_err() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "The local engine installation was cancelled.",
        false,
    )
}

fn runtime_install_error() -> ServiceError {
    ServiceError::new(
        "local_engine_runtime_install_failed",
        "The pinned Python runtime could not be installed or verified.",
        true,
    )
}
