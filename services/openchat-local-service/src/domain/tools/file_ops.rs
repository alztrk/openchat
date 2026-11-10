//! Scoped file read, write, info, and edit helpers.

use super::*;

/// Reads a bounded line range from a regular file beneath the active tool root.
///
/// The file is opened relative to a directory capability so path changes cannot redirect the
/// read outside the selected root after validation.
pub fn read_file(
    root: impl AsRef<Path>,
    path: &str,
    start_line: usize,
    line_count: usize,
) -> Result<Value, ServiceError> {
    let mut options = OpenOptions::new();
    options.read(true);
    let (relative_path, file, metadata) = open_file_under_root(root, path, &options)?;
    if metadata.len() > MAX_DIRECT_READ_BYTES {
        return Err(invalid(
            "The file exceeds the direct-read limit. Narrow the requested content or search for a specific term.",
        ));
    }
    let start_line = start_line.max(1);
    let line_count = line_count.clamp(1, MAX_READ_LINES);
    let mut reader = BufReader::new(file).take(MAX_DIRECT_READ_BYTES + 1);
    let mut line = String::new();
    let mut number = 0usize;
    let mut output = Vec::with_capacity(line_count.min(128));
    let mut output_bytes = 0usize;
    let mut truncated = false;

    while number.saturating_add(1) < start_line {
        let skipped = reader
            .skip_until(b'\n')
            .map_err(|error| filesystem_error(&error, "reading the requested file"))?;
        if skipped == 0 {
            break;
        }
        number += 1;
    }

    let mut reader = reader.take((MAX_OUTPUT_BYTES + 1) as u64);
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| filesystem_error(&error, "reading the requested file"))?;
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
        let clean_text = line.trim_end_matches(['\r', '\n']);
        output.push(json!([number, clean_text]));
    }
    truncated |= reader.get_ref().limit() == 0;
    let next_line = truncated.then_some(number);
    Ok(json!({
        "p": relative_path.to_string_lossy().replace('\\', "/"),
        "d": output,
        "n": next_line,
        "c": truncated,
    }))
}

/// Returns the type and byte length of a regular file inside the active tool root.
pub fn get_file_info(root: impl AsRef<Path>, path: &str) -> Result<Value, ServiceError> {
    let mut options = OpenOptions::new();
    options.read(true);
    let (relative, _file, metadata) = open_file_under_root(root, path, &options)?;
    Ok(json!({
        "path": relative.to_string_lossy().replace('\\', "/"),
        "type": "file",
        "size": metadata.len(),
    }))
}

/// Writes a bounded file beneath the active tool root without allowing path traversal to escape it.
///
/// Parent directories are created relative to an opened directory capability, so a path changed
/// after permission approval cannot redirect the write through an external symbolic link. Existing
/// hard-linked files are rejected to avoid changing an inode also named outside the selected root.
/// File mutations share the checkpoint and revert lock so these service operations cannot overlap.
pub fn write_file(
    root: impl AsRef<Path>,
    path: &str,
    content: &str,
) -> Result<Value, ServiceError> {
    let _file_change_guard =
        crate::file_changes::lock_file_changes().map_err(|_| file_change_lock_unavailable())?;
    let directory = crate::filesystem::open_absolute_directory_no_follow(root.as_ref())
        .map_err(|error| filesystem_error(&error, "opening the active tool root for writing"))?;
    write_file_with_file_change_lock_held(&directory, path, content)
}

/// Writes a bounded file while the caller holds the file-change lock.
pub(crate) fn write_file_with_file_change_lock_held(
    directory: &crate::filesystem::OpenedDirectory,
    path: &str,
    content: &str,
) -> Result<Value, ServiceError> {
    let (mut file, _) = open_file_for_direct_write(directory, path, content.len())?;
    write_opened_file(&mut file, path, content)
}

pub(crate) fn open_file_for_direct_write(
    directory: &crate::filesystem::OpenedDirectory,
    path: &str,
    content_length: usize,
) -> Result<(crate::filesystem::OpenedFile, bool), ServiceError> {
    if content_length > 10 * 1024 * 1024 {
        return Err(invalid("The file content exceeds the 10MB write limit."));
    }
    let requested = safe_relative_path(path)?;
    if let Some(parent) = requested
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        crate::filesystem::create_directory_chain_from_directory(directory, parent)
            .map_err(|error| filesystem_error(&error, "creating the parent directory"))?;
    }

    let mut create_options = OpenOptions::new();
    create_options.read(true).write(true).create_new(true);
    match crate::filesystem::open_mutation_file_from_directory(
        directory,
        &requested,
        &create_options,
    ) {
        Ok((file, _)) => Ok((file, true)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let mut options = OpenOptions::new();
            options.read(true).write(true);
            crate::filesystem::open_mutation_file_from_directory(directory, &requested, &options)
                .map(|(file, _)| (file, false))
                .map_err(|error| filesystem_error(&error, "opening the file for writing"))
        }
        Err(error) => Err(filesystem_error(&error, "creating or opening the file")),
    }
}

