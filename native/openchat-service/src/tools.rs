use std::{
    collections::BinaryHeap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use ignore::WalkBuilder;
use serde_json::{Value, json};

use crate::{protocol::ServiceError, provider_schema::ToolDefinition};

mod search;
pub use search::search_files;
pub(crate) mod project_tasks;
pub mod terminal;
pub mod web_search;

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
    let hidden_filter_failed = Arc::new(AtomicBool::new(false));
    let hidden_filter_failed_for_walker = Arc::clone(&hidden_filter_failed);
    let mut walker = WalkBuilder::new(directory);
    walker
        .max_depth(Some(1))
        .follow_links(false)
        .hidden(false)
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
            match path_entry_is_hidden(name, entry.path()) {
                Ok(is_hidden) => !is_hidden,
                Err(()) => {
                    hidden_filter_failed_for_walker.store(true, Ordering::Relaxed);
                    false
                }
            }
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
    incomplete |= hidden_filter_failed.load(Ordering::Relaxed);
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
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file_path)
        .map_err(|_| unavailable("The requested file could not be opened for editing."))?;

    let mut content = String::with_capacity(metadata.len() as usize);
    file.read_to_string(&mut content)
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

    file.seek(SeekFrom::Start(0))
        .map_err(|_| unavailable("Failed to rewind file for editing."))?;
    file.write_all(output.as_bytes())
        .map_err(|_| unavailable("Failed to write edited content to the file."))?;
    file.set_len(output.len() as u64)
        .map_err(|_| unavailable("Failed to adjust file size after edit."))?;
    file.flush()
        .map_err(|_| unavailable("Failed to flush edited content to the file."))?;

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
        "send_terminal_input" => "send_terminal_input",
        "web_search" => "web_search",
        "read_url_content" | "read_url" => "read_url_content",
        "get_file_info" => "get_file_info",
        "start_goal" => "start_goal",
        "goal_update" => "goal_update",
        "stop_goal" => "stop_goal",
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
            "send_terminal_input" => "send_terminal_input".to_owned(),
            "web_search" => "web_search".to_owned(),
            "read_url_content" | "read_url" => "read_url_content".to_owned(),
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
                    "command": {"type": "string", "description": "The shell command to execute. Optional if interacting with an existing terminal_id."},
                    "terminal_id": {"type": "string", "description": "Optional terminal ID to send input to or read from an active session."},
                    "input": {"type": "string", "description": "Optional input string to send to the terminal's stdin when it waits for text."},
                    "action": {"type": "string", "enum": ["execute", "input", "read", "kill"], "description": "Action to perform on the session."},
                    "timeout_seconds": {"type": "integer", "minimum": 5, "maximum": 600, "description": "Maximum session duration in seconds."},
                    "wait_ms": {"type": "integer", "minimum": 100, "maximum": 30000, "description": "Milliseconds to wait for initial output before returning (early exit on quiet output)."}
                },
                "additionalProperties": false
            }),
            "Execute a shell command or interact with an active terminal session. Supports sending input when waiting for user response.",
        ),
        "send_terminal_input" => (
            json!({
                "type": "object",
                "properties": {
                    "terminal_id": {"type": "string", "description": "The active terminal session ID."},
                    "input": {"type": "string", "description": "Text to write to terminal stdin (e.g. 'y\\n')."},
                    "action": {"type": "string", "enum": ["input", "read", "kill"], "description": "Action to perform (default: input)."},
                    "wait_ms": {"type": "integer", "minimum": 50, "maximum": 30000, "description": "Milliseconds to wait for output."}
                },
                "required": ["terminal_id"],
                "additionalProperties": false
            }),
            "Send input text to an active terminal session waiting for input, read remaining output, or terminate the session.",
        ),
        "web_search" => (
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "The search query."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10, "description": "Maximum number of search results (default 5)."}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            "Search the web across Google, Bing, and DuckDuckGo in parallel with anti-bot resistance and clean snippet output.",
        ),
        "read_url_content" => (
            json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "HTTP or HTTPS web URL to fetch and convert to clean Markdown."},
                    "max_chars": {"type": "integer", "minimum": 500, "maximum": 30000, "description": "Maximum characters of Markdown content to return (default 6000)."}
                },
                "required": ["url"],
                "additionalProperties": false
            }),
            "Fetch a web page and convert its readable content to clean, token-friendly Markdown.",
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
            description: "Command execution is unavailable in this client.",
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

