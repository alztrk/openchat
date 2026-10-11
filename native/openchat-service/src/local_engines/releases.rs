use serde::Deserialize;

use super::catalog::{EngineAsset, EngineManifest, parse_manifest};

const RELEASE_LIST_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

pub(super) async fn refresh_compatible_release(
    manifest: &mut EngineManifest,
) -> Result<String, ()> {
    let profiles = manifest
        .variants
        .iter()
        .map(|variant| {
            let asset = variant
                .assets
                .iter()
                .find(|asset| asset.role == "primary")
                .ok_or(())?;
            asset_profile(&manifest.engine_id, &asset.name)
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page={RELEASE_LIST_LIMIT}",
        manifest.source_repository
    );
    let client = reqwest::Client::builder()
        .user_agent("OpenChat local engine release checker")
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|_| ())?;
    let response = client.get(url).send().await.map_err(|_| ())?;
    if !response.status().is_success() {
        return Err(());
    }
    let releases = response
        .json::<Vec<GitHubRelease>>()
        .await
        .map_err(|_| ())?;
    let latest_upstream = releases
        .iter()
        .find(|release| !release.draft && !release.prerelease)
        .ok_or(())?;
    validate_release_tag(&latest_upstream.tag_name)?;

    let compatible =
        select_compatible_release(&releases, &profiles, &manifest.source_repository).ok_or(())?;

    manifest.release_tag.clone_from(&compatible.0.tag_name);
    manifest.release_url.clone_from(&compatible.0.html_url);
    for (variant, release_asset) in manifest.variants.iter_mut().zip(compatible.1) {
        if let Some(asset) = variant
            .assets
            .iter_mut()
            .find(|asset| asset.role == "primary")
        {
            *asset = release_asset;
        }
    }
    let latest_tag = latest_upstream.tag_name.clone();
    let manifest_bytes = serde_json::to_vec(manifest).map_err(|_| ())?;
    *manifest = parse_manifest(&manifest_bytes).map_err(|_| ())?;
    Ok(latest_tag)
}

