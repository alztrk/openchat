import { spawn, spawnSync } from 'node:child_process';

const INTERRUPT_GRACE_MS = 1500;
const FORCED_SHUTDOWN_CONFIRMATION_MS = 3000;
const PROFILE_PROCESS_GRACE_MS = 2000;
const PROCESS_MARKER_CONFIRMATION_MS = 30_000;
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
    const causeMessage = options?.cause instanceof Error ? ` ${options.cause.message}` : '';
    super(
      `Could not confirm shutdown of child process tree${pid ? ` (PID ${pid})` : ''} after ${signal}.${causeMessage}`,
      options,
    );
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

function inspectWindowsProcessesWithMarker(marker, spawnSyncProcess, timeoutMs) {
  const encodedMarker = Buffer.from(marker, 'utf16le').toString('base64');
  const script =
    `$marker = [System.Text.Encoding]::Unicode.GetString([System.Convert]::FromBase64String('${encodedMarker}')); ` +
    '$matches = @(Get-CimInstance -ClassName Win32_Process | Where-Object { ' +
    '$_.ProcessId -ne $PID -and $null -ne $_.CommandLine -and ' +
    '$_.CommandLine.IndexOf($marker, [System.StringComparison]::OrdinalIgnoreCase) -ge 0 }); ' +
    '[Console]::Out.Write(($matches | ForEach-Object { \'{0},{1}\' -f $_.ProcessId,$_.ParentProcessId }) -join "`n")';
  const result = spawnSyncProcess(
    'powershell.exe',
    ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', script],
    { encoding: 'utf8', timeout: timeoutMs, windowsHide: true },
  );

  if (result.error) {
    throw new Error('Could not inspect Windows processes while confirming desktop shutdown.', {
      cause: result.error,
    });
  }
  if (result.status !== 0) {
    throw new Error(`Windows process inspection failed with exit code ${result.status}.`);
  }

  const output = String(result.stdout ?? '').trim();
  if (!output) return [];

  return output.split(/\r?\n/u).map((line) => {
    const match = /^(\d+),(\d+)$/u.exec(line);
    if (!match) throw new Error('Windows process inspection returned invalid process identifiers.');
    const processId = Number(match[1]);
    const parentProcessId = Number(match[2]);
    if (!Number.isSafeInteger(processId) || processId <= 0 || !Number.isSafeInteger(parentProcessId)) {
      throw new Error('Windows process inspection returned invalid process identifiers.');
    }
    return { processId, parentProcessId };
  });
}

export async function waitForWindowsProcessMarkerToExit(
  marker,
  {
    platform = process.platform,
    timeoutMs = 30_000,
    pollIntervalMs = 250,
    queryTimeoutMs = 3000,
    spawnSyncProcess = spawnSync,
    now = Date.now,
    wait = (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs)),
  } = {},
) {
  if (platform !== 'win32') return true;
  if (!marker) throw new Error('A process marker is required to confirm Windows shutdown.');
  if (timeoutMs < 0 || pollIntervalMs <= 0 || queryTimeoutMs <= 0) {
    throw new Error('Invalid Windows process polling interval.');
  }

  const deadline = now() + timeoutMs;
  let completedProcessQuery = false;

  while (true) {
    const remainingMs = deadline - now();
    const boundedQueryTimeoutMs = Math.min(queryTimeoutMs, Math.max(1, remainingMs));
    let matchingProcesses;
    try {
      matchingProcesses = inspectWindowsProcessesWithMarker(marker, spawnSyncProcess, boundedQueryTimeoutMs);
    } catch (error) {
      if (error.cause?.code === 'ETIMEDOUT') {
        const remainingAfterTimeoutMs = deadline - now();
        if (remainingAfterTimeoutMs <= 0) {
          if (completedProcessQuery) return false;
          throw new Error('Windows process inspection timed out before shutdown could be confirmed.', {
            cause: error.cause,
          });
        }
        await wait(Math.min(pollIntervalMs, remainingAfterTimeoutMs));
        continue;
      }
      throw error;
    }
    completedProcessQuery = true;
    if (matchingProcesses.length === 0) return true;

    const remainingAfterQueryMs = deadline - now();
    if (remainingAfterQueryMs <= 0) return false;
    await wait(Math.min(pollIntervalMs, remainingAfterQueryMs));
  }
}

