//! Verified, app-managed installation of local inference engine releases.
//!
//! The installer downloads only catalog-pinned assets, verifies their size and
//! digest, extracts ZIP archives into an isolated staging directory, and then
//! publishes the result with one directory rename. It never starts an engine.

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    fs as async_fs,
    io::AsyncWriteExt,
    sync::{Mutex, watch},
};
use uuid::Uuid;
use zip::ZipArchive;

use crate::{
    local_engines::catalog::{EngineAsset, EngineManifest, EngineVariant},
    protocol::{EventSink, Response, ServiceError},
    storage::AppStorage,
};

const INSTALL_PROGRESS_EVENT: &str = "local.engines.install.progress";
const INSTALL_MARKER: &str = ".openchat-engine.json";
const INSTALL_MARKER_VERSION: u32 = 1;
const MAX_ARCHIVE_ENTRIES: usize = 100_000;
const MAX_ARCHIVE_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const PROGRESS_INTERVAL_BYTES: u64 = 512 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

static INSTALL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// A published engine runtime that has passed the installation marker checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledEngine {
    pub engine_id: String,
    pub variant_id: String,
    pub release_tag: String,
    pub root: PathBuf,
    pub entrypoint: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstalledMarker {
    marker_version: u32,
    engine_id: String,
    variant_id: String,
    release_tag: String,
    entrypoint: String,
}

/// Install one validated catalog variant and publish it atomically.
pub async fn install_engine_variant(
    storage: &AppStorage,
    manifest: &EngineManifest,
    variant: &EngineVariant,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<InstalledEngine, ServiceError> {
    validate_install_request(manifest, variant)?;
    check_cancelled(cancellation)?;

    let _install_guard = install_lock().lock().await;
    if let Some(installed) = installed_engine(storage, manifest, variant)? {
        return Ok(installed);
    }
    check_cancelled(cancellation)?;

    let target = installed_engine_path(storage, manifest, variant)?;
    let runtimes_root = storage.root().join("runtimes");
    let staging_root = runtimes_root
        .join(".staging")
        .join(format!("install-{}", Uuid::new_v4()));
    let payload_root = staging_root.join("payload");
    let downloads_root = staging_root.join("downloads");

    let result = async {
        async_fs::create_dir_all(&downloads_root)
            .await
            .map_err(|_| storage_error())?;
        async_fs::create_dir_all(&payload_root)
            .await
            .map_err(|_| storage_error())?;

        install_staged(
            storage,
            manifest,
            variant,
            &target,
            &staging_root,
            &payload_root,
            &downloads_root,
            request_id,
            events,
            cancellation,
        )
        .await
    }
    .await;

    if result.is_err() {
        let _ = async_fs::remove_dir_all(&staging_root).await;
    }
    result
}

/// Return the deterministic publication path for a catalog variant.
pub fn installed_engine_path(
    storage: &AppStorage,
    manifest: &EngineManifest,
    variant: &EngineVariant,
) -> Result<PathBuf, ServiceError> {
    let engine_id = safe_component(&manifest.engine_id, "engineId")?;
    let variant_id = safe_component(&variant.variant_id, "variantId")?;
    let release_tag = safe_component(&manifest.release_tag, "releaseTag")?;
    Ok(storage
        .root()
        .join("runtimes")
        .join(engine_id)
        .join(variant_id)
        .join(release_tag))
}

/// Check the marker and entrypoint of an already-published runtime.
pub fn installed_engine(
    storage: &AppStorage,
    manifest: &EngineManifest,
    variant: &EngineVariant,
) -> Result<Option<InstalledEngine>, ServiceError> {
    let root = installed_engine_path(storage, manifest, variant)?;
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(installation_invalid_error());
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(installation_invalid_error());
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage_error()),
    }

    let marker_path = root.join(INSTALL_MARKER);
    if fs::symlink_metadata(&marker_path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(installation_invalid_error());
    }
    let marker_bytes = match fs::read(&marker_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage_error()),
    };
    let marker = serde_json::from_slice::<InstalledMarker>(&marker_bytes)
        .map_err(|_| installation_invalid_error())?;
    if marker.marker_version != INSTALL_MARKER_VERSION
        || marker.engine_id != manifest.engine_id
        || marker.variant_id != variant.variant_id
        || marker.release_tag != manifest.release_tag
        || marker.entrypoint != variant.entrypoint
    {
        return Err(installation_invalid_error());
    }
    let entrypoint_relative =
        safe_relative_path(&marker.entrypoint).map_err(|_| installation_invalid_error())?;
    let entrypoint = root.join(entrypoint_relative);
    if !entrypoint.is_file() {
        return Ok(None);
    }

    Ok(Some(InstalledEngine {
        engine_id: marker.engine_id,
        variant_id: marker.variant_id,
        release_tag: marker.release_tag,
        root,
        entrypoint,
    }))
}

