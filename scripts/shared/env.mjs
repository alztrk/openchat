import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { getWorkspaceRoot } from './runner.mjs';

const localEnvTasks = new Set(['build', 'dev']);

export function taskLoadsLocalEnv(taskName) {
  return localEnvTasks.has(taskName);
}

export function loadLocalEnv() {
  const envPath = path.join(getWorkspaceRoot(), '.env');
  if (!existsSync(envPath)) return;
  try {
    const content = readFileSync(envPath, 'utf8');
    for (const line of content.split(/\r?\n/u)) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('#')) continue;
      const eqIdx = trimmed.indexOf('=');
      if (eqIdx !== -1) {
        const key = trimmed.slice(0, eqIdx).trim();
        const val = trimmed.slice(eqIdx + 1).trim().replace(/^["']|["']$/gu, '');
        if (!process.env[key]) {
          process.env[key] = val;
        }
      }
    }
  } catch {
    // Ignore
  }
}

export function ensureOpenSSLEnvironment({
  environment = process.env,
  platform = process.platform,
  fileExists = existsSync,
  workspaceRoot = getWorkspaceRoot(),
} = {}) {
  if (platform !== 'win32') {
    if (
      environment.OPENSSL_DIR &&
      path.win32.isAbsolute(environment.OPENSSL_DIR) &&
      !path.posix.isAbsolute(environment.OPENSSL_DIR)
    ) {
      delete environment.OPENSSL_DIR;
    }
    return;
  }

  if (environment.OPENSSL_DIR) return;
  const candidates = [
    path.join(workspaceRoot, 'vendor', 'openssl-windows-x64'),
    path.join(environment.ProgramFiles ?? 'C:\\Program Files', 'OpenSSL-Win64'),
  ];
  for (const candidate of candidates) {
    if (
      fileExists(path.join(candidate, 'include', 'openssl', 'ssl.h')) &&
      (fileExists(path.join(candidate, 'lib', 'libcrypto.lib')) ||
        fileExists(path.join(candidate, 'lib', 'VC', 'static', 'libcrypto64MT.lib')))
    ) {
      environment.OPENSSL_DIR = candidate;
      environment.OPENSSL_NO_VENDOR = '1';
      break;
    }
  }
}
