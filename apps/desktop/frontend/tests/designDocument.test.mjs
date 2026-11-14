import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/features/design-workspace/data/designDocument.ts');
globalThis.__openChatDesignStorageHarness = {
  values: new Map(),
  canvasResult: { kind: 'unavailable' },
  failRead: false,
  failWrite: false,
  writes: [],
};
const source = fs.readFileSync(sourcePath, 'utf8')
  .replace(
    "import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';",
    `const unifiedStorage = {
      getItem(key) {
        if (globalThis.__openChatDesignStorageHarness.failRead) throw new Error('Design storage read failed.');
        return globalThis.__openChatDesignStorageHarness.values.get(key) ?? null;
      },
      async setPersistedItem(key, value) {
        if (globalThis.__openChatDesignStorageHarness.failWrite) throw new Error('Design storage write failed.');
        globalThis.__openChatDesignStorageHarness.writes.push([key, value]);
        globalThis.__openChatDesignStorageHarness.values.set(key, value);
      },
    };`,
  )
  .replace(
    /import \{[\s\S]*?\} from '@features\/design-workspace\/data\/designCanvas';/,
    [
      'const designCanvasDataUrl = (value) => value;',
      'const parseDesignCanvasTokens = (value) => value;',
      'const readDesignCanvas = () => globalThis.__openChatDesignStorageHarness.canvasResult;',
      'const sanitizeDesignCanvasSvg = (value) => value;',
    ].join('\n'),
  );
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { addDesignComment, addDesignPage, createDesignImageNode, createDesignNode, designDocumentImageAssetsForExport, exportDesignDocumentHtml, exportDesignDocumentSvg, maximumDesignNodeCount, migrateLegacyCanvas, readDesignDocument, removeDesignColorVariable, removeDesignComment, removeDesignPage, renameDesignPage, saveDesignDocument, setDesignCommentResolved, updateDesignColorVariable, updateDesignStroke, validateDesignDocument } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

function resetStorageHarness() {
  const harness = globalThis.__openChatDesignStorageHarness;
  harness.values.clear();
  harness.canvasResult = { kind: 'unavailable' };
  harness.failRead = false;
  harness.failWrite = false;
  harness.writes.length = 0;
}

test('legacy v1 canvas is migrated without removing its recoverable source', () => {
  resetStorageHarness();
  const legacyCanvas = [{
    id: 'legacy-frame', title: 'Legacy screen', x: 12, y: 24, width: 390, height: 844,
    svg: '<svg/>', tokens: { background: '#ffffff' }, comments: [],
  }];
  globalThis.__openChatDesignStorageHarness.canvasResult = {
    kind: 'ready', canvas: { version: 1, frames: legacyCanvas },
  };

  const result = readDesignDocument();

  assert.equal(result.kind, 'ready');
  assert.equal(result.needsMigration, true);
  assert.equal(result.document.nodes[0]?.id, 'legacy-frame');
  assert.deepEqual(globalThis.__openChatDesignStorageHarness.values, new Map());
});

test('invalid and unavailable Design storage remains recoverable without replacement writes', () => {
  resetStorageHarness();
  const raw = '{broken legacy data';
  globalThis.__openChatDesignStorageHarness.values.set('design.document.v9', raw);

  assert.deepEqual(readDesignDocument(), { kind: 'recoveryRequired', rawData: raw, reason: 'invalidJson' });
  assert.deepEqual(globalThis.__openChatDesignStorageHarness.writes, []);

  resetStorageHarness();
  globalThis.__openChatDesignStorageHarness.failRead = true;
  assert.deepEqual(readDesignDocument(), { kind: 'unavailable' });
  assert.deepEqual(globalThis.__openChatDesignStorageHarness.writes, []);
});

