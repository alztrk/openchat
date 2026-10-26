import {
  existsSync,
  lstatSync,
  readdirSync,
  realpathSync,
  statSync,
} from 'node:fs';
import path from 'node:path';

import {
  isWithinDirectory,
  runCommand,
  runTask,
  workspaceRoot,
} from './task-utils.mjs';

const gibibyte = 1024n ** 3n;
const targetDirectories = [
  'target',
  'services/openchat-local-service/target',
  'apps/desktop/tauri/target',
];

function parseOptions(args) {
  let maxGiB = 20;
  let apply = false;

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      process.stdout.write(
        'Usage: npm run maintain-target -- [--max-gib 1..1024] [--apply]\n' +
          'Without --apply, this command only reports target sizes.\n',
      );
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
    throw new Error('Unknown option: ' + argument);
  }

  return { apply, maxGiB };
}

function directoryBytes(root) {
  let total = 0n;
  const pending = [root];

  while (pending.length > 0) {
    const directory = pending.pop();
    if (!directory) continue;

    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const entryPath = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) continue;
      if (entry.isDirectory()) {
        pending.push(entryPath);
      } else if (entry.isFile()) {
        total += statSync(entryPath, { bigint: true }).size;
      }
    }
  }

  return total;
}

function discoverTargets() {
  const canonicalRoot = realpathSync(workspaceRoot);
  const targets = [];

  for (const relativeTarget of targetDirectories) {
    const candidate = path.resolve(workspaceRoot, relativeTarget);
    if (!isWithinDirectory(workspaceRoot, candidate)) {
      throw new Error('A Cargo target path resolves outside the workspace.');
    }
    if (!existsSync(candidate)) continue;

    const metadata = lstatSync(candidate);
    if (metadata.isSymbolicLink()) {
      process.stderr.write('Skipping symbolic-link target directory ' + relativeTarget + '.\n');
      continue;
    }
    if (!metadata.isDirectory()) continue;

    const targetPath = realpathSync(candidate);
    if (!isWithinDirectory(canonicalRoot, targetPath)) {
      throw new Error('A Cargo target path resolves outside the workspace.');
    }

    const cargoMarker =
      existsSync(path.join(targetPath, '.rustc_info.json')) ||
      existsSync(path.join(targetPath, 'CACHEDIR.TAG'));
    if (!cargoMarker) {
      process.stderr.write('Skipping a directory without a Cargo marker: ' + relativeTarget + '.\n');
      continue;
    }

    targets.push({ path: targetPath, bytes: directoryBytes(targetPath) });
  }

  return targets;
}

function activeBuildIsRunning() {
  if (process.platform === 'win32') {
    const result = runCommand('tasklist.exe', ['/FO', 'CSV', '/NH'], {
      captureOutput: true,
    });
    return (result.stdout ?? '').split(/\r?\n/u).some((line) => {
      const processName = /^"([^"]+)"/u.exec(line)?.[1]?.toLowerCase();
      return processName === 'cargo.exe' || processName === 'rustc.exe';
    });
  }

  if (process.platform === 'linux' || process.platform === 'darwin') {
    const result = runCommand('ps', ['-A', '-o', 'comm='], { captureOutput: true });
    return (result.stdout ?? '').split(/\r?\n/u).some((line) => {
      const processName = path.basename(line.trim()).toLowerCase();
      return processName === 'cargo' || processName === 'rustc';
    });
  }

  throw new Error('Active Rust build detection is not supported on this operating system.');
}

function formatGiB(bytes) {
  return (Number(bytes) / Number(gibibyte)).toFixed(2);
}

function main() {
  const options = parseOptions(process.argv.slice(2));
  if (options === null) return;

  const manifestPath = path.join(workspaceRoot, 'Cargo.toml');
  if (!existsSync(manifestPath)) {
    throw new Error('The Cargo workspace manifest was not found.');
  }

  const targets = discoverTargets();
  const totalBytes = targets.reduce((total, target) => total + target.bytes, 0n);
  const thresholdBytes = BigInt(options.maxGiB) * gibibyte;

  process.stdout.write(
    'Cargo target total: ' +
      formatGiB(totalBytes) +
      ' GiB; limit: ' +
      options.maxGiB +
      ' GiB.\n',
  );
  for (const target of targets) {
    process.stdout.write(
      formatGiB(target.bytes) + ' GiB  ' + path.relative(workspaceRoot, target.path) + '\n',
    );
  }

  if (totalBytes <= thresholdBytes) {
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
    process.stdout.write('Previewing Cargo cleanup for ' + path.relative(workspaceRoot, target.path) + '.\n');
    runCommand(
      'cargo',
      [
        'clean',
        '--manifest-path',
        manifestPath,
        '--target-dir',
        target.path,
        '--locked',
        '--dry-run',
      ],
      { cwd: workspaceRoot },
    );
  }

  for (const target of targets) {
    process.stdout.write('Cleaning Cargo target directory ' + path.relative(workspaceRoot, target.path) + '.\n');
    runCommand(
      'cargo',
      ['clean', '--manifest-path', manifestPath, '--target-dir', target.path, '--locked'],
      { cwd: workspaceRoot },
    );
  }
}

runTask(main);
