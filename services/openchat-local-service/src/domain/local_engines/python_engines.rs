//! Managed ExLlamaV3/TabbyAPI and SGLang Python runtimes.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::AsyncWriteExt,
    process::{Child, Command},
    sync::watch,
    time::{Instant, timeout},
};
use zeroize::Zeroizing;

use crate::{
    local_engines::installer::InstalledEngine,
    protocol::{EventSink, Response, ServiceError},
    storage::AppStorage,
};

const EXLLAMA_RELEASE: &str = "tabbyapi-4728158-exllamav3-1.6.7";
const EXLLAMA_COMMIT: &str = "47281587fafbd69e50aaf77c7626f390811126b6";
const EXLLAMA_VARIANT: &str = "cu13";
const SGLANG_RELEASE: &str = "v0.5.23";
const SGLANG_VARIANT: &str = "cuda130";
const SGLANG_RUNTIME_REQUIREMENTS: [&str; 2] =
    ["sglang==0.5.23", "cuda-toolkit[nvcc,nvvm,cccl]==13.0.3"];
const EXLLAMA_INSTALL_VERIFICATION_SCRIPT: &str =
    include_str!("python_scripts/verify_exllama_install.py");
const SGLANG_CUDA_HEADER_PATCH_SCRIPT: &str =
    include_str!("python_scripts/patch_sglang_cuda_header.py");
const SGLANG_LAUNCHER_SCRIPT: &str = include_str!("python_scripts/launch_sglang.py");
const SGLANG_INSTALL_VERIFICATION_SCRIPT: &str =
    include_str!("python_scripts/verify_sglang_install.py");
const MANAGED_PROCESS_STOP_SCRIPT: &str = include_str!("python_scripts/stop_managed_process.py");
const PROGRESS_EVENT: &str = "local.engines.install.progress";
const INSTALL_TIMEOUT: Duration = Duration::from_secs(10_800);
const MARKER_NAME: &str = ".openchat-python-engine.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Engine {
    Exllama,
    Sglang,
}

impl Engine {
    pub(crate) fn parse(id: &str) -> Result<Self, ServiceError> {
        match id {
            "exllama" => Ok(Self::Exllama),
            "sglang" => Ok(Self::Sglang),
            _ => Err(unknown_engine_error()),
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Exllama => "exllama",
            Self::Sglang => "sglang",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Exllama => "ExLlama",
            Self::Sglang => "SGLang",
        }
    }

    fn release(self) -> &'static str {
        match self {
            Self::Exllama => EXLLAMA_RELEASE,
            Self::Sglang => SGLANG_RELEASE,
        }
    }

    fn variant(self, windows: bool) -> String {
        match (self, windows) {
            (Self::Exllama, true) => format!("windows-{EXLLAMA_VARIANT}"),
            (Self::Exllama, false) => format!("linux-x86_64-{EXLLAMA_VARIANT}"),
            (Self::Sglang, true) => format!("windows-wsl2-{SGLANG_VARIANT}"),
            (Self::Sglang, false) => format!("linux-x86_64-{SGLANG_VARIANT}"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct RuntimePaths {
    pub(crate) root: PathBuf,
    pub(crate) venv: PathBuf,
    pub(crate) marker: PathBuf,
    host_marker: Option<PathBuf>,
    pub(crate) wsl: bool,
    pub(crate) distro: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostMarker {
    engine_id: String,
    release_tag: String,
    distro: String,
    root: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeMarker {
    engine_id: String,
    release_tag: String,
    variant_id: String,
}

pub(crate) struct RuntimeSession {
    pub(crate) directory: PathBuf,
    pub(crate) auth_file: PathBuf,
    pub(crate) remote_pid_file: Option<PathBuf>,
    pub(crate) runner: PathBuf,
    pub(crate) api_key: Zeroizing<String>,
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
    engine_id: &str,
    model_directory: &str,
    runtime_status: &'static str,
    cuda_available: bool,
) -> Result<Value, ServiceError> {
    let engine = Engine::parse(engine_id)?;
    let (paths, path_reason, _) = catalog_paths(storage, engine).await;
    let installed_release_tag = installed_release_for_catalog(storage, engine).await?;
    let installed = installed_release_tag.is_some();
    let update_available = installed_release_tag
        .as_deref()
        .is_some_and(|release| release != engine.release());
    let compatibility = if std::env::consts::ARCH != "x86_64" {
        Some("unsupported_platform")
    } else if let Some(reason) = path_reason {
        Some(reason)
    } else if cfg!(windows) && engine == Engine::Sglang {
        if paths.distro.is_none() {
            Some("wsl_distribution_unavailable")
        } else if !cuda_available {
            Some("gpu_unavailable")
        } else {
            python_compatibility_in(&paths, engine).await
        }
    } else if !cuda_available {
        Some("gpu_unavailable")
    } else {
        python_compatibility(engine).await
    };
    let (host_os, runtime_os) = if cfg!(windows) && engine == Engine::Sglang {
        ("windows", "linux")
    } else if cfg!(windows) {
        ("windows", "windows")
    } else {
        ("linux", "linux")
    };
    let compatible = compatibility.is_none();
    let variant_id = engine.variant(cfg!(windows));
    let requirements = match engine {
        Engine::Exllama => vec![
            "NVIDIA GPU with CUDA 13 support".to_owned(),
            "Python 3.12–3.14".to_owned(),
            "Git".to_owned(),
        ],
        Engine::Sglang => vec![
            "NVIDIA GPU with CUDA 13 support".to_owned(),
            "Python 3.11–3.13".to_owned(),
            if cfg!(windows) {
                "WSL 2 with an NVIDIA-enabled Linux distribution".to_owned()
            } else {
                "Linux x86_64".to_owned()
            },
        ],
    };
    Ok(json!({
        "engineId": engine.id(),
        "displayName": engine.name(),
        "releaseTag": engine.release(),
        "latestUpstreamReleaseTag": engine.release(),
        "releaseCheckStatus": "current",
        "channel": if engine == Engine::Exllama { "pinned" } else { "stable" },
        "catalogStatus": "installable",
        "statusReason": null,
        "modelDirectory": model_directory,
        "runtimeStatus": runtime_status,
        "variants": [{
            "variantId": variant_id,
            "os": host_os,
            "runtimeOs": runtime_os,
            "architecture": "x86_64",
            "accelerator": "cuda",
            "runtimeRequirements": requirements,
            "canInstall": compatible && (!installed || update_available),
            "recommended": compatible,
            "installed": installed,
            "installedReleaseTag": installed_release_tag,
            "status": if update_available && compatible {
                "update_available"
            } else {
                variant_status(installed, compatibility)
            },
        }]
    }))
}

async fn catalog_paths(
    storage: &AppStorage,
    engine: Engine,
) -> (RuntimePaths, Option<&'static str>, Option<String>) {
    if cfg!(windows) && engine == Engine::Sglang {
        #[cfg(windows)]
        {
            let host_marker = storage
                .root()
                .join(format!("settings/{}-runtime.json", engine.id()));
            if let Some(marker) = read_host_marker(&host_marker, engine) {
                let paths = paths_from_host_marker(marker, host_marker);
                let release_tag = cached_release(&paths, engine);
                return (paths, None, release_tag);
            }
            let listing = match capture("wsl.exe", &["--list", "--verbose"]).await {
                Ok(output) => decode_wsl_output(&output),
                Err(_) => return (empty_wsl_paths(), Some("wsl_unavailable"), None),
            };
            let Some((distro, version)) = parse_default_distro(&listing) else {
                return (
                    empty_wsl_paths(),
                    Some("wsl_distribution_unavailable"),
                    None,
                );
            };
            if version != "2" {
                return (empty_wsl_paths(), Some("wsl2_required"), None);
            }
            return (
                RuntimePaths {
                    root: PathBuf::new(),
                    venv: PathBuf::new(),
                    marker: PathBuf::new(),
                    host_marker: Some(host_marker),
                    wsl: true,
                    distro: Some(distro),
                },
                None,
                None,
            );
        }
    }
    let paths = runtime_paths_sync(storage, engine)
        .ok()
        .flatten()
        .unwrap_or_else(empty_native_paths);
    let release_tag = cached_release(&paths, engine);
    (paths, None, release_tag)
}

#[cfg(windows)]
fn empty_wsl_paths() -> RuntimePaths {
    RuntimePaths {
        root: PathBuf::new(),
        venv: PathBuf::new(),
        marker: PathBuf::new(),
        host_marker: None,
        wsl: true,
        distro: None,
    }
}

fn empty_native_paths() -> RuntimePaths {
    RuntimePaths {
        root: PathBuf::new(),
        venv: PathBuf::new(),
        marker: PathBuf::new(),
        host_marker: None,
        wsl: false,
        distro: None,
    }
}

#[cfg(windows)]
fn paths_from_host_marker(marker: HostMarker, host_marker: PathBuf) -> RuntimePaths {
    let root = PathBuf::from(marker.root);
    let venv = runtime_child_path(&root, ".venv", true);
    let marker_path = runtime_child_path(&root, MARKER_NAME, true);
    RuntimePaths {
        root,
        venv,
        marker: marker_path,
        host_marker: Some(host_marker),
        wsl: true,
        distro: Some(marker.distro),
    }
}

fn variant_status(installed: bool, reason: Option<&str>) -> &'static str {
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
        Some("unsupported_platform") => "unsupported_platform",
        _ => "available",
    }
}

fn python_minor_version(output: &[u8]) -> Option<u8> {
    let version = std::str::from_utf8(output)
        .ok()?
        .trim()
        .strip_prefix("Python ")?;
    let mut parts = version.split('.');
    if parts.next()?.parse::<u8>().ok()? != 3 {
        return None;
    }
    parts.next()?.parse().ok()
}

async fn python_compatibility(engine: Engine) -> Option<&'static str> {
    if engine == Engine::Sglang {
        return sglang_python(None).await.err();
    }
    #[cfg(windows)]
    let output = capture(windows_python_launcher(), &["-3.12", "--version"]).await;
    #[cfg(target_os = "linux")]
    let output = capture("python3", &["--version"]).await;
    #[cfg(not(any(windows, target_os = "linux")))]
    let output: Result<Vec<u8>, ServiceError> = Err(compatibility_error("unsupported_platform"));

    let Ok(output) = output else {
        return Some("python_unavailable");
    };
    let Some(minor) = python_minor_version(&output) else {
        return Some("python_unsupported");
    };
    let minimum = if engine == Engine::Sglang { 11 } else { 12 };
    (minor < minimum || minor > 14).then_some("python_unsupported")
}

async fn python_compatibility_in(paths: &RuntimePaths, engine: Engine) -> Option<&'static str> {
    if engine == Engine::Sglang {
        return sglang_python(Some(paths)).await.err();
    }
    if !paths.wsl {
        return python_compatibility(engine).await;
    }
    let output = capture_wsl(paths.distro.as_deref(), &["python3", "--version"]).await;
    let Ok(output) = output else {
        return Some("python_unavailable");
    };
    let Some(minor) = python_minor_version(&output) else {
        return Some("python_unsupported");
    };
    (minor < 12 || minor > 14).then_some("python_unsupported")
}

async fn sglang_python(paths: Option<&RuntimePaths>) -> Result<std::ffi::OsString, &'static str> {
    const EXECUTABLES: [&str; 4] = ["python3.12", "python3.13", "python3.11", "python3"];
    let mut interpreter_found = false;

    for executable in EXECUTABLES {
        let output = if let Some(paths) = paths.filter(|paths| paths.wsl) {
            capture_wsl(paths.distro.as_deref(), &[executable, "--version"]).await
        } else {
            capture(executable, &["--version"]).await
        };
        let Ok(output) = output else {
            continue;
        };
        let Some(minor) = python_minor_version(&output) else {
            return Err("python_unsupported");
        };
        if (11..=13).contains(&minor) {
            return Ok(executable.into());
        }
        interpreter_found = true;
    }

    Err(if interpreter_found {
        "python_unsupported"
    } else {
        "python_unavailable"
    })
}

