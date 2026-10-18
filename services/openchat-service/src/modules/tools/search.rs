//! Searches project files with bounded traversal and deterministic text matching.
//!
//! Ignore rules, hidden-path checks, file-size limits, and result limits are
//! applied during traversal so searches stay scoped to the active workspace.

use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use cap_std::{ambient_authority, fs::Dir, fs::OpenOptions};
use ignore::WalkBuilder;
use serde_json::{Value, json};

use crate::protocol::ServiceError;

use super::{
    invalid, is_default_ignored_directory, resolve_directory, unavailable, walker_entry_is_hidden,
};

const MAX_SEARCH_RESULTS: usize = 40;
const MAX_SEARCH_FILES: usize = 100_000;
const MAX_SEARCH_DIRS: usize = 100_000;
const MAX_SEARCH_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DIRECT_SEARCH_WORKERS: usize = 8;
pub(super) const DIRECT_SEARCH_INITIAL_BATCH_FILES: usize = 2_048;
const DIRECT_SEARCH_BATCH_FILES: usize = MAX_DIRECT_SEARCH_WORKERS * 1024;

/// Maintains pagination state while ordered candidate batches are processed.
struct SearchProgress {
    /// Offset requested by the caller, applied across all scanned files.
    offset: usize,
    /// Maximum number of matches returned on the current page.
    limit: usize,
    /// Number of early files read serially before parallel candidate scanning.
    serial_file_limit: usize,
    /// Number of files already consumed by the serial scan.
    serial_file_count: usize,
    /// Number of parallel candidates consumed across completed batches.
    next_candidate_index: usize,
    /// Number of matching lines encountered before the current file.
    match_index: usize,
    /// Current page plus one retained match used to detect a following page.
    results: Vec<Value>,
    /// Whether a traversal, file read, or result limit made the scan incomplete.
    truncated: bool,
    /// Whether at least one match exists after the current page.
    has_more_matches: bool,
    /// Next deferred walker error that has not yet been ordered against a file.
    error_index: usize,
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
    /// Path relative to the directory capability opened for the active workspace.
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

struct SearchDirectoryCache<'a> {
    root: &'a Dir,
    relative_directory: Option<PathBuf>,
    directory: Option<Dir>,
    read_options: OpenOptions,
}

impl<'a> SearchDirectoryCache<'a> {
    fn new(root: &'a Dir) -> Self {
        let mut read_options = OpenOptions::new();
        read_options.read(true);
        Self {
            root,
            relative_directory: None,
            directory: None,
            read_options,
        }
    }

    fn open_file(&mut self, relative_path: &Path) -> std::io::Result<cap_std::fs::File> {
        let relative_directory = relative_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new(""));
        if self.relative_directory.as_deref() != Some(relative_directory) {
            let directory = if relative_directory.as_os_str().is_empty() {
                self.root.try_clone()?
            } else {
                self.root.open_dir(relative_directory)?
            };
            self.relative_directory = Some(relative_directory.to_path_buf());
            self.directory = Some(directory);
        }

        let Some(file_name) = relative_path.file_name() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "search candidate has no file name",
            ));
        };
        let Some(directory) = self.directory.as_ref() else {
            return Err(std::io::Error::other(
                "search candidate directory is unavailable",
            ));
        };
        crate::filesystem::open_file_from_directory(
            directory,
            Path::new(file_name),
            &self.read_options,
        )
    }
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

