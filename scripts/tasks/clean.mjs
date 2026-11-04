import { existsSync, lstatSync, realpathSync, rmSync } from 'node:fs';
import path from 'node:path';
import { getWorkspaceRoot } from '../shared/runner.mjs';
import { isWithinDirectory } from '../shared/fs-safety.mjs';
import { activeBuildIsRunning } from '../shared/target-utils.mjs';
import { withWorkspaceArtifactLock } from '../shared/artifact-task-lock.mjs';

const generatedOutputDirectories = [
  'target',
  'services/openchat-local-service/target',
  'apps/desktop/tauri/target',
  'apps/desktop/frontend/dist',
  'apps/desktop/tauri/binaries',
  'apps/desktop/tauri/gen',
];

export const clean = {
  name: 'clean',
  description: 'Previews generated output cleanup. Add --apply to remove the listed directories',
  args: [
    { name: '--help', short: '-h', description: 'Show this help message' },
    { name: '--apply', description: 'Remove the listed generated output directories' },
  ],
  async run(args) {
    const options = parseCleanOptions(args);
    if (options === null) return;

    const workspaceRoot = getWorkspaceRoot();
    if (!options.apply) {
      cleanGeneratedOutputs({ workspaceRoot, apply: false });
      return;
    }

    await withWorkspaceArtifactLock(
      'clean',
      () =>
        cleanGeneratedOutputs({
          workspaceRoot,
          apply: true,
          assertNoActiveBuild: assertNoCargoBuild,
        }),
      { workspaceRoot },
    );
  },
};

export function cleanGeneratedOutputs({
  workspaceRoot,
  apply,
  assertNoActiveBuild = () => undefined,
  write = (message) => process.stdout.write(message),
}) {
  const canonicalRoot = realpathSync(workspaceRoot);
  const targets = generatedOutputDirectories
    .map((relativePath) => resolveGeneratedOutputTarget(canonicalRoot, relativePath))
    .filter((target) => target !== null);

  if (targets.length === 0) {
    write('No generated build output directories were found.\n');
    return { targets: [], removed: [] };
  }

  for (const target of targets) {
    write(`${apply ? 'Will remove' : 'Would remove'} ${target.relativePath}.\n`);
  }

  if (!apply) {
    write('Preview only. Add --apply to remove these directories.\n');
    return { targets: targets.map((target) => target.relativePath), removed: [] };
  }

  assertNoActiveBuild();
  const removed = [];
  for (const target of targets) {
    assertNoActiveBuild();
    const currentTarget = resolveGeneratedOutputTarget(canonicalRoot, target.relativePath);
    if (currentTarget === null) continue;
    if (currentTarget.canonicalPath !== target.canonicalPath) {
      throw new Error(`Generated output path '${target.relativePath}' changed during cleanup.`);
    }
    rmSync(currentTarget.canonicalPath, { recursive: true });
    removed.push(target.relativePath);
    write(`Removed ${target.relativePath}.\n`);
  }

  return { targets: targets.map((target) => target.relativePath), removed };
}

function resolveGeneratedOutputTarget(canonicalRoot, relativePath) {
  const targetPath = path.resolve(canonicalRoot, relativePath);
  if (!isWithinDirectory(canonicalRoot, targetPath)) {
    throw new Error('Refusing to clean a path outside the workspace.');
  }
  if (!existsSync(targetPath)) return null;

  const metadata = lstatSync(targetPath);
  if (metadata.isSymbolicLink()) {
    throw new Error(`Refusing to clean a symbolic link at ${relativePath}.`);
  }
  if (!metadata.isDirectory()) {
    throw new Error(`Expected a generated directory at ${relativePath}.`);
  }

  const canonicalPath = realpathSync(targetPath);
  if (!isWithinDirectory(canonicalRoot, canonicalPath)) {
    throw new Error('Refusing to clean a path outside the workspace.');
  }
  return { relativePath, canonicalPath };
}

export function parseCleanOptions(args, write = (message) => process.stdout.write(message)) {
  let apply = false;
  for (const argument of args) {
    if (argument === '--help' || argument === '-h') {
      write('Usage: npm run openchat -- clean [--apply]\n');
      write('Without --apply, this command only previews generated output cleanup.\n');
      return null;
    }
    if (argument === '--apply' && !apply) {
      apply = true;
      continue;
    }
    throw new Error('Use only --apply with the clean task.');
  }
  return { apply };
}

function assertNoCargoBuild() {
  if (activeBuildIsRunning()) {
    throw new Error('A Cargo or rustc build is running; cleanup was not started.');
  }
}
