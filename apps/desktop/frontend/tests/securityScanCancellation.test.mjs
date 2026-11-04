import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/features/security-scans/data/securityScanCancellation.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { cancelSecurityScanOperationAndWait } = await import(
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

test('cancellation waits for accepted operation settlement before deletion can continue', async () => {
  const result = deferred();
  const wait = cancelSecurityScanOperationAndWait(
    { cancel: async () => true, result: result.promise },
    100,
  );
  await new Promise((resolve) => setTimeout(resolve, 1));
  result.resolve({ status: 'stopped' });

  assert.deepEqual(await wait, { settled: true, cancellationAccepted: true });
});

test('a rejected cancellation can finish naturally within the shared deadline', async () => {
  const result = deferred();
  const wait = cancelSecurityScanOperationAndWait(
    { cancel: async () => false, result: result.promise },
    100,
  );
  await new Promise((resolve) => setTimeout(resolve, 1));
  result.resolve({ status: 'completed' });

  assert.deepEqual(await wait, { settled: true, cancellationAccepted: false });
});

test('a rejected cancellation with an unsettled operation returns after the deadline', async () => {
  const result = deferred();
  const startedAt = Date.now();
  const outcome = await cancelSecurityScanOperationAndWait(
    { cancel: async () => false, result: result.promise },
    10,
  );

  assert.deepEqual(outcome, { settled: false, cancellationAccepted: false });
  assert.ok(Date.now() - startedAt < 500);
});

test('a stalled cancellation request cannot keep deletion pending indefinitely', async () => {
  const result = deferred();
  const cancellation = deferred();
  const outcome = await cancelSecurityScanOperationAndWait(
    { cancel: () => cancellation.promise, result: result.promise },
    10,
  );

  assert.deepEqual(outcome, { settled: false, cancellationAccepted: null });
  cancellation.resolve(true);
});

test('a failed operation is settled and safe to clean up', async () => {
  const result = deferred();
  const wait = cancelSecurityScanOperationAndWait(
    { cancel: async () => false, result: result.promise },
    100,
  );
  result.reject(new Error('operation failed'));

  assert.deepEqual(await wait, { settled: true, cancellationAccepted: null });
});
