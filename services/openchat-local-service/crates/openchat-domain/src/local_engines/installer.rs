//! Verified, app-managed installation of local inference engine releases.
//!
//! The installer downloads only catalog-pinned assets, verifies their size and
//! digest, extracts ZIP archives into an isolated staging directory, and then
//! publishes the result with one directory rename. It never starts an engine.

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
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
use url::Url;
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
const INSTALL_MARKER_VERSION: u32 = 1;
const MAX_INSTALL_MARKER_BYTES: usize = 16 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 100_000;
const MAX_ARCHIVE_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const PROGRESS_INTERVAL_BYTES: u64 = 512 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_ENGINE_ASSET_REDIRECTS: usize = 10;

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

/// Keeps progress events tied to the catalog entry and request that started installation.
struct InstallProgressContext<'a> {
    request_id: &'a Value,
    events: &'a EventSink,
    manifest: &'a EngineManifest,
    variant: &'a EngineVariant,
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
    let progress = InstallProgressContext {
        request_id,
        events,
        manifest,
        variant,
    };

    let _install_guard = install_lock().lock().await;
    if let Some(installed) = installed_engine(storage, manifest, variant)?
        && installed.release_tag == manifest.release_tag
    {
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
            &target,
            &payload_root,
            &downloads_root,
            &progress,
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
                    &progress,
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
    let releases_root = storage
        .root()
        .join("runtimes")
        .join(safe_component(&manifest.engine_id, "engineId")?)
        .join(safe_component(&variant.variant_id, "variantId")?);
    match fs::symlink_metadata(&releases_root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(installation_invalid_error());
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage_error()),
    }

    let mut installed = Vec::new();
    for entry in fs::read_dir(&releases_root).map_err(|_| storage_error())? {
        let entry = entry.map_err(|_| storage_error())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| storage_error())?;
        if metadata.file_type().is_symlink() {
            return Err(installation_invalid_error());
        }
        if !metadata.is_dir() {
            continue;
        }
        let root = entry.path();
        let marker_path = root.join(INSTALL_MARKER);
        match fs::symlink_metadata(&marker_path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(installation_invalid_error());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(storage_error()),
        }
        let marker = read_installed_marker(&marker_path)?;
        let directory_tag = entry
            .file_name()
            .into_string()
            .map_err(|_| installation_invalid_error())?;
        if marker.marker_version != INSTALL_MARKER_VERSION
            || marker.engine_id != manifest.engine_id
            || marker.variant_id != variant.variant_id
            || marker.release_tag != directory_tag
            || marker.entrypoint != variant.entrypoint
        {
            return Err(installation_invalid_error());
        }
        let entrypoint_relative =
            safe_relative_path(&marker.entrypoint).map_err(|_| installation_invalid_error())?;
        let entrypoint = root.join(entrypoint_relative);
        if !entrypoint.is_file() {
            continue;
        }
        installed.push(InstalledEngine {
            engine_id: marker.engine_id,
            variant_id: marker.variant_id,
            release_tag: marker.release_tag,
            root,
            entrypoint,
        });
    }
    installed.sort_by(|left, right| compare_release_tags(&left.release_tag, &right.release_tag));
    Ok(installed.pop())
}

fn read_installed_marker(path: &Path) -> Result<InstalledMarker, ServiceError> {
    let file = File::open(path).map_err(|_| storage_error())?;
    let read_limit = u64::try_from(MAX_INSTALL_MARKER_BYTES + 1).map_err(|_| storage_error())?;
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|_| storage_error())?;
    if bytes.len() > MAX_INSTALL_MARKER_BYTES {
        return Err(installation_invalid_error());
    }
    serde_json::from_slice(&bytes).map_err(|_| installation_invalid_error())
}

