import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/features/provider-settings/data/providerBalances.ts');
const source = fs
  .readFileSync(sourcePath, 'utf8')
  .replace(
    /import \{[^}]*\} from '@shared\/lib\/platform\/local-service';/,
    `class LocalServiceError extends Error {
      constructor(code, message) {
        super(message);
        this.code = code;
      }
    }
    const localServiceClient = {
      call: (...args) => globalThis.__providerBalancesTestState.call(...args),
    };
    const categories = ['rate_limit', 'timeout', 'provider'];
    const isServiceErrorCategory = (value) => categories.includes(value);
    const classifyServiceErrorCode = (code) => (code.includes('timeout') ? 'timeout' : 'provider');`,
  )
  .replace(
    "import { isObject } from '@shared/lib/utils/json';",
    `function isObject(value) {
      return typeof value === 'object' && value !== null && !Array.isArray(value);
    }`,
  );
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { formatBalanceAmount, loadProviderBalances } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

function deepSeekSnapshot() {
  return {
    providerId: 'deepseek',
    status: 'available',
    reasonCode: null,
    error: null,
    windows: [],
    balances: [
      {
        label: 'balance',
        currency: 'USD',
        total: 2.5,
        granted: 0,
        toppedUp: 2.5,
        limit: null,
        remaining: null,
        used: null,
      },
    ],
  };
}

function openCodeSnapshot() {
  return {
    providerId: 'opencode',
    status: 'available',
    reasonCode: null,
    error: null,
    windows: [{ label: 'rolling', windowSeconds: 18000, usedPercent: 25, resetAtUnixMs: 1800000000000 }],
    balances: [],
  };
}

async function load(providers) {
  const calls = [];
  globalThis.__providerBalancesTestState = {
    call: async (...args) => {
      calls.push(args);
      return { providers };
    },
  };
  const snapshots = await loadProviderBalances();
  return { calls, snapshots };
}

test('loads the saved-key balances through the quota RPC without sending credentials', async () => {
  const { calls, snapshots } = await load([deepSeekSnapshot(), openCodeSnapshot()]);

  assert.deepEqual(calls, [['providers.quota_balances.list', {}, 30_000]]);
  assert.deepEqual(
    snapshots.map((snapshot) => snapshot.providerId),
    ['deepseek', 'opencode'],
  );
  assert.equal(snapshots[0].balances[0].toppedUp, 2.5);
  assert.equal(snapshots[1].windows[0].usedPercent, 25);
});

test('no saved keys produce no snapshots', async () => {
  const { snapshots } = await load([]);

  assert.deepEqual(snapshots, []);
});

test('keeps the reported error code and exhausted balances for provider cards', async () => {
  const failed = {
    ...deepSeekSnapshot(),
    status: 'unavailable',
    reasonCode: 'rate_limited',
    error: { code: 'rate_limited', category: 'rate_limit' },
    balances: [],
  };
  const exhausted = { ...openCodeSnapshot(), status: 'unavailable', reasonCode: 'insufficient_balance' };

  const { snapshots } = await load([failed, exhausted]);

  assert.equal(snapshots[0].errorCode, 'rate_limited');
  assert.equal(snapshots[0].errorCategory, 'rate_limit');
  assert.equal(snapshots[1].errorCode, null);
  assert.equal(snapshots[1].errorCategory, null);
  assert.equal(snapshots[1].reasonCode, 'insufficient_balance');
});

test('rejects malformed amounts, currencies and identifiers', async () => {
  for (const total of [undefined, '2.5', Number.NaN, Number.POSITIVE_INFINITY]) {
    const snapshot = deepSeekSnapshot();
    snapshot.balances[0].total = total;
    await assert.rejects(load([snapshot]));
  }

  for (const currency of ['', 'usd', 'invalid']) {
    const snapshot = deepSeekSnapshot();
    snapshot.balances[0].currency = currency;
    await assert.rejects(load([snapshot]));
  }

  await assert.rejects(load([{ ...deepSeekSnapshot(), providerId: 'unknown' }]));
  await assert.rejects(load([{ ...deepSeekSnapshot(), providerId: ['deepseek'] }]));
  await assert.rejects(load([deepSeekSnapshot(), deepSeekSnapshot()]));
});

test('rejects malformed quota windows and error details', async () => {
  const window = { label: 'rolling', windowSeconds: 18000, usedPercent: 20, resetAtUnixMs: 1800000000000 };

  await assert.rejects(load([{ ...openCodeSnapshot(), windows: [{ ...window, usedPercent: -1 }] }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), windows: [{ ...window, usedPercent: Number.NaN }] }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), windows: [{ ...window, windowSeconds: 0 }] }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), windows: [{ ...window, resetAtUnixMs: 9e15 }] }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), windows: [{ ...window, label: 'unknown' }] }]));
  await assert.rejects(load([{ ...deepSeekSnapshot(), error: { category: 'rate_limit' } }]));
});

test('rejects inconsistent snapshot states and duplicate rows', async () => {
  const window = { label: 'rolling', windowSeconds: 18000, usedPercent: 20, resetAtUnixMs: null };
  const error = { code: 'provider_timeout' };

  await assert.rejects(load([{ ...openCodeSnapshot(), reasonCode: '' }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), status: 'unavailable', reasonCode: '' }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), status: 'unavailable', reasonCode: null }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), reasonCode: 'provider_timeout' }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), error }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), status: 'unavailable', reasonCode: 'other', error }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), error: { code: '' } }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), windows: [] }]));
  await assert.rejects(load([{ ...openCodeSnapshot(), windows: [window, window] }]));

  const duplicate = deepSeekSnapshot();
  duplicate.balances.push({ ...duplicate.balances[0] });
  await assert.rejects(load([duplicate]));

  const empty = deepSeekSnapshot();
  Object.assign(empty.balances[0], { total: null, granted: null, toppedUp: null });
  await assert.rejects(load([empty]));

  const otherCurrency = deepSeekSnapshot();
  otherCurrency.balances.push({ ...otherCurrency.balances[0], currency: 'CNY' });
  const { snapshots } = await load([otherCurrency]);
  assert.equal(snapshots[0].balances.length, 2);
});

test('accepts an unavailable snapshot that carries a matching error code', async () => {
  const { snapshots } = await load([
    {
      ...openCodeSnapshot(),
      status: 'unavailable',
      reasonCode: 'provider_timeout',
      error: { code: 'provider_timeout' },
      windows: [],
    },
  ]);

  assert.equal(snapshots[0].errorCode, 'provider_timeout');
  assert.equal(snapshots[0].errorCategory, 'timeout');
});

test('normalizes negative zero amounts', async () => {
  const snapshot = deepSeekSnapshot();
  snapshot.balances[0].granted = -0;

  const { snapshots } = await load([snapshot]);

  assert.ok(Object.is(snapshots[0].balances[0].granted, 0));
});

test('rejects a response without a provider list', async () => {
  globalThis.__providerBalancesTestState = { call: async () => ({}) };

  await assert.rejects(loadProviderBalances());
});

test('formats amounts with up to four fraction digits and survives an unknown currency', () => {
  assert.equal(formatBalanceAmount(0.0042, 'USD', 'en'), '$0.0042');
  assert.equal(formatBalanceAmount(2.5, 'USD', 'en'), '$2.50');
  assert.doesNotThrow(() => formatBalanceAmount(1, 'ZZ', 'en'));
  assert.match(formatBalanceAmount(1, 'ZZ', 'en'), /1 ZZ$/);
});