pub(crate) fn write_opened_file(
    file: &mut crate::filesystem::OpenedFile,
    path: &str,
    content: &str,
) -> Result<Value, ServiceError> {
    if content.len() > 10 * 1024 * 1024 {
        return Err(invalid("The file content exceeds the 10MB write limit."));
    }
    let metadata = file
        .metadata()
        .map_err(|error| filesystem_error(&error, "reading file metadata"))?;
    if !metadata.is_file() || has_multiple_links(file)? {
        return Err(invalid(
            "The requested file is not a regular, independently linked file.",
        ));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| filesystem_error(&error, "seeking within the file"))?;
    file.set_len(0)
        .map_err(|error| filesystem_error(&error, "truncating the file"))?;
    let mut writer = BufWriter::new(&mut *file);
    writer
        .write_all(content.as_bytes())
        .map_err(|error| filesystem_error(&error, "writing the file"))?;
    writer
        .flush()
        .map_err(|error| filesystem_error(&error, "flushing the file"))?;

    let relative = safe_relative_path(path)?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(json!({
        "path": relative,
        "bytesWritten": content.len(),
        "success": true,
    }))
}

/// Replaces one unique text occurrence in a regular file beneath the active tool root.
///
/// The opened file remains rooted to the selected directory, and hard-linked files are rejected
/// because editing one can also change a path outside that directory. The edit shares the
/// checkpoint and revert lock so these service operations cannot overlap.
pub fn edit_file(
    root: impl AsRef<Path>,
    path: &str,
    old_string: &str,
    new_string: &str,
) -> Result<Value, ServiceError> {
    let _file_change_guard =
        crate::file_changes::lock_file_changes().map_err(|_| file_change_lock_unavailable())?;
    let directory = crate::filesystem::open_absolute_directory_no_follow(root.as_ref())
        .map_err(|error| filesystem_error(&error, "opening the active tool root for editing"))?;
    edit_file_with_file_change_lock_held(&directory, path, old_string, new_string)
}

/// Edits a bounded file while the caller holds the file-change lock.
pub(crate) fn edit_file_with_file_change_lock_held(
    directory: &crate::filesystem::OpenedDirectory,
    path: &str,
    old_string: &str,
    new_string: &str,
) -> Result<Value, ServiceError> {
    let mut file = open_file_for_direct_edit(directory, path, old_string)?;
    edit_opened_file(&mut file, path, old_string, new_string)
}

pub(crate) fn open_file_for_direct_edit(
    directory: &crate::filesystem::OpenedDirectory,
    path: &str,
    old_string: &str,
) -> Result<crate::filesystem::OpenedFile, ServiceError> {
    if old_string.is_empty() {
        return Err(invalid("The target text to replace cannot be empty."));
    }
    let relative_path = safe_relative_path(path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    crate::filesystem::open_mutation_file_from_directory(directory, &relative_path, &options)
        .map(|(file, _)| file)
        .map_err(|error| filesystem_error(&error, "opening the requested file"))
}

pub(crate) fn edit_opened_file(
    file: &mut crate::filesystem::OpenedFile,
    path: &str,
    old_string: &str,
    new_string: &str,
) -> Result<Value, ServiceError> {
    if old_string.is_empty() {
        return Err(invalid("The target text to replace cannot be empty."));
    }
    let relative_path = safe_relative_path(path)?;
    let metadata = file
        .metadata()
        .map_err(|error| filesystem_error(&error, "reading file metadata"))?;
    if !metadata.is_file() || has_multiple_links(file)? {
        return Err(invalid(
            "The requested file is not a regular, independently linked file.",
        ));
    }
    if metadata.len() > MAX_EDIT_BYTES as u64 {
        return Err(invalid("The file exceeds the 10MB edit limit."));
    }
    let mut content = String::with_capacity(metadata.len() as usize);
    file.seek(SeekFrom::Start(0))
        .map_err(|error| filesystem_error(&error, "seeking within the file"))?;
    (&mut *file)
        .take(MAX_EDIT_BYTES as u64 + 1)
        .read_to_string(&mut content)
        .map_err(|error| filesystem_error(&error, "reading the file as UTF-8 text"))?;
    if content.len() > MAX_EDIT_BYTES {
        return Err(invalid("The file exceeds the 10MB edit limit."));
    }

    if !content.contains(old_string) {
        return Err(invalid(
            "The target text to replace was not found in the file.",
        ));
    }

    // Check for multiple occurrences to avoid ambiguous edits
    let Some(first_match) = content.find(old_string) else {
        return Err(invalid(
            "The target text to replace was not found in the file.",
        ));
    };
    let second_match = content[first_match + old_string.len()..].find(old_string);
    if second_match.is_some() {
        return Err(invalid(
            "The target text to replace occurs multiple times. Make the match more specific to avoid ambiguous edits.",
        ));
    }

    let output = content.replace(old_string, new_string);

    file.seek(SeekFrom::Start(0))
        .map_err(|error| filesystem_error(&error, "seeking within the file"))?;
    file.write_all(output.as_bytes())
        .map_err(|error| filesystem_error(&error, "writing the edited file"))?;
    file.set_len(output.len() as u64)
        .map_err(|error| filesystem_error(&error, "setting the edited file size"))?;
    file.flush()
        .map_err(|error| filesystem_error(&error, "flushing the edited file"))?;

    Ok(json!({
        "path": relative_path.to_string_lossy().replace('\\', "/"),
        "replacements": 1,
        "success": true,
    }))
}
