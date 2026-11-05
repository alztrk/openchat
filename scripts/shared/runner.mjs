import { copyFileSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const workspaceRoot = path.resolve(__dirname, '..', '..');
const frontendDirectory = path.join(workspaceRoot, 'apps', 'desktop', 'frontend');
const tauriDirectory = path.join(workspaceRoot, 'apps', 'desktop', 'tauri');

export function runCommand(
  command,
  args,
  { cwd = workspaceRoot, captureOutput = false, env = process.env, allowFailure = false } = {},
) {
  const result = spawnSync(command, args, {
    cwd,
    env,
    encoding: 'utf8',
    stdio: captureOutput ? 'pipe' : 'inherit',
    windowsHide: true,
  });

  if (result.error) {
    throw new Error(`Could not start ${command}: ${result.error.message}`, { cause: result.error });
  }

  if (result.status !== 0 && (!allowFailure || result.status === null)) {
    const outcome = result.status === null ? `signal ${result.signal ?? 'unknown'}` : `exit code ${result.status}`;
    throw new Error(`${command} failed with ${outcome}.`);
  }

  return result;
}

export function resolveNpmCliPath({
  npmExecPath = process.env.npm_execpath,
  execPath = process.execPath,
  platform = process.platform,
  fileExists = existsSync,
} = {}) {
  if (npmExecPath) return npmExecPath;

  if (platform === 'win32') {
    const windowsPath = path.win32;
    const bundledNpmCliPath = windowsPath.join(
      windowsPath.dirname(execPath),
      'node_modules',
      'npm',
      'bin',
      'npm-cli.js',
    );
    if (fileExists(bundledNpmCliPath)) return bundledNpmCliPath;
  }

  throw new Error('Run this task through npm, or install npm where this Node.js installation can find it.');
}

export function runNpm(args, cwd = frontendDirectory, options = {}) {
  const npmCliPath = resolveNpmCliPath();
  return runCommand(process.execPath, [npmCliPath, ...args], { ...options, cwd });
}

export function getFrontendDirectory() { return frontendDirectory; }
export function getWorkspaceRoot() { return workspaceRoot; }
export function getTauriDirectory() { return tauriDirectory; }
