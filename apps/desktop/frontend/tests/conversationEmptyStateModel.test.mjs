import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const readSource = (filePath) => readFileSync(path.resolve(filePath), 'utf8');
const paneSource = readSource('src/features/chat/components/ConversationPane.tsx');
const emptyStateSource = readSource('src/features/chat/components/ConversationEmptyState.tsx');

test('the chat empty state distinguishes an absent model from a selected model', () => {
  assert.match(paneSource, /selectedModel=\{selectedModel !== null\}/u);
  assert.doesNotMatch(paneSource, /selectedModel=\{selectedModel !== undefined\}/u);
  assert.match(emptyStateSource, /selectedModel\s*\?\s*t\('emptyChatWelcomeTitle'\)\s*:\s*t\('emptyChatSelectModelTitle'\)/u);
  assert.match(emptyStateSource, /selectedModel\s*\?\s*t\('emptyChatWelcomeBody'\)\s*:\s*t\('emptyChatSelectModelBody'\)/u);
});

test('the chat pane represents both an empty model catalog and an explicit selection', () => {
  assert.match(paneSource, /return modelByKey\.get\(selectedModelKey \?\? ''\) \?\? null/u);
  assert.match(paneSource, /hasAvailableModels=\{visibleModels\.length > 0\}/u);
  assert.match(emptyStateSource, /!hasAvailableModels/u);
});