pub(crate) async fn install(
    storage: &AppStorage,
    engine_id: &str,
    variant_id: &str,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let engine = Engine::parse(engine_id)?;
    if std::env::consts::ARCH != "x86_64" {
        return Err(compatibility_error("unsupported_platform"));
    }
    if variant_id != engine.variant(cfg!(windows)) {
        return Err(unsupported_variant_error());
    }
    check_cancelled(cancellation)?;
    let previous_paths = installed_runtime_paths(storage, engine).await?;
    let updating = previous_paths
        .iter()
        .any(|paths| cached_release(paths, engine).as_deref() != Some(engine.release()));
    let paths = runtime_paths(storage, engine).await?;
    let cuda_available = super::nvidia_driver_status().await;
    if !matches!(cuda_available, super::NvidiaDriverStatus::Detected(_)) {
        return Err(compatibility_error("gpu_unavailable"));
    }
    if let Some(reason) = python_compatibility_in(&paths, engine).await {
        return Err(compatibility_error(reason));
    }
    if updating {
        super::runtime::stop_engine(storage, engine.id()).await?;
    }
    if is_installed(&paths, engine).await
        && installed_release(&paths, engine).await.as_deref() == Some(engine.release())
    {
        return Ok(
            json!({"installed": true, "engineId": engine.id(), "variantId": variant_id, "releaseTag": engine.release()}),
        );
    }
    check_cancelled(cancellation)?;
    if !paths.wsl {
        match fs::symlink_metadata(&paths.root) {
            Ok(_) => return Err(installation_invalid_error()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(storage_error()),
        }
        fs::create_dir_all(paths.root.parent().ok_or_else(storage_error)?)
            .map_err(|_| storage_error())?;
        fs::create_dir(&paths.root).map_err(|_| storage_error())?;
    } else {
        let parent = paths.root.parent().ok_or_else(storage_error)?;
        create_wsl_directory(&paths, parent, true, cancellation).await?;
        create_wsl_directory(&paths, &paths.root, false, cancellation).await?;
    }

    let result = async {
        send_progress(
            events,
            request_id,
            engine,
            variant_id,
            "installing",
            "Python environment",
        )
        .await?;
        install_runtime(&paths, engine, request_id, events, cancellation).await?;
        write_marker(&paths, engine, variant_id, cancellation).await
    }
    .await;
    if let Err(error) = result {
        if remove_partial_runtime(&paths).await.is_err() {
            return Err(storage_error());
        }
        return Err(error);
    }
    for previous in previous_paths {
        if previous.root != paths.root {
            remove_managed_runtime(&previous, engine).await?;
        }
    }
    Ok(
        json!({"installed": true, "engineId": engine.id(), "variantId": variant_id, "releaseTag": engine.release()}),
    )
}

async fn install_runtime(
    paths: &RuntimePaths,
    engine: Engine,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let (base_python, prefix) = if engine == Engine::Sglang {
        (
            sglang_python(Some(paths))
                .await
                .map_err(compatibility_error)?,
            &[][..],
        )
    } else {
        base_python(engine, paths.wsl)
    };
    let mut venv_arguments = prefix.to_vec();
    venv_arguments.extend(["-m".to_owned(), "venv".to_owned(), path_arg(&paths.venv)?]);
    run(paths, base_python, &venv_arguments, None, cancellation).await?;
    let python = path_arg(&python_path(paths))?;
    match engine {
        Engine::Exllama => {
            send_progress(
                events,
                request_id,
                engine,
                EXLLAMA_VARIANT,
                "installing",
                "Pinned TabbyAPI source",
            )
            .await?;
            let source = runtime_child_path(&paths.root, "tabbyAPI", paths.wsl);
            run(
                paths,
                "git",
                &["init".to_owned(), path_arg(&source)?],
                None,
                cancellation,
            )
            .await?;
            run(
                paths,
                "git",
                &[
                    "-C".to_owned(),
                    path_arg(&source)?,
                    "remote".to_owned(),
                    "add".to_owned(),
                    "origin".to_owned(),
                    "https://github.com/theroyallab/tabbyAPI.git".to_owned(),
                ],
                None,
                cancellation,
            )
            .await?;
            run(
                paths,
                "git",
                &[
                    "-C".to_owned(),
                    path_arg(&source)?,
                    "fetch".to_owned(),
                    "--depth".to_owned(),
                    "1".to_owned(),
                    "origin".to_owned(),
                    EXLLAMA_COMMIT.to_owned(),
                ],
                None,
                cancellation,
            )
            .await?;
            run(
                paths,
                "git",
                &[
                    "-C".to_owned(),
                    path_arg(&source)?,
                    "checkout".to_owned(),
                    "--detach".to_owned(),
                    "FETCH_HEAD".to_owned(),
                ],
                None,
                cancellation,
            )
            .await?;
            send_progress(
                events,
                request_id,
                engine,
                EXLLAMA_VARIANT,
                "installing",
                "TabbyAPI, ExLlamaV3 and CUDA dependencies",
            )
            .await?;
            let local_project = format!("{}[cu13]", source.to_string_lossy());
            run_pip_install(
                paths,
                &python,
                &[
                    "-m".to_owned(),
                    "pip".to_owned(),
                    "install".to_owned(),
                    "--disable-pip-version-check".to_owned(),
                    "--no-cache-dir".to_owned(),
                    "-e".to_owned(),
                    local_project,
                ],
                cancellation,
            )
            .await?;
            send_progress(
                events,
                request_id,
                engine,
                EXLLAMA_VARIANT,
                "verifying",
                "ExLlamaV3 GPU runtime",
            )
            .await?;
            run_python_script(
                paths,
                &python,
                EXLLAMA_INSTALL_VERIFICATION_SCRIPT,
                &[],
                cancellation,
            )
            .await?;
        }
        Engine::Sglang => {
            send_progress(
                events,
                request_id,
                engine,
                SGLANG_VARIANT,
                "installing",
                "SGLang and CUDA dependencies",
            )
            .await?;
            let mut install_arguments = vec![
                "-m".to_owned(),
                "pip".to_owned(),
                "install".to_owned(),
                "--pre".to_owned(),
                "--disable-pip-version-check".to_owned(),
                "--no-cache-dir".to_owned(),
            ];
            install_arguments.extend(
                SGLANG_RUNTIME_REQUIREMENTS
                    .iter()
                    .map(|requirement| (*requirement).to_owned()),
            );
            run_pip_install(paths, &python, &install_arguments, cancellation).await?;
            patch_sglang_cuda_header(paths, &python, cancellation).await?;
            send_progress(
                events,
                request_id,
                engine,
                SGLANG_VARIANT,
                "verifying",
                "SGLang GPU runtime",
            )
            .await?;
            run_python_script(
                paths,
                &python,
                SGLANG_INSTALL_VERIFICATION_SCRIPT,
                &[],
                cancellation,
            )
            .await?;
        }
    }
    Ok(())
}

pub(crate) async fn uninstall(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<Value, ServiceError> {
    let engine = Engine::parse(engine_id)?;
    let paths = installed_runtime_paths(storage, engine).await?;
    if paths.is_empty() {
        return Ok(json!({"removed": false, "engineId": engine.id()}));
    }
    let host_marker = paths.iter().find_map(|paths| paths.host_marker.clone());
    for path in paths {
        remove_managed_runtime(&path, engine).await?;
    }
    if let Some(host_marker) = host_marker {
        match fs::remove_file(host_marker) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(storage_error()),
        }
    }
    Ok(json!({"removed": true, "engineId": engine.id()}))
}

pub(crate) fn installed_engine(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<Option<InstalledEngine>, ServiceError> {
    let engine = Engine::parse(engine_id)?;
    let Some(paths) = runtime_paths_sync(storage, engine)? else {
        return Ok(None);
    };
    if cached_release(&paths, engine).as_deref() != Some(engine.release()) {
        return Ok(None);
    }
    let variant_id = engine.variant(cfg!(windows));
    let entrypoint = if paths.wsl {
        PathBuf::from("wsl.exe")
    } else {
        python_path(&paths)
    };
    Ok(Some(InstalledEngine {
        engine_id: engine.id().to_owned(),
        variant_id,
        release_tag: engine.release().to_owned(),
        root: paths.root,
        entrypoint,
    }))
}

pub(crate) fn runtime_paths_sync(
    storage: &AppStorage,
    engine: Engine,
) -> Result<Option<RuntimePaths>, ServiceError> {
    if engine == Engine::Sglang && cfg!(windows) {
        #[cfg(windows)]
        {
            let host_marker = storage
                .root()
                .join(format!("settings/{}-runtime.json", engine.id()));
            let Some(marker) = read_host_marker(&host_marker, engine) else {
                return Ok(None);
            };
            let root = PathBuf::from(marker.root);
            return Ok(Some(RuntimePaths {
                venv: PathBuf::from(format!("{}/.venv", root.display())),
                marker: PathBuf::from(format!("{}/{}", root.display(), MARKER_NAME)),
                root,
                host_marker: Some(host_marker),
                wsl: true,
                distro: Some(marker.distro),
            }));
        }
        #[cfg(not(windows))]
        return Ok(None);
    }
    #[cfg(target_os = "linux")]
    {
        let root = storage.root().join(format!(
            "runtimes/{}/linux-x86_64/{}",
            engine.id(),
            engine.release()
        ));
        Ok(Some(RuntimePaths {
            venv: root.join(".venv"),
            marker: root.join(MARKER_NAME),
            root,
            host_marker: None,
            wsl: false,
            distro: None,
        }))
    }
    #[cfg(windows)]
    {
        if engine == Engine::Exllama {
            let root = storage.root().join(format!(
                "runtimes/{}/windows-x86_64/{}",
                engine.id(),
                engine.release()
            ));
            return Ok(Some(RuntimePaths {
                venv: root.join(".venv"),
                marker: root.join(MARKER_NAME),
                root,
                host_marker: None,
                wsl: false,
                distro: None,
            }));
        }
        Ok(None)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = storage;
        Ok(None)
    }
}

pub(crate) async fn launch_context(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<RuntimePaths, ServiceError> {
    let engine = Engine::parse(engine_id)?;
    if let Some(paths) = runtime_paths_sync(storage, engine)? {
        if cached_release(&paths, engine).as_deref() != Some(engine.release()) {
            return Err(engine_unavailable_error());
        }
        return Ok(paths);
    }
    if engine == Engine::Sglang && cfg!(windows) {
        #[cfg(windows)]
        {
            let paths = runtime_paths(storage, engine).await?;
            if cached_release(&paths, engine).as_deref() != Some(engine.release()) {
                return Err(engine_unavailable_error());
            }
            return Ok(paths);
        }
    }
    Err(engine_unavailable_error())
}

pub(crate) async fn model_path_for_runtime(
    paths: &RuntimePaths,
    path: &Path,
) -> Result<String, ServiceError> {
    let path = path.to_str().ok_or_else(model_unavailable_error)?;
    if paths.wsl {
        let output = capture_wsl(paths.distro.as_deref(), &["wslpath", "-a", path]).await?;
        return String::from_utf8(output)
            .map(|value| value.trim().to_owned())
            .map_err(|_| model_unavailable_error());
    }
    Ok(path.to_owned())
}

fn exllama_max_seq_len(model_context_length: Option<u32>, engine_default: Option<u32>) -> i64 {
    model_context_length
        .or(engine_default)
        .map_or(-1, i64::from)
}

pub(crate) async fn create_session(
    paths: &RuntimePaths,
    engine: Engine,
    model: &crate::local_engines::models::RegisteredModel,
    port: u16,
    settings: &super::settings::LocalEngineSettings,
) -> Result<RuntimeSession, ServiceError> {
    let session = runtime_child_path(
        &paths.root,
        &format!(".sessions/{}", uuid::Uuid::new_v4()),
        paths.wsl,
    );
    if paths.wsl {
        let session_parent = session.parent().ok_or_else(storage_error)?;
        let mut cleanup_cancellation = watch::channel(false).1;
        create_wsl_directory(paths, session_parent, true, &mut cleanup_cancellation).await?;
        create_wsl_directory(paths, &session, false, &mut cleanup_cancellation).await?;
    } else {
        fs::create_dir_all(&session).map_err(|_| storage_error())?;
    }
    let result = async {
        let api_key = Zeroizing::new(uuid::Uuid::new_v4().simple().to_string());
        let auth_file = runtime_child_path(
            &session,
            if engine == Engine::Exllama {
                "api_tokens.yml"
            } else {
                ".api-key"
            },
            paths.wsl,
        );
        let auth_contents = if engine == Engine::Exllama {
            let admin_key = Zeroizing::new(uuid::Uuid::new_v4().simple().to_string());
            format!(
                "api_key: {}\nadmin_key: {}\n",
                api_key.as_str(),
                admin_key.as_str()
            )
        } else {
            format!("{}\n", api_key.as_str())
        };
        write_private_file(paths, &auth_file, auth_contents.as_bytes()).await?;

        let model_parent = model.path.parent().ok_or_else(model_unavailable_error)?;
        let model_name = model
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(model_unavailable_error)?;
        let model_directory = model_path_for_runtime(paths, model_parent).await?;
        let runner = if engine == Engine::Sglang {
            let runner = runtime_child_path(&session, "launch_sglang.py", paths.wsl);
            write_private_file(paths, &runner, SGLANG_LAUNCHER_SCRIPT.as_bytes()).await?;
            runner
        } else {
            runtime_child_path(&paths.root, "tabbyAPI/main.py", paths.wsl)
        };
        if engine == Engine::Exllama {
            let escaped_model_dir =
                serde_json::to_string(&model_directory).map_err(|_| storage_error())?;
            let escaped_model_name =
                serde_json::to_string(model_name).map_err(|_| storage_error())?;
            let cache_mode = settings.exllama_cache_mode.as_deref().unwrap_or("fp16");
            let max_seq_len =
                exllama_max_seq_len(model.context_length, settings.exllama_max_seq_len);
            let config_path = runtime_child_path(&session, "config.yml", paths.wsl);
            let contents = format!(
                "network:\n  host: 127.0.0.1\n  port: {port}\n  disable_auth: false\n  allowed_origins: []\n  web_ui: false\nlogging:\n  log_prompt: false\n  log_generation_params: false\n  log_requests: false\n  log_chat_completion_requests: false\nmodel:\n  model_dir: {escaped_model_dir}\n  model_name: {escaped_model_name}\n  max_seq_len: {max_seq_len}\n  cache_mode: {cache_mode}\n  max_batch_size: 1\n"
            );
            write_private_file(paths, &config_path, contents.as_bytes()).await?;
        }
        let pid_file = paths
            .wsl
            .then(|| runtime_child_path(&session, ".pid", true));
        Ok(RuntimeSession {
            directory: session.clone(),
            auth_file,
            remote_pid_file: pid_file,
            runner,
            api_key,
        })
    }
    .await;
    if result.is_err() {
        remove_session_directory(paths, &session).await?;
    }
    result
}

pub(crate) async fn remove_session(
    storage: &AppStorage,
    engine_id: &str,
    auth_file: &Path,
) -> Result<(), ServiceError> {
    let paths = launch_context(storage, engine_id).await?;
    let session = auth_file.parent().ok_or_else(installation_invalid_error)?;
    remove_session_directory(&paths, session).await
}

async fn remove_session_directory(
    paths: &RuntimePaths,
    session: &Path,
) -> Result<(), ServiceError> {
    let session_root = runtime_child_path(&paths.root, ".sessions", paths.wsl);
    if !session.starts_with(&session_root)
        || session.parent() != Some(session_root.as_path())
        || !session
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.len() == 36)
    {
        return Err(installation_invalid_error());
    }
    if paths.wsl {
        remove_wsl_directory(paths, session).await
    } else {
        let metadata = fs::symlink_metadata(session).map_err(|_| storage_error())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(installation_invalid_error());
        }
        fs::remove_dir_all(session).map_err(|_| storage_error())
    }
}

pub(crate) async fn stop_wsl_process(
    storage: &AppStorage,
    engine_id: &str,
    pid_file: &Path,
    runner: &Path,
) -> Result<(), ServiceError> {
    let paths = launch_context(storage, engine_id).await?;
    if !paths.wsl || !pid_file.starts_with(&paths.root) || !runner.starts_with(&paths.root) {
        return Err(installation_invalid_error());
    }
    let runner_string = runner.to_str().ok_or_else(storage_error)?;
    run_python_script(
        &paths,
        "python3",
        MANAGED_PROCESS_STOP_SCRIPT,
        &[
            path_arg(pid_file)?,
            runner_string.to_owned(),
            "10".to_owned(),
        ],
        &mut watch::channel(false).1,
    )
    .await
}

async fn runtime_paths(storage: &AppStorage, engine: Engine) -> Result<RuntimePaths, ServiceError> {
    if engine != Engine::Sglang || !cfg!(windows) {
        return runtime_paths_sync(storage, engine)?
            .ok_or_else(|| compatibility_error("unsupported_platform"));
    }
    #[cfg(windows)]
    {
        let host_marker = storage
            .root()
            .join(format!("settings/{}-runtime.json", engine.id()));
        if let Some(marker) = read_host_marker(&host_marker, engine) {
            let root = if marker.release_tag == engine.release() {
                PathBuf::from(&marker.root)
            } else {
                Path::new(&marker.root)
                    .parent()
                    .ok_or_else(installation_invalid_error)?
                    .join(engine.release())
            };
            return Ok(RuntimePaths {
                venv: runtime_child_path(&root, ".venv", true),
                marker: runtime_child_path(&root, MARKER_NAME, true),
                root,
                host_marker: Some(host_marker),
                wsl: true,
                distro: Some(marker.distro),
            });
        }
        let output = capture("wsl.exe", &["--list", "--verbose"]).await?;
        let listing = decode_wsl_output(&output);
        let (distro, version) = parse_default_distro(&listing)
            .ok_or_else(|| compatibility_error("wsl_distribution_unavailable"))?;
        if version != "2" {
            return Err(compatibility_error("wsl2_required"));
        }
        let output = capture_wsl(Some(&distro), &["printenv", "HOME"]).await?;
        let home = String::from_utf8(output).map_err(|_| compatibility_error("wsl_unavailable"))?;
        let home = home.trim();
        if !valid_wsl_home(home) {
            return Err(compatibility_error("wsl_unavailable"));
        }
        let root = PathBuf::from(format!(
            "{}/.local/share/openchat/runtimes/{}/{}",
            home.trim_end_matches('/'),
            engine.id(),
            engine.release()
        ));
        Ok(RuntimePaths {
            venv: runtime_child_path(&root, ".venv", true),
            marker: runtime_child_path(&root, MARKER_NAME, true),
            root,
            host_marker: Some(host_marker),
            wsl: true,
            distro: Some(distro),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = storage;
        Err(compatibility_error("unsupported_platform"))
    }
}

async fn installed_release_for_catalog(
    storage: &AppStorage,
    engine: Engine,
) -> Result<Option<String>, ServiceError> {
    let paths = installed_runtime_paths(storage, engine).await?;
    Ok(paths
        .iter()
        .find_map(|paths| cached_release(paths, engine))
        .or_else(|| {
            paths
                .first()
                .and_then(|paths| cached_release(paths, engine))
        }))
}

async fn installed_runtime_paths(
    storage: &AppStorage,
    engine: Engine,
) -> Result<Vec<RuntimePaths>, ServiceError> {
    if engine == Engine::Sglang && cfg!(windows) {
        #[cfg(windows)]
        {
            let host_marker = storage
                .root()
                .join(format!("settings/{}-runtime.json", engine.id()));
            if let Some(marker) = read_host_marker(&host_marker, engine) {
                return Ok(vec![paths_from_host_marker(marker, host_marker)]);
            }
            return Ok(Vec::new());
        }
        #[cfg(not(windows))]
        return Ok(Vec::new());
    }

    let Some(current) = runtime_paths_sync(storage, engine)? else {
        return Ok(Vec::new());
    };
    let Some(parent) = current.root.parent() else {
        return Ok(Vec::new());
    };
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(storage_error()),
    };
    let mut installed = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| storage_error())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| storage_error())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let paths = RuntimePaths {
            venv: entry.path().join(".venv"),
            marker: entry.path().join(MARKER_NAME),
            root: entry.path(),
            host_marker: None,
            wsl: false,
            distro: None,
        };
        if cached_release(&paths, engine).is_some() {
            installed.push(paths);
        }
    }
    installed.sort_by_key(|paths| paths.root != current.root);
    Ok(installed)
}

