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
const ALLOWED_ASSET_ROLES: &[&str] = &["primary", "companion", "source"];

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
    #[serde(default)]
    pub host_os: Option<String>,
    pub architecture: String,
    pub accelerator: String,
    #[serde(default)]
    pub minimum_driver_version: Option<String>,
    #[serde(default)]
    pub recommended_driver_version: Option<String>,
    pub runtime_requirements: Vec<String>,
    pub entrypoint: String,
    pub required_files: Vec<String>,
    pub assets: Vec<EngineAsset>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DriverVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl DriverVersion {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        let mut parts = value.trim().split('.');
        let major = parse_version_part(parts.next()?)?;
        let minor = parse_version_part(parts.next()?)?;
        let patch = match parts.next() {
            Some(part) => parse_version_part(part)?,
            None => 0,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

fn parse_version_part(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

/// A release asset with an integrity pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineAsset {
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_tag: Option<String>,
    pub size_bytes: u64,
    pub sha256: String,
    pub role: String,
    #[serde(default)]
    pub companion_for: Option<String>,
    #[serde(default)]
    pub source_repository: Option<String>,
    #[serde(default)]
    pub source_commit: Option<String>,
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
    if let Some(source_commit) = &manifest.source_commit
        && !source_commit.is_empty()
        && (!source_commit.bytes().all(|byte| byte.is_ascii_hexdigit())
            || source_commit.bytes().any(|byte| byte.is_ascii_uppercase())
            || !(7..=64).contains(&source_commit.len()))
    {
        return Err(invalid(
            "sourceCommit must be 7 to 64 lowercase hexadecimal characters".to_owned(),
        ));
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
    if let Some(host_os) = variant.host_os.as_deref() {
        validate_allowed(host_os, "hostOs", ALLOWED_OS)?;
        if host_os == variant.os
            || !(host_os == "windows"
                && variant.os == "linux"
                && variant.variant_id.starts_with("windows-wsl2-"))
        {
            return Err(invalid(format!(
                "variant {:?} has an unsupported hostOs/runtime OS combination",
                variant.variant_id
            )));
        }
    }
    validate_allowed(&variant.architecture, "architecture", ALLOWED_ARCHITECTURES)?;
    validate_allowed(&variant.accelerator, "accelerator", ALLOWED_ACCELERATORS)?;
    validate_driver_requirements(variant, catalog_status)?;
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

fn validate_driver_requirements(
    variant: &EngineVariant,
    catalog_status: &str,
) -> Result<(), CatalogError> {
    match (
        variant.accelerator.as_str(),
        variant.minimum_driver_version.as_deref(),
        variant.recommended_driver_version.as_deref(),
    ) {
        ("cuda", Some(minimum), Some(recommended)) => {
            let minimum = DriverVersion::parse(minimum).ok_or_else(|| {
                invalid(format!(
                    "variant {:?} has an invalid minimumDriverVersion",
                    variant.variant_id
                ))
            })?;
            let recommended = DriverVersion::parse(recommended).ok_or_else(|| {
                invalid(format!(
                    "variant {:?} has an invalid recommendedDriverVersion",
                    variant.variant_id
                ))
            })?;
            if minimum > recommended {
                return Err(invalid(format!(
                    "variant {:?} recommends a driver older than its minimum",
                    variant.variant_id
                )));
            }
        }
        ("cuda", None, None) if catalog_status != "installable" => {}
        ("cuda", _, _) => {
            return Err(invalid(format!(
                "CUDA variant {:?} must declare minimumDriverVersion and recommendedDriverVersion",
                variant.variant_id
            )));
        }
        (_, None, None) => {}
        _ => {
            return Err(invalid(format!(
                "non-CUDA variant {:?} must not declare CUDA driver requirements",
                variant.variant_id
            )));
        }
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
    match (
        asset.role.as_str(),
        asset.release_repository.as_deref(),
        asset.release_tag.as_deref(),
    ) {
        ("source", None, None) => {}
        ("source", _, _) => {
            return Err(invalid(format!(
                "source asset {:?} must use sourceRepository and sourceCommit metadata",
                asset.name
            )));
        }
        (_, Some(repository), Some(release_tag)) => {
            validate_release_asset_url(&asset.url, repository, release_tag, &asset.name)?;
        }
        _ => {
            return Err(invalid(format!(
                "release asset {:?} must declare releaseRepository and releaseTag",
                asset.name
            )));
        }
    }
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
        ("source", Some(_)) => {
            return Err(invalid(format!(
                "pinned source asset {:?} must not have companionFor",
                asset.name
            )));
        }
        _ => {}
    }
    if let Some(companion_for) = &asset.companion_for {
        require_nonempty(companion_for, "companionFor")?;
    }
    match (
        asset.role.as_str(),
        asset.source_repository.as_deref(),
        asset.source_commit.as_deref(),
    ) {
        ("source", Some(repository), Some(commit)) => {
            validate_source_repository(repository)?;
            if commit.len() != 40
                || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
                || commit.bytes().any(|byte| byte.is_ascii_uppercase())
            {
                return Err(invalid(format!(
                    "source asset {:?} must pin a 40-character lowercase commit",
                    asset.name
                )));
            }
            let expected = format!("https://github.com/{repository}/archive/{commit}.zip");
            if asset.url != expected {
                return Err(invalid(format!(
                    "source asset {:?} URL must point to its pinned repository commit",
                    asset.name
                )));
            }
        }
        ("source", _, _) => {
            return Err(invalid(format!(
                "source asset {:?} must declare sourceRepository and sourceCommit",
                asset.name
            )));
        }
        (_, None, None) => {}
        _ => {
            return Err(invalid(format!(
                "non-source asset {:?} must not declare source repository metadata",
                asset.name
            )));
        }
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

pub(super) fn validate_release_asset_url(
    url: &str,
    repository: &str,
    release_tag: &str,
    asset_name: &str,
) -> Result<(), CatalogError> {
    validate_source_repository(repository)?;
    require_nonempty(release_tag, "asset releaseTag")?;
    let encoded_name = encode_path_segment(asset_name);
    let expected =
        format!("https://github.com/{repository}/releases/download/{release_tag}/{encoded_name}");
    if url != expected {
        return Err(invalid(format!(
            "asset {asset_name:?} URL must point to {repository}@{release_tag}"
        )));
    }
    Ok(())
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
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
    const VLLM_MANIFEST: &[u8] = include_bytes!("../../resources/local-engines/vllm.json");
    const EXLLAMA_MANIFEST: &[u8] = include_bytes!("../../resources/local-engines/exllama.json");

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
    fn all_shipped_engine_catalogs_have_exact_release_asset_pins() {
        for (engine_id, bytes) in [
            ("llama_cpp", LLAMA_CPP_MANIFEST),
            ("vllm", VLLM_MANIFEST),
            ("exllama", EXLLAMA_MANIFEST),
        ] {
            let manifest = parse_manifest(bytes)
                .unwrap_or_else(|error| panic!("{engine_id} catalog should validate: {error}"));
            assert_eq!(manifest.engine_id, engine_id);
            assert!(
                manifest
                    .variants
                    .iter()
                    .flat_map(|variant| &variant.assets)
                    .all(|asset| asset.role == "source"
                        || (asset.release_repository.is_some() && asset.release_tag.is_some()))
            );
        }
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
    fn release_assets_must_match_their_declared_repository_tag_and_name() {
        let mut value = llama_value();
        let asset = &mut value["variants"][0]["assets"][0];
        asset["releaseRepository"] = Value::String("ggml-org/llama.cpp".into());
        asset["releaseTag"] = Value::String("different-tag".into());

        let bytes = serde_json::to_vec(&value).expect("test value should serialize");
        assert!(parse_manifest(&bytes).is_err());

        let mut value = llama_value();
        let asset = &mut value["variants"][0]["assets"][0];
        asset["releaseRepository"] = Value::String("ggml-org/llama.cpp".into());
        asset["releaseTag"] = Value::String("b11349".into());
        asset["url"] = Value::String(
            "https://github.com/other/repo/releases/download/b11349/llama-b11349-bin-win-cpu-x64.zip"
                .into(),
        );

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

    #[test]
    fn parses_two_and_three_part_driver_versions_and_orders_them() {
        assert_eq!(
            DriverVersion::parse("528.33"),
            Some(DriverVersion {
                major: 528,
                minor: 33,
                patch: 0,
            })
        );
        assert!(
            DriverVersion::parse("580.0").expect("valid version")
                < DriverVersion::parse("580.65.6").expect("valid version")
        );
        assert_eq!(
            DriverVersion::parse("615.1.2"),
            Some(DriverVersion {
                major: 615,
                minor: 1,
                patch: 2,
            })
        );
    }

    #[test]
    fn rejects_driver_version_values_with_non_numeric_parts() {
        for value in ["", "580", "580.x", "580.1.2.3", "-1.0", "580. 1"] {
            assert_eq!(DriverVersion::parse(value), None, "accepted {value:?}");
        }
    }

    #[test]
    fn rejects_installable_cuda_variants_without_driver_requirements() {
        let mut value = llama_value();
        let cuda = value["variants"][1]
            .as_object_mut()
            .expect("CUDA variant should be an object");
        cuda.remove("minimumDriverVersion");
        cuda.remove("recommendedDriverVersion");

        let bytes = serde_json::to_vec(&value).expect("test value should serialize");
        assert!(parse_manifest(&bytes).is_err());
    }
}
