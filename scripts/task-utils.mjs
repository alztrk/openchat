import { copyFileSync, mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));

export const workspaceRoot = path.resolve(scriptDirectory, '..');
export const frontendDirectory = path.join(workspaceRoot, 'apps', 'desktop', 'frontend');
export const tauriDirectory = path.join(workspaceRoot, 'apps', 'desktop', 'tauri');

export function runCommand(command, args, { cwd = workspaceRoot, captureOutput = false } = {}) {
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

export function runNpm(args, cwd = frontendDirectory) {
  const npmCliPath = process.env.npm_execpath;
  if (!npmCliPath) {
    throw new Error('Run this task through its npm script so it can invoke npm without a shell.');
  }

  return runCommand(process.execPath, [npmCliPath, ...args], { cwd });
}

export function rustHostTargetTriple() {
  const result = runCommand('rustc', ['-vV'], { captureOutput: true });
  const match = /^host: ([^\r\n]+)$/mu.exec(result.stdout ?? '');
  if (!match?.[1]) {
    throw new Error('rustc did not report a host target triple.');
  }
  return match[1];
}

export function sidecarFileName(targetTriple) {
  const extension = targetTriple.split('-').includes('windows') ? '.exe' : '';
  return 'openchat-local-service-' + targetTriple + extension;
}

export function buildLocalService(profile, locked) {
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
  const destination = path.join(destinationDirectory, sidecarFileName(targetTriple));

  mkdirSync(destinationDirectory, { recursive: true });
  copyFileSync(source, destination);
  process.stdout.write('Prepared the local-service sidecar for ' + targetTriple + '.\n');
  return targetTriple;
}

export function isWithinDirectory(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return (
    relative !== '' &&
    relative !== '..' &&
    !relative.startsWith('..' + path.sep) &&
    !path.isAbsolute(relative)
  );
}

export function runTask(task) {
  try {
    task();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(message + '\n');
    process.exitCode = 1;
  }
}