async fn remove_managed_runtime(paths: &RuntimePaths, engine: Engine) -> Result<(), ServiceError> {
    if paths.wsl {
        let release = installed_release(paths, engine)
            .await
            .ok_or_else(installation_invalid_error)?;
        if !valid_managed_wsl_root(
            paths.root.to_str().ok_or_else(installation_invalid_error)?,
            engine,
            &release,
        ) {
            return Err(installation_invalid_error());
        }
        remove_wsl_directory(paths, &paths.root).await
    } else {
        cached_release(paths, engine).ok_or_else(installation_invalid_error)?;
        fs::remove_dir_all(&paths.root).map_err(|_| storage_error())
    }
}

async fn is_installed(paths: &RuntimePaths, engine: Engine) -> bool {
    installed_release(paths, engine).await.as_deref() == Some(engine.release())
}

async fn installed_release(paths: &RuntimePaths, engine: Engine) -> Option<String> {
    if paths.wsl {
        let marker_path = path_str(&paths.marker).ok()?;
        let output = capture_wsl(
            paths.distro.as_deref(),
            &["head", "-c", "2049", &marker_path],
        )
        .await
        .ok()?;
        let marker = parse_runtime_marker(&output, engine)?;
        let release = marker.release_tag;
        return (paths.root.file_name()?.to_str()? == release).then_some(release);
    }
    cached_release(paths, engine)
}

