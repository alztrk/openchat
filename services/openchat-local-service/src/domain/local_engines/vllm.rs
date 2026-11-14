//! Opt-in vLLM runtime installation for native Linux and Windows WSL 2.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, process::Command, sync::watch, time::timeout};

use crate::{
    local_engines::installer::InstalledEngine,
    protocol::{EventSink, Response, ServiceError},
    storage::AppStorage,
};

const RELEASE_TAG: &str = "v0.33.0";
const PACKAGE_VERSION: &str = "0.33.0";
const VARIANT_ID: &str = "cuda130";
const WHEEL_URL: &str = "https://files.pythonhosted.org/packages/8b/a8/92e278093df40fde4c4bfd6572704bb8f398d6a2280d8c82cb5efaf12047/vllm-0.33.0-cp38-abi3-manylinux_2_28_x86_64.whl";
const OPERATION_TIMEOUT: Duration = Duration::from_secs(7200);
const PROGRESS_EVENT: &str = "local.engines.install.progress";
const MARKER_NAME: &str = ".openchat-vllm.json";
const AUTH_MODULE_NAME: &str = "openchat_vllm_auth.py";

const AUTH_MODULE: &str = r#"import hmac
import os
from starlette.responses import JSONResponse

class OpenChatAuthMiddleware:
    def __init__(self, app):
        self.app = app

    async def __call__(self, scope, receive, send):
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return
        token_path = os.environ.get("OPENCHAT_VLLM_AUTH_FILE")
        try:
            with open(token_path, "r", encoding="utf-8") as token_file:
                expected = token_file.read().strip()
        except (OSError, TypeError):
            expected = ""
        headers = dict(scope.get("headers", []))
        authorization = headers.get(b"authorization", b"").decode("latin-1")
        supplied = authorization.removeprefix("Bearer ")
        if not expected or not hmac.compare_digest(supplied, expected):
            response = JSONResponse({"error": {"message": "Unauthorized", "type": "authentication_error"}}, status_code=401)
            await response(scope, receive, send)
            return
        await self.app(scope, receive, send)
"#;
const REBASE_VENV_SCRIPTS: &str = "import pathlib,sys; root=pathlib.Path(sys.argv[1]); bindir=root/'.venv'/'bin'; files=[p for p in bindir.iterdir() if p.is_file() and not p.is_symlink()]; marker=b'/.vllm-staging-'; old=None\nfor p in files:\n data=p.read_bytes(); line=data.splitlines()[0] if data else b''\n if line.startswith(b'#!') and marker in line and b'/.venv/' in line:\n  end=line.find(b'/.venv/',2); old=line[2:end] if end>2 else None; break\nif old:\n new=str(root).encode()\n for p in files:\n  data=p.read_bytes(); updated=data.replace(old,new)\n  if updated!=data: p.write_bytes(updated)";

#[derive(Clone)]
struct RuntimePaths {
    root: PathBuf,
    venv: PathBuf,
    marker: PathBuf,
    auth_module: PathBuf,
    wsl: bool,
    host_marker: Option<PathBuf>,
    distro: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HostMarker {
    engine_id: String,
    release_tag: String,
    distro: String,
    root: String,
}

pub(crate) fn runtime_child_path(root: &Path, child: &str, wsl: bool) -> PathBuf {
    if wsl {
        PathBuf::from(format!(
            "{}/{}",
            root.to_string_lossy().trim_end_matches('/'),
            child.trim_start_matches('/')
        ))
    } else {
        root.join(child)
    }
}

pub(crate) async fn engine_json(
    storage: &AppStorage,
    model_directory: &str,
    runtime_status: &'static str,
) -> Result<Value, ServiceError> {
    // Catalog refreshes must not start a stopped WSL distribution. Compatibility
    // is checked on the Windows host; Python is checked only after install starts.
    let paths = if cfg!(windows) {
        None
    } else {
        Some(runtime_paths(storage).await?)
    };
    let installed = if let Some(paths) = paths.as_ref() {
        is_installed(paths).await
    } else {
        windows_host_marker(storage).is_some()
    };
    let installed_release_tag = if let Some(paths) = paths.as_ref() {
        installed_release(paths).await
    } else {
        windows_host_marker(storage).map(|marker| marker.release_tag)
    };
    let compatibility = if let Some(paths) = paths.as_ref() {
        compatibility_status(paths).await
    } else {
        windows_compatibility_status().await
    };
    let (os, host_os, status) = if cfg!(windows) {
        ("linux", "windows", compatibility)
    } else {
        ("linux", "linux", compatibility)
    };
    let is_compatible = status.is_none();
    Ok(json!({
        "engineId": "vllm",
        "displayName": "vLLM",
        "releaseTag": RELEASE_TAG,
        "latestUpstreamReleaseTag": RELEASE_TAG,
        "releaseCheckStatus": "current",
        "channel": "stable",
        "catalogStatus": "installable",
        "statusReason": null,
        "modelDirectory": model_directory,
        "runtimeStatus": runtime_status,
        "variants": [{
            "variantId": if host_os == "windows" { format!("windows-wsl2-{VARIANT_ID}") } else { format!("linux-x86_64-{VARIANT_ID}") },
            "os": host_os,
            "runtimeOs": os,
            "architecture": "x86_64",
            "accelerator": "cuda",
            "runtimeRequirements": ["NVIDIA GPU with compute capability 7.5 or newer", "NVIDIA driver 580 or newer", "Python 3.11–3.14", "glibc 2.28 or newer", "CUDA 13.0 compatible Linux distribution"],
            "canInstall": is_compatible && (!installed || installed_release_tag.as_deref() != Some(RELEASE_TAG)),
            "recommended": is_compatible,
            "installed": installed,
            "installedReleaseTag": installed_release_tag,
            "status": catalog_variant_status(installed, status),
        }]
    }))
}

fn catalog_variant_status(installed: bool, reason: Option<&str>) -> &'static str {
    if installed {
        return "installed";
    }
    match reason {
        Some("python_unavailable") => "python_unavailable",
        Some("python_unsupported") => "python_unsupported",
        Some("wsl_unavailable") => "wsl_unavailable",
        Some("wsl_distribution_unavailable") => "wsl_distribution_unavailable",
        Some("wsl2_required") => "wsl2_required",
        Some("gpu_unavailable") => "gpu_unavailable",
        Some("gpu_unsupported") => "gpu_unsupported",
        Some("driver_unsupported") => "driver_unsupported",
        Some("driver_version_unavailable") => "driver_version_unavailable",
        Some("unsupported_platform") => "unsupported_platform",
        Some("runtime_unavailable") => "hardware_unavailable",
        _ => "available",
    }
}

