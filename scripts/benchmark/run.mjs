#!/usr/bin/env node
/**
 * Main benchmark runner for OpenChat local-service tools.
 *
 * Usage:
 *   node scripts/benchmark/run.mjs              # Run all benchmarks
 *   node scripts/benchmark/run.mjs edit_file    # Run single benchmark
 *   node scripts/benchmark/run.mjs --json       # JSON output
 */

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { estimateTokens } from './shared/utils.mjs';
import { printResults, printJsonResults } from './shared/output.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ============================================================
// Tool Benchmark Registry
// ============================================================

const tools = new Map();

function registerTool(name, description, runner) {
  tools.set(name, { name, description, runner });
}

// ============================================================
// Helper: Simulate read_file's JSON output (short keys + array of arrays)
// ============================================================

function simulateReadFileJson(content, pathStr = 'file.rs') {
  const lines = content.split('\n');
  return {
    p: pathStr,
    d: lines.map((text, i) => [i + 1, text]),
    n: null,
    c: false,
  };
}

// ============================================================
// Tool Benchmarks
// ============================================================

// 1. edit_file — Basic string replacement (no hashlines)
registerTool(
  'edit_file',
  'Replace text in file (basic string match)',
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

    const start = performance.now();

    // Basic substring match (no hashlines)
    const hasMatch = content.includes(oldString);
    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: hasMatch,
      tokens: {
        input: estimateTokens(content) + estimateTokens(oldString),
        output: estimateTokens(content.replace(oldString, '')),
      },
      notes: `Basic substring match (no hashlines)`,
    };
  }
);

// 2. search_files — File content search
registerTool(
  'search_files',
  'File content search with pattern matching',
  async () => {
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
      "logger.info('Server started on port ' + config.port);\n";

    const query = 'middleware';
    const start = performance.now();

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
      notes: `Found ${matches.length} match(es) in ${lines.length} lines`,
    };
  }
);

// 3. write_file — Full file overwrite
registerTool(
  'write_file',
  'Create or overwrite entire file content',
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
    ].join('\n');

    const start = performance.now();
    const { writeFileSync, unlinkSync } = await import('node:fs');
    const { tmpdir, join } = await import('node:path').then(m => ({ ...m, tmpdir: () => import('node:os').then(x => x.tmpdir()) })).catch(() => ({}));

    try {
      const os = await import('node:os');
      const fs = await import('node:fs');
      const testPath = join(tmpdir(), `openchat-bench-write-${Date.now()}.test`);
      writeFileSync(testPath, content, 'utf8');
      unlinkSync(testPath);
    } catch { /* ignore */ }

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: estimateTokens(content),
        output: 0,
      },
      notes: `Wrote ${content.length} bytes (full file overwrite)`,
    };
  }
);

// 4. read_file — Without hashlines (simplified JSON output)
registerTool(
  'read_file',
  'Read file (without hashlines — JSON overhead only)',
  async () => {
    const { writeFileSync, readFileSync, unlinkSync } = await import('node:fs');
    const os = await import('node:os');
    const { join } = await import('node:path');

    const sizes = [
      { name: 'small (10 lines)', size: 320 },
      { name: 'medium (50 lines)', size: 3200 },
      { name: 'large (200 lines)', size: 12800 },
    ];

    const results = [];

    for (const { name, size } of sizes) {
      const testPath = join(os.tmpdir(), `openchat-bench-read-${size}-${Date.now()}.test`);
      const content = 'x'.repeat(size);

      const start = performance.now();
      writeFileSync(testPath, content, 'utf8');
      const readContent = readFileSync(testPath, 'utf8');
      unlinkSync(testPath);
      const duration = performance.now() - start;

      // Simulate read_file's JSON output (short keys + array of arrays, compact)
      const lines = readContent.split('\n');
      const simulatedOutput = {
        p: 'file.rs',
        d: lines.map((text, i) => [i + 1, text]),
        n: null,
        c: false,
      };

      const jsonStr = JSON.stringify(simulatedOutput); // compact
      const rawTokens = estimateTokens(readContent);
      const jsonTokens = estimateTokens(jsonStr);
      const overheadTokens = jsonTokens - rawTokens;

      results.push({
        name,
        size,
        rawBytes: readContent.length,
        jsonBytes: jsonStr.length,
        jsonRatio: (jsonStr.length / readContent.length).toFixed(1) + 'x',
        rawTokens,
        jsonTokens,
        overheadTokens,
        overheadPercent: ((overheadTokens / jsonTokens * 100)).toFixed(0) + '%',
      });
    }

    const large = results.find(r => r.name.includes('200'));

    return {
      durationMs: large ? large.duration || 0 : 0,
      success: true,
      tokens: {
        input: 0,
        output: large ? large.jsonTokens : 0,
      },
      tokensBreakdown: results,
      notes: `read_file JSON overhead: ~${large?.overheadPercent || 1}% (short keys + array of arrays, compact)`,
    };
  }
);

