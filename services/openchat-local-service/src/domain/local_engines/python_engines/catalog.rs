use super::*;

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
pub(super) fn paths_from_host_marker(marker: HostMarker, host_marker: PathBuf) -> RuntimePaths {
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

pub(super) fn python_minor_version(output: &[u8]) -> Option<u8> {
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

pub(super) async fn python_compatibility_in(
    paths: &RuntimePaths,
    engine: Engine,
) -> Option<&'static str> {
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

pub(super) async fn sglang_python(
    paths: Option<&RuntimePaths>,
) -> Result<std::ffi::OsString, &'static str> {
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
