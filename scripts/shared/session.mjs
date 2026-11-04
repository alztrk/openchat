import { spawn } from 'node:child_process';

const INTERRUPT_GRACE_MS = 1500;
const FORCED_SHUTDOWN_CONFIRMATION_MS = 3000;
const TERMINATOR_TIMEOUT_MS = 5000;

export class CommandInterruptedError extends Error {
  constructor(signal) {
    super(`Command interrupted by ${signal}.`);
    this.name = 'CommandInterruptedError';
    this.signal = signal;
  }
}

export class UnconfirmedChildTerminationError extends Error {
  constructor(pid, signal, options) {
    super(`Could not confirm shutdown of child process tree${pid ? ` (PID ${pid})` : ''} after ${signal}.`, options);
    this.name = 'UnconfirmedChildTerminationError';
    this.pid = pid;
    this.signal = signal;
  }
}

export function createInterruptController({ signalSource = process, platform = process.platform } = {}) {
  let interruptedSignal = null;
  const supportedSignals = platform === 'win32' ? ['SIGINT'] : ['SIGINT', 'SIGTERM'];
  const listeners = new Map();
  const subscribers = new Set();

  function receiveSignal(signal) {
    if (interruptedSignal) return;
    interruptedSignal = signal;
    for (const subscriber of subscribers) subscriber(signal);
  }

  for (const signal of supportedSignals) {
    const listener = () => receiveSignal(signal);
    listeners.set(signal, listener);
    signalSource.on(signal, listener);
  }

  return {
    get interruptedSignal() { return interruptedSignal; },
    noteSignal(signal) { if (supportedSignals.includes(signal)) receiveSignal(signal); },
    onInterrupt(subscriber) {
      if (interruptedSignal) {
        subscriber(interruptedSignal);
        return () => undefined;
      }
      subscribers.add(subscriber);
      return () => subscribers.delete(subscriber);
    },
    throwIfInterrupted() { if (interruptedSignal) throw new CommandInterruptedError(interruptedSignal); },
    dispose() {
      for (const [signal, listener] of listeners) signalSource.off(signal, listener);
      subscribers.clear();
    },
  };
}

function waitForClose(closePromise, isClosed, timeoutMs) {
  if (isClosed()) return Promise.resolve(true);

  return new Promise((resolve) => {
    const timeout = setTimeout(() => resolve(false), timeoutMs);
    closePromise.then(() => {
      clearTimeout(timeout);
      resolve(true);
    });
  });
}

function signalUnixProcessTree(child, signal) {
  if (!child.pid) return;
  const treeSignal = signal === 'SIGTERM' ? 'SIGTERM' : 'SIGINT';
  try {
    process.kill(-child.pid, treeSignal);
  } catch (error) {
    if (error?.code !== 'ESRCH') return;
  }
}

function terminateWindowsProcessTree(pid) {
  let terminator;
  try {
    terminator = spawn('taskkill.exe', ['/PID', String(pid), '/T', '/F'], {
      stdio: 'ignore',
      windowsHide: true,
    });
  } catch {
    return Promise.resolve(false);
  }

  return new Promise((resolve) => {
    let settled = false;
    const finish = (succeeded) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      resolve(succeeded);
    };
    const timeout = setTimeout(() => {
      terminator.kill();
      finish(false);
    }, TERMINATOR_TIMEOUT_MS);
    terminator.once('error', () => finish(false));
    terminator.once('close', (code) => finish(code === 0));
  });
}

async function forceTerminateProcessTree(child, platform) {
  if (!child.pid) return false;
  if (platform === 'win32') return terminateWindowsProcessTree(child.pid);

  try {
    process.kill(-child.pid, 'SIGKILL');
    return true;
  } catch (error) {
    return error?.code === 'ESRCH';
  }
}

