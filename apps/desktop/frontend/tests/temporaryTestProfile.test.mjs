import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { reserveTemporaryTestProfileRoot } from '../../../../scripts/temporaryTestProfile.mjs';

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
