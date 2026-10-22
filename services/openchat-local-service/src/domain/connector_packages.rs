//! Installs and removes signed, version-pinned Windows connector executables.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    connectors,
    tools::web_search::url_fetch::{pinned_public_http_client, validate_http_url},
};

const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const PACKAGE_DIRECTORY: &str = "connector-packages";
const EXECUTABLE_FILE: &str = "connector.exe";
const PACKAGE_METADATA_FILE: &str = "package.json";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InstalledPackageReference {
    pub(crate) version: String,
    pub(crate) sha256: String,
    pub(crate) source_key_fingerprint: String,
    pub(crate) size_bytes: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignedPackageEntry {
    id: String,
    version: String,
    transport: String,
    package: Option<SignedPackageMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SignedPackageMetadata {
    url: String,
    sha256: String,
    size_bytes: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstalledPackageMetadata {
    id: String,
    version: String,
    sha256: String,
    source_key_fingerprint: String,
    size_bytes: u64,
}

/// Downloads a package only after verifying its catalog entry with the pinned source key.
pub(crate) async fn install_signed_package(
    storage_root: &Path,
    project_root: &Path,
    source_url: &str,
    public_key_base64: &str,
    id: &str,
    version: &str,
) -> Result<InstalledPackageReference, PackageError> {
    validate_connector_identity(id, version)?;
    let catalog = connectors::fetch_signed_catalog(source_url, public_key_base64)
        .await
        .map_err(|_| PackageError::InvalidCatalog)?;
    let source_key_fingerprint = catalog.key_fingerprint.clone();
    let entry = catalog
        .entries
        .into_iter()
        .find_map(|entry| parse_signed_entry(entry, id, version))
        .ok_or(PackageError::PackageUnavailable)?;
    if entry.transport != "stdio" {
        return Err(PackageError::PackageUnavailable);
    }
    let package = entry.package.ok_or(PackageError::PackageUnavailable)?;
    validate_package_metadata(&package)?;

    let package_url = Url::parse(&package.url).map_err(|_| PackageError::InvalidCatalog)?;
    let package_url = validate_http_url(package_url).map_err(|_| PackageError::InvalidCatalog)?;
    if package_url.scheme() != "https" {
        return Err(PackageError::InvalidCatalog);
    }
    let package_bytes = fetch_package_bytes(&package_url, package.size_bytes).await?;
    if !is_windows_pe(&package_bytes) {
        return Err(PackageError::InvalidPackage);
    }
    let digest = Sha256::digest(&package_bytes);
    let sha256 = hex_digest(&digest);
    if sha256 != package.sha256 {
        return Err(PackageError::InvalidPackage);
    }

    let project_root = fs::canonicalize(project_root).map_err(|_| PackageError::Unavailable)?;
    let package_directory = package_directory(storage_root, &project_root, id, version)?;
    ensure_package_directory(storage_root, &package_directory)?;

    let metadata = InstalledPackageMetadata {
        id: id.to_owned(),
        version: version.to_owned(),
        sha256: sha256.clone(),
        source_key_fingerprint: source_key_fingerprint.clone(),
        size_bytes: package.size_bytes,
    };
    let executable_path = package_directory.join(EXECUTABLE_FILE);
    match fs::symlink_metadata(&executable_path) {
        Ok(file_metadata) if file_metadata.is_file() && !is_reparse_point(&file_metadata) => {
            let existing = read_installed_metadata(&package_directory)?;
            if existing.id != id
                || existing.version != version
                || existing.sha256 != sha256
                || existing.source_key_fingerprint != source_key_fingerprint
                || existing.size_bytes != package.size_bytes
            {
                return Err(PackageError::InstallConflict);
            }
            verify_package_executable(&executable_path, &existing)?;
            return Ok(package_reference(existing));
        }
        Ok(_) => return Err(PackageError::Unavailable),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(PackageError::Unavailable);
        }
        Err(_) => {}
    }

    let temporary_path = package_directory.join(format!(".download-{}.tmp", uuid::Uuid::new_v4()));
    let mut owns_executable = false;
    let result = (|| {
        let mut temporary = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        temporary.write_all(&package_bytes)?;
        temporary.sync_all()?;
        drop(temporary);
        fs::hard_link(&temporary_path, &executable_path)?;
        owns_executable = true;
        fs::remove_file(&temporary_path)?;
        write_installed_metadata(&package_directory, &metadata)?;
        Ok::<(), std::io::Error>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
        if owns_executable {
            let _ = fs::remove_file(&executable_path);
        }
        return Err(PackageError::Unavailable);
    }

    Ok(package_reference(metadata))
}

async fn fetch_package_bytes(url: &Url, expected_size: u64) -> Result<Vec<u8>, PackageError> {
    let client = pinned_public_http_client(url, Some(Duration::from_secs(180)))
        .await
        .map_err(|_| PackageError::InvalidCatalog)?;
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|_| PackageError::Unavailable)?;
    if !response.status().is_success() {
        return Err(PackageError::Unavailable);
    }
    if response
        .content_length()
        .is_some_and(|length| length != expected_size)
    {
        return Err(PackageError::InvalidPackage);
    }

    let capacity = usize::try_from(expected_size).map_err(|_| PackageError::InvalidCatalog)?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| PackageError::Unavailable)?;
        if bytes.len().saturating_add(chunk.len()) > capacity {
            return Err(PackageError::InvalidPackage);
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.len() != capacity {
        return Err(PackageError::InvalidPackage);
    }
    Ok(bytes)
}

/// Resolves an installed executable and its read-only sandbox directory.
pub(crate) fn resolve_installed_package(
    storage_root: &Path,
    project_root: &Path,
    id: &str,
    reference: &InstalledPackageReference,
) -> Result<(PathBuf, PathBuf), PackageError> {
    validate_connector_identity(id, &reference.version)?;
    validate_reference(reference)?;
    let project_root = fs::canonicalize(project_root).map_err(|_| PackageError::Unavailable)?;
    let directory = package_directory(storage_root, &project_root, id, &reference.version)?;
    verify_package_directory(storage_root, &directory)?;
    let metadata = read_installed_metadata(&directory)?;
    if metadata.id != id
        || metadata.version != reference.version
        || metadata.sha256 != reference.sha256
        || metadata.source_key_fingerprint != reference.source_key_fingerprint
        || metadata.size_bytes != reference.size_bytes
    {
        return Err(PackageError::InvalidPackage);
    }
    let executable = directory.join(EXECUTABLE_FILE);
    let file_metadata = fs::symlink_metadata(&executable).map_err(|_| PackageError::Unavailable)?;
    if !file_metadata.is_file()
        || is_reparse_point(&file_metadata)
        || file_metadata.len() != reference.size_bytes
    {
        return Err(PackageError::InvalidPackage);
    }
    Ok((executable, directory))
}

/// Rechecks the installed bytes immediately before the process is launched.
pub(crate) fn verify_installed_package(
    executable: &Path,
    reference: &InstalledPackageReference,
) -> Result<(), PackageError> {
    validate_reference(reference)?;
    let metadata = fs::symlink_metadata(executable).map_err(|_| PackageError::Unavailable)?;
    if !metadata.is_file() || is_reparse_point(&metadata) || metadata.len() != reference.size_bytes
    {
        return Err(PackageError::InvalidPackage);
    }
    let mut file = File::open(executable).map_err(|_| PackageError::Unavailable)?;
    let mut digest = Sha256::new();
    let mut header = Vec::with_capacity(4096);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| PackageError::Unavailable)?;
        if count == 0 {
            break;
        }
        if header.len() < 4096 {
            let remaining = 4096 - header.len();
            header.extend_from_slice(&buffer[..count.min(remaining)]);
        }
        digest.update(&buffer[..count]);
    }
    if hex_digest(&digest.finalize()) != reference.sha256 || !is_windows_pe(&header) {
        return Err(PackageError::InvalidPackage);
    }
    Ok(())
}