/// Searches a resolved workspace directory without following symbolic links.
///
/// The serial prefix favors quick first-page results; later files are scanned
/// in bounded batches and merged in filename order.
///
/// # Returns
/// A JSON response containing the requested page, continuation offset, and
/// truncation state.
///
/// # Errors
/// Returns a service error if the workspace root cannot be opened or a worker
/// thread fails.
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
    let root_directory = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|_| unavailable("The active tool root could not be opened safely."))?;
    let mut serial_directory_cache = SearchDirectoryCache::new(&root_directory);
    let mut file_count = 0usize;
    let mut directory_count = 1usize;
    let mut bytes_read = 0u64;
    let mut queued_bytes = 0u64;
    let mut file_order = 0usize;
    let serial_file_limit = limit.saturating_add(1).min(MAX_DIRECT_SEARCH_WORKERS * 8);
    let mut progress = SearchProgress {
        offset,
        limit,
        serial_file_limit,
        serial_file_count: 0,
        next_candidate_index: 0,
        match_index: 0,
        results: Vec::with_capacity(limit.saturating_add(1)),
        truncated: false,
        has_more_matches: false,
        error_index: 0,
    };
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
            match walker_entry_is_hidden(name, entry) {
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
                    progress.serial_file_limit,
                    &mut progress.truncated,
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
                progress.serial_file_limit,
                &mut progress.truncated,
                &mut deferred_walk_errors,
            );
        }
        if entry.file_name().to_str().is_none() {
            mark_walk_problem(
                file_order,
                progress.serial_file_limit,
                &mut progress.truncated,
                &mut deferred_walk_errors,
            );
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            mark_walk_problem(
                file_order,
                progress.serial_file_limit,
                &mut progress.truncated,
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
                    progress.serial_file_limit,
                    &mut progress.truncated,
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
                progress.serial_file_limit,
                &mut progress.truncated,
                &mut deferred_walk_errors,
            );
            continue;
        };
        file_count += 1;
        if file_count > MAX_SEARCH_FILES {
            mark_walk_problem(
                file_order,
                progress.serial_file_limit,
                &mut progress.truncated,
                &mut deferred_walk_errors,
            );
            break;
        }

        file_order += 1;
        let remaining_bytes = MAX_SEARCH_BYTES.saturating_sub(bytes_read + queued_bytes);
        let bytes_to_read = metadata.len().min(remaining_bytes);
        let byte_limited = metadata.len() > remaining_bytes;
        let relative_path = match path.strip_prefix(root) {
            Ok(path) => path.to_path_buf(),
            Err(_) => {
                mark_walk_problem(
                    file_order,
                    progress.serial_file_limit,
                    &mut progress.truncated,
                    &mut deferred_walk_errors,
                );
                continue;
            }
        };
        let candidate = SearchCandidate {
            path: relative_path,
            bytes_to_read,
            byte_limited,
        };

        if progress.serial_file_count < progress.serial_file_limit {
            let window_limit = progress
                .limit
                .saturating_add(1)
                .saturating_sub(progress.results.len());
            let file_result = scan_search_file(
                &mut serial_directory_cache,
                &candidate,
                &pattern,
                progress.offset.saturating_sub(progress.match_index),
                window_limit,
                true,
                &mut line,
            );
            bytes_read = bytes_read.saturating_add(file_result.bytes_read);
            progress.truncated |= file_result.truncated;
            progress.match_index = progress.match_index.saturating_add(file_result.match_count);
            append_search_lines(
                root,
                &candidate.path,
                &file_result.matches,
                &mut progress.results,
            );
            progress.serial_file_count += 1;

            if progress.results.len() > progress.limit {
                progress.has_more_matches = true;
                progress.truncated = true;
                break 'walk;
            }
            if byte_limited {
                progress.truncated = true;
                break 'walk;
            }
        } else {
            queued_bytes = queued_bytes.saturating_add(bytes_to_read);
            candidates.push(candidate);
            let batch_file_limit = if progress.next_candidate_index == 0 {
                DIRECT_SEARCH_INITIAL_BATCH_FILES
            } else {
                DIRECT_SEARCH_BATCH_FILES
            };
            if candidates.len() >= batch_file_limit {
                let reserved_bytes = process_search_candidate_batch(
                    root,
                    &root_directory,
                    &mut serial_directory_cache,
                    &pattern,
                    &candidates,
                    &deferred_walk_errors,
                    &mut progress,
                )?;
                bytes_read = bytes_read.saturating_add(reserved_bytes);
                queued_bytes = queued_bytes.saturating_sub(reserved_bytes);
                candidates.clear();
                if progress.has_more_matches {
                    break 'walk;
                }
            }
            if byte_limited {
                break 'walk;
            }
        }
    }

    if hidden_filter_failed.load(Ordering::Relaxed) {
        mark_walk_problem(
            file_order,
            progress.serial_file_limit,
            &mut progress.truncated,
            &mut deferred_walk_errors,
        );
    }

    if !progress.has_more_matches && !candidates.is_empty() {
        process_search_candidate_batch(
            root,
            &root_directory,
            &mut serial_directory_cache,
            &pattern,
            &candidates,
            &deferred_walk_errors,
            &mut progress,
        )?;
    }
    if progress.error_index < deferred_walk_errors.len() {
        progress.truncated = true;
    }

    let next_offset = progress
        .has_more_matches
        .then_some(offset.saturating_add(limit));
    Ok(json!({
        "matches": progress.results.into_iter().take(limit).collect::<Vec<_>>(),
        "nextOffset": next_offset,
        "truncated": progress.truncated,
    }))
}

