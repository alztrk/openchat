import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/modules/conversations/stores/chatStorePostResponse.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { refreshConversationListAfterResponse } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('a conversation-list refresh failure does not fail an already completed assistant response', async () => {
  const databaseReadError = new Error('Conversation list read failed.');
  let updatedConversations = null;
  let reportedError = null;

  const result = await refreshConversationListAfterResponse(
    async () => {
      throw databaseReadError;
    },
    (conversations) => {
      updatedConversations = conversations;
    },
    (error) => {
      reportedError = error;
    },
  );

  assert.equal(result, null);
  assert.equal(updatedConversations, null);
  assert.equal(reportedError, databaseReadError);
});

test('a successful conversation-list refresh remains available for post-response notifications', async () => {
  const conversations = [{ id: 'conversation-1' }];
  let updatedConversations = null;
  let failureReported = false;

  const result = await refreshConversationListAfterResponse(
    async () => conversations,
    (items) => {
      updatedConversations = items;
    },
    () => {
      failureReported = true;
    },
  );

  assert.equal(result, conversations);
  assert.equal(updatedConversations, conversations);
  assert.equal(failureReported, false);
});