/// Removes one package version only after its project server has been removed or replaced.
pub(crate) fn remove_installed_package(
    storage_root: &Path,
    project_root: &Path,
    id: &str,
    reference: &InstalledPackageReference,
) -> Result<(), PackageError> {
    validate_connector_identity(id, &reference.version)?;
    validate_reference(reference)?;
    let project_root = fs::canonicalize(project_root).map_err(|_| PackageError::Unavailable)?;
    let directory = package_directory(storage_root, &project_root, id, &reference.version)?;
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(PackageError::Unavailable),
        Ok(_) => {}
    }
    verify_package_directory(storage_root, &directory)?;
    let executable = directory.join(EXECUTABLE_FILE);
    let package_metadata_path = directory.join(PACKAGE_METADATA_FILE);
    let executable_exists = is_regular_package_file(&executable)?;
    let metadata_exists = is_regular_package_file(&package_metadata_path)?;
    if metadata_exists {
        let metadata = read_installed_metadata(&directory)?;
        if metadata.id != id
            || metadata.version != reference.version
            || metadata.sha256 != reference.sha256
            || metadata.source_key_fingerprint != reference.source_key_fingerprint
            || metadata.size_bytes != reference.size_bytes
        {
            return Err(PackageError::InvalidPackage);
        }
    } else if executable_exists {
        verify_installed_package(&executable, reference)?;
    }
    remove_interrupted_downloads(&directory)?;
    if executable_exists {
        fs::remove_file(executable).map_err(|_| PackageError::Unavailable)?;
    }
    if metadata_exists {
        fs::remove_file(package_metadata_path).map_err(|_| PackageError::Unavailable)?;
    }
    fs::remove_dir(&directory).map_err(|_| PackageError::Unavailable)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PackageError {
    InvalidCatalog,
    PackageUnavailable,
    InvalidPackage,
    InstallConflict,
    Unavailable,
}

