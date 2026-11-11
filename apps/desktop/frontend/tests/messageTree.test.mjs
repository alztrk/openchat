import assert from 'node:assert/strict';
import test from 'node:test';

import {
  findLeafForMessage,
  getActiveBranchMessages,
  getMessageSiblings,
} from '../src/features/chat/data/messageTree.ts';

test('getActiveBranchMessages returns all messages for linear conversation without branches', () => {
  const messages = [
    { id: 'u1', role: 'user', createdAt: 1000, parentMessageId: null },
    { id: 'a1', role: 'assistant', createdAt: 2000, parentMessageId: 'u1' },
    { id: 'u2', role: 'user', createdAt: 3000, parentMessageId: 'a1' },
    { id: 'a2', role: 'assistant', createdAt: 4000, parentMessageId: 'u2' },
  ];

  const branch = getActiveBranchMessages(messages, 'a2');
  assert.equal(branch.length, 4);
  assert.deepEqual(branch.map((m) => m.id), ['u1', 'a1', 'u2', 'a2']);
});

test('getActiveBranchMessages follows active branch when user message is branched', () => {
  const messages = [
    { id: 'u1', role: 'user', createdAt: 1000, parentMessageId: null },
    { id: 'a1', role: 'assistant', createdAt: 2000, parentMessageId: 'u1' },
    // First branch
    { id: 'u2_v1', role: 'user', createdAt: 3000, parentMessageId: 'a1' },
    { id: 'a2_v1', role: 'assistant', createdAt: 4000, parentMessageId: 'u2_v1' },
    // Second branch
    { id: 'u2_v2', role: 'user', createdAt: 5000, parentMessageId: 'a1' },
    { id: 'a2_v2', role: 'assistant', createdAt: 6000, parentMessageId: 'u2_v2' },
  ];

  // Active leaf is a2_v2 (second branch)
  const branchV2 = getActiveBranchMessages(messages, 'a2_v2');
  assert.deepEqual(branchV2.map((m) => m.id), ['u1', 'a1', 'u2_v2', 'a2_v2']);

  // Active leaf is a2_v1 (first branch)
  const branchV1 = getActiveBranchMessages(messages, 'a2_v1');
  assert.deepEqual(branchV1.map((m) => m.id), ['u1', 'a1', 'u2_v1', 'a2_v1']);
});

test('getMessageSiblings correctly identifies siblings and index for branched messages', () => {
  const messages = [
    { id: 'u1', role: 'user', createdAt: 1000, parentMessageId: null },
    { id: 'a1', role: 'assistant', createdAt: 2000, parentMessageId: 'u1' },
    { id: 'u2_v1', role: 'user', createdAt: 3000, parentMessageId: 'a1' },
    { id: 'u2_v2', role: 'user', createdAt: 5000, parentMessageId: 'a1' },
    { id: 'u2_v3', role: 'user', createdAt: 7000, parentMessageId: 'a1' },
  ];

  const infoV1 = getMessageSiblings(messages, 'u2_v1');
  assert.equal(infoV1.count, 3);
  assert.equal(infoV1.index, 0);

  const infoV2 = getMessageSiblings(messages, 'u2_v2');
  assert.equal(infoV2.count, 3);
  assert.equal(infoV2.index, 1);

  const infoV3 = getMessageSiblings(messages, 'u2_v3');
  assert.equal(infoV3.count, 3);
  assert.equal(infoV3.index, 2);

  const infoRoot = getMessageSiblings(messages, 'u1');
  assert.equal(infoRoot.count, 1);
  assert.equal(infoRoot.index, 0);
});

test('getMessageSiblings distinguishes assistant retry versions sharing the same parent user message', () => {
  const messages = [
    { id: 'u1', role: 'user', createdAt: 1000, parentMessageId: null },
    { id: 'a1_v1', role: 'assistant', createdAt: 2000, parentMessageId: 'u1' },
    { id: 'a1_v2', role: 'assistant', createdAt: 3000, parentMessageId: 'u1' },
  ];

  const infoV1 = getMessageSiblings(messages, 'a1_v1');
  assert.equal(infoV1.count, 2);
  assert.equal(infoV1.index, 0);

  const infoV2 = getMessageSiblings(messages, 'a1_v2');
  assert.equal(infoV2.count, 2);
  assert.equal(infoV2.index, 1);
});

test('findLeafForMessage navigates to the deepest and newest child along the branch', () => {
  const messages = [
    { id: 'u1', role: 'user', createdAt: 1000, parentMessageId: null },
    { id: 'a1', role: 'assistant', createdAt: 2000, parentMessageId: 'u1' },
    // Branch 1
    { id: 'u2_v1', role: 'user', createdAt: 3000, parentMessageId: 'a1' },
    { id: 'a2_v1', role: 'assistant', createdAt: 4000, parentMessageId: 'u2_v1' },
    { id: 'u3_v1', role: 'user', createdAt: 5000, parentMessageId: 'a2_v1' },
    // Branch 2
    { id: 'u2_v2', role: 'user', createdAt: 6000, parentMessageId: 'a1' },
    { id: 'a2_v2', role: 'assistant', createdAt: 7000, parentMessageId: 'u2_v2' },
  ];

  // From u2_v1, leaf should be u3_v1
  assert.equal(findLeafForMessage(messages, 'u2_v1'), 'u3_v1');

  // From u2_v2, leaf should be a2_v2
  assert.equal(findLeafForMessage(messages, 'u2_v2'), 'a2_v2');

  // Terminal node without children returns itself
  assert.equal(findLeafForMessage(messages, 'a2_v2'), 'a2_v2');
});

test('getActiveBranchMessages fallback gracefully when activeLeafMessageId is null or unknown', () => {
  const messages = [
    { id: 'u1', role: 'user', createdAt: 1000, parentMessageId: null },
    { id: 'a1', role: 'assistant', createdAt: 2000, parentMessageId: 'u1' },
  ];

  assert.deepEqual(getActiveBranchMessages([], null), []);
  assert.deepEqual(
    getActiveBranchMessages(messages, 'unknown').map((m) => m.id),
    ['u1', 'a1'],
  );
  assert.deepEqual(
    getActiveBranchMessages(messages, null).map((m) => m.id),
    ['u1', 'a1'],
  );
});
