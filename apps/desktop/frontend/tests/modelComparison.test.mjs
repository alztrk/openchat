import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/features/chat/data/modelComparison.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const {
  getComparisonHistoryLoadState,
  comparisonResponseModelLabel,
  hasDistinctModelComparisonResponses,
  canContinueComparisonConversation,
  hasValidComparisonModelSelection,
  selectComparisonModelFromPicker,
  toggleComparisonModelSelection,
} = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('model comparison selection accepts two or three distinct models', () => {
  assert.equal(hasValidComparisonModelSelection(['model-a', 'model-b']), true);
  assert.equal(hasValidComparisonModelSelection(['model-a', 'model-b', 'model-c']), true);
  assert.equal(hasValidComparisonModelSelection(['model-a']), false);
  assert.equal(hasValidComparisonModelSelection(['model-a', 'model-b', 'model-c', 'model-d']), false);
  assert.equal(hasValidComparisonModelSelection(['model-a', 'model-a']), false);
});

test('only assistant siblings from distinct model routes are rendered as a comparison group', () => {
  assert.equal(
    hasDistinctModelComparisonResponses([
      { role: 'assistant', providerId: 'openai', modelId: 'model-a' },
      { role: 'assistant', providerId: 'anthropic', modelId: 'model-b' },
    ]),
    true,
  );
  assert.equal(
    hasDistinctModelComparisonResponses([
      { role: 'assistant', providerId: 'openai', modelId: 'model-a' },
      { role: 'assistant', providerId: 'openai', modelId: 'model-a' },
    ]),
    false,
  );
  assert.equal(
    hasDistinctModelComparisonResponses([
      { role: 'user', providerId: null, modelId: null },
      { role: 'assistant', providerId: 'openai', modelId: 'model-a' },
    ]),
    false,
  );
});

test('comparison response labels use the visible catalog name for an internal model id', () => {
  const response = { providerId: 'sglang', modelId: 'model-uuid' };
  const models = [{ providerId: 'sglang', modelId: 'model-uuid', displayName: 'Qwen 3' }];
  assert.equal(comparisonResponseModelLabel(response, models), 'Qwen 3');
  assert.equal(comparisonResponseModelLabel(response, []), 'model-uuid');
  assert.equal(comparisonResponseModelLabel({ providerId: null, modelId: null }, [], 'Selected model'), 'Selected model');
});

test('model comparison selection toggles entries and caps selection at three', () => {
  assert.deepEqual(toggleComparisonModelSelection(['model-a'], 'model-b'), ['model-a', 'model-b']);
  assert.deepEqual(toggleComparisonModelSelection(['model-a', 'model-b'], 'model-a'), ['model-b']);
  assert.deepEqual(
    toggleComparisonModelSelection(['model-a', 'model-b', 'model-c'], 'model-d'),
    ['model-a', 'model-b', 'model-c'],
  );
});

test('model picker adds another model on the first click and removes a selection on later clicks', () => {
  assert.deepEqual(selectComparisonModelFromPicker(['model-a'], 'model-b'), ['model-a', 'model-b']);
  assert.deepEqual(selectComparisonModelFromPicker(['model-a', 'model-b'], 'model-c'), ['model-a', 'model-b', 'model-c']);
  assert.deepEqual(selectComparisonModelFromPicker(['model-a', 'model-b'], 'model-a'), ['model-b']);
  assert.deepEqual(selectComparisonModelFromPicker(['model-a'], 'model-a'), ['model-a']);
});

test('model comparison waits for every history and fails closed when a load fails', () => {
  const conversationIds = ['conversation-a', 'conversation-b'];
  assert.equal(getComparisonHistoryLoadState(conversationIds, new Set(), new Set()), 'loading');
  assert.equal(
    getComparisonHistoryLoadState(conversationIds, new Set(['conversation-a']), new Set()),
    'loading',
  );
  assert.equal(
    getComparisonHistoryLoadState(conversationIds, new Set(conversationIds), new Set()),
    'ready',
  );
  assert.equal(
    getComparisonHistoryLoadState(conversationIds, new Set(conversationIds), new Set(['conversation-b'])),
    'failed',
  );
  assert.equal(getComparisonHistoryLoadState([], new Set(), new Set()), 'loading');
});

test('a compared conversation can continue only after its response completes', () => {
  const completed = new Set(['conversation-a']);
  assert.equal(canContinueComparisonConversation('active', 'conversation-a', completed), true);
  assert.equal(canContinueComparisonConversation('active', 'conversation-b', completed), false);
  assert.equal(canContinueComparisonConversation('sending', 'conversation-a', completed), false);
});
