//! App-managed local inference engine releases.

mod catalog;
mod external_server;
mod gguf;
pub(crate) mod installer;
pub(crate) mod models;
mod runtime;
mod settings;

use std::time::Duration;

use serde_json::{Value, json};
use tokio::{process::Command, time::timeout};

use crate::{protocol::ServiceError, storage::AppStorage};

use catalog::{CatalogError, DriverVersion, EngineManifest, EngineVariant, parse_manifest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NvidiaDriverStatus {
    NotDetected,
    VersionUnavailable,
    Detected(DriverVersion),
}

const CATALOGS: &[(&str, &[u8])] = &[(
    "llama_cpp",
    include_bytes!("../../../resources/local-engines/llama_cpp.json"),
)];

pub(crate) fn manifests() -> Result<Vec<EngineManifest>, ServiceError> {
    CATALOGS
        .iter()
        .map(|(engine_id, bytes)| {
            let manifest = parse_manifest(bytes).map_err(catalog_error)?;
            if manifest.engine_id != *engine_id {
                return Err(catalog_error(CatalogError::Invalid(
                    "embedded engine ID did not match its catalog entry".to_owned(),
                )));
            }
            Ok(manifest)
        })
        .collect()
}

pub(crate) async fn list(storage: &AppStorage) -> Result<Value, ServiceError> {
    models::ensure_storage_directories(storage.root())?;
    let nvidia_driver = nvidia_driver_status().await;
    let cuda_available = matches!(nvidia_driver, NvidiaDriverStatus::Detected(_));
    let runtime = runtime::status(storage).await?;
    let external_llama_servers = runtime::external_server_status().await;
    let llama_server_executable_path = settings::llama_server_executable_path(storage.root())?;
    let llama_server_executable_available = llama_server_executable_path
        .as_deref()
        .and_then(settings::validate_saved_executable_path)
        .is_some();
    let registered_models = models::list(storage)?;
    let engines = manifests()?
        .into_iter()
        .map(|manifest| {
            let catalog_status = manifest.catalog_status.as_str();
            let status_reason = manifest.status_reason.as_deref();
            let model_directory =
                models::default_model_directory(storage.root(), &manifest.engine_id)?
                    .to_str()
                    .ok_or_else(|| {
                        ServiceError::new(
                            "local_model_directory_unavailable",
                            "The default model directory could not be represented.",
                            false,
                        )
                    })?
                    .to_owned();
            let recommended_variant = if catalog_status == "installable" {
                recommended_variant_id(&manifest, nvidia_driver, host_os(), host_architecture())
            } else {
                None
            };
            let variants = manifest
                .variants
                .iter()
                .filter(|variant| variant_visible_for_host(&manifest.engine_id, variant, host_os()))
                .map(|variant| {
                    variant_json(
                        storage,
                        &manifest,
                        variant,
                        catalog_status,
                        nvidia_driver,
                        recommended_variant.as_deref(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({
                "engineId": manifest.engine_id,
                "displayName": manifest.display_name,
                "releaseTag": manifest.release_tag,
                "latestUpstreamReleaseTag": manifest.release_tag,
                "releaseCheckStatus": "current",
                "channel": manifest.channel,
                "catalogStatus": catalog_status,
                "statusReason": status_reason,
                "modelDirectory": model_directory,
                "runtimeStatus": if manifest.engine_id == "llama_cpp" {
                    runtime_status_for_engine(&runtime, "llama_cpp")
                } else {
                    "unavailable"
                },
                "variants": variants,
            }))
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;

    Ok(json!({
        "engines": engines,
        "models": registered_models.iter().map(|model| {
            let available = runtime::is_model_available(storage, model)?;
            models::to_json(model, available)
        }).collect::<Result<Vec<_>, ServiceError>>()?,
        "runtime": runtime,
        "externalLlamaServers": external_llama_servers,
        "llamaServerExecutablePath": llama_server_executable_path
            .as_ref()
            .and_then(|path| path.to_str()),
        "llamaServerExecutableAvailable": llama_server_executable_available,
        "host": {
            "os": host_os(),
            "architecture": host_architecture(),
            "cudaAvailable": cuda_available,
        },
    }))
}

pub(crate) async fn runtime_status(storage: &AppStorage) -> Result<Value, ServiceError> {
    Ok(json!({
        "runtime": runtime::status(storage).await?,
        "externalLlamaServers": runtime::external_server_status().await,
    }))
}

fn runtime_status_for_engine(runtime: &Value, engine_id: &str) -> &'static str {
    let servers = runtime.get("servers").and_then(Value::as_array);
    let Some(servers) = servers else {
        return "stopped";
    };
    let mut unhealthy = false;
    for server in servers {
        if server["engineId"].as_str() != Some(engine_id) {
            continue;
        }
        match server["status"].as_str() {
            Some("running") => return "running",
            Some("unhealthy") => unhealthy = true,
            _ => {}
        }
    }
    if unhealthy { "unhealthy" } else { "stopped" }
}

pub(crate) async fn start_model(
    storage: &AppStorage,
    model_id: &str,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    runtime::start_model(storage, model_id, cancellation).await
}

pub(crate) async fn stop_runtime(storage: &AppStorage) -> Result<Value, ServiceError> {
    runtime::stop(storage).await
}

pub(crate) async fn stop_model(
    storage: &AppStorage,
    model_id: &str,
) -> Result<Value, ServiceError> {
    runtime::stop_model(storage, model_id).await
}

pub(crate) async fn chat_url(
    storage: &AppStorage,
    model_id: &str,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<runtime::RuntimeChatEndpoint, ServiceError> {
    runtime::chat_url(storage, model_id, cancellation).await
}

pub(crate) fn model_catalog(storage: &AppStorage) -> Result<Value, ServiceError> {
    let models = models::list(storage)?;
    Ok(json!({
        "freshness": "current",
        "models": models.iter().map(|model| {
            let available = runtime::is_model_available(storage, model)?;
            models::to_json(model, available)
        }).collect::<Result<Vec<_>, ServiceError>>()?,
    }))
}

pub(crate) fn set_llama_server_executable_path(
    storage: &AppStorage,
    path: Option<&str>,
) -> Result<Value, ServiceError> {
    settings::set_llama_server_executable_path(storage.root(), path)
}

pub(crate) async fn detect_external_llama_servers() -> Result<Value, ServiceError> {
    let servers = runtime::find_external_server_candidates().await?;
    Ok(json!({
        "servers": servers.iter().map(|server| json!({
            "processId": server.process_id,
            "port": server.port,
        })).collect::<Vec<_>>(),
    }))
}

pub(crate) async fn connect_external_llama_server(
    process_id: u32,
    port: u16,
) -> Result<Value, ServiceError> {
    runtime::connect_external_server(process_id, port).await
}

pub(crate) async fn disconnect_external_llama_server(process_id: u32, port: u16) -> Value {
    runtime::disconnect_external_server(process_id, port).await
}

pub(crate) async fn chat_model_catalog(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<Value, ServiceError> {
    runtime::chat_model_catalog(storage, engine_id).await
}

pub(crate) fn model_is_available(
    storage: &AppStorage,
    model: &models::RegisteredModel,
) -> Result<bool, ServiceError> {
    runtime::is_model_available(storage, model)
}

pub(crate) async fn register_model(
    storage: &AppStorage,
    engine_id: &str,
    model_path: &str,
    model_directory: Option<&str>,
    storage_action: &str,
) -> Result<Value, ServiceError> {
    let database_root = storage.root().to_path_buf();
    let engine_id = engine_id.to_owned();
    let model_path = model_path.to_owned();
    let model_directory = model_directory.map(str::to_owned);
    let storage_action = storage_action.to_owned();
    let model = tokio::task::spawn_blocking(move || {
        models::register_with_storage_action(
            &database_root,
            &engine_id,
            &model_path,
            model_directory.as_deref(),
            &storage_action,
        )
    })
    .await
    .map_err(|_| {
        ServiceError::new(
            "local_model_action_failed",
            "The local model action could not be completed.",
            false,
        )
    })??;
    let available = runtime::is_model_available(storage, &model)?;
    models::to_json(&model, available)
}

pub(crate) async fn discover_models(
    storage: &AppStorage,
    engine_id: &str,
    model_directory: &str,
) -> Result<Value, ServiceError> {
    let registered_paths = models::list(storage)?
        .into_iter()
        .filter(|model| model.engine_id == engine_id)
        .map(|model| model.path)
        .collect::<Vec<_>>();
    let engine_id = engine_id.to_owned();
    let model_directory = model_directory.to_owned();
    tokio::task::spawn_blocking(move || {
        models::discover(&engine_id, &model_directory, &registered_paths)
    })
    .await
    .map_err(|_| {
        ServiceError::new(
            "local_model_discovery_failed",
            "The selected model directory could not be scanned.",
            false,
        )
    })?
}

pub(crate) async fn remove_model(
    storage: &AppStorage,
    model_id: &str,
) -> Result<Value, ServiceError> {
    runtime::stop_model(storage, model_id).await?;
    Ok(json!({"removed": models::remove(storage, model_id)?}))
}

pub(crate) async fn find_variant(
    engine_id: &str,
    variant_id: &str,
) -> Result<(EngineManifest, EngineVariant), ServiceError> {
    let manifest = manifests()?
        .into_iter()
        .find(|manifest| manifest.engine_id == engine_id)
        .ok_or_else(unknown_engine_error)?;
    let variant = manifest
        .variants
        .iter()
        .find(|variant| variant.variant_id == variant_id)
        .cloned()
        .ok_or_else(unsupported_engine_variant_error)?;
    Ok((manifest, variant))
}

pub(crate) async fn can_install(manifest: &EngineManifest, variant: &EngineVariant) -> bool {
    if !can_install_on_host(manifest, variant, host_os(), host_architecture()) {
        return false;
    }
    let driver_status = nvidia_driver_status().await;
    variant.accelerator != "cuda" || variant_hardware_status(variant, driver_status).is_none()
}

fn can_install_on_host(
    manifest: &EngineManifest,
    variant: &EngineVariant,
    os: &str,
    architecture: &str,
) -> bool {
    catalog_status_for_host(&manifest.engine_id, &manifest.catalog_status, os) == "installable"
        && variant_host_os(variant) == os
        && variant.architecture == architecture
}

fn variant_json(
    storage: &AppStorage,
    manifest: &EngineManifest,
    variant: &EngineVariant,
    catalog_status: &str,
    nvidia_driver: NvidiaDriverStatus,
    recommended_variant: Option<&str>,
) -> Result<Value, ServiceError> {
    let installed_engine = installer::installed_engine(storage, manifest, variant)?;
    let installed = installed_engine.is_some();
    let update_available = catalog_status == "installable"
        && installed_engine
            .as_ref()
            .is_some_and(|installed| installed.release_tag != manifest.release_tag);
    let platform_matches =
        variant_host_os(variant) == host_os() && variant.architecture == host_architecture();
    let hardware_status = variant_hardware_status(variant, nvidia_driver);
    let can_install = catalog_status == "installable"
        && platform_matches
        && hardware_status.is_none()
        && (!installed || update_available);
    let status = if installed {
        if update_available {
            "update_available"
        } else {
            "installed"
        }
    } else if catalog_status == "deprecated" {
        "deprecated"
    } else if catalog_status == "blocked" {
        "blocked"
    } else if !platform_matches {
        "unsupported_platform"
    } else {
        hardware_status.unwrap_or("available")
    };
    Ok(json!({
        "variantId": variant.variant_id,
        "os": variant_host_os(variant),
        "runtimeOs": variant.os,
        "architecture": variant.architecture,
        "accelerator": variant.accelerator,
        "runtimeRequirements": variant.runtime_requirements,
        "canInstall": can_install,
        "recommended": platform_matches
            && recommended_variant == Some(variant.variant_id.as_str()),
        "installed": installed,
        "installedReleaseTag": installed_engine.map(|engine| engine.release_tag),
        "status": status,
    }))
}

fn variant_hardware_status(
    variant: &EngineVariant,
    driver_status: NvidiaDriverStatus,
) -> Option<&'static str> {
    if variant.accelerator != "cuda" {
        return None;
    }
    match driver_status {
        NvidiaDriverStatus::NotDetected => Some("hardware_unavailable"),
        NvidiaDriverStatus::VersionUnavailable => Some("driver_version_unavailable"),
        NvidiaDriverStatus::Detected(driver_version) => {
            let minimum = variant
                .minimum_driver_version
                .as_deref()
                .and_then(DriverVersion::parse);
            match minimum {
                Some(minimum) if driver_version >= minimum => None,
                Some(_) => Some("driver_unsupported"),
                None => Some("driver_version_unavailable"),
            }
        }
    }
}

fn recommended_variant_id(
    manifest: &EngineManifest,
    driver_status: NvidiaDriverStatus,
    os: &str,
    architecture: &str,
) -> Option<String> {
    if manifest.catalog_status != "installable" {
        return None;
    }

    let driver_version = match driver_status {
        NvidiaDriverStatus::Detected(version) => Some(version),
        NvidiaDriverStatus::NotDetected | NvidiaDriverStatus::VersionUnavailable => None,
    };
    let recommended_cuda = manifest
        .variants
        .iter()
        .filter(|variant| {
            variant_host_os(variant) == os
                && variant.architecture == architecture
                && variant.accelerator == "cuda"
        })
        .filter_map(|variant| {
            let actual = driver_version?;
            let minimum = DriverVersion::parse(variant.minimum_driver_version.as_deref()?)?;
            let recommended = DriverVersion::parse(variant.recommended_driver_version.as_deref()?)?;
            (actual >= minimum && actual >= recommended).then_some((recommended, variant))
        })
        .max_by_key(|(recommended_driver, _)| *recommended_driver)
        .map(|(_, variant)| variant.variant_id.clone());
    if recommended_cuda.is_some() {
        return recommended_cuda;
    }

    manifest
        .variants
        .iter()
        .find(|variant| {
            variant_host_os(variant) == os
                && variant.architecture == architecture
                && variant.accelerator == "cpu"
        })
        .map(|variant| variant.variant_id.clone())
}

async fn nvidia_driver_status() -> NvidiaDriverStatus {
    let Some(executable) = nvidia_driver_executable() else {
        return NvidiaDriverStatus::NotDetected;
    };
    let result = timeout(Duration::from_secs(2), async {
        Command::new(executable)
            .args([
                "--query-gpu=driver_version",
                "--format=csv,noheader,nounits",
            ])
            .kill_on_drop(true)
            .output()
            .await
    })
    .await;
    let output = match result {
        Ok(Ok(output)) if output.status.success() => output,
        Ok(Ok(_)) | Ok(Err(_)) | Err(_) => return NvidiaDriverStatus::VersionUnavailable,
    };
    let Ok(stdout) = std::str::from_utf8(&output.stdout) else {
        return NvidiaDriverStatus::VersionUnavailable;
    };
    match parse_nvidia_driver_output(stdout) {
        Ok(Some(version)) => NvidiaDriverStatus::Detected(version),
        Ok(None) => NvidiaDriverStatus::NotDetected,
        Err(()) => NvidiaDriverStatus::VersionUnavailable,
    }
}

fn variant_host_os(variant: &EngineVariant) -> &str {
    variant.host_os.as_deref().unwrap_or(&variant.os)
}

fn variant_visible_for_host(engine_id: &str, variant: &EngineVariant, os: &str) -> bool {
    engine_id != "llama_cpp" || os != "windows" || variant_host_os(variant) == "windows"
}

fn catalog_status_for_host<'a>(_engine_id: &str, catalog_status: &'a str, _os: &str) -> &'a str {
    catalog_status
}

fn parse_nvidia_driver_output(value: &str) -> Result<Option<DriverVersion>, ()> {
    let mut versions = value.lines().map(str::trim).filter(|line| !line.is_empty());
    let Some(first) = versions.next() else {
        return Ok(None);
    };
    let version = DriverVersion::parse(first).ok_or(())?;
    if versions.any(|line| DriverVersion::parse(line) != Some(version)) {
        return Err(());
    }
    Ok(Some(version))
}

#[cfg(windows)]
fn nvidia_driver_executable() -> Option<std::path::PathBuf> {
    let windows = std::env::var_os("WINDIR").map(std::path::PathBuf::from);
    let program_files = std::env::var_os("ProgramFiles").map(std::path::PathBuf::from);
    let candidates = [
        windows.map(|path| path.join("System32").join("nvidia-smi.exe")),
        program_files.map(|path| {
            path.join("NVIDIA Corporation")
                .join("NVSMI")
                .join("nvidia-smi.exe")
        }),
    ];
    candidates.into_iter().flatten().find(|path| path.is_file())
}

#[cfg(not(windows))]
fn nvidia_driver_executable() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;

        return std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path).find_map(|directory| {
                let executable = directory.join("nvidia-smi");
                let metadata = std::fs::metadata(&executable).ok()?;
                (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
                    .then_some(executable)
            })
        });
    }

    #[cfg(not(target_os = "linux"))]
    None
}

