import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const workspaceRoot = path.resolve(scriptDirectory, '..');
const frontendDirectory = path.join(workspaceRoot, 'apps', 'desktop', 'frontend');
const tauriDirectory = path.join(workspaceRoot, 'apps', 'desktop', 'tauri');
const gibibyte = 1024n ** 3n;

function runCommand(command, args, { cwd = workspaceRoot, captureOutput = false } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: 'utf8',
    stdio: captureOutput ? 'pipe' : 'inherit',
    windowsHide: true,
  });

  if (result.error) {
    throw new Error('Could not start ' + command + ': ' + result.error.message, {
      cause: result.error,
    });
  }

  if (result.status !== 0) {
    const outcome =
      result.status === null ? 'signal ' + (result.signal ?? 'unknown') : 'exit code ' + result.status;
    throw new Error(command + ' failed with ' + outcome + '.');
  }

  return result;
}

function runNpm(args, cwd = frontendDirectory) {
  const npmCliPath = process.env.npm_execpath;
  if (!npmCliPath) {
    throw new Error('Run this task through its npm script so it can invoke npm without a shell.');
  }

  return runCommand(process.execPath, [npmCliPath, ...args], { cwd });
}

function runTask(task) {
  try {
    task();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(message + '\n');
    process.exitCode = 1;
  }
}

function isWithinDirectory(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return (
    relative !== '' &&
    relative !== '..' &&
    !relative.startsWith('..' + path.sep) &&
    !path.isAbsolute(relative)
  );
}

function rustHostTargetTriple() {
  const result = runCommand('rustc', ['-vV'], { captureOutput: true });
  const match = /^host: ([^\r\n]+)$/mu.exec(result.stdout ?? '');
  if (!match?.[1]) {
    throw new Error('rustc did not report a host target triple.');
  }
  return match[1];
}

function buildLocalService(profile, locked) {
  if (profile !== 'debug' && profile !== 'release') {
    throw new Error('The local-service build profile is invalid.');
  }

  const targetTriple = rustHostTargetTriple();
  const args = [
    'build',
    '--target',
    targetTriple,
    '-p',
    'openchat-local-service',
    '--bin',
    'openchat-local-service',
  ];
  if (profile === 'release') args.push('--release');
  if (locked) args.push('--locked');
  runCommand('cargo', args);

  const extension = targetTriple.split('-').includes('windows') ? '.exe' : '';
  const source = path.join(
    workspaceRoot,
    'target',
    targetTriple,
    profile,
    'openchat-local-service' + extension,
  );
  const destinationDirectory = path.join(tauriDirectory, 'binaries');
  const destination = path.join(
    destinationDirectory,
    'openchat-local-service-' + targetTriple + extension,
  );

  mkdirSync(destinationDirectory, { recursive: true });
  copyFileSync(source, destination);
  process.stdout.write('Prepared the local-service sidecar for ' + targetTriple + '.\n');
}

function ensureNoArguments(command, args) {
  if (args.length > 0) {
    throw new Error('The ' + command + ' task does not accept additional arguments.');
  }
}

function dev(args) {
  ensureNoArguments('dev', args);
  buildLocalService('debug', false);

  if (!existsSync(path.join(frontendDirectory, 'node_modules'))) {
    runNpm(['ci'], frontendDirectory);
  }

  runNpm(['run', 'tauri:dev'], frontendDirectory);
}

function build(args) {
  ensureNoArguments('build', args);
  buildLocalService('release', true);
  runNpm(['run', 'tauri:build'], frontendDirectory);
}

function check(args) {
  ensureNoArguments('check', args);
  runNpm(['exec', '--', 'biome', 'check'], frontendDirectory);
  runNpm(['run', 'build'], frontendDirectory);

  runCommand('cargo', ['fmt', '--all', '--', '--check'], { cwd: workspaceRoot });
  runCommand('cargo', ['check', '--workspace'], { cwd: workspaceRoot });
  runCommand('cargo', ['test', '--workspace', '--', '--test-threads=1'], {
    cwd: workspaceRoot,
  });
}

function clean(args) {
  ensureNoArguments('clean', args);
  const targets = [
    'target',
    'services/openchat-local-service/target',
    'apps/desktop/tauri/target',
    'apps/desktop/frontend/dist',
    'apps/desktop/tauri/binaries',
    'apps/desktop/tauri/gen',
  ];
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
}

function parseMaintainTargetOptions(args) {
  let maxGiB = 20;
  let apply = false;

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      process.stdout.write(
        'Usage: npm run tasks -- maintain-target [--max-gib 1..1024] [--apply]\n' +
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
    throw new Error('Unknown maintain-target option: ' + argument);
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
  const targetDirectories = [
    'target',
    'services/openchat-local-service/target',
    'apps/desktop/tauri/target',
  ];
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
      process.stderr.write(
        'Skipping a directory without a Cargo marker: ' + relativeTarget + '.\n',
      );
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

function maintainTarget(args) {
  const options = parseMaintainTargetOptions(args);
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
    process.stdout.write(
      'Previewing Cargo cleanup for ' + path.relative(workspaceRoot, target.path) + '.\n',
    );
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
    process.stdout.write(
      'Cleaning Cargo target directory ' + path.relative(workspaceRoot, target.path) + '.\n',
    );
    runCommand(
      'cargo',
      ['clean', '--manifest-path', manifestPath, '--target-dir', target.path, '--locked'],
      { cwd: workspaceRoot },
    );
  }
}

function printHelp() {
  process.stdout.write(
    'Usage: npm run tasks -- <dev|build|check|clean|maintain-target> [options]\n' +
      'Run from apps/desktop/frontend. Use --help after a task for its options.\n',
  );
}

function main() {
  const [taskName, ...args] = process.argv.slice(2);
  if (!taskName || taskName === '--help' || taskName === '-h') {
    printHelp();
    return;
  }

  const tasks = {
    dev,
    build,
    check,
    clean,
    'maintain-target': maintainTarget,
  };
  const task = tasks[taskName];
  if (!task) {
    throw new Error('Unknown task: ' + taskName + '. Run npm run tasks -- --help for usage.');
  }

  task(args);
}

runTask(main);
