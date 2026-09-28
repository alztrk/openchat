use std::{
    collections::BinaryHeap,
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use serde_json::{Value, json};

use crate::{protocol::ServiceError, provider_schema::ToolDefinition};

const MAX_LIST_RESULTS: usize = 100;
const MAX_LIST_OFFSET: usize = 100_000;
const MAX_LIST_SCAN: usize = 100_000;
const MAX_SEARCH_RESULTS: usize = 40;
const MAX_SEARCH_FILES: usize = 100_000;
const MAX_SEARCH_DIRS: usize = 100_000;
const MAX_SEARCH_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DIRECT_SEARCH_WORKERS: usize = 8;
const DIRECT_SEARCH_BATCH_FILES: usize = MAX_DIRECT_SEARCH_WORKERS * 1024;
const MAX_READ_LINES: usize = 500;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
pub(crate) const MAX_TOOL_ARGUMENT_BYTES: usize = 16 * 1024;
pub(crate) const MAX_TOOL_ROUNDS: usize = 6;
pub(crate) const MAX_TOOL_CALLS_PER_TURN: usize = 16;

struct DirectoryListing {
    entries: Vec<(String, bool)>,
    total_entries: usize,
    truncated: bool,
}

struct SearchPattern {
    lowercase_query: String,
    ascii_needle: String,
    ascii_shifts: Option<[usize; 256]>,
}

impl SearchPattern {
    fn new(query: &str) -> Self {
        let ascii_query = query.is_ascii();
        let ascii_needle = query.to_ascii_lowercase();
        Self {
            lowercase_query: query.to_lowercase(),
            ascii_shifts: ascii_query.then(|| ascii_shift_table(ascii_needle.as_bytes())),
            ascii_needle,
        }
    }

    fn matches(&self, line: &str) -> bool {
        match self.ascii_shifts.as_ref() {
            Some(shifts) => {
                contains_ascii_case_insensitive(
                    line.as_bytes(),
                    self.ascii_needle.as_bytes(),
                    shifts,
                ) || (!line.is_ascii() && line.to_lowercase().contains(&self.ascii_needle))
            }
            None => line.to_lowercase().contains(&self.lowercase_query),
        }
    }
}

struct SearchCandidate {
    path: PathBuf,
    bytes_to_read: u64,
    byte_limited: bool,
}

struct SearchLineMatch {
    number: usize,
    text: String,
}

struct SearchFileResult {
    match_count: usize,
    bytes_read: u64,
    matches: Vec<SearchLineMatch>,
    truncated: bool,
}
pub fn list_files(
    root: impl AsRef<Path>,
    path: &str,
    offset: usize,
    limit: usize,
) -> Result<Value, ServiceError> {
    let (root, directory, explicit_filter_scope) = resolve_directory(root, path)?;
    let limit = limit.clamp(1, MAX_LIST_RESULTS);
    if offset > MAX_LIST_OFFSET {
        return Err(invalid("The directory offset exceeds the supported range."));
    }

    let relative = directory.strip_prefix(&root).unwrap_or(Path::new(""));
    let retain_count = offset.saturating_add(limit).min(MAX_LIST_SCAN);
    let listing = read_directory_listing(&directory, explicit_filter_scope, retain_count)?;
    Ok(directory_listing_response(
        relative, &listing, offset, limit,
    ))
}

fn read_directory_listing(
    directory: &Path,
    explicit_filter_scope: bool,
    retain_count: usize,
) -> Result<DirectoryListing, ServiceError> {
    let mut retained = BinaryHeap::new();
    let mut total_entries = 0usize;
    let mut scanned = 0usize;
    let mut incomplete = false;
    for entry in fs::read_dir(&directory)
        .map_err(|_| unavailable("The requested directory could not be read."))?
    {
        let Ok(entry) = entry else {
            incomplete = true;
            continue;
        };
        scanned += 1;
        if scanned > MAX_LIST_SCAN {
            break;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            incomplete = true;
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            incomplete = true;
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if !explicit_filter_scope {
            if file_type.is_dir() && is_default_ignored_directory(name) {
                continue;
            }
            match entry_is_hidden(name, &entry) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(()) => {
                    incomplete = true;
                    continue;
                }
            }
        }
        total_entries += 1;
        let is_directory = file_type.is_dir();
        if retained.len() < retain_count {
            retained.push((name.to_owned(), is_directory));
        } else if retained
            .peek()
            .is_some_and(|largest: &(String, bool)| name < largest.0.as_str())
        {
            retained.pop();
            retained.push((name.to_owned(), is_directory));
        }
    }
    let was_scan_limited = scanned > MAX_LIST_SCAN || incomplete;
    let mut entries = retained.into_vec();
    entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    Ok(DirectoryListing {
        entries,
        total_entries,
        truncated: was_scan_limited,
    })
}

fn directory_listing_response(
    relative_directory: &Path,
    listing: &DirectoryListing,
    offset: usize,
    limit: usize,
) -> Value {
    let page = listing
        .entries
        .iter()
        .skip(offset)
        .take(limit)
        .map(|(name, is_dir)| {
            let path = relative_directory
                .join(name)
                .to_string_lossy()
                .replace('\\', "/");
            json!({"path": path, "type": if *is_dir { "directory" } else { "file" }})
        })
        .collect::<Vec<_>>();
    let next_offset = offset.saturating_add(page.len());
    let has_more = listing.total_entries > next_offset;

    json!({
        "entries": page,
        "nextOffset": has_more.then_some(next_offset),
        "truncated": listing.truncated,
    })
}

pub fn search_files(
    root: impl AsRef<Path>,
    path: &str,
    query: &str,
    include_hidden: bool,
    offset: usize,
    limit: usize,
) -> Result<Value, ServiceError> {
    if query.is_empty() || query.len() > 4096 || query.chars().any(char::is_control) {
        return Err(invalid("The search text is empty or invalid."));
    }
    let (root, directory, explicitly_scoped) = resolve_directory(root, path)?;
    if offset > 1_000_000 {
        return Err(invalid("The search offset exceeds the supported range."));
    }
    let include_hidden = include_hidden || explicitly_scoped;
    let limit = limit.clamp(1, MAX_SEARCH_RESULTS);

    search_files_direct(
        &root,
        &directory,
        query,
        include_hidden,
        explicitly_scoped,
        offset,
        limit,
    )
}

fn search_files_direct(
    root: &Path,
    directory: &Path,
    query: &str,
    include_hidden: bool,
    explicitly_scoped: bool,
    offset: usize,
    limit: usize,
) -> Result<Value, ServiceError> {
    let pattern = SearchPattern::new(query);
    let mut pending = vec![directory.to_path_buf()];
    let mut file_count = 0usize;
    let mut directory_count = 0usize;
    let mut bytes_read = 0u64;
    let mut queued_bytes = 0u64;
    let mut file_order = 0usize;
    let mut serial_file_count = 0usize;
    let serial_file_limit = limit.saturating_add(1).min(MAX_DIRECT_SEARCH_WORKERS * 8);
    let mut match_index = 0usize;
    let mut results = Vec::with_capacity(limit.saturating_add(1));
    let mut truncated = false;
    let mut has_more_matches = false;
    let mut candidates = Vec::new();
    let mut deferred_walk_errors = Vec::new();
    let mut line = String::new();

    'walk: while let Some(directory) = pending.pop() {
        directory_count += 1;
        if directory_count > MAX_SEARCH_DIRS {
            mark_walk_problem(
                file_order,
                serial_file_limit,
                &mut truncated,
                &mut deferred_walk_errors,
            );
            break;
        }
        let Ok(entries) = fs::read_dir(&directory) else {
            mark_walk_problem(
                file_order,
                serial_file_limit,
                &mut truncated,
                &mut deferred_walk_errors,
            );
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                mark_walk_problem(
                    file_order,
                    serial_file_limit,
                    &mut truncated,
                    &mut deferred_walk_errors,
                );
                continue;
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                mark_walk_problem(
                    file_order,
                    serial_file_limit,
                    &mut truncated,
                    &mut deferred_walk_errors,
                );
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                mark_walk_problem(
                    file_order,
                    serial_file_limit,
                    &mut truncated,
                    &mut deferred_walk_errors,
                );
                continue;
            };
            let path = entry.path();
            if file_type.is_symlink() {
                continue;
            }
            if !include_hidden {
                match entry_is_hidden(name, &entry) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(()) => {
                        mark_walk_problem(
                            file_order,
                            serial_file_limit,
                            &mut truncated,
                            &mut deferred_walk_errors,
                        );
                        continue;
                    }
                }
            }
            if file_type.is_dir() {
                if !explicitly_scoped && is_default_ignored_directory(name) {
                    continue;
                }
                if directory_count.saturating_add(pending.len()) >= MAX_SEARCH_DIRS {
                    mark_walk_problem(
                        file_order,
                        serial_file_limit,
                        &mut truncated,
                        &mut deferred_walk_errors,
                    );
                    break 'walk;
                }
                pending.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                mark_walk_problem(
                    file_order,
                    serial_file_limit,
                    &mut truncated,
                    &mut deferred_walk_errors,
                );
                continue;
            };
            file_count += 1;
            if file_count > MAX_SEARCH_FILES {
                mark_walk_problem(
                    file_order,
                    serial_file_limit,
                    &mut truncated,
                    &mut deferred_walk_errors,
                );
                break 'walk;
            }

            file_order += 1;
            let remaining_bytes = MAX_SEARCH_BYTES.saturating_sub(bytes_read + queued_bytes);
            let bytes_to_read = metadata.len().min(remaining_bytes);
            let byte_limited = metadata.len() > remaining_bytes;
            let candidate = SearchCandidate {
                path,
                bytes_to_read,
                byte_limited,
            };

            if serial_file_count < serial_file_limit {
                let window_limit = limit.saturating_add(1).saturating_sub(results.len());
                let file_result = scan_search_file(
                    &candidate,
                    &pattern,
                    offset.saturating_sub(match_index),
                    window_limit,
                    true,
                    &mut line,
                );
                bytes_read = bytes_read.saturating_add(file_result.bytes_read);
                truncated |= file_result.truncated;
                match_index = match_index.saturating_add(file_result.match_count);
                append_search_lines(root, &candidate.path, &file_result.matches, &mut results);
                serial_file_count += 1;

                if results.len() > limit {
                    has_more_matches = true;
                    truncated = true;
                    break 'walk;
                }
                if byte_limited {
                    truncated = true;
                    break 'walk;
                }
            } else {
                queued_bytes = queued_bytes.saturating_add(bytes_to_read);
                candidates.push(candidate);
                if byte_limited {
                    break 'walk;
                }
            }
        }
    }

    let mut error_index = 0usize;
    let mut candidate_start = 0usize;
    'candidate_batches: while candidate_start < candidates.len() {
        let candidate_end = candidate_start
            .saturating_add(DIRECT_SEARCH_BATCH_FILES)
            .min(candidates.len());
        let batch_results = scan_search_candidates(
            &candidates[candidate_start..candidate_end],
            &pattern,
            limit.saturating_add(1),
        )?;
        for (batch_index, file_result) in batch_results {
            let candidate_index = candidate_start + batch_index;
            let sequence = serial_file_count + candidate_index;
            while deferred_walk_errors
                .get(error_index)
                .is_some_and(|error_at| *error_at <= sequence)
            {
                truncated = true;
                error_index += 1;
            }

            let local_offset = offset.saturating_sub(match_index);
            let remaining_matches = limit.saturating_add(1).saturating_sub(results.len());
            let available_matches = file_result.match_count.saturating_sub(local_offset);
            if available_matches > 0 && remaining_matches > 0 {
                let stored_start = local_offset.min(file_result.matches.len());
                let stored_end = stored_start.saturating_add(remaining_matches);
                let stored = file_result
                    .matches
                    .get(stored_start..stored_end.min(file_result.matches.len()));
                let can_use_stored = stored
                    .is_some_and(|stored| stored.len() >= available_matches.min(remaining_matches));
                if can_use_stored {
                    if let Some(stored) = stored {
                        append_search_lines(
                            root,
                            &candidates[candidate_index].path,
                            stored,
                            &mut results,
                        );
                    }
                } else {
                    let window = scan_search_file(
                        &candidates[candidate_index],
                        &pattern,
                        local_offset,
                        remaining_matches,
                        true,
                        &mut line,
                    );
                    append_search_lines(
                        root,
                        &candidates[candidate_index].path,
                        &window.matches,
                        &mut results,
                    );
                    truncated |= window.truncated;
                }
            }
            match_index = match_index.saturating_add(file_result.match_count);
            if file_result.truncated {
                truncated = true;
            }
            if results.len() > limit {
                has_more_matches = true;
                truncated = true;
                break 'candidate_batches;
            }
        }
        candidate_start = candidate_end;
    }
    if error_index < deferred_walk_errors.len() {
        truncated = true;
    }

    let next_offset = has_more_matches.then_some(offset.saturating_add(limit));
    Ok(json!({
        "matches": results.into_iter().take(limit).collect::<Vec<_>>(),
        "nextOffset": next_offset,
        "truncated": truncated,
    }))
}