fn parse_signed_entry(value: Value, id: &str, version: &str) -> Option<SignedPackageEntry> {
    let entry: SignedPackageEntry = serde_json::from_value(value).ok()?;
    (entry.id == id && entry.version == version).then_some(entry)
}

fn validate_package_metadata(package: &SignedPackageMetadata) -> Result<(), PackageError> {
    if package.size_bytes == 0
        || package.size_bytes > MAX_PACKAGE_BYTES
        || package.sha256.len() != 64
        || !package.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        || package.url.len() > 2048
        || package.url.trim() != package.url
        || package.url.contains('?')
        || package.url.contains('#')
    {
        return Err(PackageError::InvalidCatalog);
    }
    Ok(())
}

fn validate_connector_identity(id: &str, version: &str) -> Result<(), PackageError> {
    if !valid_connector_id(id)
        || version.is_empty()
        || version.len() > 64
        || !version.as_bytes()[0].is_ascii_alphanumeric()
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'))
    {
        return Err(PackageError::InvalidCatalog);
    }
    Ok(())
}

fn validate_reference(reference: &InstalledPackageReference) -> Result<(), PackageError> {
    if reference.sha256.len() != 64
        || !reference
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || reference.source_key_fingerprint.len() != 64
        || !reference
            .source_key_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || reference.size_bytes == 0
        || reference.size_bytes > MAX_PACKAGE_BYTES
    {
        return Err(PackageError::InvalidPackage);
    }
    validate_connector_identity("package", &reference.version)
}

fn package_directory(
    storage_root: &Path,
    project_root: &Path,
    id: &str,
    version: &str,
) -> Result<PathBuf, PackageError> {
    validate_connector_identity(id, version)?;
    let storage_root = fs::canonicalize(storage_root).map_err(|_| PackageError::Unavailable)?;
    let project_hash = Sha256::digest(
        project_root
            .to_string_lossy()
            .to_ascii_lowercase()
            .as_bytes(),
    );
    Ok(storage_root
        .join(PACKAGE_DIRECTORY)
        .join(hex_digest(&project_hash))
        .join(id)
        .join(version))
}

fn ensure_package_directory(storage_root: &Path, directory: &Path) -> Result<(), PackageError> {
    let canonical_root = fs::canonicalize(storage_root).map_err(|_| PackageError::Unavailable)?;
    let package_root = canonical_root.join(PACKAGE_DIRECTORY);
    let id_directory = directory.parent().ok_or(PackageError::Unavailable)?;
    let project_directory = id_directory.parent().ok_or(PackageError::Unavailable)?;
    for path in [
        package_root,
        project_directory.to_path_buf(),
        id_directory.to_path_buf(),
        directory.to_path_buf(),
    ] {
        ensure_directory(&path)?;
    }
    let canonical_directory = fs::canonicalize(directory).map_err(|_| PackageError::Unavailable)?;
    if !canonical_directory.starts_with(&canonical_root) {
        return Err(PackageError::Unavailable);
    }
    Ok(())
}

