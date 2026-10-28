import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/modules/providers/data/connectionRemoval.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { isRetryableConnectionRemovalFailure, waitForConnectionRemoval } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('connection removal treats retryable local-service errors as an uncertain outcome', () => {
  const error = new Error('The local service stopped unexpectedly.');
  error.name = 'LocalServiceError';
  error.retryable = true;
  assert.equal(isRetryableConnectionRemovalFailure(error), true);
  assert.equal(isRetryableConnectionRemovalFailure(new Error('The provider rejected the request.')), false);
});

test('connection removal reconciles a failed RPC while its database deletion finishes', async () => {
  let checks = 0;
  await waitForConnectionRemoval(
    'connection-1',
    Promise.reject(new Error('RPC response was interrupted')),
    async () => {
      checks += 1;
      return checks < 3;
    },
    {
      intervalMs: 1,
      failedOperationReconciliationMs: 30,
      onCheckFailure: () => assert.fail('The connection state query should succeed.'),
    },
  );
  assert.equal(checks, 3);
});

test('connection removal reports the original RPC failure when the row remains', async () => {
  const rpcError = new Error('RPC failed');
  await assert.rejects(
    waitForConnectionRemoval('connection-1', Promise.reject(rpcError), async () => true, {
      intervalMs: 1,
      failedOperationReconciliationMs: 5,
      onCheckFailure: () => assert.fail('The connection state query should succeed.'),
    }),
    (error) => error === rpcError,
  );
});

test('connection removal reconciles when the transport fails before returning an operation handle', async () => {
  let checks = 0;
  const operationResult = Promise.reject(new Error('The write acknowledgement was lost'));
  await waitForConnectionRemoval(
    'connection-1',
    operationResult,
    async () => {
      checks += 1;
      return checks === 1;
    },
    {
      intervalMs: 1,
      failedOperationReconciliationMs: 30,
      onCheckFailure: () => assert.fail('The connection state query should succeed.'),
    },
  );
  assert.equal(checks, 2);
});
