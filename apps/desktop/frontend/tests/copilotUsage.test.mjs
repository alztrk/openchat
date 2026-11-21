import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/features/app-settings/data/copilotUsage.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { copilotQuotaResetAt, isCopilotQuotaVisible } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

function quota(kind, overrides = {}) {
  return {
    id: kind,
    kind,
    entitlement: 50,
    remaining: 25,
    percentRemaining: 50,
    resetAtUnixMs: null,
    unlimited: false,
    hasQuota: true,
    tokenBasedBilling: false,
    ...overrides,
  };
}

test('shows finite chat and completions quotas, including an exhausted zero allowance', () => {
  for (const kind of ['chat', 'completions']) {
    assert.equal(isCopilotQuotaVisible(quota(kind)), true);
    assert.equal(isCopilotQuotaVisible(quota(kind, { entitlement: 0, remaining: 0 })), true);
    assert.equal(isCopilotQuotaVisible(quota(kind, { unlimited: null })), true);
  }
});

test('hides unlimited or unspecified chat and completions allowances and unknown buckets', () => {
  for (const kind of ['chat', 'completions']) {
    for (const overrides of [{ unlimited: true }, { entitlement: null }, { entitlement: -1 }]) {
      assert.equal(isCopilotQuotaVisible(quota(kind, overrides)), false);
    }
  }
  assert.equal(isCopilotQuotaVisible(quota('other')), false);
});

test('preserves premium quota visibility when unlimited or unavailable', () => {
  assert.equal(isCopilotQuotaVisible(quota('premiumInteractions', { unlimited: true })), true);
  assert.equal(isCopilotQuotaVisible(quota('premiumInteractions', { entitlement: null })), true);
});

test('hides buckets when the provider denies a quota', () => {
  for (const kind of ['chat', 'completions', 'premiumInteractions']) {
    assert.equal(isCopilotQuotaVisible(quota(kind, { hasQuota: false })), false);
    assert.equal(isCopilotQuotaVisible(quota(kind, { hasQuota: null })), true);
  }
});

test('uses a positive server reset for every kind, including premium and unlimited usage', () => {
  for (const kind of ['chat', 'completions', 'premiumInteractions', 'other']) {
    for (const unlimited of [true, false]) {
      assert.equal(copilotQuotaResetAt(quota(kind, { resetAtUnixMs: 1_900_000_000_000, unlimited })), 1_900_000_000_000);
    }
  }
});

test('only premium usage falls back to the next local month for null or zero resets', () => {
  const now = new Date(2026, 11, 31, 23, 59);
  for (const resetAtUnixMs of [null, 0]) {
    for (const kind of ['chat', 'completions', 'other']) {
      assert.equal(copilotQuotaResetAt(quota(kind, { resetAtUnixMs }), now), null);
    }
    assert.equal(copilotQuotaResetAt(quota('premiumInteractions', { resetAtUnixMs }), now), new Date(2027, 0, 1).getTime());
  }
});
