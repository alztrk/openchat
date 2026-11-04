import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import { scanImports } from '../../../../scripts/check-imports.mjs';

test('import checker reports forbidden static and type-only re-exports', () => {
  const sourceRoot = path.resolve('checker-fixture', 'src');
  const sourceFile = path.join(sourceRoot, 'features', 'sample', 'index.ts');
  const result = scanImports({
    sourceRoot,
    collectFilesFn: () => [sourceFile],
    readFileFn: () => "export { Button } from '@widgets/button';\nexport type { ButtonProps } from '@widgets/types';",
  });

  assert.equal(result.errors.length, 2);
  assert.deepEqual(result.errors.map((error) => error.importPath), ['@widgets/button', '@widgets/types']);
  assert.equal(result.errors.every((error) => error.message.includes('layer violation')), true);
});

test('import checker fails closed when a source file cannot be read', () => {
  const sourceRoot = path.resolve('checker-fixture', 'src');
  const sourceFile = path.join(sourceRoot, 'shared', 'index.ts');
  const result = scanImports({
    sourceRoot,
    collectFilesFn: () => [sourceFile],
    readFileFn: () => {
      throw new Error('read denied');
    },
  });

  assert.equal(result.errors.length, 1);
  assert.equal(result.errors[0].source, path.join('shared', 'index.ts'));
  assert.equal(result.errors[0].importPath, '<unreadable>');
  assert.match(result.errors[0].message, /could not read source file: read denied/u);
});

test('import checker fails closed when it cannot enumerate the source tree', () => {
  const result = scanImports({
    sourceRoot: path.resolve('checker-fixture', 'src'),
    collectFilesFn: () => {
      throw new Error('directory denied');
    },
  });

  assert.equal(result.errors.length, 1);
  assert.equal(result.errors[0].source, '.');
  assert.equal(result.errors[0].importPath, '<source-tree>');
  assert.match(result.errors[0].message, /could not enumerate source files: directory denied/u);
});

test('import checker does not treat an unrelated alias prefix as a known layer', () => {
  const sourceRoot = path.resolve('checker-fixture', 'src');
  const sourceFile = path.join(sourceRoot, 'features', 'sample', 'index.ts');
  const result = scanImports({
    sourceRoot,
    collectFilesFn: () => [sourceFile],
    readFileFn: () => "import value from '@appx/not-an-app-alias';",
  });

  assert.deepEqual(result.errors, []);
  assert.deepEqual(result.warnings, []);
  assert.deepEqual(result.infos, []);
});