fn mark_walk_problem(
    sequence: usize,
    serial_file_limit: usize,
    truncated: &mut bool,
    deferred_errors: &mut Vec<usize>,
) {
    if sequence < serial_file_limit {
        *truncated = true;
    } else {
        deferred_errors.push(sequence);
    }
}

fn append_search_lines(
    root: &Path,
    path: &Path,
    lines: &[SearchLineMatch],
    output: &mut Vec<Value>,
) {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let relative = relative.to_string_lossy().replace('\\', "/");
    output.extend(lines.iter().map(|line| {
        json!({
            "path": relative.as_str(),
            "line": line.number,
            "text": line.text,
        })
    }));
}

fn scan_search_file(
    candidate: &SearchCandidate,
    pattern: &SearchPattern,
    skip_matches: usize,
    max_matches: usize,
    stop_when_full: bool,
    line: &mut String,
) -> SearchFileResult {
    let Ok(file) = File::open(&candidate.path) else {
        return SearchFileResult {
            match_count: 0,
            bytes_read: 0,
            matches: Vec::new(),
            truncated: true,
        };
    };
    let mut reader = BufReader::new(file.take(candidate.bytes_to_read));
    let mut match_count = 0usize;
    let mut bytes_read = 0u64;
    let mut line_number = 0usize;
    let mut matches = Vec::with_capacity(max_matches.min(MAX_SEARCH_RESULTS + 1));
    let mut truncated = candidate.byte_limited;

    loop {
        line.clear();
        let read = match reader.read_line(line) {
            Ok(read) => read,
            Err(_) => {
                truncated = true;
                break;
            }
        };
        if read == 0 {
            break;
        }
        let line_bytes = read as u64;
        let next_bytes_read = bytes_read.saturating_add(line_bytes);
        if candidate.byte_limited
            && next_bytes_read >= candidate.bytes_to_read
            && !line.ends_with('\n')
        {
            bytes_read = next_bytes_read;
            break;
        }
        bytes_read = next_bytes_read;
        line_number += 1;
        if !pattern.matches(line) {
            continue;
        }

        let matched_line_index = match_count;
        match_count = match_count.saturating_add(1);
        if matched_line_index < skip_matches {
            continue;
        }
        if matches.len() < max_matches {
            matches.push(SearchLineMatch {
                number: line_number,
                text: compact_line(line, 160),
            });
        }
        if stop_when_full && matches.len() >= max_matches {
            break;
        }
    }

    SearchFileResult {
        match_count,
        bytes_read,
        matches,
        truncated,
    }
}

