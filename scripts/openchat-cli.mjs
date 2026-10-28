import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { reserveTemporaryTestProfileRoot } from './temporaryTestProfile.mjs';

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

function buildLocalService(profile, locked, features = []) {
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
  if (features.length > 0) args.push('--features', features.join(','));
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

  ensureFrontendDependencies();
  runNpm(['run', 'tauri:dev'], frontendDirectory);
}

function devTestProfile(args) {
  if (args.length === 1 && (args[0] === '--help' || args[0] === '-h')) {
    process.stdout.write(
      'Usage: npm run openchat -- dev-test-profile [--cleanup-profile] [--profile-root <absolute-temp-path>]\n' +
        'Creates a UUID-named OS temporary profile when --profile-root is omitted.\n' +
        '--cleanup-profile removes only an automatically created profile after the app exits.\n',
    );
    return;
  }

  let cleanupProfileOnExit = false;
  let explicitProfileRoot;
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--cleanup-profile' && !cleanupProfileOnExit) {
      cleanupProfileOnExit = true;
      continue;
    }
    if (argument === '--profile-root' && explicitProfileRoot === undefined) {
      const value = args[index + 1];
      if (!value || !path.isAbsolute(value)) {
        throw new Error('--profile-root requires an absolute path.');
      }
      explicitProfileRoot = path.normalize(value);
      index += 1;
      continue;
    }
    throw new Error(
      'Use --cleanup-profile and/or --profile-root with an absolute path; cleanup requires an automatically created profile.',
    );
  }

  if (cleanupProfileOnExit && explicitProfileRoot !== undefined) {
    throw new Error('--cleanup-profile cannot be used with --profile-root.');
  }
  const profileRoot = explicitProfileRoot ?? reserveTemporaryTestProfileRoot();

  let launchError;
  try {
    buildLocalService('debug', false, ['desktop-profile-test-support']);
    ensureFrontendDependencies();
    process.stdout.write('Isolated desktop test profile: ' + profileRoot + '\n');
    runNpm(
      [
        'run',
        'tauri:dev',
        '--',
        '--features',
        'desktop-profile-test-support',
        '--',
        '--',
        '--desktop-test-profile-root',
        profileRoot,
      ],
      frontendDirectory,
    );
  } catch (error) {
    launchError = error;
  }

  if (cleanupProfileOnExit) {
    try {
      removeTemporaryTestProfile(profileRoot);
    } catch (cleanupError) {
      if (launchError) {
        throw new AggregateError([launchError, cleanupError], 'Desktop test and profile cleanup failed.');
      }
      throw cleanupError;
    }
  }
  if (launchError) throw launchError;
}

function startupBenchmark(args) {
  if (args.length === 1 && (args[0] === '--help' || args[0] === '-h')) {
    process.stdout.write('Usage: npm run openchat -- startup-benchmark\n');
    return;
  }
  ensureNoArguments('startup-benchmark', args);

  const profileRoot = reserveTemporaryTestProfileRoot();
  const resultPath = path.join(profileRoot, 'result.json');
  let benchmarkError;

  try {
    buildLocalService('debug', false, ['desktop-profile-test-support']);
    ensureFrontendDependencies();
    runNpm(
      [
        'run',
        'tauri:dev',
        '--',
        '--features',
        'startup-benchmark',
        '--',
        '--',
        '--startup-benchmark-root',
        profileRoot,
      ],
      frontendDirectory,
    );

    const result = JSON.parse(readFileSync(resultPath, 'utf8'));
    const elapsedMs = result?.processToReactReadyMs;
    const renderedRootClass = result?.performanceSnapshot?.renderedRootClass;
    if (
      typeof elapsedMs !== 'number' ||
      !Number.isFinite(elapsedMs) ||
      elapsedMs <= 0 ||
      typeof renderedRootClass !== 'string' ||
      !renderedRootClass.split(/\s+/u).includes('app-shell') ||
      renderedRootClass.includes('startup-error-page')
    ) {
      throw new Error('The desktop did not reach the OpenChat application shell.');
    }

    const databasePath = path.join(profileRoot, 'storage', 'db', 'openchat.sqlite3');
    if (!existsSync(databasePath)) {
      throw new Error('The benchmark profile did not create the active database.');
    }
    const databaseHeader = readFileSync(databasePath).subarray(0, 16);
    if (
      databaseHeader.length !== 16 ||
      databaseHeader.equals(Buffer.from('SQLite format 3\0', 'ascii'))
    ) {
      throw new Error('The benchmark profile database is missing or plaintext SQLite.');
    }

    process.stdout.write(
      'Debug startup to React ready: ' + elapsedMs.toFixed(1) + ' ms; app shell and encrypted database verified.\n',
    );
  } catch (error) {
    benchmarkError = error;
  }

  try {
    removeTemporaryTestProfile(profileRoot);
  } catch (cleanupError) {
    if (benchmarkError) {
      throw new AggregateError([benchmarkError, cleanupError], 'Startup benchmark and cleanup failed.');
    }
    throw cleanupError;
  }

  if (benchmarkError) throw benchmarkError;
}