test('failed Design document writes leave the existing saved source unchanged', async () => {
  resetStorageHarness();
  const document = validateDesignDocument({ ...documentWith([]), version: 9 });
  const raw = JSON.stringify({ version: 9, id: 'saved-source' });
  globalThis.__openChatDesignStorageHarness.values.set('design.document.v9', raw);
  globalThis.__openChatDesignStorageHarness.failWrite = true;

  await assert.rejects(saveDesignDocument(document), /Design storage write failed/);
  assert.equal(globalThis.__openChatDesignStorageHarness.values.get('design.document.v9'), raw);
  assert.deepEqual(globalThis.__openChatDesignStorageHarness.writes, []);
});

function documentWith(nodes, selectedNodeIds = []) {
  return { version: 7, id: 'document-1', name: 'Design', projectId: null, pages: [{ id: 'page-1', name: 'Page 1' }], activePageId: 'page-1', nodes, selectedNodeIds, variables: [], comments: [], prototypeFlows: [], prototypeInteractions: [], motionTracks: [], components: [] };
}

test('legacy canvas migration preserves anchored comments as document annotations', () => {
  const legacyFrame = {
    id: 'old-frame', title: 'Old screen', x: 12, y: 24, width: 390, height: 844,
    svg: '<svg/>', tokens: { background: '#ffffff' }, comments: [
      { id: 'old-comment', text: 'Adjust this spacing', point: { x: 0.25, y: 0.75 }, createdAt: 123 },
    ],
  };
  const migrated = validateDesignDocument(migrateLegacyCanvas([legacyFrame]));

  assert.deepEqual(migrated.comments, [{
    id: 'old-comment', pageId: 'legacy-page-1', nodeId: 'old-frame', text: 'Adjust this spacing',
    point: { x: 0.25, y: 0.75 }, createdAt: 123, resolved: false,
  }]);
  assert.throws(() => validateDesignDocument({ ...migrated, comments: [{ ...migrated.comments[0], pageId: 'missing' }] }));
  assert.throws(() => validateDesignDocument({ ...migrated, comments: [{ ...migrated.comments[0], point: { x: 2, y: 0.5 } }] }));
});

test('canvas annotations are anchored, resolvable, and removable', () => {
  const frame = { ...createDesignNode('frame', 20, 30, 'Screen', ''), id: 'frame-comment' };
  const document = documentWith([frame]);
  const added = addDesignComment(document, frame.id, 'Review this area', { x: 0.4, y: 0.6 }, 123);
  assert.deepEqual(added.comments[0], {
    id: added.comments[0]?.id, pageId: 'page-1', nodeId: frame.id, text: 'Review this area',
    point: { x: 0.4, y: 0.6 }, createdAt: 123, resolved: false,
  });
  const resolved = setDesignCommentResolved(added, added.comments[0].id, true);
  assert.equal(resolved.comments[0]?.resolved, true);
  assert.deepEqual(removeDesignComment(resolved, added.comments[0].id).comments, []);
  assert.throws(() => addDesignComment(document, frame.id, '   ', { x: 0.2, y: 0.2 }));
  assert.throws(() => addDesignComment(document, frame.id, 'Invalid point', { x: 2, y: 0.2 }));
});

test('typed design documents preserve valid nested frame structure and selection', () => {
  const frame = createDesignNode('frame', 0, 0, 'Screen', '');
  const text = { ...createDesignNode('text', 8, 12, 'Title', 'Welcome'), parentId: frame.id };

  const validated = validateDesignDocument(documentWith([frame, text], [text.id]));

  assert.deepEqual(validated.nodes.map((node) => node.id), [frame.id, text.id]);
  assert.deepEqual(validated.selectedNodeIds, [text.id]);
});

test('version 8 documents migrate with an empty reusable text style collection', () => {
  const title = { ...createDesignNode('text', 8, 12, 'Title', 'Welcome'), id: 'title-v8' };
  const migrated = validateDesignDocument({ ...documentWith([title]), version: 8 });

  assert.equal(migrated.version, 9);
  assert.deepEqual(migrated.textStyles, []);
  assert.equal(migrated.nodes[0]?.id, title.id);
});