fn scan_search_candidates(
    candidates: &[SearchCandidate],
    pattern: &SearchPattern,
    retained_matches: usize,
) -> Result<Vec<(usize, SearchFileResult)>, ServiceError> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = std::thread::available_parallelism()
        .map_or(2, |count| count.get().max(2))
        .min(MAX_DIRECT_SEARCH_WORKERS)
        .min(candidates.len());
    let next_candidate = AtomicUsize::new(0);

    let mut results = std::thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            workers.push(scope.spawn(|| {
                let mut results = Vec::new();
                let mut line = String::new();
                loop {
                    let candidate_index = next_candidate.fetch_add(1, Ordering::Relaxed);
                    let Some(candidate) = candidates.get(candidate_index) else {
                        break;
                    };
                    let result =
                        scan_search_file(candidate, pattern, 0, retained_matches, false, &mut line);
                    results.push((candidate_index, result));
                }
                results
            }));
        }

        let mut results = Vec::with_capacity(candidates.len());
        for worker in workers {
            let mut worker_results = worker
                .join()
                .map_err(|_| unavailable("The workspace search could not be completed."))?;
            results.append(&mut worker_results);
        }
        Ok::<_, ServiceError>(results)
    })?;
    results.sort_unstable_by_key(|(candidate_index, _)| *candidate_index);
    Ok(results)
}

