import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { loadConfigFromFile } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('Vite serves only the fixed loopback origin and restricts cross-origin reads to that origin', async () => {
  const loaded = await loadConfigFromFile(
    { command: 'serve', mode: 'development' },
    path.join(frontendRoot, 'vite.config.ts'),
    frontendRoot,
    'silent',
  );

  assert.ok(loaded);
  assert.equal(loaded.config.server?.host, '127.0.0.1');
  assert.equal(loaded.config.server?.port, 5173);
  assert.equal(loaded.config.server?.strictPort, true);
  assert.deepEqual(loaded.config.server?.cors, { origin: 'http://127.0.0.1:5173' });
});
