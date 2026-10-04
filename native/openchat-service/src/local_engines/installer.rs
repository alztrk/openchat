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
    local_engines::catalog::{
        EngineAsset, EngineManifest, EngineVariant, validate_release_asset_url,
    },
    protocol::{EventSink, Response, ServiceError},
    storage::AppStorage,
};

const INSTALL_PROGRESS_EVENT: &str = "local.engines.install.progress";
const INSTALL_MARKER: &str = ".openchat-engine.json";
const PENDING_INSTALL_MARKER: &str = ".openchat-installing.json";
const INSTALL_MARKER_VERSION: u32 = 1;
const MAX_ARCHIVE_ENTRIES: usize = 100_000;
const MAX_ARCHIVE_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const PROGRESS_INTERVAL_BYTES: u64 = 512 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PendingInstallMarker {
    marker_version: u32,
    engine_id: String,
    variant_id: String,
    release_tag: String,
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
    if uses_managed_python_runtime(&manifest.engine_id) && target.exists() {
        remove_interrupted_python_installation(&target, manifest, variant)?;
    }
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
            &payload_root,
            &downloads_root,
            request_id,
            events,
            cancellation,
        )
        .await
    }
    .await;

    match result {
        Ok(installed) => {
            remove_directory_if_present(&staging_root).await?;
            for asset in &variant.assets {
                send_progress(
                    request_id,
                    events,
                    manifest,
                    variant,
                    asset,
                    variant.assets.len().saturating_sub(1),
                    variant.assets.len(),
                    "ready",
                    asset.size_bytes,
                )
                .await?;
            }
            Ok(installed)
        }
        Err(error) => {
            remove_directory_if_present(&staging_root).await?;
            Err(error)
        }
    }
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

        let archive_path = downloads_root.join(format!("asset-{index}.download"));
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
        extract_downloaded_asset(&archive_path, payload_root, manifest, asset)?;
        if asset.name.to_ascii_lowercase().ends_with(".whl") {
            let wheel_directory = payload_root.join("wheels");
            async_fs::create_dir_all(&wheel_directory)
                .await
                .map_err(|_| storage_error())?;
            async_fs::rename(&archive_path, wheel_directory.join(&asset.name))
                .await
                .map_err(|_| storage_error())?;
        } else {
            async_fs::remove_file(&archive_path)
                .await
                .map_err(|_| storage_error())?;
        }
    }

    check_cancelled(cancellation)?;

    let published_root = if uses_managed_python_runtime(&manifest.engine_id) {
        if manifest.engine_id == "exllama" {
            normalize_exllama_source_tree(payload_root, variant)?;
        }
        write_pending_install_marker(payload_root, manifest, variant).await?;
        let target_parent = target.parent().ok_or_else(storage_error)?;
        async_fs::create_dir_all(target_parent)
            .await
            .map_err(|_| storage_error())?;
        if target.exists() {
            return Err(installation_exists_error());
        }
        async_fs::rename(payload_root, &target)
            .await
            .map_err(|_| publish_error())?;
        if let Some(asset) = variant.assets.last() {
            send_progress(
                request_id,
                events,
                manifest,
                variant,
                asset,
                asset_count.saturating_sub(1),
                asset_count,
                "setting_up_runtime",
                0,
            )
            .await?;
        }
        let install_result = match manifest.engine_id.as_str() {
            "exllama" => {
                super::python_runtime::install_exllama(target, variant, cancellation).await
            }
            "vllm" => super::vllm_runtime::install_vllm(target, variant, cancellation).await,
            _ => Err(invalid_install_error()),
        };
        if let Err(error) = install_result {
            async_fs::remove_dir_all(&target)
                .await
                .map_err(|_| storage_error())?;
            return Err(error);
        }
        target
    } else {
        payload_root
    };

    check_cancelled(cancellation)?;
    verify_required_files(published_root, manifest, variant)?;
    write_install_marker(published_root, manifest, variant).await?;
    if uses_managed_python_runtime(&manifest.engine_id) {
        async_fs::remove_file(published_root.join(PENDING_INSTALL_MARKER))
            .await
            .map_err(|_| storage_error())?;
    }
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

    if !uses_managed_python_runtime(&manifest.engine_id) {
        let target_parent = target.parent().ok_or_else(storage_error)?;
        async_fs::create_dir_all(target_parent)
            .await
            .map_err(|_| storage_error())?;
        if target.exists() {
            return Err(installation_exists_error());
        }
        async_fs::rename(payload_root, &target)
            .await
            .map_err(|_| publish_error())?;
    }

    let installed = installed_engine(storage, manifest, variant)?.ok_or_else(publish_error)?;
    Ok(installed)
}