pub fn read_file(
    root: impl AsRef<Path>,
    path: &str,
    start_line: usize,
    line_count: usize,
) -> Result<Value, ServiceError> {
    let (root, file_path, metadata) = resolve_file(root, path)?;
    if metadata.len() > MAX_OUTPUT_BYTES as u64 * 1024 {
        return Err(invalid(
            "The file exceeds the direct-read limit. Narrow the requested content or search for a specific term.",
        ));
    }
    let file =
        File::open(&file_path).map_err(|_| unavailable("The requested file could not be read."))?;
    let start_line = start_line.max(1);
    let line_count = line_count.clamp(1, MAX_READ_LINES);
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut number = 0usize;
    let mut output = Vec::new();
    let mut output_bytes = 0usize;
    let mut truncated = false;

    while number.saturating_add(1) < start_line {
        let skipped = reader
            .skip_until(b'\n')
            .map_err(|_| unavailable("The requested file could not be read."))?;
        if skipped == 0 {
            break;
        }
        number += 1;
    }

    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|_| unavailable("The requested file could not be read."))?;
        if read == 0 {
            break;
        }
        number += 1;
        if number < start_line {
            continue;
        }
        if output.len() >= line_count || output_bytes.saturating_add(read) > MAX_OUTPUT_BYTES {
            truncated = true;
            break;
        }
        output_bytes += read;
        output.push(json!({"line": number, "text": line.trim_end_matches(['\r', '\n'])}));
    }
    let next_line = truncated.then_some(number);
    let relative = file_path.strip_prefix(&root).unwrap_or(&file_path);
    Ok(json!({
        "path": relative.to_string_lossy().replace('\\', "/"),
        "lines": output,
        "nextLine": next_line,
        "truncated": truncated,
    }))
}

