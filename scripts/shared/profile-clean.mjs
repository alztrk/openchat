import { existsSync, lstatSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

export function removeTemporaryTestProfile(profileRoot) {
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

  rmSync(canonicalProfileRoot, { recursive: true });
}
