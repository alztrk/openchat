import {
  existsSync,
  lstatSync,
  realpathSync,
  rmSync,
} from 'node:fs';
import path from 'node:path';

import { isWithinDirectory, runTask, workspaceRoot } from './task-utils.mjs';

const targets = [
  'target',
  'services/openchat-local-service/target',
  'apps/desktop/tauri/target',
  'apps/desktop/frontend/dist',
  'apps/desktop/tauri/binaries',
  'apps/desktop/tauri/gen',
];

runTask(() => {
  const canonicalRoot = realpathSync(workspaceRoot);

  for (const relativeTarget of targets) {
    const target = path.resolve(workspaceRoot, relativeTarget);
    if (!isWithinDirectory(workspaceRoot, target)) {
      throw new Error('Refusing to clean a path outside the workspace.');
    }
    if (!existsSync(target)) continue;

    const metadata = lstatSync(target);
    if (metadata.isSymbolicLink()) {
      throw new Error('Refusing to clean a symbolic link at ' + relativeTarget + '.');
    }
    if (!metadata.isDirectory()) {
      throw new Error('Expected a generated directory at ' + relativeTarget + '.');
    }

    const canonicalTarget = realpathSync(target);
    if (!isWithinDirectory(canonicalRoot, canonicalTarget)) {
      throw new Error('Refusing to clean a path outside the workspace.');
    }

    rmSync(canonicalTarget, { recursive: true });
    process.stdout.write('Removed ' + relativeTarget + '.\n');
  }
});