fn compare_release_tags(left: &str, right: &str) -> std::cmp::Ordering {
    let parts = |tag: &str| {
        tag.strip_prefix('v')
            .unwrap_or(tag)
            .split('.')
            .map(|part| part.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parts(left).cmp(&parts(right))
}

async fn install_staged(
    storage: &AppStorage,
    target: &Path,
    payload_root: &Path,
    downloads_root: &Path,
    progress: &InstallProgressContext<'_>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<InstalledEngine, ServiceError> {
    let manifest = progress.manifest;
    let variant = progress.variant;
    let client = installer_client()?;
    let asset_count = variant.assets.len();

    for (index, asset) in variant.assets.iter().enumerate() {
        check_cancelled(cancellation)?;
        send_progress(progress, asset, index, asset_count, "downloading", 0).await?;

        let archive_path = downloads_root.join(format!("asset-{index}.download"));
        download_asset(
            &client,
            asset,
            &archive_path,
            progress,
            index,
            asset_count,
            cancellation,
        )
        .await?;
        check_cancelled(cancellation)?;

        send_progress(
            progress,
            asset,
            index,
            asset_count,
            "extracting",
            asset.size_bytes,
        )
        .await?;
        extract_downloaded_asset_async(&archive_path, payload_root, &asset.name).await?;
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
            progress,
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
    async_fs::rename(payload_root, &target)
        .await
        .map_err(|_| publish_error())?;

    let installed = installed_engine(storage, manifest, variant)?.ok_or_else(publish_error)?;
    Ok(installed)
}

fn installer_client() -> Result<Client, ServiceError> {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(3_600))
        .redirect(engine_asset_redirect_policy())
        .build()
        .map_err(|_| download_error())
}

/// Follows only HTTPS redirects to GitHub's release page and asset CDN.
fn engine_asset_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() > MAX_ENGINE_ASSET_REDIRECTS {
            attempt.error("GitHub release asset redirected too many times")
        } else if is_approved_engine_asset_redirect(attempt.url()) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

/// Checks the scheme, authority, port, and host of a release-asset redirect.
fn is_approved_engine_asset_redirect(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && (host == "github.com" || host == "release-assets.githubusercontent.com")
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

/// Extracts a supported release archive into the caller's isolated staging directory.
fn extract_downloaded_asset(
    archive_path: &Path,
    destination: &Path,
    asset_name: &str,
) -> Result<(), ServiceError> {
    let name = asset_name.to_ascii_lowercase();
    if name.ends_with(".zip") {
        return extract_zip_archive(archive_path, destination);
    }
    if name.ends_with(".tar.gz") {
        return extract_tar_gz_archive(archive_path, destination);
    }
    Err(unsupported_archive_error())
}

/// Runs archive decoding and synchronous file writes without occupying an async runtime worker.
async fn extract_downloaded_asset_async(
    archive_path: &Path,
    destination: &Path,
    asset_name: &str,
) -> Result<(), ServiceError> {
    let archive_path = archive_path.to_path_buf();
    let destination = destination.to_path_buf();
    let asset_name = asset_name.to_owned();
    tokio::task::spawn_blocking(move || {
        extract_downloaded_asset(&archive_path, &destination, &asset_name)
    })
    .await
    .map_err(|_| extraction_worker_error())?
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

async fn download_asset(
    client: &Client,
    asset: &EngineAsset,
    destination: &Path,
    progress: &InstallProgressContext<'_>,
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
                progress,
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
        progress,
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

/// Extracts ZIP files without applying archive permissions or creating links.
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
        let entry_name = entry.name().map_err(|_| archive_error())?;
        let relative_path = archive_relative_path(entry_name.as_ref(), entry.is_dir())?;
        if !paths.insert(relative_path.clone()) {
            return Err(archive_error());
        }
        let entry_size = entry.size();
        if entry_size > MAX_ARCHIVE_ENTRY_BYTES {
            return Err(archive_error());
        }
        if extracted_bytes
            .checked_add(entry_size)
            .is_none_or(|bytes| bytes > MAX_EXTRACTED_BYTES)
        {
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
        let mut copied_bytes = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = entry.read(&mut buffer).map_err(|_| archive_error())?;
            if read == 0 {
                break;
            }

            let read = u64::try_from(read).map_err(|_| archive_error())?;
            copied_bytes = copied_bytes
                .checked_add(read)
                .filter(|bytes| *bytes <= entry_size)
                .ok_or_else(archive_error)?;
            extracted_bytes = extracted_bytes
                .checked_add(read)
                .filter(|bytes| *bytes <= MAX_EXTRACTED_BYTES)
                .ok_or_else(archive_error)?;
            output
                .write_all(&buffer[..usize::try_from(read).map_err(|_| archive_error())?])
                .map_err(|_| storage_error())?;
        }
        if copied_bytes != entry_size {
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
    progress: &InstallProgressContext<'_>,
    asset: &EngineAsset,
    asset_index: usize,
    asset_count: usize,
    phase: &str,
    downloaded_bytes: u64,
) -> Result<(), ServiceError> {
    progress
        .events
        .send(&Response::event(
            progress.request_id.clone(),
            INSTALL_PROGRESS_EVENT,
            json!({
                "phase": phase,
                "engineId": progress.manifest.engine_id,
                "variantId": progress.variant.variant_id,
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
        let supported_asset = name.ends_with(".zip") || name.ends_with(".tar.gz");
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

/// Returns a safe error when the blocking extraction task cannot be joined.
fn extraction_worker_error() -> ServiceError {
    ServiceError::new(
        "local_engine_extraction_failed",
        "The local engine archive could not be extracted.",
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
    use std::{
        fs,
        io::{Cursor, Write},
        path::Path,
        time::{Duration, Instant},
    };

    use flate2::{Compression, write::GzEncoder};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };
    use url::Url;
    use uuid::Uuid;
    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::{
        INSTALL_MARKER, INSTALL_MARKER_VERSION, InstalledMarker, MAX_INSTALL_MARKER_BYTES,
        archive_relative_path, extract_downloaded_asset_async, extract_tar_gz_archive,
        extract_zip_archive, installed_engine, installed_engine_path, installer_client,
        is_approved_engine_asset_redirect, read_installed_marker,
    };
    use crate::{local_engines::catalog::parse_manifest, storage::AppStorage};

    const LLAMA_CPP_MANIFEST: &[u8] =
        include_bytes!("../../../resources/local-engines/llama_cpp.json");

    #[test]
    fn release_asset_redirects_are_limited_to_github_https_hosts() {
        for url in [
            "https://github.com/org/repo/releases/download/v1/runtime.zip",
            "https://release-assets.githubusercontent.com/github-production-release-asset/runtime.zip",
        ] {
            assert!(
                is_approved_engine_asset_redirect(&Url::parse(url).expect("valid URL")),
                "expected {url} to be accepted"
            );
        }
    }

    #[test]
    fn release_asset_redirects_reject_untrusted_hosts_and_authority_changes() {
        for url in [
            "http://github.com/org/repo/releases/download/v1/runtime.zip",
            "https://github.com.attacker.example/runtime.zip",
            "https://objects.githubusercontent.com/runtime.zip",
            "https://github.com:8443/org/repo/releases/download/v1/runtime.zip",
            "https://user@github.com/org/repo/releases/download/v1/runtime.zip",
        ] {
            assert!(
                !is_approved_engine_asset_redirect(&Url::parse(url).expect("valid URL")),
                "expected {url} to be rejected"
            );
        }
    }

    #[tokio::test]
    async fn installer_client_does_not_follow_redirects_to_local_addresses() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local redirect fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            read_request_headers(&mut stream).await;
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/collect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .expect("write redirect response");
        });

        let response = installer_client()
            .expect("build installer client")
            .get(format!("http://{address}/asset.zip"))
            .send()
            .await
            .expect("rejected redirect should be returned as-is");

        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        assert_eq!(response.url().host_str(), Some("127.0.0.1"));
        server.await.expect("redirect fixture should complete");
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "manual 128 MiB local-engine extraction runtime-responsiveness benchmark"]
    /// Measures runtime timer delay while unpacking a large local-engine archive.
    async fn benchmark_runtime_timer_delay_during_engine_archive_extraction() {
        let root =
            std::env::temp_dir().join(format!("openchat-extract-benchmark-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("benchmark directory should be created");
        let archive_path = root.join("runtime.zip");
        let mut archive = ZipWriter::new(
            fs::File::create(&archive_path).expect("benchmark archive should be created"),
        );
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        archive
            .start_file("runtime.bin", options)
            .expect("benchmark file entry should be created");
        let block = vec![0x5a; 1024 * 1024];
        for _ in 0..128 {
            archive
                .write_all(&block)
                .expect("benchmark archive bytes should be written");
        }
        archive
            .finish()
            .expect("benchmark archive should finish")
            .sync_all()
            .expect("benchmark archive should sync");

        let mut delays = Vec::new();
        for sample in 0..3 {
            let destination = root.join(format!("extracted-{sample}"));
            fs::create_dir_all(&destination)
                .expect("benchmark extraction directory should be created");
            let timer = tokio::spawn(tokio::time::sleep(Duration::from_millis(10)));
            tokio::task::yield_now().await;
            let started_at = Instant::now();
            let archive_path = archive_path.clone();
            let destination_for_task = destination.clone();
            let extraction = tokio::spawn(async move {
                extract_downloaded_asset_async(&archive_path, &destination_for_task, "runtime.zip")
                    .await
            });
            timer.await.expect("benchmark timer task should complete");
            let timer_delay = started_at.elapsed();
            extraction
                .await
                .expect("benchmark extraction task should complete")
                .expect("benchmark archive should extract");
            eprintln!("sample {sample}: 10 ms timer delay {timer_delay:?}, extraction completed");
            delays.push(timer_delay);
        }
        delays.sort_unstable();
        eprintln!("median 10 ms timer delay: {:?}", delays[delays.len() / 2]);
        fs::remove_dir_all(root).expect("benchmark directory should be removed");
    }

    async fn read_request_headers(stream: &mut TcpStream) {
        let mut request = Vec::new();
        let mut byte = [0; 1];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut byte).await.expect("read request headers");
            assert_ne!(read, 0, "client closed before sending request headers");
            request.push(byte[0]);
        }
    }

    fn write_zip_file(path: &Path, entry_name: &str, contents: &[u8]) {
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file(entry_name, SimpleFileOptions::default())
            .expect("ZIP file entry should be created");
        archive
            .write_all(contents)
            .expect("ZIP file contents should be written");
        fs::write(
            path,
            archive
                .finish()
                .expect("ZIP archive should finish")
                .into_inner(),
        )
        .expect("ZIP archive should be saved");
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
    fn extracts_regular_files_from_a_zip_archive() {
        let root = std::env::temp_dir().join(format!("openchat-zip-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test directory should be created");
        let archive_path = root.join("engine.zip");
        let destination = root.join("extracted");
        let contents = b"verified test executable";
        write_zip_file(&archive_path, "runtime/engine.exe", contents);
        fs::create_dir_all(&destination).expect("extraction directory should be created");

        extract_zip_archive(&archive_path, &destination)
            .expect("the regular ZIP entry should extract");

        assert_eq!(
            fs::read(destination.join("runtime/engine.exe"))
                .expect("the extracted file should be readable"),
            contents
        );
        fs::remove_dir_all(root).expect("test directory should be removed");
    }

    #[tokio::test]
    /// Confirms the installer path extracts a validated archive through the blocking pool.
    async fn extracts_archives_through_the_async_install_path() {
        let root = std::env::temp_dir().join(format!("openchat-zip-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test directory should be created");
        let archive_path = root.join("engine.zip");
        let destination = root.join("extracted");
        let contents = b"verified async install executable";
        write_zip_file(&archive_path, "runtime/engine.exe", contents);
        fs::create_dir_all(&destination).expect("extraction directory should be created");

        extract_downloaded_asset_async(&archive_path, &destination, "engine.zip")
            .await
            .expect("the async install path should extract the archive");

        assert_eq!(
            fs::read(destination.join("runtime/engine.exe"))
                .expect("the extracted file should be readable"),
            contents
        );
        fs::remove_dir_all(root).expect("test directory should be removed");
    }

    #[test]
    fn rejects_zip_paths_that_escape_the_destination() {
        let root = std::env::temp_dir().join(format!("openchat-zip-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test directory should be created");
        let archive_path = root.join("engine.zip");
        let destination = root.join("extracted");
        write_zip_file(&archive_path, "../escaped.txt", b"must not be written");
        fs::create_dir_all(&destination).expect("extraction directory should be created");

        assert!(extract_zip_archive(&archive_path, &destination).is_err());
        assert!(!root.join("escaped.txt").exists());
        fs::remove_dir_all(root).expect("test directory should be removed");
    }

    #[test]
    fn rejects_symlinks_in_zip_archives() {
        let root = std::env::temp_dir().join(format!("openchat-zip-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test directory should be created");
        let archive_path = root.join("engine.zip");
        let destination = root.join("extracted");
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .add_symlink(
                "runtime/link",
                "../../outside",
                SimpleFileOptions::default(),
            )
            .expect("ZIP symlink entry should be created");
        fs::write(
            &archive_path,
            archive
                .finish()
                .expect("ZIP archive should finish")
                .into_inner(),
        )
        .expect("ZIP archive should be saved");
        fs::create_dir_all(&destination).expect("extraction directory should be created");

        assert!(extract_zip_archive(&archive_path, &destination).is_err());
        assert!(!destination.join("runtime/link").exists());
        fs::remove_dir_all(root).expect("test directory should be removed");
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
    fn installed_marker_read_is_bounded_at_the_size_limit() {
        let root = std::env::temp_dir().join(format!("openchat-marker-test-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("test directory should be created");
        let marker_path = root.join(INSTALL_MARKER);
        let marker = InstalledMarker {
            marker_version: INSTALL_MARKER_VERSION,
            engine_id: "llama_cpp".to_owned(),
            variant_id: "windows-cpu-x64".to_owned(),
            release_tag: "v1".to_owned(),
            entrypoint: "llama-server.exe".to_owned(),
        };
        let mut bytes = serde_json::to_vec(&marker).expect("marker should serialize");
        assert!(bytes.len() < MAX_INSTALL_MARKER_BYTES);
        bytes.resize(MAX_INSTALL_MARKER_BYTES, b' ');
        fs::write(&marker_path, &bytes).expect("exact-limit marker should be writable");

        let parsed = read_installed_marker(&marker_path).expect("exact-limit marker should parse");
        assert_eq!(parsed.engine_id, marker.engine_id);
        assert_eq!(parsed.entrypoint, marker.entrypoint);

        bytes.push(b' ');
        fs::write(&marker_path, bytes).expect("oversized marker should be writable");
        assert!(read_installed_marker(&marker_path).is_err());

        fs::remove_dir_all(root).expect("test directory should be removed");
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