async fn remove_directory_if_present(path: &Path) -> Result<(), ServiceError> {
    match async_fs::try_exists(path).await {
        Ok(false) => Ok(()),
        Ok(true) => async_fs::remove_dir_all(path)
            .await
            .map_err(|_| storage_error()),
        Err(_) => Err(storage_error()),
    }
}

fn remove_interrupted_python_installation(
    target: &Path,
    manifest: &EngineManifest,
    variant: &EngineVariant,
) -> Result<(), ServiceError> {
    let metadata = fs::symlink_metadata(target).map_err(|_| storage_error())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(installation_exists_error());
    }
    let pending_path = target.join(PENDING_INSTALL_MARKER);
    let pending = fs::read(&pending_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PendingInstallMarker>(&bytes).ok());
    let Some(pending) = pending else {
        return Err(installation_exists_error());
    };
    if pending.marker_version != INSTALL_MARKER_VERSION
        || pending.engine_id != manifest.engine_id
        || pending.variant_id != variant.variant_id
        || pending.release_tag != manifest.release_tag
    {
        return Err(installation_exists_error());
    }
    fs::remove_dir_all(target).map_err(|_| storage_error())
}

fn uses_managed_python_runtime(engine_id: &str) -> bool {
    matches!(engine_id, "exllama" | "vllm")
}

fn extract_downloaded_asset(
    archive_path: &Path,
    destination: &Path,
    manifest: &EngineManifest,
    asset: &EngineAsset,
) -> Result<(), ServiceError> {
    let name = asset.name.to_ascii_lowercase();
    if name.ends_with(".whl") {
        if !uses_managed_python_runtime(&manifest.engine_id) || asset.role != "primary" {
            return Err(unsupported_archive_error());
        }
        return Ok(());
    }
    if name.ends_with(".zip") {
        return extract_zip_archive(archive_path, destination);
    }
    if name.ends_with(".tar.gz") {
        return extract_tar_gz_archive(archive_path, destination);
    }
    Err(unsupported_archive_error())
}

fn extract_tar_gz_archive(archive_path: &Path, destination: &Path) -> Result<(), ServiceError> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let archive_file = File::open(archive_path).map_err(|_| archive_error())?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = Archive::new(decoder);
    let entries = archive.entries().map_err(|_| archive_error())?;
    let mut paths = HashSet::new();
    let mut extracted_bytes = 0_u64;
    let mut entry_count = 0_usize;

    for entry in entries {
        entry_count = entry_count.checked_add(1).ok_or_else(archive_error)?;
        if entry_count > MAX_ARCHIVE_ENTRIES {
            return Err(archive_error());
        }
        let mut entry = entry.map_err(|_| archive_error())?;
        let entry_type = entry.header().entry_type();
        if !(entry_type.is_file() || entry_type.is_dir()) {
            return Err(archive_error());
        }
        let is_directory = entry_type.is_dir();
        let raw_path = entry
            .path()
            .map_err(|_| archive_error())?
            .into_owned()
            .into_os_string()
            .into_string()
            .map_err(|_| archive_error())?;
        let relative_path = archive_relative_path(&raw_path, is_directory)?;
        if !paths.insert(relative_path.clone()) {
            return Err(archive_error());
        }
        let entry_size = entry.header().size().map_err(|_| archive_error())?;
        if entry_size > MAX_ARCHIVE_ENTRY_BYTES {
            return Err(archive_error());
        }
        extracted_bytes = extracted_bytes
            .checked_add(entry_size)
            .ok_or_else(archive_error)?;
        if extracted_bytes > MAX_EXTRACTED_BYTES {
            return Err(archive_error());
        }

        let output_path = destination.join(relative_path);
        if entry_type.is_dir() {
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = entry.header().mode().map_err(|_| archive_error())?;
            fs::set_permissions(&output_path, fs::Permissions::from_mode(mode & 0o777))
                .map_err(|_| storage_error())?;
        }
    }
    if entry_count == 0 {
        return Err(archive_error());
    }
    Ok(())
}

