import { randomUUID } from 'node:crypto';
import { mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

const MAX_PROFILE_RESERVATION_ATTEMPTS = 8;

export function reserveTemporaryTestProfileRoot({ temporaryDirectory = tmpdir(), createId = randomUUID } = {}) {
  for (let attempt = 0; attempt < MAX_PROFILE_RESERVATION_ATTEMPTS; attempt += 1) {
    const profileRoot = path.join(temporaryDirectory, `openchat-desktop-test-${createId()}`);
    try {
      mkdirSync(profileRoot);
      return profileRoot;
    } catch (error) {
      if (error?.code === 'EEXIST') continue;
      throw new Error('Could not reserve a temporary desktop test profile.', { cause: error });
    }
  }
  throw new Error(`Could not reserve a unique temporary desktop test profile after ${MAX_PROFILE_RESERVATION_ATTEMPTS} attempts.`);
}