function removeTemporaryTestProfile(profileRoot) {
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
    !/^openchat-desktop-test-[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u.test(
      profileName,
    )
  ) {
    throw new Error('Refusing to remove a profile outside the desktop test temporary scope.');
  }

  rmSync(canonicalProfileRoot, { recursive: true });
}

function ensureFrontendDependencies() {
  if (!existsSync(path.join(frontendDirectory, 'node_modules'))) {
    runNpm(['ci'], frontendDirectory);
  }
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
  runCommand('cargo', ['test', '--workspace', '--features', 'openchat-local-service/desktop-profile-test-support', '--', '--test-threads=1'], {
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
        'Usage: npm run openchat -- maintain-target [--max-gib 1..1024] [--apply]\n' +
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

function pruneStaleArtifacts(targetPath, apply = false) {
  let prunedBytes = 0n;
  let prunedCount = 0;

  const candidateDirs = [
    path.join(targetPath, 'debug', 'deps'),
    path.join(targetPath, 'deps'),
  ];

  for (const depsDir of candidateDirs) {
    if (!existsSync(depsDir)) continue;
    let entries = [];
    try {
      entries = readdirSync(depsDir);
    } catch {
      continue;
    }

    const groups = new Map();
    for (const entry of entries) {
      const match = /^(.+)-([0-9a-fA-F]{16})\.([a-zA-Z0-9_]+)$/u.exec(entry);
      if (!match) continue;
      const key = `${match[1]}.${match[3]}`;
      const filePath = path.join(depsDir, entry);
      try {
        const stats = statSync(filePath);
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key).push({ path: filePath, mtimeMs: stats.mtimeMs, size: BigInt(stats.size) });
      } catch {
        // Skip inaccessible entries
      }
    }

    for (const [, items] of groups) {
      if (items.length > 1) {
        items.sort((a, b) => b.mtimeMs - a.mtimeMs);
        for (let i = 1; i < items.length; i += 1) {
          prunedBytes += items[i].size;
          prunedCount += 1;
          if (apply) {
            try {
              rmSync(items[i].path, { force: true });
            } catch {
              // Ignore
            }
          }
        }
      }
    }
  }

  const buildDirs = [
    path.join(targetPath, 'debug', 'build'),
    path.join(targetPath, 'build'),
  ];
  for (const buildDir of buildDirs) {
    if (!existsSync(buildDir)) continue;
    let entries = [];
    try {
      entries = readdirSync(buildDir);
    } catch {
      continue;
    }
    const groups = new Map();
    for (const entry of entries) {
      const match = /^(.+)-([0-9a-fA-F]{16})$/u.exec(entry);
      if (!match) continue;
      const key = match[1];
      if (key === "openssl-sys") continue;
      const dirPath = path.join(buildDir, entry);
      try {
        const stats = statSync(dirPath);
        if (stats.isDirectory()) {
          if (!groups.has(key)) groups.set(key, []);
          groups.get(key).push({ path: dirPath, mtimeMs: stats.mtimeMs });
        }
      } catch {
        // Skip
      }
    }
    for (const [, items] of groups) {
      if (items.length > 1) {
        items.sort((a, b) => b.mtimeMs - a.mtimeMs);
        for (let i = 1; i < items.length; i += 1) {
          if (apply) {
            try {
              rmSync(items[i].path, { recursive: true, force: true });
            } catch {
              // Ignore
            }
          }
        }
      }
    }
  }

  if (path.basename(targetPath) === 'target') {
    try {
      const entries = readdirSync(targetPath);
      const allowed = new Set([
        'debug',
        'CACHEDIR.TAG',
        '.rustc_info.json',
        '.rustdoc_fingerprint.json',
        'cxxbridge',
        'tmp',
        'doc',
        'flycheck0',
      ]);
      for (const entry of entries) {
        if (allowed.has(entry)) continue;
        const candidate = path.join(targetPath, entry);
        try {
          const stats = statSync(candidate);
          if (stats.isDirectory()) {
            prunedBytes += directoryBytes(candidate);
            prunedCount += 1;
            if (apply) {
              rmSync(candidate, { recursive: true, force: true });
            }
          }
        } catch {
          // Ignore
        }
      }
    } catch {
      // Ignore
    }
  }

  return { prunedBytes, prunedCount };
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

  for (const target of targets) {
    const { prunedBytes, prunedCount } = pruneStaleArtifacts(target.path, options.apply);
    if (prunedBytes > 0n || prunedCount > 0) {
      const mode = options.apply ? 'Pruned' : 'Previewed pruning for';
      process.stdout.write(
        `${mode} ${prunedCount} stale artifact${prunedCount === 1 ? '' : 's'} (${formatGiB(prunedBytes)} GiB) in ${path.relative(workspaceRoot, target.path)}.\n`,
      );
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
    'Usage: npm run openchat -- <dev|dev-test-profile|startup-benchmark|build|check|clean|maintain-target> [options]\n' +
      'Run from apps/desktop/frontend. Use --help after a task for its options.\n',
  );
}

function ensureOpenSslEnvironment() {
  if (process.env.OPENSSL_DIR) return;
  if (process.platform === 'win32') {
    const candidates = [
      'D:\\tools\\openssl',
      path.join(workspaceRoot, 'vendor', 'openssl-windows-x64'),
      path.join(process.env.ProgramFiles ?? 'C:\\Program Files', 'OpenSSL-Win64'),
      'C:\\OpenSSL-Win64',
    ];
    for (const candidate of candidates) {
      if (
        existsSync(path.join(candidate, 'include', 'openssl', 'ssl.h')) &&
        (existsSync(path.join(candidate, 'lib', 'libcrypto.lib')) ||
          existsSync(path.join(candidate, 'lib', 'VC', 'static', 'libcrypto64MT.lib')))
      ) {
        process.env.OPENSSL_DIR = candidate;
        process.env.OPENSSL_NO_VENDOR = '1';
        break;
      }
    }
  }
}

function main() {
  ensureOpenSslEnvironment();
  const [taskName, ...args] = process.argv.slice(2);
  if (!taskName || taskName === '--help' || taskName === '-h') {
    printHelp();
    return;
  }

  const tasks = {
    dev,
    'dev-test-profile': devTestProfile,
    'startup-benchmark': startupBenchmark,
    build,
    check,
    clean,
    'maintain-target': maintainTarget,
  };
  const task = tasks[taskName];
  if (!task) {
    throw new Error('Unknown task: ' + taskName + '. Run npm run openchat -- --help for usage.');
  }

  task(args);
}

runTask(main);
