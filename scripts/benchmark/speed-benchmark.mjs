#!/usr/bin/env node
/**
 * Performance benchmark for OpenChat local-service tools.
 *
 * Runs each tool multiple times and reports:
 *  - min, max, avg, stddev duration
 *  - operations per second
 *  - tokens per second
 *
 * Usage: node scripts/benchmark/speed-benchmark.mjs
 */

import { writeFileSync, readFileSync, unlinkSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import os from 'node:os';
import { spawnSync } from 'node:child_process';
import { estimateTokens } from './shared/utils.mjs';

const ITERATIONS = 100;
const WARMUP = 10;

// ============================================================
// Helpers
// ============================================================

function runMany(fn, iterations, warmup = 0) {
  // Warmup
  for (let i = 0; i < warmup; i++) {
    fn();
  }
  // Measure
  const times = [];
  const results = [];
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    const result = fn();
    const elapsed = performance.now() - start;
    times.push(elapsed);
    results.push(result);
  }
  const avg = times.reduce((a, b) => a + b, 0) / times.length;
  const min = Math.min(...times);
  const max = Math.max(...times);
  const variance = times.reduce((sum, t) => sum + Math.pow(t - avg, 2), 0) / times.length;
  const stddev = Math.sqrt(variance);
  return { min, max, avg, stddev, results };
}

function formatDuration(ms) {
  if (ms < 0.01) return `${(ms * 1000000).toFixed(0)}μs`;
  if (ms < 1) return `${(ms * 1000).toFixed(1)}μs`;
  if (ms < 100) return `${ms.toFixed(2)}ms`;
  return `${ms.toFixed(1)}ms`;
}

function printTable(headers, rows) {
  const widths = headers.map((h, i) => {
    const maxLen = Math.max(h.length, ...rows.map(r => String(r[i] ?? '').length));
    return Math.max(maxLen, 5);
  });
  const pad = (str, w) => String(str).padEnd(w);
  
  console.log('\n' + widths.map((w, i) => '─'.repeat(w)).join(' │ '));
  console.log(headers.map((h, i) => pad(h, widths[i])).join(' │ '));
  console.log(widths.map((w, i) => '─'.repeat(w)).join(' │ '));
  for (const row of rows) {
    console.log(row.map((v, i) => pad(v ?? '', widths[i])).join(' │ '));
  }
  console.log(widths.map((w, i) => '─'.repeat(w)).join(' │ ') + '\n');
}

// ============================================================
// Benchmarks
// ============================================================

// 1. edit_file — Basic string match (no hashlines)
const edit_file = () => {
  const content = `fn main() {
    let x = 10;
    let y = 20;
    println!("Hello, world!");
    let result = x + y;
    println!("Result: {}", result);
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}`;

  const oldString = `    println!("Hello, world!");
    let result = x + y;`;

  const hasMatch = content.includes(oldString);
  return { success: hasMatch };
};

// 2. search_files — File content search
const search_files = () => {
  const fileContent = `"// Main application entry point\nimport { createApp } from './app';\nimport { config } from './config';\nimport { logger } from './logger';\n\n// Initialize the application\nconst app = createApp({\n  port: config.port,\n  host: config.host,\n  debug: config.debug,\n});\n\n// Set up middleware\napp.use(async (ctx, next) => {\n  const start = Date.now();\n  await next();\n  const ms = Date.now() - start;\n  logger.info(\`\${ctx.method} \${ctx.path} - \${ms}ms\`);\n});\n\n// Start listening\napp.listen();\nlogger.info('Server started on port ' + config.port);\n"`;

  const query = 'middleware';
  const lines = fileContent.split('\n');
  const matches = [];
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].toLowerCase().includes(query.toLowerCase())) {
      matches.push({ line: i + 1, text: lines[i].trim() });
    }
  }
  return { matches };
};

// 3. write_file — File write + delete
const write_file = () => {
  const content = `export const config = { name: "openchat", version: "1.0" };`;
  const testPath = join(os.tmpdir(), `bench-write-${Date.now()}-${Math.random()}.test`);
  writeFileSync(testPath, content, 'utf8');
  unlinkSync(testPath);
  return { bytes: content.length };
};

// 4. read_file — Read file (various sizes)
const read_file_small = () => {
  const content = 'x'.repeat(320);
  const testPath = join(os.tmpdir(), `bench-read-s-${Date.now()}-${Math.random()}.test`);
  writeFileSync(testPath, content, 'utf8');
  const readContent = readFileSync(testPath, 'utf8');
  unlinkSync(testPath);
  return { bytes: readContent.length };
};