pub fn get_file_info(root: impl AsRef<Path>, path: &str) -> Result<Value, ServiceError> {
    let (root, file_path, metadata) = resolve_file(root, path)?;
    let relative = file_path.strip_prefix(&root).unwrap_or(&file_path);
    Ok(json!({
        "path": relative.to_string_lossy().replace('\\', "/"),
        "type": if metadata.is_dir() { "directory" } else { "file" },
        "size": metadata.len(),
    }))
}

pub fn definitions() -> Vec<ToolDefinition> {
    [
        (
            "list_files",
            "List files and folders. In approval mode use a relative path, `project:/...`, or `openchat:/...`; in full-access mode use `desktop:/...` for the user's Desktop or an absolute path for other folders.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Approval mode: relative to the project root, or prefix with project:/ or openchat:/. Full-access mode: desktop:/ for the user's Desktop, otherwise an absolute directory path."},
                    "offset": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100}
                },
                "required": ["path", "offset", "limit"],
                "additionalProperties": false
            }),
        ),
        (
            "search_files",
            "Search readable files. In approval mode use a relative path, `project:/...`, or `openchat:/...`; in full-access mode use `desktop:/...` for the user's Desktop or an absolute path for other folders.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Approval mode: relative to the project root, or prefix with project:/ or openchat:/. Full-access mode: desktop:/ for the user's Desktop, otherwise an absolute directory path."},
                    "query": {"type": "string"},
                    "includeHidden": {"type": "boolean"},
                    "offset": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 40}
                },
                "required": ["path", "query", "includeHidden", "offset", "limit"],
                "additionalProperties": false
            }),
        ),
        (
            "read_file",
            "Read a bounded range of lines. In approval mode use a relative path, `project:/...`, or `openchat:/...`; in full-access mode use `desktop:/...` for the user's Desktop or an absolute path for other folders.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Approval mode: relative to the project root, or prefix with project:/ or openchat:/. Full-access mode: desktop:/ for the user's Desktop, otherwise an absolute file path."},
                    "startLine": {"type": "integer", "minimum": 1},
                    "lineCount": {"type": "integer", "minimum": 1, "maximum": 500}
                },
                "required": ["path", "startLine", "lineCount"],
                "additionalProperties": false
            }),
        ),
        (
            "get_file_info",
            "Return the size and type of a file. In approval mode use a relative path, `project:/...`, or `openchat:/...`; in full-access mode use `desktop:/...` for the user's Desktop or an absolute path for other folders.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Approval mode: relative to the project root, or prefix with project:/ or openchat:/. Full-access mode: desktop:/ for the user's Desktop, otherwise an absolute file path."}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        ),
    ]
    .into_iter()
    .map(|(name, description, parameters)| ToolDefinition {
        name,
        description,
        parameters,
    })
    .collect()
}

mod executor;
pub(crate) use executor::tool_call_limit_error;
pub use executor::{ToolExecutor, ToolPermissionMode};

