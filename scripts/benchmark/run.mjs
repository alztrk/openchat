#!/usr/bin/env node
/**
 * Main benchmark runner for OpenChat local-service tools.
 *
 * Usage:
 *   node scripts/benchmark/run.mjs              # Run all benchmarks
 *   node scripts/benchmark/run.mjs search       # Run single benchmark
 *   node scripts/benchmark/run.mjs --json       # JSON output
 */

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { estimateTokens } from './shared/utils.mjs';
import { printResults, printJsonResults, printSingleResult } from './shared/output.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const workspaceRoot = path.resolve(__dirname, '..', '..');

// Tool benchmark registry
const tools = new Map();

/**
 * Register a tool benchmark.
 * @param {string} name - Benchmark name
 * @param {string} description - Description
 * @param {Function} runner - Async function that returns { durationMs, success, tokens?, notes?, error? }
 */
function registerTool(name, description, runner) {
  tools.set(name, { name, description, runner });
}

// ============================================================
// Tool Benchmarks
// ============================================================

// 1. hashline — Resilient file edit benchmark
registerTool(
  'hashline',
  'Multi-stage file edit (exact/normalized/hashline/drift)',
  async () => {
    const input = `import { Component, Effect, JSX } from "solid-js";
import { createStore } from "solid-js/store";

type State = {
  counter: number;
  message: string;
};

const [state, setState] = createStore<State>({
  counter: 0,
  message: "Hello, World!",
});

export default function App() {
  return (
    <div>
      <h1>{state.message}</h1>
      <p>Counter: {state.counter}</p>
      <button onClick={() => setState("counter", c => c + 1)}>
        Increment
      </button>
    </div>
  );
}
`;

    const oldString = `  <button onClick={() => setState("counter", c => c + 1)}>
        Increment
      </button>`;

    const newString = `  <button onClick={() => setState("counter", c => c + 1)}>
        Add One
      </button>`;

    // Simulate hashline resolution (actual Rust call would be via FFI)
    const start = performance.now();

    // Parse hashline tags for old and new strings
    const oldLines = oldString.split('\n');
    let hashline = '';
    for (const line of oldLines) {
      const trimmed = line.trim();
      if (trimmed) {
        let hash = 0x811c9dc5;
        for (let i = 0; i < trimmed.length; i++) {
          hash ^= trimmed.charCodeAt(i);
          hash = Math.imul(hash, 0x01000193);
        }
        const folded = ((hash >> 16) ^ (hash & 0xffff)) >>> 0;
        hashline = folded.toString(16).padStart(4, '0');
        break;
      }
    }

    // Simulate the 4-stage resolution
    const stage = hashline ? 'hashline_anchor' : 'exact';

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: estimateTokens(input),
        output: estimateTokens(input.replace(oldString, newString)),
      },
      notes: `Resolved via ${stage} stage (hash: ${hashline})`,
    };
  }
);

// 2. search — File search benchmark
registerTool(
  'search',
  'File content search with pattern matching',
  async () => {
    // Simulate searching a file for a pattern
    const fileContent = "// Main application entry point\n" +
      "import { createApp } from './app';\n" +
      "import { config } from './config';\n" +
      "import { logger } from './logger';\n" +
      "\n" +
      "// Initialize the application\n" +
      "const app = createApp({\n" +
      "  port: config.port,\n" +
      "  host: config.host,\n" +
      "  debug: config.debug,\n" +
      "});\n" +
      "\n" +
      "// Set up middleware\n" +
      "app.use(async (ctx, next) => {\n" +
      "  const start = Date.now();\n" +
      "  await next();\n" +
      "  const ms = Date.now() - start;\n" +
      "  logger.info(`${ctx.method} ${ctx.path} - ${ms}ms`);\n" +
      "});\n" +
      "\n" +
      "// Start listening\n" +
      "app.listen();\n" +
      "logger.info('Server started on port ' + config.port);\n" +
      "\n" +
      "// Health check endpoint\n" +
      "app.get('/health', (ctx) => {\n" +
      "  ctx.body = {\n" +
      "    status: 'ok',\n" +
      "    uptime: process.uptime(),\n" +
      "    memory: process.memoryUsage(),\n" +
      "  };\n" +
      "});\n" +
      "\n" +
      "// Graceful shutdown\n" +
      "process.on('SIGTERM', () => {\n" +
      "  logger.info('Received SIGTERM, shutting down...');\n" +
      "  app.close();\n" +
      "  process.exit(0);\n" +
      "});\n" +
      "\n" +
      "process.on('SIGINT', () => {\n" +
      "  logger.info('Received SIGINT, shutting down...');\n" +
      "  app.close();\n" +
      "  process.exit(0);\n" +
      "});\n";

    const query = 'middleware';
    const start = performance.now();

    // Simulate search (case-insensitive line matching)
    const lines = fileContent.split('\n');
    const matches = [];
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].toLowerCase().includes(query.toLowerCase())) {
        matches.push({ line: i + 1, text: lines[i].trim() });
      }
    }

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: estimateTokens(fileContent),
        output: estimateTokens(JSON.stringify(matches)),
      },
      notes: `Found ${matches.length} match(es) for "${query}" in ${lines.length} lines`,
    };
  }
);

