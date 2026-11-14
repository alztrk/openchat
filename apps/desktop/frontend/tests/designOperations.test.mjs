import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

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
const documentModule = await import(`data:text/javascript;base64,${Buffer.from(compiledDocument).toString('base64')}`);
globalThis.__openChatDesignOperationValidator = documentModule.validateDesignDocument;
globalThis.__openChatDesignPathPointParser = documentModule.parseDesignPathPoints;
globalThis.__openChatDesignNodeFactory = documentModule.createDesignNode;
const operationsSource = fs.readFileSync(path.resolve('src/features/design-workspace/data/designOperations.ts'), 'utf8')
  .replace(
    /import \{[\s\S]*?\} from '\.\/designDocument';/,
    'const validateDesignDocument = globalThis.__openChatDesignOperationValidator; const parseDesignPathPoints = globalThis.__openChatDesignPathPointParser; const createDesignNode = globalThis.__openChatDesignNodeFactory; const maximumDesignNodeCount = 10_000;',
  );
const compiledOperations = ts.transpileModule(operationsSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { applyDesignTextStyle, arrangeDesignSelection, createDesignComponent, createDesignComponentInstance, createDesignComponentVariant, createDesignTextStyle, designNodesForResponsiveBreakpoint, expandDesignMoveSelection, groupDesignSelection, importSharedDesignTextStyle, removeDesignNodes, removeDesignMotionTrack, removeDesignTextStyle, renameDesignNode, reorderDesignSelection, reflowDesignFrameConstraints, rotateDesignSelection, saveDesignResponsiveOverrides, setDesignFrameAutoLayout, setDesignPrototypeFlow, setDesignPrototypeInteraction, removeDesignPrototypeInteraction, setDesignMotionTrack, setDesignSelectionShadow, setDesignSelectionConstraints, snapDesignMove, synchronizeDesignComponentDependents, updateDesignImageSelection, updateDesignPathPoints, updateDesignResponsiveGeometry, updateDesignTextSelection, updateDesignTextStyle } = await import(`data:text/javascript;base64,${Buffer.from(compiledOperations).toString('base64')}`);
const { createDesignNode } = documentModule;

function documentWith(nodes, selectedNodeIds) {
  return { version: 9, id: 'document-1', name: 'Design', projectId: null, pages: [{ id: 'page-1', name: 'Page 1' }], activePageId: 'page-1', nodes, selectedNodeIds, variables: [], textStyles: [], comments: [], prototypeFlows: [], prototypeInteractions: [], motionTracks: [], components: [] };
}

test('grouping selected layers creates a frame without changing their canvas positions', () => {
  const first = { ...createDesignNode('rectangle', 40, 60, 'First', ''), id: 'first', width: 80, height: 40 };
  const second = { ...createDesignNode('text', 160, 100, 'Second', 'Text'), id: 'second', width: 100, height: 24 };
  const source = documentWith([first, second], [first.id, second.id]);

  const grouped = groupDesignSelection(source, 'Group');
  const frame = grouped.nodes.find((node) => node.id === grouped.selectedNodeIds[0]);

  assert.equal(frame?.type, 'frame');
  assert.deepEqual([frame?.x, frame?.y, frame?.width, frame?.height], [24, 44, 252, 96]);
  assert.deepEqual(grouped.nodes.filter((node) => node.id === 'first' || node.id === 'second').map((node) => [node.x, node.y, node.parentId]), [
    [40, 60, frame?.id], [160, 100, frame?.id],
  ]);
  assert.deepEqual(expandDesignMoveSelection(grouped, [frame.id]).map((node) => node.id), [frame.id, 'first', 'second']);
});

test('grouping refuses locked layers and preserves the source document', () => {
  const first = { ...createDesignNode('rectangle', 0, 0, 'First', ''), id: 'first', locked: true };
  const second = { ...createDesignNode('rectangle', 80, 0, 'Second', ''), id: 'second' };
  const source = documentWith([first, second], [first.id, second.id]);

  assert.throws(() => groupDesignSelection(source, 'Group'));
  assert.deepEqual(source.nodes.map((node) => node.parentId), [null, null]);
});

test('moving selection snaps matching edges and centers to visible peer nodes', () => {
  const moving = { ...createDesignNode('rectangle', 0, 0), id: 'moving', width: 40, height: 20 };
  const target = { ...createDesignNode('rectangle', 100, 100), id: 'target', width: 40, height: 20 };
  const result = snapDesignMove([moving], [moving, target], 58, 77, 6);

  assert.deepEqual(result, {
    deltaX: 60,
    deltaY: 80,
    guides: { vertical: 100, horizontal: 100 },
  });
});

test('alignment snapping ignores hidden and selected peers and stays within screen tolerance', () => {
  const moving = { ...createDesignNode('rectangle', 0, 0), id: 'moving', width: 20, height: 20 };
  const selectedPeer = { ...moving, id: 'selected-peer', x: 200, y: 200 };
  const hiddenPeer = { ...moving, id: 'hidden-peer', x: 100, visible: false };
  const distantPeer = { ...moving, id: 'distant-peer', x: 300, y: 300 };
  const result = snapDesignMove([moving, selectedPeer], [moving, selectedPeer, hiddenPeer, distantPeer], 15, 0, 4);

  assert.equal(result.deltaX, 15);
  assert.equal(result.guides.vertical, null);
  assert.equal(result.guides.horizontal, null);
});

test('reusable text styles apply to selected text, update linked dependents, and detach safely', () => {
  const first = { ...createDesignNode('text', 0, 0, 'Heading', 'First'), id: 'text-first' };
  const second = { ...createDesignNode('text', 0, 48, 'Heading', 'Second'), id: 'text-second' };
  const source = documentWith([first, second], [first.id]);
  const created = createDesignTextStyle(source, 'Display');
  const style = created.textStyles[0];
  assert.ok(style);

  const applied = applyDesignTextStyle({ ...created, selectedNodeIds: [first.id, second.id] }, style.id);
  assert.equal(applied.nodes.find((node) => node.id === second.id)?.type === 'text' ? applied.nodes.find((node) => node.id === second.id)?.textStyleId : undefined, style.id);
  const updated = updateDesignTextStyle(applied, style.id, { name: 'Display large', color: '#123456', fontSize: 32, fontWeight: 700 });
  const linked = updated.nodes.filter((node) => node.type === 'text');
  assert.deepEqual(linked.map((node) => node.type === 'text' ? [node.color, node.fontSize, node.fontWeight] : []), [
    ['#123456', 32, 700], ['#123456', 32, 700],
  ]);

  const manuallyEdited = updateDesignTextSelection({ ...updated, selectedNodeIds: [first.id] }, {
    content: 'Changed', color: '#000000', fontSize: 20, fontWeight: 500,
  });
  assert.equal(manuallyEdited.nodes.find((node) => node.id === first.id)?.type === 'text' ? manuallyEdited.nodes.find((node) => node.id === first.id)?.textStyleId : 'invalid', undefined);
  const detached = removeDesignTextStyle(manuallyEdited, style.id);
  assert.equal(detached.nodes.find((node) => node.id === second.id)?.type === 'text' ? detached.nodes.find((node) => node.id === second.id)?.textStyleId : 'invalid', undefined);
  assert.deepEqual(detached.textStyles, []);
});

test('shared text style imports preserve stable library IDs and refresh existing document copies', () => {
  const title = { ...createDesignNode('text', 0, 0, 'Heading', 'Title'), id: 'shared-style-node' };
  const source = documentWith([title], [title.id]);
  const first = importSharedDesignTextStyle(source, {
    id: 'library-style-1', name: 'Heading', color: '#234567', fontSize: 24, fontWeight: 600,
  });
  assert.equal(first.nodes[0]?.type === 'text' ? first.nodes[0]?.textStyleId : undefined, 'library-style-1');

  const refreshed = importSharedDesignTextStyle(first, {
    id: 'library-style-1', name: 'Heading', color: '#345678', fontSize: 28, fontWeight: 700,
  });
  assert.equal(refreshed.textStyles.length, 1);
  assert.equal(refreshed.nodes[0]?.type === 'text' ? refreshed.nodes[0]?.color : undefined, '#345678');
  assert.throws(() => importSharedDesignTextStyle({
    ...source,
    textStyles: [{ id: 'local-style', name: 'Heading', color: '#000000', fontSize: 20, fontWeight: 500 }],
  }, { id: 'library-style-2', name: 'heading', color: '#234567', fontSize: 24, fontWeight: 600 }));
});

test('responsive layout overrides save selected geometry without changing the base layout', () => {
  const shape = createDesignNode('rectangle', 20, 30, 'Card', '');
  const locked = { ...createDesignNode('rectangle', 50, 60, 'Locked', ''), locked: true };
  const source = documentWith([shape, locked], [shape.id, locked.id]);
  const mobile = saveDesignResponsiveOverrides(source, 'mobile');

  assert.deepEqual(mobile.nodes.find((node) => node.id === shape.id)?.responsiveOverrides?.mobile,
    { x: shape.x, y: shape.y, width: shape.width, height: shape.height });
  assert.equal(mobile.nodes.find((node) => node.id === shape.id)?.x, shape.x);
  assert.equal(mobile.nodes.find((node) => node.id === locked.id)?.responsiveOverrides, undefined);
  const tablet = { ...shape, responsiveOverrides: { tablet: { x: 8, y: 12, width: 640, height: 420 } } };
  const tabletPreview = designNodesForResponsiveBreakpoint([tablet], 'tablet')[0];
  assert.deepEqual(tabletPreview && { x: tabletPreview.x, y: tabletPreview.y, width: tabletPreview.width, height: tabletPreview.height },
    { x: 8, y: 12, width: 640, height: 420 });
  assert.equal(designNodesForResponsiveBreakpoint(mobile.nodes, null), mobile.nodes);
  const edited = updateDesignResponsiveGeometry(mobile, 'mobile', shape.id, { x: 8, y: 12, width: 320, height: 240 });
  assert.deepEqual(edited.nodes.find((node) => node.id === shape.id)?.responsiveOverrides?.mobile,
    { x: 8, y: 12, width: 320, height: 240 });
  assert.equal(edited.nodes.find((node) => node.id === shape.id)?.x, shape.x);
  assert.throws(() => updateDesignResponsiveGeometry(mobile, 'mobile', locked.id, { x: 0, y: 0, width: 1, height: 1 }));
  assert.throws(() => saveDesignResponsiveOverrides(documentWith([locked], [locked.id]), 'tablet'));
  assert.throws(() => saveDesignResponsiveOverrides(source, 'desktop'));
});

test('components create variants and instances and synchronize master styling', () => {
  const frame = { ...createDesignNode('frame', 0, 0, 'Button', ''), width: 140, height: 48 };
  const label = { ...createDesignNode('text', 12, 8, 'Label', 'Continue'), parentId: frame.id };
  const base = documentWith([frame, label], [frame.id]);
  const component = createDesignComponent(base, 'Action button');
  const definition = component.components[0];
  assert.ok(definition);
  assert.equal(component.nodes.find((node) => node.id === frame.id)?.componentRef?.role, 'master');

  const variant = createDesignComponentVariant(component, definition.id, 'Compact');
  const compact = variant.components[0]?.variants.find((item) => item.name === 'Compact');
  assert.ok(compact);
  const withInstance = createDesignComponentInstance(variant, definition.id, compact.id);
  const instanceLabel = withInstance.nodes.find((node) => node.componentRef?.role === 'instance' && node.componentRef.sourceNodeId === label.id);
  assert.ok(instanceLabel);
  assert.equal(instanceLabel.componentRef?.variantId, compact.id);

  const changedMaster = updateDesignTextSelection({ ...withInstance, selectedNodeIds: [label.id] }, { content: 'Next', color: '#000000', fontSize: 24, fontWeight: 500 });
  const synced = synchronizeDesignComponentDependents(withInstance, changedMaster);
  const syncedVariantLabel = synced.nodes.find((node) => node.componentRef?.role === 'variant' && node.componentRef.sourceNodeId === label.id);
  assert.equal(syncedVariantLabel?.type === 'text' ? syncedVariantLabel.content : null, 'Next');
  const removed = removeDesignNodes(withInstance, new Set([frame.id]));
  assert.deepEqual(removed.components, []);
  assert.equal(removed.nodes.some((node) => node.componentRef?.componentId === definition.id), false);
  assert.throws(() => createDesignComponent(base, ''));
});

test('prototype flows and click or hover interactions validate frame targets and transitions', () => {
  const start = createDesignNode('frame', 0, 0, 'Start', '');
  const target = createDesignNode('frame', 500, 0, 'Target', '');
  const button = { ...createDesignNode('rectangle', 40, 40, 'Continue', ''), parentId: start.id };
  const document = documentWith([start, target, button], [button.id]);
  const flow = setDesignPrototypeFlow(document, 'Checkout', start.id);
  const linked = setDesignPrototypeInteraction(flow, button.id, target.id, 'dissolve');
  assert.equal(linked.prototypeFlows[0]?.startNodeId, start.id);
  assert.deepEqual(linked.prototypeInteractions[0], {
    id: linked.prototypeInteractions[0]?.id,
    sourceNodeId: button.id,
    targetNodeId: target.id,
    trigger: 'click',
    presentation: 'navigate',
    transition: 'dissolve',
    durationMs: 240,
  });
  assert.deepEqual(removeDesignPrototypeInteraction(linked, button.id).prototypeInteractions, []);
  assert.equal(setDesignPrototypeInteraction(flow, button.id, target.id, 'instant', 'hover').prototypeInteractions[0]?.trigger, 'hover');
  assert.equal(setDesignPrototypeInteraction(flow, button.id, target.id, 'instant', 'click', 'overlay').prototypeInteractions[0]?.presentation, 'overlay');
  assert.throws(() => setDesignPrototypeInteraction(document, button.id, button.id, 'instant'));
  assert.throws(() => setDesignPrototypeInteraction(document, button.id, 'missing-frame', 'instant'));
});

test('motion tracks use bounded ordered keyframes and follow node deletion', () => {
  const shape = createDesignNode('rectangle', 10, 20, 'Fade in', '');
  const document = documentWith([shape], [shape.id]);
  const animated = setDesignMotionTrack(document, shape.id, 'opacity', [
    { timeMs: 0, value: 0, easing: 'ease-in' },
    { timeMs: 600, value: 1, easing: 'ease-out' },
  ]);

  assert.equal(animated.motionTracks.length, 1);
  assert.deepEqual(animated.motionTracks[0]?.keyframes.map(({ timeMs, value }) => ({ timeMs, value })), [
    { timeMs: 0, value: 0 },
    { timeMs: 600, value: 1 },
  ]);
  assert.throws(() => setDesignMotionTrack(document, shape.id, 'opacity', [
    { timeMs: 0, value: -1, easing: 'linear' },
    { timeMs: 600, value: 1, easing: 'linear' },
  ]));
  assert.equal(removeDesignMotionTrack(animated, animated.motionTracks[0].id).motionTracks.length, 0);
  assert.equal(removeDesignNodes(animated, new Set([shape.id])).motionTracks.length, 0);
});

test('vector path point edits preserve the node and reject invalid geometry', () => {
  const path = createDesignNode('path', 40, 60, 'Triangle', '');
  const document = documentWith([path], [path.id]);
  const updated = updateDesignPathPoints(document, path.id, [{ x: 70, y: 4 }, { x: 150, y: 110 }, { x: 10, y: 110 }]);
  assert.deepEqual(updated.nodes[0]?.type === 'path' ? updated.nodes[0].points : null, [{ x: 70, y: 4 }, { x: 150, y: 110 }, { x: 10, y: 110 }]);
  assert.throws(() => updateDesignPathPoints(document, path.id, [{ x: 1, y: 2 }]));
});

test('aligning selected nodes preserves their dimensions and unselected nodes', () => {
  const first = { ...createDesignNode('rectangle', 10, 20, 'First', ''), width: 40 };
  const second = { ...createDesignNode('ellipse', 100, 60, 'Second', ''), width: 20 };
  const untouched = createDesignNode('text', 400, 500, 'Untouched', 'Text');
  const result = arrangeDesignSelection(documentWith([first, second, untouched], [first.id, second.id]), 'align-right');

  assert.equal(result.nodes[0]?.x, 80);
  assert.equal(result.nodes[0]?.width, 40);
  assert.equal(result.nodes[1]?.x, 100);
  assert.equal(result.nodes[2]?.x, untouched.x);
});

test('distribution spaces intermediate nodes evenly while keeping endpoints fixed', () => {
  const nodes = [
    { ...createDesignNode('rectangle', 0, 0, 'First', ''), width: 20 },
    { ...createDesignNode('rectangle', 50, 0, 'Middle', ''), width: 20 },
    { ...createDesignNode('rectangle', 120, 0, 'Last', ''), width: 20 },
  ];
  const result = arrangeDesignSelection(documentWith(nodes, nodes.map((node) => node.id)), 'distribute-horizontal');

  assert.deepEqual(result.nodes.map((node) => node.x), [0, 60, 120]);
});

test('distribution leaves selections with too few nodes unchanged', () => {
  const nodes = [createDesignNode('rectangle', 0, 0, 'First', ''), createDesignNode('rectangle', 20, 0, 'Second', '')];
  const document = documentWith(nodes, nodes.map((node) => node.id));
  assert.deepEqual(arrangeDesignSelection(document, 'distribute-vertical'), document);
});

test('reordering preserves selected block order and moves it to the requested stacking edge', () => {
  const nodes = [
    createDesignNode('rectangle', 0, 0, 'Back', ''),
    createDesignNode('rectangle', 0, 0, 'Selected one', ''),
    createDesignNode('rectangle', 0, 0, 'Middle', ''),
    createDesignNode('rectangle', 0, 0, 'Selected two', ''),
  ];
  const document = documentWith(nodes, [nodes[1].id, nodes[3].id]);

  assert.deepEqual(reorderDesignSelection(document, 'front').nodes.map((node) => node.name), ['Back', 'Middle', 'Selected one', 'Selected two']);
  assert.deepEqual(reorderDesignSelection(document, 'back').nodes.map((node) => node.name), ['Selected one', 'Selected two', 'Back', 'Middle']);
  assert.deepEqual(document.nodes.map((node) => node.name), ['Back', 'Selected one', 'Middle', 'Selected two']);
});

test('renaming validates names and changes only the addressed layer', () => {
  const nodes = [createDesignNode('rectangle', 0, 0, 'Card', ''), createDesignNode('text', 0, 0, 'Label', 'Text')];
  const document = documentWith(nodes, [nodes[0].id]);
  const renamed = renameDesignNode(document, nodes[0].id, 'Primary card');

  assert.equal(renamed.nodes[0]?.name, 'Primary card');
  assert.equal(renamed.nodes[1]?.name, 'Label');
  assert.equal(document.nodes[0]?.name, 'Card');
  assert.throws(() => renameDesignNode(document, nodes[0].id, '  '));
  assert.throws(() => renameDesignNode(document, nodes[0].id, 'bad\nname'));
});

test('rotation applies in bounded steps to unlocked selected nodes only', () => {
  const nodes = [
    { ...createDesignNode('rectangle', 0, 0, 'Selected', ''), rotation: 350 },
    { ...createDesignNode('ellipse', 0, 0, 'Locked', ''), locked: true },
    createDesignNode('text', 0, 0, 'Unselected', ''),
  ];
  const document = documentWith(nodes, [nodes[0].id, nodes[1].id]);
  const result = rotateDesignSelection(document, 15);

  assert.equal(result.nodes[0]?.rotation, 5);
  assert.equal(result.nodes[1]?.rotation, 0);
  assert.equal(result.nodes[2]?.rotation, 0);
});

test('shadow effects are structured, bounded, and removable from selected nodes', () => {
  const nodes = [createDesignNode('rectangle', 0, 0, 'Card', ''), createDesignNode('ellipse', 0, 0, 'Locked', '')];
  nodes[1].locked = true;
  const document = documentWith(nodes, [nodes[0].id, nodes[1].id]);
  const effected = setDesignSelectionShadow(document, { type: 'shadow', color: '#112233', offsetX: 2, offsetY: 4, blur: 12 });

  assert.deepEqual(effected.nodes[0]?.effects, [{ type: 'shadow', color: '#112233', offsetX: 2, offsetY: 4, blur: 12 }]);
  assert.deepEqual(effected.nodes[1]?.effects, []);
  assert.throws(() => setDesignSelectionShadow(document, { type: 'shadow', color: 'red', offsetX: 0, offsetY: 0, blur: 12 }));
  assert.deepEqual(setDesignSelectionShadow(effected, null).nodes[0]?.effects, []);
});

test('text editing validates structured typography and leaves non-text nodes unchanged', () => {
  const text = createDesignNode('text', 0, 0, 'Heading', 'Old');
  const shape = createDesignNode('rectangle', 0, 0, 'Shape', '');
  const document = documentWith([text, shape], [text.id, shape.id]);
  const edited = updateDesignTextSelection(document, { content: 'New heading', color: '#123456', fontSize: 32, fontWeight: 700 });

  assert.equal(edited.nodes[0]?.type === 'text' ? edited.nodes[0].content : null, 'New heading');
  assert.equal(edited.nodes[0]?.type === 'text' ? edited.nodes[0].fontWeight : null, 700);
  assert.deepEqual(edited.nodes[1], shape);
  assert.throws(() => updateDesignTextSelection(document, { content: 'Text', color: 'red', fontSize: 32, fontWeight: 700 }));
});

test('image editing validates fill behavior and alternative text on the selected image', () => {
  const image = { ...createDesignNode('rectangle', 0, 0, 'Reference', ''), type: 'image', assetId: '0123456789abcdef0123456789abcdef', mimeType: 'image/png', sizeBytes: 16, fit: 'cover', altText: 'Old description' };
  const document = documentWith([image], [image.id]);
  const edited = updateDesignImageSelection(document, { fit: 'contain', altText: 'Studio apartment floor plan' });

  assert.equal(edited.nodes[0]?.type === 'image' ? edited.nodes[0].fit : null, 'contain');
  assert.equal(edited.nodes[0]?.type === 'image' ? edited.nodes[0].altText : null, 'Studio apartment floor plan');
  assert.equal(document.nodes[0]?.type === 'image' ? document.nodes[0].fit : null, 'cover');
  assert.throws(() => updateDesignImageSelection(document, { fit: 'stretch', altText: 'Description' }));
  assert.throws(() => updateDesignImageSelection(document, { fit: 'cover', altText: 'x'.repeat(257) }));
  assert.throws(() => updateDesignImageSelection(documentWith([{ ...image, locked: true }], [image.id]), { fit: 'contain', altText: 'Locked' }));
});

test('frame auto layout positions children and supports responsive stretch on resize', () => {
  const frame = { ...createDesignNode('frame', 100, 200, 'Stack', ''), width: 240, height: 180 };
  const first = { ...createDesignNode('rectangle', 400, 400, 'First', ''), width: 40, height: 20, parentId: frame.id };
  const second = { ...createDesignNode('text', 500, 500, 'Second', 'Text'), width: 60, height: 30, parentId: frame.id };
  const document = documentWith([frame, first, second], [frame.id]);
  const layout = { direction: 'vertical', alignment: 'stretch', gap: 8, padding: 12 };
  const configured = setDesignFrameAutoLayout(document, frame.id, layout);

  assert.deepEqual(configured.nodes.map((node) => [node.x, node.y, node.width, node.height]), [
    [100, 200, 240, 180],
    [112, 212, 216, 20],
    [112, 240, 216, 30],
  ]);
  const resized = { ...configured, nodes: configured.nodes.map((node) => node.id === frame.id && node.type === 'frame' ? { ...node, height: 240 } : node) };
  const reflowed = setDesignFrameAutoLayout(resized, frame.id, layout);
  assert.equal(reflowed.nodes[1]?.width, 216);
  assert.equal(reflowed.nodes[2]?.y, 240);
  const withoutLayout = setDesignFrameAutoLayout(reflowed, frame.id, null);
  assert.equal(withoutLayout.nodes[0]?.type === 'frame' ? withoutLayout.nodes[0].autoLayout : 'invalid', undefined);
});

test('moving a frame expands to its nested descendants while selection remains intact', () => {
  const outer = createDesignNode('frame', 0, 0, 'Outer', '');
  const inner = { ...createDesignNode('frame', 10, 10, 'Inner', ''), parentId: outer.id };
  const child = { ...createDesignNode('rectangle', 20, 20, 'Child', ''), parentId: inner.id };
  const document = documentWith([outer, inner, child], [outer.id]);

  assert.deepEqual(expandDesignMoveSelection(document, [outer.id]).map((node) => node.id), [outer.id, inner.id, child.id]);
  assert.deepEqual(document.selectedNodeIds, [outer.id]);
  assert.throws(() => expandDesignMoveSelection(document, ['missing']));
});

test('frame resize applies each child constraint mode predictably', () => {
  const frame = { ...createDesignNode('frame', 0, 0, 'Responsive frame', ''), width: 100, height: 100 };
  const modes = ['start', 'end', 'start-end', 'center', 'scale'];
  const children = modes.map((mode) => ({
    ...createDesignNode('rectangle', 10, 20, `Child ${mode}`, ''),
    parentId: frame.id,
    width: 30,
    height: 40,
    constraints: { horizontal: mode, vertical: mode },
  }));
  const document = documentWith([frame, ...children], [children[0].id]);
  const resized = { ...document, nodes: document.nodes.map((node) => node.id === frame.id && node.type === 'frame' ? { ...node, width: 200, height: 150 } : node) };
  const result = reflowDesignFrameConstraints(resized, frame, children);
  assert.deepEqual(result.nodes.slice(1).map((node) => [node.x, node.y, node.width, node.height]), [
    [10, 20, 30, 40],
    [110, 70, 30, 40],
    [10, 20, 130, 90],
    [60, 45, 30, 40],
    [20, 30, 60, 60],
  ]);

  const constrained = setDesignSelectionConstraints(document, { horizontal: 'end', vertical: 'center' });
  assert.deepEqual(constrained.nodes[1]?.constraints, { horizontal: 'end', vertical: 'center' });
  assert.throws(() => setDesignSelectionConstraints(document, { horizontal: 'free', vertical: 'center' }));
});
