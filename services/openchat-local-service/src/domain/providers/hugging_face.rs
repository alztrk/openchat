//! Searches Hugging Face model metadata and downloads validated model files.
//!
//! Repository paths and revisions are checked before use; downloads enforce
//! response bounds, resumable range validation, cancellation, and safe joining.

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use futures_util::StreamExt;
use reqwest::{
    Client, Response, StatusCode, Url,
    header::{CONTENT_RANGE, LINK, RANGE},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex, watch},
    task,
};

use crate::{
    local_engines,
    protocol::{EventSink, Response as RpcResponse, ServiceError},
    storage::AppStorage,
};

const HUB_BASE: &str = "https://huggingface.co";
const MODEL_SEARCH_LIMIT: &str = "24";
const MAX_API_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_MODEL_README_BYTES: usize = 512 * 1024;
const MAX_MODEL_SEARCH_CURSOR_LENGTH: usize = 4096;
const MAX_REPOSITORY_FILES: usize = 20_000;
const MAX_HUB_REDIRECTS: usize = 5;
const DOWNLOAD_PROGRESS_EVENT: &str = "models.hub.download.progress";
static MODEL_DOWNLOAD_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelFormat {
    Gguf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelSort {
    Downloads,
    Likes,
    RecentlyUpdated,
}

impl ModelSort {
    fn parse(value: &str) -> Result<Self, ServiceError> {
        match value {
            "downloads" => Ok(Self::Downloads),
            "likes" => Ok(Self::Likes),
            "recently_updated" => Ok(Self::RecentlyUpdated),
            _ => Err(invalid_request_error()),
        }
    }

    fn hub_value(self) -> &'static str {
        match self {
            Self::Downloads => "downloads",
            Self::Likes => "likes",
            Self::RecentlyUpdated => "lastModified",
        }
    }
}

impl ModelFormat {
    fn parse(value: &str) -> Result<Self, ServiceError> {
        match value {
            "gguf" => Ok(Self::Gguf),
            _ => Err(invalid_request_error()),
        }
    }

    fn filter(self) -> &'static str {
        match self {
            Self::Gguf => "gguf",
        }
    }

    fn engine_id(self) -> &'static str {
        match self {
            Self::Gguf => "llama_cpp",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HubFile {
    path: String,
    size: Option<u64>,
    sha256: Option<String>,
}

#[derive(Clone, Debug)]
struct DownloadGroup {
    id: String,
    display_name: String,
    files: Vec<HubFile>,
    total_size: Option<u64>,
}

#[derive(Debug)]
struct ModelDetails {
    repo_id: String,
    revision: String,
    gated: bool,
    private: bool,
    license: Option<String>,
    files: Vec<HubFile>,
    groups: Vec<DownloadGroup>,
}

/// Holds validated RPC fields and cancellation state for one model download.
pub(crate) struct ModelDownloadRequest<'a> {
    pub(crate) repo_id: &'a str,
    pub(crate) revision: &'a str,
    pub(crate) format: &'a str,
    pub(crate) group_id: &'a str,
    pub(crate) component_path: Option<&'a str>,
    pub(crate) model_directory: Option<&'a str>,
    pub(crate) request_id: &'a Value,
    pub(crate) events: &'a EventSink,
    pub(crate) cancellation: &'a mut watch::Receiver<bool>,
}

#[derive(Clone, Copy)]
/// Holds stable model-file metadata shared by progress events during one download.
struct DownloadProgressContext<'a> {
    events: &'a EventSink,
    request_id: &'a Value,
    details: &'a ModelDetails,
    file: &'a HubFile,
    total_bytes: u64,
    file_index: usize,
    file_count: usize,
}