pub(crate) async fn install(
    storage: &AppStorage,
    variant_id: &str,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let expected_variant = if cfg!(windows) {
        format!("windows-wsl2-{VARIANT_ID}")
    } else {
        format!("linux-x86_64-{VARIANT_ID}")
    };
    if variant_id != expected_variant {
        return Err(unsupported_variant_error());
    }
    let paths = runtime_paths(storage).await?;
    if let Some(reason) = compatibility_status(&paths).await {
        return Err(compatibility_error(reason));
    }
    if is_installed(&paths).await && installed_release(&paths).await.as_deref() == Some(RELEASE_TAG)
    {
        rebase_venv_scripts(&paths, cancellation).await?;
        return Ok(
            json!({"installed": true, "engineId": "vllm", "variantId": variant_id, "releaseTag": RELEASE_TAG}),
        );
    }
    check_cancelled(cancellation)?;

    let staging_name = format!(".vllm-staging-{}", uuid::Uuid::new_v4());
    let staging_root = if paths.wsl {
        let root = paths.root.to_string_lossy();
        let parent = root
            .strip_suffix(&format!("/{PACKAGE_VERSION}"))
            .ok_or_else(installation_invalid_error)?;
        PathBuf::from(format!("{parent}/{staging_name}"))
    } else {
        paths.root.with_file_name(staging_name)
    };
    let staging_paths = RuntimePaths {
        root: staging_root.clone(),
        venv: runtime_child_path(&staging_root, ".venv", paths.wsl),
        marker: runtime_child_path(&staging_root, MARKER_NAME, paths.wsl),
        auth_module: runtime_child_path(&staging_root, AUTH_MODULE_NAME, paths.wsl),
        wsl: paths.wsl,
        host_marker: None,
        distro: paths.distro.clone(),
    };
    let target = staging_paths.venv.clone();
    let temporary_directory = runtime_child_path(&staging_paths.root, ".tmp", paths.wsl);
    let target_string = target.to_str().ok_or_else(storage_error)?.to_owned();
    let temporary_directory_string = temporary_directory
        .to_str()
        .ok_or_else(storage_error)?
        .to_owned();
    let root_string = staging_paths
        .root
        .to_str()
        .ok_or_else(storage_error)?
        .to_owned();
    let progress = |phase: &str, file_name: &str| {
        Response::event(
            request_id.clone(),
            PROGRESS_EVENT,
            json!({
                "engineId": "vllm",
                "variantId": variant_id,
                "phase": phase,
                "assetName": file_name,
                "assetIndex": 0,
                "assetCount": 1,
                "downloadedBytes": 0,
                "totalBytes": 0,
            }),
        )
    };
    events
        .send(&progress("installing", "Python environment"))
        .await
        .map_err(|_| install_error())?;

    let mut published = false;
    let result = async {
        events
            .send(&progress("installing", "Preparing managed directories"))
            .await
            .map_err(|_| install_error())?;
        if paths.wsl {
            run_wsl_in(paths.distro.as_deref(),
                &[
                    "python3", "-c",
                    "import pathlib,sys; p=pathlib.Path(sys.argv[1]); p.mkdir(parents=True, exist_ok=False)",
                    &root_string,
                ], cancellation,
            ).await?;
            run_wsl_in(paths.distro.as_deref(),
                &[
                    "python3", "-c",
                    "import pathlib,sys; pathlib.Path(sys.argv[1]).mkdir()",
                    &temporary_directory_string,
                ], cancellation,
            ).await?;
            events
                .send(&progress("installing", "Preparing isolated Python environment"))
                .await
                .map_err(|_| install_error())?;
            events
                .send(&progress("installing", "Installing vLLM and CUDA dependencies"))
                .await
                .map_err(|_| install_error())?;
            run_wsl_in(paths.distro.as_deref(),
                &[
                    "python3", "-m", "venv", &target_string,
                ], cancellation,
            ).await?;
            run_wsl_in(paths.distro.as_deref(),
                &[
                    "env", &format!("TMPDIR={temporary_directory_string}"),
                    &runtime_child_path(&target, "bin/python", paths.wsl).to_string_lossy(), "-m", "pip", "install",
                    "--disable-pip-version-check", "--no-cache-dir", "--only-binary=:all:",
                    WHEEL_URL,
                    "--extra-index-url", "https://download.pytorch.org/whl/cu130",
                ], cancellation,
            ).await?;
        } else {
            let parent = staging_paths.root.parent().ok_or_else(storage_error)?;
            fs::create_dir_all(parent).map_err(|_| storage_error())?;
            fs::create_dir(&staging_paths.root).map_err(|_| installation_invalid_error())?;
            fs::create_dir(&temporary_directory).map_err(|_| storage_error())?;
            run_native(
                "python3",
                &["-m", "venv", &target_string],
                cancellation,
            ).await?;
            events
                .send(&progress("installing", "Preparing isolated Python environment"))
                .await
                .map_err(|_| install_error())?;
            events
                .send(&progress("installing", "Installing vLLM and CUDA dependencies"))
                .await
                .map_err(|_| install_error())?;
            run_native(
                "env",
                &[
                    &format!("TMPDIR={temporary_directory_string}"),
                    &runtime_child_path(&target, "bin/python", false).to_string_lossy(),
                    "-m",
                    "pip",
                    "install",
                    "--disable-pip-version-check",
                    "--no-cache-dir",
                    "--only-binary=:all:",
                    WHEEL_URL,
                    "--extra-index-url",
                    "https://download.pytorch.org/whl/cu130",
                ],
                cancellation,
            ).await?;
        }
        if paths.wsl {
            run_wsl_in(
                paths.distro.as_deref(),
                &[
                    "python3",
                    "-c",
                    "import pathlib,shutil,sys; shutil.rmtree(pathlib.Path(sys.argv[1]))",
                    &temporary_directory_string,
                ],
                cancellation,
            )
            .await?;
        } else {
            fs::remove_dir_all(&temporary_directory).map_err(|_| storage_error())?;
        }
        write_runtime_files(&staging_paths, cancellation).await?;
        events
            .send(&progress("verifying", "Verifying vLLM installation"))
            .await
            .map_err(|_| install_error())?;
        let python = runtime_child_path(&target, "bin/python", paths.wsl);
        let version_output = if paths.wsl {
            capture_wsl_in(
                paths.distro.as_deref(),
                &[
                    &python.to_string_lossy(),
                    "-c",
                    "import importlib.metadata,sys; sys.exit(0 if importlib.metadata.version('vllm') == sys.argv[1] else 1)",
                    PACKAGE_VERSION,
                ],
                cancellation,
            )
            .await?
        } else {
            capture_native(
                &python.to_string_lossy(),
                &[
                    "-c",
                    "import importlib.metadata,sys; sys.exit(0 if importlib.metadata.version('vllm') == sys.argv[1] else 1)",
                    PACKAGE_VERSION,
                ],
                cancellation,
            )
            .await?
        };
        drop(version_output);
        events
            .send(&progress("publishing", "vLLM runtime"))
            .await
            .map_err(|_| install_error())?;
        publish_staging(&staging_paths, &paths, cancellation).await?;
        published = true;
        rebase_venv_scripts(&paths, cancellation).await?;
        write_marker(&paths, cancellation).await?;
        Ok(())
    }.await;

    if let Err(error) = result {
        if published {
            remove_unpublished_runtime(&paths).await?;
        } else {
            remove_staging(&staging_paths).await?;
        }
        return Err(error);
    }
    Ok(
        json!({"installed": true, "engineId": "vllm", "variantId": variant_id, "releaseTag": RELEASE_TAG}),
    )
}

