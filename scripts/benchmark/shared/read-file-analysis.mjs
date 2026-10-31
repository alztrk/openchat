/**
 * read_file token usage analysis — current format & alternatives
 * 
 * Goal: Understand where tokens come from and how to reduce them
 * without making changes to the actual code.
 */

function estimateTokens(text) {
  if (!text || text.length === 0) return 0;
  return Math.max(1, Math.ceil(text.length / 4) + 1);
}

// Simulate current read_file output format
function simulateCurrentReadFile(content, pathStr = 'file.rs', startLine = 1, lineCount = 500) {
  const lines = content.split('\n');
  const outputLines = [];
  let outputBytes = 0;
  let truncated = false;
  let nextLine = null;

  for (let i = 0; i < lines.length && outputLines.length < lineCount; i++) {
    if (i < startLine - 1) continue;
    const text = lines[i];
    const size = Buffer.byteLength(text, 'utf-8') + 1;
    if (outputBytes + size > 64 * 1024) {
      truncated = true;
      nextLine = i + 1;
      break;
    }
    outputLines.push({ line: i + 1, text });
    outputBytes += size;
  }

  const result = {
    path: pathStr,
    lines: outputLines,
    nextLine: nextLine,
    truncated: truncated,
  };

  const jsonStr = JSON.stringify(result, null, 2); // prettified
  return {
    output: jsonStr,
    bytes: jsonStr.length,
    tokens: estimateTokens(jsonStr),
  };
}

// Simulate current read_file output format (compact JSON)
function simulateCompactReadFile(content, pathStr = 'file.rs', startLine = 1, lineCount = 500) {
  const lines = content.split('\n');
  const outputLines = [];
  let outputBytes = 0;
  let truncated = false;
  let nextLine = null;

  for (let i = 0; i < lines.length && outputLines.length < lineCount; i++) {
    if (i < startLine - 1) continue;
    const text = lines[i];
    const size = Buffer.byteLength(text, 'utf-8') + 1;
    if (outputBytes + size > 64 * 1024) {
      truncated = true;
      nextLine = i + 1;
      break;
    }
    outputLines.push({ line: i + 1, text });
    outputBytes += size;
  }

  const result = {
    path: pathStr,
    lines: outputLines,
    nextLine: nextLine,
    truncated: truncated,
  };

  const jsonStr = JSON.stringify(result); // compact
  return {
    output: jsonStr,
    bytes: jsonStr.length,
    tokens: estimateTokens(jsonStr),
  };
}

// Alternative 1: Array of arrays [lineNumber, text] instead of {line, text}
function simulateArrayFormat(content, pathStr = 'file.rs', startLine = 1, lineCount = 500) {
  const lines = content.split('\n');
  const outputLines = [];
  let outputBytes = 0;
  let truncated = false;
  let nextLine = null;

  for (let i = 0; i < lines.length && outputLines.length < lineCount; i++) {
    if (i < startLine - 1) continue;
    const text = lines[i];
    const size = Buffer.byteLength(text, 'utf-8') + 1;
    if (outputBytes + size > 64 * 1024) {
      truncated = true;
      nextLine = i + 1;
      break;
    }
    outputLines.push([i + 1, text]); // Array instead of object
    outputBytes += size;
  }

  const result = {
    path: pathStr,
    lines: outputLines,
    nextLine: nextLine,
    truncated: truncated,
  };

  const jsonStr = JSON.stringify(result, null, 2);
  return {
    output: jsonStr,
    bytes: jsonStr.length,
    tokens: estimateTokens(jsonStr),
  };
}

// Alternative 2: Plain text format (like OpenCode)
function simulateTextFormat(content, pathStr = 'file.rs', startLine = 1, lineCount = 500) {
  const lines = content.split('\n');
  let count = 0;
  let outputBytes = 0;
  let truncated = false;
  let nextLine = null;
  const rawLines = [];

  for (let i = 0; i < lines.length; i++) {
    if (count < startLine - 1) { count++; continue; }
    if (rawLines.length >= lineCount) { truncated = true; break; }
    if (outputBytes + Buffer.byteLength(lines[i], 'utf-8') > 64 * 1024) {
      truncated = true;
      nextLine = count + 1;
      break;
    }
    rawLines.push(`${count + 1}: ${lines[i]}`);
    outputBytes += Buffer.byteLength(lines[i], 'utf-8') + 3; // "N: "
    count++;
  }

  let output = `\n ${pathStr} \n file \n \n`;
  output += rawLines.join('\n');
  if (truncated) {
    output += `\n\n(Showing lines ${startLine}-${count} of ${count}. Use offset=${count + 1} to continue.)`;
  } else {
    output += `\n\n(End of file - total ${count} lines)`;
  }
  output += '\n ';

  return {
    output,
    bytes: Buffer.byteLength(output, 'utf-8'),
    tokens: estimateTokens(output),
  };
}