export function terminateWindowsProcessesUsingMarker(
  marker,
  { platform = process.platform, spawnSyncProcess = spawnSync } = {},
) {
  if (platform !== 'win32') return { matchedProcessCount: 0, failure: null };
  if (!marker) throw new Error('A process marker is required to terminate Windows processes.');

  const matchingProcesses = inspectWindowsProcessesWithMarker(marker, spawnSyncProcess, TERMINATOR_TIMEOUT_MS);
  const matchedIds = new Set(matchingProcesses.map(({ processId }) => processId));
  const rootProcesses = matchingProcesses.filter(({ parentProcessId }) => !matchedIds.has(parentProcessId));
  const terminationTargets = rootProcesses.length > 0 ? rootProcesses : matchingProcesses;
  const terminationDeadline = Date.now() + TERMINATOR_TIMEOUT_MS;
  let failure = null;

  for (const { processId } of terminationTargets) {
    const remainingMs = terminationDeadline - Date.now();
    if (remainingMs <= 0) {
      failure ??= new Error('Windows process termination exceeded its time limit.');
      break;
    }
    const result = spawnSyncProcess(
      'taskkill.exe',
      ['/PID', String(processId), '/T', '/F'],
      { encoding: 'utf8', timeout: remainingMs, windowsHide: true },
    );
    if (result.error) {
      failure ??= new Error('Could not stop a Windows process that references the temporary profile.', {
        cause: result.error,
      });
    } else if (result.status !== 0) {
      failure ??= new Error(`Windows process termination failed with exit code ${result.status}.`);
    }
  }

  return { matchedProcessCount: matchingProcesses.length, failure };
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
    shutdownMarker,
    shutdownMarkerConfirmationMs = PROCESS_MARKER_CONFIRMATION_MS,
    shutdownMarkerGraceMs = PROFILE_PROCESS_GRACE_MS,
    waitForShutdownMarkerToExit = waitForWindowsProcessMarkerToExit,
    terminateProcessesUsingMarker = terminateWindowsProcessesUsingMarker,
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

        const childClosedDuringGrace = await waitForClose(childClosedPromise, () => childClosed, shutdownGraceMs);
        let terminationConfirmed = true;
        let childClosedAfterTermination = childClosedDuringGrace;
        if (!childClosedDuringGrace) {
          terminationConfirmed = await terminateProcessTree(child, platform);
          childClosedAfterTermination = await waitForClose(
            childClosedPromise,
            () => childClosed,
            shutdownConfirmationMs,
          );
        }

        if (platform === 'win32' && shutdownMarker) {
          let markerProcessesStopped = await waitForShutdownMarkerToExit(shutdownMarker, {
            platform,
            timeoutMs: shutdownMarkerGraceMs,
          });
          if (!markerProcessesStopped) {
            const termination = await terminateProcessesUsingMarker(shutdownMarker, { platform });
            markerProcessesStopped = await waitForShutdownMarkerToExit(shutdownMarker, {
              platform,
              timeoutMs: shutdownMarkerConfirmationMs,
            });
            if (markerProcessesStopped) {
              settle(reject, new CommandInterruptedError(signal));
              return;
            }

            child.unref?.();
            settle(
              reject,
              new UnconfirmedChildTerminationError(child.pid, signal, {
                cause: termination.failure ?? new Error(
                  `Windows processes still reference the temporary profile (${termination.matchedProcessCount} found).`,
                ),
              }),
            );
            return;
          }
        } else if (!terminationConfirmed || !childClosedAfterTermination) {
          child.unref?.();
          settle(reject, new UnconfirmedChildTerminationError(child.pid, signal));
          return;
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