async fn install_staged(
    storage: &AppStorage,
    manifest: &EngineManifest,
    variant: &EngineVariant,
    target: &Path,
    staging_root: &Path,
    payload_root: &Path,
    downloads_root: &Path,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<InstalledEngine, ServiceError> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(3_600))
        .build()
        .map_err(|_| download_error())?;
    let asset_count = variant.assets.len();

    for (index, asset) in variant.assets.iter().enumerate() {
        check_cancelled(cancellation)?;
        send_progress(
            request_id,
            events,
            manifest,
            variant,
            asset,
            index,
            asset_count,
            "downloading",
            0,
        )
        .await?;

        let archive_path = downloads_root.join(format!("asset-{index}.zip"));
        download_asset(
            &client,
            asset,
            &archive_path,
            request_id,
            events,
            manifest,
            variant,
            index,
            asset_count,
            cancellation,
        )
        .await?;
        check_cancelled(cancellation)?;

        send_progress(
            request_id,
            events,
            manifest,
            variant,
            asset,
            index,
            asset_count,
            "extracting",
            asset.size_bytes,
        )
        .await?;
        extract_zip_archive(&archive_path, payload_root)?;
        async_fs::remove_file(&archive_path)
            .await
            .map_err(|_| storage_error())?;
    }

    check_cancelled(cancellation)?;
    verify_required_files(payload_root, manifest, variant)?;
    write_install_marker(payload_root, manifest, variant).await?;
    check_cancelled(cancellation)?;

    if let Some(asset) = variant.assets.last() {
        send_progress(
            request_id,
            events,
            manifest,
            variant,
            asset,
            asset_count.saturating_sub(1),
            asset_count,
            "publishing",
            asset.size_bytes,
        )
        .await?;
    }

    let target_parent = target.parent().ok_or_else(storage_error)?;
    async_fs::create_dir_all(target_parent)
        .await
        .map_err(|_| storage_error())?;
    if target.exists() {
        return Err(installation_exists_error());
    }
    async_fs::rename(payload_root, target)
        .await
        .map_err(|_| publish_error())?;

    let installed = installed_engine(storage, manifest, variant)?.ok_or_else(|| publish_error())?;
    for asset in &variant.assets {
        send_progress(
            request_id,
            events,
            manifest,
            variant,
            asset,
            asset_count.saturating_sub(1),
            asset_count,
            "ready",
            asset.size_bytes,
        )
        .await?;
    }
    let _ = async_fs::remove_dir_all(staging_root).await;
    Ok(installed)
}

