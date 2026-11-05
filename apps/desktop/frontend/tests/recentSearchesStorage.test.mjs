import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const storageSource = readSource('src/widgets/title-bar/recentSearches.ts');
const searchSource = readSource('src/widgets/title-bar/TitleBarSearch.tsx');
const localePaths = [
  'src/shared/lib/i18n/locales/de.json',
  'src/shared/lib/i18n/locales/en.json',
  'src/shared/lib/i18n/locales/es.json',
  'src/shared/lib/i18n/locales/fr.json',
  'src/shared/lib/i18n/locales/tr.json',
];

test('recent searches use app storage and keep the existing key for legacy migration', () => {
  assert.match(storageSource, /unifiedStorage/u);
  assert.match(storageSource, /openchat:recent_searches/u);
  assert.doesNotMatch(storageSource, /localStorage/u);
  assert.doesNotMatch(searchSource, /localStorage/u);
  assert.match(searchSource, /unifiedStorage\.isInitialized[\s\S]*?openchat-storage-ready/u);
});

test('recent searches have explicit count and length limits and await persistence', () => {
  assert.match(storageSource, /maximumCount: 5/u);
  assert.match(storageSource, /maximumLength: 256/u);
  assert.match(storageSource, /await unifiedStorage\.setPersistedItem/u);
  assert.match(storageSource, /await unifiedStorage\.removePersistedItem/u);
  assert.match(searchSource, /maxLength=\{recentSearchLimits\.maximumLength\}/u);
});

test('recent-search storage failures preserve visible state and expose recovery actions', () => {
  assert.match(storageSource, /value\.some\(\(item\) => typeof item !== 'string'\)/u);
  assert.match(searchSource, /catch \{\s*setRecentSearchError\('loadFailed'\)/u);
  assert.match(searchSource, /catch \{\s*setRecentSearchError\(error\)/u);
  assert.match(searchSource, /setRecentSearches\(next\)[\s\S]*?setRecentSearchError\(null\)/u);
  assert.match(searchSource, /if \(recentSearchError === 'loadFailed'\) return Promise\.resolve\(false\)/u);
  assert.match(searchSource, /removeRecentSearch\(recentSearches, term\), 'deleteFailed'/u);
  assert.match(searchSource, /recentSearchesDeleteFailed/u);
  assert.match(searchSource, /onClick=\{reloadRecentSearches\}/u);
  assert.match(searchSource, /onClick=\{handleClearAllRecent\}/u);
});

test('all supported locales explain recent-search storage and recovery failures', () => {
  for (const localePath of localePaths) {
    const locale = JSON.parse(readSource(localePath));
    for (const key of [
      'recentSearchesPrivacyNotice',
      'recentSearchesLoadFailed',
      'recentSearchesSaveFailed',
      'recentSearchesDeleteFailed',
      'recentSearchesClearFailed',
    ]) {
      assert.equal(typeof locale[key], 'string', `${localePath} is missing ${key}`);
    }
  }
});