fn parse_runtime_marker(bytes: &[u8], engine: Engine) -> Option<RuntimeMarker> {
    if bytes.len() > 2048 {
        return None;
    }
    let marker: RuntimeMarker = serde_json::from_slice(bytes).ok()?;
    (marker.engine_id == engine.id()
        && marker.variant_id == engine.variant(cfg!(windows))
        && valid_release_tag(&marker.release_tag))
    .then_some(marker)
}

fn cached_release(paths: &RuntimePaths, engine: Engine) -> Option<String> {
    if paths.wsl {
        return paths
            .host_marker
            .as_deref()
            .and_then(|path| read_host_marker(path, engine))
            .map(|marker| marker.release_tag);
    }
    let marker = read_runtime_marker(paths, engine)?;
    (paths.root.file_name()?.to_str()? == marker.release_tag).then_some(marker.release_tag)
}

fn read_runtime_marker(paths: &RuntimePaths, engine: Engine) -> Option<RuntimeMarker> {
    if paths.wsl {
        return None;
    }
    let marker = fs::read(&paths.marker).ok()?;
    let marker: RuntimeMarker = serde_json::from_slice(&marker).ok()?;
    (marker.engine_id == engine.id()
        && valid_release_tag(&marker.release_tag)
        && marker.variant_id == engine.variant(cfg!(windows)))
    .then_some(marker)
}

