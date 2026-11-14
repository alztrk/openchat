import { getTauriDirectory, getWorkspaceRoot, resolveNpmCliPath, runCommand, runNpm } from './runner.mjs';
import { runInterruptibleCommand } from './session.mjs';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';

const frontendWorkspaceName = 'openchat-frontend';
const frontendDependencyCheckArgs = ['ls', '--workspace', frontendWorkspaceName, '--all', '--parseable', '--silent'];

export function rustBuildEnvironment({
  environment = process.env,
  platform = process.platform,
  fileExists = existsSync,
} = {}) {
  if (platform !== 'win32' || environment.CMAKE) return environment;

  const programFiles = environment.ProgramFiles ?? 'C:\\Program Files';
  const cmakeExecutable = path.win32.join(programFiles, 'CMake', 'bin', 'cmake.exe');
  return fileExists(cmakeExecutable) ? { ...environment, CMAKE: cmakeExecutable } : environment;
}

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
  const source = hostLocalServiceArtifactPath(getWorkspaceRoot(), profile, targetTriple);
  const destinationDirectory = path.join(getTauriDirectory(), 'binaries');
  const destination = path.join(destinationDirectory, `openchat-local-service-${targetTriple}${extension}`);
  mkdirSync(destinationDirectory, { recursive: true });
  copyFileSync(source, destination);
  process.stdout.write(`Prepared the local-service sidecar for ${targetTriple}.\n`);
}

export function hostLocalServiceArtifactPath(workspaceRoot, profile, targetTriple) {
  const extension = targetTriple.split('-').includes('windows') ? '.exe' : '';
  // Host builds use Cargo's shared target/<profile> cache; the triple is only needed for Tauri's filename.
  return path.join(workspaceRoot, 'target', profile, `openchat-local-service${extension}`);
}

export function localServiceBuildArgs(profile, locked, features = []) {
  if (profile !== 'debug' && profile !== 'release') {
    throw new Error(`Invalid build profile: ${profile}`);
  }

  const args = ['build', '-p', 'openchat-local-service', '--bin', 'openchat-local-service'];
  if (profile === 'release') args.push('--release');
  if (locked) args.push('--locked');
  if (features.length > 0) args.push('--features', features.join(','));
  return args;
}

export function buildLocalService(profile, locked, features = []) {
  const args = localServiceBuildArgs(profile, locked, features);
  const targetTriple = rustHostTargetTriple();
  runCommand('cargo', args, { env: rustBuildEnvironment() });
  installLocalServiceSidecar(profile, targetTriple);
}

export async function buildLocalServiceInterruptibly(profile, locked, features, interruptController) {
  const args = localServiceBuildArgs(profile, locked, features);
  const targetTriple = await rustHostTargetTripleInterruptibly(interruptController);
  interruptController.throwIfInterrupted();
  await runInterruptibleCommand('cargo', args, {
    env: rustBuildEnvironment(),
    interruptController,
  });
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