fn select_compatible_release<'a>(
    releases: &'a [GitHubRelease],
    profiles: &[(String, String)],
    repository: &str,
) -> Option<(&'a GitHubRelease, Vec<EngineAsset>)> {
    releases
        .iter()
        .filter(|release| !release.draft && !release.prerelease)
        .find_map(|release| {
            let version = release_version(&release.tag_name)?;
            let assets = profiles
                .iter()
                .map(|(asset_prefix, asset_suffix)| {
                    let expected_name = format!("{asset_prefix}{version}{asset_suffix}");
                    let asset = release.assets.iter().find(|asset| {
                        asset.name == expected_name
                            && asset.size > 0
                            && asset.digest.as_deref().is_some_and(valid_sha256_digest)
                    })?;
                    Some(EngineAsset {
                        name: asset.name.clone(),
                        url: asset.browser_download_url.clone(),
                        release_repository: Some(repository.to_owned()),
                        release_tag: Some(release.tag_name.clone()),
                        size_bytes: asset.size,
                        sha256: asset.digest.clone()?,
                        role: "primary".to_owned(),
                        companion_for: None,
                        source_repository: None,
                        source_commit: None,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some((release, assets))
        })
}

fn asset_profile(engine_id: &str, asset_name: &str) -> Result<(String, String), ()> {
    let package = match engine_id {
        "vllm" => "vllm-",
        "exllama" => "exllamav3-",
        _ => return Err(()),
    };
    let without_package = asset_name.strip_prefix(package).ok_or(())?;
    let version_end = without_package.find(['+', '-']).ok_or(())?;
    let suffix = without_package[version_end..].to_owned();
    Ok((package.to_owned(), suffix))
}

fn release_version(tag: &str) -> Option<&str> {
    let version = tag.strip_prefix('v')?;
    if version.is_empty()
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
        || version.starts_with('.')
        || version.ends_with('.')
        || version.contains("..")
    {
        return None;
    }
    Some(version)
}

fn validate_release_tag(tag: &str) -> Result<(), ()> {
    release_version(tag).map(|_| ()).ok_or(())
}

fn valid_sha256_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(|value| {
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

#[cfg(test)]
mod tests {
    use super::{
        GitHubRelease, asset_profile, release_version, select_compatible_release,
        valid_sha256_digest,
    };

    #[test]
    fn extracts_only_the_platform_and_dependency_profile_from_asset_names() {
        assert_eq!(
            asset_profile(
                "exllama",
                "exllamav3-1.5.2+cu128.torch2.9.0-cp312-cp312-win_amd64.whl"
            ),
            Ok((
                "exllamav3-".to_owned(),
                "+cu128.torch2.9.0-cp312-cp312-win_amd64.whl".to_owned()
            ))
        );
        assert_eq!(
            asset_profile("vllm", "vllm-0.30.0-cp38-abi3-manylinux_2_28_x86_64.whl"),
            Ok((
                "vllm-".to_owned(),
                "-cp38-abi3-manylinux_2_28_x86_64.whl".to_owned()
            ))
        );
    }

    #[test]
    fn release_tags_and_hashes_are_validated_before_selection() {
        assert_eq!(release_version("v1.5.4"), Some("1.5.4"));
        assert_eq!(release_version("v1.5.4-rc1"), None);
        assert_eq!(release_version("v../1"), None);
        assert!(valid_sha256_digest(&format!("sha256:{}", "a".repeat(64))));
        assert!(!valid_sha256_digest("sha1:abc"));
    }

    #[test]
    fn chooses_the_newest_release_with_every_required_profile_and_digest() {
        let releases = serde_json::from_value::<Vec<GitHubRelease>>(serde_json::json!([
            {
                "tag_name": "v1.5.4",
                "html_url": "https://github.com/turboderp-org/exllamav3/releases/tag/v1.5.4",
                "draft": false,
                "prerelease": false,
                "assets": [
                    {"name": "exllamav3-1.5.4+cu128.torch2.10.0-cp312-cp312-win_amd64.whl", "browser_download_url": "https://github.com/turboderp-org/exllamav3/releases/download/v1.5.4/win.whl", "size": 20, "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                    {"name": "exllamav3-1.5.4+cu128.torch2.10.0-cp312-cp312-linux_x86_64.whl", "browser_download_url": "https://github.com/turboderp-org/exllamav3/releases/download/v1.5.4/linux.whl", "size": 20, "digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}
                ]
            },
            {
                "tag_name": "v1.5.2",
                "html_url": "https://github.com/turboderp-org/exllamav3/releases/tag/v1.5.2",
                "draft": false,
                "prerelease": false,
                "assets": [
                    {"name": "exllamav3-1.5.2+cu128.torch2.9.0-cp312-cp312-win_amd64.whl", "browser_download_url": "https://github.com/turboderp-org/exllamav3/releases/download/v1.5.2/win.whl", "size": 20, "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                    {"name": "exllamav3-1.5.2+cu128.torch2.9.0-cp312-cp312-linux_x86_64.whl", "browser_download_url": "https://github.com/turboderp-org/exllamav3/releases/download/v1.5.2/linux.whl", "size": 20, "digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}
                ]
            }
        ]))
        .expect("release fixture should deserialize");
        let profiles = [
            asset_profile(
                "exllama",
                "exllamav3-1.5.2+cu128.torch2.9.0-cp312-cp312-win_amd64.whl",
            )
            .expect("Windows profile should parse"),
            asset_profile(
                "exllama",
                "exllamav3-1.5.2+cu128.torch2.9.0-cp312-cp312-linux_x86_64.whl",
            )
            .expect("Linux profile should parse"),
        ];

        let (release, assets) =
            select_compatible_release(&releases, &profiles, "turboderp-org/exllamav3")
                .expect("a compatible stable release should be selected");

        assert_eq!(release.tag_name, "v1.5.2");
        assert_eq!(assets.len(), 2);
        assert!(
            assets
                .iter()
                .all(|asset| asset.release_tag.as_deref() == Some("v1.5.2"))
        );
    }
}