fn resolve_directory(
    root: impl AsRef<Path>,
    path: &str,
) -> Result<(PathBuf, PathBuf, bool), ServiceError> {
    let root = canonical_root(root)?;
    let requested = safe_relative_path(path)?;
    let directory = root
        .join(&requested)
        .canonicalize()
        .map_err(|_| unavailable("The requested directory is unavailable."))?;
    if !directory.starts_with(&root) || !directory.is_dir() {
        return Err(invalid(
            "The requested path is outside the active tool root or is not a directory.",
        ));
    }
    let mut relative = PathBuf::new();
    let explicit_hidden = requested.components().any(|component| {
        relative.push(component.as_os_str());
        let Some(name) = component.as_os_str().to_str() else {
            return false;
        };
        is_default_ignored_directory(name)
            || fs::metadata(root.join(&relative)).is_ok_and(|metadata| is_hidden(name, &metadata))
    });
    Ok((root, directory, explicit_hidden))
}

fn resolve_file(
    root: impl AsRef<Path>,
    path: &str,
) -> Result<(PathBuf, PathBuf, fs::Metadata), ServiceError> {
    let root = canonical_root(root)?;
    let requested = safe_relative_path(path)?;
    let file_path = root
        .join(requested)
        .canonicalize()
        .map_err(|_| unavailable("The requested file is unavailable."))?;
    if !file_path.starts_with(&root) {
        return Err(invalid(
            "The requested path is outside the active tool root or is not a file.",
        ));
    }
    let metadata = fs::metadata(&file_path)
        .map_err(|_| unavailable("The requested file information could not be read."))?;
    if !metadata.is_file() {
        return Err(invalid(
            "The requested path is outside the active tool root or is not a file.",
        ));
    }
    Ok((root, file_path, metadata))
}

fn canonical_root(root: impl AsRef<Path>) -> Result<PathBuf, ServiceError> {
    let root = root
        .as_ref()
        .canonicalize()
        .map_err(|_| invalid("The active tool root is unavailable."))?;
    if !root.is_dir() {
        return Err(invalid("The active tool root is not a folder."));
    }
    Ok(root)
}

fn safe_relative_path(path: &str) -> Result<PathBuf, ServiceError> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(
            "Tool paths must be relative to the active tool root.",
        ));
    }
    Ok(path.to_path_buf())
}

fn is_hidden(name: &str, metadata: &fs::Metadata) -> bool {
    name.starts_with('.') || windows_hidden(metadata)
}

fn entry_is_hidden(name: &str, entry: &fs::DirEntry) -> Result<bool, ()> {
    if name.starts_with('.') {
        return Ok(true);
    }
    #[cfg(windows)]
    {
        let metadata = entry.metadata().map_err(|_| ())?;
        Ok(windows_hidden(&metadata))
    }
    #[cfg(not(windows))]
    {
        let _ = entry;
        Ok(false)
    }
}

fn is_default_ignored_directory(name: &str) -> bool {
    [
        ".git",
        ".dart_tool",
        ".gradle",
        ".next",
        ".nuxt",
        "build",
        "coverage",
        "DerivedData",
        "dist",
        "node_modules",
        "Pods",
        "target",
        "vendor",
    ]
    .iter()
    .any(|ignored| name.eq_ignore_ascii_case(ignored))
}

#[cfg(windows)]
fn windows_hidden(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn windows_hidden(_metadata: &fs::Metadata) -> bool {
    false
}

fn compact_line(line: &str, max_bytes: usize) -> String {
    let mut result = String::new();
    for character in line.trim().chars() {
        if result.len() + character.len_utf8() > max_bytes {
            result.push('…');
            break;
        }
        result.push(character);
    }
    result
}

fn ascii_shift_table(needle: &[u8]) -> [usize; 256] {
    let mut shifts = [needle.len(); 256];
    for (index, byte) in needle
        .iter()
        .enumerate()
        .take(needle.len().saturating_sub(1))
    {
        shifts[byte.to_ascii_lowercase() as usize] = needle.len() - index - 1;
    }
    shifts
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8], shifts: &[usize; 256]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    let last_start = haystack.len() - needle.len();
    let last_index = needle.len() - 1;
    let mut start = 0usize;
    while start <= last_start {
        let mut index = needle.len();
        while index > 0 && haystack[start + index - 1].eq_ignore_ascii_case(&needle[index - 1]) {
            index -= 1;
        }
        if index == 0 {
            return true;
        }
        let tail = haystack[start + last_index].to_ascii_lowercase() as usize;
        start += shifts[tail].max(1);
    }
    false
}

fn invalid(message: &str) -> ServiceError {
    ServiceError::new("invalid_tool_input", message, false)
}

fn unavailable(message: &str) -> ServiceError {
    ServiceError::new("tool_path_unavailable", message, false)
}

#[cfg(test)]
mod tests;
