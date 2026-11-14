import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { createReleaseManifest, verifyReleaseFiles, verifyReleaseManifest } from '../../../../scripts/shared/release-integrity.mjs';

async function withDirectory(run) {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'openchat-release-integrity-test-'));
  try {
    await run(directory);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test('release manifests cover package files and detect modified package bytes', async () => {
  await withDirectory(async (directory) => {
    writeFileSync(path.join(directory, 'OpenChat-linux.deb'), 'package one');
    writeFileSync(path.join(directory, 'OpenChat-linux.AppImage'), 'package two');

    assert.equal(await createReleaseManifest(directory), 2);
    assert.equal(await verifyReleaseManifest(directory), 2);

    writeFileSync(path.join(directory, 'OpenChat-linux.deb'), 'changed package');
    await assert.rejects(() => verifyReleaseManifest(directory), /integrity check failed/u);
  });
});

test('release manifest verification rejects missing, unlisted, and malformed artifacts', async () => {
  await withDirectory((directory) => {
    writeFileSync(path.join(directory, 'OpenChat-setup.exe'), 'package');
    return (async () => {
      await assert.rejects(() => verifyReleaseManifest(directory), /ENOENT/u);
      await createReleaseManifest(directory);
      writeFileSync(path.join(directory, 'extra.package'), 'unlisted');
      await assert.rejects(() => verifyReleaseManifest(directory), /does not cover exactly/u);
      rmSync(path.join(directory, 'extra.package'));
      writeFileSync(path.join(directory, 'SHA256SUMS'), 'not a checksum\n');
      await assert.rejects(() => verifyReleaseManifest(directory), /malformed entry/u);
    })();
  });
});

test('aggregate manifest includes exactly the requested release assets', async () => {
  await withDirectory(async (directory) => {
    writeFileSync(path.join(directory, 'OpenChat-setup.exe'), 'installer');
    writeFileSync(path.join(directory, 'OpenChat-SBOM.spdx.json'), '{"spdxVersion":"SPDX-2.3"}');

    const manifest = await verifyReleaseFiles(directory, ['OpenChat-setup.exe', 'OpenChat-SBOM.spdx.json']);
    assert.match(manifest, /OpenChat-SBOM\.spdx\.json/u);
    assert.match(manifest, /OpenChat-setup\.exe/u);
    await assert.rejects(() => verifyReleaseFiles(directory, ['missing.exe']), /ENOENT/u);
    await assert.rejects(() => verifyReleaseFiles(directory, ['..\\outside.exe']), /regular file/u);
    assert.equal(readFileSync(path.join(directory, 'OpenChat-setup.exe'), 'utf8'), 'installer');
  });
});
