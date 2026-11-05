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
  terminateWindowsProcessesUsingMarker,
  waitForWindowsProcessMarkerToExit,
} from '../../../../scripts/shared/session.mjs';

test('Windows shutdown confirmation skips process inspection on other platforms', async () => {
  let inspected = false;
  const stopped = await waitForWindowsProcessMarkerToExit('profile marker', {
    platform: 'linux',
    spawnSyncProcess: () => {
      inspected = true;
      throw new Error('process inspection should not run');
    },
  });

  assert.equal(stopped, true);
  assert.equal(inspected, false);
});

test('Windows shutdown confirmation polls until no process contains the profile marker', async () => {
  const marker = 'C:\\temp\\openchat-profile';
  const encodedMarker = Buffer.from(marker, 'utf16le').toString('base64');
  const processRows = ['10,1\n11,10', '12,1', ''];
  const waitDelays = [];
  let currentTime = 0;

  const stopped = await waitForWindowsProcessMarkerToExit(marker, {
    platform: 'win32',
    timeoutMs: 1000,
    pollIntervalMs: 250,
    now: () => currentTime,
    wait: async (delayMs) => {
      waitDelays.push(delayMs);
      currentTime += delayMs;
    },
    spawnSyncProcess: (command, args, options) => {
      assert.equal(command, 'powershell.exe');
      assert.equal(options.windowsHide, true);
      assert.ok(args.at(-1).includes(encodedMarker));
      return { status: 0, stdout: processRows.shift() ?? '' };
    },
  });

  assert.equal(stopped, true);
  assert.deepEqual(waitDelays, [250, 250]);
});

test('Windows shutdown confirmation stops after the configured timeout', async () => {
  let currentTime = 0;
  let processQueries = 0;
  const waitDelays = [];

  const stopped = await waitForWindowsProcessMarkerToExit('profile marker', {
    platform: 'win32',
    timeoutMs: 500,
    pollIntervalMs: 200,
    now: () => currentTime,
    wait: async (delayMs) => {
      waitDelays.push(delayMs);
      currentTime += delayMs;
    },
    spawnSyncProcess: () => {
      processQueries += 1;
      return { status: 0, stdout: '10,1' };
    },
  });

  assert.equal(stopped, false);
  assert.equal(processQueries, 4);
  assert.deepEqual(waitDelays, [200, 200, 100]);
});

test('Windows shutdown confirmation reports process-inspection failures', async () => {
  await assert.rejects(
    waitForWindowsProcessMarkerToExit('profile marker', {
      platform: 'win32',
      spawnSyncProcess: () => ({ status: 1, stdout: '', stderr: 'denied' }),
    }),
    /Windows process inspection failed with exit code 1/u,
  );
});

test('Windows shutdown confirmation retries a timed-out process query', async () => {
  let currentTime = 0;
  let processQueries = 0;
  const waitDelays = [];

  const stopped = await waitForWindowsProcessMarkerToExit('profile marker', {
    platform: 'win32',
    timeoutMs: 1000,
    pollIntervalMs: 250,
    queryTimeoutMs: 100,
    now: () => currentTime,
    wait: async (delayMs) => {
      waitDelays.push(delayMs);
      currentTime += delayMs;
    },
    spawnSyncProcess: (_command, _args, options) => {
      processQueries += 1;
      if (processQueries === 1) {
        currentTime += options.timeout;
        return { error: Object.assign(new Error('timed out'), { code: 'ETIMEDOUT' }), status: null };
      }
      return { status: 0, stdout: '' };
    },
  });

  assert.equal(stopped, true);
  assert.equal(processQueries, 2);
  assert.deepEqual(waitDelays, [250]);
});

test('Windows shutdown confirmation reports repeated query timeouts without a successful check', async () => {
  let currentTime = 0;
  const waitDelays = [];

  await assert.rejects(
    waitForWindowsProcessMarkerToExit('profile marker', {
      platform: 'win32',
      timeoutMs: 300,
      pollIntervalMs: 50,
      queryTimeoutMs: 100,
      now: () => currentTime,
      wait: async (delayMs) => {
        waitDelays.push(delayMs);
        currentTime += delayMs;
      },
      spawnSyncProcess: (_command, _args, options) => {
        currentTime += options.timeout;
        return { error: Object.assign(new Error('timed out'), { code: 'ETIMEDOUT' }), status: null };
      },
    }),
    /Windows process inspection timed out before shutdown could be confirmed/u,
  );
  assert.deepEqual(waitDelays, [50, 50]);
});

