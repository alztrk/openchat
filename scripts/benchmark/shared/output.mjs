/**
 * Formatted output helpers for benchmark results.
 */

/**
 * Print a benchmark result table to stdout.
 * @param {Array<{name: string, durationMs: number, success: boolean, tokens?: {input: number, output: number}, notes?: string}>} results
 */
export function printResults(results) {
  // Calculate column widths
  const nameWidth = Math.max(20, ...results.map(r => r.name.length));
  const timeWidth = 12;
  const tokensWidth = 20;
  const statusWidth = 10;

  // Header
  const separator = '─'.repeat(nameWidth) + '  ' + '─'.repeat(timeWidth) + '  ' + '─'.repeat(tokensWidth) + '  ' + '─'.repeat(statusWidth);
  const header = padRight('Tool', nameWidth) + '  ' + padCenter('Duration', timeWidth) + '  ' + padCenter('Tokens', tokensWidth) + '  ' + padCenter('Status', statusWidth);

  console.log('\n' + separator);
  console.log(header);
  console.log(separator);

  // Results
  for (const result of results) {
    const status = result.success ? '✓ OK' : '✗ FAIL';
    const tokens = result.tokens
      ? `in:${result.tokens.input}  out:${result.tokens.output}`
      : (result.outputSize !== undefined ? `in:${estimateTokens(result.input || '')}` : '-');
    const statusStr = padCenter(result.success ? '✓ OK' : '✗ FAIL', statusWidth);
    const line = padRight(result.name, nameWidth) + '  ' + padRight(result.durationMs.toFixed(2) + 'ms', timeWidth) + '  ' + padRight(tokens, tokensWidth) + '  ' + statusStr;
    console.log(line);

    if (result.notes) {
      console.log('  ' + result.notes);
    }
  }

  console.log(separator);

  // Summary
  const successful = results.filter(r => r.success);
  const totalInputTokens = results.reduce((sum, r) => sum + (r.tokens?.input || 0), 0);
  const totalOutputTokens = results.reduce((sum, r) => sum + (r.tokens?.output || 0), 0);

  console.log(`\nTotal: ${results.length} benchmarks | ${successful.length} passed | ${results.length - successful.length} failed`);
  console.log(`Total tokens: ${totalInputTokens + totalOutputTokens} (input: ${totalInputTokens}, output: ${totalOutputTokens})`);
  console.log('');
}

/**
 * Print results as JSON (for --json flag).
 * @param {Array} results
 */
export function printJsonResults(results) {
  const json = results.map(r => ({
    name: r.name,
    durationMs: Math.round(r.durationMs * 100) / 100,
    success: r.success,
    tokens: r.tokens,
    notes: r.notes,
  }));
  console.log(JSON.stringify(json, null, 2));
}

/**
 * Print a single benchmark result with details.
 * @param {object} result
 */
export function printSingleResult(result) {
  const status = result.success ? '✓' : '✗';
  console.log(`\n${status} ${result.name}`);
  console.log(`  Duration: ${result.durationMs.toFixed(2)}ms`);
  if (result.tokens) {
    console.log(`  Tokens: input=${result.tokens.input}, output=${result.tokens.output}, total=${result.tokens.input + result.tokens.output}`);
  }
  if (result.notes) {
    console.log(`  Notes: ${result.notes}`);
  }
  if (result.error) {
    console.log(`  Error: ${result.error}`);
  }
  console.log('');
}

// Utility functions (duplicated from utils.mjs to avoid circular dependency)
function padRight(str, width) {
  return str.padEnd(width);
}

function padCenter(str, width) {
  const padding = Math.max(0, width - str.length);
  const left = Math.floor(padding / 2);
  return ' '.repeat(left) + str + ' '.repeat(padding - left);
}

function estimateTokens(text) {
  if (!text || text.length === 0) return 0;
  // Simplified cl100k_base approximation: ~4 chars per token for English
  return Math.max(1, Math.ceil(text.length / 4) + 1);
}
