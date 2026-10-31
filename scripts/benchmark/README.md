# Benchmark System

Manual benchmark runner for OpenChat local-service tools. Measures execution speed and token usage for each tool.

## Architecture

```
scripts/benchmark/
├── run.mjs                      # Main benchmark runner (Node.js)
├── README.md                    # This file
├── shared/
│   ├── utils.mjs                # Token counting & timing utilities
│   └── output.mjs               # Formatted output helpers
└── tools/                       # Individual tool benchmark scripts
    ├── search.mjs
    ├── hashline.mjs
    ├── write_file.mjs
    └── ...
```

## How It Works

1. Each benchmark script in `tools/` executes a representative test case for a tool
2. Timing is measured with `performance.now()` (microsecond precision)
3. Token counting uses the `tiktoken` Rust crate via `cargo test` — see Token Counting below
4. Results are printed as a table: tool name, execution time (ms), success/failure

## Running Benchmarks

### Run all benchmarks
```bash
node scripts/benchmark/run.mjs
```

### Run a single tool benchmark
```bash
node scripts/benchmark/run.mjs edit_file
node scripts/benchmark/run.mjs search_files
node scripts/benchmark/run.mjs write_file
node scripts/benchmark/run.mjs execute_command
```

### Run benchmarks and save results
```bash
node scripts/benchmark/run.mjs --json > benchmark-results.json
```

## Token Counting

Token counts are measured separately using the Rust backend's `tiktoken` crate integration. The benchmark runner estimates input/output tokens by calling the tool's JSON payload and counting tokens using the `cl100k_base` encoding (compatible with GPT-4/GPT-3.5).

For accurate token counts, run the Rust-side token measurement tests:
```bash
cd services/openchat-local-service
cargo test token_count
```

## Available Benchmarks

| Tool | Description | Backend Rust Source |
|------|-------------|---------------------|
| `edit_file` | Replace text with 4-stage pipeline (exact → normalized → hashline → drift) | `hashline.rs::apply_resilient_edit()` called by `edit_file` tool |
| `search_files` | File content search with pattern matching | `search.rs::search_files()` |
| `write_file` | Create or overwrite entire file content | `mod.rs::write_file()` |
| `read_file` | Read file contents with size limits | `mod.rs::read_file()` |
| `list_files` | List directory contents with metadata | `mod.rs::list_files()` |
| `execute_command` | Execute shell command with sandbox | `terminal.rs::execute_with_workspace_roots()` |
| `project_tasks` | Project task CRUD operations | `project_tasks.rs` |
| `get_file_info` | Get file metadata (size, type, permissions) | `mod.rs::get_file_info()` |
| `web_search` | Parse search results from HTML (Bing/DuckDuckGo) | `web_search.rs::parse_bing_html()` |

## Adding New Benchmarks

1. Create `scripts/benchmark/tools/<name>.mjs`
2. Export a function that returns `{ durationMs, success, tokens }`
3. Register it in `scripts/benchmark/run.mjs` tool list

Example:
```javascript
export async function run() {
  const start = performance.now();
  // ... execute tool test ...
  const end = performance.now();
  return { durationMs: end - start, success: true, outputSize: result.length };
}
```