fn read_host_marker(path: &Path, engine: Engine) -> Option<HostMarker> {
    let marker = fs::read(path).ok()?;
    let marker: HostMarker = serde_json::from_slice(&marker).ok()?;
    (marker.engine_id == engine.id()
        && valid_release_tag(&marker.release_tag)
        && valid_distro_name(&marker.distro)
        && valid_managed_wsl_root(&marker.root, engine, &marker.release_tag))
    .then_some(marker)
}

async fn write_marker(
    paths: &RuntimePaths,
    engine: Engine,
    variant_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let marker = serde_json::to_vec(&RuntimeMarker {
        engine_id: engine.id().to_owned(),
        release_tag: engine.release().to_owned(),
        variant_id: variant_id.to_owned(),
    })
    .map_err(|_| storage_error())?;
    if paths.wsl {
        write_wsl_file(paths, &paths.marker, &marker, cancellation).await?;
        let host_marker = paths.host_marker.as_ref().ok_or_else(storage_error)?;
        if let Some(parent) = host_marker.parent() {
            fs::create_dir_all(parent).map_err(|_| storage_error())?;
        }
        let distro = paths.distro.as_deref().ok_or_else(storage_error)?;
        let host_marker = serde_json::to_vec(&HostMarker {
            engine_id: engine.id().to_owned(),
            release_tag: engine.release().to_owned(),
            distro: distro.to_owned(),
            root: paths.root.to_string_lossy().into_owned(),
        })
        .map_err(|_| storage_error())?;
        fs::write(
            paths.host_marker.as_ref().ok_or_else(storage_error)?,
            host_marker,
        )
        .map_err(|_| storage_error())
    } else {
        fs::write(&paths.marker, marker).map_err(|_| storage_error())
    }
}

async fn write_private_file(
    paths: &RuntimePaths,
    path: &Path,
    contents: &[u8],
) -> Result<(), ServiceError> {
    if paths.wsl {
        write_wsl_file(paths, path, contents, &mut watch::channel(false).1).await
    } else {
        fs::write(path, contents).map_err(|_| storage_error())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .map_err(|_| storage_error())?;
        }
        Ok(())
    }
}

async fn create_wsl_directory(
    paths: &RuntimePaths,
    directory: &Path,
    create_parents: bool,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    if !paths.wsl {
        return Err(installation_invalid_error());
    }
    let mut arguments = Vec::with_capacity(2);
    if create_parents {
        arguments.push("-p".to_owned());
    }
    arguments.push(path_arg(directory)?);
    run(paths, "mkdir", &arguments, None, cancellation).await
}

async fn run_python_script(
    paths: &RuntimePaths,
    python: &str,
    script_source: &str,
    script_arguments: &[String],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let mut arguments = Vec::with_capacity(script_arguments.len() + 1);
    arguments.push("-".to_owned());
    arguments.extend_from_slice(script_arguments);
    run_with_environment(
        paths,
        python,
        &arguments,
        None,
        None,
        Some(script_source.as_bytes()),
        cancellation,
    )
    .await
}

