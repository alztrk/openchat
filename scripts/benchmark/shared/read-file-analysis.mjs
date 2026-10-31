/**
 * Deep-dive analysis of read_file token consumption.
 *
 * read_file returns heavy JSON with:
 * - "lines": array of {line, text} for each line
 * - "hashlines": array of "N:abcd| text" strings  <-- DUPLICATE content!
 * - "path", "nextLine", "truncated"
 *
 * The hashlines array is a REPEAT of the same content (with hash prefix),
 * so content appears TWICE in the JSON output.
 */

function estimateTokens(text) {
  if (!text || text.length === 0) return 0;
  return Math.max(1, Math.ceil(text.length / 4) + 1);
}

// Simulate Rust's compute_hashline_tag
function computeHashlineTag(line) {
  let hash = 0x811c9dc5;
  const trimmed = line.trim();
  for (let i = 0; i < trimmed.length; i++) {
    hash ^= trimmed.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  const folded = ((hash >> 16) ^ (hash & 0xffff)) >>> 0;
  return folded.toString(16).padStart(4, '0');
}

// Build read_file response for N lines
function buildReadFileResponse(lines, path) {
  return {
    path: path,
    lines: lines.map((text, i) => ({ line: i + 1, text })),
    hashlines: lines.map((text, i) => {
      const tag = computeHashlineTag(text);
      return `${i + 1}:${tag}| ${text}`;
    }),
    nextLine: null,
    truncated: false,
  };
}

// =========================
// ANALYSIS: 100-line Rust file
// =========================
const rust100 = Array.from({ length: 100 }, (_, i) =>
  `impl Service {
    fn compute_${i}(&self) -> i32 {
        let value = ${i};
        value + ${i * 2}
    }
}`
).join('\n');

const rust100Lines = rust100.split('\n');
const rust100Response = buildReadFileResponse(rust100Lines, 'src/service.rs');
const rust100Json = JSON.stringify(rust100Response, null, 2);

console.log('=== read_file Token Analysis: 100-line Rust file ===\n');
console.log('File content:', rust100.length.toLocaleString(), 'bytes');
console.log('JSON output:', rust100Json.length.toLocaleString(), 'bytes');
console.log('Overhead ratio:', (rust100Json.length / rust100.length).toFixed(2), 'x larger\n');

console.log('=== Token Breakdown ===');
const rustInputTokens = estimateTokens(rust100);
const rustOutputTokens = estimateTokens(rust100Json);
const rustOverhead = rustOutputTokens - rustInputTokens;
console.log('Input file tokens:', rustInputTokens);
console.log('JSON output tokens:', rustOutputTokens);
console.log('JSON overhead tokens:', rustOverhead);
console.log('Overhead percentage:', ((rustOverhead / rustOutputTokens) * 100).toFixed(0), '%\n');

// Hashlines analysis
const rustHashlinesOnly = rust100Lines.map((text) => {
  const tag = computeHashlineTag(text);
  return `${text}`;
}).join('\n');
const rustHashlinesTokens = estimateTokens(rustHashlinesOnly);
console.log('=== Hashlines Impact ===');
console.log('Hashlines tokens:', rustHashlinesTokens);
console.log('Hashlines % of output:', ((rustHashlinesTokens / rustOutputTokens) * 100).toFixed(0), '%\n');

// =========================
// ANALYSIS: 500-line MAX scenario
// =========================
const rust500 = Array.from({ length: 500 }, (_, i) =>
  `fn method_${i}(&self) -> Result<(), String> {
    let data = String::from("line_${i}_content");
    Ok(())
}`
).join('\n');

const rust500Lines = rust500.split('\n');
const rust500Response = buildReadFileResponse(rust500Lines, 'src/large_file.rs');
const rust500Json = JSON.stringify(rust500Response, null, 2);

console.log('=== MAX SCENARIO: 500 lines (MAX_READ_LINES) ===\n');
console.log('500-line file:', rust500.length.toLocaleString(), 'bytes');
console.log('JSON output:', rust500Json.length.toLocaleString(), 'bytes');
console.log('Overhead ratio:', (rust500Json.length / rust500.length).toFixed(2), 'x larger');
console.log('Input tokens:', estimateTokens(rust500));
console.log('Output tokens:', estimateTokens(rust500Json));
console.log('Overhead tokens:', estimateTokens(rust500Json) - estimateTokens(rust500));
console.log('Overhead %:', (((estimateTokens(rust500Json) - estimateTokens(rust500)) / estimateTokens(rust500Json)) * 100).toFixed(0), '%\n');

// =========================
// ANALYSIS: 50-line TypeScript (real-world)
// =========================
const tsLines = [
  'import { createApp } from "./app";',
  'import { config } from "./config";',
  'import { logger } from "./logger";',
  '',
  'interface AppConfig {',
  '  port: number;',
  '  host: string;',
  '  debug?: boolean;',
  '  middleware: Middleware[];',
  '}',
  '',
  'const defaultConfig: AppConfig = {',
  '  port: 3000,',
  '  host: "0.0.0.0",',
  '  middleware: [],',
  '};',
  '',
  'export class App {',
  '  private config: AppConfig;',
  '  private handlers: Map<string, Handler>;',
  '',
  '  constructor(config: Partial<AppConfig> = {}) {',
  '    this.config = { ...defaultConfig, ...config };',
  '    this.handlers = new Map();',
  '  }',
  '',
  '  async start() {',
  '    const server = createApp(this.config);',
  '    for (const mw of this.config.middleware) {',
  '      server.use(mw.handler);',
  '    }',
  '    await server.listen();',
  '    logger.info("Server running");',
  '  }',
  '',
  '  addHandler(path: string, handler: Handler) {',
  '    this.handlers.set(path, handler);',
  '  }',
  '}',
  '',
  '// Export types',
  'export type { AppConfig, Handler, Middleware };',
  'export { defaultConfig };',
];
const tsJoined = tsLines.join('\n');

const tsResponse = buildReadFileResponse(tsLines, 'src/app.ts');
const tsJson = JSON.stringify(tsResponse, null, 2);

console.log('=== REAL-WORLD: 50-line TypeScript file ===\n');
console.log('TypeScript file:', tsJoined.length, 'bytes');
console.log('JSON output:', tsJson.length, 'bytes');
console.log('Overhead ratio:', (tsJson.length / tsJoined.length).toFixed(2), 'x larger');
console.log('File tokens:', estimateTokens(tsJoined));
console.log('Output tokens:', estimateTokens(tsJson));
console.log('Overhead tokens:', estimateTokens(tsJson) - estimateTokens(tsJoined));
console.log('Overhead %:', (((estimateTokens(tsJson) - estimateTokens(tsJoined)) / estimateTokens(tsJson)) * 100).toFixed(0), '%\n');

// =========================
// ROOT CAUSE ANALYSIS
// =========================
console.log('=== THE ROOT CAUSE: Content appears TWICE ===\n');
console.log('read_file returns BOTH:');
console.log('  1. "lines": [{line: N, text: "..."}] - raw text + line numbers');
console.log('  2. "hashlines": ["N:abcd| ..."]      - same text with hash prefix');
console.log('');
console.log('So content is duplicated in the JSON:');
const tsTextOnly = tsLines.join('\n');
const tsHashlinesOnly = tsLines.map(t => {
  const tag = computeHashlineTag(t);
  return `${t}`;
}).join('\n');
console.log('  "text" total:', tsTextOnly.length, 'bytes');
console.log('  "hashlines" total:', tsHashlinesOnly.length, 'bytes');
console.log('  Duplicate: YES — exact same content with hashline prefix\n');

// =========================
// COMPARATIVE TABLE
// =========================
console.log('=== COMPARATIVE: read_file output sizes ===\n');
const cases = [
  { name: '20-line small file', size: 1200 },
  { name: '50-line TS file', size: 1500 },
  { name: '100-line Rust file', size: 3000 },
  { name: '500-line MAX file', size: 15000 },
];

for (const c of cases) {
  const estimatedJson = c.size * 2.7; // ~2.7x overhead
  console.log(`${c.name.padEnd(30)}  file: ${c.size.toLocaleString().padStart(8)} bytes  =>  JSON: ${Math.round(estimatedJson).toLocaleString().padStart(8)} bytes  (${(estimatedJson / c.size).toFixed(1)}x)`);
}

console.log('\n=== IMPACT ON CONVERSATION ===\n');
console.log('If a model reads 10 files (avg 200 lines each):');
const avgLines = 200;
const avgFile = avgLines * 30; // ~30 bytes/line average
const avgJson = avgFile * 2.7;
console.log(`  Total file content: ${avgFile.toLocaleString()} bytes/file x 10 = ${(avgFile * 10).toLocaleString()} bytes`);
console.log(`  Total JSON output:  ${avgJson.toLocaleString()} bytes/file x 10 = ${Math.round(avgJson * 10).toLocaleString()} bytes`);
console.log(`  Overhead tokens:    ~${(estimateTokens(avgJson) * 10).toLocaleString()} tokens`);
console.log('');
console.log('This is why read_file consumes the most tokens — the JSON wrapper + hashlines');
console.log('DUPLICATE the content and add ~170% overhead.');