enum ModelReadme {
    Available(String),
    Missing,
    AccessDenied,
    TooLarge,
    Unavailable(&'static str),
}

pub(crate) async fn search(
    query: &str,
    format: &str,
    sort: &str,
    cursor: Option<&str>,
) -> Result<Value, ServiceError> {
    let format = ModelFormat::parse(format)?;
    let sort = ModelSort::parse(sort)?;
    let query = query.trim();
    if query.chars().count() > 120 {
        return Err(invalid_request_error());
    }
    if cursor.is_some_and(|value| {
        value.is_empty()
            || value.len() > MAX_MODEL_SEARCH_CURSOR_LENGTH
            || value.chars().any(char::is_control)
    }) {
        return Err(invalid_request_error());
    }

    let client = client()?;
    let parameters = vec![
        ("filter", format.filter()),
        ("limit", MODEL_SEARCH_LIMIT),
        ("sort", sort.hub_value()),
        ("direction", "-1"),
    ];
    let mut request = client
        .get(format!("{HUB_BASE}/api/models"))
        .query(&parameters)
        .query(&[("search", query)]);
    if let Some(cursor) = cursor {
        request = request.query(&[("cursor", cursor)]);
    }
    let response = request
        .send()
        .await
        .map_err(hub_network_error_with_details)?;
    if !response.status().is_success() {
        return Err(http_status_error(response.status()));
    }
    let next_cursor = next_model_cursor(&response)?;
    let value = response_json(response).await?;
    let entries = value.as_array().ok_or_else(invalid_hub_response_error)?;
    let models = entries
        .iter()
        .map(parse_search_result)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(json!({
        "models": models,
        "nextCursor": next_cursor,
        "freshness": "current"
    }))
}

pub(crate) async fn files(repo_id: &str, format: &str) -> Result<Value, ServiceError> {
    let format = ModelFormat::parse(format)?;
    let details = fetch_details(repo_id, format).await?;
    let readme = if details.gated || details.private {
        ModelReadme::AccessDenied
    } else if let Some(file) = details
        .files
        .iter()
        .find(|file| file.path.eq_ignore_ascii_case("README.md"))
    {
        if file
            .size
            .is_some_and(|size| size > MAX_MODEL_README_BYTES as u64)
        {
            ModelReadme::TooLarge
        } else {
            match fetch_readme(&details, file).await {
                Ok(readme) => readme,
                Err(error) if error.code == "hugging_face_access_denied" => {
                    ModelReadme::AccessDenied
                }
                Err(error) if error.code == "hugging_face_model_unavailable" => {
                    ModelReadme::Missing
                }
                Err(error) => ModelReadme::Unavailable(error.code),
            }
        }
    } else {
        ModelReadme::Missing
    };
    Ok(details_to_json(&details, &readme))
}

pub(crate) async fn download(
    storage: &AppStorage,
    request: ModelDownloadRequest<'_>,
) -> Result<Value, ServiceError> {
    let ModelDownloadRequest {
        repo_id,
        revision,
        format,
        group_id,
        component_path,
        model_directory,
        request_id,
        events,
        cancellation,
    } = request;
    if !is_commit_sha(revision) {
        return Err(invalid_request_error());
    }
    let format = ModelFormat::parse(format)?;
    let details = fetch_details(repo_id, format).await?;
    if details.revision != revision {
        return Err(revision_changed_error());
    }
    if details.gated || details.private {
        return Err(hub_access_denied_error());
    }
    let _download_guard = tokio::select! {
        _ = cancellation.changed() => return Err(cancelled_error()),
        guard = MODEL_DOWNLOAD_LOCK.lock() => guard,
    };
    let model_group = details
        .groups
        .iter()
        .find(|group| group.id == group_id)
        .ok_or_else(unsupported_file_set_error)?;
    if model_group.files.is_empty()
        || model_group.total_size.is_none()
        || model_group
            .files
            .iter()
            .any(|file| file.size.is_none() || file.sha256.is_none())
    {
        return Err(unsupported_file_set_error());
    }

    let download_group = if let Some(component_path) = component_path {
        let file = details
            .files
            .iter()
            .find(|file| file.path == component_path && model_file_kind(&file.path) != "model")
            .ok_or_else(unsupported_file_set_error)?;
        if file.size.is_none() || file.sha256.is_none() {
            return Err(unsupported_file_set_error());
        }
        DownloadGroup {
            id: format!("component:{}", file.path),
            display_name: file.path.clone(),
            files: vec![file.clone()],
            total_size: file.size,
        }
    } else {
        model_group.clone()
    };

    check_cancelled(cancellation)?;
    let storage_root = storage.root();
    let engine_root = local_engines::models::resolve_model_directory(
        storage_root,
        format.engine_id(),
        model_directory,
    )?;
    let repo_slug = repo_slug(&details.repo_id)?;
    let revision_short = &details.revision[..12];
    let bundle_digest = Sha256::digest(format!(
        "{}\n{}\n{}",
        details.repo_id, details.revision, model_group.id
    ));
    let bundle_key = hex_prefix(&bundle_digest, 16);
    let staging_key = if let Some(component_path) = component_path {
        let digest = Sha256::digest(format!("{bundle_key}\n{component_path}"));
        hex_prefix(&digest, 16)
    } else {
        bundle_key.clone()
    };
    let download_root = engine_root.join(".downloads").join(staging_key);
    let destination = engine_root.join(format!("{repo_slug}-{revision_short}-{bundle_key}"));

    let pending_group = group_missing_from_bundle(&destination, &download_group)?;
    if !pending_group.files.is_empty() {
        fs::create_dir_all(&download_root)
            .await
            .map_err(|_| model_storage_error())?;
        download_group_files(
            &download_client()?,
            &details,
            &pending_group,
            &download_root,
            request_id,
            events,
            cancellation,
        )
        .await?;
        check_cancelled(cancellation)?;
        merge_download_group(&destination, &download_root, &pending_group).await?;
    }

    let model = if component_path.is_none() || bundle_group_is_complete(&destination, model_group)?
    {
        let registered_path = match format {
            ModelFormat::Gguf => {
                let primary_file = model_group
                    .files
                    .first()
                    .ok_or_else(unsupported_file_set_error)?;
                destination.join(&primary_file.path)
            }
        };
        Some(register_downloaded_model(storage, format.engine_id(), registered_path).await?)
    } else {
        None
    };

    Ok(json!({
        "engineId": format.engine_id(),
        "model": model,
        "repoId": details.repo_id,
        "revision": details.revision,
        "componentPath": component_path,
        "downloadedFiles": download_group.files.len(),
        "totalBytes": download_group.total_size,
    }))
}

fn client() -> Result<Client, ServiceError> {
    Client::builder()
        .user_agent("OpenChat model catalog")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(45))
        .redirect(hugging_face_redirect_policy())
        .build()
        .map_err(hub_network_error_with_details)
}

fn download_client() -> Result<Client, ServiceError> {
    Client::builder()
        .user_agent("OpenChat model downloader")
        .connect_timeout(Duration::from_secs(15))
        .redirect(hugging_face_redirect_policy())
        .build()
        .map_err(hub_network_error_with_details)
}

/// Restricts Hub redirects to HTTPS endpoints owned by Hugging Face.
pub(crate) fn hugging_face_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() > MAX_HUB_REDIRECTS {
            attempt.error("Hugging Face redirected too many times")
        } else if is_approved_hugging_face_redirect(attempt.url()) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

/// Checks the scheme, authority, port, and host of a Hub storage redirect.
fn is_approved_hugging_face_redirect(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && (host == "huggingface.co" || host == "hf.co" || host.ends_with(".hf.co"))
}