fn host_os() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "macos",
        _ => "unsupported",
    }
}

fn host_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => "unsupported",
    }
}

fn catalog_error(_error: CatalogError) -> ServiceError {
    ServiceError::new(
        "local_engine_catalog_unavailable",
        "Local engine information could not be loaded.",
        false,
    )
}

fn unknown_engine_error() -> ServiceError {
    ServiceError::new(
        "local_engine_unavailable",
        "The selected local engine is unavailable.",
        false,
    )
}

fn unsupported_engine_variant_error() -> ServiceError {
    ServiceError::new(
        "local_engine_variant_unavailable",
        "The selected local engine package is unavailable.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        CATALOGS, NvidiaDriverStatus, catalog::DriverVersion, catalog::parse_manifest,
        parse_nvidia_driver_output, recommended_variant_id, variant_hardware_status,
    };

    #[test]
    fn every_embedded_engine_catalog_is_valid() {
        let catalogs = CATALOGS
            .iter()
            .map(|(engine_id, bytes)| {
                let manifest = parse_manifest(bytes)
                    .unwrap_or_else(|error| panic!("{engine_id} catalog is invalid: {error}"));
                assert_eq!(manifest.engine_id, *engine_id);
                manifest
            })
            .collect::<Vec<_>>();
        let engine_ids = catalogs
            .iter()
            .map(|manifest| manifest.engine_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(engine_ids, ["llama_cpp"]);
    }

    #[test]
    fn windows_llama_cpp_catalog_hides_non_windows_builds() {
        let manifest = CATALOGS
            .iter()
            .find(|(candidate_id, _)| *candidate_id == "llama_cpp")
            .and_then(|(_, bytes)| parse_manifest(bytes).ok())
            .expect("llama.cpp catalog should validate");
        let visible_variants = manifest
            .variants
            .iter()
            .filter(|variant| super::variant_visible_for_host("llama_cpp", variant, "windows"))
            .collect::<Vec<_>>();

        assert!(!visible_variants.is_empty());
        assert!(
            visible_variants
                .iter()
                .all(|variant| super::variant_host_os(variant) == "windows")
        );
        assert!(manifest.variants.iter().any(|variant| {
            super::variant_host_os(variant) == "linux"
                && !super::variant_visible_for_host("llama_cpp", variant, "windows")
        }));
        assert!(manifest.variants.iter().any(|variant| {
            super::variant_host_os(variant) == "macos"
                && !super::variant_visible_for_host("llama_cpp", variant, "windows")
        }));
    }

    fn llama_cpp_manifest() -> super::catalog::EngineManifest {
        parse_manifest(CATALOGS[0].1).expect("llama.cpp catalog should validate")
    }

    #[test]
    fn recommends_the_newest_cuda_variant_supported_by_the_driver_branch() {
        let manifest = llama_cpp_manifest();
        let driver = NvidiaDriverStatus::Detected(
            DriverVersion::parse("615.0").expect("valid synthetic driver version"),
        );

        assert_eq!(
            recommended_variant_id(&manifest, driver, "windows", "x86_64").as_deref(),
            Some("win-x86_64-cuda-13.4")
        );
    }

    #[test]
    fn recommends_cuda_12_when_the_13_4_driver_branch_is_not_met() {
        let manifest = llama_cpp_manifest();
        let driver = NvidiaDriverStatus::Detected(
            DriverVersion::parse("580.65").expect("valid synthetic driver version"),
        );

        assert_eq!(
            recommended_variant_id(&manifest, driver, "windows", "x86_64").as_deref(),
            Some("win-x86_64-cuda-12.4")
        );
    }

    #[test]
    fn compatibility_floor_does_not_automatically_mark_a_cuda_variant_recommended() {
        let manifest = llama_cpp_manifest();
        let cuda_12 = manifest
            .variants
            .iter()
            .find(|variant| variant.variant_id == "win-x86_64-cuda-12.4")
            .expect("CUDA 12.4 variant should exist");
        let cuda_13 = manifest
            .variants
            .iter()
            .find(|variant| variant.variant_id == "win-x86_64-cuda-13.4")
            .expect("CUDA 13.4 variant should exist");

        let driver = NvidiaDriverStatus::Detected(
            DriverVersion::parse("551.60").expect("valid synthetic driver version"),
        );
        assert_eq!(variant_hardware_status(cuda_12, driver), None);
        assert_eq!(
            recommended_variant_id(&manifest, driver, "windows", "x86_64").as_deref(),
            Some("win-x86_64-cpu")
        );

        let driver = NvidiaDriverStatus::Detected(
            DriverVersion::parse("551.61").expect("valid synthetic driver version"),
        );
        assert_eq!(
            recommended_variant_id(&manifest, driver, "windows", "x86_64").as_deref(),
            Some("win-x86_64-cuda-12.4")
        );

        let driver = NvidiaDriverStatus::Detected(
            DriverVersion::parse("580.0").expect("valid synthetic driver version"),
        );
        assert_eq!(variant_hardware_status(cuda_13, driver), None);
        assert_eq!(
            recommended_variant_id(&manifest, driver, "windows", "x86_64").as_deref(),
            Some("win-x86_64-cuda-12.4")
        );
    }

    #[test]
    fn recommends_cpu_when_cuda_is_too_old_or_driver_version_is_unknown() {
        let manifest = llama_cpp_manifest();
        let old_driver = NvidiaDriverStatus::Detected(
            DriverVersion::parse("527.0").expect("valid synthetic driver version"),
        );
        assert_eq!(
            recommended_variant_id(&manifest, old_driver, "windows", "x86_64").as_deref(),
            Some("win-x86_64-cpu")
        );
        assert_eq!(
            recommended_variant_id(
                &manifest,
                NvidiaDriverStatus::VersionUnavailable,
                "windows",
                "x86_64"
            )
            .as_deref(),
            Some("win-x86_64-cpu")
        );
    }

    #[test]
    fn cuda_installability_requires_a_verified_supported_driver() {
        let manifest = llama_cpp_manifest();
        let cuda_variant = manifest
            .variants
            .iter()
            .find(|variant| variant.variant_id == "win-x86_64-cuda-12.4")
            .expect("CUDA variant should exist");

        assert_eq!(
            variant_hardware_status(cuda_variant, NvidiaDriverStatus::NotDetected),
            Some("hardware_unavailable")
        );
        assert_eq!(
            variant_hardware_status(cuda_variant, NvidiaDriverStatus::VersionUnavailable),
            Some("driver_version_unavailable")
        );
        assert_eq!(
            variant_hardware_status(
                cuda_variant,
                NvidiaDriverStatus::Detected(
                    DriverVersion::parse("528.32").expect("valid synthetic driver version")
                )
            ),
            Some("driver_unsupported")
        );
        assert_eq!(
            variant_hardware_status(
                cuda_variant,
                NvidiaDriverStatus::Detected(
                    DriverVersion::parse("528.33").expect("valid synthetic driver version")
                )
            ),
            None
        );
    }

    #[test]
    fn parses_one_driver_version_shared_by_all_nvidia_devices() {
        assert_eq!(
            parse_nvidia_driver_output(" 551.61\r\n551.61\r\n"),
            Ok(Some(
                DriverVersion::parse("551.61").expect("valid synthetic driver version")
            ))
        );
    }

    #[test]
    fn rejects_missing_or_conflicting_nvidia_driver_versions() {
        assert_eq!(parse_nvidia_driver_output(" \r\n"), Ok(None));
        assert_eq!(parse_nvidia_driver_output("551.61\nnot-a-version"), Err(()));
        assert_eq!(parse_nvidia_driver_output("551.61\n552.1"), Err(()));
    }
}
