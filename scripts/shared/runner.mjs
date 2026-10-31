import { copyFileSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const workspaceRoot = path.resolve(__dirname, '..');
const frontendDirectory = path.join(workspaceRoot, 'apps', 'desktop', 'frontend');
const tauriDirectory = path.join(workspaceRoot, 'apps', 'desktop', 'tauri');

export function runCommand(command, args, { cwd = workspaceRoot, captureOutput = false } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: 'utf8',
    stdio: captureOutput ? 'pipe' : 'inherit',
    windowsHide: true,
  });

  if (result.error) {
    throw new Error(`Could not start ${command}: ${result.error.message}`, { cause: result.error });
  }

  if (result.status !== 0) {
    const outcome = result.status === null ? `signal ${result.signal ?? 'unknown'}` : `exit code ${result.status}`;
    throw new Error(`${command} failed with ${outcome}.`);
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

export function getFrontendDirectory() { return frontendDirectory; }
export function getWorkspaceRoot() { return workspaceRoot; }
export function getTauriDirectory() { return tauriDirectory; }
