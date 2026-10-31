//! Directory listing with bounded gitignore handling.

use super::*;

pub(super) struct DirectoryListing {
    entries: Vec<(String, bool)>,
    total_entries: usize,
    truncated: bool,
}

/// Lists a bounded, lexicographically ordered page of direct directory entries.
///
/// Pagination is stable by entry name. The scan stops after 100,000 entries and
/// marks the response truncated when the scan or a visibility filter is incomplete.
/// Symbolic links and entries that are neither regular files nor directories are omitted.
///
/// # Errors
///
/// Returns an error when the requested directory escapes the allowed root, cannot be
/// resolved, or the requested offset exceeds the supported range.
pub fn list_files(
    root: impl AsRef<Path>,
    path: &str,
    offset: usize,
    limit: usize,
) -> Result<Value, ServiceError> {
    let (root, directory, root_handle, directory_handle, explicit_filter_scope) =
        resolve_directory_for_listing(root, path)?;
    let limit = limit.clamp(1, MAX_LIST_RESULTS);
    if offset > MAX_LIST_OFFSET {
        return Err(invalid("The directory offset exceeds the supported range."));
    }

    let relative = directory.strip_prefix(&root).unwrap_or(Path::new(""));
    let retain_count = offset.saturating_add(limit).min(MAX_LIST_SCAN);
    let ignore_matchers =
        load_gitignore_matchers(&root, &directory, &root_handle, &directory_handle)?;
    let listing = read_directory_listing_from_capability(
        &directory_handle,
        &directory,
        &ignore_matchers,
        explicit_filter_scope,
        retain_count,
    )?;
    verify_directory_handles(&root, &directory, &root_handle, &directory_handle)?;
    Ok(directory_listing_response(
        relative, &listing, offset, limit,
    ))
}

