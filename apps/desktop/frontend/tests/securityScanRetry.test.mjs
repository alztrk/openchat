import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/modules/security-scans/data/securityScanRetry.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { retryDelayFromProviderDetails } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('reads a single provider Retry-After value from safe error details', () => {
  assert.equal(retryDelayFromProviderDetails('HTTP status 429; retry-after-seconds=900'), 900_000);
});

test('accepts the provider maximum Retry-After delay', () => {
  assert.equal(retryDelayFromProviderDetails('retry-after-seconds=86400'), 86_400_000);
});

test('ignores missing, malformed, duplicate, zero, or excessive Retry-After values', () => {
  for (const details of [
    null,
    'HTTP status 429',
    'retry-after-seconds=0',
    'retry-after-seconds=86401',
    'retry-after-seconds=30seconds',
    'retry-after-seconds=30; retry-after-seconds=60',
  ]) {
    assert.equal(retryDelayFromProviderDetails(details), null);
  }
});