async fn response_json(response: Response) -> Result<Value, ServiceError> {
    if !response.status().is_success() {
        return Err(http_status_error(response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_API_RESPONSE_BYTES as u64)
    {
        return Err(invalid_hub_response_error());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(hub_network_error_with_details)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_API_RESPONSE_BYTES {
            return Err(invalid_hub_response_error());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid_hub_response_error())
}

fn next_model_cursor(response: &Response) -> Result<Option<String>, ServiceError> {
    for header in response.headers().get_all(LINK) {
        let header = header.to_str().map_err(|_| invalid_hub_response_error())?;
        for link in split_link_header(header) {
            let Some(target) = next_link_target(link)? else {
                continue;
            };
            let url = response
                .url()
                .join(target)
                .map_err(|_| invalid_hub_response_error())?;
            if url.scheme() != "https"
                || url.host_str() != Some("huggingface.co")
                || url.path() != "/api/models"
            {
                return Err(invalid_hub_response_error());
            }
            let cursor = url
                .query_pairs()
                .find_map(|(key, value)| (key == "cursor").then(|| value.into_owned()))
                .filter(|value| {
                    !value.is_empty()
                        && value.len() <= MAX_MODEL_SEARCH_CURSOR_LENGTH
                        && !value.chars().any(char::is_control)
                })
                .ok_or_else(invalid_hub_response_error)?;
            return Ok(Some(cursor));
        }
    }
    Ok(None)
}

fn split_link_header(header: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let mut start = 0;
    let mut inside_target = false;
    let mut inside_quote = false;
    let mut escaped = false;
    for (index, character) in header.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if inside_quote => escaped = true,
            '"' => inside_quote = !inside_quote,
            '<' if !inside_quote => inside_target = true,
            '>' if !inside_quote => inside_target = false,
            ',' if !inside_quote && !inside_target => {
                entries.push(header[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    entries.push(header[start..].trim());
    entries
}

fn next_link_target(link: &str) -> Result<Option<&str>, ServiceError> {
    let parameters = link.split(';').skip(1);
    let relation = parameters.filter_map(|parameter| {
        let (name, value) = parameter.trim().split_once('=')?;
        name.eq_ignore_ascii_case("rel")
            .then_some(value.trim().trim_matches('"'))
    });
    let is_next = relation
        .flat_map(str::split_ascii_whitespace)
        .any(|value| value.eq_ignore_ascii_case("next"));
    if !is_next {
        return Ok(None);
    }

    let open = link.find('<').ok_or_else(invalid_hub_response_error)?;
    let target_start = open + 1;
    let close = link[target_start..]
        .find('>')
        .map(|offset| offset + target_start)
        .ok_or_else(invalid_hub_response_error)?;
    if close == target_start {
        return Err(invalid_hub_response_error());
    }
    Ok(Some(&link[target_start..close]))
}

async fn fetch_readme(details: &ModelDetails, file: &HubFile) -> Result<ModelReadme, ServiceError> {
    let url = download_url(&details.repo_id, &details.revision, &file.path)?;
    let response = client()?
        .get(url)
        .send()
        .await
        .map_err(hub_network_error_with_details)?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(ModelReadme::Missing);
    }
    if !response.status().is_success() {
        return Err(http_status_error(response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MODEL_README_BYTES as u64)
    {
        return Ok(ModelReadme::TooLarge);
    }

    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(hub_network_error_with_details)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_MODEL_README_BYTES {
            return Ok(ModelReadme::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    let readme = String::from_utf8(bytes).map_err(|_| invalid_hub_response_error())?;
    Ok(ModelReadme::Available(readme))
}

fn parse_search_result(value: &Value) -> Result<Value, ServiceError> {
    let object = value.as_object().ok_or_else(invalid_hub_response_error)?;
    let repo_id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| valid_repo_id(value))
        .ok_or_else(invalid_hub_response_error)?;
    let card_data = object.get("cardData").and_then(Value::as_object);
    let license = card_data
        .and_then(|data| data.get("license"))
        .and_then(Value::as_str);
    let pipeline = object
        .get("pipeline_tag")
        .and_then(Value::as_str)
        .or_else(|| {
            card_data
                .and_then(|data| data.get("pipeline_tag"))?
                .as_str()
        });

    Ok(json!({
        "repoId": repo_id,
        "downloads": non_negative_integer(object.get("downloads")),
        "likes": non_negative_integer(object.get("likes")),
        "pipelineTag": pipeline,
        "libraryName": object.get("library_name").and_then(Value::as_str),
        "license": license,
        "gated": gated_value(object.get("gated")),
        "private": object.get("private").and_then(Value::as_bool).unwrap_or(false),
        "revision": object.get("sha").and_then(Value::as_str),
        "tags": object.get("tags").and_then(Value::as_array).map(|tags| {
            tags.iter().filter_map(Value::as_str).take(24).collect::<Vec<_>>()
        }).unwrap_or_default(),
    }))
}

async fn fetch_details(repo_id: &str, format: ModelFormat) -> Result<ModelDetails, ServiceError> {
    if !valid_repo_id(repo_id) {
        return Err(invalid_request_error());
    }
    let url = api_model_url(repo_id)?;
    let response = client()?
        .get(url)
        .query(&[("blobs", "true")])
        .send()
        .await
        .map_err(hub_network_error_with_details)?;
    let value = response_json(response).await?;
    let object = value.as_object().ok_or_else(invalid_hub_response_error)?;
    if object.get("id").and_then(Value::as_str) != Some(repo_id) {
        return Err(invalid_hub_response_error());
    }
    let revision = object
        .get("sha")
        .and_then(Value::as_str)
        .filter(|sha| is_commit_sha(sha))
        .ok_or_else(|| {
            ServiceError::new(
                "hugging_face_revision_unavailable",
                "The selected model revision could not be pinned safely.",
                true,
            )
        })?
        .to_owned();
    let gated = gated_value(object.get("gated"));
    let private = object
        .get("private")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let license = object
        .get("cardData")
        .and_then(Value::as_object)
        .and_then(|card| card.get("license"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let siblings = object
        .get("siblings")
        .and_then(Value::as_array)
        .ok_or_else(invalid_hub_response_error)?;
    if siblings.len() > MAX_REPOSITORY_FILES {
        return Err(repository_too_large_error());
    }
    let files = siblings
        .iter()
        .map(parse_hub_file)
        .collect::<Result<Vec<_>, _>>()?;
    let groups = compatible_groups(format, &files);

    Ok(ModelDetails {
        repo_id: repo_id.to_owned(),
        revision,
        gated,
        private,
        license,
        files,
        groups,
    })
}

fn parse_hub_file(value: &Value) -> Result<HubFile, ServiceError> {
    let object = value.as_object().ok_or_else(invalid_hub_response_error)?;
    let path = object
        .get("rfilename")
        .and_then(Value::as_str)
        .filter(|path| valid_hub_path(path))
        .ok_or_else(invalid_hub_response_error)?
        .to_owned();
    let lfs = object.get("lfs").and_then(Value::as_object);
    let size = object
        .get("size")
        .and_then(value_as_u64)
        .or_else(|| lfs.and_then(|lfs| lfs.get("size")).and_then(value_as_u64));
    let sha256 = parse_hub_file_sha256(object)?;
    Ok(HubFile { path, size, sha256 })
}

fn parse_hub_file_sha256(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<String>, ServiceError> {
    let lfs = object.get("lfs").and_then(Value::as_object);
    let raw_sha256 = lfs
        .and_then(|lfs| lfs.get("sha256").or_else(|| lfs.get("oid")))
        .or_else(|| object.get("sha256"));
    let Some(raw_sha256) = raw_sha256 else {
        return Ok(None);
    };
    if raw_sha256.is_null() {
        return Ok(None);
    }
    let digest = raw_sha256
        .as_str()
        .filter(|value| is_sha256_digest(value))
        .ok_or_else(invalid_hub_response_error)?;
    Ok(Some(digest.to_ascii_lowercase()))
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn compatible_groups(format: ModelFormat, files: &[HubFile]) -> Vec<DownloadGroup> {
    match format {
        ModelFormat::Gguf => gguf_groups(files),
    }
}

fn gguf_groups(files: &[HubFile]) -> Vec<DownloadGroup> {
    let mut groups = BTreeMap::<String, Vec<HubFile>>::new();
    for file in files
        .iter()
        .filter(|file| has_extension(&file.path, "gguf") && model_file_kind(&file.path) == "model")
    {
        let filename = file_name(&file.path);
        let key = shard_info(filename)
            .map(|(prefix, _, _)| format!("shard:{prefix}"))
            .unwrap_or_else(|| format!("file:{}", file.path));
        groups.entry(key).or_default().push(file.clone());
    }

    let mut result = Vec::new();
    for (key, mut group_files) in groups {
        let display_name = if let Some((prefix, _, total)) = group_files
            .first()
            .and_then(|file| shard_info(file_name(&file.path)))
        {
            group_files.sort_by_key(|file| shard_info(file_name(&file.path)).map(|x| x.1));
            let complete = group_files.len() == total
                && group_files.iter().enumerate().all(|(index, file)| {
                    shard_info(file_name(&file.path)).is_some_and(
                        |(candidate, shard_index, candidate_total)| {
                            candidate == prefix
                                && candidate_total == total
                                && shard_index == index + 1
                        },
                    )
                });
            if !complete {
                continue;
            }
            format!("{prefix} ({total} parts)")
        } else {
            group_files[0].path.clone()
        };
        result.push(DownloadGroup {
            id: group_id(&key),
            display_name,
            total_size: sum_sizes(&group_files),
            files: group_files,
        });
    }
    result.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    result
}

fn details_to_json(details: &ModelDetails, readme: &ModelReadme) -> Value {
    let (readme_status, readme_text, readme_error_code) = match readme {
        ModelReadme::Available(text) => ("available", Some(text.as_str()), None),
        ModelReadme::Missing => ("missing", None, None),
        ModelReadme::AccessDenied => ("access_denied", None, None),
        ModelReadme::TooLarge => ("too_large", None, None),
        ModelReadme::Unavailable(code) => ("unavailable", None, Some(*code)),
    };
    json!({
        "repoId": details.repo_id,
        "revision": details.revision,
        "gated": details.gated,
        "private": details.private,
        "license": details.license,
        "readme": readme_text,
        "readmeStatus": readme_status,
        "readmeErrorCode": readme_error_code,
        "files": details.files.iter().filter(|file| {
            has_extension(&file.path, "gguf") || is_model_metadata(&file.path)
        }).map(file_to_json).collect::<Vec<_>>(),
        "downloadGroups": details.groups.iter().map(|group| json!({
            "id": group.id,
            "displayName": group.display_name,
            "totalBytes": group.total_size,
            "files": group.files.iter().map(file_to_json).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn file_to_json(file: &HubFile) -> Value {
    json!({
        "path": file.path,
        "sizeBytes": file.size,
        "sha256": file.sha256,
        "kind": model_file_kind(&file.path),
    })
}

async fn download_group_files(
    client: &Client,
    details: &ModelDetails,
    group: &DownloadGroup,
    download_root: &Path,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<(), ServiceError> {
    let total_bytes = group.total_size.unwrap_or_default();
    let mut completed_bytes = 0_u64;
    let file_count = group.files.len();

    for (index, file) in group.files.iter().enumerate() {
        check_cancelled(cancellation)?;
        let target = safe_join(download_root, &file.path)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|_| model_storage_error())?;
        }
        let progress = DownloadProgressContext {
            events,
            request_id,
            details,
            file,
            total_bytes,
            file_index: index + 1,
            file_count,
        };
        if let Some(expected_size) = file.size
            && fs::metadata(&target)
                .await
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() == expected_size)
        {
            if verify_file_sha256(&target, expected_size, file.sha256.as_deref(), cancellation)
                .await?
            {
                completed_bytes = completed_bytes.saturating_add(expected_size);
                send_progress(progress, completed_bytes).await?;
                continue;
            }
            let _ = fs::remove_file(&target).await;
        }

        let part = partial_path(&target);
        let downloaded = download_file(
            client,
            &part,
            &target,
            completed_bytes,
            progress,
            cancellation,
        )
        .await?;
        completed_bytes = completed_bytes.saturating_add(downloaded);
    }
    Ok(())
}

async fn download_file(
    client: &Client,
    part_path: &Path,
    target_path: &Path,
    completed_bytes: u64,
    progress: DownloadProgressContext<'_>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<u64, ServiceError> {
    let file = progress.file;
    let details = progress.details;
    let url = download_url(&details.repo_id, &details.revision, &file.path)?;
    download_file_from_url(
        client,
        url,
        part_path,
        target_path,
        completed_bytes,
        progress,
        cancellation,
    )
    .await
}

async fn download_file_from_url(
    client: &Client,
    url: Url,
    part_path: &Path,
    target_path: &Path,
    completed_bytes: u64,
    progress: DownloadProgressContext<'_>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<u64, ServiceError> {
    let file = progress.file;
    let mut offset = fs::metadata(part_path)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    if file.size.is_some_and(|size| offset > size) {
        offset = 0;
    }

    let mut request = client.get(url);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let response = tokio::select! {
        _ = cancellation.changed() => return Err(cancelled_error()),
        response = request.send() => response.map_err(hub_network_error_with_details)?,
    };

    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE
        && file.size.is_some_and(|size| size == offset)
    {
        let actual_sha256 = if file.sha256.is_some() {
            let hasher = hash_file_prefix(part_path, offset, cancellation).await?;
            Some(hex_prefix(&hasher.finalize(), 32))
        } else {
            None
        };
        finish_partial(
            part_path,
            target_path,
            file.size,
            file.sha256.as_deref(),
            actual_sha256.as_deref(),
        )?;
        return Ok(offset);
    }
    if !response.status().is_success() {
        return Err(http_status_error(response.status()));
    }

    let append = if response.status() == StatusCode::PARTIAL_CONTENT {
        validate_content_range(&response, offset, file.size)?;
        offset > 0
    } else {
        offset = 0;
        false
    };
    let mut hasher = if append && file.sha256.is_some() {
        Some(hash_file_prefix(part_path, offset, cancellation).await?)
    } else if file.sha256.is_some() {
        Some(Sha256::new())
    } else {
        None
    };
    let mut output = open_partial(part_path, append).await?;
    let mut stream = response.bytes_stream();
    let mut current_bytes = offset;
    send_progress(progress, completed_bytes.saturating_add(current_bytes)).await?;

    loop {
        let next_chunk = tokio::select! {
            _ = cancellation.changed() => {
                output.flush().await.map_err(|_| model_storage_error())?;
                return Err(cancelled_error());
            }
            chunk = stream.next() => chunk,
        };
        let Some(chunk) = next_chunk else {
            break;
        };
        let chunk = chunk.map_err(hub_network_error_with_details)?;
        current_bytes = current_bytes
            .checked_add(chunk.len() as u64)
            .ok_or_else(download_size_error)?;
        if file.size.is_some_and(|size| current_bytes > size) {
            return Err(invalid_hub_response_error());
        }
        if let Some(hasher) = hasher.as_mut() {
            hasher.update(&chunk);
        }
        output
            .write_all(&chunk)
            .await
            .map_err(|_| model_storage_error())?;
        send_progress(progress, completed_bytes.saturating_add(current_bytes)).await?;
    }

    output.flush().await.map_err(|_| model_storage_error())?;
    output.sync_all().await.map_err(|_| model_storage_error())?;
    drop(output);
    let actual_sha256 = hasher.map(|digest| hex_prefix(&digest.finalize(), 32));
    finish_partial(
        part_path,
        target_path,
        file.size,
        file.sha256.as_deref(),
        actual_sha256.as_deref(),
    )?;
    Ok(current_bytes)
}

async fn hash_file_prefix(
    path: &Path,
    expected_bytes: u64,
    cancellation: &watch::Receiver<bool>,
) -> Result<Sha256, ServiceError> {
    let mut file = File::open(path).await.map_err(|_| model_storage_error())?;
    let mut hasher = Sha256::new();
    let mut remaining = expected_bytes;
    let mut buffer = vec![0_u8; 64 * 1024];

    while remaining > 0 {
        check_cancelled(cancellation)?;
        let limit = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = file
            .read(&mut buffer[..limit])
            .await
            .map_err(|_| model_storage_error())?;
        if read == 0 {
            return Err(incomplete_download_error());
        }
        hasher.update(&buffer[..read]);
        remaining = remaining.saturating_sub(read as u64);
    }
    Ok(hasher)
}

async fn verify_file_sha256(
    path: &Path,
    expected_bytes: u64,
    expected_sha256: Option<&str>,
    cancellation: &watch::Receiver<bool>,
) -> Result<bool, ServiceError> {
    let Some(expected_sha256) = expected_sha256 else {
        return Ok(true);
    };
    let hasher = hash_file_prefix(path, expected_bytes, cancellation).await?;
    let actual = hex_prefix(&hasher.finalize(), 32);
    Ok(actual == expected_sha256)
}

async fn open_partial(path: &Path, append: bool) -> Result<File, ServiceError> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(path)
        .await
        .map_err(|_| model_storage_error())
}

fn finish_partial(
    part_path: &Path,
    target_path: &Path,
    expected_size: Option<u64>,
    expected_sha256: Option<&str>,
    actual_sha256: Option<&str>,
) -> Result<(), ServiceError> {
    let metadata = std::fs::metadata(part_path).map_err(|_| model_storage_error())?;
    if !metadata.is_file() || expected_size.is_some_and(|size| size != metadata.len()) {
        return Err(incomplete_download_error());
    }
    if let Some(expected_sha256) = expected_sha256
        && actual_sha256 != Some(expected_sha256)
    {
        let _ = std::fs::remove_file(part_path);
        return Err(checksum_mismatch_error());
    }
    std::fs::rename(part_path, target_path).map_err(|_| model_storage_error())
}

fn validate_content_range(
    response: &Response,
    offset: u64,
    expected_total: Option<u64>,
) -> Result<(), ServiceError> {
    let range = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(invalid_hub_response_error)?;
    let (_, range) = range
        .split_once(' ')
        .ok_or_else(invalid_hub_response_error)?;
    let (bytes, total) = range
        .split_once('/')
        .ok_or_else(invalid_hub_response_error)?;
    let (start, _) = bytes
        .split_once('-')
        .ok_or_else(invalid_hub_response_error)?;
    let start = start
        .parse::<u64>()
        .map_err(|_| invalid_hub_response_error())?;
    let total = total
        .parse::<u64>()
        .map_err(|_| invalid_hub_response_error())?;
    if start != offset || expected_total.is_some_and(|expected| expected != total) {
        return Err(invalid_hub_response_error());
    }
    Ok(())
}

async fn send_progress(
    progress: DownloadProgressContext<'_>,
    downloaded_bytes: u64,
) -> Result<(), ServiceError> {
    let data = json!({
        "repoId": progress.details.repo_id,
        "fileName": file_name(&progress.file.path),
        "downloadedBytes": downloaded_bytes,
        "fileTotalBytes": progress.file.size,
        "totalDownloadedBytes": downloaded_bytes,
        "totalBytes": progress.total_bytes,
        "fileIndex": progress.file_index,
        "fileCount": progress.file_count,
    });
    progress
        .events
        .send(&RpcResponse::event(
            progress.request_id.clone(),
            DOWNLOAD_PROGRESS_EVENT,
            data,
        ))
        .await
        .map_err(|_| {
            ServiceError::new(
                "protocol_unavailable",
                "The local service could not deliver the model download progress update.",
                true,
            )
        })
}

async fn register_downloaded_model(
    storage: &AppStorage,
    engine_id: &'static str,
    path: PathBuf,
) -> Result<Value, ServiceError> {
    let storage_root = storage.root().to_path_buf();
    let database_queue = storage.database_queue_handle();
    let path = path.to_str().ok_or_else(model_storage_error)?.to_owned();
    let engine = engine_id.to_owned();
    let model = task::spawn_blocking(move || {
        local_engines::models::register_with_storage_action(
            database_queue,
            &storage_root,
            &engine,
            &path,
            None,
            "keep",
        )
    })
    .await
    .map_err(|_| model_storage_error())??;
    let available = local_engines::model_is_available(storage, &model)?;
    local_engines::models::to_json(&model, available)
}

fn group_missing_from_bundle(
    directory: &Path,
    group: &DownloadGroup,
) -> Result<DownloadGroup, ServiceError> {
    let mut files = Vec::new();
    for file in &group.files {
        let path = safe_join(directory, &file.path)?;
        if !bundle_file_is_present(&path, file)? {
            files.push(file.clone());
        }
    }
    Ok(DownloadGroup {
        id: group.id.clone(),
        display_name: group.display_name.clone(),
        total_size: sum_sizes(&files),
        files,
    })
}

fn bundle_group_is_complete(directory: &Path, group: &DownloadGroup) -> Result<bool, ServiceError> {
    for file in &group.files {
        let path = safe_join(directory, &file.path)?;
        if !bundle_file_is_present(&path, file)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn bundle_file_is_present(path: &Path, file: &HubFile) -> Result<bool, ServiceError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_file()
                && file.size.is_some_and(|size| metadata.len() == size) =>
        {
            Ok(true)
        }
        Ok(_) => Err(bundle_conflict_error()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(model_storage_error()),
    }
}

async fn merge_download_group(
    destination: &Path,
    download_root: &Path,
    group: &DownloadGroup,
) -> Result<(), ServiceError> {
    fs::create_dir_all(destination)
        .await
        .map_err(|_| model_storage_error())?;
    for file in &group.files {
        let target = safe_join(destination, &file.path)?;
        if bundle_file_is_present(&target, file)? {
            continue;
        }

        let source = safe_join(download_root, &file.path)?;
        let metadata =
            std::fs::symlink_metadata(&source).map_err(|_| incomplete_download_error())?;
        if !metadata.file_type().is_file() || file.size.is_some_and(|size| metadata.len() != size) {
            return Err(incomplete_download_error());
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|_| model_storage_error())?;
        }
        fs::rename(source, target)
            .await
            .map_err(|_| model_storage_error())?;
    }

    fs::remove_dir_all(download_root)
        .await
        .map_err(|_| model_storage_error())?;
    Ok(())
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, ServiceError> {
    if !valid_hub_path(relative) {
        return Err(invalid_request_error());
    }
    let path = Path::new(relative);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_request_error());
    }
    Ok(root.join(path))
}

fn api_model_url(repo_id: &str) -> Result<Url, ServiceError> {
    let mut url = Url::parse(HUB_BASE).map_err(|_| invalid_hub_response_error())?;
    let (owner, name) = repo_id.split_once('/').ok_or_else(invalid_request_error)?;
    url.path_segments_mut()
        .map_err(|_| invalid_hub_response_error())?
        .extend(["api", "models", owner, name]);
    Ok(url)
}

fn download_url(repo_id: &str, revision: &str, file_path: &str) -> Result<Url, ServiceError> {
    if !is_commit_sha(revision) || !valid_hub_path(file_path) {
        return Err(invalid_request_error());
    }
    let (owner, name) = repo_id.split_once('/').ok_or_else(invalid_request_error)?;
    let mut url = Url::parse(HUB_BASE).map_err(|_| invalid_hub_response_error())?;
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| invalid_hub_response_error())?;
    segments.extend([owner, name, "resolve", revision]);
    for segment in file_path.split('/') {
        segments.push(segment);
    }
    drop(segments);
    Ok(url)
}

fn valid_repo_id(repo_id: &str) -> bool {
    let Some((owner, name)) = repo_id.split_once('/') else {
        return false;
    };
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return false;
    }
    [owner, name].into_iter().all(|segment| {
        segment != "."
            && segment != ".."
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    })
}

fn valid_hub_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|segment| {
            if segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.ends_with('.')
                || segment.ends_with(' ')
                || segment
                    .chars()
                    .any(|character| matches!(character, '<' | '>' | '"' | '|' | '?' | '*'))
            {
                return false;
            }
            let stem = segment.split('.').next().unwrap_or_default();
            !is_windows_device_name(stem)
        })
}

fn is_windows_device_name(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            upper.strip_prefix(prefix).is_some_and(|suffix| {
                suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9')
            })
        })
}

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn repo_slug(repo_id: &str) -> Result<String, ServiceError> {
    if !valid_repo_id(repo_id) {
        return Err(invalid_request_error());
    }
    Ok(repo_id.replace('/', "--"))
}

fn group_id(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    format!("gguf-{}", hex_prefix(&digest, 16))
}

fn hex_prefix(bytes: &[u8], count: usize) -> String {
    bytes
        .iter()
        .take(count)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn shard_info(filename: &str) -> Option<(String, usize, usize)> {
    let stem = filename.strip_suffix(".gguf")?;
    let (indexed, total) = stem.rsplit_once("-of-")?;
    let (prefix, index) = indexed.rsplit_once('-')?;
    let index = index.parse::<usize>().ok()?;
    let total = total.parse::<usize>().ok()?;
    if prefix.is_empty() || index == 0 || total < 2 || index > total {
        return None;
    }
    Some((prefix.to_owned(), index, total))
}

fn model_file_kind(path: &str) -> &'static str {
    let normalized = path.to_ascii_lowercase();
    let tokens = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.contains(&"mtp") {
        "mtp"
    } else if ["vision", "mmproj", "projector"]
        .iter()
        .any(|marker| tokens.contains(marker))
    {
        "vision"
    } else if ["draft", "speculator", "medusa", "adapter", "lora"]
        .iter()
        .any(|marker| tokens.contains(marker))
    {
        "auxiliary"
    } else {
        "model"
    }
}

fn is_model_metadata(path: &str) -> bool {
    if path.contains('/') {
        return false;
    }
    matches!(
        path.to_ascii_lowercase().as_str(),
        "config.json"
            | "generation_config.json"
            | "tokenizer.json"
            | "tokenizer_config.json"
            | "special_tokens_map.json"
            | "added_tokens.json"
            | "chat_template.jinja"
            | "vocab.json"
            | "merges.txt"
            | "tokenizer.model"
            | "spiece.model"
            | "sentencepiece.bpe.model"
            | "preprocessor_config.json"
            | "processor_config.json"
    )
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn sum_sizes(files: &[HubFile]) -> Option<u64> {
    files
        .iter()
        .try_fold(0_u64, |total, file| total.checked_add(file.size?))
}

fn value_as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
}

fn non_negative_integer(value: Option<&Value>) -> u64 {
    value.and_then(value_as_u64).unwrap_or_default()
}

fn gated_value(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => !value.is_empty() && value != "false",
        _ => false,
    }
}

fn partial_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".openchat-partial");
    target.with_file_name(name)
}

fn check_cancelled(cancellation: &watch::Receiver<bool>) -> Result<(), ServiceError> {
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }
    Ok(())
}

fn http_status_error(status: StatusCode) -> ServiceError {
    if status == StatusCode::TOO_MANY_REQUESTS {
        ServiceError::new(
            "hugging_face_rate_limited",
            "Hugging Face is temporarily rate-limiting requests. Try again shortly.",
            true,
        )
    } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        hub_access_denied_error()
    } else if status == StatusCode::NOT_FOUND {
        ServiceError::new(
            "hugging_face_model_unavailable",
            "The selected Hugging Face model or file is unavailable.",
            false,
        )
    } else {
        ServiceError::new(
            "hugging_face_unavailable",
            "Hugging Face could not complete the request.",
            status.is_server_error(),
        )
    }
}

fn invalid_request_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_request_invalid",
        "The Hugging Face model request is invalid.",
        false,
    )
}

fn invalid_hub_response_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_response_invalid",
        "Hugging Face returned model information OpenChat could not read.",
        true,
    )
}