pub(crate) async fn uninstall(storage: &AppStorage) -> Result<Value, ServiceError> {
    let paths = runtime_paths(storage).await?;
    if !is_installed(&paths).await {
        return Ok(json!({"removed": false, "engineId": "vllm"}));
    }
    remove_managed_runtime(&paths).await?;
    Ok(json!({"removed": true, "engineId": "vllm"}))
}

pub(crate) fn installed_engine(
    storage: &AppStorage,
) -> Result<Option<InstalledEngine>, ServiceError> {
    #[cfg(windows)]
    {
        let Some(marker) = windows_host_marker(storage) else {
            return Ok(None);
        };
        if !valid_managed_root(&marker.root) || !valid_distro_name(&marker.distro) {
            return Ok(None);
        }
        let root = PathBuf::from(&marker.root);
        return Ok(Some(InstalledEngine {
            engine_id: "vllm".to_owned(),
            variant_id: format!("windows-wsl2-{VARIANT_ID}"),
            release_tag: marker.release_tag,
            root,
            entrypoint: PathBuf::from("wsl.exe"),
        }));
    }
    #[cfg(target_os = "linux")]
    {
        let paths = runtime_paths_sync(storage)?;
        if cached_release(&paths).as_deref() != Some(RELEASE_TAG) {
            return Ok(None);
        }
        return Ok(Some(InstalledEngine {
            engine_id: "vllm".to_owned(),
            variant_id: format!("linux-x86_64-{VARIANT_ID}"),
            release_tag: RELEASE_TAG.to_owned(),
            entrypoint: paths.venv.join("bin/vllm"),
            root: paths.root,
        }));
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = storage;
        Ok(None)
    }
}

pub(crate) struct LaunchContext {
    pub(crate) root: PathBuf,
    pub(crate) distro: Option<String>,
}

pub(crate) fn launch_context(storage: &AppStorage) -> Result<LaunchContext, ServiceError> {
    #[cfg(windows)]
    {
        let marker = windows_host_marker(storage).ok_or_else(engine_unavailable_error)?;
        if !valid_managed_root(&marker.root) || !valid_distro_name(&marker.distro) {
            return Err(installation_invalid_error());
        }
        return Ok(LaunchContext {
            root: PathBuf::from(marker.root),
            distro: Some(marker.distro),
        });
    }
    #[cfg(target_os = "linux")]
    {
        let paths = runtime_paths_sync(storage)?;
        if cached_release(&paths).as_deref() != Some(RELEASE_TAG) {
            return Err(engine_unavailable_error());
        }
        return Ok(LaunchContext {
            root: paths.root,
            distro: None,
        });
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = storage;
        Err(compatibility_error("unsupported_platform"))
    }
}