async fn write_wsl_file(
    paths: &RuntimePaths,
    path: &Path,
    contents: &[u8],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let path = path_str(path)?;
    let mut command = wsl_command(paths.distro.as_deref());
    let mut child = command
        .args(["--exec", "install", "-D", "-m", "600", "/dev/stdin", &path])
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

async fn remove_partial_runtime(paths: &RuntimePaths) -> Result<(), ServiceError> {
    if paths.wsl {
        remove_wsl_directory(paths, &paths.root).await
    } else {
        match fs::symlink_metadata(&paths.root) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(installation_invalid_error()),
            Ok(_) => fs::remove_dir_all(&paths.root).map_err(|_| storage_error()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(storage_error()),
        }
    }
}

async fn remove_wsl_directory(paths: &RuntimePaths, path: &Path) -> Result<(), ServiceError> {
    if !paths.wsl || !path.starts_with(&paths.root) {
        return Err(installation_invalid_error());
    }
    let path = path_str(path)?;
    if capture_wsl(paths.distro.as_deref(), &["test", "!", "-L", &path])
        .await
        .is_err()
    {
        return Err(installation_invalid_error());
    }
    run(
        paths,
        "rm",
        &["-rf".to_owned(), "--".to_owned(), path.to_owned()],
        None,
        &mut watch::channel(false).1,
    )
    .await
}

fn base_python(engine: Engine, wsl: bool) -> (std::ffi::OsString, &'static [String]) {
    static WINDOWS_EXLLAMA_PREFIX: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    if cfg!(windows) && engine == Engine::Exllama && !wsl {
        let prefix = WINDOWS_EXLLAMA_PREFIX.get_or_init(|| vec!["-3.12".to_owned()]);
        (windows_python_launcher(), prefix)
    } else {
        ("python3".into(), &[])
    }
}

fn windows_python_launcher() -> std::ffi::OsString {
    #[cfg(windows)]
    {
        windows_python_launcher_from(
            std::env::var_os("SystemRoot").as_deref(),
            std::env::var_os("WINDIR").as_deref(),
            std::ffi::OsStr::new("py.exe"),
        )
    }
    #[cfg(not(windows))]
    {
        "py.exe".into()
    }
}

fn windows_python_launcher_from(
    system_root: Option<&std::ffi::OsStr>,
    windows_directory: Option<&std::ffi::OsStr>,
    fallback: &std::ffi::OsStr,
) -> std::ffi::OsString {
    [system_root, windows_directory]
        .into_iter()
        .flatten()
        .map(|directory| PathBuf::from(directory).join("py.exe"))
        .find(|launcher| launcher.is_file())
        .unwrap_or_else(|| PathBuf::from(fallback))
        .into_os_string()
}

pub(crate) fn python_path(paths: &RuntimePaths) -> PathBuf {
    if cfg!(windows) && !paths.wsl {
        paths.venv.join("Scripts/python.exe")
    } else {
        runtime_child_path(&paths.venv, "bin/python", paths.wsl)
    }
}

async fn run<P, A>(
    paths: &RuntimePaths,
    program: P,
    args: &[A],
    current_directory: Option<&Path>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError>
where
    P: AsRef<std::ffi::OsStr>,
    A: AsRef<std::ffi::OsStr>,
{
    run_with_environment(
        paths,
        program,
        args,
        current_directory,
        None,
        None,
        cancellation,
    )
    .await
}

async fn run_pip_install(
    paths: &RuntimePaths,
    python: &str,
    args: &[String],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let temporary_directory = runtime_child_path(&paths.root, ".tmp", paths.wsl);
    if paths.wsl {
        create_wsl_directory(paths, &temporary_directory, true, cancellation).await?;
    } else {
        fs::create_dir_all(&temporary_directory).map_err(|_| storage_error())?;
    }
    let temporary_directory = path_arg(&temporary_directory)?;
    run_with_environment(
        paths,
        python,
        args,
        None,
        Some(("TMPDIR", temporary_directory.as_str())),
        None,
        cancellation,
    )
    .await
}

async fn patch_sglang_cuda_header(
    paths: &RuntimePaths,
    python: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    run_python_script(
        paths,
        python,
        SGLANG_CUDA_HEADER_PATCH_SCRIPT,
        &[],
        cancellation,
    )
    .await
}

async fn run_with_environment<P, A>(
    paths: &RuntimePaths,
    program: P,
    args: &[A],
    current_directory: Option<&Path>,
    environment: Option<(&str, &str)>,
    input: Option<&[u8]>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError>
where
    P: AsRef<std::ffi::OsStr>,
    A: AsRef<std::ffi::OsStr>,
{
    let mut command = if paths.wsl {
        let mut command = wsl_command(paths.distro.as_deref());
        command.arg("--exec");
        if let Some((key, value)) = environment {
            command.arg("/usr/bin/env").arg(format!("{key}={value}"));
        }
        command.arg(program.as_ref()).args(args);
        command
    } else {
        let mut command = Command::new(program.as_ref());
        command.args(args);
        if let Some((key, value)) = environment {
            command.env(key, value);
        }
        if let Some(directory) = current_directory {
            command.current_dir(directory);
        }
        command
    };
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| install_error())?;
    if let Some(input) = input {
        let mut stdin = child.stdin.take().ok_or_else(storage_error)?;
        stdin.write_all(input).await.map_err(|_| install_error())?;
    }
    wait_child(&mut child, cancellation).await
}

async fn capture<P: AsRef<std::ffi::OsStr>>(
    program: P,
    args: &[&str],
) -> Result<Vec<u8>, ServiceError> {
    let output = timeout(
        Duration::from_secs(20),
        Command::new(program.as_ref()).args(args).output(),
    )
    .await
    .map_err(|_| compatibility_error("runtime_unavailable"))?
    .map_err(|_| compatibility_error("runtime_unavailable"))?;
    if !output.status.success() {
        return Err(compatibility_error("runtime_unavailable"));
    }
    Ok(output.stdout)
}

async fn capture_wsl(distro: Option<&str>, args: &[&str]) -> Result<Vec<u8>, ServiceError> {
    let mut command = wsl_command(distro);
    let output = timeout(
        Duration::from_secs(30),
        command.args(["--exec"]).args(args).output(),
    )
    .await
    .map_err(|_| compatibility_error("wsl_unavailable"))?
    .map_err(|_| compatibility_error("wsl_unavailable"))?;
    if !output.status.success() {
        return Err(compatibility_error("wsl_unavailable"));
    }
    Ok(output.stdout)
}

fn wsl_command(distro: Option<&str>) -> Command {
    let mut command = Command::new("wsl.exe");
    if let Some(distro) = distro {
        command.args(["--distribution", distro]);
    }
    command
}

async fn wait_child(
    child: &mut Child,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let deadline = Instant::now() + INSTALL_TIMEOUT;
    loop {
        if *cancellation.borrow() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(cancelled_error());
        }
        match child.try_wait().map_err(|_| install_error())? {
            Some(status) if status.success() => return Ok(()),
            Some(_) => return Err(install_error()),
            None => {}
        }
        if Instant::now() >= deadline {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(install_timeout_error());
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(250)) => {},
            changed = cancellation.changed() => if changed.is_err() || *cancellation.borrow() {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(cancelled_error());
            },
        }
    }
}

async fn send_progress(
    events: &EventSink,
    request_id: &Value,
    engine: Engine,
    variant_id: &str,
    phase: &str,
    asset: &str,
) -> Result<(), ServiceError> {
    events
        .send(&Response::event(
            request_id.clone(),
            PROGRESS_EVENT,
            json!({
                "engineId": engine.id(),
                "variantId": variant_id,
                "phase": phase,
                "assetName": asset,
                "assetIndex": 0,
                "assetCount": 1,
                "downloadedBytes": 0,
                "totalBytes": 0,
            }),
        ))
        .await
        .map_err(|_| install_error())
}

#[cfg(windows)]
fn parse_default_distro(listing: &str) -> Option<(String, String)> {
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
    let distro = fields[..fields.len() - 2].join(" ");
    valid_distro_name(&distro).then_some((distro, version))
}

#[cfg(windows)]
fn decode_wsl_output(output: &[u8]) -> String {
    let utf16_little_endian = output.starts_with(&[0xff, 0xfe])
        || (output.len() % 2 == 0
            && output.chunks_exact(2).filter(|pair| pair[1] == 0).count() * 2 >= output.len() / 2);
    if utf16_little_endian {
        let bytes = output.strip_prefix(&[0xff, 0xfe]).unwrap_or(output);
        let decoded = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&decoded)
            .trim_start_matches('\u{feff}')
            .to_owned()
    } else {
        String::from_utf8_lossy(output)
            .trim_start_matches('\u{feff}')
            .to_owned()
    }
}

fn valid_distro_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._- ".contains(&byte))
}

#[cfg(windows)]
fn valid_wsl_home(home: &str) -> bool {
    (home.starts_with("/home/") || home == "/root")
        && !home.contains('\\')
        && !home.split('/').any(|part| part == "..")
}

