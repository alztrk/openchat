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

export function discoverTargets() {
  const workspaceRoot = getWorkspaceRoot();
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

export function pruneStaleArtifacts(targetPath, apply = false) {
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
            try { rmSync(items[i].path, { force: true }); } catch { /* Ignore */ }
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
      if (key === 'openssl-sys') continue;
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
            try { rmSync(items[i].path, { recursive: true, force: true }); } catch { /* Ignore */ }
          }
        }
      }
    }
  }

  if (path.basename(targetPath) === 'target') {
    try {
      const entries = readdirSync(targetPath);
      const allowed = new Set(['debug', 'CACHEDIR.TAG', '.rustc_info.json', '.rustdoc_fingerprint.json', 'cxxbridge', 'tmp', 'doc', 'flycheck0']);
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
