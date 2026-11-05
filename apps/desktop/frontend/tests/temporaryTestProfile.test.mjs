import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { reserveTemporaryTestProfileRoot } from '../../../../scripts/shared/profile.mjs';
import { removeTemporaryTestProfile } from '../../../../scripts/shared/profile-clean.mjs';

test('reserves a new profile directory before returning its path', () => {
  const temporaryDirectory = mkdtempSync(path.join(tmpdir(), 'openchat-profile-reserve-'));
  try {
    let generatedIds = 0;
    const profileRoot = reserveTemporaryTestProfileRoot({
      temporaryDirectory,
      createId: () => {
        generatedIds += 1;
        return 'fresh-profile';
      },
    });

    assert.equal(profileRoot, path.join(temporaryDirectory, 'openchat-desktop-test-fresh-profile'));
    assert.equal(generatedIds, 1);
    assert.deepEqual(readdirSync(profileRoot), []);
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});

test('retries a profile-name collision without modifying the existing directory', () => {
  const temporaryDirectory = mkdtempSync(path.join(tmpdir(), 'openchat-profile-collision-'));
  const existingProfileRoot = path.join(temporaryDirectory, 'openchat-desktop-test-collision');
  mkdirSync(existingProfileRoot);
  const existingDataPath = path.join(existingProfileRoot, 'existing.txt');
  writeFileSync(existingDataPath, 'preserve');

  try {
    const ids = ['collision', 'new-profile'];
    const profileRoot = reserveTemporaryTestProfileRoot({
      temporaryDirectory,
      createId: () => ids.shift() ?? 'unexpected',
    });

    assert.equal(profileRoot, path.join(temporaryDirectory, 'openchat-desktop-test-new-profile'));
    assert.equal(readFileSync(existingDataPath, 'utf8'), 'preserve');
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});

test('stops after the bounded number of profile-name collisions', () => {
  const temporaryDirectory = mkdtempSync(path.join(tmpdir(), 'openchat-profile-exhaustion-'));
  const existingProfileRoot = path.join(temporaryDirectory, 'openchat-desktop-test-collision');
  mkdirSync(existingProfileRoot);

  try {
    let generatedIds = 0;
    assert.throws(
      () =>
        reserveTemporaryTestProfileRoot({
          temporaryDirectory,
          createId: () => {
            generatedIds += 1;
            return 'collision';
          },
        }),
      /Could not reserve a unique temporary desktop test profile after 8 attempts/u,
    );
    assert.equal(generatedIds, 8);
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});

test('retries transient profile removal errors and deletes the test profile after the lock clears', async () => {
  const profileRoot = reserveTemporaryTestProfileRoot();
  const retryDelays = [];
  let attempts = 0;

  try {
    await removeTemporaryTestProfile(profileRoot, {
      removeDirectory(target, options) {
        assert.equal(target, profileRoot);
        assert.deepEqual(options, { recursive: true });
        attempts += 1;
        if (attempts < 3) {
          throw Object.assign(new Error('profile is still in use'), { code: 'EPERM' });
        }
        rmSync(target, options);
      },
      wait: async (delayMs) => retryDelays.push(delayMs),
    });

    assert.equal(attempts, 3);
    assert.deepEqual(retryDelays, [100, 250]);
    assert.equal(existsSync(profileRoot), false);
  } finally {
    if (existsSync(profileRoot)) rmSync(profileRoot, { recursive: true, force: true });
  }
});

test('preserves the profile and reports a persistent removal error after bounded retries', async () => {
  const profileRoot = reserveTemporaryTestProfileRoot();
  const retryDelays = [];
  let attempts = 0;

  try {
    await assert.rejects(
      removeTemporaryTestProfile(profileRoot, {
        removeDirectory() {
          attempts += 1;
          throw Object.assign(new Error('profile remains locked'), { code: 'EPERM' });
        },
        wait: async (delayMs) => retryDelays.push(delayMs),
      }),
      (error) => error.code === 'EPERM' && error.message === 'profile remains locked',
    );

    assert.equal(attempts, 8);
    assert.deepEqual(retryDelays, [100, 250, 500, 1000, 2000, 4000, 8000]);
    assert.equal(existsSync(profileRoot), true);
  } finally {
    if (existsSync(profileRoot)) rmSync(profileRoot, { recursive: true, force: true });
  }
});

test('does not retry permanent profile removal errors', async () => {
  const profileRoot = reserveTemporaryTestProfileRoot();
  let attempts = 0;

  try {
    await assert.rejects(
      removeTemporaryTestProfile(profileRoot, {
        removeDirectory() {
          attempts += 1;
          throw Object.assign(new Error('profile access denied'), { code: 'EACCES' });
        },
        wait: async () => assert.fail('permanent errors should not be retried'),
      }),
      (error) => error.code === 'EACCES',
    );

    assert.equal(attempts, 1);
    assert.equal(existsSync(profileRoot), true);
  } finally {
    if (existsSync(profileRoot)) rmSync(profileRoot, { recursive: true, force: true });
  }
});
