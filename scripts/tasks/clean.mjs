import { runCommand, getWorkspaceRoot } from '../shared/runner.mjs';
import { isWithinDirectory } from '../shared/fs-safety.mjs';
import { ensureNoArguments } from '../help.mjs';
import { existsSync, realpathSync, lstatSync, rmSync } from 'node:fs';
import path from 'node:path';

export const clean = {
  name: 'clean',
  description: 'Removes all generated build and bundle output directories.',
  args: [],
  run() {
    ensureNoArguments('clean', []);

    const targets = [
      'target',
      'services/openchat-local-service/target',
      'apps/desktop/tauri/target',
      'apps/desktop/frontend/dist',
      'apps/desktop/tauri/binaries',
      'apps/desktop/tauri/gen',
    ];
    const canonicalRoot = realpathSync(getWorkspaceRoot());

    for (const relativeTarget of targets) {
      const target = path.resolve(getWorkspaceRoot(), relativeTarget);
      if (!isWithinDirectory(getWorkspaceRoot(), target)) {
        throw new Error(`Refusing to clean a path outside the workspace.`);
      }
      if (!existsSync(target)) continue;

      const metadata = lstatSync(target);
      if (metadata.isSymbolicLink()) {
        throw new Error(`Refusing to clean a symbolic link at ${relativeTarget}.`);
      }
      if (!metadata.isDirectory()) {
        throw new Error(`Expected a generated directory at ${relativeTarget}.`);
      }

      const canonicalTarget = realpathSync(target);
      if (!isWithinDirectory(canonicalRoot, canonicalTarget)) {
        throw new Error(`Refusing to clean a path outside the workspace.`);
      }

      rmSync(canonicalTarget, { recursive: true });
      process.stdout.write(`Removed ${relativeTarget}.\n`);
    }
  },
};
