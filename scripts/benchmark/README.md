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
node scripts/benchmark/run.mjs search
node scripts/benchmark/run.mjs hashline
node scripts/benchmark/run.mjs write_file
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

| Tool | Description | Notes |
|------|-------------|-------|
| `search` | File search with pattern matching | Tests indexed vs direct search |
| `hashline` | Resilient file edit via hashline anchors | Measures edit resolution speed |
| `write_file` | Create or overwrite a file | Tests write speed with size variants |
| `read_file` | Read file contents | Tests read speed with size variants |
| `edit_file` | Multi-stage text replacement | Tests exact, normalized, hashline, drift stages |
| `list_files` | List directory contents | Tests directory traversal speed |
| `web_search` | Multi-engine web search | Measures HTTP latency + parsing |
| `terminal` | Command execution | Depends on host system |
| `project_tasks` | Project task management | Tests task CRUD operations |

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
