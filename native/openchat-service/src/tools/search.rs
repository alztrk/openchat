use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use ignore::WalkBuilder;
use serde_json::{Value, json};

use crate::protocol::ServiceError;

use super::{
    invalid, is_default_ignored_directory, path_entry_is_hidden, resolve_directory, unavailable,
};

const MAX_SEARCH_RESULTS: usize = 40;
const MAX_SEARCH_FILES: usize = 100_000;
const MAX_SEARCH_DIRS: usize = 100_000;
const MAX_SEARCH_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DIRECT_SEARCH_WORKERS: usize = 8;
const DIRECT_SEARCH_BATCH_FILES: usize = MAX_DIRECT_SEARCH_WORKERS * 1024;
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

pub(super) fn search_files_direct(
    root: &Path,
    directory: &Path,
    query: &str,
    include_hidden: bool,
    explicitly_scoped: bool,
    offset: usize,
    limit: usize,
) -> Result<Value, ServiceError> {
    let pattern = SearchPattern::new(query);
    let mut file_count = 0usize;
    let mut directory_count = 1usize;
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
    let hidden_filter_failed = Arc::new(AtomicBool::new(false));
    let hidden_filter_failed_for_walker = Arc::clone(&hidden_filter_failed);

    let mut walker = WalkBuilder::new(directory);
    walker
        .follow_links(false)
        .hidden(false)
        .parents(true)
        .ignore(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .sort_by_file_name(|left, right| left.cmp(right))
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            let Some(name) = entry.file_name().to_str() else {
                return true;
            };
            let is_directory = entry.file_type().is_some_and(|kind| kind.is_dir());
            if !explicitly_scoped && is_directory && is_default_ignored_directory(name) {
                return false;
            }
            if include_hidden {
                return true;
            }
            match path_entry_is_hidden(name, entry.path()) {
                Ok(is_hidden) => !is_hidden,
                Err(()) => {
                    hidden_filter_failed_for_walker.store(true, Ordering::Relaxed);
                    false
                }
            }
        });

    'walk: for result in walker.build() {
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => {
                mark_walk_problem(
                    file_order,
                    serial_file_limit,
                    &mut truncated,
                    &mut deferred_walk_errors,
                );
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        if entry.error().is_some() {
            mark_walk_problem(
                file_order,
                serial_file_limit,
                &mut truncated,
                &mut deferred_walk_errors,
            );
        }
        if entry.file_name().to_str().is_none() {
            mark_walk_problem(
                file_order,
                serial_file_limit,
                &mut truncated,
                &mut deferred_walk_errors,
            );
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            mark_walk_problem(
                file_order,
                serial_file_limit,
                &mut truncated,
                &mut deferred_walk_errors,
            );
            continue;
        };
        let path = entry.path().to_path_buf();
        if entry.path_is_symlink() {
            continue;
        }
        if file_type.is_dir() {
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
            break;
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

    if hidden_filter_failed.load(Ordering::Relaxed) {
        mark_walk_problem(
            file_order,
            serial_file_limit,
            &mut truncated,
            &mut deferred_walk_errors,
        );
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

pub(super) fn ascii_shift_table(needle: &[u8]) -> [usize; 256] {
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

pub(super) fn contains_ascii_case_insensitive(
    haystack: &[u8],
    needle: &[u8],
    shifts: &[usize; 256],
) -> bool {
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
