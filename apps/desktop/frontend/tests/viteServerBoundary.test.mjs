import assert from 'node:assert/strict';
import { request as httpRequest } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createServer, loadConfigFromFile } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function requestWithHost(port, host) {
  return new Promise((resolve, reject) => {
    const request = httpRequest(
      {
        hostname: '127.0.0.1',
        port,
        path: '/',
        headers: { host },
      },
      (response) => {
        response.resume();
        response.on('end', () => resolve(response.statusCode));
      },
    );
    request.on('error', reject);
    request.end();
  });
}

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

test('Vite serves the loopback host and rejects untrusted DNS host headers', async () => {
  const server = await createServer({
    configFile: path.join(frontendRoot, 'vite.config.ts'),
    logLevel: 'silent',
    server: {
      host: '127.0.0.1',
      port: 0,
      strictPort: false,
      hmr: false,
    },
  });

  try {
    await server.listen();
    const address = server.httpServer?.address();
    assert.ok(address && typeof address !== 'string');
    const trustedHost = await requestWithHost(address.port, `127.0.0.1:${address.port}`);
    const untrustedHost = await requestWithHost(
      address.port,
      `openchat-untrusted-host.invalid:${address.port}`,
    );

    assert.equal(trustedHost, 200);
    assert.equal(untrustedHost, 403);
  } finally {
    await server.close();
  }
});
