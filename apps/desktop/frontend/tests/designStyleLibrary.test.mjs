import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

globalThis.__openChatDesignStyleLibraryHarness = { calls: [], response: { items: [] } };
const source = fs.readFileSync(path.resolve('src/features/design-workspace/data/designStyleLibrary.ts'), 'utf8')
  .replace(
    "import { localServiceClient } from '@shared/lib/platform/local-service';",
    'const localServiceClient = { call: async (method, params) => { globalThis.__openChatDesignStyleLibraryHarness.calls.push({ method, params }); return globalThis.__openChatDesignStyleLibraryHarness.response; } };',
  );
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const library = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('shared Design text styles use bounded local-service list, revisioned save, and delete RPCs', async () => {
  const harness = globalThis.__openChatDesignStyleLibraryHarness;
  harness.calls.length = 0;
  harness.response = { items: [{
    id: 'style-1', name: 'Heading', color: '#123456', fontSize: 28, fontWeight: 600, revision: 3, updatedAtUnixMs: 500,
  }] };

  const [style] = await library.listSharedDesignTextStyles();
  assert.equal(style?.revision, 3);
  assert.deepEqual(harness.calls[0], { method: 'design.styles.list', params: { limit: 2_000 } });

  harness.response = { saved: true, revision: 4 };
  assert.equal(await library.saveSharedDesignTextStyle({
    id: 'style-1', name: 'Heading', color: '#123456', fontSize: 28, fontWeight: 600,
  }, 3), 4);
  assert.deepEqual(harness.calls[1], {
    method: 'design.styles.save',
    params: { id: 'style-1', name: 'Heading', color: '#123456', fontSize: 28, fontWeight: 600, expectedRevision: 3 },
  });

  harness.response = { deleted: true };
  await library.deleteSharedDesignTextStyle('style-1', 4);
  assert.deepEqual(harness.calls[2], { method: 'design.styles.delete', params: { id: 'style-1', expectedRevision: 4 } });
});

test('shared Design text styles reject malformed library records and mutation responses', async () => {
  const harness = globalThis.__openChatDesignStyleLibraryHarness;
  harness.response = { items: [{ id: 'style-1', name: 'Heading', color: '#fff', fontSize: 28, fontWeight: 600, revision: 1, updatedAtUnixMs: 1 }] };
  await assert.rejects(library.listSharedDesignTextStyles(), /shared Design style is invalid/);

  harness.response = { saved: true, revision: 0 };
  await assert.rejects(library.saveSharedDesignTextStyle({
    id: 'style-1', name: 'Heading', color: '#123456', fontSize: 28, fontWeight: 600,
  }, null), /save response is invalid/);

  harness.response = { deleted: false };
  await assert.rejects(library.deleteSharedDesignTextStyle('style-1', 1), /no longer exists/);
});