async fn download_asset(
    client: &Client,
    asset: &EngineAsset,
    destination: &Path,
    request_id: &Value,
    events: &EventSink,
    manifest: &EngineManifest,
    variant: &EngineVariant,
    asset_index: usize,
    asset_count: usize,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let response = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(cancelled_error());
        }
        response = client.get(&asset.url).send() => response.map_err(|_| download_error())?,
    };
    if !response.status().is_success() {
        return Err(download_error());
    }

    let mut response = response;
    let mut output = async_fs::File::create(destination)
        .await
        .map_err(|_| storage_error())?;
    let mut digest = Sha256::new();
    let mut received = 0_u64;
    let mut last_reported_bytes = 0_u64;
    let mut last_reported_at = Instant::now();

    loop {
        let next_chunk = tokio::select! {
            chunk = response.chunk() => chunk,
            changed = cancellation.changed() => {
                match changed {
                    Ok(()) if !*cancellation.borrow() => continue,
                    _ => return Err(cancelled_error()),
                }
            }
        };
        let Some(chunk) = next_chunk.map_err(|_| download_error())? else {
            break;
        };
        check_cancelled(cancellation)?;
        let chunk_len = u64::try_from(chunk.len()).map_err(|_| download_error())?;
        received = received
            .checked_add(chunk_len)
            .ok_or_else(|| download_error())?;
        if received > asset.size_bytes {
            return Err(integrity_error());
        }
        digest.update(&chunk);
        output
            .write_all(&chunk)
            .await
            .map_err(|_| storage_error())?;

        if received == asset.size_bytes
            || received.saturating_sub(last_reported_bytes) >= PROGRESS_INTERVAL_BYTES
            || last_reported_at.elapsed() >= PROGRESS_INTERVAL
        {
            send_progress(
                request_id,
                events,
                manifest,
                variant,
                asset,
                asset_index,
                asset_count,
                "downloading",
                received,
            )
            .await?;
            last_reported_bytes = received;
            last_reported_at = Instant::now();
        }
    }

    output.flush().await.map_err(|_| storage_error())?;
    output.sync_all().await.map_err(|_| storage_error())?;
    send_progress(
        request_id,
        events,
        manifest,
        variant,
        asset,
        asset_index,
        asset_count,
        "verifying",
        received,
    )
    .await?;
    check_cancelled(cancellation)?;
    if received != asset.size_bytes || !digest_matches(&asset.sha256, digest.finalize().as_slice())
    {
        return Err(integrity_error());
    }
    Ok(())
}

fn extract_zip_archive(archive_path: &Path, destination: &Path) -> Result<(), ServiceError> {
    let archive_file = File::open(archive_path).map_err(|_| archive_error())?;
    let mut archive = ZipArchive::new(archive_file).map_err(|_| archive_error())?;
    if archive.is_empty() || archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(archive_error());
    }

    let mut paths = HashSet::with_capacity(archive.len());
    let mut extracted_bytes = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| archive_error())?;
        if entry.is_symlink() {
            return Err(archive_error());
        }
        let relative_path = archive_relative_path(entry.name())?;
        if !paths.insert(relative_path.clone()) {
            return Err(archive_error());
        }
        let entry_size = entry.size();
        if entry_size > MAX_ARCHIVE_ENTRY_BYTES {
            return Err(archive_error());
        }
        extracted_bytes = extracted_bytes
            .checked_add(entry_size)
            .ok_or_else(archive_error)?;
        if extracted_bytes > MAX_EXTRACTED_BYTES {
            return Err(archive_error());
        }

        let output_path = destination.join(&relative_path);
        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|_| storage_error())?;
            continue;
        }
        let parent = output_path.parent().ok_or_else(archive_error)?;
        fs::create_dir_all(parent).map_err(|_| storage_error())?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output_path)
            .map_err(|_| storage_error())?;
        let copied = io::copy(&mut entry, &mut output).map_err(|_| storage_error())?;
        if copied != entry_size {
            return Err(archive_error());
        }
        output.sync_all().map_err(|_| storage_error())?;
    }
    Ok(())
}

