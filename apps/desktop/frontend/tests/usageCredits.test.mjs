import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const jsonSource = fs.readFileSync(path.resolve('src/shared/lib/utils/json.ts'), 'utf8');
const source = fs
  .readFileSync(path.resolve('src/features/app-settings/data/usageCredits.ts'), 'utf8')
  .replace("import { isObject } from '@shared/lib/utils/json';", jsonSource);
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { parseUsageCredits, usageCreditsDisplay } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('optional credits preserve flags, zero and fractional balances without coercion', () => {
  assert.equal(parseUsageCredits(null), null);
  assert.equal(parseUsageCredits(undefined), null);
  for (const balance of [0, 12.5, null, undefined]) {
    const credits = { hasCredits: true, unlimited: false, balance };
    assert.deepEqual(parseUsageCredits(credits), { ...credits, balance: balance ?? null });
  }
  assert.deepEqual(parseUsageCredits({ hasCredits: false, unlimited: true }), {
    hasCredits: false,
    unlimited: true,
    balance: null,
  });
});

test('malformed credits and balances fail rather than silently hiding provider data', () => {
  for (const value of [false, 0, 'credits', [], {}, { hasCredits: true }, { unlimited: false }]) {
    assert.throws(() => parseUsageCredits(value), Error);
  }
  for (const flag of [null, undefined, 'true', 0, [], {}]) {
    assert.throws(() => parseUsageCredits({ hasCredits: flag, unlimited: false }), Error);
    assert.throws(() => parseUsageCredits({ hasCredits: true, unlimited: flag }), Error);
  }
  for (const balance of [-1, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY, '12.5', true, [], {}]) {
    assert.throws(() => parseUsageCredits({ hasCredits: true, unlimited: false, balance }), Error);
  }
});

test('credit rows distinguish unlimited, unavailable, balance and hidden states', () => {
  for (const hasCredits of [true, false]) {
    for (const balance of [null, 0, 12.5]) {
      assert.deepEqual(usageCreditsDisplay({ hasCredits, unlimited: true, balance }), { kind: 'unlimited' });
    }
  }
  assert.deepEqual(usageCreditsDisplay({ hasCredits: true, unlimited: false, balance: null }), { kind: 'unavailable' });
  for (const balance of [0, 12.5]) {
    assert.deepEqual(usageCreditsDisplay({ hasCredits: true, unlimited: false, balance }), { kind: 'balance', balance });
  }
  for (const balance of [null, 0, 12.5]) {
    assert.deepEqual(usageCreditsDisplay({ hasCredits: false, unlimited: false, balance }), { kind: 'hidden' });
  }
});
