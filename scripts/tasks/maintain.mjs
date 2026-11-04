import { existsSync } from 'node:fs';
import path from 'node:path';
import { runCommand, getWorkspaceRoot } from '../shared/runner.mjs';
import { activeBuildIsRunning, discoverTargets, formatGiB, pruneStaleArtifacts } from '../shared/target-utils.mjs';
import { withWorkspaceArtifactLock } from '../shared/artifact-task-lock.mjs';

const bytesPerGiB = 1024n ** 3n;

export const maintainTarget = {
  name: 'maintain-target',
  description: 'Reports Cargo target sizes and previews cleanup. Add --apply to clean',
  args: [
    { name: '--help', short: '-h', description: 'Show this help message' },
    { name: '--apply', description: 'Actually clean the target directories (default: preview only)' },
    { name: '--max-gib', value: '<number>', description: 'Max target size in GiB (default: 20, range: 1-1024)' },
  ],
  async run(args) {
    await runMaintainTarget(args);
  },
};

export async function runMaintainTarget(
  args,
  {
    workspaceRoot = getWorkspaceRoot(),
    discoverTargetsFn = discoverTargets,
    pruneStaleArtifactsFn = pruneStaleArtifacts,
    activeBuildIsRunningFn = activeBuildIsRunning,
    withArtifactTaskLock = withWorkspaceArtifactLock,
    runCommandFn = runCommand,
    write = (message) => process.stdout.write(message),
  } = {},
) {
  const options = parseMaintainTargetOptions(args, write);
  if (options === null) return;

  const manifestPath = path.join(workspaceRoot, 'Cargo.toml');
  if (!existsSync(manifestPath)) {
    throw new Error('The Cargo workspace manifest was not found.');
  }

  const targets = discoverTargetsFn(workspaceRoot);
  const totalBytes = sumTargetBytes(targets);
  const thresholdBytes = BigInt(options.maxGiB) * bytesPerGiB;
  writeTargetSummary(targets, totalBytes, options.maxGiB, workspaceRoot, write);

  if (totalBytes <= thresholdBytes) {
    write('Target usage is within the limit; no cleanup is needed.\n');
    return;
  }

  if (!options.apply) {
    const stalePlans = planStaleCleanup(targets, pruneStaleArtifactsFn);
    const staleBytes = sumPrunedBytes(stalePlans);
    writeStalePlan(stalePlans, workspaceRoot, write);
    const projectedBytes = subtractFloorAtZero(totalBytes, staleBytes);

    if (projectedBytes > thresholdBytes) {
      for (const target of targets) {
        write(`Would clean Cargo target directory ${path.relative(workspaceRoot, target.path)} (${formatGiB(target.bytes)} GiB) after stale-artifact pruning.\n`);
      }
    } else {
      write(`Pruning the listed stale artifacts would bring target usage to ${formatGiB(projectedBytes)} GiB.\n`);
    }
    write('Preview only. Add --apply to perform the listed cleanup.\n');
    return;
  }

  await withArtifactTaskLock('maintain-target', async () => {
    assertNoActiveBuild(activeBuildIsRunningFn);

    const currentTargets = discoverTargetsFn(workspaceRoot);
    const currentBytes = sumTargetBytes(currentTargets);
    if (currentBytes <= thresholdBytes) {
      write(`Target usage is now ${formatGiB(currentBytes)} GiB; no cleanup is needed.\n`);
      return;
    }

    const stalePlans = planStaleCleanup(currentTargets, pruneStaleArtifactsFn);
    const staleBytes = sumPrunedBytes(stalePlans);
    const projectedBytes = subtractFloorAtZero(currentBytes, staleBytes);

    if (projectedBytes > thresholdBytes) {
      for (const target of currentTargets) {
        write(`Will clean Cargo target directory ${path.relative(workspaceRoot, target.path)} (${formatGiB(target.bytes)} GiB).\n`);
      }
      for (const target of currentTargets) {
        assertTargetStillDiscovered(target, discoverTargetsFn(workspaceRoot));
        assertNoActiveBuild(activeBuildIsRunningFn);
        runCargoClean(target, manifestPath, workspaceRoot, true, runCommandFn);
      }
      for (const target of currentTargets) {
        assertTargetStillDiscovered(target, discoverTargetsFn(workspaceRoot));
        assertNoActiveBuild(activeBuildIsRunningFn);
        runCargoClean(target, manifestPath, workspaceRoot, false, runCommandFn);
      }
      write('Cargo target cleanup completed.\n');
      return;
    }

    writeStalePlan(stalePlans, workspaceRoot, write, 'Will prune');
    for (const target of currentTargets) {
      const plan = stalePlans.find((candidate) => candidate.target.path === target.path);
      if (!plan || plan.prunedPaths.length === 0) continue;
      assertTargetStillDiscovered(target, discoverTargetsFn(workspaceRoot));
      pruneStaleArtifactsFn(target.path, true, () => assertNoActiveBuild(activeBuildIsRunningFn), plan.prunedPaths);
    }

    const remainingTargets = discoverTargetsFn(workspaceRoot);
    const remainingBytes = sumTargetBytes(remainingTargets);
    if (remainingBytes <= thresholdBytes) {
      write(`Target usage after stale-artifact cleanup: ${formatGiB(remainingBytes)} GiB.\n`);
      return;
    }
    write(`Stale-artifact cleanup completed, but target usage remains ${formatGiB(remainingBytes)} GiB. Review a new preview before full cleanup.\n`);
  }, { workspaceRoot });
}

