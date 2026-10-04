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
    io::AsyncWriteExt,
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
const DOWNLOAD_PROGRESS_EVENT: &str = "models.hub.download.progress";
static MODEL_DOWNLOAD_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelFormat {
    Gguf,
    Transformers,
    Exllama,
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
            "transformers" => Ok(Self::Transformers),
            "exllama" => Ok(Self::Exllama),
            _ => Err(invalid_request_error()),
        }
    }

    fn filter(self) -> &'static str {
        match self {
            Self::Gguf => "gguf",
            Self::Transformers => "transformers",
            Self::Exllama => "exl3",
        }
    }

    fn engine_id(self) -> &'static str {
        match self {
            Self::Gguf => "llama_cpp",
            Self::Transformers => "vllm",
            Self::Exllama => "exllama",
        }
    }
}

#[derive(Clone, Debug)]
struct HubFile {
    path: String,
    size: Option<u64>,
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
    let mut parameters = vec![
        ("filter", format.filter()),
        ("limit", MODEL_SEARCH_LIMIT),
        ("sort", sort.hub_value()),
        ("direction", "-1"),
    ];
    if format == ModelFormat::Transformers {
        parameters.push(("pipeline_tag", "text-generation"));
    }
    let mut request = client
        .get(format!("{HUB_BASE}/api/models"))
        .query(&parameters)
        .query(&[("search", query)]);
    if let Some(cursor) = cursor {
        request = request.query(&[("cursor", cursor)]);
    }
    let response = request.send().await.map_err(|_| hub_network_error())?;
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
    repo_id: &str,
    revision: &str,
    format: &str,
    group_id: &str,
    component_path: Option<&str>,
    model_directory: Option<&str>,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
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
        || model_group.files.iter().any(|file| file.size.is_none())
    {
        return Err(unsupported_file_set_error());
    }

    let download_group = if let Some(component_path) = component_path {
        let file = details
            .files
            .iter()
            .find(|file| file.path == component_path && model_file_kind(&file.path) != "model")
            .ok_or_else(unsupported_file_set_error)?;
        if file.size.is_none() {
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
            ModelFormat::Transformers | ModelFormat::Exllama => destination.clone(),
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
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| hub_network_error())
}

fn download_client() -> Result<Client, ServiceError> {
    Client::builder()
        .user_agent("OpenChat model downloader")
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| hub_network_error())
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
        let chunk = chunk.map_err(|_| hub_network_error())?;
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
        .map_err(|_| hub_network_error())?;
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
        let chunk = chunk.map_err(|_| hub_network_error())?;
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
        .map_err(|_| hub_network_error())?;
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
    let size = object.get("size").and_then(value_as_u64);
    Ok(HubFile { path, size })
}