// Alternative 3: Single-line JSON per line (no newline separators)
function simulateSingleLineJSON(content, pathStr = 'file.rs', startLine = 1, lineCount = 500) {
  const lines = content.split('\n');
  const outputLines = [];
  let outputBytes = 0;
  let truncated = false;
  let nextLine = null;

  for (let i = 0; i < lines.length && outputLines.length < lineCount; i++) {
    if (i < startLine - 1) continue;
    const text = lines[i];
    const size = Buffer.byteLength(text, 'utf-8') + 1;
    if (outputBytes + size > 64 * 1024) {
      truncated = true;
      nextLine = i + 1;
      break;
    }
    outputLines.push({ l: i + 1, t: text }); // Shorter keys
    outputBytes += size;
  }

  const result = {
    p: pathStr,
    d: outputLines,
    n: nextLine,
    c: truncated,
  };

  const jsonStr = JSON.stringify(result, null, 2);
  return {
    output: jsonStr,
    bytes: jsonStr.length,
    tokens: estimateTokens(jsonStr),
  };
}

// Alternative 4: Base64 encoded content with line numbers
function simulateBase64Format(content, pathStr = 'file.rs', startLine = 1, lineCount = 500) {
  const lines = content.split('\n');
  const outputLines = [];
  let outputBytes = 0;
  let truncated = false;
  let nextLine = null;

  for (let i = 0; i < lines.length && outputLines.length < lineCount; i++) {
    if (i < startLine - 1) continue;
    const text = lines[i];
    const size = Buffer.byteLength(text, 'utf-8') + 1;
    if (outputBytes + size > 64 * 1024) {
      truncated = true;
      nextLine = i + 1;
      break;
    }
    outputLines.push({ l: i + 1, b: Buffer.from(text).toString('base64') });
    outputBytes += size;
  }

  const result = {
    p: pathStr,
    d: outputLines,
    n: nextLine,
    c: truncated,
  };

  const jsonStr = JSON.stringify(result);
  return {
    output: jsonStr,
    bytes: jsonStr.length,
    tokens: estimateTokens(jsonStr),
  };
}

// ===================== ANALYSIS =====================

const testCases = [
  {
    name: 'Small file (35 lines, ~1KB)',
    content: [
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
      'export type { AppConfig, Handler, Middleware };',
    ].join('\n'),
  },
  {
    name: 'Medium file (100 lines, ~14KB)',
    content: Array.from({ length: 100 }, (_, i) => {
      return `impl Service_${i} {
    fn compute_${i}(&self, data: &[u8]) -> Result<i32, Error> {
        let value = data.iter().fold(0, |acc, x| acc + x as i32);
        Ok(value + ${i})
    }

    fn process_${i}(&self, input: String) -> String {
        let processed = input.trim().to_uppercase();
        format!("result_{}: {:?}", ${i}, processed)
    }
}`;
    }).join('\n'),
  },
  {
    name: 'Large file (200 lines, ~24KB)',
    content: Array.from({ length: 200 }, (_, i) => {
      const mod = i % 3;
      if (mod === 0) {
        return `// Handler function ${i}
pub fn handler_${i}(ctx: &Context) -> HttpResponse {
    let data = ctx.params.get("id").unwrap_or("0");
    let result = compute(data);
    HttpResponse::Ok().body(result)
}

async fn compute(id: &str) -> String {
    let value: i32 = id.parse().unwrap_or(0);
    format!("computed: {}", value * 2)
}`;
      } else if (mod === 1) {
        return `import { Service_${i}, Options_${i} } from './types';
import { logger } from './utils';

export class Processor_${i} {
  private service: Service_${i};
  private opts: Options_${i};

  constructor(opts: Partial<Options_${i}> = {}) {
    this.service = new Service_${i}();
    this.opts = { ...defaultOpts_${i}, ...opts };
  }

  async process(input: string): Promise<string> {
    logger.info("Processing item ${i}");
    return this.service.transform(input);
  }
}`;
      } else {
        return `pub struct Config_${i} {
    pub name: String,
    pub enabled: bool,
    pub max_retries: u32,
    pub timeout_ms: u64,
}

impl Config_${i} {
    pub fn default() -> Self {
        Config_${i} {
            name: "module_${i}".to_string(),
            enabled: true,
            max_retries: 3,
            timeout_ms: 5000,
        }
    }
}`;
      }
    }).join('\n'),
  },
];

console.log('=== read_file Token Usage Analysis ===\n');
console.log('Format comparison: Current → Compact JSON → Array of arrays → Text format\n');
console.log('='.repeat(120));

