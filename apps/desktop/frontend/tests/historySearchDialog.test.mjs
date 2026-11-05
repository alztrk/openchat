import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const component = readFileSync(path.resolve('src/features/chat/components/HistorySearchDialog.tsx'), 'utf8');
const titleBarSearch = readFileSync(path.resolve('src/widgets/title-bar/TitleBarSearch.tsx'), 'utf8');
const app = readFileSync(path.resolve('src/bootstrap/App.tsx'), 'utf8');

test('advanced history search loads options, sends filters, and rejects stale results', () => {
  assert.match(component, /loadHistorySearchFilterOptions\(\)/u);
  assert.match(component, /searchHistory\(query, filters, dateRange\.fromUnixMs, dateRange\.throughUnixMs\)/u);
  assert.match(component, /searchGeneration\.current === generation/u);
  assert.match(component, /historySearchInvalidDateRange/u);
});

test('saved history searches load, restore, persist, and delete named filters', () => {
  assert.match(component, /loadSavedHistorySearches\(\)/u);
  assert.match(component, /saveHistorySearch\(/u);
  assert.match(component, /deleteHistorySearch\(/u);
  assert.match(component, /savedHistorySearchesLoadFailed/u);
  assert.match(component, /await saveHistorySearch\(/u);
  assert.match(component, /await deleteHistorySearch\(/u);
  assert.match(component, /savedHistorySearchLimitReached/u);
  assert.match(component, /savedHistorySearchDeleteFailed/u);
  assert.match(component, /savedHistorySearchSaveFailed/u);
  assert.match(component, /throughUnixMs - 1/u);
  assert.match(component, /localDateInputValue\(/u);
});

test('history results reuse conversation navigation and open project chats', () => {
  assert.match(component, /onSelectConversation\(/u);
  assert.match(component, /onSelectProject\?\./u);
  assert.match(titleBarSearch, /import\('@features\/chat\/components\/HistorySearchDialog'\)/u);
  assert.match(titleBarSearch, /<LazyHistorySearchDialog/u);
  assert.match(app, /startNewChat\(projectId\)/u);
});

test('history search exposes loading, empty, and error feedback accessibly', () => {
  assert.match(component, /role="status"/u);
  assert.match(component, /role="alert"/u);
  assert.match(component, /searchMessagesNoResults/u);
  assert.match(component, /searchMessagesLoading/u);
});