fn hub_network_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_unavailable",
        "Hugging Face could not be reached. Check the connection and try again.",
        true,
    )
}

fn hub_network_error_with_details(error: reqwest::Error) -> ServiceError {
    hub_network_error().with_details(error.without_url().to_string())
}

fn hub_access_denied_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_access_denied",
        "This repository requires Hugging Face access that is not connected in OpenChat.",
        false,
    )
}

fn revision_changed_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_revision_changed",
        "The selected Hugging Face model changed. Reload its details and try again.",
        true,
    )
}

fn unsupported_file_set_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_model_format_unsupported",
        "This repository does not contain a complete model file set for the selected format.",
        false,
    )
}

fn repository_too_large_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_repository_too_large",
        "This repository contains too many files to inspect safely.",
        false,
    )
}

fn model_storage_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_model_storage_unavailable",
        "The model download could not be saved in OpenChat's model folder.",
        true,
    )
}

fn bundle_conflict_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_download_conflict",
        "A different or incomplete download already occupies this model folder.",
        false,
    )
}

fn incomplete_download_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_download_incomplete",
        "The downloaded file size did not match Hugging Face's file metadata.",
        true,
    )
}

fn checksum_mismatch_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_checksum_mismatch",
        "The downloaded file failed SHA-256 integrity verification.",
        true,
    )
}

