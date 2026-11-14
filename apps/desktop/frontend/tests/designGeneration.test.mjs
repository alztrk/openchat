import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

function compileDataModule(source, replacements) {
  let prepared = source;
  for (const [pattern, replacement] of replacements) prepared = prepared.replace(pattern, replacement);
  const compiled = ts.transpileModule(prepared, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);
}

const documentSource = fs.readFileSync(path.resolve('src/features/design-workspace/data/designDocument.ts'), 'utf8');
const documentModule = await compileDataModule(documentSource, [
  ["import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';", 'const unifiedStorage = { getItem: () => null, setPersistedItem: async () => undefined };'],
  [/import \{[\s\S]*?\} from '@features\/design-workspace\/data\/designCanvas';/, [
    'const designCanvasDataUrl = (value) => value;',
    'const parseDesignCanvasTokens = (value) => value;',
    'const readDesignCanvas = () => ({ kind: "unavailable" });',
    'const sanitizeDesignCanvasSvg = (value) => value;',
  ].join('\n')],
]);
globalThis.__openChatDesignGenerationValidator = documentModule.validateDesignDocument;
globalThis.__openChatDesignGenerationValidatorModule = documentModule;
const operationsSource = fs.readFileSync(path.resolve('src/features/design-workspace/data/designOperations.ts'), 'utf8');
const operationsModule = await compileDataModule(operationsSource, [
  [/import \{[\s\S]*?\} from '\.\/designDocument';/, 'const { validateDesignDocument, parseDesignPathPoints } = globalThis.__openChatDesignGenerationValidatorModule;'],
]);
globalThis.__openChatDesignOperationRemover = operationsModule.removeDesignNodes;
globalThis.__openChatDesignComponentOperations = operationsModule;
const generationSource = fs.readFileSync(path.resolve('src/features/design-workspace/data/designGeneration.ts'), 'utf8');
const generationModule = await compileDataModule(generationSource, [
  ["import { loadRouteParameters } from '@entities/model/modelRouteParameters';", 'const loadRouteParameters = () => ({});'],
  [/import \{ localServiceClient,[^}]*\} from '@shared\/lib\/platform\/local-service';/, 'const localServiceClient = { call: async (method, params) => { globalThis.__openChatDesignProjectInstructionRequest = { method, params }; if (globalThis.__openChatDesignProjectInstructionError) throw globalThis.__openChatDesignProjectInstructionError; return { instructions: globalThis.__openChatDesignProjectInstructions ?? "" }; }, startOperation: async (method, params, onEvent) => { globalThis.__openChatDesignGenerationRequest = { method, params }; for (const event of globalThis.__openChatDesignGenerationEvents ?? []) onEvent(event); return { cancel: async () => true, result: Promise.resolve({ content: globalThis.__openChatDesignGenerationResponse ?? "{}" }) }; } };'],
  [/import \{[^}]*\} from '\.\/designDocument';/, 'const { maximumDesignNodeCount, parseDesignNode, validateDesignDocument } = globalThis.__openChatDesignGenerationValidatorModule;'],
  [/import \{[^}]*\} from '\.\/designOperations';/, 'const { removeDesignNodes, synchronizeDesignComponentDependents } = globalThis.__openChatDesignComponentOperations;'],
]);
const { applyDesignOperations, extractCompleteOperations, readGeneratedOperations, startDesignGeneration } = generationModule;
const { createDesignNode } = documentModule;

function documentWith(nodes, selectedNodeIds = []) {
  return { version: 6, id: 'document-1', name: 'Design', projectId: null, pages: [{ id: 'page-1', name: 'Page 1' }], activePageId: 'page-1', nodes, selectedNodeIds, variables: [], comments: [], prototypeFlows: [], prototypeInteractions: [], motionTracks: [], components: [] };
}

test('streamed design parsing exposes only complete operation objects and respects JSON strings', () => {
  const first = { type: 'create', node: { id: 'shape-1', name: 'A } shape' } };
  const second = { type: 'delete', nodeId: 'shape-2' };
  const prefix = `{"operations":[${JSON.stringify(first)},${JSON.stringify(second).slice(0, -1)}`;

  assert.deepEqual(extractCompleteOperations(prefix), [first]);
  assert.deepEqual(extractCompleteOperations(`${prefix}}]}`), [first, second]);
  assert.deepEqual(extractCompleteOperations('{"other":[]}'), []);
  assert.deepEqual(extractCompleteOperations(JSON.stringify({ note: '"operations":[{"type":"delete"}]', operations: [first] })), [first]);
});