fn verify_required_files(
    destination: &Path,
    manifest: &EngineManifest,
    variant: &EngineVariant,
) -> Result<(), ServiceError> {
    let entrypoint = safe_relative_path(&variant.entrypoint).map_err(|_| required_file_error())?;
    if !destination.join(entrypoint).is_file() {
        return Err(required_file_error());
    }
    for required_file in &variant.required_files {
        let relative = safe_relative_path(required_file).map_err(|_| required_file_error())?;
        if !destination.join(relative).is_file() {
            return Err(required_file_error());
        }
    }
    let _ = manifest;
    Ok(())
}

async fn write_install_marker(
    destination: &Path,
    manifest: &EngineManifest,
    variant: &EngineVariant,
) -> Result<(), ServiceError> {
    let marker = InstalledMarker {
        marker_version: INSTALL_MARKER_VERSION,
        engine_id: manifest.engine_id.clone(),
        variant_id: variant.variant_id.clone(),
        release_tag: manifest.release_tag.clone(),
        entrypoint: variant.entrypoint.clone(),
    };
    let encoded = serde_json::to_vec(&marker).map_err(|_| storage_error())?;
    let marker_path = destination.join(INSTALL_MARKER);
    let mut file = async_fs::File::create(marker_path)
        .await
        .map_err(|_| storage_error())?;
    file.write_all(&encoded)
        .await
        .map_err(|_| storage_error())?;
    file.sync_all().await.map_err(|_| storage_error())
}

async fn send_progress(
    request_id: &Value,
    events: &EventSink,
    manifest: &EngineManifest,
    variant: &EngineVariant,
    asset: &EngineAsset,
    asset_index: usize,
    asset_count: usize,
    phase: &str,
    downloaded_bytes: u64,
) -> Result<(), ServiceError> {
    events
        .send(&Response::event(
            request_id.clone(),
            INSTALL_PROGRESS_EVENT,
            json!({
                "phase": phase,
                "engineId": manifest.engine_id,
                "variantId": variant.variant_id,
                "assetName": asset.name,
                "assetIndex": asset_index,
                "assetCount": asset_count,
                "downloadedBytes": downloaded_bytes,
                "totalBytes": asset.size_bytes,
            }),
        ))
        .await
        .map_err(|_| progress_error())
}

fn validate_install_request(
    manifest: &EngineManifest,
    variant: &EngineVariant,
) -> Result<(), ServiceError> {
    if manifest.catalog_status != "installable" || variant.assets.is_empty() {
        return Err(not_installable_error());
    }
    safe_component(&manifest.engine_id, "engineId")?;
    safe_component(&manifest.release_tag, "releaseTag")?;
    safe_component(&variant.variant_id, "variantId")?;
    if !manifest
        .variants
        .iter()
        .any(|candidate| candidate.variant_id == variant.variant_id)
    {
        return Err(invalid_install_error());
    }
    safe_relative_path(&variant.entrypoint).map_err(|_| invalid_install_error())?;
    for required_file in &variant.required_files {
        safe_relative_path(required_file).map_err(|_| invalid_install_error())?;
    }
    let mut names = HashSet::with_capacity(variant.assets.len());
    let mut primary_count = 0;
    for asset in &variant.assets {
        if asset.name.trim().is_empty()
            || !names.insert(asset.name.clone())
            || asset.size_bytes == 0
        {
            return Err(invalid_install_error());
        }
        if asset.role == "primary" {
            primary_count += 1;
            if asset.companion_for.is_some() {
                return Err(invalid_install_error());
            }
        } else if asset.role == "companion" {
            let Some(companion_for) = asset.companion_for.as_deref() else {
                return Err(invalid_install_error());
            };
            if !manifest
                .variants
                .iter()
                .any(|candidate| candidate.variant_id == companion_for)
            {
                return Err(invalid_install_error());
            }
        } else {
            return Err(invalid_install_error());
        }
        if !asset.url.starts_with("https://github.com/")
            || !asset.url.contains("/releases/download/")
        {
            return Err(invalid_install_error());
        }
        validate_digest_text(&asset.sha256)?;
        if !asset.name.to_ascii_lowercase().ends_with(".zip") {
            return Err(unsupported_archive_error());
        }
    }
    if primary_count != 1 {
        return Err(invalid_install_error());
    }
    Ok(())
}