fn download_size_error() -> ServiceError {
    ServiceError::new(
        "hugging_face_download_size_invalid",
        "The downloaded model file is too large to represent safely.",
        false,
    )
}

fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "The model download was cancelled.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tokio::{
        fs,
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::watch,
    };
    use url::Url;
    use uuid::Uuid;

    use crate::protocol::EventSink;

    use super::{
        DownloadProgressContext, HubFile, ModelDetails, client, download_client,
        download_file_from_url, hex_prefix, is_approved_hugging_face_redirect, parse_hub_file,
    };

    #[test]
    fn parse_hub_file_extracts_lfs_sha256_and_rejects_malformed_digests() {
        let valid_sha = "0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef";
        let file = parse_hub_file(&json!({
            "rfilename": "model-Q4_K_M.gguf",
            "lfs": {
                "size": 4096,
                "sha256": valid_sha
            }
        }))
        .expect("parse valid LFS file");
        assert_eq!(file.path, "model-Q4_K_M.gguf");
        assert_eq!(file.size, Some(4096));
        assert_eq!(
            file.sha256.as_deref(),
            Some(valid_sha.to_ascii_lowercase().as_str())
        );

        let malformed = parse_hub_file(&json!({
            "rfilename": "model-Q4_K_M.gguf",
            "size": 4096,
            "lfs": {
                "sha256": "not-a-valid-sha256"
            }
        }))
        .expect_err("reject malformed sha256");
        assert_eq!(malformed.code, "hugging_face_response_invalid");
    }

    #[tokio::test]
    async fn download_file_verifies_sha256_across_resumed_partial_content() {
        let payload = b"GGUF-resumable-verified-payload-bytes";
        let expected_sha256 = hex_prefix(&Sha256::digest(payload), 32);
        let prefix_len = 15;
        let remaining_payload = &payload[prefix_len..];
        let total_len = payload.len();

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local resume fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept resume request");
            let headers = read_request_headers(&mut stream).await;
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains(&format!("range: bytes={prefix_len}-")),
                "expected Range header in {headers}"
            );
            let response_header = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {prefix_len}-{}/{total_len}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                total_len - 1,
                remaining_payload.len()
            );
            stream
                .write_all(response_header.as_bytes())
                .await
                .expect("write partial content headers");
            stream
                .write_all(remaining_payload)
                .await
                .expect("write remaining payload");
        });

        let temp_dir = TestTempDir::new();
        let part_path = temp_dir.0.join("model.gguf.openchat-partial");
        let target_path = temp_dir.0.join("model.gguf");
        fs::write(&part_path, &payload[..prefix_len])
            .await
            .expect("seed partial file");

        let file = HubFile {
            path: "model.gguf".to_owned(),
            size: Some(total_len as u64),
            sha256: Some(expected_sha256),
        };
        let details = sample_details(file.clone());
        let events = EventSink::new();
        let request_id = json!("req-resume-sha256");
        let (_tx, mut cancellation) = watch::channel(false);
        let progress = DownloadProgressContext {
            events: &events,
            request_id: &request_id,
            details: &details,
            file: &file,
            total_bytes: total_len as u64,
            file_index: 1,
            file_count: 1,
        };

        let downloaded = download_file_from_url(
            &download_client().expect("build download client"),
            Url::parse(&format!("http://{address}/model.gguf")).expect("parse fixture url"),
            &part_path,
            &target_path,
            0,
            progress,
            &mut cancellation,
        )
        .await
        .expect("resumed download with matching sha256 should succeed");

        assert_eq!(downloaded, total_len as u64);
        assert!(
            !part_path.exists(),
            "partial file should be moved to target"
        );
        let saved = fs::read(&target_path).await.expect("read completed target");
        assert_eq!(saved, payload);
        server.await.expect("server task should complete");
    }

    #[tokio::test]
    async fn download_file_rejects_sha256_mismatch_and_removes_corrupt_partial_file() {
        let payload = b"GGUF-corrupted-transfer-bytes";
        let wrong_sha256 = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local mismatch fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept download request");
            read_request_headers(&mut stream).await;
            let response_header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            stream
                .write_all(response_header.as_bytes())
                .await
                .expect("write response headers");
            stream.write_all(payload).await.expect("write payload");
        });

        let temp_dir = TestTempDir::new();
        let part_path = temp_dir.0.join("corrupt.gguf.openchat-partial");
        let target_path = temp_dir.0.join("corrupt.gguf");
        let file = HubFile {
            path: "corrupt.gguf".to_owned(),
            size: Some(payload.len() as u64),
            sha256: Some(wrong_sha256.to_owned()),
        };
        let details = sample_details(file.clone());
        let events = EventSink::new();
        let request_id = json!("req-mismatch-sha256");
        let (_tx, mut cancellation) = watch::channel(false);
        let progress = DownloadProgressContext {
            events: &events,
            request_id: &request_id,
            details: &details,
            file: &file,
            total_bytes: payload.len() as u64,
            file_index: 1,
            file_count: 1,
        };

        let error = download_file_from_url(
            &download_client().expect("build download client"),
            Url::parse(&format!("http://{address}/corrupt.gguf")).expect("parse fixture url"),
            &part_path,
            &target_path,
            0,
            progress,
            &mut cancellation,
        )
        .await
        .expect_err("sha256 mismatch should fail the download");

        assert_eq!(error.code, "hugging_face_checksum_mismatch");
        assert!(!part_path.exists(), "corrupt partial file must be removed");
        assert!(
            !target_path.exists(),
            "corrupt file must not be promoted to target"
        );
        server.await.expect("server task should complete");
    }

    #[test]
    fn redirect_policy_allows_documented_hub_storage_hosts_only_over_https() {
        for url in [
            "https://huggingface.co/model/file.gguf",
            "https://hf.co/model/file.gguf",
            "https://cas-server.xethub.hf.co/model/file.gguf",
            "https://us.aws.cdn.hf.co/model/file.gguf",
        ] {
            assert!(
                is_approved_hugging_face_redirect(&Url::parse(url).expect("valid URL")),
                "expected {url} to be accepted"
            );
        }
    }

    #[test]
    fn redirect_policy_rejects_untrusted_hosts_and_url_authority_changes() {
        for url in [
            "http://cdn-lfs-us-1.hf.co/model/file.gguf",
            "https://huggingface.co.attacker.example/model/file.gguf",
            "https://not-hf.co/model/file.gguf",
            "https://huggingface.co:8443/model/file.gguf",
            "https://user@huggingface.co/model/file.gguf",
        ] {
            assert!(
                !is_approved_hugging_face_redirect(&Url::parse(url).expect("valid URL")),
                "expected {url} to be rejected"
            );
        }
    }

    #[tokio::test]
    async fn hub_client_does_not_follow_a_redirect_to_a_local_address() {
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

        let response = client()
            .expect("build Hub client")
            .get(format!("http://{address}/models"))
            .send()
            .await
            .expect("rejected redirect should be returned as-is");

        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        assert_eq!(response.url().host_str(), Some("127.0.0.1"));
        server.await.expect("redirect fixture should complete");
    }

    /// Reads the loopback request before writing a deterministic fixture response.
    async fn read_request_headers(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        let mut byte = [0; 1];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut byte).await.expect("read request headers");
            assert_ne!(read, 0, "client closed before sending request headers");
            request.push(byte[0]);
        }
        String::from_utf8_lossy(&request).into_owned()
    }

    fn sample_details(file: HubFile) -> ModelDetails {
        ModelDetails {
            repo_id: "openchat/test-model".to_owned(),
            revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            gated: false,
            private: false,
            license: Some("apache-2.0".to_owned()),
            files: vec![file],
            groups: Vec::new(),
        }
    }

    struct TestTempDir(PathBuf);

    impl TestTempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("openchat-hf-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).expect("create temp test dir");
            Self(dir)
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
