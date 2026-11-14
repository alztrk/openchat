import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

globalThis.__openChatDesignDocumentRpcResponse = null;
const documentSource = fs.readFileSync(path.resolve('src/features/design-workspace/data/designDocument.ts'), 'utf8')
  .replace(
    "import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';",
    'const unifiedStorage = { getItem: () => null, setPersistedItem: async () => undefined };',
  )
  .replace(
    /import \{[\s\S]*?\} from '@features\/design-workspace\/data\/designCanvas';/,
    [
      'const designCanvasDataUrl = (value) => value;',
      'const parseDesignCanvasTokens = (value) => value;',
      'const readDesignCanvas = () => ({ kind: "unavailable" });',
      'const sanitizeDesignCanvasSvg = (value) => value;',
    ].join('\n'),
  );
const compiledDocument = ts.transpileModule(documentSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
globalThis.__openChatDesignDocumentValidator = (await import(
  `data:text/javascript;base64,${Buffer.from(compiledDocument).toString('base64')}`
)).validateDesignDocument;
const source = fs.readFileSync(path.resolve('src/features/design-workspace/data/designDocumentRepository.ts'), 'utf8')
  .replace(
    "import { localServiceClient } from '@shared/lib/platform/local-service';",
    'const localServiceClient = { call: async () => globalThis.__openChatDesignDocumentRpcResponse };',
  )
  .replace(
    "import { validateDesignDocument, type DesignDocument } from './designDocument';",
    'const validateDesignDocument = globalThis.__openChatDesignDocumentValidator;',
  );
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { getDesignDocument } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

function rpcResponse(document, projectId = document.projectId) {
  return {
    document: {
      id: document.id,
      name: document.name,
      projectId,
      revision: 4,
      updatedAtUnixMs: 100,
      documentJson: JSON.stringify(document),
    },
  };
}

test('repository marks prior typed documents for page-schema migration', async () => {
  globalThis.__openChatDesignDocumentRpcResponse = rpcResponse({
    version: 2,
    id: 'design-1',
    name: 'Design',
    projectId: null,
    nodes: [],
    selectedNodeIds: [],
  });

  const stored = await getDesignDocument('design-1');

  assert.equal(stored.needsMigration, true);
  assert.equal(stored.document.version, 9);
  assert.equal(stored.document.activePageId, stored.document.pages[0]?.id);
});

test('repository migrates version 5 documents when vector paths enter the schema', async () => {
  globalThis.__openChatDesignDocumentRpcResponse = rpcResponse({
    version: 5,
    id: 'design-2',
    name: 'Design',
    projectId: null,
    pages: [{ id: 'page-1', name: 'Page 1' }],
    activePageId: 'page-1',
    nodes: [],
    selectedNodeIds: [],
    variables: [],
    comments: [],
    prototypeFlows: [],
    prototypeInteractions: [],
    components: [],
  });

  const stored = await getDesignDocument('design-2');

  assert.equal(stored.needsMigration, true);
  assert.equal(stored.document.activePageId, 'page-1');
  assert.deepEqual(stored.document.motionTracks, []);
});

test('repository migrates version 7 documents to the current responsive schema', async () => {
  globalThis.__openChatDesignDocumentRpcResponse = rpcResponse({
    version: 7,
    id: 'design-3',
    name: 'Design',
    projectId: null,
    pages: [{ id: 'page-1', name: 'Page 1' }],
    activePageId: 'page-1',
    nodes: [],
    selectedNodeIds: [],
    variables: [],
    comments: [],
    prototypeFlows: [],
    prototypeInteractions: [],
    motionTracks: [],
    components: [],
  });

  const stored = await getDesignDocument('design-3');

  assert.equal(stored.needsMigration, true);
  assert.equal(stored.document.version, 9);
  assert.deepEqual(stored.document.motionTracks, []);
});

test('repository detaches a design document when its linked project was deleted', async () => {
  globalThis.__openChatDesignDocumentRpcResponse = rpcResponse({
    version: 8,
    id: 'design-detached',
    name: 'Design',
    projectId: 'deleted-project',
    pages: [{ id: 'page-1', name: 'Page 1' }],
    activePageId: 'page-1',
    nodes: [],
    selectedNodeIds: [],
    variables: [],
    comments: [],
    prototypeFlows: [],
    prototypeInteractions: [],
    motionTracks: [],
    components: [],
  }, null);

  const stored = await getDesignDocument('design-detached');

  assert.equal(stored.projectId, null);
  assert.equal(stored.document.projectId, null);
  assert.equal(stored.needsMigration, true);
});
