/**
 * Token counting utilities using cl100k_base encoding (GPT-4/GPT-3.5 compatible).
 *
 * This implements a simplified byte-pair encoding token counter that approximates
 * tiktoken's cl100k_base encoding. For production accuracy, use the Rust tiktoken crate.
 */

// Simplified BPE-like token estimation for cl100k_base
// Covers the most common byte pairs and single-byte tokens
const COMMON_TOKENS = new Set([
  // Common words
  'the', 'and', 'you', 'that', 'was', 'for', 'are', 'with', 'this', 'they',
  'from', 'have', 'been', 'had', 'first', 'one', 'their', 'has', 'out', 'but',
  'not', 'what', 'all', 'were', 'when', 'your', 'can', 'said', 'there', 'each',
  'which', 'she', 'do', 'how', 'their', 'if', 'up', 'about', 'into', 'through',
  'during', 'before', 'after', 'above', 'between', 'below', 'while', 'where',
  // Common punctuation
  ' .', ' !', ' ?', ' ,', ':"', ' ("', ' ("', ' --', '…', '\n', '``', "''",
  ' (', ' )', ' -', ' in', ' of', ' to', ' is', ' it', ' as', ' at', ' or',
  ' an', ' be', ' this', ' that', ' on', ' by', ' do', ' go', ' me', ' my',
  ' we', ' us', ' he', ' her', ' him', ' its', ' us', ' I', ' A', ' B', ' C',
  ' D', ' E', ' F', ' G', ' H', ' I', ' J', ' K', ' L', ' M', ' N', ' O', ' P',
  ' Q', ' R', ' S', ' T', ' U', ' V', ' W', ' X', ' Y', ' Z',
  ' 0', ' 1', ' 2', ' 3', ' 4', ' 5', ' 6', ' 7', ' 8', ' 9',
]);

/**
 * Estimate token count using cl100k_base heuristic.
 * This is an approximation — not identical to tiktoken-rs.
 * For accurate counts, use the Rust tiktoken crate directly.
 *
 * @param {string} text - Text to count tokens for
 * @returns {number} Estimated token count
 */
export function estimateTokens(text) {
  if (!text || text.length === 0) return 0;

  let count = 0;
  let i = 0;
  const len = text.length;

  while (i < len) {
    // Skip whitespace
    if (text[i] === ' ' || text[i] === '\n' || text[i] === '\t' || text[i] === '\r') {
      i++;
      continue;
    }

    // Try to match multi-byte UTF-8 sequences
    let char;
    let charBytes = 1;
    const code = text.charCodeAt(i);

    if (code < 0x80) {
      char = text[i];
      charBytes = 1;
    } else if (code < 0x800) {
      char = text.substring(i, i + 2);
      charBytes = 2;
    } else if (code < 0xD800 || code >= 0xE000) {
      char = text.substring(i, i + 3);
      charBytes = 3;
    } else {
      char = text.substring(i, i + 4);
      charBytes = 4;
    }

    // Check if this character/token is in our common set
    const wordStart = i;
    let wordEnd = i + charBytes;
    while (wordEnd < len && text[wordEnd] !== ' ' && text[wordEnd] !== '\n' && text[wordEnd] !== '\t' && text[wordEnd] !== '\r') {
      wordEnd++;
    }

    const word = text.substring(wordStart, wordEnd);
    if (word.length <= 3 || COMMON_TOKENS.has(word)) {
      count += 1;
    } else if (word.length <= 4) {
      count += 1; // Short words are typically single tokens
    } else {
      // Longer words: split into subwords (simplified)
      // Approximate: ~4 chars per token for English
      count += Math.max(1, Math.ceil(word.length / 4));
    }

    i = wordEnd;
  }

  // Add a few tokens for the beginning-of-text token
  return Math.max(1, count + 1);
}

/**
 * Estimate tokens from a JSON string.
 * @param {string|object} data - JSON string or object
 * @returns {number} Estimated token count
 */
export function estimateJsonTokens(data) {
  const text = typeof data === 'string' ? data : JSON.stringify(data);
  return estimateTokens(text);
}

/**
 * Calculate execution time in milliseconds with microsecond precision.
 * @returns {{ start: number, end: () => number }}
 */
export function createTimer() {
  const start = performance.now();
  return {
    start,
    end: () => performance.now() - start,
  };
}

/**
 * Run a benchmark test multiple times and return statistics.
 * @param {Function} fn - Async function to benchmark
 * @param {number} iterations - Number of iterations
 * @returns {{ avg: number, min: number, max: number, stddev: number }}
 */
export async function runMultiple(fn, iterations = 5) {
  const results = [];
  for (let i = 0; i < iterations; i++) {
    const timer = createTimer();
    await fn();
    results.push(timer.end());
  }

  const avg = results.reduce((a, b) => a + b, 0) / results.length;
  const min = Math.min(...results);
  const max = Math.max(...results);
  const variance = results.reduce((sum, r) => sum + Math.pow(r - avg, 2), 0) / results.length;
  const stddev = Math.sqrt(variance);

  return { avg, min, max, stddev };
}

/**
 * Format duration with appropriate precision.
 * @param {number} ms - Duration in milliseconds
 * @returns {string}
 */
export function formatDuration(ms) {
  if (ms < 1) return `${(ms * 1000).toFixed(1)}μs`;
  if (ms < 100) return `${ms.toFixed(2)}ms`;
  if (ms < 1000) return `${ms.toFixed(1)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}
