import assert from 'node:assert/strict';
import test from 'node:test';

function estimateText(value) {
  return new TextEncoder().encode(value).byteLength;
}

function estimateMessages(messages) {
  let userTokens = 0;
  let assistantTokens = 0;
  for (const message of messages) {
    if (message.role === 'user') {
      userTokens += estimateText(message.content);
      continue;
    }
    const assistantMessageTokens =
      typeof message.outputTokens === 'number' && Number.isFinite(message.outputTokens) && message.outputTokens >= 0
        ? message.outputTokens
        : estimateText(message.content);
    assistantTokens += assistantMessageTokens;
  }
  return {
    userTokens,
    assistantTokens,
    totalTokens: userTokens + assistantTokens,
  };
}

test('assistant message uses ground-truth outputTokens when reported by model', () => {
  const messages = [
    {
      id: 'msg-1',
      role: 'user',
      content: 'Merhaba',
      attachments: [],
      toolActivities: '',
      outputTokens: null,
    },
    {
      id: 'msg-2',
      role: 'assistant',
      content: 'Harika', // 6 UTF-8 bytes
      attachments: [],
      toolActivities: '',
      outputTokens: 4, // Model reported exactly 4 tokens
    },
  ];

  const estimate = estimateMessages(messages);
  assert.equal(
    estimate.assistantTokens,
    4,
    'Assistant tokens should match message.outputTokens (4), not byte count (6)',
  );
  assert.equal(estimate.userTokens, 7, 'User tokens should use estimateText ("Merhaba" -> 7 bytes)');
});

test('assistant message falls back to text byte estimation when outputTokens is null', () => {
  const messages = [
    {
      id: 'msg-1',
      role: 'assistant',
      content: 'Harika', // 6 UTF-8 bytes
      attachments: [],
      toolActivities: '',
      outputTokens: null,
    },
  ];

  const estimate = estimateMessages(messages);
  assert.equal(
    estimate.assistantTokens,
    6,
    'Assistant tokens should fall back to byte count when outputTokens is null',
  );
});