fn validate_digest_text(value: &str) -> Result<(), ServiceError> {
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    if digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        || digest.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(invalid_install_error());
    }
    Ok(())
}

fn digest_matches(expected: &str, actual: &[u8]) -> bool {
    let expected = expected.strip_prefix("sha256:").unwrap_or(expected);
    if expected.len() != 64 || expected.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return false;
    }
    let mut actual_hex = String::with_capacity(64);
    for byte in actual {
        actual_hex.push_str(&format!("{byte:02x}"));
    }
    actual_hex == expected
}

fn archive_relative_path(name: &str) -> Result<PathBuf, ServiceError> {
    if name.is_empty()
        || name.contains('\\')
        || name.contains('\0')
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(archive_error());
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return Err(archive_error());
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(archive_error());
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(archive_error());
    }
    Ok(relative)
}

fn safe_relative_path(value: &str) -> Result<PathBuf, ()> {
    if value.is_empty()
        || value.contains('\0')
        || value
            .split(['/', '\\'])
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(());
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(());
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(());
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(());
    }
    Ok(relative)
}

fn safe_component<'a>(value: &'a str, field: &str) -> Result<&'a str, ServiceError> {
    let mut components = Path::new(value).components();
    let is_single_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if !is_single_normal_component
        || value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.chars().any(|character| character.is_control())
    {
        return Err(ServiceError::new(
            "local_engine_invalid_catalog",
            format!("The local engine {field} is invalid."),
            false,
        ));
    }
    Ok(value)
}

fn check_cancelled(cancellation: &watch::Receiver<bool>) -> Result<(), ServiceError> {
    if *cancellation.borrow() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn install_lock() -> &'static Mutex<()> {
    INSTALL_LOCK.get_or_init(|| Mutex::new(()))
}

fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "The local engine installation was cancelled.",
        false,
    )
}

fn storage_error() -> ServiceError {
    ServiceError::new(
        "local_engine_storage_failed",
        "Local engine files could not be prepared.",
        true,
    )
}

fn download_error() -> ServiceError {
    ServiceError::new(
        "local_engine_download_failed",
        "The local engine release could not be downloaded.",
        true,
    )
}

fn integrity_error() -> ServiceError {
    ServiceError::new(
        "local_engine_integrity_failed",
        "The local engine release failed its integrity check.",
        false,
    )
}

fn archive_error() -> ServiceError {
    ServiceError::new(
        "local_engine_archive_invalid",
        "The local engine archive is invalid or unsafe.",
        false,
    )
}

fn required_file_error() -> ServiceError {
    ServiceError::new(
        "local_engine_required_file_missing",
        "The local engine archive is missing a required file.",
        false,
    )
}

fn unsupported_archive_error() -> ServiceError {
    ServiceError::new(
        "local_engine_archive_unsupported",
        "This local engine release archive format is not supported yet.",
        false,
    )
}

fn not_installable_error() -> ServiceError {
    ServiceError::new(
        "local_engine_not_installable",
        "This local engine variant is not available for managed installation.",
        false,
    )
}

fn invalid_install_error() -> ServiceError {
    ServiceError::new(
        "local_engine_invalid_catalog",
        "The local engine catalog entry is invalid.",
        false,
    )
}

fn installation_invalid_error() -> ServiceError {
    ServiceError::new(
        "local_engine_installation_invalid",
        "The installed local engine runtime is incomplete or invalid.",
        false,
    )
}

fn installation_exists_error() -> ServiceError {
    ServiceError::new(
        "local_engine_installation_exists",
        "A local engine runtime already exists but could not be verified.",
        false,
    )
}

fn publish_error() -> ServiceError {
    ServiceError::new(
        "local_engine_publish_failed",
        "The verified local engine runtime could not be activated.",
        true,
    )
}

