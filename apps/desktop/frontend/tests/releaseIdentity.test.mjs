import assert from 'node:assert/strict';
import test from 'node:test';

import {
  assertCleanReleaseTree,
  parseBuildChannel,
  resolveReleaseIdentity,
} from '../../../../scripts/shared/release-identity.mjs';
import { readApplicationBaseVersion } from '../../../../scripts/tasks/build.mjs';
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