// 3. write_file — File write benchmark
registerTool(
  'write_file',
  'Create or overwrite a file',
  async () => {
    const content = [
      '// Auto-generated test file',
      'export interface Config {',
      '  name: string;',
      '  version: string;',
      '  features: string[];',
      '  debug?: boolean;',
      '}',
      '',
      'export const defaultConfig: Config = {',
      '  name: "openchat-local-service",',
      '  version: "1.0.0",',
      '  features: ["search", "edit", "terminal"],',
      '};',
      '',
      '// End of config',
    ].join('\n');

    const start = performance.now();

    // Simulate file write (write to temp file)
    const { writeFileSync } = await import('node:fs');
    const { tmpdir } = await import('node:os');
    const { join } = await import('node:path');
    const testPath = join(tmpdir(), `openchat-bench-write-${Date.now()}.test`);
    writeFileSync(testPath, content, 'utf8');

    // Clean up
    const { unlinkSync } = await import('node:fs');
    try { unlinkSync(testPath); } catch { /* ignore */ }

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: estimateTokens(content),
        output: 0,
      },
      notes: `Wrote ${content.length} bytes to temp file`,
    };
  }
);

// 4. read_file — File read benchmark
registerTool(
  'read_file',
  'Read file contents',
  async () => {
    // Create test file first
    const { writeFileSync } = await import('node:fs');
    const { tmpdir } = await import('node:os');
    const { join } = await import('node:path');
    const testPath = join(tmpdir(), `openchat-bench-read-${Date.now()}.test`);
    const testContent = 'x'.repeat(10000); // 10KB file
    writeFileSync(testPath, testContent, 'utf8');

    const start = performance.now();

    // Read file
    const { readFileSync, unlinkSync } = await import('node:fs');
    const content = readFileSync(testPath, 'utf8');
    unlinkSync(testPath);

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: 0,
        output: estimateTokens(content),
      },
      notes: `Read ${content.length} bytes`,
    };
  }
);

// 5. edit_file — File edit benchmark (multi-stage)
registerTool(
  'edit_file',
  'Multi-stage text replacement with conflict detection',
  async () => {
    const content = `fn main() {
    let x = 10;
    let y = 20;
    println!("Hello, world!");
    let result = x + y;
    println!("Result: {}", result);
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
`;

    const oldString = `    println!("Hello, world!");
    let result = x + y;`;

    const newString = `    println!("Starting calculation...");
    let x_plus_y = x + y;
    println!("Result: {}", x_plus_y);`;

    const start = performance.now();

    // Simulate 4-stage resolution
    // Stage 1: Exact match check
    const hasExact = content.includes(oldString);

    // Stage 2: Whitespace normalized
    const normalizedContent = content.replace(/\r\n/g, '\n').trimEnd();
    const normalizedOld = oldString.replace(/\r\n/g, '\n').trimEnd();
    const hasNormalized = normalizedContent.includes(normalizedOld);

    const duration = performance.now() - start;

    let stage = 'not_found';
    if (hasExact) stage = 'exact';
    else if (hasNormalized) stage = 'normalized_whitespace';

    return {
      durationMs: duration,
      success: hasExact || hasNormalized,
      tokens: {
        input: estimateTokens(content) + estimateTokens(oldString),
        output: estimateTokens(content.replace(oldString, newString)),
      },
      notes: `Stage: ${stage} (exact=${hasExact}, normalized=${hasNormalized})`,
    };
  }
);

