import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const searchSource = readFileSync(path.resolve('src/widgets/title-bar/TitleBarSearch.tsx'), 'utf8');

test('the title bar search exposes only a visible popup and its active result to assistive technology', () => {
  assert.match(
    searchSource,
    /const showRecentSearches = isOpen && !query\.trim\(\) && \(recentSearches\.length > 0 \|\| recentSearchError !== null\)/u,
  );
  assert.match(searchSource, /const showSearchResults = isOpen && shouldSearch/u);
  assert.match(searchSource, /const isPopupVisible = showRecentSearches \|\| showSearchResults/u);
  assert.match(searchSource, /aria-expanded=\{isPopupVisible\}/u);
  assert.match(searchSource, /aria-controls=\{[\s\S]*?recentSearchesListboxId[\s\S]*?searchResultsListboxId[\s\S]*?\}/u);
  assert.match(searchSource, /aria-activedescendant=\{activeSearchOptionId \?\? activeRecentSearchOptionId\}/u);
  assert.match(searchSource, /id=\{searchResultsListboxId\}[\s\S]*?role="listbox"/u);
  assert.match(searchSource, /id=\{`\$\{searchResultsListboxId\}-option-\$\{conversationTargetIndex\}`\}/u);
  assert.match(searchSource, /id=\{`\$\{searchResultsListboxId\}-option-\$\{messageTargetIndex\}`\}/u);
  assert.match(searchSource, /showSearchResults && navigableTargets\[selectedIndex\]/u);
});

test('recent searches expose selected listbox options and keep delete buttons outside the listbox', () => {
  const recentListboxStart = searchSource.indexOf('id={recentSearchesListboxId}');
  const recentActionsStart = searchSource.indexOf("t('recentSearchesActions')", recentListboxStart);

  assert.ok(recentListboxStart >= 0);
  assert.ok(recentActionsStart > recentListboxStart);

  const recentListboxSource = searchSource.slice(recentListboxStart, recentActionsStart);
  assert.match(recentListboxSource, /role="listbox"/u);
  assert.match(recentListboxSource, /role="option"/u);
  assert.match(recentListboxSource, /aria-selected=\{selectedRecentSearchIndex === index\}/u);
  assert.doesNotMatch(recentListboxSource, /<Button/u);
  assert.match(searchSource, /aria-label=\{t\('deleteRecentSearch', \{ term \}\)\}/u);
  assert.match(searchSource, /event\.key === 'Escape' && isOpen/u);
  assert.match(searchSource, /if \(index < 0\) return direction > 0 \? 0 : recentSearches\.length - 1/u);
  assert.match(searchSource, /return \(index \+ direction \+ recentSearches\.length\) % recentSearches\.length/u);
  assert.match(searchSource, /handleSelectRecentSearch\(selected\)/u);
});

test('recent-search delete buttons have distinct accessible names in every locale', () => {
  const localePaths = ['de', 'en', 'es', 'fr', 'tr'].map((locale) => `src/shared/lib/i18n/locales/${locale}.json`);

  for (const localePath of localePaths) {
    const locale = JSON.parse(readFileSync(path.resolve(localePath), 'utf8'));
    assert.match(locale.deleteRecentSearch, /\{\{term\}\}/u, `${localePath} must include the search term`);
    assert.equal(typeof locale.recentSearchesActions, 'string', `${localePath} is missing recentSearchesActions`);
  }
});