pub(super) fn read_directory_listing_from_capability(
    directory: &Dir,
    directory_path: &Path,
    ignore_matchers: &[Gitignore],
    explicit_filter_scope: bool,
    retain_count: usize,
) -> Result<DirectoryListing, ServiceError> {
    let entries = directory
        .entries()
        .map_err(|error| filesystem_error(&error, "opening the requested directory"))?;
    let mut retained = BinaryHeap::with_capacity(retain_count);
    let mut total_entries = 0usize;
    let mut scanned = 0usize;
    let mut incomplete = false;

    for result in entries {
        let Ok(entry) = result else {
            incomplete = true;
            continue;
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            incomplete = true;
            continue;
        };
        let Some(file_type) = entry.file_type().ok() else {
            incomplete = true;
            continue;
        };
        if file_type.is_symlink() || (!file_type.is_dir() && !file_type.is_file()) {
            continue;
        }
        let candidate_path = directory_path.join(&name);
        if is_ignored_by_gitignore(&candidate_path, file_type.is_dir(), ignore_matchers) {
            continue;
        }

        if !explicit_filter_scope
            && ((file_type.is_dir() && is_default_ignored_directory(&name))
                || name.starts_with('.')
                || cap_entry_is_hidden(&entry))
        {
            continue;
        }

        scanned += 1;
        if scanned > MAX_LIST_SCAN {
            break;
        }
        total_entries += 1;
        let is_directory = file_type.is_dir();
        if retained.len() < retain_count {
            retained.push((name.clone(), is_directory));
        } else if retained
            .peek()
            .is_some_and(|largest: &(String, bool)| name.as_str() < largest.0.as_str())
        {
            retained.pop();
            retained.push((name, is_directory));
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

pub(super) fn is_ignored_by_gitignore(
    path: &Path,
    is_directory: bool,
    matchers: &[Gitignore],
) -> bool {
    for matcher in matchers {
        let matched = matcher.matched(path, is_directory);
        if !matched.is_none() {
            return matched.is_ignore();
        }
    }
    false
}

pub(super) fn load_gitignore_matchers(
    root_path: &Path,
    directory_path: &Path,
    root: &crate::filesystem::OpenedDirectory,
    directory: &crate::filesystem::OpenedDirectory,
) -> Result<Vec<Gitignore>, ServiceError> {
    let mut matchers = Vec::new();
    let mut ancestor_count = 0usize;
    let mut file_count = 0usize;
    let mut total_bytes = 0usize;
    let mut total_lines = 0usize;

    for ancestor in directory_path.ancestors() {
        ancestor_count += 1;
        if ancestor_count > MAX_GITIGNORE_ANCESTORS {
            return Err(invalid(
                "The directory path is too deep to load ignore rules safely.",
            ));
        }
        let ancestor_handle =
            open_listing_ancestor(root_path, directory_path, root, directory, ancestor)?;
        let ignore_bytes = match read_bounded_file_from_directory(
            &ancestor_handle,
            Path::new(".gitignore"),
            MAX_GITIGNORE_FILE_BYTES,
        ) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(filesystem_error(&error, "reading project ignore rules")),
        };
        if ignore_bytes.len() > MAX_GITIGNORE_FILE_BYTES {
            return Err(invalid(
                "A project .gitignore file exceeds the supported size limit.",
            ));
        }
        file_count += 1;
        total_bytes = total_bytes
            .checked_add(ignore_bytes.len())
            .filter(|total| *total <= MAX_GITIGNORE_TOTAL_BYTES)
            .ok_or_else(|| invalid("Project .gitignore files exceed the supported total size."))?;
        if file_count > MAX_GITIGNORE_FILES {
            return Err(invalid(
                "The directory hierarchy contains too many .gitignore files.",
            ));
        }

        let ignore_text = std::str::from_utf8(&ignore_bytes)
            .map_err(|_| invalid("A project .gitignore file is not valid UTF-8."))?;
        let ignore_path = ancestor.join(".gitignore");
        let mut builder = GitignoreBuilder::new(ancestor);
        for (line_index, line) in ignore_text.lines().enumerate() {
            total_lines += 1;
            if total_lines > MAX_GITIGNORE_LINES {
                return Err(invalid(
                    "Project .gitignore files contain too many rule lines.",
                ));
            }
            if line.len() > MAX_GITIGNORE_LINE_BYTES {
                return Err(invalid(
                    "A project .gitignore rule exceeds the supported line length.",
                ));
            }
            let line = if line_index == 0 {
                line.strip_prefix('\u{feff}').unwrap_or(line)
            } else {
                line
            };
            builder
                .add_line(Some(ignore_path.clone()), line)
                .map_err(|_| invalid("A project .gitignore file contains an invalid rule."))?;
        }
        let matcher = builder
            .build()
            .map_err(|_| invalid("A project .gitignore file contains invalid rules."))?;
        matchers.push(matcher);
    }

    Ok(matchers)
}

pub(super) fn open_listing_ancestor(
    root_path: &Path,
    directory_path: &Path,
    root: &crate::filesystem::OpenedDirectory,
    directory: &crate::filesystem::OpenedDirectory,
    ancestor: &Path,
) -> Result<crate::filesystem::OpenedDirectory, ServiceError> {
    if ancestor == directory_path {
        return directory
            .try_clone()
            .map_err(|error| filesystem_error(&error, "opening a project ancestor directory"));
    }
    if ancestor == root_path {
        return root
            .try_clone()
            .map_err(|error| filesystem_error(&error, "opening a project ancestor directory"));
    }
    if let Ok(relative) = ancestor.strip_prefix(root_path) {
        return crate::filesystem::open_directory_from_directory(root, relative)
            .map_err(|error| filesystem_error(&error, "opening a project ancestor directory"));
    }
    crate::filesystem::open_absolute_directory_no_follow(ancestor)
        .map_err(|error| filesystem_error(&error, "opening a project ancestor directory"))
}

#[cfg(test)]
pub(super) fn read_directory_listing(
    directory: &Path,
    explicit_filter_scope: bool,
    retain_count: usize,
) -> Result<DirectoryListing, ServiceError> {
    let directory_handle = crate::filesystem::open_absolute_directory_no_follow(directory)
        .map_err(|error| filesystem_error(&error, "opening the requested directory"))?;
    read_directory_listing_from_capability(
        &directory_handle,
        directory,
        &[],
        explicit_filter_scope,
        retain_count,
    )
}

#[cfg(windows)]
pub(super) fn cap_entry_is_hidden(entry: &cap_std::fs::DirEntry) -> bool {
    use cap_std::fs::MetadataExt;

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    entry
        .metadata()
        .is_ok_and(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
}

#[cfg(not(windows))]
pub(super) fn cap_entry_is_hidden(_entry: &cap_std::fs::DirEntry) -> bool {
    false
}

// Kept only as a benchmark baseline for the former path-based walker.
#[cfg(test)]
pub(super) fn read_directory_listing_legacy(
    directory: &Path,
    explicit_filter_scope: bool,
    retain_count: usize,
) -> Result<DirectoryListing, ServiceError> {
    if !has_gitignore_rule_ancestor(directory)
        && let Ok(entries) = fs::read_dir(directory)
    {
        return read_directory_listing_from_entries(entries, explicit_filter_scope, retain_count);
    }

    let mut retained = BinaryHeap::with_capacity(retain_count);
    let mut total_entries = 0usize;
    let mut scanned = 0usize;
    let mut incomplete = false;
    let mut walker = WalkBuilder::new(directory);
    walker
        .max_depth(Some(1))
        .follow_links(false)
        .hidden(!explicit_filter_scope)
        .parents(true)
        .ignore(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            let Some(name) = entry.file_name().to_str() else {
                return true;
            };
            let is_directory = entry.file_type().is_some_and(|kind| kind.is_dir());
            if !explicit_filter_scope && is_directory && is_default_ignored_directory(name) {
                return false;
            }
            if explicit_filter_scope {
                return true;
            }
            !name.starts_with('.')
        });

    for result in walker.build() {
        let Ok(entry) = result else {
            incomplete = true;
            continue;
        };
        if entry.depth() == 0 {
            continue;
        }
        if entry.error().is_some() {
            incomplete = true;
        }
        scanned += 1;
        if scanned > MAX_LIST_SCAN {
            break;
        }
        let Some(name) = entry.file_name().to_str() else {
            incomplete = true;
            continue;
        };
        let Some(file_type) = entry.file_type() else {
            incomplete = true;
            continue;
        };
        if entry.path_is_symlink() {
            continue;
        }
        if !file_type.is_dir() && !file_type.is_file() {
            continue;
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

#[cfg(test)]
pub(super) fn has_gitignore_rule_ancestor(directory: &Path) -> bool {
    for ancestor in directory.ancestors() {
        match fs::symlink_metadata(ancestor.join(".gitignore")) {
            Ok(_) => return true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return true,
        }
    }
    false
}

#[cfg(test)]
pub(super) fn read_directory_listing_from_entries(
    entries: fs::ReadDir,
    explicit_filter_scope: bool,
    retain_count: usize,
) -> Result<DirectoryListing, ServiceError> {
    let mut retained = BinaryHeap::with_capacity(retain_count);
    let mut total_entries = 0usize;
    let mut scanned = 0usize;
    let mut incomplete = false;

    for entry in entries {
        let Ok(entry) = entry else {
            incomplete = true;
            continue;
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            incomplete = true;
            continue;
        };
        let Some(file_type) = entry.file_type().ok() else {
            incomplete = true;
            continue;
        };
        if !explicit_filter_scope
            && ((file_type.is_dir() && is_default_ignored_directory(&name))
                || name.starts_with('.'))
        {
            continue;
        }
        #[cfg(windows)]
        if !explicit_filter_scope
            && entry
                .metadata()
                .is_ok_and(|metadata| windows_hidden(&metadata))
        {
            continue;
        }

        scanned += 1;
        if scanned > MAX_LIST_SCAN {
            break;
        }
        if file_type.is_symlink() || (!file_type.is_dir() && !file_type.is_file()) {
            continue;
        }

        total_entries += 1;
        let is_directory = file_type.is_dir();
        if retained.len() < retain_count {
            retained.push((name.clone(), is_directory));
        } else if retained
            .peek()
            .is_some_and(|largest: &(String, bool)| name.as_str() < largest.0.as_str())
        {
            retained.pop();
            retained.push((name, is_directory));
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

pub(super) fn directory_listing_response(
    relative_directory: &Path,
    listing: &DirectoryListing,
    offset: usize,
    limit: usize,
) -> Value {
    let relative_prefix = if relative_directory.as_os_str().is_empty() {
        None
    } else {
        Some(relative_directory.to_string_lossy().replace('\\', "/"))
    };

    let page = listing
        .entries
        .iter()
        .skip(offset)
        .take(limit)
        .map(|(name, is_dir)| {
            let path = match &relative_prefix {
                Some(prefix) => format!("{prefix}/{name}"),
                None => name.clone(),
            };
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