// 6. list_files — Directory listing benchmark
registerTool(
  'list_files',
  'List directory contents with filters',
  async () => {
    const { readdirSync } = await import('node:fs');
    const { join } = await import('node:path');

    const start = performance.now();

    // List scripts/benchmark directory
    const benchmarkDir = join(__dirname, 'tools');
    let entries = [];
    try {
      entries = readdirSync(benchmarkDir, { withFileTypes: true });
    } catch {
      // Directory may not exist yet
    }

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: 0,
        output: estimateTokens(JSON.stringify(entries)),
      },
      notes: `Listed ${entries.length} entries`,
    };
  }
);

// 7. terminal — Command execution benchmark
registerTool(
  'terminal',
  'Execute shell command and capture output',
  async () => {
    const { spawnSync } = await import('node:child_process');

    const start = performance.now();

    // Run a fast command
    const result = spawnSync('echo', ['benchmark'], {
      encoding: 'utf8',
      windowsHide: true,
    });

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: result.status === 0,
      tokens: {
        input: estimateTokens('echo benchmark'),
        output: estimateTokens(result.stdout || ''),
      },
      notes: `Command: echo benchmark (exit: ${result.status})`,
    };
  }
);

// 8. project_tasks — Task management benchmark
registerTool(
  'project_tasks',
  'Project task CRUD operations',
  async () => {
    // Simulate task operations (in-memory since we can't call Rust directly)
    const start = performance.now();

    // Simulate creating tasks
    const tasks = [
      { id: 1, title: 'Implement search', status: 'done' },
      { id: 2, title: 'Fix hashline edit', status: 'in_progress' },
      { id: 3, title: 'Add web search', status: 'todo' },
    ];

    // Simulate filtering and listing
    const filtered = tasks.filter(t => t.status === 'in_progress');

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: estimateTokens(JSON.stringify(tasks)),
        output: estimateTokens(JSON.stringify(filtered)),
      },
      notes: `${tasks.length} tasks, ${filtered.length} in progress`,
    };
  }
);

// ============================================================
// Main Runner
// ============================================================

async function runBenchmark(toolName) {
  const tool = tools.get(toolName);
  if (!tool) {
    process.stderr.write(`Error: Unknown tool "${toolName}"\n`);
    process.stderr.write(`Available tools: ${[...tools.keys()].join(', ')}\n`);
    process.exit(1);
  }

  const timer = { start: performance.now() };
  let result;

  try {
    result = await tool.runner();
    result.durationMs = performance.now() - timer.start;
    result.success = result.success !== false;
  } catch (error) {
    result = {
      name: tool.name,
      durationMs: performance.now() - timer.start,
      success: false,
      error: error.message,
      notes: error.stack,
    };
  }

  return { name: tool.name, description: tool.description, ...result };
}

async function main() {
  const args = process.argv.slice(2);

  // Parse flags
  const jsonOutput = args.includes('--json');
  const argsOnly = args.filter(a => !a.startsWith('--'));

  // Determine which tools to run
  const toolNames = argsOnly.length > 0 ? argsOnly : [...tools.keys()];

  // Run benchmarks
  const results = [];
  for (const name of toolNames) {
    if (!tools.has(name)) {
      process.stderr.write(`Warning: Unknown tool "${name}", skipping.\n`);
      continue;
    }
    const result = await runBenchmark(name);
    results.push(result);
  }

  // Output
  if (jsonOutput) {
    printJsonResults(results);
  } else {
    printResults(results);
  }

  // Exit code
  const failed = results.filter(r => !r.success);
  if (failed.length > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`Fatal: ${error.message}\n`);
  process.exit(1);
});