pub(crate) async fn model_path_for_runtime(
    context: &LaunchContext,
    path: &Path,
) -> Result<String, ServiceError> {
    let path = path.to_str().ok_or_else(model_unavailable_error)?;
    if let Some(distro) = context.distro.as_deref() {
        let output = capture_wsl_in(
            Some(distro),
            &["wslpath", "-a", path],
            &mut watch::channel(false).1,
        )
        .await?;
        return String::from_utf8(output)
            .map(|value| value.trim().to_owned())
            .map_err(|_| model_unavailable_error());
    }
    Ok(path.to_owned())
}

pub(crate) async fn write_auth_file(
    context: &LaunchContext,
    token: &str,
) -> Result<PathBuf, ServiceError> {
    let path = runtime_child_path(
        &context.root,
        &format!(".session-{}.token", uuid::Uuid::new_v4()),
        context.distro.is_some(),
    );
    if let Some(distro) = context.distro.as_deref() {
        let paths = RuntimePaths {
            root: context.root.clone(),
            venv: runtime_child_path(&context.root, ".venv", true),
            marker: runtime_child_path(&context.root, MARKER_NAME, true),
            auth_module: runtime_child_path(&context.root, AUTH_MODULE_NAME, true),
            wsl: true,
            host_marker: None,
            distro: Some(distro.to_owned()),
        };
        write_wsl_file(
            &paths,
            &path,
            token.as_bytes(),
            &mut watch::channel(false).1,
        )
        .await?;
    } else {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::write(&path, token).map_err(|_| storage_error())?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .map_err(|_| storage_error())?;
        }
        #[cfg(not(target_os = "linux"))]
        return Err(compatibility_error("unsupported_platform"));
    }
    Ok(path)
}

pub(crate) async fn remove_auth_file(
    storage: &AppStorage,
    path: &Path,
) -> Result<(), ServiceError> {
    let context = launch_context(storage)?;
    if !path.starts_with(&context.root) {
        return Err(installation_invalid_error());
    }
    if let Some(distro) = context.distro.as_deref() {
        let path = path.to_str().ok_or_else(storage_error)?;
        run_wsl_in(
            Some(distro),
            &[
                "python3",
                "-c",
                "import pathlib,sys; pathlib.Path(sys.argv[1]).unlink(missing_ok=True)",
                path,
            ],
            &mut watch::channel(false).1,
        )
        .await
    } else {
        fs::remove_file(path).map_err(|_| storage_error())
    }
}

pub(crate) async fn stop_wsl_process(
    storage: &AppStorage,
    pid_file: &Path,
) -> Result<(), ServiceError> {
    let context = launch_context(storage)?;
    let Some(distro) = context.distro.as_deref() else {
        return Ok(());
    };
    let executable = runtime_child_path(&context.root, ".venv/bin/vllm", true);
    if !pid_file.starts_with(&context.root) {
        return Err(installation_invalid_error());
    }
    let pid_file = pid_file.to_str().ok_or_else(storage_error)?;
    let executable = executable.to_str().ok_or_else(storage_error)?;
    let script = "import os,pathlib,signal,sys,time; p=pathlib.Path(sys.argv[1]); exe=os.fsencode(sys.argv[2]); raw=p.read_text().strip() if p.exists() else ''; pid=int(raw) if raw.isdecimal() else 0; proc=pathlib.Path('/proc')/str(pid)/'cmdline'; args=proc.read_bytes().split(b'\\0') if pid and proc.exists() else []; owned=bool(exe in args); os.kill(pid,signal.SIGTERM) if owned else None; deadline=time.monotonic()+5;\nwhile owned and time.monotonic()<deadline and proc.exists():\n time.sleep(0.1); args=proc.read_bytes().split(b'\\0') if proc.exists() else []; owned=exe in args\nif owned and proc.exists(): os.kill(pid,signal.SIGKILL)\np.unlink(missing_ok=True)";
    run_wsl_in(
        Some(distro),
        &["python3", "-c", script, pid_file, executable],
        &mut watch::channel(false).1,
    )
    .await
}

#[cfg(windows)]
fn valid_managed_root(root: &str) -> bool {
    (root.starts_with("/home/") || root.starts_with("/root/"))
        && root.ends_with(&format!(
            "/.local/share/openchat/runtimes/vllm/{PACKAGE_VERSION}"
        ))
        && !root.contains('\\')
        && !root.split('/').any(|part| part == "..")
}

fn valid_distro_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._- ".contains(&byte))
}

fn parse_wsl_default_distro(listing: &str) -> Option<(String, String)> {
    let default = listing
        .lines()
        .find(|line| line.trim_start().starts_with('*'))?
        .trim_start()
        .strip_prefix('*')?
        .trim();
    let fields = default.split_whitespace().collect::<Vec<_>>();
    if fields.len() < 3 {
        return None;
    }
    let version = fields.last()?.to_string();
    let name = fields[..fields.len() - 2].join(" ");
    valid_distro_name(&name).then_some((name, version))
}

#[cfg(target_os = "linux")]
fn runtime_paths_sync(storage: &AppStorage) -> Result<RuntimePaths, ServiceError> {
    let root = storage
        .root()
        .join("runtimes/vllm/linux-x86_64")
        .join(PACKAGE_VERSION);
    Ok(RuntimePaths {
        venv: root.join(".venv"),
        marker: root.join(MARKER_NAME),
        auth_module: root.join(AUTH_MODULE_NAME),
        root,
        wsl: false,
        host_marker: None,
        distro: None,
    })
}