pub fn opencode_responses_wire_tools(defs: &[ToolDefinition]) -> Vec<Value> {
    let wire_tools = opencode_wire_tools(defs);
    wire_tools
        .into_iter()
        .filter_map(|mut tool| {
            let func = tool.get_mut("function")?.take();
            let mut obj = func.as_object()?.clone();
            obj.insert("type".to_owned(), json!("function"));
            Some(Value::Object(obj))
        })
        .collect()
}

#[allow(dead_code)]
pub fn opencode_required_tools() -> Vec<ToolDefinition> {
    definitions_for_provider("opencode")
}

pub fn definitions_for_provider(provider_id: &str) -> Vec<ToolDefinition> {
    definitions()
        .into_iter()
        .filter(|tool| {
            tool.name != "generate_image" || matches!(provider_id, "chatgpt" | "chatgpt_api")
        })
        .collect()
}

pub fn definitions_for_model(
    provider_id: &str,
    supports_tool_calls: Option<bool>,
) -> Vec<ToolDefinition> {
    apply_tool_call_capability(definitions_for_provider(provider_id), supports_tool_calls)
}

fn apply_tool_call_capability(
    mut definitions: Vec<ToolDefinition>,
    supports_tool_calls: Option<bool>,
) -> Vec<ToolDefinition> {
    match supports_tool_calls {
        Some(false) => definitions.clear(),
        Some(true) => {}
        None => definitions.retain(|tool| tool.name != "ask_user"),
    }
    definitions
}

pub fn definitions_for_chatgpt_model() -> Vec<ToolDefinition> {
    let mut definitions = chatgpt_model_tool_definitions();
    definitions.push(delegate_task_tool_definition());
    definitions
}

pub(crate) fn delegate_task_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "delegate_task",
        description: "Run one bounded child analysis with the current ChatGPT model. The child can only use read-only project and web tools, and its work is reported back to this response.",
        parameters: json!({
            "type": "object",
            "properties": {
                "task": {"type": "string", "minLength": 1, "maxLength": 4000},
                "context": {"type": "string", "maxLength": 8000}
            },
            "required": ["task"],
            "additionalProperties": false
        }),
    }
}

fn chatgpt_model_tool_definitions() -> Vec<ToolDefinition> {
    definitions_for_provider("chatgpt")
        .into_iter()
        .map(|mut tool| {
            if tool.name == "generate_image"
                && let Some(properties) = tool
                    .parameters
                    .get_mut("properties")
                    .and_then(Value::as_object_mut)
            {
                properties.remove("count");
            }
            tool
        })
        .collect()
}

pub fn definitions_for_chatgpt_api() -> Vec<ToolDefinition> {
    chatgpt_api_tool_definitions()
}

fn chatgpt_api_tool_definitions() -> Vec<ToolDefinition> {
    let mut definitions = definitions_for_provider("chatgpt_api");
    definitions.push(delegate_task_tool_definition());
    if let Some(image_tool) = definitions
        .iter_mut()
        .find(|tool| tool.name == "generate_image")
        && let Some(properties) = image_tool
            .parameters
            .get_mut("properties")
            .and_then(Value::as_object_mut)
    {
        properties.insert(
            "count".to_owned(),
            json!({
                "type": "integer",
                "minimum": 1,
                "maximum": 3,
                "description": "Number of images to generate. Defaults to one."
            }),
        );
        properties.insert(
            "quality".to_owned(),
            json!({
                "type": "string",
                "enum": ["low", "medium", "high", "xhigh", "max", "auto"],
                "description": "xhigh and max require gpt-image-2.5-sunburst or gpt-image-2.5-flare."
            }),
        );
    }
    definitions
}

