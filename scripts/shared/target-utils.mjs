import { readdirSync, statSync, realpathSync, existsSync, lstatSync, rmSync } from 'node:fs';
import path from 'node:path';
import { isWithinDirectory } from './fs-safety.mjs';
import { getWorkspaceRoot, runCommand } from './runner.mjs';

const gibibyte = 1024n ** 3n;

export function directoryBytes(root) {
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

export function discoverTargets(workspaceRoot = getWorkspaceRoot()) {
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
      process.stderr.write(`Skipping symbolic-link target directory ${relativeTarget}.\n`);
      continue;
    }
    if (!metadata.isDirectory()) continue;

    const targetPath = realpathSync(candidate);
    if (!isWithinDirectory(canonicalRoot, targetPath)) {
      throw new Error('A Cargo target path resolves outside the workspace.');
    }

    const cargoMarker = existsSync(path.join(targetPath, '.rustc_info.json')) || existsSync(path.join(targetPath, 'CACHEDIR.TAG'));
    if (!cargoMarker) {
      process.stderr.write(`Skipping a directory without a Cargo marker: ${relativeTarget}.\n`);
      continue;
    }

    targets.push({ path: targetPath, bytes: directoryBytes(targetPath) });
  }

  return targets;
}

export function formatGiB(bytes) {
  return (Number(bytes) / Number(gibibyte)).toFixed(2);
}

export function pruneStaleArtifacts(targetPath, apply = false, beforeRemove = () => undefined, expectedPaths) {
  let prunedBytes = 0n;
  const prunedPaths = [];
  const targetMetadata = lstatSync(targetPath);
  if (!targetMetadata.isDirectory() || targetMetadata.isSymbolicLink()) {
    throw new Error('Refusing to prune a non-directory or symbolic-link Cargo target.');
  }
  const canonicalTarget = realpathSync(targetPath);

  const candidateDirs = [
    resolveTargetDirectory(targetPath, canonicalTarget, path.join('debug', 'deps')),
    resolveTargetDirectory(targetPath, canonicalTarget, 'deps'),
  ];

  for (const depsDir of candidateDirs) {
    if (depsDir === null) continue;
    const entries = readdirSync(depsDir);

    const groups = new Map();
    for (const entry of entries) {
      const match = /^(.+)-([0-9a-fA-F]{16})\.([a-zA-Z0-9_]+)$/u.exec(entry);
      if (!match) continue;
      const key = `${match[1]}.${match[3]}`;
      const filePath = path.join(depsDir, entry);
      const stats = lstatSync(filePath);
      if (!stats.isFile() || stats.isSymbolicLink()) continue;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push({ path: filePath, mtimeMs: stats.mtimeMs, size: BigInt(stats.size) });
    }

    for (const [, items] of groups) {
      if (items.length > 1) {
        items.sort((a, b) => b.mtimeMs - a.mtimeMs || a.path.localeCompare(b.path));
        for (let i = 1; i < items.length; i += 1) {
          const item = items[i];
          prunedPaths.push(item.path);
          prunedBytes += item.size;
        }
      }
    }
  }

  const buildDirs = [
    resolveTargetDirectory(targetPath, canonicalTarget, path.join('debug', 'build')),
    resolveTargetDirectory(targetPath, canonicalTarget, 'build'),
  ];

  for (const buildDir of buildDirs) {
    if (buildDir === null) continue;
    const entries = readdirSync(buildDir);
    const groups = new Map();
    for (const entry of entries) {
      const match = /^(.+)-([0-9a-fA-F]{16})$/u.exec(entry);
      if (!match) continue;
      const key = match[1];
      if (key === 'openssl-sys') continue;
      const dirPath = path.join(buildDir, entry);
      const stats = lstatSync(dirPath);
      if (!stats.isDirectory() || stats.isSymbolicLink()) continue;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push({ path: dirPath, mtimeMs: stats.mtimeMs });
    }
    for (const [, items] of groups) {
      if (items.length > 1) {
        items.sort((a, b) => b.mtimeMs - a.mtimeMs || a.path.localeCompare(b.path));
        for (let i = 1; i < items.length; i += 1) {
          const item = items[i];
          prunedPaths.push(item.path);
          prunedBytes += directoryBytes(item.path);
        }
      }
    }
  }

  if (apply) {
    if (expectedPaths) {
      const expected = [...expectedPaths].sort();
      const discovered = [...prunedPaths].sort();
      if (expected.length !== discovered.length || expected.some((candidate, index) => candidate !== discovered[index])) {
        throw new Error('Stale Cargo artifacts changed after preview. Run maintain-target again to review the current artifacts.');
      }
    }
    for (const candidatePath of prunedPaths) {
      beforeRemove();
      const relativePath = path.relative(canonicalTarget, candidatePath);
      if (
        relativePath === '' ||
        relativePath === '..' ||
        relativePath.startsWith('..' + path.sep) ||
        path.isAbsolute(relativePath)
      ) {
        throw new Error('Refusing to prune a path outside the Cargo target directory.');
      }
      const canonicalParent = realpathSync(path.dirname(candidatePath));
      if (!isWithinDirectory(canonicalTarget, canonicalParent)) {
        throw new Error('Refusing to prune a path outside the Cargo target directory.');
      }
      const currentMetadata = lstatSync(candidatePath);
      if (currentMetadata.isSymbolicLink() || (!currentMetadata.isFile() && !currentMetadata.isDirectory())) {
        throw new Error('Refusing to prune a changed Cargo target artifact.');
      }
      rmSync(candidatePath, { recursive: currentMetadata.isDirectory() });
    }
  }

  return { prunedBytes, prunedCount: prunedPaths.length, prunedPaths };
}

function resolveTargetDirectory(targetPath, canonicalTarget, relativePath) {
  let candidatePath = targetPath;
  for (const segment of relativePath.split(path.sep)) {
    candidatePath = path.join(candidatePath, segment);
    if (!existsSync(candidatePath)) return null;

    const metadata = lstatSync(candidatePath);
    if (metadata.isSymbolicLink()) {
      throw new Error(`Refusing to prune artifacts through symbolic-link directory '${relativePath}'.`);
    }
    if (!metadata.isDirectory()) return null;
  }

  const canonicalPath = realpathSync(candidatePath);
  if (!isWithinDirectory(canonicalTarget, canonicalPath)) {
    throw new Error('Refusing to prune a path outside the Cargo target directory.');
  }
  return canonicalPath;
}

export function activeBuildIsRunning() {
  if (process.platform === 'win32') {
    const result = runCommand('tasklist.exe', ['/FO', 'CSV', '/NH'], { captureOutput: true });
    return (result.stdout ?? '').split(/\r?\n/u).some((line) => {
      const processName = /^"([^"]+)"/u.exec(line)?.[1]?.toLowerCase();
      return processName === 'cargo.exe' || processName === 'rustc.exe';
    });
  }

  if (process.platform === 'linux' || process.platform === 'darwin') {
    const result = runCommand('ps', ['-A', '-o', 'comm='], { captureOutput: true });
    return (result.stdout ?? '').split(/\r?\n/u).some((line) => {
      const processName = line.trim().toLowerCase();
      return processName === 'cargo' || processName === 'rustc';
    });
  }

  throw new Error('Active Rust build detection is not supported on this operating system.');
}
