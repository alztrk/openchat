import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';
import test from 'node:test';

import {
  isWithinRichMarkdownBudget,
  MAX_RICH_MARKDOWN_CHARACTERS,
} from '../src/modules/conversations/data/messageMarkdownBudget.ts';
import { estimateWrappedLineCount } from '../src/modules/conversations/data/virtualMessageLayout.ts';

function splitBasedLineCount(content, charsPerLine) {
  return content.split('\n').reduce((count, line) => count + Math.max(1, Math.ceil(line.length / charsPerLine)), 0);
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

test('wrapped line estimates match the split-based behavior at text boundaries', () => {
  const cases = [
    { content: '', charsPerLine: 32 },
    { content: 'short', charsPerLine: 32 },
    { content: 'x'.repeat(65), charsPerLine: 32 },
    { content: 'first\nsecond', charsPerLine: 32 },
    { content: 'first\n', charsPerLine: 32 },
    { content: '\n\n', charsPerLine: 32 },
    { content: 'a\r\nb\r', charsPerLine: 2 },
    { content: 'ışık 🌱', charsPerLine: 3 },
  ];

  for (const { content, charsPerLine } of cases) {
    assert.equal(
      estimateWrappedLineCount(content, charsPerLine),
      splitBasedLineCount(content, charsPerLine),
      `line estimate differs for ${JSON.stringify(content)}`,
    );
  }
});

test('wrapped line estimates handle newline-heavy messages without changing counts', () => {
  const content = '\n'.repeat(250_000);
  assert.equal(estimateWrappedLineCount(content, 80), 250_001);
  assert.equal(estimateWrappedLineCount(content, 80), splitBasedLineCount(content, 80));
});

test('rich Markdown parsing uses an inclusive 512 Ki code-unit budget', () => {
  assert.equal(isWithinRichMarkdownBudget('x'.repeat(MAX_RICH_MARKDOWN_CHARACTERS)), true);
  assert.equal(isWithinRichMarkdownBudget('x'.repeat(MAX_RICH_MARKDOWN_CHARACTERS + 1)), false);
  assert.equal(isWithinRichMarkdownBudget('line one\nline two'), true);
});

test('split and boundary-scan estimators produce the same result on a large fixture', (context) => {
  const charsPerLine = 80;
  const content = `${'x'.repeat(40)}\n`.repeat(100_000);
  const expected = splitBasedLineCount(content, charsPerLine);
  assert.equal(estimateWrappedLineCount(content, charsPerLine), expected);

  const splitSamples = [];
  const scanSamples = [];
  for (let sample = 0; sample < 5; sample += 1) {
    let startedAt = performance.now();
    assert.equal(splitBasedLineCount(content, charsPerLine), expected);
    splitSamples.push(performance.now() - startedAt);

    startedAt = performance.now();
    assert.equal(estimateWrappedLineCount(content, charsPerLine), expected);
    scanSamples.push(performance.now() - startedAt);
  }

  context.diagnostic(
    `4.1 MB / 100,000-line fixture median: split=${median(splitSamples).toFixed(2)} ms; scan=${median(scanSamples).toFixed(2)} ms`,
  );
});
