import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/modules/conversations/stores/modelCatalogRefreshGuard.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { createModelCatalogRefreshGuard } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('a delayed model refresh cannot replace a newer catalog result', () => {
  const guard = createModelCatalogRefreshGuard();
  const olderRefresh = guard.begin();
  const newerRefresh = guard.begin();

  assert.equal(guard.isCurrent(olderRefresh), false);
  assert.equal(guard.isCurrent(newerRefresh), true);
});
