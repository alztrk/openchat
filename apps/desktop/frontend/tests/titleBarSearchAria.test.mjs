import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const searchSource = readFileSync(path.resolve('src/widgets/title-bar/TitleBarSearch.tsx'), 'utf8');

test('the title bar search exposes only a visible popup and its active result to assistive technology', () => {
  assert.match(searchSource, /const showRecentSearches = isOpen && !query\.trim\(\) && recentSearches\.length > 0/u);
  assert.match(searchSource, /const showSearchResults = isOpen && shouldSearch/u);
  assert.match(searchSource, /const isPopupVisible = showRecentSearches \|\| showSearchResults/u);
  assert.match(searchSource, /aria-expanded=\{isPopupVisible\}/u);
  assert.match(searchSource, /aria-controls=\{[\s\S]*?recentSearchesListboxId[\s\S]*?searchResultsListboxId[\s\S]*?\}/u);
  assert.match(searchSource, /aria-activedescendant=\{activeSearchOptionId\}/u);
  assert.match(searchSource, /id=\{searchResultsListboxId\}[\s\S]*?role="listbox"/u);
  assert.match(searchSource, /id=\{`\$\{searchResultsListboxId\}-option-\$\{conversationTargetIndex\}`\}/u);
  assert.match(searchSource, /id=\{`\$\{searchResultsListboxId\}-option-\$\{messageTargetIndex\}`\}/u);
  assert.match(searchSource, /showSearchResults && navigableTargets\[selectedIndex\]/u);
});