pub fn definitions_for_request(
    provider_id: &str,
    supports_tool_calls: Option<bool>,
) -> Vec<ToolDefinition> {
    match provider_id {
        "chatgpt" => {
            apply_tool_call_capability(definitions_for_chatgpt_model(), supports_tool_calls)
        }
        "chatgpt_api" => {
            apply_tool_call_capability(definitions_for_chatgpt_api(), supports_tool_calls)
        }
        _ => definitions_for_model(provider_id, supports_tool_calls),
    }
}

pub fn context_usage_definitions(
    provider_id: &str,
    uses_responses_api: bool,
    supports_tool_calls: Option<bool>,
) -> Vec<(String, Value)> {
    let mut definitions = definitions_for_request(provider_id, supports_tool_calls);
    if !definitions.is_empty() {
        definitions.extend(crate::goals::control_tool_definitions());
    }
    if provider_id == "opencode" {
        if definitions.is_empty() {
            return Vec::new();
        }
        let wire_tools = if uses_responses_api {
            opencode_responses_wire_tools(&definitions)
        } else {
            opencode_wire_tools(&definitions)
        };
        return wire_tools
            .into_iter()
            .filter_map(|tool| {
                let wire_name = tool
                    .get("name")
                    .or_else(|| tool.pointer("/function/name"))?
                    .as_str()?;
                Some((internal_tool_name(true, wire_name), tool))
            })
            .collect();
    }

    definitions
        .into_iter()
        .map(|tool| {
            let definition = if provider_id == "chatgpt" {
                json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "strict": false,
                    "parameters": tool.parameters,
                })
            } else {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    }
                })
            };
            (tool.name.to_owned(), definition)
        })
        .collect()
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
            "Execute a shell command or interact with an active terminal session. Runs in the project folder by default. In approval mode, requires user approval.",
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The command string to execute. Optional if terminal_id is provided."},
                    "terminal_id": {"type": "string", "description": "Optional active terminal session ID to interact with."},
                    "input": {"type": "string", "description": "Optional input text to write to terminal stdin."},
                    "action": {"type": "string", "enum": ["execute", "input", "read", "kill"], "description": "Optional action on the session."},
                    "timeout_seconds": {"type": "integer", "minimum": 5, "maximum": 600, "description": "Maximum duration for the session in seconds."},
                    "wait_ms": {"type": "integer", "minimum": 100, "maximum": 30000, "description": "Milliseconds to wait for output before returning."}
                },
                "additionalProperties": false
            }),
        ),
        (
            "send_terminal_input",
            "Send input text to an active terminal session waiting for input, read remaining output, or terminate the session.",
            json!({
                "type": "object",
                "properties": {
                    "terminal_id": {"type": "string", "description": "The active terminal session ID."},
                    "input": {"type": "string", "description": "Text to write to terminal stdin."},
                    "action": {"type": "string", "enum": ["input", "read", "kill"], "description": "Action to perform (default: input)."},
                    "wait_ms": {"type": "integer", "minimum": 50, "maximum": 30000, "description": "Milliseconds to wait for output."}
                },
                "required": ["terminal_id"],
                "additionalProperties": false
            }),
        ),
        (
            "git_status",
            "Inspect the attached Git project and return the current branch, upstream counts, and changed file states. This read-only operation does not run a shell command.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ),
        (
            "git_diff",
            "Read staged and unstaged Git diffs for the attached project. Output is bounded and external diff or text conversion drivers are disabled.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ),
        (
            "git_history",
            "Read recent Git commit subjects and timestamps for the attached project.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "Maximum number of recent commits. Defaults to 20."}
                },
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
        (
            "web_search",
            "Search the web across Google, Bing, and DuckDuckGo in parallel. Returns top results with titles, URLs, and snippets.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10, "description": "Maximum number of search results (default 5)."}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        (
            "read_url_content",
            "Fetch a webpage and convert its readable content to clean, token-friendly Markdown.",
            json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "HTTP or HTTPS URL to fetch."},
                    "max_chars": {"type": "integer", "minimum": 500, "maximum": 30000, "description": "Maximum number of characters to return (default 6000)."}
                },
                "required": ["url"],
                "additionalProperties": false
            }),
        ),
        (
            "generate_image",
            "Generate an image using the connected ChatGPT image-generation capability.",
            json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 10000,
                        "description": "A detailed description of the image to generate."
                    },
                    "model": {
                        "type": "string",
                        "maxLength": 128,
                        "description": "Optional image model. Defaults to gpt-image-2."
                    },
                    "size": {
                        "type": "string",
                        "description": "Optional size such as auto, 1024x1024, 1536x1024, or 1024x1536."
                    },
                    "quality": {
                        "type": "string",
                        "enum": ["low", "medium", "high", "auto"]
                    },
                    "count": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 3,
                        "description": "Number of images to generate when the connected image endpoint supports multiple outputs."
                    },
                    "background": {
                        "type": "string",
                        "enum": ["transparent", "opaque", "auto"]
                    }
                },
                "required": ["prompt"],
                "additionalProperties": false
            }),
        ),
        (
            "ask_user",
            "Pause this response and ask the user one or more questions. Use choice questions for a single selection and text questions for a free-form reply. Wait for the answer before continuing.",
            json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 32,
                        "items": {
                            "oneOf": [
                                {
                                    "type": "object",
                                    "properties": {
                                        "id": {"type": "string", "minLength": 1, "maxLength": 128},
                                        "kind": {"type": "string", "const": "choice"},
                                        "title": {"type": "string", "minLength": 1, "maxLength": 2000},
                                        "description": {"type": "string", "maxLength": 2000},
                                        "options": {
                                            "type": "array",
                                            "minItems": 2,
                                            "maxItems": 128,
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "id": {"type": "string", "minLength": 1, "maxLength": 128},
                                                    "label": {"type": "string", "minLength": 1, "maxLength": 1000}
                                                },
                                                "required": ["id", "label"],
                                                "additionalProperties": false
                                            }
                                        },
                                        "required": {"type": "boolean"},
                                        "placeholder": {"type": "string", "maxLength": 1000},
                                        "maxLength": {"type": "integer", "minimum": 1, "maximum": 16000}
                                    },
                                    "required": ["id", "kind", "title", "options"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "id": {"type": "string", "minLength": 1, "maxLength": 128},
                                        "kind": {"type": "string", "const": "text"},
                                        "title": {"type": "string", "minLength": 1, "maxLength": 2000},
                                        "description": {"type": "string", "maxLength": 2000},
                                        "required": {"type": "boolean"},
                                        "placeholder": {"type": "string", "maxLength": 1000},
                                        "maxLength": {"type": "integer", "minimum": 1, "maximum": 16000}
                                    },
                                    "required": ["id", "kind", "title"],
                                    "additionalProperties": false
                                }
                            ]
                        }
                    }
                },
                "required": ["questions"],
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
pub(crate) use executor::ImageGenerationContext;
pub(crate) use executor::tool_call_limit_error;
pub use executor::{ToolExecutor, ToolPermissionMode};
pub(crate) use executor::{ToolPermissionRule, ToolPermissionRules, parse_tool_permission_rules};