fn compatible_groups(format: ModelFormat, files: &[HubFile]) -> Vec<DownloadGroup> {
    match format {
        ModelFormat::Gguf => gguf_groups(files),
        ModelFormat::Transformers => transformers_group(files, false),
        ModelFormat::Exllama => transformers_group(files, true),
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

fn transformers_group(files: &[HubFile], exllama: bool) -> Vec<DownloadGroup> {
    let has_safetensors = files
        .iter()
        .any(|file| model_file_kind(&file.path) == "model" && is_model_weight(file, "safetensors"));
    let weight_extension = if has_safetensors || exllama {
        "safetensors"
    } else {
        "bin"
    };
    let weights = files
        .iter()
        .filter(|file| {
            model_file_kind(&file.path) == "model" && is_model_weight(file, weight_extension)
        })
        .cloned()
        .collect::<Vec<_>>();
    if weights.is_empty() || !has_root_file(files, "config.json") {
        return Vec::new();
    }
    if exllama && !has_tokenizer_file(files) {
        return Vec::new();
    }

    let mut selected = weights;
    let index_name = if weight_extension == "safetensors" {
        "model.safetensors.index.json"
    } else {
        "pytorch_model.bin.index.json"
    };
    if let Some(index_file) = files.iter().find(|file| file.path == index_name) {
        selected.push(index_file.clone());
    }
    selected.extend(
        files
            .iter()
            .filter(|file| is_model_metadata(&file.path))
            .cloned(),
    );
    selected.sort_by(|left, right| left.path.cmp(&right.path));
    selected.dedup_by(|left, right| left.path == right.path);

    let label = if exllama {
        "EXL3 model snapshot"
    } else if weight_extension == "safetensors" {
        "Transformers · safetensors"
    } else {
        "Transformers · PyTorch weights"
    };
    vec![DownloadGroup {
        id: if exllama {
            "exl3-snapshot"
        } else {
            "transformers-snapshot"
        }
        .to_owned(),
        display_name: label.to_owned(),
        total_size: sum_sizes(&selected),
        files: selected,
    }]
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
            has_extension(&file.path, "gguf") || is_model_metadata(&file.path) ||
                is_model_weight(file, "safetensors") || is_model_weight(file, "bin") ||
                is_auxiliary_model_weight(file)
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
        if let Some(expected_size) = file.size
            && fs::metadata(&target)
                .await
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() == expected_size)
        {
            completed_bytes = completed_bytes.saturating_add(expected_size);
            send_progress(
                events,
                request_id,
                details,
                file,
                completed_bytes,
                total_bytes,
                index + 1,
                file_count,
            )
            .await?;
            continue;
        }

        let part = partial_path(&target);
        let downloaded = download_file(
            client,
            details,
            file,
            &part,
            &target,
            completed_bytes,
            total_bytes,
            index + 1,
            file_count,
            request_id,
            events,
            cancellation,
        )
        .await?;
        completed_bytes = completed_bytes.saturating_add(downloaded);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_file(
    client: &Client,
    details: &ModelDetails,
    file: &HubFile,
    part_path: &Path,
    target_path: &Path,
    completed_bytes: u64,
    total_bytes: u64,
    file_index: usize,
    file_count: usize,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<u64, ServiceError> {
    let mut offset = fs::metadata(part_path)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    if file.size.is_some_and(|size| offset > size) {
        offset = 0;
    }

    let url = download_url(&details.repo_id, &details.revision, &file.path)?;
    let mut request = client.get(url);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let response = tokio::select! {
        _ = cancellation.changed() => return Err(cancelled_error()),
        response = request.send() => response.map_err(|_| hub_network_error())?,
    };

    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE
        && file.size.is_some_and(|size| size == offset)
    {
        finish_partial(part_path, target_path, file.size)?;
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
    let mut output = open_partial(part_path, append).await?;
    let mut stream = response.bytes_stream();
    let mut current_bytes = offset;
    send_progress(
        events,
        request_id,
        details,
        file,
        completed_bytes.saturating_add(current_bytes),
        total_bytes,
        file_index,
        file_count,
    )
    .await?;

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
        let chunk = chunk.map_err(|_| hub_network_error())?;
        current_bytes = current_bytes
            .checked_add(chunk.len() as u64)
            .ok_or_else(download_size_error)?;
        if file.size.is_some_and(|size| current_bytes > size) {
            return Err(invalid_hub_response_error());
        }
        output
            .write_all(&chunk)
            .await
            .map_err(|_| model_storage_error())?;
        send_progress(
            events,
            request_id,
            details,
            file,
            completed_bytes.saturating_add(current_bytes),
            total_bytes,
            file_index,
            file_count,
        )
        .await?;
    }

    output.flush().await.map_err(|_| model_storage_error())?;
    output.sync_all().await.map_err(|_| model_storage_error())?;
    drop(output);
    finish_partial(part_path, target_path, file.size)?;
    Ok(current_bytes)
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
) -> Result<(), ServiceError> {
    let metadata = std::fs::metadata(part_path).map_err(|_| model_storage_error())?;
    if !metadata.is_file() || expected_size.is_some_and(|size| size != metadata.len()) {
        return Err(incomplete_download_error());
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
    events: &EventSink,
    request_id: &Value,
    details: &ModelDetails,
    file: &HubFile,
    downloaded_bytes: u64,
    total_bytes: u64,
    file_index: usize,
    file_count: usize,
) -> Result<(), ServiceError> {
    let data = json!({
        "repoId": details.repo_id,
        "fileName": file_name(&file.path),
        "downloadedBytes": downloaded_bytes,
        "fileTotalBytes": file.size,
        "totalDownloadedBytes": downloaded_bytes,
        "totalBytes": total_bytes,
        "fileIndex": file_index,
        "fileCount": file_count,
    });
    events
        .send(&RpcResponse::event(
            request_id.clone(),
            DOWNLOAD_PROGRESS_EVENT,
            data,
        ))
        .await
        .map_err(|_| hub_network_error())
}

async fn register_downloaded_model(
    storage: &AppStorage,
    engine_id: &'static str,
    path: PathBuf,
) -> Result<Value, ServiceError> {
    let storage_root = storage.root().to_path_buf();
    let path = path.to_str().ok_or_else(model_storage_error)?.to_owned();
    let engine = engine_id.to_owned();
    let model = task::spawn_blocking(move || {
        local_engines::models::register_with_storage_action(
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
    Ok(local_engines::models::to_json(&model, available)?)
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
    let mut url = Url::parse(HUB_BASE).map_err(|_| hub_network_error())?;
    let (owner, name) = repo_id.split_once('/').ok_or_else(invalid_request_error)?;
    url.path_segments_mut()
        .map_err(|_| hub_network_error())?
        .extend(["api", "models", owner, name]);
    Ok(url)
}

fn download_url(repo_id: &str, revision: &str, file_path: &str) -> Result<Url, ServiceError> {
    if !is_commit_sha(revision) || !valid_hub_path(file_path) {
        return Err(invalid_request_error());
    }
    let (owner, name) = repo_id.split_once('/').ok_or_else(invalid_request_error)?;
    let mut url = Url::parse(HUB_BASE).map_err(|_| hub_network_error())?;
    let mut segments = url.path_segments_mut().map_err(|_| hub_network_error())?;
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

fn is_model_weight(file: &HubFile, extension: &str) -> bool {
    if !has_extension(&file.path, extension) || file.path.contains('/') {
        return false;
    }
    let filename = file_name(&file.path).to_ascii_lowercase();
    if ["cal_", "trace", "measurement", "noise", "recipe", "qbench"]
        .iter()
        .any(|marker| filename.contains(marker))
    {
        return false;
    }
    filename.starts_with("model")
        || filename.starts_with("pytorch_model")
        || filename.starts_with("consolidated")
        || filename.starts_with("weights")
}

fn is_auxiliary_model_weight(file: &HubFile) -> bool {
    model_file_kind(&file.path) != "model"
        && ["safetensors", "bin", "pt", "pth", "onnx"]
            .iter()
            .any(|extension| has_extension(&file.path, extension))
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

fn has_root_file(files: &[HubFile], name: &str) -> bool {
    files.iter().any(|file| file.path == name)
}

fn has_tokenizer_file(files: &[HubFile]) -> bool {
    [
        "tokenizer.json",
        "tokenizer.model",
        "spiece.model",
        "sentencepiece.bpe.model",
        "vocab.json",
    ]
    .iter()
    .any(|name| has_root_file(files, name))
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
