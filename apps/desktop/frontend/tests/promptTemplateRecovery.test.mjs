import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const storageSource = readSource('src/features/model-catalog/data/promptTemplates.ts');
const tabSource = readSource('src/features/model-catalog/components/PromptTemplatesTab.tsx');
const localePaths = [
  'src/shared/lib/i18n/locales/de.json',
  'src/shared/lib/i18n/locales/en.json',
  'src/shared/lib/i18n/locales/es.json',
  'src/shared/lib/i18n/locales/fr.json',
  'src/shared/lib/i18n/locales/tr.json',
];

test('invalid prompt template data retains its raw value without replacing it during read', () => {
  assert.match(storageSource, /new PromptTemplateReadError\('corrupt', encoded\)/u);
  assert.match(storageSource, /new PromptTemplateReadError\('unavailable'\)/u);
  assert.match(storageSource, /parsePromptTemplates\(value\)/u);
  assert.doesNotMatch(storageSource, /unifiedStorage\.(?:setItem|removeItem)\(storageKey/u);
});

test('prompt template writes and recovery reset await durable storage operations', () => {
  assert.match(storageSource, /await unifiedStorage\.setPersistedItem\(storageKey/u);
  assert.match(storageSource, /await unifiedStorage\.removePersistedItem\(storageKey\)/u);
  assert.match(tabSource, /await writePromptTemplates\(next\)/u);
  assert.match(tabSource, /await resetPromptTemplates\(\)/u);
});

test('the recovery surface offers retry, raw export, and confirmed reset without rendering raw data', () => {
  assert.match(tabSource, /onClick=\{loadTemplates\}/u);
  assert.match(tabSource, /pendingAnchor\.download = 'openchat-prompt-templates-raw\.txt'/u);
  assert.match(tabSource, /title=\{t\('promptTemplatesResetTitle'\)\}/u);
  assert.match(tabSource, /onClick=\{confirmReset\}/u);
  assert.doesNotMatch(tabSource, /\{loadError\.rawValue\}/u);
});

test('all supported locales have prompt template recovery copy', () => {
  for (const localePath of localePaths) {
    const locale = JSON.parse(readSource(localePath));
    for (const key of [
      'promptTemplatesCorruptData',
      'promptTemplatesRetry',
      'promptTemplatesExportRaw',
      'promptTemplatesReset',
      'promptTemplatesResetTitle',
      'promptTemplatesResetConfirmation',
      'promptTemplatesResetConfirm',
      'promptTemplatesExportFailed',
      'promptTemplatesResetFailed',
    ]) {
      assert.equal(typeof locale[key], 'string', `${localePath} is missing ${key}`);
    }
  }
});
