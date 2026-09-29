use std::{
    collections::BinaryHeap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Component, Path, PathBuf},
};

use serde_json::{Value, json};

use crate::{protocol::ServiceError, provider_schema::ToolDefinition};

mod search;
pub use search::search_files;

const MAX_LIST_RESULTS: usize = 100;
const MAX_LIST_OFFSET: usize = 100_000;
const MAX_LIST_SCAN: usize = 100_000;
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
    for entry in fs::read_dir(directory)
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

pub fn write_file(
    root: impl AsRef<Path>,
    path: &str,
    content: &str,
) -> Result<Value, ServiceError> {
    if content.len() > 10 * 1024 * 1024 {
        return Err(invalid("The file content exceeds the 10MB write limit."));
    }
    let root = canonical_root(root)?;
    let requested = safe_relative_path(path)?;
    let file_path = root.join(&requested);
    if !file_path.starts_with(&root) {
        return Err(invalid(
            "The requested path is outside the active tool root.",
        ));
    }
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| unavailable("The parent directory could not be created."))?;
    }
    let mut file = File::create(&file_path)
        .map_err(|_| unavailable("The requested file could not be created for writing."))?;
    let mut writer = BufWriter::new(&mut file);
    writer
        .write_all(content.as_bytes())
        .map_err(|_| unavailable("Failed to write content to the file."))?;
    writer
        .flush()
        .map_err(|_| unavailable("Failed to flush content to the file."))?;

    let relative = file_path.strip_prefix(&root).unwrap_or(&file_path);
    Ok(json!({
        "path": relative.to_string_lossy().replace('\\', "/"),
        "bytesWritten": content.len(),
        "success": true,
    }))
}

pub fn edit_file(
    root: impl AsRef<Path>,
    path: &str,
    old_string: &str,
    new_string: &str,
) -> Result<Value, ServiceError> {
    if old_string.is_empty() {
        return Err(invalid("The target text to replace cannot be empty."));
    }
    let (root, file_path, metadata) = resolve_file(root, path)?;
    if metadata.len() > 10 * 1024 * 1024 {
        return Err(invalid("The file exceeds the 10MB edit limit."));
    }
    let content = fs::read_to_string(&file_path)
        .map_err(|_| unavailable("The file could not be read as valid UTF-8 text."))?;

    let mut matches = content.match_indices(old_string);
    let first = matches.next();
    if first.is_none() {
        return Err(invalid(
            "The target text to replace was not found in the file.",
        ));
    }
    if matches.next().is_some() {
        return Err(invalid(
            "The target text is ambiguous and appears multiple times. Provide more unique surrounding context.",
        ));
    }
    let (start_idx, _) = first.unwrap();

    let new_capacity = content
        .len()
        .saturating_add(new_string.len())
        .saturating_sub(old_string.len());
    let mut output = String::with_capacity(new_capacity);
    output.push_str(&content[..start_idx]);
    output.push_str(new_string);
    output.push_str(&content[start_idx + old_string.len()..]);

    fs::write(&file_path, output.as_bytes())
        .map_err(|_| unavailable("Failed to write edited content to the file."))?;

    let relative = file_path.strip_prefix(&root).unwrap_or(&file_path);
    Ok(json!({
        "path": relative.to_string_lossy().replace('\\', "/"),
        "replacements": 1,
        "success": true,
    }))
}

pub fn opencode_wire_name(internal_name: &str) -> &'static str {
    match internal_name {
        "read_file" | "read" => "read",
        "write_file" | "write" => "write",
        "edit_file" | "edit" => "edit",
        "list_files" | "list_directory" | "glob" => "glob",
        "search_files" | "grep" => "grep",
        "execute_command" | "bash" => "bash",
        "get_file_info" => "get_file_info",
        _ => "custom",
    }
}

pub fn internal_tool_name(is_opencode: bool, wire_name: &str) -> String {
    if is_opencode {
        match wire_name {
            "read" => "read_file".to_owned(),
            "write" => "write_file".to_owned(),
            "edit" => "edit_file".to_owned(),
            "glob" => "list_files".to_owned(),
            "grep" => "search_files".to_owned(),
            "bash" => "execute_command".to_owned(),
            _ => wire_name.to_owned(),
        }
    } else {
        wire_name.to_owned()
    }
}