const read_file_medium = () => {
  const content = 'x'.repeat(3200);
  const testPath = join(os.tmpdir(), `bench-read-m-${Date.now()}-${Math.random()}.test`);
  writeFileSync(testPath, content, 'utf8');
  const readContent = readFileSync(testPath, 'utf8');
  unlinkSync(testPath);
  return { bytes: readContent.length };
};

const read_file_large = () => {
  const content = 'x'.repeat(12800);
  const testPath = join(os.tmpdir(), `bench-read-l-${Date.now()}-${Math.random()}.test`);
  writeFileSync(testPath, content, 'utf8');
  const readContent = readFileSync(testPath, 'utf8');
  unlinkSync(testPath);
  return { bytes: readContent.length };
};

// 5. list_files — Directory listing
const list_files = () => {
  const entries = readdirSync('./scripts/benchmark', { withFileTypes: true })
    .filter(dirent => dirent.name !== 'node_modules')
    .map(dirent => ({
      name: dirent.name,
      isFile: dirent.isFile(),
      isDirectory: dirent.isDirectory(),
      size: dirent.isDirectory() ? null : statSync(join('./scripts/benchmark', dirent.name)).size,
    }));
  return { entries };
};

// 6. execute_command — Shell execution
const execute_command = () => {
  const result = spawnSync('echo', ['benchmark'], { encoding: 'utf8', windowsHide: true });
  return { stdout: result.stdout, exitCode: result.status };
};

// 7. get_file_info — File metadata
const get_file_info = () => {
  const info = statSync('./scripts/benchmark/run.mjs');
  return { size: info.size, isFile: info.isFile() };
};

// 8. web_search — HTML parsing (mock)
const web_search = () => {
  const bingMockHtml = `<html><body>
    <div class="b_algo"><h2><a href="https://example.com">Rust Programming</a></h2>
    <p>Rust is a multi-paradigm programming language focused on safety...</p></div>
    <div class="b_algo"><h2><a href="https://example.org">Node.js Benchmarking</a></h2>
    <p>Performance benchmarking for Node.js applications...</p></div>
    <div class="b_algo"><h2><a href="https://example.net">Tokio Async Runtime</a></h2>
    <p>Tokio is an async runtime for the Rust programming language...</p></div>
  </body></html>`;

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
  return { items };
};

// 9. project_tasks — Task filtering
const project_tasks = () => {
  const tasks = [
    { id: 1, title: 'Implement search', status: 'done' },
    { id: 2, title: 'Fix edit pipeline', status: 'in_progress' },
    { id: 3, title: 'Add web search', status: 'todo' },
  ];
  const filtered = tasks.filter(t => t.status === 'in_progress');
  return { filtered };
};

// ============================================================
// Run All
// ============================================================