fn normalize_exllama_source_tree(
    payload_root: &Path,
    variant: &EngineVariant,
) -> Result<(), ServiceError> {
    let commit = variant
        .assets
        .iter()
        .find(|asset| asset.role == "source")
        .and_then(|asset| asset.source_commit.as_deref())
        .ok_or_else(invalid_install_error)?;
    let source_root = payload_root.join(format!("tabbyAPI-{commit}"));
    let expected_start = source_root.join("start.py");
    if !expected_start.is_file() || payload_root.join("tabbyAPI").exists() {
        return Err(required_file_error());
    }
    fs::rename(source_root, payload_root.join("tabbyAPI")).map_err(|_| storage_error())
}

async fn write_pending_install_marker(
    destination: &Path,
    manifest: &EngineManifest,
    variant: &EngineVariant,
) -> Result<(), ServiceError> {
    let marker = PendingInstallMarker {
        marker_version: INSTALL_MARKER_VERSION,
        engine_id: manifest.engine_id.clone(),
        variant_id: variant.variant_id.clone(),
        release_tag: manifest.release_tag.clone(),
    };
    let encoded = serde_json::to_vec(&marker).map_err(|_| storage_error())?;
    let marker_path = destination.join(PENDING_INSTALL_MARKER);
    let mut file = async_fs::File::create(marker_path)
        .await
        .map_err(|_| storage_error())?;
    file.write_all(&encoded)
        .await
        .map_err(|_| storage_error())?;
    file.sync_all().await.map_err(|_| storage_error())
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
            chunk = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, response.chunk()) => {
                match chunk {
                    Ok(chunk) => chunk,
                    Err(_) => return Err(download_timeout_error()),
                }
            },
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
        received = received.checked_add(chunk_len).ok_or_else(download_error)?;
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
        let relative_path = archive_relative_path(entry.name(), entry.is_dir())?;
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
    let is_runtime_setup = phase == "setting_up_runtime";
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
                "downloadedBytes": if is_runtime_setup { 0 } else { downloaded_bytes },
                "totalBytes": if is_runtime_setup { 0 } else { asset.size_bytes },
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
        .any(|candidate| candidate == variant)
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
        } else if asset.role == "source" {
            if asset.companion_for.is_some() {
                return Err(invalid_install_error());
            }
        } else {
            return Err(invalid_install_error());
        }
        let is_pinned_source_archive = if asset.role == "source" {
            asset
                .source_repository
                .as_deref()
                .zip(asset.source_commit.as_deref())
                .is_some_and(|(repository, commit)| {
                    asset.url == format!("https://github.com/{repository}/archive/{commit}.zip")
                })
        } else {
            false
        };
        let is_pinned_release_asset = if asset.role != "source" {
            asset
                .release_repository
                .as_deref()
                .zip(asset.release_tag.as_deref())
                .is_some_and(|(repository, tag)| {
                    validate_release_asset_url(&asset.url, repository, tag, &asset.name).is_ok()
                })
        } else {
            false
        };
        if !is_pinned_release_asset && !is_pinned_source_archive {
            return Err(invalid_install_error());
        }
        validate_digest_text(&asset.sha256)?;
        let name = asset.name.to_ascii_lowercase();
        let supported_asset = name.ends_with(".zip")
            || name.ends_with(".tar.gz")
            || (uses_managed_python_runtime(&manifest.engine_id)
                && asset.role == "primary"
                && name.ends_with(".whl"));
        if !supported_asset {
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

fn archive_relative_path(name: &str, is_directory: bool) -> Result<PathBuf, ServiceError> {
    let name = if is_directory {
        name.strip_suffix('/').unwrap_or(name)
    } else {
        name
    };
    if name.is_empty()
        || name.contains('\\')
        || name.contains('\0')
        || name.split('/').enumerate().any(|(index, part)| {
            part.is_empty() || part == "." || part == ".." || (index == 0 && part.contains(':'))
        })
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

fn download_timeout_error() -> ServiceError {
    ServiceError::new(
        "local_engine_download_stalled",
        "The local engine download stopped receiving data for 60 seconds.",
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
    use std::fs;

    use flate2::{Compression, write::GzEncoder};
    use uuid::Uuid;

    use super::{
        INSTALL_MARKER, INSTALL_MARKER_VERSION, InstalledMarker, archive_relative_path,
        extract_downloaded_asset, extract_tar_gz_archive, installed_engine, installed_engine_path,
        validate_install_request,
    };
    use crate::{local_engines::catalog::parse_manifest, storage::AppStorage};

    const LLAMA_CPP_MANIFEST: &[u8] =
        include_bytes!("../../resources/local-engines/llama_cpp.json");
    const VLLM_MANIFEST: &[u8] = include_bytes!("../../resources/local-engines/vllm.json");

    #[test]
    fn validates_locked_vllm_assets_for_linux_and_windows_wsl() {
        let manifest = parse_manifest(VLLM_MANIFEST).expect("vLLM catalog should validate");
        for variant_id in ["linux-x86_64-cuda13", "windows-wsl2-linux-x86_64-cuda13"] {
            let variant = manifest
                .variants
                .iter()
                .find(|variant| variant.variant_id == variant_id)
                .expect("supported vLLM variant should exist");
            validate_install_request(&manifest, variant)
                .expect("pinned release assets should pass installer validation");
            let wheel = variant
                .assets
                .iter()
                .find(|asset| asset.name.ends_with(".whl"))
                .expect("vLLM wheel should be pinned");
            extract_downloaded_asset(
                std::path::Path::new("not-read-for-verified-wheel"),
                std::path::Path::new("unused-destination"),
                &manifest,
                wheel,
            )
            .expect("verified vLLM wheel should be retained for managed installation");
        }
    }

    #[test]
    fn rejects_unsafe_archive_paths() {
        assert!(archive_relative_path("../payload.exe", false).is_err());
        assert!(archive_relative_path("/absolute/payload.exe", false).is_err());
        assert!(archive_relative_path("C:/payload.exe", false).is_err());
        assert!(archive_relative_path("folder\\payload.exe", false).is_err());
        assert!(archive_relative_path("folder/./payload.exe", false).is_err());
        assert!(archive_relative_path("folder/payload.exe", false).is_ok());
        assert!(archive_relative_path("folder/", true).is_ok());
        assert!(archive_relative_path("folder//", true).is_err());
    }

    #[test]
    fn extracts_regular_files_from_a_gzip_tar_archive() {
        let root = std::env::temp_dir().join(format!("openchat-tar-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test directory should be created");
        let archive_path = root.join("uv.tar.gz");
        let archive_file = fs::File::create(&archive_path).expect("archive file should be created");
        let encoder = GzEncoder::new(archive_file, Compression::default());
        let mut archive = tar::Builder::new(encoder);
        let contents = b"verified test executable";
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        archive
            .append_data(&mut header, "uv-x86_64-unknown-linux-gnu/uv", &contents[..])
            .expect("archive entry should be written");
        archive
            .into_inner()
            .expect("tar archive should finish")
            .finish()
            .expect("gzip archive should finish");

        let destination = root.join("extracted");
        fs::create_dir_all(&destination).expect("extraction directory should be created");
        extract_tar_gz_archive(&archive_path, &destination)
            .expect("the regular archive entry should extract");
        assert_eq!(
            fs::read(destination.join("uv-x86_64-unknown-linux-gnu/uv"))
                .expect("the extracted file should be readable"),
            contents
        );
        fs::remove_dir_all(root).expect("test directory should be removed");
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
        fs::remove_dir_all(root).expect("test storage should be removed");
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
