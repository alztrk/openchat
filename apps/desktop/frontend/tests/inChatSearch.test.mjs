import assert from 'node:assert/strict';
import test from 'node:test';

import {
  findInChatMatches,
  isCodeBlockMessage,
  isErrorMessage,
} from '../src/modules/conversations/data/inChatSearch.ts';

function createMessage(overrides = {}) {
  return {
    id: 'msg-1',
    conversationId: 'conv-1',
    role: 'user',
    content: 'Hello world',
    attachments: [],
    createdAt: Date.now(),
    outputTokens: null,
    tokensPerSecond: null,
    elapsedMicroseconds: null,
    providerId: null,
    modelId: null,
    citationSources: '',
    personalMemoryFactIds: '',
    reasoningSummaries: '',
    toolActivities: '',
    status: 'complete',
    failureCode: null,
    ...overrides,
  };
}

test('findInChatMatches returns empty array when query is empty and filter is all', () => {
  const messages = [createMessage({ content: 'test message' })];
  const matches = findInChatMatches(messages, '', 'all');
  assert.equal(matches.length, 0);
});

test('findInChatMatches finds matches case-insensitively across messages', () => {
  const messages = [
    createMessage({ id: 'msg-1', content: 'React hooks are great' }),
    createMessage({ id: 'msg-2', role: 'assistant', content: 'Indeed, REACT provides useState and useEffect' }),
    createMessage({ id: 'msg-3', role: 'user', content: 'Unrelated topic' }),
  ];

  const matches = findInChatMatches(messages, 'react', 'all');
  assert.equal(matches.length, 2);
  assert.equal(matches[0]?.messageId, 'msg-1');
  assert.equal(matches[0]?.start, 0);
  assert.equal(matches[0]?.end, 5);
  assert.equal(matches[0]?.matchedText, 'React');

  assert.equal(matches[1]?.messageId, 'msg-2');
  assert.equal(matches[1]?.matchedText, 'REACT');
});

test('findInChatMatches finds multiple occurrences within a single message', () => {
  const messages = [createMessage({ id: 'msg-1', content: 'The test results showed test passed on the first test.' })];

  const matches = findInChatMatches(messages, 'test', 'all');
  assert.equal(matches.length, 3);
  assert.equal(matches[0]?.occurrenceIndexInMessage, 0);
  assert.equal(matches[1]?.occurrenceIndexInMessage, 1);
  assert.equal(matches[2]?.occurrenceIndexInMessage, 2);
  assert.equal(matches[0]?.matchIndex, 0);
  assert.equal(matches[1]?.matchIndex, 1);
  assert.equal(matches[2]?.matchIndex, 2);
});

test('filter user prompts matches only user messages', () => {
  const messages = [
    createMessage({ id: 'msg-1', role: 'user', content: 'How do I center a div?' }),
    createMessage({ id: 'msg-2', role: 'assistant', content: 'You can use flexbox to center a div.' }),
    createMessage({ id: 'msg-3', role: 'user', content: 'Thanks, what about grid?' }),
  ];

  const matches = findInChatMatches(messages, 'div', 'user');
  assert.equal(matches.length, 1);
  assert.equal(matches[0]?.messageId, 'msg-1');

  // Cycling with empty query
  const emptyFilterMatches = findInChatMatches(messages, '', 'user');
  assert.equal(emptyFilterMatches.length, 2);
  assert.equal(emptyFilterMatches[0]?.messageId, 'msg-1');
  assert.equal(emptyFilterMatches[1]?.messageId, 'msg-3');
});

test('filter code blocks identifies messages with markdown fences and tool activities', () => {
  const codeMsg = createMessage({
    id: 'msg-code',
    role: 'assistant',
    content: 'Here is some code:\n```ts\nconst x = 1;\n```',
  });
  const plainMsg = createMessage({
    id: 'msg-plain',
    role: 'assistant',
    content: 'Just plain text without code.',
  });
  const toolMsg = createMessage({
    id: 'msg-tool',
    role: 'assistant',
    content: 'Ran command',
    toolActivities: '[{"tool":"run_command"}]',
  });

  assert.equal(isCodeBlockMessage(codeMsg), true);
  assert.equal(isCodeBlockMessage(plainMsg), false);
  assert.equal(isCodeBlockMessage(toolMsg), true);

  const messages = [codeMsg, plainMsg, toolMsg];
  const matches = findInChatMatches(messages, '', 'code');
  assert.equal(matches.length, 2);
  assert.equal(matches[0]?.messageId, 'msg-code');
  assert.equal(matches[1]?.messageId, 'msg-tool');
});

test('filter errors / blockers identifies status error, failureCode, and error keywords', () => {
  const statusError = createMessage({
    id: 'msg-err-status',
    role: 'assistant',
    content: 'Failed to generate',
    status: 'failed',
  });
  const failureCodeMsg = createMessage({
    id: 'msg-err-code',
    role: 'assistant',
    content: 'Network issue',
    failureCode: 'providerTimeout',
  });
  const keywordMsg = createMessage({
    id: 'msg-err-kw',
    role: 'assistant',
    content: 'Found a fatal exception in the database driver.',
  });
  const normalMsg = createMessage({
    id: 'msg-normal',
    role: 'assistant',
    content: 'Everything is running smoothly.',
  });

  assert.equal(isErrorMessage(statusError), true);
  assert.equal(isErrorMessage(failureCodeMsg), true);
  assert.equal(isErrorMessage(keywordMsg), true);
  assert.equal(isErrorMessage(normalMsg), false);

  const messages = [statusError, failureCodeMsg, keywordMsg, normalMsg];
  const matches = findInChatMatches(messages, '', 'error');
  assert.equal(matches.length, 3);
});
