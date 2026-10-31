import { runCommand, getWorkspaceRoot } from '../shared/runner.mjs';
import { directoryBytes, discoverTargets, formatGiB, pruneStaleArtifacts, activeBuildIsRunning } from '../shared/target-utils.mjs';
import { realpathSync, existsSync } from 'node:fs';
import path from 'node:path';

export const maintainTarget = {
  name: 'maintain-target',
  description: 'Reports Cargo target sizes and previews cleanup. Add --apply to clean.',
  args: [
    { name: '--help', short: '-h', description: 'Show this help message' },
    { name: '--apply', description: 'Actually clean the target directories (default: preview only)' },
    { name: '--max-gib', value: '<number>', description: 'Max target size in GiB (default: 20, range: 1-1024)' },
  ],
  run(args) {
    const options = parseMaintainTargetOptions(args);
    if (options === null) return;

    const manifestPath = path.join(getWorkspaceRoot(), 'Cargo.toml');
    if (!existsSync(manifestPath)) {
      throw new Error('The Cargo workspace manifest was not found.');
    }

    const targets = discoverTargets();
    const totalBytes = targets.reduce((total, target) => total + target.bytes, 0n);
    const thresholdBytes = BigInt(options.maxGiB) * 1024n ** 3n;

    process.stdout.write(
      `Cargo target total: ${formatGiB(totalBytes)} GiB; limit: ${options.maxGiB} GiB.\n`,
    );
    for (const target of targets) {
      process.stdout.write(`${formatGiB(target.bytes)} GiB  ${path.relative(getWorkspaceRoot(), target.path)}\n`);
    }

    for (const target of targets) {
      const { prunedBytes, prunedCount } = pruneStaleArtifacts(target.path, options.apply);
      if (prunedBytes > 0n || prunedCount > 0) {
        const mode = options.apply ? 'Pruned' : 'Previewed pruning for';
        process.stdout.write(`${mode} ${prunedCount} stale artifact${prunedCount === 1 ? '' : 's'} (${formatGiB(prunedBytes)} GiB) in ${path.relative(getWorkspaceRoot(), target.path)}.\n`);
      }
    }

    const updatedTargets = discoverTargets();
    const remainingBytes = updatedTargets.reduce((total, target) => total + target.bytes, 0n);

    if (remainingBytes <= thresholdBytes) {
      process.stdout.write('Target usage is within the limit; no cleanup is needed.\n');
      return;
    }
    if (!options.apply) {
      process.stdout.write('Preview only. Add --apply to clean these Cargo target directories.\n');
      return;
    }
    if (activeBuildIsRunning()) {
      process.stdout.write('A Cargo or rustc process is running; cleanup was skipped.\n');
      return;
    }

    for (const target of targets) {
      process.stdout.write(`Previewing Cargo cleanup for ${path.relative(getWorkspaceRoot(), target.path)}.\n`);
      runCommand(
        'cargo',
        ['clean', '--manifest-path', manifestPath, '--target-dir', target.path, '--locked', '--dry-run'],
        { cwd: getWorkspaceRoot() },
      );
    }

    for (const target of targets) {
      process.stdout.write(`Cleaning Cargo target directory ${path.relative(getWorkspaceRoot(), target.path)}.\n`);
      runCommand(
        'cargo',
        ['clean', '--manifest-path', manifestPath, '--target-dir', target.path, '--locked'],
        { cwd: getWorkspaceRoot() },
      );
    }
  },
};

function parseMaintainTargetOptions(args) {
  let maxGiB = 20;
  let apply = false;

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      process.stdout.write('Usage: npm run openchat -- maintain-target [--max-gib 1..1024] [--apply]\n');
      process.stdout.write('Without --apply, this command only reports target sizes.\n');
      return null;
    }
    if (argument === '--apply') {
      apply = true;
      continue;
    }
    if (argument === '--max-gib') {
      const value = args[index + 1];
      if (!value || !/^\d+$/u.test(value)) {
        throw new Error('--max-gib requires a whole number from 1 to 1024.');
      }
      maxGiB = Number(value);
      if (!Number.isInteger(maxGiB) || maxGiB < 1 || maxGiB > 1024) {
        throw new Error('--max-gib must be between 1 and 1024.');
      }
      index += 1;
      continue;
    }
    throw new Error('Unknown maintain-target option: ' + argument);
  }

  return { apply, maxGiB };
}