async fn runtime_paths(storage: &AppStorage) -> Result<RuntimePaths, ServiceError> {
    #[cfg(windows)]
    {
        if let Some(marker) = windows_host_marker(storage) {
            if !valid_managed_root(&marker.root) || !valid_distro_name(&marker.distro) {
                return Err(installation_invalid_error());
            }
            let root = PathBuf::from(marker.root);
            return Ok(RuntimePaths {
                venv: runtime_child_path(&root, ".venv", true),
                marker: runtime_child_path(&root, MARKER_NAME, true),
                auth_module: runtime_child_path(&root, AUTH_MODULE_NAME, true),
                host_marker: Some(storage.root().join("settings/vllm-runtime.json")),
                distro: Some(marker.distro),
                root,
                wsl: true,
            });
        }
        let listing = capture_host("wsl.exe", &["--list", "--verbose"]).await?;
        let listing = decode_windows_process_output(&listing);
        let (distro, version) = parse_wsl_default_distro(&listing)
            .ok_or_else(|| compatibility_error("wsl_distribution_unavailable"))?;
        if version != "2" {
            return Err(compatibility_error("wsl2_required"));
        }
        let home = capture_wsl_in(
            Some(&distro),
            &["printenv", "HOME"],
            &mut watch::channel(false).1,
        )
        .await?;
        let home = String::from_utf8(home).map_err(|_| compatibility_error("wsl_unavailable"))?;
        let home = home.trim();
        if !home.starts_with('/') || home.contains('\\') || home.split('/').any(|part| part == "..")
        {
            return Err(compatibility_error("wsl_unavailable"));
        }
        let root = PathBuf::from(format!(
            "{}/.local/share/openchat/runtimes/vllm/{PACKAGE_VERSION}",
            home.trim_end_matches('/')
        ));
        return Ok(RuntimePaths {
            venv: runtime_child_path(&root, ".venv", true),
            marker: runtime_child_path(&root, MARKER_NAME, true),
            auth_module: runtime_child_path(&root, AUTH_MODULE_NAME, true),
            host_marker: Some(storage.root().join("settings/vllm-runtime.json")),
            distro: Some(distro),
            root,
            wsl: true,
        });
    }
    #[cfg(target_os = "linux")]
    {
        let root = storage
            .root()
            .join("runtimes/vllm/linux-x86_64")
            .join(PACKAGE_VERSION);
        return Ok(RuntimePaths {
            venv: root.join(".venv"),
            marker: root.join(MARKER_NAME),
            auth_module: root.join(AUTH_MODULE_NAME),
            host_marker: None,
            distro: None,
            root,
            wsl: false,
        });
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = storage;
    #[allow(unreachable_code)]
    Err(compatibility_error("unsupported_platform"))
}

async fn compatibility_status(paths: &RuntimePaths) -> Option<&'static str> {
    if !paths.wsl && !cfg!(target_os = "linux") {
        return Some("unsupported_platform");
    }
    if paths.wsl {
        let python = match capture_wsl_in(
            paths.distro.as_deref(),
            &["python3", "--version"],
            &mut watch::channel(false).1,
        )
        .await
        {
            Ok(python) => python,
            Err(_) => return Some("python_unavailable"),
        };
        let version = String::from_utf8_lossy(&python);
        if !python_supported(&version) {
            return Some("python_unsupported");
        }
        let glibc = match capture_wsl_in(
            paths.distro.as_deref(),
            &["getconf", "GNU_LIBC_VERSION"],
            &mut watch::channel(false).1,
        )
        .await
        {
            Ok(glibc) => glibc,
            Err(_) => return Some("unsupported_platform"),
        };
        if !glibc_supported(&String::from_utf8_lossy(&glibc)) {
            return Some("unsupported_platform");
        }
        return compatibility_gpu_status(paths.distro.as_deref(), true).await;
    }
    let python = match capture_native("python3", &["--version"], &mut watch::channel(false).1).await
    {
        Ok(python) => python,
        Err(_) => return Some("python_unavailable"),
    };
    if !python_supported(&String::from_utf8_lossy(&python)) {
        return Some("python_unsupported");
    }
    let glibc = match capture_native(
        "getconf",
        &["GNU_LIBC_VERSION"],
        &mut watch::channel(false).1,
    )
    .await
    {
        Ok(glibc) => glibc,
        Err(_) => return Some("unsupported_platform"),
    };
    if !glibc_supported(&String::from_utf8_lossy(&glibc)) {
        return Some("unsupported_platform");
    }
    compatibility_gpu_status(None, false).await
}

async fn compatibility_gpu_status(distro: Option<&str>, wsl: bool) -> Option<&'static str> {
    let args = [
        "--query-gpu=driver_version,compute_cap",
        "--format=csv,noheader,nounits",
    ];
    let output = if wsl {
        capture_wsl_in(
            distro,
            &["/usr/lib/wsl/lib/nvidia-smi", args[0], args[1]],
            &mut watch::channel(false).1,
        )
        .await
    } else {
        capture_native("nvidia-smi", &args, &mut watch::channel(false).1).await
    };
    output.map_or_else(
        |_| Some("gpu_unavailable"),
        |output| check_gpu_output(&output),
    )
}