#[cfg(test)]
mod image_tool_tests {
    use super::{
        context_usage_definitions, definitions, definitions_for_chatgpt_api,
        definitions_for_chatgpt_model, definitions_for_model, definitions_for_provider,
        definitions_for_request,
    };

    #[test]
    fn image_generation_is_advertised_only_for_supported_chatgpt_routes() {
        assert!(
            definitions()
                .iter()
                .any(|tool| tool.name == "generate_image")
        );
        assert!(
            definitions_for_provider("chatgpt")
                .iter()
                .any(|tool| tool.name == "generate_image")
        );
        assert!(
            definitions_for_chatgpt_api()
                .iter()
                .any(|tool| tool.name == "generate_image")
        );
        let api_image_tool = definitions_for_chatgpt_api()
            .into_iter()
            .find(|tool| tool.name == "generate_image")
            .expect("API image tool");
        assert_eq!(
            api_image_tool.parameters["properties"]["count"]["maximum"],
            3
        );
        for provider_id in [
            "opencode",
            "gemini",
            "groq",
            "cerebras",
            "mistral",
            "openrouter",
        ] {
            assert!(
                !definitions_for_provider(provider_id)
                    .iter()
                    .any(|tool| tool.name == "generate_image"),
                "{provider_id}"
            );
        }
    }

