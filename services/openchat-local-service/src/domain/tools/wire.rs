//! OpenCode wire tool naming and schema conversion.

use super::*;

pub fn opencode_wire_name(internal_name: &str) -> String {
    match internal_name {
        "read_file" | "read" => "read".to_owned(),
        "write_file" | "write" => "write".to_owned(),
        "edit_file" | "edit" => "edit".to_owned(),
        "list_files" | "list_directory" | "glob" => "glob".to_owned(),
        "search_files" | "grep" => "grep".to_owned(),
        "execute_command" | "bash" => "bash".to_owned(),
        "send_terminal_input"
        | "web_search"
        | "read_url_content"
        | "read_url"
        | "get_file_info"
        | "start_goal"
        | "goal_update"
        | "stop_goal" => {
            if internal_name == "read_url" {
                "read_url_content".to_owned()
            } else {
                internal_name.to_owned()
            }
        }
        _ => internal_name.to_owned(),
    }
}

pub(crate) fn canonical_tool_alias(name: &str) -> Option<&'static str> {
    match name {
        "read" => Some("read_file"),
        "write" => Some("write_file"),
        "edit" => Some("edit_file"),
        "glob" | "list_directory" => Some("list_files"),
        "grep" => Some("search_files"),
        "bash" => Some("execute_command"),
        "read_url" => Some("read_url_content"),
        _ => None,
    }
}

pub fn internal_tool_name(is_opencode: bool, wire_name: &str) -> String {
    if is_opencode {
        canonical_tool_alias(wire_name)
            .unwrap_or(wire_name)
            .to_owned()
    } else {
        wire_name.to_owned()
    }
}

pub fn opencode_wire_tool(tool: &ToolDefinition) -> Value {
    let wire_name = opencode_wire_name(&tool.name);
    let (parameters, description) = match wire_name.as_str() {
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
            "Execute a shell command or interact with an active terminal session. Each command runs in a fresh process. A successful working-directory change persists within this conversation and its approved workspace roots; new Windows commands use the current system and user PATH. Other shell variables do not persist. Supports sending input when waiting for user response.",
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
        _ => (tool.parameters.clone(), tool.description.as_str()),
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
    let mut seen_names = std::collections::HashSet::with_capacity(defs.len() + 2);
    let mut tools = Vec::with_capacity(defs.len() + 2);
    for def in defs {
        let wire_tool = opencode_wire_tool(def);
        if let Some(name) = wire_tool.pointer("/function/name").and_then(Value::as_str)
            && seen_names.insert(name.to_owned())
        {
            tools.push(wire_tool);
        }
    }
    // OpenCode's gateway requires `bash` and `read` entries in the wire tool list.
    // When the active profile intentionally omits `execute_command` or `read_file`
    // (for example, read-only security scans), emit inert gate stubs rather than
    // advertising active shell or file tools to the model.
    if seen_names.insert("bash".to_owned()) {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "bash",
                "description": "Command execution is unavailable in this session. Never call this tool.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {"type": "string"}
                    },
                    "required": ["command"],
                    "additionalProperties": false
                }
            }
        }));
    }
    if seen_names.insert("read".to_owned()) {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "read",
                "description": "File reading is unavailable in this session. Never call this tool.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filePath": {"type": "string"}
                    },
                    "required": ["filePath"],
                    "additionalProperties": false
                }
            }
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

pub fn responses_wire_tools(defs: &[ToolDefinition]) -> Vec<Value> {
    defs.iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            })
        })
        .collect()
}