pub fn opencode_wire_tool(tool: &ToolDefinition) -> Value {
    let wire_name = opencode_wire_name(tool.name);
    let (parameters, description) = match wire_name {
        "read" => (
            json!({
                "type": "object",
                "properties": {
                    "filePath": {"type": "string", "description": "The path to the file to read."},
                    "path": {"type": "string", "description": "Alternative path parameter."},
                    "offset": {"type": "integer", "minimum": 1, "description": "The line number to start reading from (1-indexed)."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 500, "description": "The maximum number of lines to read."}
                },
                "additionalProperties": false
            }),
            "Read a bounded range of lines from a file.",
        ),
        "write" => (
            json!({
                "type": "object",
                "properties": {
                    "filePath": {"type": "string", "description": "The path to the file to create or overwrite."},
                    "path": {"type": "string", "description": "Alternative path parameter."},
                    "content": {"type": "string", "description": "The content to write to the file."}
                },
                "required": ["content"],
                "additionalProperties": false
            }),
            "Create or overwrite a file with the given content.",
        ),
        "edit" => (
            json!({
                "type": "object",
                "properties": {
                    "filePath": {"type": "string", "description": "The path to the file to modify."},
                    "path": {"type": "string", "description": "Alternative path parameter."},
                    "oldString": {"type": "string", "description": "The unique text to replace."},
                    "newString": {"type": "string", "description": "The replacement text."}
                },
                "required": ["oldString", "newString"],
                "additionalProperties": false
            }),
            "Replace a unique occurrence of text within a file.",
        ),
        "glob" => (
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "The glob pattern or directory prefix to match."},
                    "path": {"type": "string", "description": "Optional search directory."}
                },
                "additionalProperties": false
            }),
            "List files matching a glob pattern or prefix.",
        ),
        "grep" => (
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "The text pattern to search for in files."},
                    "path": {"type": "string", "description": "Optional directory to search in."},
                    "include": {"type": "string", "description": "Optional file pattern to include."}
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
            "Search for text matches within project files.",
        ),
        "bash" => (
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The shell command to execute."},
                    "workdir": {"type": "string", "description": "Optional working directory."}
                },
                "required": ["command"],
                "additionalProperties": false
            }),
            "Executes a given shell command.",
        ),
        _ => (tool.parameters.clone(), tool.description),
    };

    json!({
        "type": "function",
        "function": {
            "name": wire_name,
            "description": description,
            "parameters": parameters,
        }
    })
}

pub fn opencode_wire_tools(defs: &[ToolDefinition]) -> Vec<Value> {
    let mut tools = defs.iter().map(opencode_wire_tool).collect::<Vec<_>>();
    if !tools
        .iter()
        .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("bash"))
    {
        tools.push(opencode_wire_tool(&ToolDefinition {
            name: "execute_command",
            description: "Executes a given shell command.",
            parameters: json!({"type": "object", "properties": {"command": {"type": "string"}}, "required": ["command"]}),
        }));
    }
    if !tools
        .iter()
        .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("read"))
    {
        tools.push(opencode_wire_tool(&ToolDefinition {
            name: "read_file",
            description: "Read a bounded range of lines from a file.",
            parameters: json!({"type": "object", "properties": {"filePath": {"type": "string"}}}),
        }));
    }
    tools
}

#[allow(dead_code)]
pub fn opencode_required_tools() -> Vec<ToolDefinition> {
    definitions_for_provider("opencode")
}

pub fn definitions_for_provider(_provider_id: &str) -> Vec<ToolDefinition> {
    definitions()
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
            "write_file",
            "Create or overwrite a file with content. In approval mode use a relative path, `project:/...`, or `openchat:/...`; in full-access mode use `desktop:/...` for the user's Desktop or an absolute path for other folders.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Approval mode: relative to the project root, or prefix with project:/ or openchat:/. Full-access mode: desktop:/ for the user's Desktop, otherwise an absolute file path."},
                    "content": {"type": "string", "description": "The file content to write."}
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        ),
        (
            "edit_file",
            "Replace a unique string occurrence in a file with new content. In approval mode use a relative path, `project:/...`, or `openchat:/...`; in full-access mode use `desktop:/...` for the user's Desktop or an absolute path for other folders.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Approval mode: relative to the project root, or prefix with project:/ or openchat:/. Full-access mode: desktop:/ for the user's Desktop, otherwise an absolute file path."},
                    "oldString": {"type": "string", "description": "The exact existing text to replace."},
                    "newString": {"type": "string", "description": "The new replacement text."}
                },
                "required": ["path", "oldString", "newString"],
                "additionalProperties": false
            }),
        ),
        (
            "execute_command",
            "Execute a command safely in the project environment.",
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The command string to execute."},
                    "workdir": {"type": "string", "description": "Optional working directory."}
                },
                "required": ["command"],
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

fn invalid(message: &str) -> ServiceError {
    ServiceError::new("invalid_tool_input", message, false)
}

fn unavailable(message: &str) -> ServiceError {
    ServiceError::new("tool_path_unavailable", message, false)
}

#[cfg(test)]
mod tests;
