import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import test from 'node:test';
import ts from 'typescript';

function compileTsModule(source) {
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return import(`data:text/javascript;base64,${Buffer.from(output).toString('base64')}`);
}

const source = readFileSync(path.resolve('src/features/chat/state/chatStoreUtils.ts'), 'utf8')
  .replace("import { isObject } from '@shared/lib/utils/json';", '')
  .replace(
    "export { isObject, numberOrNull } from '@shared/lib/utils/json';",
    `export function numberOrNull(value) {
      return typeof value === 'number' && Number.isFinite(value) ? value : null;
    }
    export function isObject(value) {
      return typeof value === 'object' && value !== null && !Array.isArray(value);
    }`,
  );

const { upsertMessage } = await compileTsModule(source);

function message(id, content) {
  return { id, content };
}

test('upsertMessage replaces the latest streamed message without mutating its input', () => {
  const earlier = message('earlier', 'first');
  const latest = message('latest', 'old response');
  const messages = [earlier, latest];
  const update = message('latest', 'updated response');

  const result = upsertMessage(messages, update);

  assert.notStrictEqual(result, messages);
  assert.strictEqual(result[0], earlier);
  assert.strictEqual(result[1], update);
  assert.equal(latest.content, 'old response');
});

test('upsertMessage preserves order when replacing a non-latest message', () => {
  const first = message('first', 'old first');
  const latest = message('latest', 'last');
  const update = message('first', 'updated first');

  const result = upsertMessage([first, latest], update);

  assert.deepEqual(result, [update, latest]);
  assert.strictEqual(result[0], update);
  assert.strictEqual(result[1], latest);
});

test('upsertMessage appends an unseen message and handles an empty list', () => {
  const first = message('first', 'first');
  const second = message('second', 'second');

  assert.deepEqual(upsertMessage([], first), [first]);
  assert.deepEqual(upsertMessage([first], second), [first, second]);
});

function measureLatestMessageUpdates(count, rounds = 160, warmupRounds = 25) {
  let messages = Array.from({ length: count }, (_, index) => message(`message-${index}`, 'initial'));
  const durations = [];

  for (let round = 0; round < rounds + warmupRounds; round += 1) {
    const latest = messages[messages.length - 1];
    if (!latest) throw new RangeError('Benchmark message list is empty.');
    const update = message(latest.id, `stream-${round}`);
    const startedAt = performance.now();
    messages = upsertMessage(messages, update);
    const elapsed = performance.now() - startedAt;
    if (round >= warmupRounds) durations.push(elapsed);
  }

  durations.sort((left, right) => left - right);
  const p50 = durations[Math.floor(durations.length * 0.5)];
  const p95 = durations[Math.floor(durations.length * 0.95)];
  if (p50 === undefined || p95 === undefined) throw new RangeError('Benchmark did not collect enough samples.');
  return { p50, p95 };
}

test('benchmark diagnostic: latest-message updates scale across loaded conversation history', (context) => {
  const measurements = [100, 1_000, 5_000, 10_000, 50_000].map((count) => {
    const { p50, p95 } = measureLatestMessageUpdates(count);
    return `${count} rows: p50=${p50.toFixed(3)} ms, p95=${p95.toFixed(3)} ms`;
  });

  context.diagnostic(
    `Production upsertMessage only; synthetic arrays exclude React rendering, DOM work, storage, and provider latency. ${measurements.join('; ')}`,
  );
});
