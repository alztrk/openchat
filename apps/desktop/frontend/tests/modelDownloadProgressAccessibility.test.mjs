import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const componentSource = readSource('src/features/model-catalog/components/ModelCatalogTab.tsx');
const localePaths = ['de', 'en', 'es', 'fr', 'tr'].map((locale) => `src/shared/lib/i18n/locales/${locale}.json`);

test('model download progress has a progress-specific accessible name and localized value text', () => {
  const statusStart = componentSource.indexOf('function ModelDownloadStatus(');
  assert.ok(statusStart >= 0);
  const statusSource = componentSource.slice(statusStart);

  assert.equal((statusSource.match(/aria-label=\{t\('modelDownloadProgressLabel'\)\}/gu) ?? []).length, 2);
  assert.match(statusSource, /aria-valuetext=\{[\s\S]*?t\('modelDownloadProgressValue'/u);
  assert.match(statusSource, /downloaded: formatBytes\(progress\.downloadedBytes, locale\)/u);
  assert.match(statusSource, /total: formatBytes\(progress\.totalBytes, locale\)/u);
  assert.match(statusSource, /progress && progress\.totalBytes > 0[\s\S]*?: undefined/u);
  assert.match(statusSource, /value=\{fraction \?\? undefined\}/u);
  assert.doesNotMatch(statusSource, /aria-label=\{t\('modelDownloadOptionsLabel'\)\}/u);
});

test('all supported locales describe both model download progress and its byte range', () => {
  for (const localePath of localePaths) {
    const locale = JSON.parse(readSource(localePath));
    assert.equal(typeof locale.modelDownloadProgressLabel, 'string', `${localePath} is missing the progress label`);
    assert.match(locale.modelDownloadProgressValue, /\{downloaded\}/u, `${localePath} is missing downloaded bytes`);
    assert.match(locale.modelDownloadProgressValue, /\{total\}/u, `${localePath} is missing total bytes`);
  }
});
