import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import {
  hostLocalServiceArtifactPath,
  localServiceBuildArgs,
} from '../../../../scripts/shared/rust.mjs';

test('host sidecar builds use Cargo default target directories', () => {
  assert.deepEqual(localServiceBuildArgs('debug', true), [
    'build',
    '-p',
    'openchat-local-service',
    '--bin',
    'openchat-local-service',
    '--locked',
  ]);
  assert.deepEqual(localServiceBuildArgs('release', false, ['desktop-profile-test-support']), [
    'build',
    '-p',
    'openchat-local-service',
    '--bin',
    'openchat-local-service',
    '--release',
    '--features',
    'desktop-profile-test-support',
  ]);
});

test('host sidecar artifacts resolve from Cargo profile directories', () => {
  assert.equal(
    hostLocalServiceArtifactPath('workspace', 'debug', 'x86_64-pc-windows-msvc'),
    path.join('workspace', 'target', 'debug', 'openchat-local-service.exe'),
  );
  assert.equal(
    hostLocalServiceArtifactPath('workspace', 'release', 'x86_64-unknown-linux-gnu'),
    path.join('workspace', 'target', 'release', 'openchat-local-service'),
  );
});

test('host sidecar build arguments reject unsupported profiles', () => {
  assert.throws(() => localServiceBuildArgs('custom', true), /Invalid build profile/u);
});