/// Scans and merges one bounded batch while preserving traversal order.
///
/// Advances `progress` and returns the candidate bytes reserved against the
/// search-wide read limit. Worker failures are returned to the caller.
fn process_search_candidate_batch(
    root: &Path,
    root_directory: &Dir,
    serial_directory_cache: &mut SearchDirectoryCache<'_>,
    pattern: &SearchPattern,
    candidates: &[SearchCandidate],
    deferred_walk_errors: &[usize],
    progress: &mut SearchProgress,
) -> Result<u64, ServiceError> {
    let batch_results = scan_search_candidates(
        root_directory,
        candidates,
        pattern,
        progress.limit.saturating_add(1),
    )?;
    let candidate_sequence_start = progress
        .serial_file_count
        .saturating_add(progress.next_candidate_index);
    progress.next_candidate_index = progress
        .next_candidate_index
        .saturating_add(candidates.len());
    let reserved_bytes = candidates.iter().fold(0u64, |total, candidate| {
        total.saturating_add(candidate.bytes_to_read)
    });
    let mut line = String::new();

    for (candidate_index, file_result) in batch_results {
        let sequence = candidate_sequence_start.saturating_add(candidate_index);
        while deferred_walk_errors
            .get(progress.error_index)
            .is_some_and(|error_at| *error_at <= sequence)
        {
            progress.truncated = true;
            progress.error_index += 1;
        }

        let local_offset = progress.offset.saturating_sub(progress.match_index);
        let remaining_matches = progress
            .limit
            .saturating_add(1)
            .saturating_sub(progress.results.len());
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
                        &mut progress.results,
                    );
                }
            } else {
                let window = scan_search_file(
                    serial_directory_cache,
                    &candidates[candidate_index],
                    pattern,
                    local_offset,
                    remaining_matches,
                    true,
                    &mut line,
                );
                append_search_lines(
                    root,
                    &candidates[candidate_index].path,
                    &window.matches,
                    &mut progress.results,
                );
                progress.truncated |= window.truncated;
            }
        }
        progress.match_index = progress.match_index.saturating_add(file_result.match_count);
        progress.truncated |= file_result.truncated;
        if progress.results.len() > progress.limit {
            progress.has_more_matches = true;
            progress.truncated = true;
            break;
        }
    }

    Ok(reserved_bytes)
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
    directory_cache: &mut SearchDirectoryCache<'_>,
    candidate: &SearchCandidate,
    pattern: &SearchPattern,
    skip_matches: usize,
    max_matches: usize,
    stop_when_full: bool,
    line: &mut String,
) -> SearchFileResult {
    let Ok(file) = directory_cache.open_file(&candidate.path) else {
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
    let mut matches = Vec::new();
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
            if matches.is_empty() {
                matches.reserve_exact(max_matches.min(MAX_SEARCH_RESULTS + 1));
            }
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
    root_directory: &Dir,
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
                let mut directory_cache = SearchDirectoryCache::new(root_directory);
                loop {
                    let candidate_index = next_candidate.fetch_add(1, Ordering::Relaxed);
                    let Some(candidate) = candidates.get(candidate_index) else {
                        break;
                    };
                    let result = scan_search_file(
                        &mut directory_cache,
                        candidate,
                        pattern,
                        0,
                        retained_matches,
                        false,
                        &mut line,
                    );
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

#[cfg(all(test, windows))]
mod tests {
    use std::{fs, os::windows::fs::symlink_file, path::PathBuf};

    use cap_std::{ambient_authority, fs::Dir};

    use super::{SearchCandidate, SearchDirectoryCache, SearchPattern, scan_search_file};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-search-root-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated search directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn candidate_swapped_to_outside_symlink_is_not_read() {
        let root = TestDirectory::new();
        let candidate_path = root.0.join("candidate.txt");
        fs::write(&candidate_path, "safe marker\n").expect("write initial candidate");
        let root_directory =
            Dir::open_ambient_dir(&root.0, ambient_authority()).expect("open search capability");
        let outside_directory = TestDirectory::new();
        let outside = outside_directory.0.join("private.txt");
        fs::write(&outside, "private marker\n").expect("write outside fixture");
        fs::remove_file(&candidate_path).expect("remove enumerated candidate");
        symlink_file(&outside, &candidate_path).expect("replace candidate with outside symlink");
        let candidate = SearchCandidate {
            path: PathBuf::from("candidate.txt"),
            bytes_to_read: 64,
            byte_limited: false,
        };
        let mut directory_cache = SearchDirectoryCache::new(&root_directory);

        let result = scan_search_file(
            &mut directory_cache,
            &candidate,
            &SearchPattern::new("private"),
            0,
            4,
            false,
            &mut String::new(),
        );

        assert_eq!(result.match_count, 0);
        assert!(result.matches.is_empty());
        assert!(result.truncated);
    }
}
