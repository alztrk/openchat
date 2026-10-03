//! Typed catalog metadata for app-managed local inference runtimes.
//!
//! The catalog is deliberately kept separate from the installer.  This module
//! only parses and validates metadata that was shipped with OpenChat; it never
//! fetches a URL or executes a downloaded file.

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use url::Url;

const SUPPORTED_SCHEMA_VERSION: u32 = 1;

const ALLOWED_ENGINE_IDS: &[&str] = &["llama_cpp", "vllm", "exllama"];
const ALLOWED_OS: &[&str] = &["windows", "linux", "macos"];
const ALLOWED_ARCHITECTURES: &[&str] = &["x86_64", "aarch64"];
const ALLOWED_ACCELERATORS: &[&str] = &["cpu", "cuda", "vulkan", "metal", "xpu"];
const ALLOWED_CHANNELS: &[&str] = &["stable", "preview", "nightly"];
const ALLOWED_CATALOG_STATUSES: &[&str] = &["installable", "blocked"];
const ALLOWED_ASSET_ROLES: &[&str] = &["primary", "companion"];

/// A release catalog entry for one local inference engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineManifest {
    pub schema_version: u32,
    pub engine_id: String,
    pub display_name: String,
    pub source_repository: String,
    pub release_tag: String,
    pub release_url: String,
    #[serde(default)]
    pub source_commit: Option<String>,
    pub channel: String,
    pub license: String,
    pub catalog_status: String,
    #[serde(default)]
    pub status_reason: Option<String>,
    pub variants: Vec<EngineVariant>,
    pub sources: Vec<String>,
}

/// One operating-system, architecture, and accelerator combination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineVariant {
    pub variant_id: String,
    pub os: String,
    pub architecture: String,
    pub accelerator: String,
    pub runtime_requirements: Vec<String>,
    pub entrypoint: String,
    pub required_files: Vec<String>,
    pub assets: Vec<EngineAsset>,
    pub capabilities: Vec<String>,
}

/// A release asset with an integrity pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineAsset {
    pub name: String,
    pub url: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub role: String,
    #[serde(default)]
    pub companion_for: Option<String>,
}

/// Errors returned while reading or validating a catalog manifest.
#[derive(Debug)]
pub enum CatalogError {
    Json(serde_json::Error),
    Invalid(String),
}

impl Display for CatalogError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(source) => write!(formatter, "invalid engine catalog JSON: {source}"),
            Self::Invalid(message) => write!(formatter, "invalid engine catalog: {message}"),
        }
    }
}

impl std::error::Error for CatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

impl From<serde_json::Error> for CatalogError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json(source)
    }
}

/// Parse and validate one manifest from embedded bytes or a caller-owned buffer.
pub fn parse_manifest(bytes: &[u8]) -> Result<EngineManifest, CatalogError> {
    let manifest = serde_json::from_slice::<EngineManifest>(bytes)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &EngineManifest) -> Result<(), CatalogError> {
    if manifest.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(invalid(format!(
            "schemaVersion must be {SUPPORTED_SCHEMA_VERSION}, got {}",
            manifest.schema_version
        )));
    }

    require_nonempty(&manifest.engine_id, "engineId")?;
    if !ALLOWED_ENGINE_IDS.contains(&manifest.engine_id.as_str()) {
        return Err(invalid(format!(
            "unsupported engineId {:?}",
            manifest.engine_id
        )));
    }
    require_nonempty(&manifest.display_name, "displayName")?;
    validate_source_repository(&manifest.source_repository)?;
    require_nonempty(&manifest.release_tag, "releaseTag")?;
    validate_release_url(
        &manifest.release_url,
        &manifest.source_repository,
        &manifest.release_tag,
    )?;
    if let Some(source_commit) = &manifest.source_commit {
        if !source_commit.is_empty()
            && (!source_commit.bytes().all(|byte| byte.is_ascii_hexdigit())
                || source_commit.bytes().any(|byte| byte.is_ascii_uppercase())
                || !(7..=64).contains(&source_commit.len()))
        {
            return Err(invalid(
                "sourceCommit must be 7 to 64 lowercase hexadecimal characters".to_owned(),
            ));
        }
    }
    validate_allowed(&manifest.channel, "channel", ALLOWED_CHANNELS)?;
    require_nonempty(&manifest.license, "license")?;
    validate_allowed(
        &manifest.catalog_status,
        "catalogStatus",
        ALLOWED_CATALOG_STATUSES,
    )?;
    if manifest.catalog_status == "blocked" {
        match manifest.status_reason.as_deref() {
            Some(reason) if !reason.trim().is_empty() => {}
            _ => {
                return Err(invalid(
                    "blocked catalogs must include a nonempty statusReason".to_owned(),
                ));
            }
        }
    }
    if manifest.variants.is_empty() {
        return Err(invalid("variants must not be empty".to_owned()));
    }
    if manifest.sources.is_empty() {
        return Err(invalid("sources must not be empty".to_owned()));
    }

    let mut variant_ids = Vec::with_capacity(manifest.variants.len());
    for variant in &manifest.variants {
        validate_variant(variant, manifest.catalog_status.as_str(), &mut variant_ids)?;
    }
    validate_companion_references(&manifest.variants, &variant_ids)?;
    for source in &manifest.sources {
        validate_https_source_url(source)?;
    }

    Ok(())
}