test('Windows profile termination targets only roots in the matching process tree', () => {
  const terminationTargets = [];

  const result = terminateWindowsProcessesUsingMarker('temporary profile path', {
    platform: 'win32',
    spawnSyncProcess: (command, args, options) => {
      assert.equal(options.windowsHide, true);
      if (command === 'powershell.exe') {
        assert.ok(args.at(-1).includes(Buffer.from('temporary profile path', 'utf16le').toString('base64')));
        return { status: 0, stdout: '10,1\n11,10\n12,1' };
      }
      terminationTargets.push(args);
      return { status: 0, stdout: '' };
    },
  });

  assert.equal(result.matchedProcessCount, 3);
  assert.equal(result.failure, null);
  assert.deepEqual(terminationTargets, [
    ['/PID', '10', '/T', '/F'],
    ['/PID', '12', '/T', '/F'],
  ]);
});

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

test('confirms all Windows profile processes before cleaning after Ctrl+C', async () => {
  const signalSource = new EventEmitter();
  const child = new EventEmitter();
  let markerConfirmationRan = false;
  let cleanupRanAfterConfirmation = false;

  const session = runTemporaryProfileSession({
    signalSource,
    platform: 'win32',
    run: async (interruptController) => {
      await runInterruptibleCommand('desktop-cli', [], {
        interruptController,
        platform: 'win32',
        shutdownMarker: 'temporary profile path',
        spawnProcess: () => child,
        waitForShutdownMarkerToExit: async (marker, options) => {
          assert.equal(marker, 'temporary profile path');
          assert.equal(options.platform, 'win32');
          markerConfirmationRan = true;
          return true;
        },
      });
    },
    cleanup: () => {
      cleanupRanAfterConfirmation = markerConfirmationRan;
    },
  });

  signalSource.emit('SIGINT');
  setImmediate(() => child.emit('close', null, null));

  assert.deepEqual(await session, { interruptedSignal: 'SIGINT' });
  assert.equal(markerConfirmationRan, true);
  assert.equal(cleanupRanAfterConfirmation, true);
});

test('terminates remaining Windows profile processes before cleaning after Ctrl+C', async () => {
  const signalSource = new EventEmitter();
  const child = new EventEmitter();
  let markerChecks = 0;
  let markedProcessesTerminated = false;
  let cleanupRanAfterConfirmation = false;

  const session = runTemporaryProfileSession({
    signalSource,
    platform: 'win32',
    run: async (interruptController) => {
      await runInterruptibleCommand('desktop-cli', [], {
        interruptController,
        platform: 'win32',
        shutdownMarker: 'temporary profile path',
        spawnProcess: () => child,
        waitForShutdownMarkerToExit: async (_marker, options) => {
          markerChecks += 1;
          assert.equal(options.platform, 'win32');
          return markerChecks === 2;
        },
        terminateProcessesUsingMarker: async (marker, options) => {
          assert.equal(marker, 'temporary profile path');
          assert.equal(options.platform, 'win32');
          markedProcessesTerminated = true;
          return { matchedProcessCount: 2, failure: null };
        },
      });
    },
    cleanup: () => {
      cleanupRanAfterConfirmation = markedProcessesTerminated && markerChecks === 2;
    },
  });

  signalSource.emit('SIGINT');
  setImmediate(() => child.emit('close', null, null));

  assert.deepEqual(await session, { interruptedSignal: 'SIGINT' });
  assert.equal(cleanupRanAfterConfirmation, true);
});

test('preserves the Windows profile when matching processes remain after Ctrl+C', async () => {
  const signalSource = new EventEmitter();
  const child = new EventEmitter();
  let cleanupRan = false;
  let markerChecks = 0;

  const session = runTemporaryProfileSession({
    signalSource,
    platform: 'win32',
    getRetainedProfilePath: () => 'temporary profile path',
    run: async (interruptController) => {
      await runInterruptibleCommand('desktop-cli', [], {
        interruptController,
        platform: 'win32',
        shutdownMarker: 'temporary profile path',
        spawnProcess: () => child,
        waitForShutdownMarkerToExit: async () => {
          markerChecks += 1;
          return false;
        },
        terminateProcessesUsingMarker: async () => ({ matchedProcessCount: 1, failure: null }),
      });
    },
    cleanup: () => {
      cleanupRan = true;
    },
  });

  signalSource.emit('SIGINT');
  setImmediate(() => child.emit('close', null, null));

  await assert.rejects(session, (error) =>
    error instanceof UnconfirmedChildTerminationError &&
      error.message.includes('temporary profile path'),
  );
  assert.equal(markerChecks, 2);
  assert.equal(cleanupRan, false);
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
