import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/features/design-workspace/data/designRevisionComparison.ts'), 'utf8')
  .replace("import type { DesignDocument } from './designDocument';", '');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { compareDesignRevisions } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

function documentWith(nodes) {
  return { nodes };
}

test('revision comparison counts added, removed, and modified nodes by stable ID', () => {
  const current = documentWith([
    { id: 'same', name: 'Changed', x: 12, y: 0 },
    { id: 'added', name: 'New', x: 0, y: 0 },
  ]);
  const previous = documentWith([
    { id: 'same', name: 'Original', x: 0, y: 0 },
    { id: 'removed', name: 'Old', x: 0, y: 0 },
  ]);

  assert.deepEqual(compareDesignRevisions(current, previous), {
    addedNodes: 1,
    removedNodes: 1,
    changedNodes: 1,
  });
});

test('revision comparison reports an unchanged document without differences', () => {
  const current = documentWith([{ id: 'same', name: 'Frame', x: 0, y: 0 }]);

  assert.deepEqual(compareDesignRevisions(current, current), {
    addedNodes: 0,
    removedNodes: 0,
    changedNodes: 0,
  });
});
