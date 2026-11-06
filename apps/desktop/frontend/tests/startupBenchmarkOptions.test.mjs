import assert from 'node:assert/strict';
import test from 'node:test';

import { parseStartupBenchmarkArgs } from '../../../../scripts/tasks/dev.mjs';

test('startup benchmark defaults to the Vite development server', () => {
  assert.deepEqual(parseStartupBenchmarkArgs([]), { productionFrontend: false });
});

test('startup benchmark accepts the production frontend option', () => {
  assert.deepEqual(parseStartupBenchmarkArgs(['--production-frontend']), { productionFrontend: true });
});

test('startup benchmark rejects unknown or repeated options', () => {
  assert.throws(() => parseStartupBenchmarkArgs(['--release']), /--production-frontend/u);
  assert.throws(
    () => parseStartupBenchmarkArgs(['--production-frontend', '--production-frontend']),
    /--production-frontend/u,
  );
});