function parseMaintainTargetOptions(args, write) {
  let maxGiB = 20;
  let apply = false;

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      write('Usage: npm run openchat -- maintain-target [--max-gib 1..1024] [--apply]\n');
      write('Without --apply, this command only reports target sizes and previews cleanup.\n');
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

function sumTargetBytes(targets) {
  return targets.reduce((total, target) => total + target.bytes, 0n);
}

function sumPrunedBytes(stalePlans) {
  return stalePlans.reduce((total, plan) => total + plan.prunedBytes, 0n);
}

function subtractFloorAtZero(value, amount) {
  return value > amount ? value - amount : 0n;
}

function planStaleCleanup(targets, pruneStaleArtifactsFn) {
  return targets.map((target) => ({
    target,
    ...pruneStaleArtifactsFn(target.path, false),
  }));
}

function writeTargetSummary(targets, totalBytes, maxGiB, workspaceRoot, write) {
  write(`Cargo target total: ${formatGiB(totalBytes)} GiB; limit: ${maxGiB} GiB.\n`);
  for (const target of targets) {
    write(`${formatGiB(target.bytes)} GiB  ${path.relative(workspaceRoot, target.path)}\n`);
  }
}

function writeStalePlan(stalePlans, workspaceRoot, write, action = 'Would prune') {
  for (const plan of stalePlans) {
    for (const stalePath of plan.prunedPaths) {
      write(`${action} stale Cargo artifact ${path.relative(workspaceRoot, stalePath)}.\n`);
    }
  }
}

function assertNoActiveBuild(activeBuildIsRunningFn) {
  if (activeBuildIsRunningFn()) {
    throw new Error('A Cargo or rustc build is running; target cleanup was not started.');
  }
}

function assertTargetStillDiscovered(target, discoveredTargets) {
  if (!discoveredTargets.some((currentTarget) => currentTarget.path === target.path)) {
    throw new Error('A Cargo target directory changed during cleanup. Run maintain-target again to review the current targets.');
  }
}

function runCargoClean(target, manifestPath, workspaceRoot, dryRun, runCommandFn) {
  const args = ['clean', '--manifest-path', manifestPath, '--target-dir', target.path, '--locked'];
  if (dryRun) args.push('--dry-run');
  runCommandFn('cargo', args, { cwd: workspaceRoot });
}
