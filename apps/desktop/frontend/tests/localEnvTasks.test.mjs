import assert from 'node:assert/strict';
import test from 'node:test';

import { ensureOpenSSLEnvironment, taskLoadsLocalEnv } from '../../../../scripts/shared/env.mjs';

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

test('non-Windows tasks ignore inherited Windows OpenSSL directories but preserve native paths', () => {
  const environment = {
    OPENSSL_DIR: 'D:\\tools\\openssl',
    OPENSSL_NO_VENDOR: '1',
  };

  ensureOpenSSLEnvironment({ environment, platform: 'linux' });

  assert.equal(environment.OPENSSL_DIR, undefined);
  assert.equal(environment.OPENSSL_NO_VENDOR, '1');

  const nativeEnvironment = { OPENSSL_DIR: '/usr' };
  ensureOpenSSLEnvironment({ environment: nativeEnvironment, platform: 'linux' });
  assert.equal(nativeEnvironment.OPENSSL_DIR, '/usr');
});