fn progress_error() -> ServiceError {
    ServiceError::new(
        "local_engine_progress_failed",
        "Local engine installation progress could not be delivered.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use uuid::Uuid;

    use super::{
        INSTALL_MARKER, INSTALL_MARKER_VERSION, InstalledMarker, archive_relative_path,
        installed_engine, installed_engine_path,
    };
    use crate::{local_engines::catalog::parse_manifest, storage::AppStorage};

    const LLAMA_CPP_MANIFEST: &[u8] =
        include_bytes!("../../resources/local-engines/llama_cpp.json");

    #[test]
    fn rejects_unsafe_archive_paths() {
        assert!(archive_relative_path("../payload.exe").is_err());
        assert!(archive_relative_path("/absolute/payload.exe").is_err());
        assert!(archive_relative_path("C:/payload.exe").is_err());
        assert!(archive_relative_path("folder\\payload.exe").is_err());
        assert!(archive_relative_path("folder/./payload.exe").is_err());
        assert!(archive_relative_path("folder/payload.exe").is_ok());
    }

    #[test]
    fn installed_marker_and_path_are_verified() {
        let root = std::env::temp_dir().join(format!("openchat-engine-test-{}", Uuid::new_v4()));
        let storage = AppStorage::open_at(root.clone()).expect("test storage should open");
        let manifest = parse_manifest(LLAMA_CPP_MANIFEST).expect("catalog should validate");
        let variant = &manifest.variants[0];
        let runtime_path = installed_engine_path(&storage, &manifest, variant)
            .expect("catalog path should be safe");
        fs::create_dir_all(&runtime_path).expect("runtime path should be creatable");
        let entrypoint = runtime_path.join(&variant.entrypoint);
        fs::write(&entrypoint, b"test").expect("entrypoint should be writable");
        let marker = InstalledMarker {
            marker_version: INSTALL_MARKER_VERSION,
            engine_id: manifest.engine_id.clone(),
            variant_id: variant.variant_id.clone(),
            release_tag: manifest.release_tag.clone(),
            entrypoint: variant.entrypoint.clone(),
        };
        fs::write(
            runtime_path.join(INSTALL_MARKER),
            serde_json::to_vec(&marker).expect("marker should serialize"),
        )
        .expect("marker should be writable");

        let installed = installed_engine(&storage, &manifest, variant)
            .expect("marker should be readable")
            .expect("runtime should be reported as installed");
        assert_eq!(installed.root, runtime_path);
        assert_eq!(installed.entrypoint, entrypoint);

        drop(storage);
        fs::remove_dir_all(PathBuf::from(root)).expect("test storage should be removed");
    }

    #[test]
    fn marker_with_wrong_engine_is_rejected() {
        let root = std::env::temp_dir().join(format!("openchat-engine-test-{}", Uuid::new_v4()));
        let storage = AppStorage::open_at(root.clone()).expect("test storage should open");
        let manifest = parse_manifest(LLAMA_CPP_MANIFEST).expect("catalog should validate");
        let variant = &manifest.variants[0];
        let runtime_path = installed_engine_path(&storage, &manifest, variant)
            .expect("catalog path should be safe");
        fs::create_dir_all(&runtime_path).expect("runtime path should be creatable");
        let marker = serde_json::json!({
            "markerVersion": INSTALL_MARKER_VERSION,
            "engineId": "wrong",
            "variantId": variant.variant_id,
            "releaseTag": manifest.release_tag,
            "entrypoint": variant.entrypoint,
        });
        fs::write(
            runtime_path.join(INSTALL_MARKER),
            serde_json::to_vec(&marker).expect("marker should serialize"),
        )
        .expect("marker should be writable");

        assert!(installed_engine(&storage, &manifest, variant).is_err());

        drop(storage);
        fs::remove_dir_all(root).expect("test storage should be removed");
    }
}
