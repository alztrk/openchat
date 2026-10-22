import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const source = fs.readFileSync(path.resolve('src/modules/providers/data/modelCatalogLoader.ts'), 'utf8');

test('Copilot catalog RPC waits longer than the service catalog HTTP request budget', () => {
  assert.match(source, /const copilotModelCatalogTimeoutMs = 95_000;/);
  assert.match(
    source,
    /client\.call\(\s*'compatible\.models\.list'[\s\S]*?copilotModelCatalogTimeoutMs\s*,\s*\)/,
  );
});
