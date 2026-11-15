import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/features/chat/state/chatStoreLocalModels.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { mergeRefreshedLocalModels, shouldRefreshLocalModelsAfterChatStart } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

const isLocalEngine = (providerId) => ['llama_cpp', 'vllm', 'exllama', 'sglang'].includes(providerId);

test('a local chat start requests an updated local model catalog only for local engines', () => {
  assert.equal(shouldRefreshLocalModelsAfterChatStart('chat.started', 'vllm', isLocalEngine), true);
  assert.equal(shouldRefreshLocalModelsAfterChatStart('chat.started', 'chatgpt', isLocalEngine), false);
  assert.equal(shouldRefreshLocalModelsAfterChatStart('chat.delta', 'vllm', isLocalEngine), false);
});

test('refreshed local limits replace cached local models and retain remote providers', () => {
  const remoteModel = { key: 'chatgpt:gpt-model', providerId: 'chatgpt' };
  const staleLocalModel = { key: 'vllm:model-1', providerId: 'vllm', contextWindow: null };
  const refreshedLocalModel = { key: 'vllm:model-1', providerId: 'vllm', contextWindow: 32_768 };

  assert.deepEqual(
    mergeRefreshedLocalModels([remoteModel, staleLocalModel], [refreshedLocalModel], isLocalEngine),
    [remoteModel, refreshedLocalModel],
  );
});