test('text style collections and node references must be unique and compatible', () => {
  const title = { ...createDesignNode('text', 8, 12, 'Title', 'Welcome'), id: 'title-styled' };
  const style = { id: 'text-style-1', name: 'Heading', color: '#123456', fontSize: 28, fontWeight: 600 };
  const document = { ...documentWith([{ ...title, textStyleId: style.id }]), version: 9, textStyles: [style] };

  assert.equal(validateDesignDocument(document).nodes[0]?.type === 'text' ? validateDesignDocument(document).nodes[0]?.textStyleId : undefined, style.id);
  assert.throws(() => validateDesignDocument({ ...document, textStyles: [] }));
  assert.throws(() => validateDesignDocument({ ...document, textStyles: [style, { ...style, id: 'text-style-2', name: 'heading' }] }));
  const rectangle = { ...createDesignNode('rectangle', 0, 0, 'Card', ''), textStyleId: style.id };
  assert.throws(() => validateDesignDocument({ ...document, nodes: [rectangle] }));
});

test('prototype document validation accepts click and hover triggers and rejects unknown triggers', () => {
  const frame = { ...createDesignNode('frame', 0, 0, 'Start', ''), id: 'frame-start' };
  const target = { ...createDesignNode('frame', 500, 0, 'Target', ''), id: 'frame-target' };
  const button = { ...createDesignNode('rectangle', 24, 24, 'Continue', ''), id: 'button-continue', parentId: frame.id };
  const document = documentWith([frame, target, button]);
  const interaction = { id: 'interaction-1', sourceNodeId: button.id, targetNodeId: target.id, trigger: 'hover', transition: 'dissolve', durationMs: 240 };
  assert.equal(validateDesignDocument({ ...document, prototypeInteractions: [interaction] }).prototypeInteractions[0]?.trigger, 'hover');
  assert.throws(() => validateDesignDocument({ ...document, prototypeInteractions: [{ ...interaction, trigger: 'focus' }] }));
});

test('version 6 prototype and frame data migrates with its existing behavior preserved', () => {
  const start = { ...createDesignNode('frame', 0, 0, 'Start', ''), id: 'frame-start' };
  const target = { ...createDesignNode('frame', 500, 0, 'Target', ''), id: 'frame-target' };
  const button = { ...createDesignNode('rectangle', 24, 24, 'Continue', ''), id: 'button-continue', parentId: start.id };
  const document = documentWith([start, target, button]);
  const migrated = validateDesignDocument({
    ...document,
    version: 6,
    nodes: [start, target, button].map((node) => node.type === 'frame' ? { ...node, scrollable: undefined } : node),
    prototypeInteractions: [{ id: 'interaction-1', sourceNodeId: button.id, targetNodeId: target.id, trigger: 'click', transition: 'instant', durationMs: 0 }],
  });
  assert.equal(migrated.version, 9);
  assert.equal(migrated.nodes.find((node) => node.id === start.id)?.type === 'frame' ? migrated.nodes.find((node) => node.id === start.id)?.scrollable : null, false);
  assert.equal(migrated.prototypeInteractions[0]?.presentation, 'navigate');
});

test('design documents reject duplicate identifiers, broken parents, and hierarchy cycles', () => {
  const frame = createDesignNode('frame', 0, 0, 'Screen', '');
  const shape = createDesignNode('rectangle', 10, 10, 'Card', '');
  assert.throws(() => validateDesignDocument(documentWith([frame, { ...shape, id: frame.id }])));
  assert.throws(() => validateDesignDocument(documentWith([{ ...shape, parentId: 'missing-frame' }])));

  const childFrame = { ...createDesignNode('frame', 0, 0, 'Nested frame', ''), parentId: frame.id };
  assert.throws(() => validateDesignDocument(documentWith([
    { ...frame, parentId: childFrame.id },
    childFrame,
  ])));
});

test('design documents reject invalid selections, unsupported dimensions, and excess node counts', () => {
  const shape = createDesignNode('ellipse', 0, 0, 'Circle', '');
  assert.throws(() => validateDesignDocument(documentWith([shape], ['missing-node'])));
  assert.throws(() => validateDesignDocument(documentWith([{ ...shape, width: 0 }])));
  assert.throws(() => validateDesignDocument(documentWith(Array.from({ length: maximumDesignNodeCount + 1 }, (_, index) => ({
    ...shape,
    id: `shape-${index}`,
  })))));
});

