import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/features/chat/data/chatStorageRetry.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { chatStorageRetryDelay } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('message and file-change requests retry a restarted sidecar with bounded delays', () => {
  for (const operation of ['messageSave', 'fileChangesList']) {
    const failure = { code: 'service_unavailable', retryable: true, diagnosticCode: null };
    assert.deepEqual(
      [0, 1, 2, 3].map((attempt) => chatStorageRetryDelay(operation, failure, attempt)),
      [250, 750, 1500, null],
    );
  }
});

test('message persistence retries transient initialization and SQLite failures only', () => {
  assert.equal(
    chatStorageRetryDelay(
      'messageSave',
      { code: 'storage_initialization_failed', retryable: true, diagnosticCode: 'sqlite_busy' },
      0,
    ),
    250,
  );
  assert.equal(
    chatStorageRetryDelay(
      'messageSave',
      { code: 'storage_initialization_failed', retryable: false, diagnosticCode: 'sqlite_busy' },
      0,
    ),
    null,
  );
  assert.equal(
    chatStorageRetryDelay(
      'messageSave',
      { code: 'storage_chat_unavailable', retryable: true, diagnosticCode: 'sqlite_corrupt' },
      0,
    ),
    null,
  );
});

test('file-change loading retries typed lock and broker contention but not workspace failures', () => {
  assert.equal(
    chatStorageRetryDelay(
      'fileChangesList',
      { code: 'file_changes_temporarily_busy', retryable: true, diagnosticCode: 'file_change_lock_busy' },
      0,
    ),
    500,
  );
  assert.equal(
    chatStorageRetryDelay(
      'fileChangesList',
      { code: 'file_changes_temporarily_busy', retryable: true, diagnosticCode: 'storage_broker_timeout' },
      0,
    ),
    250,
  );
  assert.equal(
    chatStorageRetryDelay(
      'fileChangesList',
      { code: 'file_changes_workspace_unavailable', retryable: false, diagnosticCode: null },
      0,
    ),
    null,
  );
  assert.equal(
    chatStorageRetryDelay(
      'fileChangesList',
      { code: 'file_changes_unavailable', retryable: true, diagnosticCode: null },
      0,
    ),
    null,
  );
});

test('file-change loading retries transient database initialization failures', () => {
  const failure = {
    code: 'storage_initialization_failed',
    retryable: true,
    diagnosticCode: 'sqlite_busy',
  };
  assert.deepEqual(
    [0, 1, 2].map((attempt) => chatStorageRetryDelay('fileChangesList', failure, attempt)),
    [250, 750, null],
  );
  assert.equal(
    chatStorageRetryDelay(
      'fileChangesList',
      { code: 'database_prepare_failed', retryable: true, diagnosticCode: 'sqlite_corrupt' },
      0,
    ),
    null,
  );
});