test('AI requests attach selected image references and selected project context without transcript persistence', async () => {
  globalThis.window = { btoa: (value) => Buffer.from(value, 'binary').toString('base64') };
  globalThis.__openChatDesignProjectInstructions = 'Use the approved product colors and compact mobile layouts.';
  const frame = createDesignNode('frame', 0, 0, 'Screen', '');
  const requestDocument = documentWith([frame]);
  const model = {
    key: 'model-key', providerId: 'provider', modelId: 'model', supportsImages: true,
    displayName: 'Model', description: null, supportsReasoning: false, supportsFastMode: false,
    reasoningLevels: [], contextWindow: null, catalogStale: false,
  };
  const operation = await startDesignGeneration({
    prompt: 'Use this visual reference',
    model,
    document: requestDocument,
    references: [{ id: 'asset-1', name: 'design-reference-1.png', mimeType: 'image/png', bytes: new Uint8Array([1, 2, 3]) }],
    projectContext: { id: 'project-1', name: 'Workspace project' },
  });
  await assert.rejects(operation.result, /supported design/);
  const request = globalThis.__openChatDesignGenerationRequest;
  assert.equal(request.method, 'chat.temporary.send');
  assert.match(request.params.conversationId, /^tmp_[a-f0-9]{32}$/);
  assert.equal(request.params.messages[0].attachments[0].data, 'AQID');
  assert.equal(request.params.messages[0].attachments[0].kind, 'image');
  assert.match(request.params.messages[0].content, /Workspace project/);
  assert.match(request.params.messages[0].content, /approved product colors and compact mobile layouts/);
  assert.deepEqual(globalThis.__openChatDesignProjectInstructionRequest, {
    method: 'project.instructions.get',
    params: { projectId: 'project-1' },
  });
  assert.equal(Object.hasOwn(request.params, 'projectId'), false);
});

test('standalone Design generation does not load or attach project instructions', async () => {
  delete globalThis.__openChatDesignProjectInstructionRequest;
  const frame = createDesignNode('frame', 0, 0, 'Screen', '');
  const model = {
    key: 'model-key', providerId: 'provider', modelId: 'model', supportsImages: false,
    displayName: 'Model', description: null, supportsReasoning: false, supportsFastMode: false,
    reasoningLevels: [], contextWindow: null, catalogStale: false,
  };
  const operation = await startDesignGeneration({ prompt: 'Create a screen', model, document: documentWith([frame]) });
  await assert.rejects(operation.result, /supported design/);
  assert.equal(globalThis.__openChatDesignProjectInstructionRequest, undefined);
  assert.doesNotMatch(globalThis.__openChatDesignGenerationRequest.params.messages[0].content, /Selected project context:/);
});

test('AI generation fails clearly when selected project instructions cannot be read', async () => {
  globalThis.__openChatDesignProjectInstructionError = new Error('project instructions unavailable');
  delete globalThis.__openChatDesignGenerationRequest;
  const frame = createDesignNode('frame', 0, 0, 'Screen', '');
  const model = {
    key: 'model-key', providerId: 'provider', modelId: 'model', supportsImages: false,
    displayName: 'Model', description: null, supportsReasoning: false, supportsFastMode: false,
    reasoningLevels: [], contextWindow: null, catalogStale: false,
  };
  await assert.rejects(startDesignGeneration({ prompt: 'Create a screen', model, document: documentWith([frame]), projectContext: { id: 'project-1', name: 'Project' } }), /project instructions unavailable/);
  assert.equal(globalThis.__openChatDesignGenerationRequest, undefined);
  delete globalThis.__openChatDesignProjectInstructionError;
});

test('AI generation rejects oversized selected project instructions before sending the provider request', async () => {
  globalThis.__openChatDesignProjectInstructions = 'x'.repeat(16 * 1024 + 1);
  delete globalThis.__openChatDesignGenerationRequest;
  const frame = createDesignNode('frame', 0, 0, 'Screen', '');
  const model = {
    key: 'model-key', providerId: 'provider', modelId: 'model', supportsImages: false,
    displayName: 'Model', description: null, supportsReasoning: false, supportsFastMode: false,
    reasoningLevels: [], contextWindow: null, catalogStale: false,
  };
  await assert.rejects(startDesignGeneration({ prompt: 'Create a screen', model, document: documentWith([frame]), projectContext: { id: 'project-1', name: 'Project' } }), /exceed their supported size/);
  assert.equal(globalThis.__openChatDesignGenerationRequest, undefined);
});