fn validate_variant(
    variant: &EngineVariant,
    catalog_status: &str,
    variant_ids: &mut Vec<String>,
) -> Result<(), CatalogError> {
    require_nonempty(&variant.variant_id, "variantId")?;
    if variant_ids.iter().any(|id| id == &variant.variant_id) {
        return Err(invalid(format!(
            "duplicate variantId {:?}",
            variant.variant_id
        )));
    }
    variant_ids.push(variant.variant_id.clone());

    validate_allowed(&variant.os, "os", ALLOWED_OS)?;
    validate_allowed(&variant.architecture, "architecture", ALLOWED_ARCHITECTURES)?;
    validate_allowed(&variant.accelerator, "accelerator", ALLOWED_ACCELERATORS)?;
    validate_nonempty_unique(&variant.runtime_requirements, "runtimeRequirements")?;

    if catalog_status == "installable" {
        require_nonempty(&variant.entrypoint, "entrypoint")?;
        if variant.required_files.is_empty() {
            return Err(invalid(format!(
                "installable variant {:?} must have requiredFiles",
                variant.variant_id
            )));
        }
        if variant.assets.is_empty() {
            return Err(invalid(format!(
                "installable variant {:?} must have release assets",
                variant.variant_id
            )));
        }
    }
    validate_nonempty_unique(&variant.required_files, "requiredFiles")?;
    validate_nonempty_unique(&variant.capabilities, "capabilities")?;

    let mut asset_names = Vec::with_capacity(variant.assets.len());
    for asset in &variant.assets {
        validate_asset(asset, &variant.variant_id, &mut asset_names)?;
    }

    Ok(())
}

fn validate_asset(
    asset: &EngineAsset,
    variant_id: &str,
    asset_names: &mut Vec<String>,
) -> Result<(), CatalogError> {
    require_nonempty(&asset.name, "asset name")?;
    if asset.name.contains('/')
        || asset.name.contains('\\')
        || asset.name == "."
        || asset.name == ".."
    {
        return Err(invalid(format!(
            "asset {:?} has an invalid path-like name",
            asset.name
        )));
    }
    if asset_names.iter().any(|name| name == &asset.name) {
        return Err(invalid(format!(
            "duplicate asset {:?} in variant {:?}",
            asset.name, variant_id
        )));
    }
    asset_names.push(asset.name.clone());
    validate_github_url(&asset.url, "asset URL", false)?;
    if asset.size_bytes == 0 {
        return Err(invalid(format!(
            "asset {:?} has zero sizeBytes",
            asset.name
        )));
    }
    validate_sha256(&asset.sha256, &asset.name)?;
    validate_allowed(&asset.role, "asset role", ALLOWED_ASSET_ROLES)?;
    match (asset.role.as_str(), asset.companion_for.as_deref()) {
        ("primary", Some(_)) => {
            return Err(invalid(format!(
                "primary asset {:?} must not have companionFor",
                asset.name
            )));
        }
        ("companion", None) => {
            return Err(invalid(format!(
                "companion asset {:?} must have companionFor",
                asset.name
            )));
        }
        _ => {}
    }
    if let Some(companion_for) = &asset.companion_for {
        require_nonempty(companion_for, "companionFor")?;
    }
    Ok(())
}

fn validate_companion_references(
    variants: &[EngineVariant],
    variant_ids: &[String],
) -> Result<(), CatalogError> {
    for variant in variants {
        for asset in &variant.assets {
            if let Some(companion_for) = &asset.companion_for
                && !variant_ids.iter().any(|id| id == companion_for)
            {
                return Err(invalid(format!(
                    "asset {:?} in variant {:?} references unknown companionFor {:?}",
                    asset.name, variant.variant_id, companion_for
                )));
            }
        }
    }
    Ok(())
}

fn validate_source_repository(repository: &str) -> Result<(), CatalogError> {
    let mut parts = repository.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || owner.is_empty()
        || name.is_empty()
        || owner
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        || name
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(invalid(format!(
            "sourceRepository must use the owner/repository form, got {repository:?}"
        )));
    }
    Ok(())
}

fn validate_release_url(
    url: &str,
    repository: &str,
    release_tag: &str,
) -> Result<(), CatalogError> {
    validate_github_url(url, "releaseUrl", false)?;
    let parsed =
        Url::parse(url).map_err(|_| invalid(format!("releaseUrl is not a valid URL: {url:?}")))?;
    let expected = format!("/{repository}/releases/tag/{release_tag}");
    if !parsed.path().starts_with(&expected) {
        return Err(invalid(format!(
            "releaseUrl does not point to the declared release {repository}@{release_tag}"
        )));
    }
    Ok(())
}