fn verify_package_directory(storage_root: &Path, directory: &Path) -> Result<(), PackageError> {
    let canonical_root = fs::canonicalize(storage_root).map_err(|_| PackageError::Unavailable)?;
    let mut current = canonical_root.join(PACKAGE_DIRECTORY);
    let id_directory = directory.parent().ok_or(PackageError::Unavailable)?;
    let project_directory = id_directory.parent().ok_or(PackageError::Unavailable)?;
    for path in [
        current.clone(),
        project_directory.to_path_buf(),
        id_directory.to_path_buf(),
        directory.to_path_buf(),
    ] {
        let metadata = fs::symlink_metadata(&path).map_err(|_| PackageError::Unavailable)?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err(PackageError::Unavailable);
        }
        current = path;
    }
    let canonical_directory = fs::canonicalize(current).map_err(|_| PackageError::Unavailable)?;
    if !canonical_directory.starts_with(&canonical_root) {
        return Err(PackageError::Unavailable);
    }
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), PackageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => Ok(()),
        Ok(_) => Err(PackageError::Unavailable),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|_| PackageError::Unavailable)?;
            let metadata = fs::symlink_metadata(path).map_err(|_| PackageError::Unavailable)?;
            if metadata.is_dir() && !is_reparse_point(&metadata) {
                Ok(())
            } else {
                Err(PackageError::Unavailable)
            }
        }
        Err(_) => Err(PackageError::Unavailable),
    }
}

fn read_installed_metadata(directory: &Path) -> Result<InstalledPackageMetadata, PackageError> {
    let path = directory.join(PACKAGE_METADATA_FILE);
    let metadata = fs::symlink_metadata(&path).map_err(|_| PackageError::Unavailable)?;
    if !metadata.is_file() || is_reparse_point(&metadata) || metadata.len() > 4096 {
        return Err(PackageError::InvalidPackage);
    }
    let bytes = fs::read(path).map_err(|_| PackageError::Unavailable)?;
    serde_json::from_slice(&bytes).map_err(|_| PackageError::InvalidPackage)
}

fn is_regular_package_file(path: &Path) -> Result<bool, PackageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_reparse_point(&metadata) => Ok(true),
        Ok(_) => Err(PackageError::InvalidPackage),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(PackageError::Unavailable),
    }
}

fn remove_interrupted_downloads(directory: &Path) -> Result<(), PackageError> {
    let entries = fs::read_dir(directory).map_err(|_| PackageError::Unavailable)?;
    for entry in entries {
        let entry = entry.map_err(|_| PackageError::Unavailable)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let is_temporary_metadata = name.starts_with(".metadata-");
        let prefix = if is_temporary_metadata {
            ".metadata-"
        } else {
            ".download-"
        };
        let Some(identifier) = name
            .strip_prefix(prefix)
            .and_then(|name| name.strip_suffix(".tmp"))
        else {
            continue;
        };
        if uuid::Uuid::parse_str(identifier).is_err() {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| PackageError::Unavailable)?;
        if !metadata.is_file() || is_reparse_point(&metadata) {
            return Err(PackageError::InvalidPackage);
        }
        fs::remove_file(entry.path()).map_err(|_| PackageError::Unavailable)?;
    }
    Ok(())
}

fn write_installed_metadata(
    directory: &Path,
    metadata: &InstalledPackageMetadata,
) -> Result<(), std::io::Error> {
    let path = directory.join(PACKAGE_METADATA_FILE);
    let temporary_path = directory.join(format!(".metadata-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        file.write_all(&serde_json::to_vec(metadata).map_err(std::io::Error::other)?)?;
        file.sync_all()?;
        drop(file);
        fs::hard_link(&temporary_path, &path)?;
        fs::remove_file(&temporary_path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

fn verify_package_executable(
    executable: &Path,
    metadata: &InstalledPackageMetadata,
) -> Result<(), PackageError> {
    verify_installed_package(
        executable,
        &InstalledPackageReference {
            version: metadata.version.clone(),
            sha256: metadata.sha256.clone(),
            source_key_fingerprint: metadata.source_key_fingerprint.clone(),
            size_bytes: metadata.size_bytes,
        },
    )
}

fn package_reference(metadata: InstalledPackageMetadata) -> InstalledPackageReference {
    InstalledPackageReference {
        version: metadata.version,
        sha256: metadata.sha256,
        source_key_fingerprint: metadata.source_key_fingerprint,
        size_bytes: metadata.size_bytes,
    }
}

fn valid_connector_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 24
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_windows_pe(bytes: &[u8]) -> bool {
    if bytes.len() < 64 || &bytes[..2] != b"MZ" {
        return false;
    }
    let offset = u32::from_le_bytes([bytes[60], bytes[61], bytes[62], bytes[63]]) as usize;
    bytes
        .get(offset..offset.saturating_add(4))
        .is_some_and(|signature| signature == b"PE\0\0")
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