fn valid_managed_wsl_root(root: &str, engine: Engine, release_tag: &str) -> bool {
    let suffix = format!(
        "/.local/share/openchat/runtimes/{}/{}",
        engine.id(),
        release_tag
    );
    valid_release_tag(release_tag)
        && (root.starts_with("/home/") || root.starts_with("/root/"))
        && root.ends_with(&suffix)
        && !root.contains('\\')
        && !root.split('/').any(|part| part == "..")
}

fn valid_release_tag(release_tag: &str) -> bool {
    !release_tag.is_empty()
        && release_tag.len() <= 128
        && release_tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn path_arg(path: &Path) -> Result<String, ServiceError> {
    path.to_str().map(str::to_owned).ok_or_else(storage_error)
}
fn path_str(path: &Path) -> Result<&str, ServiceError> {
    path.to_str().ok_or_else(storage_error)
}

fn check_cancelled(cancellation: &watch::Receiver<bool>) -> Result<(), ServiceError> {
    if *cancellation.borrow() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn unknown_engine_error() -> ServiceError {
    ServiceError::new(
        "local_engine_not_found",
        "The selected local engine is not available.",
        false,
    )
}
fn unsupported_variant_error() -> ServiceError {
    ServiceError::new(
        "local_engine_install_unavailable",
        "The selected local engine package is not available for this device.",
        false,
    )
}
fn compatibility_error(reason: &str) -> ServiceError {
    ServiceError::new("local_engine_install_unavailable", reason, false)
}
fn engine_unavailable_error() -> ServiceError {
    ServiceError::new(
        "local_engine_not_installed",
        "The selected local engine is not installed.",
        false,
    )
}
fn model_unavailable_error() -> ServiceError {
    ServiceError::new(
        "local_model_unavailable",
        "The selected model is not available to this engine.",
        false,
    )
}
fn install_error() -> ServiceError {
    ServiceError::new(
        "local_engine_install_failed",
        "The selected local engine could not be installed.",
        true,
    )
}
fn install_timeout_error() -> ServiceError {
    ServiceError::new(
        "local_engine_install_timeout",
        "The local engine installation exceeded the allowed time.",
        true,
    )
}
fn installation_invalid_error() -> ServiceError {
    ServiceError::new(
        "local_engine_installation_invalid",
        "The managed local engine installation could not be verified.",
        false,
    )
}
fn storage_error() -> ServiceError {
    ServiceError::new(
        "local_engine_storage_failed",
        "The local engine runtime files could not be accessed.",
        false,
    )
}
fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "The local engine installation was cancelled.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use uuid::Uuid;

    use super::{
        Engine, RuntimeMarker, SGLANG_CUDA_HEADER_PATCH_SCRIPT, SGLANG_INSTALL_VERIFICATION_SCRIPT,
        SGLANG_LAUNCHER_SCRIPT, SGLANG_RELEASE, SGLANG_RUNTIME_REQUIREMENTS, engine_json,
        exllama_max_seq_len, install, installed_engine, installed_release_for_catalog,
        installed_runtime_paths, parse_runtime_marker, python_minor_version, runtime_child_path,
        runtime_paths_sync, uninstall, valid_managed_wsl_root, valid_release_tag,
        windows_python_launcher_from, write_marker,
    };
    #[cfg(windows)]
    use super::{decode_wsl_output, parse_default_distro, valid_wsl_home};
    use crate::{protocol::EventSink, storage::AppStorage};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-python-engine-test-{}", Uuid::new_v4()));
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
    fn identifies_supported_python_engines_and_variants() {
        let exllama = Engine::parse("exllama").expect("ExLlama should be supported");
        let sglang = Engine::parse("sglang").expect("SGLang should be supported");

        assert_eq!(exllama.id(), "exllama");
        assert_eq!(sglang.id(), "sglang");
        assert_eq!(exllama.variant(true), "windows-cu13".to_owned());
        assert_eq!(sglang.variant(true), "windows-wsl2-cuda130".to_owned());
        assert_eq!(
            Engine::parse("onnx")
                .expect_err("unknown engines must be rejected")
                .code,
            "local_engine_not_found"
        );
    }

    #[test]
    fn exllama_context_prefers_model_then_engine_then_model_config() {
        assert_eq!(exllama_max_seq_len(Some(16_384), Some(8_192)), 16_384);
        assert_eq!(exllama_max_seq_len(None, Some(8_192)), 8_192);
        assert_eq!(exllama_max_seq_len(None, None), -1);
    }

    #[test]
    fn finds_python_launcher_from_either_windows_directory_variable() {
        let temporary = TestDirectory::new();
        let system_root = temporary.0.join("system-root");
        let windows_directory = temporary.0.join("windows-directory");
        fs::create_dir_all(&system_root).expect("system root should be created");
        fs::create_dir_all(&windows_directory).expect("Windows directory should be created");
        let windir_launcher = windows_directory.join("py.exe");
        fs::write(&windir_launcher, b"launcher").expect("launcher fixture should be written");

        assert_eq!(
            windows_python_launcher_from(
                Some(system_root.as_os_str()),
                Some(windows_directory.as_os_str()),
                std::ffi::OsStr::new("py.exe"),
            ),
            windir_launcher.into_os_string()
        );
    }

    #[test]
    fn managed_sglang_runtime_installs_and_uses_its_cuda_compiler() {
        assert!(SGLANG_RUNTIME_REQUIREMENTS.contains(&"sglang==0.5.23"));
        assert!(SGLANG_RUNTIME_REQUIREMENTS.contains(&"cuda-toolkit[nvcc,nvvm,cccl]==13.0.3"));
        assert!(
            SGLANG_CUDA_HEADER_PATCH_SCRIPT.contains("libc_name != \"glibc\" or version < (2, 42)")
        );
        assert!(SGLANG_CUDA_HEADER_PATCH_SCRIPT.contains("noexcept\\(true\\)"));
        assert!(SGLANG_CUDA_HEADER_PATCH_SCRIPT.contains("if len(matches) != 1:"));
        assert!(SGLANG_LAUNCHER_SCRIPT.contains("site.getsitepackages()"));
        assert!(
            SGLANG_LAUNCHER_SCRIPT.contains("cuda_libraries = os.path.join(cuda_home, \"lib64\")")
        );
        assert!(SGLANG_LAUNCHER_SCRIPT.contains("os.environ[\"CUDA_HOME\"] = cuda_home"));
        assert!(
            SGLANG_LAUNCHER_SCRIPT
                .contains("for variable in (\"LIBRARY_PATH\", \"LD_LIBRARY_PATH\"):")
        );
        assert!(SGLANG_LAUNCHER_SCRIPT.contains("/usr/lib/wsl/lib"));
        assert!(SGLANG_LAUNCHER_SCRIPT.contains("if __name__ == \"__main__\":"));
        assert!(SGLANG_INSTALL_VERIFICATION_SCRIPT.contains("torch.cuda.is_available()"));
    }

    #[test]
    fn wsl_runtime_release_probe_reads_the_serialized_marker_field() {
        let marker = RuntimeMarker {
            engine_id: Engine::Sglang.id().to_owned(),
            release_tag: SGLANG_RELEASE.to_owned(),
            variant_id: Engine::Sglang.variant(true),
        };
        let marker_json = serde_json::to_vec(&marker).expect("runtime marker should serialize");
        let parsed = parse_runtime_marker(&marker_json, Engine::Sglang)
            .expect("matching runtime marker should be accepted");
        assert_eq!(parsed.release_tag, SGLANG_RELEASE);
        assert!(parse_runtime_marker(&marker_json, Engine::Exllama).is_none());
        assert!(parse_runtime_marker(&vec![b' '; 2049], Engine::Sglang).is_none());
    }

    #[test]
    fn reads_supported_python_minor_versions_from_version_output() {
        assert_eq!(python_minor_version(b"Python 3.12.8\n"), Some(12));
        assert_eq!(python_minor_version(b"Python 2.7.18\n"), None);
        assert_eq!(python_minor_version(b"Python development build"), None);
    }

    #[cfg(windows)]
    #[test]
    fn parses_default_wsl_distribution_and_utf16_output() {
        let listing = "  NAME              STATE           VERSION\n* Ubuntu-24.04      Stopped         2\n  Debian            Stopped         2\n";
        assert_eq!(
            parse_default_distro(listing),
            Some(("Ubuntu-24.04".to_owned(), "2".to_owned()))
        );

        let utf16 = "* Ubuntu-24.04 Stopped 2\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(decode_wsl_output(&utf16), "* Ubuntu-24.04 Stopped 2\n");
        assert_eq!(parse_default_distro("Ubuntu-24.04 Stopped 2\n"), None);
    }

    #[test]
    fn restricts_managed_wsl_paths_to_the_engine_runtime_root() {
        assert!(valid_managed_wsl_root(
            "/home/alican/.local/share/openchat/runtimes/sglang/v0.5.23",
            Engine::Sglang,
            "v0.5.23"
        ));
        assert!(!valid_managed_wsl_root(
            "/home/alican/.local/share/openchat/runtimes/exllama/v0.5.23",
            Engine::Sglang,
            "v0.5.23"
        ));
        assert!(!valid_managed_wsl_root(
            "/home/alican/.local/share/openchat/runtimes/sglang/../outside",
            Engine::Sglang,
            "../outside"
        ));
    }

    #[cfg(windows)]
    #[test]
    fn accepts_only_safe_wsl_home_paths() {
        assert!(valid_wsl_home("/home/alican"));
        assert!(valid_wsl_home("/root"));
        assert!(!valid_wsl_home("/mnt/c/Users/alican"));
        assert!(!valid_wsl_home("/home/../tmp"));
    }

    #[test]
    fn accepts_safe_release_tags_for_managed_runtime_updates() {
        assert!(valid_release_tag("v0.5.23"));
        assert!(valid_release_tag("tabbyapi-4728158-exllamav3-1.6.7"));
        assert!(!valid_release_tag("../outside"));
        assert!(!valid_release_tag("v0.5.23/other"));
        assert!(!valid_release_tag(""));
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[tokio::test]
    async fn discovers_previous_native_runtime_release_as_an_update() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
        let engine = Engine::Exllama;
        let current = runtime_paths_sync(&storage, engine)
            .expect("runtime paths should resolve")
            .expect("native ExLlama paths should exist");
        let previous_root = current
            .root
            .parent()
            .expect("runtime version should have a platform parent")
            .join("previous-release");
        fs::create_dir_all(&previous_root).expect("previous runtime should be created");
        let marker = RuntimeMarker {
            engine_id: engine.id().to_owned(),
            release_tag: "previous-release".to_owned(),
            variant_id: engine.variant(cfg!(windows)),
        };
        fs::write(
            previous_root.join(super::MARKER_NAME),
            serde_json::to_vec(&marker).expect("marker should serialize"),
        )
        .expect("previous release marker should be written");

        let installed = installed_runtime_paths(&storage, engine)
            .await
            .expect("managed runtimes should be discoverable");
        assert_eq!(installed.len(), 1);
        assert_eq!(
            installed_release_for_catalog(&storage, engine)
                .await
                .expect("installed release should be readable")
                .as_deref(),
            Some("previous-release")
        );
    }

    #[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
    #[tokio::test]
    async fn install_rejects_an_unavailable_variant_without_creating_runtime_files() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
        let (_cancel, mut cancellation) = tokio::sync::watch::channel(false);

        let error = install(
            &storage,
            "exllama",
            "unsupported-variant",
            &serde_json::json!("request-1"),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect_err("an unlisted runtime package must be rejected");

        assert_eq!(error.code, "local_engine_install_unavailable");
        assert!(!storage.root().join("runtimes/exllama").exists());
    }

    #[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
    #[tokio::test]
    async fn cancelled_install_stops_before_runtime_discovery_or_creation() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
        let (cancel, mut cancellation) = tokio::sync::watch::channel(false);
        cancel
            .send(true)
            .expect("cancellation receiver should be active");

        let error = install(
            &storage,
            "exllama",
            &Engine::Exllama.variant(cfg!(windows)),
            &serde_json::json!("request-2"),
            &EventSink::new(),
            &mut cancellation,
        )
        .await
        .expect_err("a cancelled installation must stop before preflight work");

        assert_eq!(error.code, "operation_cancelled");
        assert!(!storage.root().join("runtimes/exllama").exists());
    }

    #[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
    #[tokio::test]
    async fn successful_runtime_marker_registers_the_installed_engine() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
        let engine = Engine::Exllama;
        let paths = runtime_paths_sync(&storage, engine)
            .expect("runtime paths should resolve")
            .expect("native ExLlama paths should exist");
        fs::create_dir_all(&paths.root).expect("managed runtime directory should be created");
        let (_cancel, mut cancellation) = tokio::sync::watch::channel(false);

        write_marker(
            &paths,
            engine,
            &engine.variant(cfg!(windows)),
            &mut cancellation,
        )
        .await
        .expect("successful installation should write its runtime marker");

        let installed = installed_engine(&storage, engine.id())
            .expect("installed runtime should be discoverable")
            .expect("managed runtime marker should register the engine");
        assert_eq!(installed.engine_id, engine.id());
        assert_eq!(installed.variant_id, engine.variant(cfg!(windows)));
        assert_eq!(installed.release_tag, engine.release());
        assert_eq!(installed.root, paths.root);
    }

    #[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
    #[tokio::test]
    async fn catalog_marks_python_engines_incompatible_without_a_cuda_gpu() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");

        let catalog = engine_json(&storage, "exllama", "/models/exllama", "stopped", false)
            .await
            .expect("engine catalog should load without a CUDA GPU");

        assert_eq!(catalog["engineId"], "exllama");
        assert_eq!(catalog["variants"][0]["status"], "gpu_unavailable");
        assert_eq!(catalog["variants"][0]["canInstall"], false);
        assert_eq!(catalog["modelDirectory"], "/models/exllama");
    }

    #[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
    #[tokio::test]
    async fn uninstall_removes_only_the_managed_runtime_and_preserves_downloaded_models() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
        let engine = Engine::Exllama;
        let paths = runtime_paths_sync(&storage, engine)
            .expect("runtime paths should resolve")
            .expect("native ExLlama paths should exist");
        fs::create_dir_all(&paths.root).expect("managed runtime directory should be created");
        let marker = RuntimeMarker {
            engine_id: engine.id().to_owned(),
            release_tag: engine.release().to_owned(),
            variant_id: engine.variant(cfg!(windows)),
        };
        fs::write(
            &paths.marker,
            serde_json::to_vec(&marker).expect("runtime marker should serialize"),
        )
        .expect("runtime marker should be written");

        let model_directory = storage.root().join("models/exllama");
        fs::create_dir_all(&model_directory).expect("model directory should exist");
        let model_file = model_directory.join("model.safetensors");
        fs::write(&model_file, b"model data").expect("downloaded model fixture should be written");

        let result = uninstall(&storage, engine.id())
            .await
            .expect("verified managed runtime should uninstall");

        assert_eq!(result["removed"], true);
        assert!(!paths.root.exists());
        assert_eq!(
            fs::read(&model_file).expect("downloaded model should remain"),
            b"model data"
        );
    }

    #[test]
    fn joins_native_and_wsl_paths_without_changing_separators() {
        assert_eq!(
            runtime_child_path(
                std::path::Path::new("/opt/openchat/runtime"),
                "models",
                true
            ),
            std::path::PathBuf::from("/opt/openchat/runtime/models")
        );
        assert_eq!(
            runtime_child_path(std::path::Path::new("C:/OpenChat/runtime"), "models", false),
            std::path::PathBuf::from("C:/OpenChat/runtime/models")
        );
    }
}
