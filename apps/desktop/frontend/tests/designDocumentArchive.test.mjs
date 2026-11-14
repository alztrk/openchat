import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/features/design-workspace/data/designDocumentArchive.ts');
const source = fs.readFileSync(sourcePath, 'utf8')
  .replace(
    "import { validateDesignDocument, type DesignDocument, type DesignImageNode } from './designDocument';",
    'const validateDesignDocument = (document) => document;',
  );
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { decodeDesignDocumentArchive, encodeDesignDocumentArchive } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

function documentWithImage() {
  return {
    version: 6,
    id: 'document-1',
    name: 'Portable design',
    projectId: null,
    pages: [{ id: 'page-1', name: 'Page 1' }],
    activePageId: 'page-1',
    nodes: [{
      id: 'image-1', pageId: 'page-1', name: 'Reference image', type: 'image',
      x: 0, y: 0, width: 120, height: 80, parentId: null, rotation: 0,
      visible: true, locked: false, effects: [], assetId: 'a'.repeat(32),
      mimeType: 'image/jpeg', sizeBytes: 7, fit: 'contain', altText: 'Reference',
    }],
    selectedNodeIds: [],
    variables: [],
    comments: [],
    prototypeFlows: [],
    prototypeInteractions: [],
    motionTracks: [],
    components: [],
  };
}

test('portable Design archives preserve typed document data and embedded assets', () => {
  const document = documentWithImage();
  const bytes = new Uint8Array([0xff, 0xd8, 0xff, 0x00, 0x01, 0x02, 0xd9]);
  const encoded = encodeDesignDocumentArchive(document, new Map([[document.nodes[0].assetId, bytes]]));
  const decoded = decodeDesignDocumentArchive(encoded);

  assert.deepEqual(decoded.document, document);
  assert.deepEqual(decoded.assets[0]?.bytes, bytes);
  assert.equal(decoded.assets[0]?.mimeType, 'image/jpeg');
});

test('portable Design archives reject broken references, malformed Base64, and image signatures', () => {
  const document = documentWithImage();
  const encoded = encodeDesignDocumentArchive(document, new Map([[document.nodes[0].assetId, new Uint8Array([0xff, 0xd8, 0xff, 0x00, 0x01, 0x02, 0xd9])]]));
  const archive = JSON.parse(encoded);

  archive.assets[0].dataBase64 = '***';
  assert.throws(() => decodeDesignDocumentArchive(JSON.stringify(archive)));

  archive.assets[0].dataBase64 = btoa('not jpeg');
  archive.assets[0].sizeBytes = 8;
  archive.document.nodes[0].sizeBytes = 8;
  assert.throws(() => decodeDesignDocumentArchive(JSON.stringify(archive)));

  assert.throws(() => encodeDesignDocumentArchive(document, new Map()));
});