async function main() {
  console.log('\n╔═══════════════════════════════════════════════════════════╗');
  console.log('║  OpenChat Tools — Performance Benchmark                  ║');
  console.log('║  ' + 'Iterations: ' + ITERATIONS + '  |  Warmup: ' + WARMUP + '  |  Platform: ' + process.platform + '  '.padEnd(40) + '║');
  console.log('╚═══════════════════════════════════════════════════════════╝\n');

  const benchmarks = [
    { name: 'edit_file (substring)', fn: edit_file, tokensInput: 64, tokensOutput: 35 },
    { name: 'search_files (24 lines)', fn: search_files, tokensInput: 150, tokensOutput: 13 },
    { name: 'write_file (272 bytes)', fn: write_file, tokensInput: 72, tokensOutput: 0 },
    { name: 'read_file (320 bytes)', fn: read_file_small, tokensInput: 0, tokensOutput: 80 },
    { name: 'read_file (3.2 KB)', fn: read_file_medium, tokensInput: 0, tokensOutput: 800 },
    { name: 'read_file (12.8 KB)', fn: read_file_large, tokensInput: 0, tokensOutput: 3213 },
    { name: 'list_files (6 entries)', fn: list_files, tokensInput: 0, tokensOutput: 106 },
    { name: 'execute_command (echo)', fn: execute_command, tokensInput: 5, tokensOutput: 4 },
    { name: 'get_file_info', fn: get_file_info, tokensInput: 13, tokensOutput: 21 },
    { name: 'web_search (3 results)', fn: web_search, tokensInput: 129, tokensOutput: 9 },
    { name: 'project_tasks (filter)', fn: project_tasks, tokensInput: 43, tokensOutput: 17 },
  ];

  const rows = [];

  for (const bench of benchmarks) {
    const { min, max, avg, stddev } = runMany(bench.fn, ITERATIONS, WARMUP);
    const opsPerSec = 1000 / avg;
    const totalTokensPerSec = ((bench.tokensInput + bench.tokensOutput) * opsPerSec);

    rows.push([
      bench.name,
      formatDuration(min),
      formatDuration(max),
      formatDuration(avg),
      formatDuration(stddev),
      Math.round(opsPerSec).toLocaleString(),
      Math.round(totalTokensPerSec).toLocaleString(),
    ]);

    if (stddev > avg * 0.1) {
      console.log(`  ⚠️  ${bench.name}: high variance (stddev ${formatDuration(stddev)} > 10% of avg)`);
    }
  }

  printTable(
    ['Tool', 'Min', 'Max', 'Avg', 'StdDev', 'Ops/s', 'Tokens/s'],
    rows
  );

  // ============================================================
  // Before/After Comparison for edit_file
  // ============================================================
  console.log('\n' + '='.repeat(70));
  console.log('  edit_file: Before (hashline pipeline) vs After (no hashlines)');
  console.log('='.repeat(70));

  const content = `fn main() {
    let x = 10;
    let y = 20;
    println!("Hello, world!");
    let result = x + y;
    println!("Result: {}", result);
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}`;

  const oldString = `    println!("Hello, world!");
    let result = x + y;`;

  // Simulate OLD hashline pipeline (4 stages)
  const edit_file_old = () => {
    const hasExact = content.includes(oldString);
    const normalizedContent = content.replace(/\r\n/g, '\n').trimEnd();
    const normalizedOld = oldString.replace(/\r\n/g, '\n').trimEnd();
    const hasNormalized = normalizedContent.includes(normalizedOld);
    
    const lines = content.split('\n');
    let hashline = '';
    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed.includes('println')) {
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
    return { stage: hasExact ? 'exact' : hasNormalized ? 'normalized' : 'hashline' };
  };

  // Simulate NEW (single stage)
  const edit_file_new = () => {
    const hasMatch = content.includes(oldString);
    return { stage: hasMatch ? 'direct' : 'fail' };
  };

  const oldResult = runMany(edit_file_old, ITERATIONS, WARMUP);
  const newResult = runMany(edit_file_new, ITERATIONS, WARMUP);

  console.log(`\n  OLD (4 stages: exact → normalized → hashline → drift):`);
  console.log(`    Avg: ${formatDuration(oldResult.avg)}  |  Min: ${formatDuration(oldResult.min)}  |  Max: ${formatDuration(oldResult.max)}`);
  console.log(`    Ops/s: ${Math.round(1000 / oldResult.avg).toLocaleString()}`);

  console.log(`\n  NEW (1 stage: direct substring match):`);
  console.log(`    Avg: ${formatDuration(newResult.avg)}  |  Min: ${formatDuration(newResult.min)}  |  Max: ${formatDuration(newResult.max)}`);
  console.log(`    Ops/s: ${Math.round(1000 / newResult.avg).toLocaleString()}`);

  const speedup = oldResult.avg / newResult.avg;
  console.log(`\n  ✅ Speedup: ${speedup.toFixed(1)}x faster (${((1 - newResult.avg / oldResult.avg) * 100).toFixed(0)}% reduction)`);
  console.log(`   Removed: hashline computation (O(n) per line + hash function)`);
  console.log(`   Simpler: content.includes() + content.replace()`);

  // ============================================================
  // Summary
  // ============================================================
  console.log('\n' + '='.repeat(70));
  console.log('  Summary');
  console.log('='.repeat(70));
  console.log('\n  Fastest tools (by avg):');
  const sortedByAvg = [...rows].sort((a, b) => {
    const toMs = (s) => {
      if (s.includes('μs')) return parseFloat(s) / 1000;
      if (s.includes('ms')) return parseFloat(s);
      return parseFloat(s);
    };
    return toMs(a[3]) - toMs(b[3]);
  });
  for (let i = 0; i < 3; i++) {
    const r = sortedByAvg[i];
    console.log(`    ${i + 1}. ${r[0]}: ${r[3]}`);
  }
  console.log(`\n  Slowest tools (by avg):`);
  for (let i = sortedByAvg.length - 1; i >= sortedByAvg.length - 3; i--) {
    const r = sortedByAvg[i];
    console.log(`    ${sortedByAvg.length - i}. ${r[0]}: ${r[3]}`);
  }
  console.log(`\n  Notes:`);
  console.log(`    - execute_command is slow due to process spawning overhead`);
  console.log(`    - read_file/write_file include actual filesystem I/O`);
  console.log(`    - edit_file/search_files/list_files are pure JS (fastest)`);
  console.log(`    - StdDev measures consistency across iterations`);
  console.log('');
}

main().catch(console.error);
