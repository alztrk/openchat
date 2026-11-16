import { fork } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { removeTemporaryTestProfile } from './profile-clean.mjs';
import { waitForWindowsProcessMarkerToExit } from './session.mjs';

const PROFILE_CLEANUP_TIMEOUT_MS = 30_000;
const guardPath = fileURLToPath(new URL('./profile-cleanup-guard.mjs', import.meta.url));

export function startProfileCleanupGuard(profileRoot, { forkProcess = fork } = {}) {
  let child;
  try {
    child = forkProcess(guardPath, [], {
      detached: true,
      windowsHide: true,
      stdio: ['ignore', 'ignore', 'ignore', 'ipc'],
    });
  } catch (error) {
    throw new Error('Could not start the isolated profile cleanup guard.', { cause: error });
  }

  child.unref?.();
  child.channel?.unref?.();

  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      child.off('error', onError);
      child.off('exit', onExit);
      if (error) reject(error);
      else resolve();
    };
    const onError = (error) => finish(new Error('The isolated profile cleanup guard failed to start.', { cause: error }));
    const onExit = (code, signal) => {
      finish(new Error(`The isolated profile cleanup guard exited before accepting its profile (${code ?? signal ?? 'unknown'}).`));
    };

    child.once('error', onError);
    child.once('exit', onExit);
    try {
      child.send({ profileRoot }, (error) => {
        finish(error ? new Error('Could not provide the isolated profile to its cleanup guard.', { cause: error }) : null);
      });
    } catch (error) {
      finish(new Error('Could not provide the isolated profile to its cleanup guard.', { cause: error }));
    }
  });
}

export async function cleanupProfileAfterParentExit(
  profileRoot,
  {
    waitForProcesses = waitForWindowsProcessMarkerToExit,
    removeProfile = removeTemporaryTestProfile,
    timeoutMs = PROFILE_CLEANUP_TIMEOUT_MS,
  } = {},
) {
  const shutdownConfirmed = await waitForProcesses(profileRoot, {
    platform: 'win32',
    timeoutMs,
  });
  if (!shutdownConfirmed) return false;

  await removeProfile(profileRoot);
  return true;
}

export async function runProfileCleanupGuard(
  {
    processSource = process,
    waitForProcesses,
    removeProfile,
    timeoutMs,
  } = {},
) {
  const profileRoot = await new Promise((resolve, reject) => {
    let receivedProfileRoot;
    let settled = false;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      processSource.off('message', onMessage);
      processSource.off('disconnect', onDisconnect);
      if (error) reject(error);
      else resolve(receivedProfileRoot);
    };
    const onMessage = (message) => {
      if (
        typeof message !== 'object' ||
        message === null ||
        !('profileRoot' in message) ||
        typeof message.profileRoot !== 'string'
      ) {
        finish(new Error('The cleanup guard received an invalid profile reference.'));
        return;
      }
      receivedProfileRoot = message.profileRoot;
    };
    const onDisconnect = () => {
      if (!receivedProfileRoot) {
        finish(new Error('The cleanup guard disconnected before receiving a profile reference.'));
        return;
      }
      finish(null);
    };

    processSource.on('message', onMessage);
    processSource.once('disconnect', onDisconnect);
  });

  return cleanupProfileAfterParentExit(profileRoot, {
    waitForProcesses,
    removeProfile,
    timeoutMs,
  });
}

if (process.argv[1] && path.resolve(process.argv[1]) === guardPath) {
  await runProfileCleanupGuard().catch((error) => {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(message + '\n');
    process.exitCode = 1;
  });
}
