import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { validateSpdxFile, validateSpdxJson } from '../../../../scripts/shared/sbom-validation.mjs';

const validDocument = {
  spdxVersion: 'SPDX-2.3',
  documentNamespace: 'https://openchat.example/sbom/test',
  packages: [{ name: 'test-only-fixture', SPDXID: 'SPDXRef-Package-test' }],
};

test('SPDX validation accepts a non-empty SPDX 2.3 document', () => {
  assert.deepEqual(validateSpdxJson(JSON.stringify(validDocument)), { packageCount: 1 });
});

test('SPDX validation rejects malformed JSON and non-object documents', () => {
  assert.throws(() => validateSpdxJson('{'), /valid JSON/u);
  assert.throws(() => validateSpdxJson('[]'), /JSON object/u);
  assert.throws(() => validateSpdxJson('null'), /JSON object/u);
});

test('SPDX validation rejects a missing version, empty packages, or an empty namespace', () => {
  assert.throws(() => validateSpdxJson(JSON.stringify({ ...validDocument, spdxVersion: 'SPDX-3.0' })), /SPDX-2\.3/u);
  assert.throws(() => validateSpdxJson(JSON.stringify({ ...validDocument, packages: [] })), /non-empty array/u);
  assert.throws(() => validateSpdxJson(JSON.stringify({ ...validDocument, documentNamespace: '  ' })), /non-empty/u);
});

test('SPDX file validation reports a missing file and validates readable JSON', () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'openchat-sbom-test-'));
  const filePath = path.join(directory, 'OpenChat-SBOM.spdx.json');
  try {
    assert.throws(() => validateSpdxFile(filePath), /missing or unreadable/u);
    writeFileSync(filePath, JSON.stringify(validDocument), { encoding: 'utf8', flag: 'wx' });
    assert.deepEqual(validateSpdxFile(filePath), { packageCount: 1 });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test('the tagged release workflow gates publishing on a pinned, validated SBOM artifact', () => {
  const workflowPath = fileURLToPath(new URL('../../../../.github/workflows/desktop-ci.yml', import.meta.url));
  const workflow = readFileSync(workflowPath, 'utf8');
  const sbomJob = workflow.split('\n  sbom:\n')[1]?.split(/\n {2}[a-z][a-z0-9-]*:\n/u)[0] ?? '';
  const releaseJob = workflow.split('\n  release:\n')[1] ?? '';

  assert.match(sbomJob, /if: startsWith\(github\.ref, 'refs\/tags\/v'\)/u);
  assert.match(sbomJob, /permissions:\s*contents: read/u);
  assert.doesNotMatch(sbomJob, /contents: write/u);
  assert.match(sbomJob, /actions\/setup-node@[0-9a-f]{40}/u);
  assert.match(sbomJob, /node-version: 22/u);
  assert.match(sbomJob, /anchore\/sbom-action@[0-9a-f]{40}/u);
  assert.match(sbomJob, /syft-version: v1\.57\.1/u);
  assert.match(sbomJob, /node scripts\/validate-sbom\.mjs release-artifacts\/sbom\/OpenChat-SBOM\.spdx\.json/u);
  assert.match(sbomJob, /name: OpenChat-SBOM[\s\S]*?retention-days: 14/u);
  assert.ok(releaseJob.includes('      - sbom'));
  assert.match(workflow, /release-artifacts\/sbom\/OpenChat-SBOM\.spdx\.json/u);
});
