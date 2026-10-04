use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::Stdio,
    time::Duration,
};

use tokio::{fs, process::Command, sync::watch, time::timeout};

use crate::{
    local_engines::{catalog::EngineVariant, wsl},
    protocol::ServiceError,
};

const VLLM_VERSION: &str = "0.30.0";
const PYTHON_VERSION: &str = "3.12.13";
const PROJECT_RELATIVE_PATH: &str = "python/environment";
const UV_RELATIVE_PATH: &str = "uv-x86_64-unknown-linux-gnu/uv";
const WHEEL_RELATIVE_PATH: &str = "wheels/vllm-0.30.0-cp38-abi3-manylinux_2_28_x86_64.whl";

pub(crate) async fn install_vllm(
    runtime_root: &Path,
    variant: &EngineVariant,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    if !matches!(
        variant.variant_id.as_str(),
        "linux-x86_64-cuda13" | "windows-wsl2-linux-x86_64-cuda13"
    ) {
        return Err(runtime_install_error());
    }
    let use_wsl = variant.host_os.as_deref() == Some("windows");
    if use_wsl != cfg!(windows) {
        return Err(runtime_install_error());
    }

    let uv_windows = runtime_root.join(UV_RELATIVE_PATH);
    let wheel_windows = runtime_root.join(WHEEL_RELATIVE_PATH);
    let project_windows = runtime_root.join(PROJECT_RELATIVE_PATH);
    let uv = wsl::path(&uv_windows, use_wsl)
        .await
        .map_err(|_| runtime_install_error())?;
    let wheel = wsl::path(&wheel_windows, use_wsl)
        .await
        .map_err(|_| runtime_install_error())?;
    let project = wsl::path(&project_windows, use_wsl)
        .await
        .map_err(|_| runtime_install_error())?;
    let python = wsl::path(
        &runtime_root.join("python/environment/.venv/bin/python"),
        use_wsl,
    )
    .await
    .map_err(|_| runtime_install_error())?;

    require_regular_file(&uv_windows)?;
    require_regular_file(&wheel_windows)?;
    if use_wsl {
        wsl::make_executable(&uv)
            .await
            .map_err(|_| runtime_install_error())?;
    }
    fs::create_dir_all(&project_windows)
        .await
        .map_err(|_| runtime_install_error())?;
    write_bytes(
        &project_windows.join("pyproject.toml"),
        include_bytes!("../../resources/local-engines/python/vllm-linux-cuda130/pyproject.toml"),
    )
    .await?;
    write_bytes(
        &project_windows.join("uv.lock"),
        include_bytes!("../../resources/local-engines/python/vllm-linux-cuda130/uv.lock"),
    )
    .await?;

    check_cancelled(cancellation)?;
    run_uv(
        &uv,
        use_wsl,
        [
            OsString::from("--no-cache"),
            OsString::from("python"),
            OsString::from("install"),
            OsString::from("--no-bin"),
            OsString::from(PYTHON_VERSION),
        ],
        cancellation,
    )
    .await?;

    let python_install_dir = run_uv_for_output(
        &uv,
        use_wsl,
        [
            OsString::from("--no-config"),
            OsString::from("python"),
            OsString::from("dir"),
        ],
        cancellation,
    )
    .await?;
    let python_install_dir = wsl::linux_path_from_output(python_install_dir.as_bytes())
        .map_err(|_| runtime_install_error())?;
    let interpreter = run_uv_for_output(
        &uv,
        use_wsl,
        [
            OsString::from("--no-config"),
            OsString::from("--managed-python"),
            OsString::from("python"),
            OsString::from("find"),
            OsString::from(PYTHON_VERSION),
            OsString::from("--no-project"),
        ],
        cancellation,
    )
    .await?;
    let interpreter =
        wsl::linux_path_from_output(interpreter.as_bytes()).map_err(|_| runtime_install_error())?;
    if !wsl::linux_path_is_within(&interpreter, &python_install_dir) {
        return Err(runtime_install_error());
    }

    run_uv(
        &uv,
        use_wsl,
        [
            OsString::from("--no-cache"),
            OsString::from("sync"),
            OsString::from("--locked"),
            OsString::from("--no-install-project"),
            OsString::from("--no-install-package"),
            OsString::from("vllm"),
            OsString::from("--no-build"),
            OsString::from("--python"),
            OsString::from(&interpreter),
            OsString::from("--project"),
            project.as_os_str().to_owned(),
        ],
        cancellation,
    )
    .await?;

    run_uv(
        &uv,
        use_wsl,
        [
            OsString::from("--no-cache"),
            OsString::from("pip"),
            OsString::from("install"),
            OsString::from("--offline"),
            OsString::from("--no-deps"),
            OsString::from("--python"),
            python.as_os_str().to_owned(),
            wheel.into_os_string(),
        ],
        cancellation,
    )
    .await?;

    if use_wsl {
        wsl::make_executable(&python)
            .await
            .map_err(|_| runtime_install_error())?;
        let vllm_executable = wsl::path(
            &runtime_root.join("python/environment/.venv/bin/vllm"),
            true,
        )
        .await
        .map_err(|_| runtime_install_error())?;
        wsl::make_executable(&vllm_executable)
            .await
            .map_err(|_| runtime_install_error())?;
    }
    verify_vllm_imports(&python, use_wsl, cancellation).await?;

    fs::remove_file(&wheel_windows)
        .await
        .map_err(|_| runtime_install_error())?;
    Ok(())
}

async fn verify_vllm_imports(
    python: &Path,
    use_wsl: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let code = format!(
        "import importlib.metadata as m; import torch; import vllm; assert m.version('vllm') == '{VLLM_VERSION}'; assert torch.version.cuda and torch.version.cuda.startswith('13.'); assert torch.cuda.is_available(); assert torch.cuda.get_device_capability(0) >= (7, 5)"
    );
    let mut command = wsl::command(python, use_wsl).map_err(|_| runtime_install_error())?;
    command
        .args([OsStr::new("-c"), OsStr::new(&code)])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    run_command(&mut command, cancellation).await
}

async fn run_uv<I, S>(
    uv: &Path,
    use_wsl: bool,
    arguments: I,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = wsl::command(uv, use_wsl).map_err(|_| runtime_install_error())?;
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    run_command(&mut command, cancellation).await
}

async fn run_uv_for_output<I, S>(
    uv: &Path,
    use_wsl: bool,
    arguments: I,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<String, ServiceError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    check_cancelled(cancellation)?;
    let mut command = wsl::command(uv, use_wsl).map_err(|_| runtime_install_error())?;
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
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
    String::from_utf8(output.stdout).map_err(|_| runtime_install_error())
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
