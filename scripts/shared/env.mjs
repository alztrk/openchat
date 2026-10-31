import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { getWorkspaceRoot } from './runner.mjs';

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

export function ensureOpenSSLEnvironment() {
  if (process.env.OPENSSL_DIR) return;
  if (process.platform === 'win32') {
    const candidates = [
      path.join(getWorkspaceRoot(), 'vendor', 'openssl-windows-x64'),
      path.join(process.env.ProgramFiles ?? 'C:\\Program Files', 'OpenSSL-Win64'),
    ];
    for (const candidate of candidates) {
      if (
        existsSync(path.join(candidate, 'include', 'openssl', 'ssl.h')) &&
        (existsSync(path.join(candidate, 'lib', 'libcrypto.lib')) ||
          existsSync(path.join(candidate, 'lib', 'VC', 'static', 'libcrypto64MT.lib')))
      ) {
        process.env.OPENSSL_DIR = candidate;
        process.env.OPENSSL_NO_VENDOR = '1';
        break;
      }
    }
  }
}