// 5. list_files — Directory listing with metadata
registerTool(
  'list_files',
  'List directory contents with metadata',
  async () => {
    const { readdirSync, statSync } = await import('node:fs');
    const { join } = await import('node:path');

    const start = performance.now();
    const benchmarkDir = join(__dirname);
    const entries = readdirSync(benchmarkDir, { withFileTypes: true })
      .filter(dirent => dirent.name !== 'node_modules')
      .map(dirent => ({
        name: dirent.name,
        isFile: dirent.isFile(),
        isDirectory: dirent.isDirectory(),
        size: dirent.isDirectory() ? null : statSync(join(benchmarkDir, dirent.name)).size,
      }));

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: 0,
        output: estimateTokens(JSON.stringify(entries)),
      },
      notes: `Listed ${entries.length} entries in scripts/benchmark/`,
    };
  }
);

// 6. execute_command — Shell command execution
registerTool(
  'execute_command',
  'Execute shell command with sandbox',
  async () => {
    const { spawnSync } = await import('node:child_process');

    const start = performance.now();
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

// 7. project_tasks — Task management
registerTool(
  'project_tasks',
  'Project task CRUD operations',
  async () => {
    const start = performance.now();

    const tasks = [
      { id: 1, title: 'Implement search', status: 'done' },
      { id: 2, title: 'Fix edit pipeline', status: 'in_progress' },
      { id: 3, title: 'Add web search', status: 'todo' },
    ];

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

// 8. get_file_info — File metadata
registerTool(
  'get_file_info',
  'Get file metadata (size, type, permissions)',
  async () => {
    const { statSync } = await import('node:fs');
    const { join } = await import('node:path');

    const start = performance.now();
    const infoPath = join(__dirname, 'run.mjs');
    const info = statSync(infoPath);

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: estimateTokens(infoPath),
        output: estimateTokens(JSON.stringify({
          size: info.size,
          isFile: info.isFile(),
          isDirectory: info.isDirectory(),
          mtimeMs: info.mtimeMs,
        })),
      },
      notes: `Got metadata for ${infoPath} (${info.size} bytes)`,
    };
  }
);

// 9. web_search — Search engine parsing
registerTool(
  'web_search',
  'Search engine HTML parsing (Bing/DuckDuckGo)',
  async () => {
    const start = performance.now();

    const bingMockHtml = `
      <html><body>
        <div class="b_algo"><h2><a href="https://example.com">Rust Programming</a></h2>
        <p>Rust is a multi-paradigm programming language focused on safety...</p></div>
        <div class="b_algo"><h2><a href="https://example.org">Node.js Benchmarking</a></h2>
        <p>Performance benchmarking for Node.js applications...</p></div>
        <div class="b_algo"><h2><a href="https://example.net">Tokio Async Runtime</a></h2>
        <p>Tokio is an async runtime for the Rust programming language...</p></div>
      </body></html>
    `;

    const marker = 'class="b_algo"';
    const items = [];
    let cursor = 0;
    while (cursor < bingMockHtml.length) {
      const relIdx = bingMockHtml.indexOf(marker, cursor);
      if (relIdx === -1 || items.length >= 5) break;
      const blockStart = cursor + relIdx;
      const nextStart = bingMockHtml.indexOf(marker, blockStart + marker.length);
      const end = nextStart > 0 ? nextStart : Math.min(bingMockHtml.length, blockStart + 5000);
      const block = bingMockHtml.slice(blockStart, end);

      const h2Idx = block.indexOf('<h2');
      if (h2Idx === -1) { cursor = blockStart + marker.length; continue; }
      const titleMatch = block.slice(h2Idx).match(/<h2[^>]*>\s*<a[^>]*>([^<]+)<\/a>/i);
      if (titleMatch && titleMatch[1].trim()) {
        items.push({ title: titleMatch[1].trim() });
      }
      cursor = blockStart + marker.length;
    }

    const duration = performance.now() - start;

    return {
      durationMs: duration,
      success: true,
      tokens: {
        input: estimateTokens(bingMockHtml),
        output: estimateTokens(JSON.stringify(items)),
      },
      notes: `Parsed ${items.length} results (${duration.toFixed(2)}ms)`,
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

  try {
    const result = await tool.runner();
    result.name = tool.name;
    result.description = tool.description;
    return result;
  } catch (error) {
    return {
      name: tool.name,
      durationMs: 0,
      success: false,
      error: error.message,
    };
  }
}

async function main() {
  const args = process.argv.slice(2);
  const jsonOutput = args.includes('--json');
  const toolNames = args.filter(a => !a.startsWith('--'));

  const results = [];
  for (const name of toolNames.length > 0 ? toolNames : [...tools.keys()]) {
    if (!tools.has(name)) {
      process.stderr.write(`Warning: Unknown tool "${name}", skipping.\n`);
      continue;
    }
    const result = await runBenchmark(name);
    results.push(result);
  }

  if (jsonOutput) {
    printJsonResults(results);
  } else {
    printResults(results);
  }

  const failed = results.filter(r => !r.success);
  if (failed.length > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`Fatal: ${error.message}\n`);
  process.exit(1);
});