test('AI generation forwards only validated completed operations as provider deltas arrive', async () => {
  const generatedNode = {
    id: 'streamed-text', type: 'text', name: 'Title', x: 8, y: 12, width: 200, height: 40,
    parentId: null, rotation: 0, visible: true, locked: false, content: 'Welcome',
    color: '#000000', fontSize: 24, fontWeight: 500,
  };
  const response = JSON.stringify({ operations: [{ type: 'create', node: generatedNode }] });
  globalThis.__openChatDesignGenerationResponse = response;
  globalThis.__openChatDesignGenerationEvents = [...response].map((delta) => ({ name: 'chat.text.delta', data: { delta } }));
  const model = {
    key: 'model-key', providerId: 'provider', modelId: 'model', supportsImages: false,
    displayName: 'Model', description: null, supportsReasoning: false, supportsFastMode: false,
    reasoningLevels: [], contextWindow: null, catalogStale: false,
  };
  const streamed = [];
  const operation = await startDesignGeneration({ prompt: 'Add a title', model, document: documentWith([]), onOperations: (items) => streamed.push(...items) });

  assert.deepEqual(streamed, [{ type: 'create', node: { ...generatedNode, pageId: 'page-1', effects: [] } }]);
  assert.deepEqual(await operation.result, []);
  globalThis.__openChatDesignGenerationEvents = [];
  globalThis.__openChatDesignGenerationResponse = undefined;
});

test('AI design operations create and update only valid typed nodes', () => {
  const existing = createDesignNode('rectangle', 0, 0, 'Card', '');
  const created = createDesignNode('text', 12, 16, 'Title', 'Welcome');
  const document = documentWith([existing]);
  const result = applyDesignOperations(document, [
    { type: 'update', node: { ...existing, x: 24, name: 'Main card' } },
    { type: 'create', node: created },
  ]);

  assert.equal(result.nodes.find((node) => node.id === existing.id)?.x, 24);
  assert.equal(result.nodes.some((node) => node.id === created.id), true);
  assert.deepEqual(result.selectedNodeIds, [created.id]);
});

test('AI design operations create editable structured vector paths', () => {
  const generatedPath = {
    id: 'generated-path', type: 'path', name: 'Mark', x: 20, y: 30, width: 160, height: 120,
    parentId: null, rotation: 0, visible: true, locked: false,
    points: [{ x: 80, y: 0 }, { x: 160, y: 120 }, { x: 0, y: 120 }],
    closed: true, fill: '#D9DDE2', stroke: '#000000', strokeWidth: 1,
  };
  const operations = readGeneratedOperations(JSON.stringify({ operations: [{ type: 'create', node: generatedPath }] }), documentWith([]));
  const created = applyDesignOperations(documentWith([]), operations);
  assert.deepEqual(created.nodes[0]?.type === 'path' ? created.nodes[0].points : null, generatedPath.points);
});

test('AI deletion removes descendants and unauthorized IDs are rejected', () => {
  const frame = createDesignNode('frame', 0, 0, 'Screen', '');
  const child = { ...createDesignNode('text', 8, 8, 'Title', 'Welcome'), parentId: frame.id };
  const document = documentWith([frame, child]);
  const result = applyDesignOperations(document, [{ type: 'delete', nodeId: frame.id }]);
  assert.deepEqual(result.nodes, []);

  assert.throws(() => readGeneratedOperations(JSON.stringify({
    operations: [{ type: 'delete', nodeId: 'hidden-or-unknown-node' }],
  }), document));
});

test('AI updates retain component references for shared instance synchronization', () => {
  const { createDesignComponent } = globalThis.__openChatDesignComponentOperations;
  const frame = createDesignNode('frame', 0, 0, 'Button', '');
  const label = { ...createDesignNode('text', 8, 8, 'Label', 'Continue'), parentId: frame.id };
  const component = createDesignComponent(documentWith([frame, label], [frame.id]), 'Action');
  const masterLabel = component.nodes.find((node) => node.id === label.id);
  assert.ok(masterLabel);
  const response = readGeneratedOperations(JSON.stringify({ operations: [{
    type: 'update', node: { ...masterLabel, content: 'Next' },
  }] }), { ...component, selectedNodeIds: [] });
  assert.equal(response[0]?.type === 'update' ? response[0].node.componentRef?.role : null, 'master');
});

