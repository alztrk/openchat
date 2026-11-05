import { existsSync, lstatSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

const REMOVAL_RETRY_DELAYS_MS = [100, 250, 500, 1000, 2000, 4000, 8000];
const RETRYABLE_REMOVAL_CODES = new Set(['EBUSY', 'ENOTEMPTY', 'EPERM']);

export async function removeTemporaryTestProfile(
  profileRoot,
  { removeDirectory = rmSync, wait = (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs)) } = {},
) {
  if (!existsSync(profileRoot)) return;

  const temporaryRoot = realpathSync(tmpdir());
  const metadata = lstatSync(profileRoot);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error('Refusing to remove an invalid desktop test profile.');
  }

  const canonicalProfileRoot = realpathSync(profileRoot);
  const profileName = path.basename(profileRoot);
  if (
    path.dirname(canonicalProfileRoot) !== temporaryRoot ||
    path.basename(canonicalProfileRoot) !== profileName ||
    !/^openchat-desktop-test-[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u.test(profileName)
  ) {
    throw new Error('Refusing to remove a profile outside the desktop test temporary scope.');
  }

  for (let attempt = 0; ; attempt += 1) {
    try {
      removeDirectory(canonicalProfileRoot, { recursive: true });
      return;
    } catch (error) {
      if (!RETRYABLE_REMOVAL_CODES.has(error?.code) || attempt >= REMOVAL_RETRY_DELAYS_MS.length) {
        throw error;
      }

      await wait(REMOVAL_RETRY_DELAYS_MS[attempt]);
    }
  }
}
