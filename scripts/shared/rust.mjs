import { runCommand } from './runner.mjs';
import { runInterruptibleCommand } from './session.mjs';
import { mkdirSync, copyFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { getWorkspaceRoot, getTauriDirectory, getFrontendDirectory, runNpm } from './runner.mjs';

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

export function ensureFrontendDependencies() {
  if (!existsSync(path.join(getFrontendDirectory(), 'node_modules'))) {
    runNpm(['ci'], getFrontendDirectory());
  }
}

export async function ensureFrontendDependenciesInterruptibly(interruptController) {
  if (!existsSync(path.join(getFrontendDirectory(), 'node_modules'))) {
    const npmCliPath = process.env.npm_execpath;
    if (!npmCliPath) {
      throw new Error('Run this task through its npm script so it can invoke npm without a shell.');
    }
    await runInterruptibleCommand(process.execPath, [npmCliPath, 'ci'], {
      cwd: getFrontendDirectory(),
      interruptController,
    });
  }
}