test('AI responses reject unsupported operation types and malformed node values', () => {
  const document = documentWith([]);
  assert.throws(() => readGeneratedOperations(JSON.stringify({ operations: [{ type: 'execute', code: 'anything' }] }), document));
  assert.throws(() => readGeneratedOperations(JSON.stringify({
    operations: [{ type: 'create', node: { type: 'script', name: 'Invalid' } }],
  }), document));
});

test('AI variants are create-only and revisions are scoped to the selected nodes', () => {
  const first = createDesignNode('frame', 0, 0, 'First', '');
  const second = createDesignNode('rectangle', 240, 0, 'Second', '');
  const document = documentWith([first, second], [first.id]);
  const update = { type: 'update', node: { ...first, name: 'Revised' } };
  const newNode = { ...createDesignNode('rectangle', 24, 24, 'Child', ''), id: 'child-node', parentId: first.id };
  assert.equal(readGeneratedOperations(JSON.stringify({ operations: [update] }), document, 'revise', [first.id]).length, 1);
  assert.equal(readGeneratedOperations(JSON.stringify({ operations: [{ type: 'create', node: newNode }] }), document, 'revise', [first.id]).length, 1);
  assert.throws(() => readGeneratedOperations(JSON.stringify({ operations: [{ type: 'create', node: { ...newNode, id: 'outside', parentId: null } }] }), document, 'revise', [first.id]));
  assert.throws(() => readGeneratedOperations(JSON.stringify({ operations: [{ ...update, node: { ...second, name: 'Unauthorized' } }] }), document, 'revise', [first.id]));
  assert.throws(() => readGeneratedOperations(JSON.stringify({ operations: [update] }), document, 'variant', [first.id]));
  const alternative = { ...createDesignNode('frame', 320, 0, 'Alternative', ''), id: 'variant-frame' };
  assert.equal(readGeneratedOperations(JSON.stringify({ operations: [{ type: 'create', node: alternative }] }), document, 'variant', [first.id]).length, 1);
});

test('AI can create nested frames in operation order and cannot attach nodes to unknown parents', () => {
  const operations = [
    { type: 'create', node: { id: 'generated-frame', type: 'frame', name: 'Screen', x: 0, y: 0, width: 320, height: 640, parentId: null, rotation: 0, visible: true, locked: false, fill: '#FFFFFF', autoLayout: { direction: 'vertical', alignment: 'stretch', gap: 12, padding: 16 } } },
    { type: 'create', node: { id: 'generated-title', type: 'text', name: 'Title', x: 16, y: 20, width: 200, height: 40, parentId: 'generated-frame', rotation: 0, visible: true, locked: false, content: 'Welcome', color: '#000000', fontSize: 24, fontWeight: 500, constraints: { horizontal: 'start-end', vertical: 'center' } } },
  ];
  const document = documentWith([]);
  const accepted = readGeneratedOperations(JSON.stringify({ operations }), document);
  const generatedDocument = applyDesignOperations(document, accepted);
  assert.equal(generatedDocument.nodes[1]?.parentId, 'generated-frame');
  assert.deepEqual(generatedDocument.nodes[0]?.type === 'frame' ? generatedDocument.nodes[0].autoLayout : null, operations[0].node.autoLayout);
  assert.deepEqual(generatedDocument.nodes[1]?.constraints, operations[1].node.constraints);

  const invalid = [{ ...operations[0], node: { ...operations[0].node, id: 'orphan', parentId: 'missing-frame' } }];
  assert.throws(() => readGeneratedOperations(JSON.stringify({ operations: invalid }), document));
  const invalidLayout = [{ ...operations[0], node: { ...operations[0].node, autoLayout: { ...operations[0].node.autoLayout, gap: 5000 } } }];
  assert.throws(() => readGeneratedOperations(JSON.stringify({ operations: invalidLayout }), document));
  const invalidConstraints = [{ ...operations[1], node: { ...operations[1].node, constraints: { horizontal: 'free', vertical: 'center' } } }];
  assert.throws(() => readGeneratedOperations(JSON.stringify({ operations: invalidConstraints }), document));
});