fn cached_release(paths: &RuntimePaths) -> Option<String> {
    let path = paths.host_marker.as_ref().unwrap_or(&paths.marker);
    let metadata = fs::symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 4096 {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if paths.host_marker.is_some() {
        let marker: HostMarker = serde_json::from_slice(&bytes).ok()?;
        (marker.engine_id == "vllm" && marker.release_tag == RELEASE_TAG)
            .then_some(marker.release_tag)
    } else {
        let marker: Value = serde_json::from_slice(&bytes).ok()?;
        (marker["engineId"] == "vllm" && marker["releaseTag"] == RELEASE_TAG)
            .then(|| RELEASE_TAG.to_owned())
    }
}

fn windows_host_marker(storage: &AppStorage) -> Option<HostMarker> {
    let path = storage.root().join("settings/vllm-runtime.json");
    let metadata = fs::symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 4096 {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let marker: HostMarker = serde_json::from_slice(&bytes).ok()?;
    (marker.engine_id == "vllm" && marker.release_tag == RELEASE_TAG).then_some(marker)
}

async fn windows_compatibility_status() -> Option<&'static str> {
    let output = match capture_host("wsl.exe", &["--list", "--verbose"]).await {
        Ok(output) => output,
        Err(_) => return Some("wsl_unavailable"),
    };
    let text = decode_windows_process_output(&output);
    let Some((_, version)) = parse_wsl_default_distro(&text) else {
        return Some("wsl_distribution_unavailable");
    };
    if version != "2" {
        return Some("wsl2_required");
    }
    let gpu = match capture_host(
        "nvidia-smi.exe",
        &[
            "--query-gpu=driver_version,compute_cap",
            "--format=csv,noheader,nounits",
        ],
    )
    .await
    {
        Ok(gpu) => gpu,
        Err(_) => return Some("gpu_unavailable"),
    };
    check_gpu_output(&gpu)
}

fn decode_windows_process_output(output: &[u8]) -> String {
    let units = output
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

async fn capture_host(program: &str, args: &[&str]) -> Result<Vec<u8>, ServiceError> {
    let output = timeout(
        Duration::from_secs(10),
        Command::new(program).args(args).output(),
    )
    .await
    .map_err(|_| compatibility_error("runtime_unavailable"))?
    .map_err(|_| compatibility_error("runtime_unavailable"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(compatibility_error("runtime_unavailable"))
    }
}

fn python_supported(output: &str) -> bool {
    let version = output.split_whitespace().last().unwrap_or_default();
    let mut parts = version.split('.');
    let Some(major) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return false;
    };
    let Some(minor) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return false;
    };
    major == 3 && (11..=14).contains(&minor)
}

fn glibc_supported(output: &str) -> bool {
    let Some(version) = output.split_whitespace().last() else {
        return false;
    };
    let mut parts = version.split('.');
    let Some(major) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return false;
    };
    let Some(minor) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return false;
    };
    (major, minor) >= (2, 28)
}

fn check_gpu_output(output: &[u8]) -> Option<&'static str> {
    let Ok(output) = std::str::from_utf8(output) else {
        return Some("gpu_unavailable");
    };
    let mut devices = 0;
    for line in output.lines() {
        let mut fields = line.split(',').map(str::trim);
        let driver = fields
            .next()
            .and_then(crate::local_engines::catalog::DriverVersion::parse);
        let capability = fields.next().and_then(|value| value.parse::<f32>().ok());
        devices += 1;
        let Some(driver) = driver else {
            return Some("driver_version_unavailable");
        };
        if driver
            < (crate::local_engines::catalog::DriverVersion {
                major: 580,
                minor: 0,
                patch: 0,
            })
        {
            return Some("driver_unsupported");
        }
        let Some(capability) = capability else {
            return Some("gpu_unavailable");
        };
        if capability < 7.5 {
            return Some("gpu_unsupported");
        }
    }
    if devices == 0 {
        Some("gpu_unavailable")
    } else {
        None
    }
}

async fn is_installed(paths: &RuntimePaths) -> bool {
    installed_release(paths).await.is_some()
}

async fn installed_release(paths: &RuntimePaths) -> Option<String> {
    if paths.wsl {
        if let Some(release) = cached_release(paths) {
            return Some(release);
        }
    }
    if paths.wsl {
        let output = capture_wsl_in(
            paths.distro.as_deref(),
            &[
                "python3",
                "-c",
                "import json,sys; print(json.load(open(sys.argv[1])).get('releaseTag',''))",
                paths.marker.to_str()?,
            ],
            &mut watch::channel(false).1,
        )
        .await
        .ok()?;
        let tag = String::from_utf8(output).ok()?.trim().to_owned();
        return (tag == RELEASE_TAG).then_some(tag);
    }
    let metadata = fs::symlink_metadata(&paths.marker).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    let bytes = fs::read(&paths.marker).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    (value["engineId"] == "vllm" && value["releaseTag"] == RELEASE_TAG)
        .then(|| RELEASE_TAG.to_owned())
}

async fn write_runtime_files(
    paths: &RuntimePaths,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    if paths.wsl {
        write_wsl_file(
            paths,
            &paths.auth_module,
            AUTH_MODULE.as_bytes(),
            cancellation,
        )
        .await
    } else {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::write(&paths.auth_module, AUTH_MODULE).map_err(|_| storage_error())?;
            fs::set_permissions(&paths.auth_module, fs::Permissions::from_mode(0o600))
                .map_err(|_| storage_error())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = cancellation;
            Err(storage_error())
        }
    }
}

async fn write_marker(
    paths: &RuntimePaths,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let marker = serde_json::to_vec(
        &json!({"engineId":"vllm","releaseTag":RELEASE_TAG,"variantId":VARIANT_ID}),
    )
    .map_err(|_| storage_error())?;
    if paths.wsl {
        write_wsl_file(paths, &paths.marker, &marker, cancellation).await?;
        let host_marker = paths.host_marker.as_ref().ok_or_else(storage_error)?;
        let distro = paths.distro.as_deref().ok_or_else(storage_error)?;
        let host_value = serde_json::to_vec(&HostMarker {
            engine_id: "vllm".to_owned(),
            release_tag: RELEASE_TAG.to_owned(),
            distro: distro.to_owned(),
            root: paths.root.to_string_lossy().into_owned(),
        })
        .map_err(|_| storage_error())?;
        if let Some(parent) = host_marker.parent() {
            fs::create_dir_all(parent).map_err(|_| storage_error())?;
        }
        fs::write(host_marker, host_value).map_err(|_| storage_error())
    } else {
        fs::write(&paths.marker, marker).map_err(|_| storage_error())
    }
}

