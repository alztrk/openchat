import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { EventEmitter } from 'node:events';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  runInterruptibleCommand,
  runTemporaryProfileSession,
  CommandInterruptedError,
  UnconfirmedChildTerminationError,
  createInterruptController,
} from '../../../../scripts/shared/session.mjs';

test('waits for an interrupted child process before cleaning the temporary profile', async () => {
  const signalSource = new EventEmitter();
  const child = new EventEmitter();
  let childClosed = false;
  let cleanupRanAfterChildClosed = false;

  const closeChildOnSignal = () => {
    setImmediate(() => {
      childClosed = true;
      child.emit('close', null, 'SIGINT');
    });
  };

  const session = runTemporaryProfileSession({
    signalSource,
    platform: 'win32',
    run: async (interruptController) => {
      await runInterruptibleCommand('test-child', [], {
        interruptController,
        spawnProcess: () => {
          signalSource.once('SIGINT', closeChildOnSignal);
          return child;
        },
      });
    },
    cleanup: () => {
      cleanupRanAfterChildClosed = childClosed;
    },
  });

  signalSource.emit('SIGINT');

  assert.deepEqual(await session, { interruptedSignal: 'SIGINT' });
  assert.equal(cleanupRanAfterChildClosed, true);
  assert.equal(signalSource.listenerCount('SIGINT'), 0);
});

test('cleans the temporary profile after a build or launch error', async () => {
  const signalSource = new EventEmitter();
  let cleanupRan = false;

  await assert.rejects(
    runTemporaryProfileSession({
      signalSource,
      platform: 'win32',
      run: () => {
        throw new Error('build failed');
      },
      cleanup: () => {
        cleanupRan = true;
      },
    }),
    /build failed/u,
  );

  assert.equal(cleanupRan, true);
  assert.equal(signalSource.listenerCount('SIGINT'), 0);
});

test('force-terminates an unresponsive child tree before cleaning the temporary profile', async () => {
  const signalSource = new EventEmitter();
  const child = new EventEmitter();
  child.pid = 4242;
  child.unref = () => undefined;
  let forcedTerminationRequested = false;
  let cleanupRanAfterChildClosed = false;
  let childClosed = false;

  const session = runTemporaryProfileSession({
    signalSource,
    platform: 'win32',
    run: async (interruptController) => {
      await runInterruptibleCommand('unresponsive-child', [], {
        interruptController,
        platform: 'win32',
        shutdownGraceMs: 0,
        shutdownConfirmationMs: 10,
        spawnProcess: () => child,
        terminateProcessTree: async (target, platform) => {
          forcedTerminationRequested = target === child && platform === 'win32';
          childClosed = true;
          child.emit('close', 1, null);
          return true;
        },
      });
    },
    cleanup: () => {
      cleanupRanAfterChildClosed = childClosed;
    },
  });

  signalSource.emit('SIGINT');

  assert.deepEqual(await session, { interruptedSignal: 'SIGINT' });
  assert.equal(forcedTerminationRequested, true);
  assert.equal(cleanupRanAfterChildClosed, true);
});

test('preserves the temporary profile when child-tree shutdown cannot be confirmed', async () => {
  const signalSource = new EventEmitter();
  const child = new EventEmitter();
  child.pid = 4243;
  let unrefCalled = false;
  let cleanupRan = false;

  const session = runTemporaryProfileSession({
    signalSource,
    platform: 'win32',
    getRetainedProfilePath: () => 'temporary profile path',
    run: async (interruptController) => {
      await runInterruptibleCommand('unresponsive-child', [], {
        interruptController,
        platform: 'win32',
        shutdownGraceMs: 0,
        shutdownConfirmationMs: 0,
        spawnProcess: () => {
          child.unref = () => { unrefCalled = true; };
          return child;
        },
        terminateProcessTree: async () => false,
      });
    },
    cleanup: () => {
      cleanupRan = true;
    },
  });

  signalSource.emit('SIGINT');

  await assert.rejects(
    session,
    (error) =>
      error instanceof UnconfirmedChildTerminationError &&
      error.message.includes('temporary profile path'),
  );
  assert.equal(unrefCalled, true);
  assert.equal(cleanupRan, false);
});

test('interrupt terminates an unresponsive Windows child process tree', { skip: process.platform !== 'win32' }, async () => {
  const temporaryDirectory = mkdtempSync(path.join(os.tmpdir(), 'openchat-child-tree-test-'));
  const grandchildMarkerPath = path.join(temporaryDirectory, 'grandchild-survived');
  const signalSource = new EventEmitter();
  const interruptController = createInterruptController({ signalSource, platform: 'win32' });
  const grandchildSource =
    `setTimeout(() => require('node:fs').writeFileSync(process.argv[1], 'alive'), 3500);`;
  const parentSource = [
    "require('node:child_process').spawn(process.execPath, [",
    "'-e',",
    `${JSON.stringify(grandchildSource)},`,
    `${JSON.stringify(grandchildMarkerPath)}`,
    "], { stdio: 'ignore', windowsHide: true });",
    'setInterval(() => {}, 1000);',
  ].join(' ');

  try {
    const command = runInterruptibleCommand(process.execPath, ['-e', parentSource], {
      interruptController,
      platform: 'win32',
    });
    await new Promise((resolve) => setTimeout(resolve, 300));
    signalSource.emit('SIGINT');
    await assert.rejects(command, CommandInterruptedError);
    await new Promise((resolve) => setTimeout(resolve, 3400));
    assert.equal(existsSync(grandchildMarkerPath), false);
  } finally {
    interruptController.dispose();
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});
