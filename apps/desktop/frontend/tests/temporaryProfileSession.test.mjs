import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import test from 'node:test';

import { runInterruptibleCommand, runTemporaryProfileSession } from '../../../../scripts/temporaryProfileSession.mjs';

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