test('SVG export respects the selected frame and escapes text as XML', () => {
  const frame = { ...createDesignNode('frame', 100, 200, 'Selected', ''), width: 300, height: 500 };
  const text = {
    ...createDesignNode('text', 120, 220, 'Heading', '<script>&"'),
    parentId: frame.id,
  };
  const outside = createDesignNode('rectangle', 800, 900, 'Outside', '');
  const svg = exportDesignDocumentSvg(documentWith([frame, text, outside], [frame.id]));

  assert.match(svg, /viewBox="100 200 300 500"/);
  assert.match(svg, /&lt;script&gt;&amp;&quot;/);
  assert.doesNotMatch(svg, /#D9DDE2/);
});

test('HTML code export preserves selected layout and escapes user content', () => {
  const card = { ...createDesignNode('frame', 100, 200, 'Card', ''), width: 240, height: 120, fill: '#ffffff' };
  const label = { ...createDesignNode('text', 112, 216, 'Heading', '<script>alert("x")</script>'), parentId: card.id };
  const other = createDesignNode('rectangle', 700, 800, 'Outside', '');
  const html = exportDesignDocumentHtml({ ...documentWith([card, label, other], [card.id]), name: '<Design>' });

  assert.match(html, /<title>&lt;Design&gt;<\/title>/);
  assert.match(html, /width:240px;height:120px/);
  assert.match(html, /&lt;script&gt;alert\(&quot;x&quot;\)&lt;\/script&gt;/);
  assert.doesNotMatch(html, /<script>alert/);
  assert.doesNotMatch(html, /Outside/);
});

test('image nodes validate bounded library references and export embedded assets', () => {
  const image = createDesignImageNode({ assetId: 'a'.repeat(32), mimeType: 'image/png', sizeBytes: 2048, pageId: 'page-1', x: 12, y: 24, name: 'Photo', altText: 'A product photo' });
  const document = documentWith([image]);
  assert.equal(designDocumentImageAssetsForExport(document)[0]?.assetId, image.assetId);
  assert.match(exportDesignDocumentSvg(document, new Map([[image.assetId, 'data:image/png;base64,AA==']])), /href="data:image\/png;base64,AA=="/);
  assert.throws(() => validateDesignDocument(documentWith([{ ...image, mimeType: 'image/svg+xml' }])));
  assert.throws(() => validateDesignDocument(documentWith([{ ...image, sizeBytes: 32 * 1024 * 1024 + 1 }])));
});

test('color variables validate their references and render through SVG export', () => {
  const shape = { ...createDesignNode('rectangle', 0, 0, 'Card', ''), fillVariableId: 'brand-color' };
  const document = {
    ...documentWith([shape]),
    variables: [{ id: 'brand-color', name: 'Brand', value: '#123456' }],
  };
  const validated = validateDesignDocument(document);
  assert.match(exportDesignDocumentSvg(validated), /fill="#123456"/);

  assert.throws(() => validateDesignDocument({
    ...document,
    variables: [],
  }));
  assert.throws(() => validateDesignDocument({
    ...document,
    variables: [document.variables[0], document.variables[0]],
  }));
});

test('editing a color variable updates linked nodes and deletion keeps their resolved colors', () => {
  const shape = { ...createDesignNode('rectangle', 0, 0, 'Card', ''), fillVariableId: 'brand-color' };
  const text = { ...createDesignNode('text', 0, 120, 'Title', 'Title'), colorVariableId: 'brand-color' };
  const document = {
    ...documentWith([shape, text]),
    variables: [{ id: 'brand-color', name: 'Brand', value: '#123456' }],
  };

  const updated = updateDesignColorVariable(document, 'brand-color', 'Accent', '#654321');
  assert.equal(updated.variables[0]?.name, 'Accent');
  assert.equal(updated.nodes[0]?.type === 'rectangle' ? updated.nodes[0].fill : null, '#654321');
  assert.match(exportDesignDocumentSvg(updated), /fill="#654321"/);

  const removed = removeDesignColorVariable(updated, 'brand-color');
  assert.deepEqual(removed.variables, []);
  assert.equal(removed.nodes[0]?.type === 'rectangle' ? removed.nodes[0].fillVariableId : 'missing', undefined);
  assert.equal(removed.nodes[0]?.type === 'rectangle' ? removed.nodes[0].fill : null, '#654321');
  assert.throws(() => updateDesignColorVariable(document, 'missing', 'Broken', '#654321'));
  assert.throws(() => updateDesignColorVariable(document, 'brand-color', '', '#654321'));
});

test('structured strokes validate, export, and apply to selected vector primitives', () => {
  const rectangle = createDesignNode('rectangle', 10, 20, 'Card', '');
  const ellipse = createDesignNode('ellipse', 60, 20, 'Dot', '');
  const text = createDesignNode('text', 120, 20, 'Label', 'Label');
  const document = documentWith([rectangle, ellipse, text]);
  const updated = updateDesignStroke(document, [rectangle.id, ellipse.id], '#336699', 3);

  assert.equal(updated.nodes[0]?.type === 'rectangle' ? updated.nodes[0].stroke : null, '#336699');
  assert.equal(updated.nodes[1]?.type === 'ellipse' ? updated.nodes[1].strokeWidth : null, 3);
  assert.equal(updated.nodes[2]?.type === 'text' ? updated.nodes[2].color : null, '#23272C');
  assert.match(exportDesignDocumentSvg(updated), /stroke="#336699" stroke-width="3"/);
  assert.throws(() => updateDesignStroke(document, [rectangle.id], 'red', 1));
  assert.throws(() => updateDesignStroke(document, [rectangle.id], '#336699', Number.POSITIVE_INFINITY));
  assert.throws(() => updateDesignStroke(document, ['missing'], '#336699', 1));
});

test('structured shadow effects serialize to SVG and reject unsafe values', () => {
  const shape = { ...createDesignNode('rectangle', 10, 20, 'Card', ''), effects: [{ type: 'shadow', color: '#112233', offsetX: 2, offsetY: 4, blur: 12 }] };
  const svg = exportDesignDocumentSvg(documentWith([shape]));
  assert.match(svg, /filter:drop-shadow\(2px 4px 12px #112233\)/);
  assert.throws(() => validateDesignDocument(documentWith([{ ...shape, effects: [{ ...shape.effects[0], blur: 4097 }] }])));
});

test('version 2 documents migrate to version 9 without losing their nodes', () => {
  const shape = createDesignNode('rectangle', 0, 0, 'Card', '');
  const { pageId: _pageId, ...legacyShape } = shape;
  const migrated = validateDesignDocument({
    version: 2,
    id: 'document-1',
    name: 'Legacy design',
    projectId: null,
    nodes: [legacyShape],
    selectedNodeIds: [shape.id],
  });

  assert.equal(migrated.version, 9);
  assert.deepEqual(migrated.motionTracks, []);
  assert.equal(migrated.pages.length, 1);
  assert.equal(migrated.nodes[0]?.pageId, migrated.pages[0]?.id);
  assert.deepEqual(migrated.selectedNodeIds, [shape.id]);
  assert.throws(() => validateDesignDocument({ ...migrated, pages: undefined }));
  const { pageId: _migratedPageId, ...missingPageNode } = migrated.nodes[0];
  assert.throws(() => validateDesignDocument({ ...migrated, nodes: [missingPageNode] }));
});

test('responsive geometry overrides validate and remain attached to typed nodes', () => {
  const shape = createDesignNode('rectangle', 20, 30, 'Card', '');
  const responsive = validateDesignDocument({ ...documentWith([shape]), version: 9, nodes: [{
    ...shape,
    responsiveOverrides: { mobile: { x: 4, y: 8, width: 320, height: 240 }, tablet: { x: 12, y: 16, width: 640, height: 360 } },
  }] });

  assert.deepEqual(responsive.nodes[0]?.responsiveOverrides, {
    mobile: { x: 4, y: 8, width: 320, height: 240 },
    tablet: { x: 12, y: 16, width: 640, height: 360 },
  });
  assert.throws(() => validateDesignDocument({ ...responsive, nodes: [{ ...shape, responsiveOverrides: { desktop: { x: 0, y: 0, width: 1, height: 1 } } }] }));
  assert.throws(() => validateDesignDocument({ ...responsive, nodes: [{ ...shape, responsiveOverrides: { mobile: { x: 0, y: 0, width: 0, height: 1 } } }] }));
});

test('version 3 documents migrate typed collections without losing pages or nodes', () => {
  const shape = createDesignNode('rectangle', 0, 0, 'Card', '');
  const { pageId: _pageId, ...legacyShape } = shape;
  const migrated = validateDesignDocument({ version: 3, id: 'document-v3', name: 'Old document', projectId: null,
    pages: [{ id: 'page-old', name: 'Old page' }], activePageId: 'page-old', nodes: [{ ...legacyShape, pageId: 'page-old' }], selectedNodeIds: [], variables: [] });
  assert.equal(migrated.version, 9);
  assert.equal(migrated.pages[0]?.id, 'page-old');
  assert.deepEqual(migrated.comments, []);
  assert.deepEqual(migrated.components, []);
  assert.deepEqual(migrated.prototypeInteractions, []);
  assert.deepEqual(migrated.motionTracks, []);
});

test('version 5 documents migrate to the current schema while retaining frame behavior', () => {
  const frame = createDesignNode('frame', 10, 20, 'Screen', '');
  const legacy = { ...documentWith([frame]), version: 5 };
  const migrated = validateDesignDocument(legacy);

  assert.equal(migrated.version, 9);
  assert.deepEqual(migrated.nodes, [{ ...frame, scrollable: false }]);
  assert.deepEqual(migrated.motionTracks, []);
});

test('structured vector paths validate points and export through SVG and HTML', () => {
  const path = createDesignNode('path', 24, 40, 'Triangle', '');
  const document = documentWith([path]);
  assert.match(exportDesignDocumentSvg(document), /<path d="M 80 0 L 160 120 L 0 120 Z"/);
  assert.match(exportDesignDocumentHtml(document), /<svg aria-label="Triangle"/);
  assert.throws(() => validateDesignDocument(documentWith([{ ...path, points: [{ x: 0, y: Number.POSITIVE_INFINITY }] }])));
  assert.throws(() => validateDesignDocument(documentWith([{ ...path, points: [{ x: 0, y: 0 }, { x: 1, y: 1 }] }])));
});

test('pages can be added, renamed, and removed without cross-page selection or parenting', () => {
  const frame = { ...createDesignNode('frame', 0, 0, 'Screen', ''), id: 'frame-1' };
  const document = documentWith([frame], [frame.id]);
  const secondPage = addDesignPage(document, 'Settings');
  const secondPageId = secondPage.activePageId;
  assert.equal(secondPage.pages.length, 2);
  assert.deepEqual(secondPage.selectedNodeIds, []);

  const renamed = renameDesignPage(secondPage, secondPageId, 'Preferences');
  assert.equal(renamed.pages[1]?.name, 'Preferences');
  assert.throws(() => validateDesignDocument({
    ...renamed,
    selectedNodeIds: [frame.id],
  }));
  assert.throws(() => validateDesignDocument({
    ...renamed,
    nodes: [...renamed.nodes, { ...createDesignNode('rectangle', 4, 4, 'Child', '', secondPageId), parentId: frame.id }],
  }));

  const removed = removeDesignPage(renamed, secondPageId);
  assert.equal(removed.pages.length, 1);
  assert.equal(removed.activePageId, document.activePageId);
  assert.equal(removed.nodes[0]?.id, frame.id);
  assert.throws(() => removeDesignPage(document, document.activePageId));
});
