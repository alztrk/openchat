import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/modules/conversations/stores/modelCatalogProgress.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { loadModelCatalogsProgressively } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

test('publishes ready catalogs while a slower source is pending, then reports full settlement', async () => {
  const chatGpt = deferred();
  const local = deferred();
  const apiKey = deferred();
  const progress = [];
  let reportApiKeyProgress;
  const loading = loadModelCatalogsProgressively(
    {
      chatGpt: chatGpt.promise,
      local: local.promise,
      apiKey: (onProgress) => {
        reportApiKeyProgress = onProgress;
        return apiKey.promise;
      },
    },
    (result) => progress.push(result),
  );

  local.resolve([{ key: 'llama_cpp:test' }]);
  await new Promise(setImmediate);
  assert.equal(progress.length, 1);
  assert.equal(progress[0].local.value[0].key, 'llama_cpp:test');
  assert.equal(progress[0].chatGpt, null);
  assert.equal(progress[0].apiKey, null);
  assert.equal(progress[0].allSettled, false);

  reportApiKeyProgress({ models: [{ key: 'gemini:fast' }], providers: [] });
  await new Promise(setImmediate);
  assert.equal(progress.length, 2);
  assert.equal(progress[1].apiKey.value.models[0].key, 'gemini:fast');
  assert.equal(progress[1].chatGpt, null);
  assert.equal(progress[1].allSettled, false);

  chatGpt.resolve({ models: [], providers: [] });
  await new Promise(setImmediate);
  assert.equal(progress.length, 3);
  assert.equal(progress[2].allSettled, false);

  apiKey.reject(new Error('provider catalog unavailable'));
  await loading;
  assert.equal(progress.length, 4);
  assert.equal(progress[3].apiKey.status, 'rejected');
  assert.equal(progress[3].allSettled, true);
});
