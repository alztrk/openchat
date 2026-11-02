import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

function compileTsModule(source) {
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return import(`data:text/javascript;base64,${Buffer.from(output).toString('base64')}`);
}

function extractFunction(source, name) {
  const marker = `function ${name}(`;
  const start = source.indexOf(marker);
  if (start < 0) throw new Error(`Could not find ${name} in production source`);
  const nextFunction = source.indexOf('\nfunction ', start + marker.length);
  return source.slice(start, nextFunction < 0 ? source.length : nextFunction);
}

const contextUsageSource = fs.readFileSync(
  new URL('../src/features/chat/data/contextUsage.ts', import.meta.url),
  'utf8',
);
const productionEstimators = await compileTsModule(
  [
    extractFunction(contextUsageSource, 'estimateContextUsage'),
    extractFunction(contextUsageSource, 'estimateMessages'),
    extractFunction(contextUsageSource, 'estimateAttachment'),
    extractFunction(contextUsageSource, 'estimateText'),
    extractFunction(contextUsageSource, 'stringifyJson'),
    'const opaqueCheckpointEstimate = 4096;',
    'function parseToolActivities(value) { return Array.isArray(value) ? value : []; }',
    'export { estimateContextUsage, estimateMessages, estimateText };',
  ].join('\n\n'),
);
const { estimateContextUsage, estimateMessages, estimateText } = productionEstimators;

test('assistant context estimate uses visible content, not provider completion usage', () => {
  const estimate = estimateMessages([
    {
      id: 'assistant-1',
      role: 'assistant',
      content: 'Harika',
      attachments: [],
      toolActivities: '',
      outputTokens: 900,
    },
  ]);

  assert.equal(estimate.assistantTokens, estimateText('Harika'));
  assert.notEqual(estimate.assistantTokens, 900);
});

test('dense punctuation and non-ASCII text are not treated as UTF-8 byte counts', () => {
  const text = 'hello {"key": 1} 世界';
  assert.notEqual(estimateText(text), new TextEncoder().encode(text).byteLength);
  assert.ok(estimateText(text) > 0);
});

test('provider input baseline adds each post-prompt component exactly once', () => {
  const model = {
    providerId: 'provider',
    modelId: 'model',
    connectionId: null,
    workspaceId: null,
  };
  const promptMessage = {
    id: 'prompt',
    role: 'user',
    content: 'Already counted by provider',
    attachments: [],
    toolActivities: '',
  };
  const postPromptMessage = {
    id: 'new-message',
    role: 'assistant',
    content: 'New response',
    attachments: [{ name: 'notes.txt', kind: 'text', sizeBytes: 16 }],
    toolActivities: [
      {
        status: 'completed',
        callId: 'call-1',
        name: 'read_file',
        arguments: { path: 'notes.txt' },
        output: 'new tool output',
      },
    ],
  };
  const pendingAttachments = [{ name: 'pending.png', kind: 'image', sizeBytes: 0 }];
  const memory = {
    compactionKind: null,
    compactionProviderId: null,
    compactionModelId: null,
    compactionConnectionId: null,
    compactionWorkspaceId: null,
    compactedThroughMessageId: null,
    lastPrompt: {
      providerId: 'provider',
      modelId: 'model',
      connectionId: null,
      workspaceId: null,
      messageId: 'prompt',
      inputTokens: 100,
    },
  };

  const estimate = estimateContextUsage(
    [promptMessage, postPromptMessage],
    'draft',
    pendingAttachments,
    model,
    null,
    memory,
  );
  const postPromptTokens = estimateMessages([postPromptMessage]).totalTokens;
  const pendingImageTokens = 2048;

  assert.equal(estimate.providerInputTokens, 100);
  assert.equal(
    estimate.totalTokens,
    100 + postPromptTokens + pendingImageTokens + estimateText('draft'),
  );
  assert.equal(estimate.attachments.filter((attachment) => attachment.isDraft).length, 1);
});
