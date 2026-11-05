import assert from 'node:assert/strict';
import test from 'node:test';

import { taskLoadsLocalEnv } from '../../../../scripts/shared/env.mjs';

test('only development and release build tasks load the local environment file', () => {
  assert.equal(taskLoadsLocalEnv('dev'), true);
  assert.equal(taskLoadsLocalEnv('build'), true);

  for (const taskName of [
    'dev-test-profile',
    'startup-benchmark',
    'check',
    'check-types',
    'clean',
    'maintain-target',
    '--help',
    undefined,
  ]) {
    assert.equal(taskLoadsLocalEnv(taskName), false, `${String(taskName)} must not load .env`);
  }
});
