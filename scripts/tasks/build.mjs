import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { assertCleanReleaseTree, parseBuildChannel, resolveReleaseIdentity } from '../shared/release-identity.mjs';
import { getTauriDirectory, getWorkspaceRoot, runCommand } from '../shared/runner.mjs';
import { buildLocalService } from '../shared/rust.mjs';

export function readApplicationBaseVersion(workspaceRoot, tauriDirectory) {
  const tauriConfig = JSON.parse(readFileSync(path.join(tauriDirectory, 'tauri.conf.json'), 'utf8'));
  const desktopCargo = readFileSync(path.join(tauriDirectory, 'Cargo.toml'), 'utf8');
  const cargoLock = readFileSync(path.join(workspaceRoot, 'Cargo.lock'), 'utf8');
  const frontendPackage = JSON.parse(
    readFileSync(path.join(tauriDirectory, '..', 'frontend', 'package.json'), 'utf8'),
  );
  const frontendLock = JSON.parse(readFileSync(path.join(workspaceRoot, 'package-lock.json'), 'utf8'));
  const cargoVersion = /^version\s*=\s*"([^"]+)"/mu.exec(desktopCargo)?.[1];
  const cargoLockVersion = /^name\s*=\s*"openchat-desktop"\s*\r?\nversion\s*=\s*"([^"]+)"/mu.exec(cargoLock)?.[1];
  const frontendLockVersion = frontendLock.packages?.['apps/desktop/frontend']?.version;

  if (
    !tauriConfig.version ||
    cargoVersion !== tauriConfig.version ||
    cargoLockVersion !== tauriConfig.version ||
    frontendPackage.version !== tauriConfig.version ||
    frontendLockVersion !== tauriConfig.version
  ) {
    throw new Error('Tauri, Rust, and frontend manifest/lockfile versions must match before packaging.');
  }

  return tauriConfig.version;
}

function readGitValue(workspaceRoot, args) {
  const result = runCommand('git', args, { cwd: workspaceRoot, captureOutput: true });
  return result.stdout.trim();
}

function releaseIdentity(workspaceRoot, tauriDirectory, channel) {
  const dirtyState = readGitValue(workspaceRoot, ['status', '--porcelain', '--untracked-files=all']);
  assertCleanReleaseTree(dirtyState);

  const commitCount = Number(readGitValue(workspaceRoot, ['rev-list', '--count', 'HEAD']));
  const commitSha = readGitValue(workspaceRoot, ['rev-parse', '--short=12', 'HEAD']);
  const tagResult = runCommand('git', ['describe', '--tags', '--exact-match', 'HEAD'], {
    cwd: workspaceRoot,
    captureOutput: true,
    allowFailure: true,
  });
  const tag = tagResult.status === 0 ? tagResult.stdout.trim() : null;

  return resolveReleaseIdentity({
    channel,
    baseVersion: readApplicationBaseVersion(workspaceRoot, tauriDirectory),
    commitCount,
    commitSha,
    tag,
  });
}

export function resolveWindowsSigningConfig(channel, platform, thumbprint) {
  if (platform !== 'win32' || channel !== 'stable') return null;

  const normalizedThumbprint = thumbprint?.replaceAll(/\s/gu, '').toUpperCase();
  if (!normalizedThumbprint || !/^[A-F0-9]{40}$/u.test(normalizedThumbprint)) {
    throw new Error('Stable Windows packages require an imported Authenticode signing certificate.');
  }

  return {
    bundle: {
      windows: {
        certificateThumbprint: normalizedThumbprint,
        digestAlgorithm: 'sha256',
        timestampUrl: 'http://timestamp.digicert.com',
        tsp: true,
      },
    },
  };
}

export const task = {
  name: 'build',
  description: 'Builds a uniquely identified preview or stable package for the host OS.',
  args: [
    { name: '--preview', description: 'Build a preview package identified by commit count and SHA' },
    { name: '--stable', description: 'Build a stable package from the matching vMAJOR.MINOR.PATCH tag' },
  ],
  run(args) {
    const channel = parseBuildChannel(args);
    const workspaceRoot = getWorkspaceRoot();
    const tauriDirectory = getTauriDirectory();
    const identity = releaseIdentity(workspaceRoot, tauriDirectory, channel);
    const windowsSigningConfig = resolveWindowsSigningConfig(
      identity.channel,
      process.platform,
      process.env.OPENCHAT_WINDOWS_SIGNING_THUMBPRINT,
    );
    const releaseConfig = {
      version: identity.version,
      ...(windowsSigningConfig ?? {}),
    };
    const temporaryDirectory = mkdtempSync(path.join(os.tmpdir(), 'openchat-release-config-'));
    const configPath = path.join(temporaryDirectory, 'release-config.json');

    try {
      writeFileSync(configPath, JSON.stringify(releaseConfig), { encoding: 'utf8', flag: 'wx' });
      process.stdout.write(`Building ${identity.channel} ${identity.version} (${identity.buildId}).\n`);
      buildLocalService('release', true);
      const tauriArgs = [];
      if (process.platform === 'linux') tauriArgs.push('--config', 'tauri.linux.conf.json');
      tauriArgs.push('--config', configPath);
      const tauriCliPath = path.join(workspaceRoot, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
      runCommand(process.execPath, [tauriCliPath, 'build', ...tauriArgs], { cwd: tauriDirectory });
    } finally {
      rmSync(temporaryDirectory, { recursive: true, force: true });
    }
  },
};