for (const testCase of testCases) {
  console.log(`\n--- ${testCase.name} (${testCase.content.length.toLocaleString()} bytes, ${testCase.content.split('\n').length} lines) ---\n`);

  // Current format (prettified JSON with {line, text})
  const current = simulateCurrentReadFile(testCase.content);
  console.log(`Current (prettified {line,text}):`);
  console.log(`  ${current.bytes.toLocaleString()} bytes  →  ${current.tokens.toLocaleString()} tokens`);

  // Compact JSON (same structure, no spaces/newlines)
  const compact = simulateCompactReadFile(testCase.content);
  console.log(`Compact JSON (no whitespace):`);
  console.log(`  ${compact.bytes.toLocaleString()} bytes  →  ${compact.tokens.toLocaleString()} tokens  (${((current.tokens - compact.tokens) / current.tokens * 100).toFixed(0)}% reduction)`);

  // Array of arrays [line, text]
  const array = simulateArrayFormat(testCase.content);
  console.log(`Array format [[N,"text"]]:`);
  console.log(`  ${array.bytes.toLocaleString()} bytes  →  ${array.tokens.toLocaleString()} tokens  (${((current.tokens - array.tokens) / current.tokens * 100).toFixed(0)}% reduction)`);

  // Text format (OpenCode style)
  const text = simulateTextFormat(testCase.content);
  console.log(`Text format ("N: content"):`);
  console.log(`  ${text.bytes.toLocaleString()} bytes  →  ${text.tokens.toLocaleString()} tokens  (${((current.tokens - text.tokens) / current.tokens * 100).toFixed(0)}% reduction)`);

  console.log('\n');
}

console.log('='.repeat(120));
console.log('\n=== TOKEN BREAKDOWN ANALYSIS ===\n');

// Detailed analysis for a single 50-line file
const medium = testCases[0].content.split('\n').slice(0, 50).join('\n');

console.log('50-line file token breakdown:\n');

const current = simulateCurrentReadFile(medium);
console.log(`Current format output (${current.bytes.toLocaleString()} bytes, ${current.tokens.toLocaleString()} tokens):`);
console.log(`  JSON wrapper: {"path":"...","lines":[...],"nextLine":...,"truncated":...} ≈ ${(estimateTokens(JSON.stringify({path:"",lines:[],nextLine:null,truncated:false}))).toLocaleString()} tokens`);
console.log(`  Line objects: {"line": N, "text": "content"} ≈ ${estimateTokens(medium.split('\n').map((_, i) => `{"line":${i+1},"text":"..."}`).join('\n,')).toLocaleString()} tokens (${current.tokens - estimateTokens(JSON.stringify({path:"",lines:[],nextLine:null,truncated:false}))} total)`);

console.log('\n');

console.log('=== RECOMMENDATIONS (by impact) ===\n');

console.log(`1. COMPACT JSON (IMMEDIATE — no behavior change)
   - Remove newlines/spaces from JSON output
   - Current: ${current.tokens.toLocaleString()} tokens → Compact: ${estimateTokens(JSON.stringify(JSON.parse(current.output))).toLocaleString()} tokens
   - Reduction: ~${((current.tokens - estimateTokens(JSON.stringify(JSON.parse(current.output)))) / current.tokens * 100).toFixed(0)}%
   - Effort: TRIVIAL (just remove "null, 2" from JSON.stringify())

2. ARRAY OF ARRAYS [line, text] (MODERATE)
   - Replace {line, text} with [line, text]
   - Saves: ${current.tokens.toLocaleString()} → ~${estimateTokens(JSON.stringify(JSON.parse(current.output).lines.map(l => [l.line, l.text])).toLocaleString())} tokens
   - Reduction: ~30-40%
   - Effort: MODERATE (change JSON structure, update frontend parser)

3. TEXT FORMAT (HIGH — changes API contract)
   - OpenCode style: "N: content" per line
   - Saves: ${current.tokens.toLocaleString()} → ~${estimateTokens('\n file \n file \n \n' + medium.split('\n').map((l, i) => `${i+1}: ${l}`).join('\n') + '\n\n(End of file - total 50 lines)\n ').toLocaleString()} tokens
   - Reduction: ~${((current.tokens - estimateTokens('\n file \n file \n \n' + medium.split('\n').map((l, i) => `${i+1}: ${l}`).join('\n') + '\n\n(End of file - total 50 lines)\n ')) / current.tokens * 100).toFixed(0)}%
   - Effort: HIGH (breaks JSON API, requires frontend changes)

4. BASE64 ENCODING (LOW — actually INCREASES tokens)
   - Base64 adds ~33% overhead to binary content
   - NOT recommended for text files

5. REMOVE OPTIONAL FIELDS (MINIMAL)
   - Remove "nextLine", "truncated" when not needed
   - Saves: ~${estimateTokens('"nextLine":null,"truncated":false')} tokens per file (negligible)
   - NOT worth the complexity

=== CONCLUSION ===

Best ROI: Compact JSON (IMMEDIATE) + Array of arrays (MODERATE)
Total potential: ~${((current.tokens - estimateTokens(JSON.stringify(JSON.parse(current.output)).lines.map(l => [l.line, l.text]))) / current.tokens * 100).toFixed(0)}% reduction with minimal changes

Largest impact would come from NOT using JSON at all:
- Plain text format saves ~${((current.tokens - estimateTokens('\n file \n file \n \n' + medium.split('\n').map((l, i) => `${i+1}: ${l}`).join('\n') + '\n\n(End of file - total 50 lines)\n ')) / current.tokens * 100).toFixed(0)}% tokens
- But changes the entire API contract (breaking change)