async fn write_wsl_file(
    paths: &RuntimePaths,
    path: &Path,
    contents: &[u8],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let path = path.to_str().ok_or_else(storage_error)?;
    let script = "import pathlib,sys; p=pathlib.Path(sys.argv[1]); p.parent.mkdir(parents=True,exist_ok=True); p.write_bytes(sys.stdin.buffer.read()); p.chmod(0o600)";
    let mut command = Command::new("wsl.exe");
    if let Some(distro) = paths.distro.as_deref() {
        command.args(["--distribution", distro]);
    }
    let mut child = command
        .args(["--exec", "python3", "-c", script, path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| compatibility_error("wsl_unavailable"))?;
    let mut stdin = child.stdin.take().ok_or_else(storage_error)?;
    stdin
        .write_all(contents)
        .await
        .map_err(|_| storage_error())?;
    drop(stdin);
    wait_child(&mut child, cancellation).await
}

async fn rebase_venv_scripts(
    paths: &RuntimePaths,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let root = paths.root.to_str().ok_or_else(storage_error)?;
    if paths.wsl {
        run_wsl_in(
            paths.distro.as_deref(),
            &["python3", "-c", REBASE_VENV_SCRIPTS, root],
            cancellation,
        )
        .await
    } else {
        run_native("python3", &["-c", REBASE_VENV_SCRIPTS, root], cancellation).await
    }
}

async fn remove_managed_runtime(paths: &RuntimePaths) -> Result<(), ServiceError> {
    if paths.wsl {
        if installed_release(paths).await.is_none() {
            return Err(installation_invalid_error());
        }
        let path = paths.root.to_str().ok_or_else(storage_error)?;
        let script = "import pathlib,shutil,sys; p=pathlib.Path(sys.argv[1]); marker=p/'.openchat-vllm.json'; data=__import__('json').loads(marker.read_text()); assert data.get('engineId')=='vllm' and data.get('releaseTag')==sys.argv[2]; shutil.rmtree(p)";
        run_wsl_in(
            paths.distro.as_deref(),
            &["python3", "-c", script, path, RELEASE_TAG],
            &mut watch::channel(false).1,
        )
        .await?;
        if let Some(marker) = paths.host_marker.as_ref() {
            fs::remove_file(marker).map_err(|_| storage_error())?;
        }
        Ok(())
    } else {
        if installed_release(paths).await.is_none() {
            return Err(installation_invalid_error());
        }
        fs::remove_dir_all(&paths.root).map_err(|_| storage_error())
    }
}

async fn publish_staging(
    staging: &RuntimePaths,
    destination: &RuntimePaths,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    if staging.wsl {
        let source = staging.root.to_str().ok_or_else(storage_error)?;
        let destination = destination.root.to_str().ok_or_else(storage_error)?;
        run_wsl_in(
            staging.distro.as_deref(),
            &[
                "python3",
                "-c",
                "import pathlib,sys; source=pathlib.Path(sys.argv[1]); target=pathlib.Path(sys.argv[2]); target.exists() and sys.exit(7); source.rename(target)",
                source,
                destination,
            ],
            cancellation,
        )
        .await
    } else {
        match fs::symlink_metadata(&destination.root) {
            Ok(_) => return Err(installation_invalid_error()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(storage_error()),
        }
        fs::rename(&staging.root, &destination.root).map_err(|_| storage_error())
    }
}

async fn remove_staging(paths: &RuntimePaths) -> Result<(), ServiceError> {
    if paths.wsl {
        let path = paths.root.to_str().ok_or_else(storage_error)?;
        run_wsl_in(
            paths.distro.as_deref(),
            &[
                "python3",
                "-c",
                "import pathlib,shutil,sys; p=pathlib.Path(sys.argv[1]); p.is_symlink() and sys.exit(8); p.exists() and shutil.rmtree(p)",
                path,
            ],
            &mut watch::channel(false).1,
        )
        .await
    } else {
        match fs::symlink_metadata(&paths.root) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(installation_invalid_error()),
            Ok(_) => fs::remove_dir_all(&paths.root).map_err(|_| storage_error()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(storage_error()),
        }
    }
}

async fn remove_unpublished_runtime(paths: &RuntimePaths) -> Result<(), ServiceError> {
    if paths.wsl {
        let path = paths.root.to_str().ok_or_else(storage_error)?;
        run_wsl_in(
            paths.distro.as_deref(),
            &[
                "python3",
                "-c",
                "import pathlib,shutil,sys; p=pathlib.Path(sys.argv[1]); p.is_symlink() and sys.exit(8); p.exists() and shutil.rmtree(p)",
                path,
            ],
            &mut watch::channel(false).1,
        )
        .await
    } else {
        match fs::symlink_metadata(&paths.root) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(installation_invalid_error()),
            Ok(_) => fs::remove_dir_all(&paths.root).map_err(|_| storage_error()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(storage_error()),
        }
    }
}

async fn run_native(
    program: &str,
    args: &[&str],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| install_error())?;
    wait_child(&mut child, cancellation).await
}

async fn capture_native(
    program: &str,
    args: &[&str],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Vec<u8>, ServiceError> {
    let child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| compatibility_error("runtime_unavailable"))?;
    tokio::select! {
        _ = cancellation.changed() => Err(cancelled_error()),
        result = timeout(Duration::from_secs(10), child.wait_with_output()) => {
            let output = result.map_err(|_| compatibility_error("runtime_unavailable"))?
                .map_err(|_| compatibility_error("runtime_unavailable"))?;
            if output.status.success() { Ok(output.stdout) } else { Err(compatibility_error("runtime_unavailable")) }
        }
    }
}

async fn run_wsl_in(
    distro: Option<&str>,
    args: &[&str],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let mut command = Command::new("wsl.exe");
    if let Some(distro) = distro {
        command.args(["--distribution", distro]);
    }
    let mut child = command
        .arg("--exec")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| compatibility_error("wsl_unavailable"))?;
    wait_child(&mut child, cancellation).await
}

