import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import test from 'node:test';

import {
  cleanupProfileAfterParentExit,
  runProfileCleanupGuard,
  startProfileCleanupGuard,
} from '../../../../scripts/shared/profile-cleanup-guard.mjs';

test('profile cleanup guard receives its path over a hidden detached IPC child', async () => {
  const profileRoot = 'C:\\temp\\openchat-desktop-test-00000000-0000-4000-8000-000000000000';
  const child = new EventEmitter();
  let unrefCalled = false;
  let channelUnrefCalled = false;
  let call;
  child.unref = () => { unrefCalled = true; };
  child.channel = { unref: () => { channelUnrefCalled = true; } };
  child.send = (message, callback) => {
    call.message = message;
    callback(null);
  };

  await startProfileCleanupGuard(profileRoot, {
    forkProcess: (file, args, options) => {
      call = { file, args, options };
      return child;
    },
  });

  assert.equal(call.file.endsWith('profile-cleanup-guard.mjs'), true);
  assert.deepEqual(call.args, []);
  assert.equal(call.options.detached, true);
  assert.equal(call.options.windowsHide, true);
  assert.deepEqual(call.options.stdio, ['ignore', 'ignore', 'ignore', 'ipc']);
  assert.deepEqual(call.message, { profileRoot });
  assert.equal(unrefCalled, true);
  assert.equal(channelUnrefCalled, true);
});

test('profile cleanup guard waits for the parent to disconnect before checking process shutdown', async () => {
  const profileRoot = 'C:\\temp\\openchat-desktop-test-00000000-0000-4000-8000-000000000000';
  const processSource = new EventEmitter();
  let checkedProfile;
  let removedProfile;
  const pending = runProfileCleanupGuard({
    processSource,
    waitForProcesses: async (marker, options) => {
      checkedProfile = { marker, options };
      return true;
    },
    removeProfile: async (value) => { removedProfile = value; },
    timeoutMs: 4000,
  });

  processSource.emit('message', { profileRoot });
  assert.equal(checkedProfile, undefined);
  processSource.emit('disconnect');

  assert.equal(await pending, true);
  assert.deepEqual(checkedProfile, {
    marker: profileRoot,
    options: { platform: 'win32', timeoutMs: 4000 },
  });
  assert.equal(removedProfile, profileRoot);
});

test('profile cleanup guard preserves the directory when shutdown cannot be confirmed', async () => {
  const profileRoot = 'C:\\temp\\openchat-desktop-test-00000000-0000-4000-8000-000000000000';
  let removeCalled = false;

  const confirmed = await cleanupProfileAfterParentExit(profileRoot, {
    waitForProcesses: async () => false,
    removeProfile: async () => { removeCalled = true; },
  });

  assert.equal(confirmed, false);
  assert.equal(removeCalled, false);
});

test('profile cleanup guard rejects an invalid IPC message without deleting a profile', async () => {
  const processSource = new EventEmitter();
  const pending = runProfileCleanupGuard({ processSource });

  processSource.emit('message', { profileRoot: 7 });

  await assert.rejects(pending, /invalid profile reference/u);
});
