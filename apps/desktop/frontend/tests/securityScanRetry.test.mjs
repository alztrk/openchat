import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/features/security-scans/data/securityScanRetry.ts');
const source = fs.readFileSync(sourcePath, 'utf8').replace(
  "import { LocalServiceError } from '@shared/lib/platform/local-service';",
  `export class LocalServiceError extends Error {
    constructor(retryAfterMs) {
      super('Provider request failed.');
      this.retryAfterMs = retryAfterMs;
    }
  }`,
);
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { LocalServiceError, retryDelayFromServiceError } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('returns the bounded retry delay from a local service error', () => {
  assert.equal(retryDelayFromServiceError(new LocalServiceError(900_000)), 900_000);
});

test('returns the maximum retry delay supported by a local service error', () => {
  assert.equal(retryDelayFromServiceError(new LocalServiceError(86_400_000)), 86_400_000);
});

test('returns null for unrelated and non-retryable errors', () => {
  assert.equal(retryDelayFromServiceError(new Error('Provider request failed.')), null);
  assert.equal(retryDelayFromServiceError(new LocalServiceError(null)), null);
});