export async function runInterruptibleCommand(
  command,
  args,
  {
    cwd,
    captureOutput = false,
    interruptController,
    spawnProcess = spawn,
    allowFailure = false,
    platform = process.platform,
    shutdownGraceMs = INTERRUPT_GRACE_MS,
    shutdownConfirmationMs = FORCED_SHUTDOWN_CONFIRMATION_MS,
    terminateProcessTree = forceTerminateProcessTree,
  } = {},
) {
  interruptController?.throwIfInterrupted();

  return new Promise((resolve, reject) => {
    let child;
    try {
      child = spawnProcess(command, args, {
        cwd,
        stdio: captureOutput ? ['ignore', 'pipe', 'inherit'] : 'inherit',
        windowsHide: true,
        detached: platform !== 'win32',
      });
    } catch (error) {
      reject(new Error(`Could not start ${command}: ${error.message}`, { cause: error }));
      return;
    }

    let stdout = '';
    let settled = false;
    let childClosed = false;
    let interruptShutdown;
    let unsubscribeInterrupt = () => undefined;
    let resolveChildClosed;
    const childClosedPromise = new Promise((resolveClose) => {
      resolveChildClosed = resolveClose;
    });

    const settle = (callback, value) => {
      if (settled) return;
      settled = true;
      unsubscribeInterrupt();
      callback(value);
    };

    const handleInterrupt = (signal) => {
      if (interruptShutdown) return;
      interruptShutdown = (async () => {
        if (!childClosed && platform !== 'win32') signalUnixProcessTree(child, signal);

        if (!await waitForClose(childClosedPromise, () => childClosed, shutdownGraceMs)) {
          const terminationConfirmed = await terminateProcessTree(child, platform);
          const childClosedAfterTermination = await waitForClose(
            childClosedPromise,
            () => childClosed,
            shutdownConfirmationMs,
          );
          if (!terminationConfirmed || !childClosedAfterTermination) {
            child.unref?.();
            settle(reject, new UnconfirmedChildTerminationError(child.pid, signal));
            return;
          }
        }

        settle(reject, new CommandInterruptedError(signal));
      })().catch((error) => {
        child.unref?.();
        settle(reject, new UnconfirmedChildTerminationError(child.pid, signal, { cause: error }));
      });
    };

    if (captureOutput && child.stdout) {
      child.stdout.setEncoding('utf8');
      child.stdout.on('data', (chunk) => { stdout += chunk; });
    }

    child.once('error', (error) => {
      settle(reject, new Error(`Could not start ${command}: ${error.message}`, { cause: error }));
    });
    child.once('close', (code, signal) => {
      childClosed = true;
      resolveChildClosed();
      if (signal) interruptController?.noteSignal(signal);
      const receivedSignal = interruptController?.interruptedSignal;
      if (receivedSignal) {
        handleInterrupt(receivedSignal);
        return;
      }

      if (code !== 0 && (!allowFailure || code === null)) {
        const outcome = code === null ? `signal ${signal ?? 'unknown'}` : `exit code ${code}`;
        settle(reject, new Error(`${command} failed with ${outcome}.`));
        return;
      }

      settle(resolve, allowFailure ? { stdout, status: code } : { stdout });
    });

    unsubscribeInterrupt = interruptController?.onInterrupt(handleInterrupt) ?? (() => undefined);
  });
}

export async function runTemporaryProfileSession({
  run,
  cleanup,
  signalSource = process,
  platform = process.platform,
  getRetainedProfilePath = () => undefined,
}) {
  const interruptController = createInterruptController({ signalSource, platform });
  let runError;
  let cleanupError;

  try { await run(interruptController); } catch (error) { runError = error; }
  if (runError instanceof UnconfirmedChildTerminationError) {
    const profilePath = getRetainedProfilePath();
    if (profilePath) runError.message += ` The temporary profile was retained at ${profilePath}.`;
  }
  if (!(runError instanceof UnconfirmedChildTerminationError)) {
    try { await cleanup(); } catch (error) { cleanupError = error; }
  }

  try { await new Promise((resolve) => setImmediate(resolve)); }
  finally { interruptController.dispose(); }

  if (cleanupError) {
    if (runError && !(runError instanceof CommandInterruptedError)) {
      throw new AggregateError([runError, cleanupError], 'Temporary profile run and cleanup failed.');
    }
    throw cleanupError;
  }

  if (runError && !(runError instanceof CommandInterruptedError)) throw runError;
  if (interruptController.interruptedSignal) return { interruptedSignal: interruptController.interruptedSignal };
  if (runError) throw runError;
  return { interruptedSignal: null };
}
