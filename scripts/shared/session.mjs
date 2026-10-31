import { spawn } from 'node:child_process';

export class CommandInterruptedError extends Error {
  constructor(signal) {
    super(`Command interrupted by ${signal}.`);
    this.name = 'CommandInterruptedError';
    this.signal = signal;
  }
}

export function createInterruptController({ signalSource = process, platform = process.platform } = {}) {
  let interruptedSignal = null;
  const supportedSignals = platform === 'win32' ? ['SIGINT'] : ['SIGINT', 'SIGTERM'];
  const listeners = new Map();

  function receiveSignal(signal) {
    interruptedSignal ??= signal;
  }

  for (const signal of supportedSignals) {
    const listener = () => receiveSignal(signal);
    listeners.set(signal, listener);
    signalSource.on(signal, listener);
  }

  return {
    get interruptedSignal() { return interruptedSignal; },
    noteSignal(signal) { if (supportedSignals.includes(signal)) receiveSignal(signal); },
    throwIfInterrupted() { if (interruptedSignal) throw new CommandInterruptedError(interruptedSignal); },
    dispose() { for (const [sig, listener] of listeners) signalSource.off(sig, listener); },
  };
}

export async function runInterruptibleCommand(command, args, { cwd, captureOutput = false, interruptController, spawnProcess = spawn } = {}) {
  interruptController?.throwIfInterrupted();

  return new Promise((resolve, reject) => {
    let child;
    try {
      child = spawnProcess(command, args, {
        cwd,
        stdio: captureOutput ? ['ignore', 'pipe', 'inherit'] : 'inherit',
        windowsHide: true,
      });
    } catch (error) {
      reject(new Error(`Could not start ${command}: ${error.message}`, { cause: error }));
      return;
    }

    let stdout = '';
    let settled = false;
    const rejectOnce = (error) => { if (settled) return; settled = true; reject(error); };

    if (captureOutput && child.stdout) {
      child.stdout.setEncoding('utf8');
      child.stdout.on('data', (chunk) => { stdout += chunk; });
    }

    child.once('error', (error) => {
      rejectOnce(new Error(`Could not start ${command}: ${error.message}`, { cause: error }));
    });
    child.once('close', (code, signal) => {
      if (settled) return;
      if (signal) interruptController?.noteSignal(signal);

      const receivedSignal = interruptController?.interruptedSignal;
      if (receivedSignal) {
        rejectOnce(new CommandInterruptedError(receivedSignal));
        return;
      }

      if (code !== 0) {
        const outcome = code === null ? `signal ${signal ?? 'unknown'}` : `exit code ${code}`;
        rejectOnce(new Error(`${command} failed with ${outcome}.`));
        return;
      }

      settled = true;
      resolve({ stdout });
    });
  });
}

export async function runTemporaryProfileSession({ run, cleanup, signalSource = process, platform = process.platform }) {
  const interruptController = createInterruptController({ signalSource, platform });
  let runError;
  let cleanupError;

  try { await run(interruptController); } catch (error) { runError = error; }
  try { await cleanup(); } catch (error) { cleanupError = error; }

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