    #[test]
    fn delegated_child_runs_are_available_on_both_responses_routes() {
        for provider_id in ["chatgpt", "chatgpt_api"] {
            assert!(
                definitions_for_request(provider_id, Some(true))
                    .iter()
                    .any(|tool| tool.name == "delegate_task"),
                "{provider_id}"
            );
            assert!(definitions_for_request(provider_id, Some(false)).is_empty());
        }

        assert!(
            !definitions_for_provider("openrouter")
                .iter()
                .any(|tool| tool.name == "delegate_task")
        );
    }

    #[test]
    fn image_generation_is_independent_from_image_input_capability() {
        assert!(
            definitions_for_chatgpt_model()
                .iter()
                .any(|tool| tool.name == "generate_image")
        );
        let image_tool = definitions_for_chatgpt_model()
            .into_iter()
            .find(|tool| tool.name == "generate_image")
            .expect("OAuth image tool");
        assert!(image_tool.parameters["properties"].get("count").is_none());
    }

    #[test]
    fn model_capability_controls_provider_tools_and_ask_user_availability() {
        let supported = definitions_for_model("opencode", Some(true));
        assert!(supported.iter().any(|tool| tool.name == "ask_user"));

        let unsupported = definitions_for_model("opencode", Some(false));
        assert!(unsupported.is_empty());
        assert!(context_usage_definitions("opencode", false, Some(false)).is_empty());

        let unknown = definitions_for_model("opencode", None);
        assert!(!unknown.iter().any(|tool| tool.name == "ask_user"));
        assert!(unknown.iter().any(|tool| tool.name == "read_file"));
        let unknown_context = context_usage_definitions("opencode", false, None);
        assert!(!unknown_context.iter().any(|(name, _)| name == "ask_user"));

        assert!(definitions_for_request("gemini", Some(false)).is_empty());
        assert!(
            definitions_for_request("gemini", Some(true))
                .iter()
                .any(|tool| tool.name == "ask_user")
        );
        let unknown_mistral = definitions_for_request("mistral", None);
        assert!(!unknown_mistral.iter().any(|tool| tool.name == "ask_user"));
        assert!(unknown_mistral.iter().any(|tool| tool.name == "read_file"));
        let unknown_context = context_usage_definitions("mistral", false, None);
        assert!(!unknown_context.iter().any(|(name, _)| name == "ask_user"));
    }
}

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

pub(crate) fn path_entry_is_hidden(name: &str, path: &Path) -> Result<bool, ()> {
    if name.starts_with('.') {
        return Ok(true);
    }
    #[cfg(windows)]
    {
        fs::symlink_metadata(path)
            .map(|metadata| windows_hidden(&metadata))
            .map_err(|_| ())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(false)
    }
}

pub(crate) fn is_default_ignored_directory(name: &str) -> bool {
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
