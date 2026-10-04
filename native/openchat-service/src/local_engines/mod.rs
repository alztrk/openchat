//! App-managed local inference engine releases.

mod catalog;
mod gguf;
pub(crate) mod installer;
pub(crate) mod models;
mod runtime;

use std::time::Duration;

use serde_json::{Value, json};
use tokio::{process::Command, time::timeout};

use crate::{protocol::ServiceError, storage::AppStorage};

use catalog::{CatalogError, EngineManifest, EngineVariant, parse_manifest};

const CATALOGS: &[(&str, &[u8])] = &[
    (
        "llama_cpp",
        include_bytes!("../../resources/local-engines/llama_cpp.json"),
    ),
    (
        "vllm",
        include_bytes!("../../resources/local-engines/vllm.json"),
    ),
    (
        "exllama",
        include_bytes!("../../resources/local-engines/exllama.json"),
    ),
];

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
    let cuda_available = nvidia_gpu_available().await;
    let runtime = runtime::status(storage).await?;
    let registered_models = models::list(storage)?;
    let engines = manifests()?
        .into_iter()
        .map(|manifest| {
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
            let variants = manifest
                .variants
                .iter()
                .map(|variant| variant_json(storage, &manifest, variant, cuda_available))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({
                "engineId": manifest.engine_id,
                "displayName": manifest.display_name,
                "releaseTag": manifest.release_tag,
                "channel": manifest.channel,
                "catalogStatus": manifest.catalog_status,
                "statusReason": manifest.status_reason,
                "modelDirectory": model_directory,
                "runtimeStatus": if manifest.engine_id == "llama_cpp" {
                    runtime.get("status").cloned().unwrap_or(Value::Null)
                } else {
                    json!("unavailable")
                },
                "variants": variants,
            }))
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;

    Ok(json!({
        "engines": engines,
        "models": registered_models.iter().map(|model| {
            Ok(models::to_json(model, runtime::is_model_available(storage, model)?))
        }).collect::<Result<Vec<_>, ServiceError>>()?,
        "runtime": runtime,
        "host": {
            "os": host_os(),
            "architecture": host_architecture(),
            "cudaAvailable": cuda_available,
        },
    }))
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

pub(crate) async fn chat_url(
    storage: &AppStorage,
    model_id: &str,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<runtime::RuntimeChatEndpoint, ServiceError> {
    runtime::chat_url(storage, model_id, cancellation).await
}

pub(crate) fn model_catalog(storage: &AppStorage) -> Result<Value, ServiceError> {
    let installed = installed_engine_ids(storage)?;
    let models = models::list(storage)?;
    Ok(json!({
        "freshness": "current",
        "models": models.iter().map(|model| {
            let path_available = if model.path_kind == "file" {
                model.path.is_file()
            } else {
                model.path.is_dir()
            };
            let available = path_available && installed.contains(&model.engine_id);
            models::to_json(model, available)
        }).collect::<Vec<_>>(),
    }))
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
    Ok(models::to_json(&model, available))
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
    let state = runtime::status(storage).await?;
    if state.get("modelId").and_then(Value::as_str) == Some(model_id) {
        runtime::stop(storage).await?;
    }
    Ok(json!({"removed": models::remove(storage, model_id)?}))
}

fn installed_engine_ids(storage: &AppStorage) -> Result<Vec<String>, ServiceError> {
    let mut engine_ids = Vec::new();
    for manifest in manifests()? {
        if manifest.catalog_status != "installable" {
            continue;
        }
        for variant in &manifest.variants {
            if installer::installed_engine(storage, &manifest, variant)?.is_some() {
                engine_ids.push(manifest.engine_id.clone());
                break;
            }
        }
    }
    Ok(engine_ids)
}

pub(crate) fn find_variant(
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
    manifest.catalog_status == "installable"
        && cfg!(windows)
        && variant.os == host_os()
        && variant.architecture == host_architecture()
        && (variant.accelerator != "cuda" || nvidia_gpu_available().await)
}

fn variant_json(
    storage: &AppStorage,
    manifest: &EngineManifest,
    variant: &EngineVariant,
    cuda_available: bool,
) -> Result<Value, ServiceError> {
    let installed = installer::installed_engine(storage, manifest, variant)?.is_some();
    let platform_matches =
        cfg!(windows) && variant.os == host_os() && variant.architecture == host_architecture();
    let hardware_matches = variant.accelerator != "cuda" || cuda_available;
    let can_install = manifest.catalog_status == "installable"
        && platform_matches
        && hardware_matches
        && !installed;
    let status = if installed {
        "installed"
    } else if manifest.catalog_status == "blocked" {
        "blocked"
    } else if !platform_matches {
        "unsupported_platform"
    } else if !hardware_matches {
        "hardware_unavailable"
    } else {
        "available"
    };
    let matching_accelerator_variants = manifest
        .variants
        .iter()
        .filter(|candidate| {
            candidate.os == variant.os
                && candidate.architecture == variant.architecture
                && candidate.accelerator == variant.accelerator
        })
        .count();
    let accelerator_recommended = match variant.accelerator.as_str() {
        "cuda" => cuda_available && matching_accelerator_variants == 1,
        "cpu" => !cuda_available,
        _ => false,
    };
    Ok(json!({
        "variantId": variant.variant_id,
        "os": variant.os,
        "architecture": variant.architecture,
        "accelerator": variant.accelerator,
        "runtimeRequirements": variant.runtime_requirements,
        "canInstall": can_install,
        "recommended": accelerator_recommended,
        "installed": installed,
        "status": status,
    }))
}

async fn nvidia_gpu_available() -> bool {
    let Some(executable) = nvidia_driver_executable() else {
        return false;
    };
    timeout(
        Duration::from_secs(2),
        Command::new(executable)
            .args(["--query-gpu=name", "--format=csv,noheader"])
            .output(),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .is_some_and(|output| {
        output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty()
    })
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
    use super::{CATALOGS, catalog::parse_manifest};

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

        assert_eq!(engine_ids, ["llama_cpp", "vllm", "exllama"]);
    }
}