fn validate_github_url(value: &str, field: &str, allow_api_host: bool) -> Result<(), CatalogError> {
    let parsed = Url::parse(value).map_err(|_| invalid(format!("{field} is not a valid URL")))?;
    if parsed.scheme() != "https" {
        return Err(invalid(format!("{field} must use HTTPS")));
    }
    let host = parsed.host_str().unwrap_or_default();
    let host_allowed = host == "github.com"
        || (allow_api_host && host == "api.github.com")
        || (allow_api_host && host == "raw.githubusercontent.com");
    if !host_allowed {
        return Err(invalid(format!(
            "{field} must use an approved GitHub host, got {host:?}"
        )));
    }
    if parsed.username() != "" || parsed.password().is_some() || parsed.port().is_some() {
        return Err(invalid(format!(
            "{field} must not contain credentials or a custom port"
        )));
    }
    if parsed.path().is_empty() || parsed.path() == "/" {
        return Err(invalid(format!("{field} must contain a path")));
    }
    Ok(())
}

fn validate_https_source_url(value: &str) -> Result<(), CatalogError> {
    let parsed =
        Url::parse(value).map_err(|_| invalid("sources entries must be valid URLs".to_owned()))?;
    if parsed.scheme() != "https" {
        return Err(invalid("sources entries must use HTTPS".to_owned()));
    }
    if parsed.host_str().is_none_or(str::is_empty)
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.path().is_empty()
        || parsed.path() == "/"
    {
        return Err(invalid(
            "sources entries must have a host and path and no credentials or custom port"
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str, asset_name: &str) -> Result<(), CatalogError> {
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(format!(
            "asset {:?} must have a lowercase 64-hex SHA-256 digest",
            asset_name
        )));
    }
    if digest.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(invalid(format!(
            "asset {:?} SHA-256 digest must use lowercase hexadecimal",
            asset_name
        )));
    }
    Ok(())
}

fn validate_allowed(value: &str, field: &str, allowed: &[&str]) -> Result<(), CatalogError> {
    if !allowed.contains(&value) {
        return Err(invalid(format!(
            "unsupported {field} {:?}; expected one of {}",
            value,
            allowed.join(", ")
        )));
    }
    Ok(())
}

fn validate_nonempty_unique(values: &[String], field: &str) -> Result<(), CatalogError> {
    let mut seen: Vec<&str> = Vec::with_capacity(values.len());
    for value in values {
        require_nonempty(value, field)?;
        if seen.iter().any(|item| *item == value) {
            return Err(invalid(format!("duplicate value in {field}: {value:?}")));
        }
        seen.push(value);
    }
    Ok(())
}

fn require_nonempty(value: &str, field: &str) -> Result<(), CatalogError> {
    if value.trim().is_empty() {
        return Err(invalid(format!("{field} must not be empty")));
    }
    Ok(())
}

fn invalid(message: String) -> CatalogError {
    CatalogError::Invalid(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const LLAMA_CPP_MANIFEST: &[u8] =
        include_bytes!("../../resources/local-engines/llama_cpp.json");

    fn llama_value() -> Value {
        serde_json::from_slice(LLAMA_CPP_MANIFEST).expect("current catalog must be JSON")
    }

    #[test]
    fn parses_current_llama_cpp_catalog() {
        let manifest =
            parse_manifest(LLAMA_CPP_MANIFEST).expect("llama.cpp catalog should validate");

        assert_eq!(manifest.engine_id, "llama_cpp");
        assert_eq!(manifest.catalog_status, "installable");
        assert_eq!(manifest.variants.len(), 8);
    }

    #[test]
    fn rejects_malformed_digest() {
        let mut value = llama_value();
        value["variants"][0]["assets"][0]["sha256"] = Value::String("sha256:not-a-digest".into());

        let bytes = serde_json::to_vec(&value).expect("test value should serialize");
        assert!(parse_manifest(&bytes).is_err());
    }

    #[test]
    fn rejects_non_https_release_url() {
        let mut value = llama_value();
        value["releaseUrl"] =
            Value::String("http://github.com/ggml-org/llama.cpp/releases/tag/b11349".into());

        let bytes = serde_json::to_vec(&value).expect("test value should serialize");
        assert!(parse_manifest(&bytes).is_err());
    }

    #[test]
    fn rejects_duplicate_variant_ids() {
        let mut value = llama_value();
        let first_variant = value["variants"][0].clone();
        value["variants"]
            .as_array_mut()
            .expect("variants should be an array")
            .push(first_variant);

        let bytes = serde_json::to_vec(&value).expect("test value should serialize");
        assert!(parse_manifest(&bytes).is_err());
    }

    #[test]
    fn rejects_invalid_companion_reference() {
        let mut value = llama_value();
        value["variants"][1]["assets"][1]["companionFor"] = Value::String("missing-variant".into());

        let bytes = serde_json::to_vec(&value).expect("test value should serialize");
        assert!(parse_manifest(&bytes).is_err());
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let mut value = llama_value();
        value["schemaVersion"] = Value::Number(2.into());

        let bytes = serde_json::to_vec(&value).expect("test value should serialize");
        assert!(parse_manifest(&bytes).is_err());
    }
}
