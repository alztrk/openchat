import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/shared/lib/preferences/notificationPreferences.ts');
const source = fs
  .readFileSync(sourcePath, 'utf8')
  .replace(
    "import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';",
    'const unifiedStorage = { getItem: () => null, setPersistedItem: async () => undefined };',
  )
  .replace("import { invoke } from '@shared/lib/platform/tauri';", 'const invoke = async () => undefined;');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { commitNotificationTimeDraft, isValidNotificationTime, shouldSkipWindowsNotificationDelivery } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('quiet-hours time validation accepts only complete 24-hour HH:mm values', () => {
  for (const value of ['00:00', '08:05', '23:59']) {
    assert.equal(isValidNotificationTime(value), true);
  }
  for (const value of ['', '0', '09:', '24:00', '12:60', '9:05']) {
    assert.equal(isValidNotificationTime(value), false);
  }
});

test('notification delivery continues when Windows cannot read the current status', () => {
  assert.equal(shouldSkipWindowsNotificationDelivery('unavailable'), false);
  assert.equal(shouldSkipWindowsNotificationDelivery('enabled'), false);
  assert.equal(shouldSkipWindowsNotificationDelivery('disabledForApplication'), true);
  assert.equal(shouldSkipWindowsNotificationDelivery('disabledForUser'), true);
  assert.equal(shouldSkipWindowsNotificationDelivery('disabledByGroupPolicy'), true);
  assert.equal(shouldSkipWindowsNotificationDelivery('disabledByManifest'), true);
  assert.equal(shouldSkipWindowsNotificationDelivery('notSupported'), true);
});

test('an incomplete quiet-hours draft never reaches persistence', async () => {
  let saveCalls = 0;

  const result = await commitNotificationTimeDraft('', '22:00', async () => {
    saveCalls += 1;
    return true;
  });

  assert.equal(result, 'invalid');
  assert.equal(saveCalls, 0);
});

test('a valid quiet-hours draft persists only when it differs from the saved value', async () => {
  const savedValues = [];
  const save = async (value) => {
    savedValues.push(value);
    return true;
  };

  assert.equal(await commitNotificationTimeDraft('22:00', '22:00', save), 'unchanged');
  assert.equal(await commitNotificationTimeDraft('09:45', '22:00', save), 'saved');
  assert.deepEqual(savedValues, ['09:45']);
});

test('a failed quiet-hours save remains distinguishable from a successful commit', async () => {
  const result = await commitNotificationTimeDraft('09:45', '22:00', async () => false);

  assert.equal(result, 'failed');
});
