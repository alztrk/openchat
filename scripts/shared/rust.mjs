import { getTauriDirectory, getWorkspaceRoot, resolveNpmCliPath, runCommand, runNpm } from './runner.mjs';
import { runInterruptibleCommand } from './session.mjs';
import { mkdirSync, copyFileSync } from 'node:fs';
import path from 'node:path';

const frontendWorkspaceName = 'openchat-frontend';
const frontendDependencyCheckArgs = ['ls', '--workspace', frontendWorkspaceName, '--all', '--parseable', '--silent'];

export function rustHostTargetTriple() {
  const result = runCommand('rustc', ['-vV'], { captureOutput: true });
  const match = /^host: ([^\r\n]+)$/mu.exec(result.stdout ?? '');
  if (!match?.[1]) {
    throw new Error('rustc did not report a host target triple.');
  }
  return match[1];
}

export async function rustHostTargetTripleInterruptibly(interruptController) {
  const result = await runInterruptibleCommand('rustc', ['-vV'], {
    captureOutput: true,
    interruptController,
  });
  const match = /^host: ([^\r\n]+)$/mu.exec(result.stdout ?? '');
  if (!match?.[1]) {
    throw new Error('rustc did not report a host target triple.');
  }
  return match[1];
}

export function installLocalServiceSidecar(profile, targetTriple) {
  const extension = targetTriple.split('-').includes('windows') ? '.exe' : '';
  const source = path.join(getWorkspaceRoot(), 'target', targetTriple, profile, `openchat-local-service${extension}`);
  const destinationDirectory = path.join(getTauriDirectory(), 'binaries');
  const destination = path.join(destinationDirectory, `openchat-local-service-${targetTriple}${extension}`);
  mkdirSync(destinationDirectory, { recursive: true });
  copyFileSync(source, destination);
  process.stdout.write(`Prepared the local-service sidecar for ${targetTriple}.\n`);
}

export function buildLocalService(profile, locked, features = []) {
  if (profile !== 'debug' && profile !== 'release') {
    throw new Error(`Invalid build profile: ${profile}`);
  }
  const targetTriple = rustHostTargetTriple();
  const args = [
    'build', '--target', targetTriple,
    '-p', 'openchat-local-service', '--bin', 'openchat-local-service',
  ];
  if (profile === 'release') args.push('--release');
  if (locked) args.push('--locked');
  if (features.length > 0) args.push('--features', features.join(','));
  runCommand('cargo', args);
  installLocalServiceSidecar(profile, targetTriple);
}

export async function buildLocalServiceInterruptibly(profile, locked, features, interruptController) {
  if (profile !== 'debug' && profile !== 'release') {
    throw new Error(`Invalid build profile: ${profile}`);
  }
  const targetTriple = await rustHostTargetTripleInterruptibly(interruptController);
  interruptController.throwIfInterrupted();
  const args = [
    'build', '--target', targetTriple,
    '-p', 'openchat-local-service', '--bin', 'openchat-local-service',
  ];
  if (profile === 'release') args.push('--release');
  if (locked) args.push('--locked');
  if (features.length > 0) args.push('--features', features.join(','));
  await runInterruptibleCommand('cargo', args, { interruptController });
  installLocalServiceSidecar(profile, targetTriple);
}

export function ensureFrontendDependencies({
  workspaceRoot = getWorkspaceRoot(),
  runNpmFn = runNpm,
  write = (message) => process.stdout.write(message),
} = {}) {
  const result = runNpmFn(frontendDependencyCheckArgs, workspaceRoot, { captureOutput: true, allowFailure: true });
  if (result.status === 0) return;

  write('Frontend workspace dependencies are incomplete; installing from the workspace root.\n');
  runNpmFn(['ci'], workspaceRoot);
}

export async function ensureFrontendDependenciesInterruptibly(
  interruptController,
  {
    workspaceRoot = getWorkspaceRoot(),
    npmCliPath = resolveNpmCliPath(),
    runInterruptibleCommandFn = runInterruptibleCommand,
    write = (message) => process.stdout.write(message),
  } = {},
) {
  const result = await runInterruptibleCommandFn(
    process.execPath,
    [npmCliPath, ...frontendDependencyCheckArgs],
    { cwd: workspaceRoot, captureOutput: true, interruptController, allowFailure: true },
  );
  if (result.status === 0) return;

  write('Frontend workspace dependencies are incomplete; installing from the workspace root.\n');
  await runInterruptibleCommandFn(process.execPath, [npmCliPath, 'ci'], { cwd: workspaceRoot, interruptController });
}
