/**
 * read_file JSON output simulation.
 * 
 * Replicates the exact structure returned by Rust's read_file() function:
 * {
 *   "path": "file.rs",
 *   "lines": [{ "line": 1, "text": "..." }],
 *   "hashlines": ["1:abcd| ..."],  <-- content duplicated with hash prefix
 *   "nextLine": null,
 *   "truncated": false
 * }
 * 
 * This structure causes ~2.7-6x overhead in JSON output size.
 */

export function computeHashlineTag(line) {
  let hash = 0x811c9dc5;
  const trimmed = line.trim();
  for (let i = 0; i < trimmed.length; i++) {
    hash ^= trimmed.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  const folded = ((hash >> 16) ^ (hash & 0xffff)) >>> 0;
  return folded.toString(16).padStart(4, '0');
}

export function buildSimulatedReadFileOutput(content, lines, path = 'file.rs') {
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

export function estimateTokens(text) {
  if (!text || text.length === 0) return 0;
  return Math.max(1, Math.ceil(text.length / 4) + 1);
}
