import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';

import {
  assertCleanReleaseTree,
  parseBuildChannel,
  resolveReleaseIdentity,
} from '../../../../scripts/shared/release-identity.mjs';
import {
  readApplicationBaseVersion,
  resolveWindowsSigningConfig,
} from '../../../../scripts/tasks/build.mjs';
import { getTauriDirectory, getWorkspaceRoot } from '../../../../scripts/shared/runner.mjs';

test('release build requires exactly one explicit channel', () => {
  assert.equal(parseBuildChannel(['--preview']), 'preview');
  assert.equal(parseBuildChannel(['--stable']), 'stable');
  assert.throws(() => parseBuildChannel([]), /Choose exactly one release channel/u);
  assert.throws(() => parseBuildChannel(['--preview', '--stable']), /Choose exactly one release channel/u);
});

test('preview package version includes both the commit number and SHA', () => {
  const first = resolveReleaseIdentity({
    channel: 'preview',
    baseVersion: '0.1.11',
    commitCount: 241,
    commitSha: 'abcdef012345',
    tag: null,
  });
  const second = resolveReleaseIdentity({
    channel: 'preview',
    baseVersion: '0.1.11',
    commitCount: 242,
    commitSha: 'fedcba987654',
    tag: null,
  });

  assert.equal(first.version, '0.1.11-preview.241.gabcdef012345');
  assert.notEqual(first.version, second.version);
});

test('stable package uses the configured SemVer and requires its exact tag', () => {
  const identity = resolveReleaseIdentity({
    channel: 'stable',
    baseVersion: '1.2.0',
    commitCount: 300,
    commitSha: 'abcdef012345',
    tag: 'v1.2.0',
  });

  assert.equal(identity.version, '1.2.0+300.gabcdef012345');
  assert.throws(
    () => resolveReleaseIdentity({ channel: 'stable', baseVersion: '1.2.0', commitCount: 300, commitSha: 'abcdef012345', tag: null }),
    /exact Git tag v1\.2\.0/u,
  );
});

test('release identity rejects malformed commit and base version values', () => {
  assert.throws(
    () => resolveReleaseIdentity({ channel: 'preview', baseVersion: '1.2', commitCount: 3, commitSha: 'abcdef012345', tag: null }),
    /MAJOR\.MINOR\.PATCH/u,
  );
  assert.throws(
    () => resolveReleaseIdentity({ channel: 'preview', baseVersion: '1.2.3', commitCount: 0, commitSha: 'abcdef012345', tag: null }),
    /valid commit number and commit SHA/u,
  );
});

test('release packages reject a dirty working tree', () => {
  assert.doesNotThrow(() => assertCleanReleaseTree(''));
  assert.throws(() => assertCleanReleaseTree(' M README.md'), /clean Git working tree/u);
});

test('application version is synchronized across Tauri, Rust, npm, and both lockfiles', () => {
  assert.equal(readApplicationBaseVersion(getWorkspaceRoot(), getTauriDirectory()), '0.1.11');
});

test('stable Windows packages require a valid imported Authenticode certificate', () => {
  assert.throws(
    () => resolveWindowsSigningConfig('stable', 'win32', undefined),
    /require an imported Authenticode signing certificate/u,
  );
  assert.throws(
    () => resolveWindowsSigningConfig('stable', 'win32', 'not-a-thumbprint'),
    /require an imported Authenticode signing certificate/u,
  );

  assert.deepEqual(
    resolveWindowsSigningConfig('stable', 'win32', 'a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2'),
    {
      bundle: {
        windows: {
          certificateThumbprint: 'A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2',
          digestAlgorithm: 'sha256',
          timestampUrl: 'http://timestamp.digicert.com',
          tsp: true,
        },
      },
    },
  );
});

test('preview and non-Windows packages do not require Authenticode credentials', () => {
  assert.equal(resolveWindowsSigningConfig('preview', 'win32', undefined), null);
  assert.equal(resolveWindowsSigningConfig('stable', 'linux', undefined), null);
});

test('stable Windows release CI imports, verifies, and removes its Authenticode certificate', () => {
  const workflow = readFileSync(
    new URL('../../../../.github/workflows/desktop-ci.yml', import.meta.url),
    'utf8',
  ).replaceAll('\r\n', '\n');
  const windowsJob = workflow.split('\n  windows-installer:\n')[1]?.split(/\n {2}[a-z][a-z0-9-]*:\n/u)[0];
  assert.ok(windowsJob);

  const importStep = windowsJob.indexOf('Import Windows signing certificate');
  const buildStep = windowsJob.indexOf('Build the Windows installer');
  const verifyStep = windowsJob.indexOf('Verify signed Windows application and installer');
  const cleanupStep = windowsJob.indexOf('Remove Windows signing certificate from runner');

  assert.ok(importStep >= 0 && importStep < buildStep);
  assert.ok(buildStep < verifyStep && verifyStep < cleanupStep);
  assert.match(windowsJob, /if: startsWith\(github\.ref, 'refs\/tags\/v'\)[\s\S]*?WINDOWS_CERTIFICATE: \$\{\{ secrets\.WINDOWS_CERTIFICATE \}\}/u);
  assert.match(windowsJob, /WINDOWS_CERTIFICATE_PASSWORD: \$\{\{ secrets\.WINDOWS_CERTIFICATE_PASSWORD \}\}/u);
  assert.match(windowsJob, /OPENCHAT_WINDOWS_SIGNING_THUMBPRINT: \$\{\{ steps\.windows-signing-certificate\.outputs\.thumbprint \}\}/u);
  assert.match(windowsJob, /signtool\.exe[\s\S]*?verify \/pa \/all \/v/u);
  assert.match(windowsJob, /always\(\) && steps\.windows-signing-certificate\.outcome == 'success'/u);
  assert.match(windowsJob, /openchat-desktop\.exe/u);
  assert.match(windowsJob, /bundle\/nsis\/\*-setup\.exe/u);
});