async fn capture_wsl_in(
    distro: Option<&str>,
    args: &[&str],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Vec<u8>, ServiceError> {
    let mut command = Command::new("wsl.exe");
    if let Some(distro) = distro {
        command.args(["--distribution", distro]);
    }
    let child = command
        .arg("--exec")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| compatibility_error("wsl_unavailable"))?;
    tokio::select! {
        _ = cancellation.changed() => Err(cancelled_error()),
        result = timeout(Duration::from_secs(10), child.wait_with_output()) => {
            let output = result.map_err(|_| compatibility_error("wsl_unavailable"))?
                .map_err(|_| compatibility_error("wsl_unavailable"))?;
            if output.status.success() { Ok(output.stdout) } else { Err(compatibility_error("wsl_unavailable")) }
        }
    }
}

async fn wait_child(
    child: &mut tokio::process::Child,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    tokio::select! {
        _ = cancellation.changed() => { let _ = child.kill().await; Err(cancelled_error()) },
        result = timeout(OPERATION_TIMEOUT, child.wait()) => {
            let status = result.map_err(|_| install_error())?.map_err(|_| install_error())?;
            if status.success() { Ok(()) } else { Err(install_error()) }
        }
    }
}

fn check_cancelled(cancellation: &watch::Receiver<bool>) -> Result<(), ServiceError> {
    if *cancellation.borrow() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn unsupported_variant_error() -> ServiceError {
    ServiceError::new(
        "local_engine_variant_unavailable",
        "The selected vLLM runtime is unavailable for this platform.",
        false,
    )
}
fn engine_unavailable_error() -> ServiceError {
    ServiceError::new(
        "local_engine_not_installed",
        "The selected local inference engine is not installed.",
        false,
    )
}
fn model_unavailable_error() -> ServiceError {
    ServiceError::new(
        "local_model_unavailable",
        "The selected Transformers model directory is unavailable.",
        false,
    )
}
fn install_error() -> ServiceError {
    ServiceError::new(
        "local_engine_install_failed",
        "vLLM could not be installed. Check that Python 3.11–3.14 and a CUDA-compatible Linux environment are available, then retry.",
        true,
    )
}
fn storage_error() -> ServiceError {
    ServiceError::new(
        "local_engine_storage_unavailable",
        "The vLLM runtime could not be saved in its managed folder.",
        false,
    )
}
fn installation_invalid_error() -> ServiceError {
    ServiceError::new(
        "local_engine_installation_invalid",
        "The vLLM installation marker is missing or invalid; OpenChat left the files untouched.",
        false,
    )
}
fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "The vLLM operation was cancelled.",
        false,
    )
}
fn compatibility_error(reason: &str) -> ServiceError {
    ServiceError::new("local_engine_install_unavailable", reason, false)
}

#[cfg(test)]
mod tests {
    use super::{
        catalog_variant_status, decode_windows_process_output, glibc_supported,
        parse_wsl_default_distro, runtime_child_path,
    };
    use std::path::Path;

    #[test]
    fn reads_default_wsl_distribution_and_version_without_starting_it() {
        let listing = "  NAME                   STATE           VERSION\n* Ubuntu 22.04           Stopped         2\n  docker-desktop         Stopped         2\n";
        assert_eq!(
            parse_wsl_default_distro(listing),
            Some(("Ubuntu 22.04".to_owned(), "2".to_owned()))
        );
        assert_eq!(parse_wsl_default_distro("  NAME STATE VERSION\n"), None);
    }

    #[test]
    fn decodes_wsl_utf16_output_and_maps_compatibility_reasons() {
        let output = "* Ubuntu Stopped 2\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            decode_windows_process_output(&output),
            "* Ubuntu Stopped 2\r\n"
        );
        assert_eq!(
            catalog_variant_status(false, Some("wsl2_required")),
            "wsl2_required"
        );
        assert_eq!(
            catalog_variant_status(false, Some("gpu_unavailable")),
            "gpu_unavailable"
        );
        assert_eq!(
            catalog_variant_status(false, Some("driver_unsupported")),
            "driver_unsupported"
        );
        assert_eq!(
            catalog_variant_status(true, Some("wsl_unavailable")),
            "installed"
        );
    }

    #[test]
    fn checks_the_pinned_wheel_glibc_floor() {
        assert!(glibc_supported("glibc 2.28"));
        assert!(glibc_supported("glibc 2.43"));
        assert!(!glibc_supported("glibc 2.27"));
        assert!(!glibc_supported("musl 1.2.5"));
    }

    #[test]
    fn wsl_runtime_paths_keep_linux_separators_on_windows_hosts() {
        let root = Path::new("/home/agnes/.local/share/openchat/runtimes/vllm/0.33.0");
        let executable = runtime_child_path(root, ".venv/bin/vllm", true);
        assert_eq!(
            executable.to_str(),
            Some("/home/agnes/.local/share/openchat/runtimes/vllm/0.33.0/.venv/bin/vllm")
        );
        assert!(!executable.to_string_lossy().contains('\\'));
    }
}
