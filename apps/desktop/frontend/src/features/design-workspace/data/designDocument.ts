import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';
import {
  designCanvasDataUrl,
  parseDesignCanvasTokens,
  readDesignCanvas,
  sanitizeDesignCanvasSvg,
  type DesignCanvasFrame,
  type DesignCanvasTokens,
} from '@features/design-workspace/data/designCanvas';

const storageKey = 'design.document.v9';
const previousStorageKeys = ['design.document.v8', 'design.document.v7', 'design.document.v6', 'design.document.v5', 'design.document.v4', 'design.document.v2'] as const;
const maximumDocumentBytes = 8 * 1024 * 1024;
export const maximumDesignNodeCount = 10_000;
export const maximumDesignMotionTrackCount = 5_000;

interface NodeBase {
  id: string;
  pageId: string;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  parentId: string | null;
  rotation: number;
  visible: boolean;
  locked: boolean;
  effects: DesignShadowEffect[];
  constraints?: DesignConstraints;
  componentRef?: DesignComponentReference;
  textStyleId?: string;
  responsiveOverrides?: Partial<Record<DesignResponsiveBreakpoint, DesignResponsiveGeometry>>;
}

export interface DesignComponentReference {
  componentId: string;
  sourceNodeId: string;
  variantId: string;
  role: 'master' | 'variant' | 'instance';
}

export type DesignConstraintMode = 'start' | 'end' | 'start-end' | 'center' | 'scale';

export interface DesignConstraints {
  horizontal: DesignConstraintMode;
  vertical: DesignConstraintMode;
}

export interface DesignShadowEffect {
  type: 'shadow';
  color: string;
  offsetX: number;
  offsetY: number;
  blur: number;
}

export interface DesignCornerRadii {
  topLeft: number;
  topRight: number;
  bottomRight: number;
  bottomLeft: number;
}

export interface DesignGradientStop {
  position: number;
  color: string;
}

export type DesignFillStyle =
  | { type: 'gradient'; endColor: string; angle: number; stops?: DesignGradientStop[] }
  | { type: 'image'; assetId: string; mimeType: DesignImageNode['mimeType']; sizeBytes: number; fit: 'cover' | 'contain' };

export interface DesignFrameNode extends NodeBase {
  type: 'frame';
  scrollable?: boolean;
  autoLayout?: DesignFrameAutoLayout;
  fill: string;
  fillStyle?: DesignFillStyle;
  radius?: number;
  cornerRadii?: DesignCornerRadii;
  fillVariableId?: string;
  stroke: string;
  strokeWidth: number;
}

export type DesignResponsiveBreakpoint = 'mobile' | 'tablet';

export interface DesignResponsiveGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface DesignFrameAutoLayout {
  direction: 'horizontal' | 'vertical';
  alignment: 'start' | 'center' | 'end' | 'stretch';
  gap: number;
  padding: number;
}

export interface DesignRectangleNode extends NodeBase {
  type: 'rectangle';
  fill: string;
  fillStyle?: DesignFillStyle;
  radius: number;
  cornerRadii?: DesignCornerRadii;
  fillVariableId?: string;
  stroke: string;
  strokeWidth: number;
}

export interface DesignEllipseNode extends NodeBase {
  type: 'ellipse';
  fill: string;
  fillStyle?: DesignFillStyle;
  fillVariableId?: string;
  stroke: string;
  strokeWidth: number;
}

export interface DesignPathNode extends NodeBase {
  type: 'path';
  points: Array<{ x: number; y: number }>;
  closed: boolean;
  fill: string;
  fillStyle?: DesignFillStyle;
  fillVariableId?: string;
  stroke: string;
  strokeWidth: number;
}

export interface DesignTextNode extends NodeBase {
  type: 'text';
  content: string;
  color: string;
  fontSize: number;
  fontWeight: number;
  fontFamily?: 'sans-serif' | 'serif' | 'monospace';
  lineHeight?: number;
  letterSpacing?: number;
  textAlign?: 'left' | 'center' | 'right';
  colorVariableId?: string;
}

export interface DesignSvgNode extends NodeBase {
  type: 'imported-svg';
  svg: string;
  tokens: DesignCanvasTokens;
}

export interface DesignImageNode extends NodeBase {
  type: 'image';
  assetId: string;
  mimeType: 'image/png' | 'image/jpeg' | 'image/webp';
  sizeBytes: number;
  fit: 'cover' | 'contain';
  altText: string;
}

export type DesignNode =
  | DesignFrameNode
  | DesignRectangleNode
  | DesignEllipseNode
  | DesignPathNode
  | DesignTextNode
  | DesignSvgNode
  | DesignImageNode;

export interface DesignColorVariable {
  id: string;
  name: string;
  value: string;
}

export interface DesignTextStyle {
  id: string;
  name: string;
  color: string;
  fontSize: number;
  fontWeight: number;
  fontFamily?: DesignTextNode['fontFamily'];
  lineHeight?: number;
  letterSpacing?: number;
  textAlign?: DesignTextNode['textAlign'];
}

export interface DesignComponentVariant {
  id: string;
  name: string;
  rootNodeId: string;
}

export interface DesignComponent {
  id: string;
  name: string;
  variants: DesignComponentVariant[];
}

export interface DesignComment {
  id: string;
  pageId: string;
  nodeId: string | null;
  text: string;
  point: { x: number; y: number } | null;
  createdAt: number;
  resolved: boolean;
}

export interface DesignPrototypeFlow {
  id: string;
  name: string;
  pageId: string;
  startNodeId: string;
}

export interface DesignPrototypeInteraction {
  id: string;
  sourceNodeId: string;
  targetNodeId: string;
  trigger: 'click' | 'hover';
  presentation: 'navigate' | 'overlay';
  transition: 'instant' | 'dissolve' | 'slide';
  durationMs: number;
}

export type DesignMotionProperty = 'x' | 'y' | 'rotation' | 'opacity' | 'scale';
export type DesignMotionEasing = 'linear' | 'ease-in' | 'ease-out' | 'ease-in-out';

export interface DesignMotionKeyframe {
  id: string;
  timeMs: number;
  value: number;
  easing: DesignMotionEasing;
}

export interface DesignMotionTrack {
  id: string;
  nodeId: string;
  property: DesignMotionProperty;
  keyframes: DesignMotionKeyframe[];
}

export interface DesignPage {
  id: string;
  name: string;
}

export interface DesignDocument {
  version: 9;
  id: string;
  name: string;
  projectId: string | null;
  pages: DesignPage[];
  activePageId: string;
  nodes: DesignNode[];
  selectedNodeIds: string[];
  variables: DesignColorVariable[];
  textStyles: DesignTextStyle[];
  comments: DesignComment[];
  prototypeFlows: DesignPrototypeFlow[];
  prototypeInteractions: DesignPrototypeInteraction[];
  motionTracks: DesignMotionTrack[];
  components: DesignComponent[];
}

export type DesignDocumentReadResult =
  | { kind: 'loading' }
  | { kind: 'ready'; document: DesignDocument; needsMigration: boolean }
  | { kind: 'recoveryRequired'; rawData: string; reason: 'invalidJson' | 'invalidStructure' | 'tooLarge' }
  | { kind: 'unavailable' };

export function emptyDesignDocument(firstPageName = 'Page 1'): DesignDocument {
  const pageId = crypto.randomUUID();
  return {
    version: 9,
    id: crypto.randomUUID(),
    name: '',
    projectId: null,
    pages: [{ id: pageId, name: firstPageName }],
    activePageId: pageId,
    nodes: [],
    selectedNodeIds: [],
    variables: [],
    textStyles: [],
    comments: [],
    prototypeFlows: [],
    prototypeInteractions: [],
    motionTracks: [],
    components: [],
  };
}

export function readDesignDocument(): DesignDocumentReadResult {
  let encoded: string | null;
  let storedUnderPreviousKey = false;
  try {
    encoded = unifiedStorage.getItem(storageKey);
    if (encoded === null) {
      for (const previousStorageKey of previousStorageKeys) {
        encoded = unifiedStorage.getItem(previousStorageKey);
        if (encoded !== null) {
          storedUnderPreviousKey = true;
          break;
        }
      }
    }
  } catch {
    return { kind: 'unavailable' };
  }

  if (encoded !== null) {
    if (new TextEncoder().encode(encoded).byteLength > maximumDocumentBytes) {
      return { kind: 'recoveryRequired', rawData: encoded, reason: 'tooLarge' };
    }
    let value: unknown;
    try {
      value = JSON.parse(encoded);
    } catch {
      return { kind: 'recoveryRequired', rawData: encoded, reason: 'invalidJson' };
    }
    try {
      return { kind: 'ready', document: validateDesignDocument(value), needsMigration: storedUnderPreviousKey || !isRecord(value) || value.version !== 9 };
    } catch {
      return { kind: 'recoveryRequired', rawData: encoded, reason: 'invalidStructure' };
    }
  }

  const legacy = readDesignCanvas();
  if (legacy.kind === 'unavailable') return { kind: 'unavailable' };
  if (legacy.kind === 'recoveryRequired') return legacy;
  return { kind: 'ready', document: migrateLegacyCanvas(legacy.canvas.frames), needsMigration: true };
}

export async function saveDesignDocument(document: DesignDocument): Promise<void> {
  const validated = validateDesignDocument(document);
  const encoded = JSON.stringify(validated);
  if (new TextEncoder().encode(encoded).byteLength > maximumDocumentBytes) {
    throw new Error('The design document exceeds its storage limit.');
  }
  await unifiedStorage.setPersistedItem(storageKey, encoded);
}

export function createDesignNode(
  type: 'frame' | 'rectangle' | 'ellipse' | 'path' | 'text',
  x: number,
  y: number,
  name: string,
  defaultText: string,
  pageId = 'page-1',
): DesignNode {
  const base: NodeBase = {
    id: crypto.randomUUID(),
    pageId,
    name,
    x,
    y,
    width: type === 'frame' ? 390 : type === 'text' ? 220 : 160,
    height: type === 'frame' ? 844 : type === 'text' ? 48 : 120,
    parentId: null,
    rotation: 0,
    visible: true,
    locked: false,
    effects: [],
  };
  switch (type) {
    case 'frame':
      return { ...base, type, fill: '#FFFFFF', stroke: '#000000', strokeWidth: 0 };
    case 'rectangle':
      return { ...base, type, fill: '#D9DDE2', radius: 8, stroke: '#000000', strokeWidth: 0 };
    case 'ellipse':
      return { ...base, type, fill: '#8E563B', stroke: '#000000', strokeWidth: 0 };
    case 'path':
      return { ...base, type, points: [{ x: 80, y: 0 }, { x: 160, y: 120 }, { x: 0, y: 120 }], closed: true, fill: '#D9DDE2', stroke: '#000000', strokeWidth: 0 };
    case 'text':
      return { ...base, type, content: defaultText, color: '#23272C', fontSize: 24, fontWeight: 500 };
  }
}

export function createDesignImageNode(input: {
  assetId: string;
  mimeType: DesignImageNode['mimeType'];
  sizeBytes: number;
  pageId: string;
  x: number;
  y: number;
  name: string;
  altText: string;
}): DesignImageNode {
  return {
    id: crypto.randomUUID(),
    pageId: input.pageId,
    type: 'image',
    name: input.name,
    x: input.x,
    y: input.y,
    width: 320,
    height: 220,
    parentId: null,
    rotation: 0,
    visible: true,
    locked: false,
    effects: [],
    assetId: input.assetId,
    mimeType: input.mimeType,
    sizeBytes: input.sizeBytes,
    fit: 'cover',
    altText: input.altText,
  };
}

export function addDesignPage(document: DesignDocument, name: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const normalizedName = name.trim();
  if (!normalizedName || Array.from(normalizedName).length > 128 || validated.pages.length >= 100) {
    throw new Error('The design page cannot be added.');
  }
  const page = { id: crypto.randomUUID(), name: normalizedName };
  return validateDesignDocument({ ...validated, pages: [...validated.pages, page], activePageId: page.id, selectedNodeIds: [] });
}

export function renameDesignPage(document: DesignDocument, pageId: string, name: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const normalizedName = name.trim();
  if (!normalizedName || Array.from(normalizedName).length > 128 || !validated.pages.some((page) => page.id === pageId)) {
    throw new Error('The design page name is invalid.');
  }
  return validateDesignDocument({
    ...validated,
    pages: validated.pages.map((page) => page.id === pageId ? { ...page, name: normalizedName } : page),
  });
}

export function removeDesignPage(document: DesignDocument, pageId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  if (validated.pages.length < 2 || !validated.pages.some((page) => page.id === pageId)) {
    throw new Error('The design page cannot be removed.');
  }
  const pages = validated.pages.filter((page) => page.id !== pageId);
  const activePageId = validated.activePageId === pageId ? pages[0]?.id : validated.activePageId;
  if (!activePageId) throw new Error('The active design page is invalid.');
  const pageNodeIds = new Set(validated.nodes.filter((node) => node.pageId === pageId).map((node) => node.id));
  const removedComponentIds = new Set(validated.nodes.filter((node) => pageNodeIds.has(node.id) && node.componentRef?.role === 'master').map((node) => node.componentRef?.componentId).filter((id): id is string => id !== undefined));
  const removedVariantKeys = new Set(validated.nodes.filter((node) => pageNodeIds.has(node.id) && node.componentRef?.role === 'variant').map((node) => node.componentRef ? `${node.componentRef.componentId}:${node.componentRef.variantId}` : ''));
  const removedNodeIds = new Set(validated.nodes.filter((node) => pageNodeIds.has(node.id) ||
    (node.componentRef && (removedComponentIds.has(node.componentRef.componentId) || removedVariantKeys.has(`${node.componentRef.componentId}:${node.componentRef.variantId}`)))).map((node) => node.id));
  return validateDesignDocument({
    ...validated,
    pages,
    activePageId,
    nodes: validated.nodes.filter((node) => !removedNodeIds.has(node.id)),
    selectedNodeIds: validated.selectedNodeIds.filter((id) => !removedNodeIds.has(id)),
    comments: validated.comments.map((comment) => comment.nodeId && removedNodeIds.has(comment.nodeId) ? { ...comment, nodeId: null } : comment),
    components: validated.components.filter((component) => !removedComponentIds.has(component.id)).map((component) => ({
      ...component,
      variants: component.variants.filter((variant) => !removedVariantKeys.has(`${component.id}:${variant.id}`)),
    })).filter((component) => component.variants.length > 0),
    prototypeFlows: validated.prototypeFlows.filter((flow) => flow.pageId !== pageId && !removedNodeIds.has(flow.startNodeId)),
    prototypeInteractions: validated.prototypeInteractions.filter((interaction) => !removedNodeIds.has(interaction.sourceNodeId) && !removedNodeIds.has(interaction.targetNodeId)),
  });
}

export function addDesignComment(
  document: DesignDocument,
  nodeId: string,
  text: string,
  point: { x: number; y: number },
  createdAt = Date.now(),
): DesignDocument {
  const validated = validateDesignDocument(document);
  const frame = validated.nodes.find((node) => node.id === nodeId);
  const normalizedText = text.trim();
  if (frame?.type !== 'frame' || frame.pageId !== validated.activePageId || !normalizedText || Array.from(normalizedText).length > 2_000 ||
    !Number.isFinite(point.x) || !Number.isFinite(point.y) || point.x < 0 || point.x > 1 || point.y < 0 || point.y > 1 ||
    !Number.isFinite(createdAt) || validated.comments.length >= 10_000) {
    throw new Error('The design annotation is invalid.');
  }
  return validateDesignDocument({
    ...validated,
    comments: [...validated.comments, { id: crypto.randomUUID(), pageId: frame.pageId, nodeId, text: normalizedText, point, createdAt, resolved: false }],
  });
}

export function setDesignCommentResolved(document: DesignDocument, commentId: string, resolved: boolean): DesignDocument {
  const validated = validateDesignDocument(document);
  if (!validated.comments.some((comment) => comment.id === commentId)) throw new Error('The design annotation no longer exists.');
  return validateDesignDocument({ ...validated, comments: validated.comments.map((comment) => comment.id === commentId ? { ...comment, resolved } : comment) });
}

export function removeDesignComment(document: DesignDocument, commentId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  if (!validated.comments.some((comment) => comment.id === commentId)) throw new Error('The design annotation no longer exists.');
  return validateDesignDocument({ ...validated, comments: validated.comments.filter((comment) => comment.id !== commentId) });
}

export function replaceNode(document: DesignDocument, node: DesignNode): DesignDocument {
  return {
    ...document,
    nodes: document.nodes.map((current) => (current.id === node.id ? node : current)),
  };
}

export function updateDesignColorVariable(
  document: DesignDocument,
  variableId: string,
  name: string,
  value: string,
): DesignDocument {
  const validated = validateDesignDocument(document);
  const normalizedName = name.trim();
  if (!normalizedName || Array.from(normalizedName).length > 128 || !isColor(value)) {
    throw new Error('The design color variable is invalid.');
  }
  if (!validated.variables.some((variable) => variable.id === variableId)) {
    throw new Error('The design color variable does not exist.');
  }
  const nodes = validated.nodes.map((node) => {
    if ((node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' || node.type === 'path') && node.fillVariableId === variableId) {
      return { ...node, fill: value };
    }
    if (node.type === 'text' && node.colorVariableId === variableId) return { ...node, color: value };
    return node;
  });
  return validateDesignDocument({
    ...validated,
    variables: validated.variables.map((variable) => variable.id === variableId ? { ...variable, name: normalizedName, value } : variable),
    nodes,
  });
}

export function removeDesignColorVariable(document: DesignDocument, variableId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  if (!validated.variables.some((variable) => variable.id === variableId)) {
    throw new Error('The design color variable does not exist.');
  }
  const nodes = validated.nodes.map((node) => {
    if ((node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' || node.type === 'path') && node.fillVariableId === variableId) {
      const { fillVariableId: _fillVariableId, ...unlinked } = node;
      return unlinked;
    }
    if (node.type === 'text' && node.colorVariableId === variableId) {
      const { colorVariableId: _colorVariableId, ...unlinked } = node;
      return unlinked;
    }
    return node;
  });
  return validateDesignDocument({
    ...validated,
    variables: validated.variables.filter((variable) => variable.id !== variableId),
    nodes,
  });
}

export function updateDesignStroke(
  document: DesignDocument,
  selectedNodeIds: string[],
  stroke: string,
  strokeWidth: number,
): DesignDocument {
  const validated = validateDesignDocument(document);
  if (!isColor(stroke) || !Number.isFinite(strokeWidth) || strokeWidth < 0 || strokeWidth > 4096) {
    throw new Error('The design stroke is invalid.');
  }
  const selected = new Set(selectedNodeIds);
  const availableNodeIds = new Set(validated.nodes.map((node) => node.id));
  if (selected.size !== selectedNodeIds.length || selectedNodeIds.some((id) => !availableNodeIds.has(id))) {
    throw new Error('The selected design nodes are invalid.');
  }
  const nodes = validated.nodes.map((node) => {
    if (!selected.has(node.id) || (node.type !== 'frame' && node.type !== 'rectangle' && node.type !== 'ellipse' && node.type !== 'path')) return node;
    return { ...node, stroke, strokeWidth };
  });
  return validateDesignDocument({ ...validated, nodes });
}

export function migrateLegacyCanvas(frames: DesignCanvasFrame[]): DesignDocument {
  const nodes: DesignNode[] = frames.flatMap((frame) => [
    {
      id: frame.id,
      pageId: 'legacy-page-1',
      type: 'frame',
      name: frame.title,
      x: frame.x,
      y: frame.y,
      width: frame.width,
      height: frame.height,
      parentId: null,
      rotation: 0,
      visible: true,
      locked: false,
      effects: [],
      fill: frame.tokens.background,
      stroke: '#000000',
      strokeWidth: 0,
    },
    {
      id: `${frame.id}-imported-content`,
      pageId: 'legacy-page-1',
      type: 'imported-svg',
      name: `${frame.title} artwork`,
      x: frame.x,
      y: frame.y,
      width: frame.width,
      height: frame.height,
      parentId: frame.id,
      rotation: 0,
      visible: true,
      locked: true,
      effects: [],
      svg: frame.svg,
      tokens: frame.tokens,
    },
  ]);
  return {
    version: 9,
    id: crypto.randomUUID(),
    name: 'Design',
    projectId: null,
    pages: [{ id: 'legacy-page-1', name: 'Page 1' }],
    activePageId: 'legacy-page-1',
    nodes,
    selectedNodeIds: [],
    variables: [],
    textStyles: [],
    comments: frames.flatMap((frame) => frame.comments.map((comment) => ({
      id: comment.id,
      pageId: 'legacy-page-1',
      nodeId: frame.id,
      text: comment.text,
      point: comment.point,
      createdAt: comment.createdAt,
      resolved: false,
    }))),
    prototypeFlows: [],
    prototypeInteractions: [],
    motionTracks: [],
    components: [],
  };
}

export function designNodeDataUrl(node: DesignSvgNode): string {
  return designCanvasDataUrl(sanitizeDesignCanvasSvg(node.svg, node.width, node.height, node.tokens));
}

export function exportDesignDocumentSvg(document: DesignDocument, imageDataUrls: ReadonlyMap<string, string> = new Map()): string {
  const validated = validateDesignDocument(document);
  const selected = new Set(validated.selectedNodeIds);
  if (selected.size > 0) {
    let changed = true;
    while (changed) {
      changed = false;
      for (const node of validated.nodes) {
        if (node.parentId !== null && selected.has(node.parentId) && !selected.has(node.id)) {
          selected.add(node.id);
          changed = true;
        }
      }
    }
  }
  const nodes = validated.nodes.filter((node) => node.pageId === validated.activePageId && node.visible && (selected.size === 0 || selected.has(node.id)));
  const bounds = nodes.length === 0
    ? { left: 0, top: 0, right: 1, bottom: 1 }
    : nodes.reduce((result, node) => ({
      left: Math.min(result.left, node.x),
      top: Math.min(result.top, node.y),
      right: Math.max(result.right, node.x + node.width),
      bottom: Math.max(result.bottom, node.y + node.height),
    }), { left: Number.POSITIVE_INFINITY, top: Number.POSITIVE_INFINITY, right: Number.NEGATIVE_INFINITY, bottom: Number.NEGATIVE_INFINITY });
  const width = Math.max(1, bounds.right - bounds.left);
  const height = Math.max(1, bounds.bottom - bounds.top);
  const variablesById = new Map(validated.variables.map((variable) => [variable.id, variable.value]));
  const content = nodes.map((node) => exportNodeSvg(node, variablesById, imageDataUrls)).join('');
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="${bounds.left} ${bounds.top} ${width} ${height}"><title>${escapeXml(validated.name || 'Design')}</title>${content}</svg>`;
}

export function designDocumentImageAssetsForExport(document: DesignDocument): Array<Pick<DesignImageNode, 'assetId' | 'mimeType' | 'sizeBytes'>> {
  const validated = validateDesignDocument(document);
  const selected = new Set(validated.selectedNodeIds);
  if (selected.size > 0) {
    let changed = true;
    while (changed) {
      changed = false;
      for (const node of validated.nodes) {
        if (node.parentId !== null && selected.has(node.parentId) && !selected.has(node.id)) {
          selected.add(node.id);
          changed = true;
        }
      }
    }
  }
  const assets = validated.nodes.filter((node) => node.pageId === validated.activePageId && node.visible && (selected.size === 0 || selected.has(node.id))).flatMap((node) => {
    const result: Array<Pick<DesignImageNode, 'assetId' | 'mimeType' | 'sizeBytes'>> = [];
    if (node.type === 'image') result.push({ assetId: node.assetId, mimeType: node.mimeType, sizeBytes: node.sizeBytes });
    if ('fillStyle' in node && node.fillStyle?.type === 'image') result.push(node.fillStyle);
    return result;
  });
  return [...new Map(assets.map((asset) => [asset.assetId, asset])).values()];
}

export function exportDesignDocumentHtml(document: DesignDocument, imageDataUrls: ReadonlyMap<string, string> = new Map()): string {
  const validated = validateDesignDocument(document);
  const selected = new Set(validated.selectedNodeIds);
  if (selected.size > 0) {
    let changed = true;
    while (changed) {
      changed = false;
      for (const node of validated.nodes) {
        if (node.parentId !== null && selected.has(node.parentId) && !selected.has(node.id)) {
          selected.add(node.id);
          changed = true;
        }
      }
    }
  }
  const nodes = validated.nodes.filter((node) => node.pageId === validated.activePageId && node.visible && (selected.size === 0 || selected.has(node.id)));
  const bounds = nodes.length === 0
    ? { left: 0, top: 0, right: 1, bottom: 1 }
    : nodes.reduce((result, node) => ({
      left: Math.min(result.left, node.x), top: Math.min(result.top, node.y),
      right: Math.max(result.right, node.x + node.width), bottom: Math.max(result.bottom, node.y + node.height),
    }), { left: Number.POSITIVE_INFINITY, top: Number.POSITIVE_INFINITY, right: Number.NEGATIVE_INFINITY, bottom: Number.NEGATIVE_INFINITY });
  const width = Math.max(1, bounds.right - bounds.left);
  const height = Math.max(1, bounds.bottom - bounds.top);
  const variables = new Map(validated.variables.map((variable) => [variable.id, variable.value]));
  const body = nodes.map((node) => exportNodeHtml(node, bounds.left, bounds.top, variables, imageDataUrls)).join('\n');
  return `<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1">\n<title>${escapeHtml(validated.name || 'Design')}</title>\n<style>\n*{box-sizing:border-box}html,body{margin:0;min-width:100%;min-height:100%;font-family:Arial,sans-serif}.design-export{position:relative;width:${width}px;height:${height}px;overflow:hidden;background:#fff}.design-export>*{position:absolute;overflow:hidden}\n</style>\n</head>\n<body>\n<main class="design-export" aria-label="${escapeHtml(validated.name || 'Design')}">\n${body}\n</main>\n</body>\n</html>`;
}

function exportNodeHtml(
  node: DesignNode,
  originX: number,
  originY: number,
  variables: ReadonlyMap<string, string>,
  imageDataUrls: ReadonlyMap<string, string>,
): string {
  const x = node.x - originX;
  const y = node.y - originY;
  const transform = node.rotation === 0 ? '' : `transform:rotate(${node.rotation}deg);`;
  const shadow = node.effects.map((effect) => `${effect.offsetX}px ${effect.offsetY}px ${effect.blur}px ${effect.color}`).join(',');
  const commonStyle = `left:${x}px;top:${y}px;width:${node.width}px;height:${node.height}px;${transform}${shadow ? `filter:drop-shadow(${escapeHtml(shadow)});` : ''}`;
  const name = escapeHtml(node.name);
  switch (node.type) {
    case 'frame':
      return `<section aria-label="${name}" style="${commonStyle}${exportBackgroundStyle(node, variables, imageDataUrls)}border-radius:${designCornerRadiiCss(node)};${node.strokeWidth ? `border:${node.strokeWidth}px solid ${node.stroke};` : ''}"></section>`;
    case 'rectangle':
      return `<div aria-label="${name}" role="img" style="${commonStyle}${exportBackgroundStyle(node, variables, imageDataUrls)}border-radius:${designCornerRadiiCss(node)};${node.strokeWidth ? `border:${node.strokeWidth}px solid ${node.stroke};` : ''}"></div>`;
    case 'ellipse':
      return `<div aria-label="${name}" role="img" style="${commonStyle}${exportBackgroundStyle(node, variables, imageDataUrls)}border-radius:50%;${node.strokeWidth ? `border:${node.strokeWidth}px solid ${node.stroke};` : ''}"></div>`;
    case 'path':
      return `<svg aria-label="${name}" role="img" viewBox="0 0 ${node.width} ${node.height}" style="${commonStyle};overflow:visible"><path d="${designPathData(node.points, node.closed)}" fill="${node.closed ? resolveVariable(variables, node.fillVariableId, node.fill) : 'none'}" stroke="${node.strokeWidth > 0 ? node.stroke : 'none'}" stroke-width="${node.strokeWidth}" stroke-linejoin="round" stroke-linecap="round"/></svg>`;
    case 'text':
      return `<p style="${commonStyle}margin:0;color:${resolveVariable(variables, node.colorVariableId, node.color)};font-family:${node.fontFamily ?? 'sans-serif'};font-size:${node.fontSize}px;font-weight:${node.fontWeight};line-height:${node.lineHeight ?? 1.2};letter-spacing:${node.letterSpacing ?? 0}px;text-align:${node.textAlign ?? 'left'};white-space:pre-wrap">${escapeHtml(node.content)}</p>`;
    case 'imported-svg':
      return `<img alt="${name}" src="${escapeHtml(designNodeDataUrl(node))}" style="${commonStyle}">`;
    case 'image': {
      const dataUrl = imageDataUrls.get(node.assetId);
      if (!dataUrl) throw new Error('A design image asset is unavailable for code export.');
      return `<img alt="${escapeHtml(node.altText)}" src="${escapeHtml(dataUrl)}" style="${commonStyle}object-fit:${node.fit}">`;
    }
  }
}

function exportBackgroundStyle(
  node: DesignFrameNode | DesignRectangleNode | DesignEllipseNode,
  variables: ReadonlyMap<string, string>,
  imageDataUrls: ReadonlyMap<string, string>,
): string {
  const color = resolveVariable(variables, node.fillVariableId, node.fill);
  if (node.fillStyle?.type === 'gradient') {
    const stops = node.fillStyle.stops ?? [{ position: 0, color }, { position: 100, color: node.fillStyle.endColor }];
    const gradientStops = stops.map((stop) => `${stop.color} ${stop.position}%`).join(',');
    return `background:linear-gradient(${node.fillStyle.angle}deg,${gradientStops});`;
  }
  if (node.fillStyle?.type === 'image') {
    const dataUrl = imageDataUrls.get(node.fillStyle.assetId);
    if (!dataUrl) throw new Error('A design fill image asset is unavailable for code export.');
    return `background-image:url("${escapeHtml(dataUrl)}");background-position:center;background-repeat:no-repeat;background-size:${node.fillStyle.fit};`;
  }
  return `background:${color};`;
}

function escapeHtml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll("'", '&#39;');
}

function exportNodeSvg(node: DesignNode, variablesById: Map<string, string>, imageDataUrls?: ReadonlyMap<string, string>): string {
  const centerX = node.x + node.width / 2;
  const centerY = node.y + node.height / 2;
  const rotation = node.rotation === 0 ? '' : ` transform="rotate(${node.rotation} ${centerX} ${centerY})"`;
  const filter = node.effects.map((effect) => `drop-shadow(${effect.offsetX}px ${effect.offsetY}px ${effect.blur}px ${effect.color})`).join(' ');
  const groupStart = `<g${rotation}${filter ? ` style="filter:${escapeXml(filter)}"` : ''}>`;
  const groupEnd = '</g>';
  const x = node.x;
  const y = node.y;
  switch (node.type) {
    case 'frame': {
      const fill = exportSvgFill(node, variablesById, imageDataUrls);
      return `${fill.definition}${groupStart}<path d="${roundedRectanglePath(x, y, node.width, node.height, node)}" fill="${fill.value}" ${exportStroke(node)}/>${groupEnd}`;
    }
    case 'rectangle': {
      const fill = exportSvgFill(node, variablesById, imageDataUrls);
      return `${fill.definition}${groupStart}<path d="${roundedRectanglePath(x, y, node.width, node.height, node)}" fill="${fill.value}" ${exportStroke(node)}/>${groupEnd}`;
    }
    case 'ellipse': {
      const fill = exportSvgFill(node, variablesById, imageDataUrls);
      return `${fill.definition}${groupStart}<ellipse cx="${centerX}" cy="${centerY}" rx="${node.width / 2}" ry="${node.height / 2}" fill="${fill.value}" ${exportStroke(node)}/>${groupEnd}`;
    }
    case 'text':
      return `${groupStart}<text x="${node.textAlign === 'center' ? x + node.width / 2 : node.textAlign === 'right' ? x + node.width : x}" y="${y + node.fontSize}" fill="${resolveVariable(variablesById, node.colorVariableId, node.color)}" font-size="${node.fontSize}" font-weight="${node.fontWeight}" font-family="${node.fontFamily ?? 'sans-serif'}" letter-spacing="${node.letterSpacing ?? 0}" text-anchor="${node.textAlign === 'center' ? 'middle' : node.textAlign === 'right' ? 'end' : 'start'}">${escapeXml(node.content)}</text>${groupEnd}`;
    case 'path':
      return `${groupStart}<path d="${designPathData(node.points, node.closed)}" transform="translate(${x} ${y})" fill="${node.closed ? resolveVariable(variablesById, node.fillVariableId, node.fill) : 'none'}" stroke="${node.strokeWidth > 0 ? node.stroke : 'none'}" stroke-width="${node.strokeWidth}" stroke-linejoin="round" stroke-linecap="round"/>${groupEnd}`;
    case 'imported-svg':
      return `${groupStart}<image x="${x}" y="${y}" width="${node.width}" height="${node.height}" href="${escapeXml(designNodeDataUrl(node))}" preserveAspectRatio="none"/>${groupEnd}`;
    case 'image': {
      const dataUrl = imageDataUrls?.get(node.assetId);
      if (!dataUrl) throw new Error('A design image asset is unavailable for export.');
      const preserve = node.fit === 'cover' ? 'xMidYMid slice' : 'xMidYMid meet';
      return `${groupStart}<image x="${x}" y="${y}" width="${node.width}" height="${node.height}" href="${escapeXml(dataUrl)}" preserveAspectRatio="${preserve}"/>${groupEnd}`;
    }
  }
}

function exportSvgFill(
  node: DesignFrameNode | DesignRectangleNode | DesignEllipseNode,
  variables: ReadonlyMap<string, string>,
  imageDataUrls: ReadonlyMap<string, string> | undefined,
): { definition: string; value: string } {
  const color = resolveVariable(variables, node.fillVariableId, node.fill);
  if (node.fillStyle?.type === 'gradient') {
    const id = svgFillResourceId(node.id);
    const stops = node.fillStyle.stops ?? [{ position: 0, color }, { position: 100, color: node.fillStyle.endColor }];
    return {
      definition: `<defs><linearGradient id="${id}" gradientTransform="rotate(${node.fillStyle.angle} .5 .5)">${stops.map((stop) => `<stop offset="${stop.position}%" stop-color="${escapeXml(stop.color)}"/>`).join('')}</linearGradient></defs>`,
      value: `url(#${id})`,
    };
  }
  if (node.fillStyle?.type === 'image') {
    const dataUrl = imageDataUrls?.get(node.fillStyle.assetId);
    if (!dataUrl) throw new Error('A design fill image asset is unavailable for SVG export.');
    const id = svgFillResourceId(node.id);
    const preserve = node.fillStyle.fit === 'cover' ? 'xMidYMid slice' : 'xMidYMid meet';
    return {
      definition: `<defs><pattern id="${id}" patternUnits="userSpaceOnUse" x="${node.x}" y="${node.y}" width="${node.width}" height="${node.height}"><image href="${escapeXml(dataUrl)}" x="${node.x}" y="${node.y}" width="${node.width}" height="${node.height}" preserveAspectRatio="${preserve}"/></pattern></defs>`,
      value: `url(#${id})`,
    };
  }
  return { definition: '', value: escapeXml(color) };
}

function designNodeCornerRadii(node: DesignFrameNode | DesignRectangleNode): DesignCornerRadii {
  if (node.cornerRadii) return node.cornerRadii;
  const radius = node.radius ?? 0;
  return { topLeft: radius, topRight: radius, bottomRight: radius, bottomLeft: radius };
}

function designCornerRadiiCss(node: DesignFrameNode | DesignRectangleNode): string {
  const radii = designNodeCornerRadii(node);
  return `${radii.topLeft}px ${radii.topRight}px ${radii.bottomRight}px ${radii.bottomLeft}px`;
}

function roundedRectanglePath(x: number, y: number, width: number, height: number, node: DesignFrameNode | DesignRectangleNode): string {
  const source = designNodeCornerRadii(node);
  const scale = Math.min(
    1,
    width / Math.max(source.topLeft + source.topRight, 1),
    width / Math.max(source.bottomLeft + source.bottomRight, 1),
    height / Math.max(source.topLeft + source.bottomLeft, 1),
    height / Math.max(source.topRight + source.bottomRight, 1),
  );
  const topLeft = source.topLeft * scale;
  const topRight = source.topRight * scale;
  const bottomRight = source.bottomRight * scale;
  const bottomLeft = source.bottomLeft * scale;
  return [
    `M ${x + topLeft} ${y}`,
    `L ${x + width - topRight} ${y}`,
    `Q ${x + width} ${y} ${x + width} ${y + topRight}`,
    `L ${x + width} ${y + height - bottomRight}`,
    `Q ${x + width} ${y + height} ${x + width - bottomRight} ${y + height}`,
    `L ${x + bottomLeft} ${y + height}`,
    `Q ${x} ${y + height} ${x} ${y + height - bottomLeft}`,
    `L ${x} ${y + topLeft}`,
    `Q ${x} ${y} ${x + topLeft} ${y}`,
    'Z',
  ].join(' ');
}

function svgFillResourceId(value: string): string {
  return `design-fill-${Array.from(value).map((character) => character.codePointAt(0)?.toString(16) ?? '0').join('-')}`;
}

function exportStroke(node: DesignFrameNode | DesignRectangleNode | DesignEllipseNode | DesignPathNode): string {
  return node.strokeWidth > 0 ? `stroke="${node.stroke}" stroke-width="${node.strokeWidth}"` : 'stroke="none"';
}

function resolveVariable(variablesById: ReadonlyMap<string, string>, variableId: string | undefined, fallback: string): string {
  return variableId ? variablesById.get(variableId) ?? fallback : fallback;
}

function escapeXml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll("'", '&apos;');
}

export function validateDesignDocument(value: unknown): DesignDocument {
  if (
    !isRecord(value) ||
    (value.version !== 2 && value.version !== 3 && value.version !== 4 && value.version !== 5 && value.version !== 6 && value.version !== 7 && value.version !== 8 && value.version !== 9) ||
    typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 ||
    typeof value.name !== 'string' || Array.from(value.name).length > 256 || value.name.includes('\0') ||
    !(value.projectId === null || (typeof value.projectId === 'string' && value.projectId.length <= 128)) ||
    !Array.isArray(value.nodes) || value.nodes.length > maximumDesignNodeCount ||
    !Array.isArray(value.selectedNodeIds)
  ) {
    throw new Error('The design document structure is invalid.');
  }
  const variables = value.variables === undefined ? [] : Array.isArray(value.variables) ? value.variables.map(parseColorVariable) : null;
  if (!variables || variables.length > 1000) throw new Error('The design color variables are invalid.');
  const variableIds = new Set(variables.map((variable) => variable.id));
  if (variableIds.size !== variables.length) throw new Error('Design color variable identifiers must be unique.');
  const textStyles = value.textStyles === undefined ? [] : Array.isArray(value.textStyles) ? value.textStyles.map(parseDesignTextStyle) : null;
  if (!textStyles || textStyles.length > 2_000 || new Set(textStyles.map((style) => style.id)).size !== textStyles.length ||
    new Set(textStyles.map((style) => style.name.toLocaleLowerCase())).size !== textStyles.length) {
    throw new Error('The reusable text styles are invalid.');
  }
  const textStyleIds = new Set(textStyles.map((style) => style.id));
  const pages = value.pages === undefined && value.version === 2
    ? [{ id: 'legacy-page-1', name: 'Page 1' }]
    : Array.isArray(value.pages) ? value.pages.map(parsePage) : null;
  if (!pages || pages.length === 0 || pages.length > 100) throw new Error('The design pages are invalid.');
  const pageIds = new Set(pages.map((page) => page.id));
  if (pageIds.size !== pages.length) throw new Error('Design page identifiers must be unique.');
  const activePageId = value.activePageId === undefined && value.version === 2 ? pages[0]?.id : value.activePageId;
  if (typeof activePageId !== 'string' || !pageIds.has(activePageId)) throw new Error('The active design page is invalid.');
  const comments = value.comments === undefined ? [] : Array.isArray(value.comments) ? value.comments.map(parseDesignComment) : null;
  if (!comments || comments.length > 10_000) throw new Error('The design comments are invalid.');
  if (new Set(comments.map((comment) => comment.id)).size !== comments.length) throw new Error('Design comment identifiers must be unique.');
  if (comments.some((comment) => !pageIds.has(comment.pageId))) throw new Error('A design comment references a missing page.');
  const prototypeFlows = value.prototypeFlows === undefined ? [] : Array.isArray(value.prototypeFlows) ? value.prototypeFlows.map(parsePrototypeFlow) : null;
  const prototypeInteractions = value.prototypeInteractions === undefined ? [] : Array.isArray(value.prototypeInteractions) ? value.prototypeInteractions.map(parsePrototypeInteraction) : null;
  if (!prototypeFlows || prototypeFlows.length > 100 || !prototypeInteractions || prototypeInteractions.length > 10_000) {
    throw new Error('The prototype connections are invalid.');
  }
  const motionTracks = value.motionTracks === undefined ? [] : Array.isArray(value.motionTracks) ? value.motionTracks.map(parseDesignMotionTrack) : null;
  if (!motionTracks || motionTracks.length > maximumDesignMotionTrackCount || new Set(motionTracks.map((track) => `${track.nodeId}:${track.property}`)).size !== motionTracks.length) {
    throw new Error('The design motion tracks are invalid.');
  }
  if (new Set(prototypeFlows.map((flow) => flow.id)).size !== prototypeFlows.length ||
    new Set(prototypeFlows.map((flow) => flow.name.toLocaleLowerCase())).size !== prototypeFlows.length) {
    throw new Error('Prototype flow identifiers and names must be unique.');
  }
  const components = value.components === undefined ? [] : Array.isArray(value.components) ? value.components.map(parseDesignComponent) : null;
  if (!components || components.length > 2_000 || new Set(components.map((component) => component.id)).size !== components.length ||
    new Set(components.map((component) => component.name.toLocaleLowerCase())).size !== components.length) {
    throw new Error('The design components are invalid.');
  }
  const nodes = value.nodes.map((node) => parseNode(node, pages[0]?.id ?? '', value.version === 2));
  if (nodes.some((node) => !pageIds.has(node.pageId))) throw new Error('A design node references a missing page.');
  if (nodes.some((node) => node.textStyleId !== undefined && (!textStyleIds.has(node.textStyleId) || node.type !== 'text'))) {
    throw new Error('A design node references a missing or incompatible text style.');
  }
  const nodesById = new Map(nodes.map((node) => [node.id, node]));
  if (prototypeFlows.some((flow) => !pageIds.has(flow.pageId) || nodesById.get(flow.startNodeId)?.type !== 'frame' || nodesById.get(flow.startNodeId)?.pageId !== flow.pageId)) {
    throw new Error('A prototype flow references a missing frame.');
  }
  if (new Set(prototypeInteractions.map((interaction) => interaction.sourceNodeId)).size !== prototypeInteractions.length ||
    prototypeInteractions.some((interaction) => {
      const source = nodesById.get(interaction.sourceNodeId);
      const target = nodesById.get(interaction.targetNodeId);
      return !source || target?.type !== 'frame' || source.id === target.id || source.pageId !== target.pageId;
    })) throw new Error('A prototype interaction references an invalid target.');
  if (motionTracks.some((track) => !nodesById.has(track.nodeId))) throw new Error('A motion track references a missing design node.');
  const componentsById = new Map(components.map((component) => [component.id, component]));
  const variantIds = new Set(components.flatMap((component) => component.variants.map((variant) => `${component.id}:${variant.id}`)));
  if (nodes.some((node) => node.componentRef && (
    !componentsById.has(node.componentRef.componentId) ||
    !variantIds.has(`${node.componentRef.componentId}:${node.componentRef.variantId}`) ||
    (node.componentRef.role === 'master' && node.componentRef.sourceNodeId !== node.id) ||
    (node.componentRef.role !== 'master' && nodesById.get(node.componentRef.sourceNodeId)?.componentRef?.role !== 'master') ||
    (node.componentRef.role !== 'master' && nodesById.get(node.componentRef.sourceNodeId)?.componentRef?.componentId !== node.componentRef.componentId)
  ))) throw new Error('A design node references an invalid component.');
  if (components.some((component) => component.variants.some((variant) => {
    const root = nodesById.get(variant.rootNodeId);
    return root?.type !== 'frame' || root.componentRef?.componentId !== component.id || root.componentRef.variantId !== variant.id;
  }))) throw new Error('A component variant root is invalid.');
  for (const node of nodes) {
    if ((('fillVariableId' in node && node.fillVariableId !== undefined && !variableIds.has(node.fillVariableId)) ||
      ('colorVariableId' in node && node.colorVariableId !== undefined && !variableIds.has(node.colorVariableId)))) {
      throw new Error('A design node references a missing color variable.');
    }
  }
  const ids = new Set(nodes.map((node) => node.id));
  if (ids.size !== nodes.length) throw new Error('Design node identifiers must be unique.');
  if (new Set(value.selectedNodeIds).size !== value.selectedNodeIds.length) {
    throw new Error('A design node cannot be selected more than once.');
  }
  const activeNodeIds = new Set(nodes.filter((node) => node.pageId === activePageId).map((node) => node.id));
  const selectedNodeIds = value.selectedNodeIds.filter(
    (id): id is string => typeof id === 'string' && activeNodeIds.has(id),
  );
  if (selectedNodeIds.length !== value.selectedNodeIds.length) {
    throw new Error('The selected design nodes are invalid.');
  }
  const checked = new Set<string>();
  for (const node of nodes) {
    const path = new Set<string>();
    let current: DesignNode | undefined = node;
    while (current && current.parentId !== null && !checked.has(current.id)) {
      if (path.has(current.id)) throw new Error('The design node hierarchy contains a cycle.');
      path.add(current.id);
      const parent = nodesById.get(current.parentId);
      if (!parent) throw new Error('A design parent node is missing.');
      if (parent.type !== 'frame') throw new Error('A design node parent must be a frame.');
      if (parent.pageId !== current.pageId) throw new Error('A design hierarchy cannot cross pages.');
      current = parent;
    }
    for (const id of path) checked.add(id);
  }
  return {
    version: 9,
    id: value.id,
    name: value.name,
    projectId: value.projectId,
    pages,
    activePageId,
    nodes,
    selectedNodeIds,
    variables,
    textStyles,
    comments,
    prototypeFlows,
    prototypeInteractions,
    motionTracks,
    components,
  };
}

function parseDesignTextStyle(value: unknown): DesignTextStyle {
  if (!isRecord(value) || Object.keys(value).some((key) => !['id', 'name', 'color', 'fontSize', 'fontWeight', 'fontFamily', 'lineHeight', 'letterSpacing', 'textAlign'].includes(key)) || Object.keys(value).length < 5 || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 || value.id.includes('\0') ||
    typeof value.name !== 'string' || value.name.trim().length === 0 || Array.from(value.name).length > 128 || value.name.includes('\0') ||
    typeof value.color !== 'string' || !isColor(value.color) ||
    typeof value.fontSize !== 'number' || !Number.isFinite(value.fontSize) || value.fontSize < 1 || value.fontSize > 4096 ||
    typeof value.fontWeight !== 'number' || !Number.isInteger(value.fontWeight) || value.fontWeight < 100 || value.fontWeight > 900 ||
    (value.fontFamily !== undefined && value.fontFamily !== 'sans-serif' && value.fontFamily !== 'serif' && value.fontFamily !== 'monospace') ||
    (value.lineHeight !== undefined && (typeof value.lineHeight !== 'number' || !Number.isFinite(value.lineHeight) || value.lineHeight < 0.5 || value.lineHeight > 4)) ||
    (value.letterSpacing !== undefined && (typeof value.letterSpacing !== 'number' || !Number.isFinite(value.letterSpacing) || value.letterSpacing < -100 || value.letterSpacing > 100)) ||
    (value.textAlign !== undefined && value.textAlign !== 'left' && value.textAlign !== 'center' && value.textAlign !== 'right')) {
    throw new Error('A reusable text style is invalid.');
  }
  return {
    id: value.id, name: value.name.trim(), color: value.color, fontSize: value.fontSize, fontWeight: value.fontWeight,
    ...(value.fontFamily === undefined ? {} : { fontFamily: value.fontFamily }),
    ...(value.lineHeight === undefined ? {} : { lineHeight: value.lineHeight }),
    ...(value.letterSpacing === undefined ? {} : { letterSpacing: value.letterSpacing }),
    ...(value.textAlign === undefined ? {} : { textAlign: value.textAlign }),
  };
}

function parseDesignComponent(value: unknown): DesignComponent {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 ||
    typeof value.name !== 'string' || value.name.trim().length === 0 || Array.from(value.name).length > 128 ||
    !Array.isArray(value.variants) || value.variants.length === 0 || value.variants.length > 100) {
    throw new Error('A design component is invalid.');
  }
  const variants = value.variants.map((variant): DesignComponentVariant => {
    if (!isRecord(variant) || typeof variant.id !== 'string' || variant.id.length === 0 || variant.id.length > 128 ||
      typeof variant.name !== 'string' || variant.name.trim().length === 0 || Array.from(variant.name).length > 128 ||
      typeof variant.rootNodeId !== 'string' || variant.rootNodeId.length === 0 || variant.rootNodeId.length > 128) {
      throw new Error('A component variant is invalid.');
    }
    return { id: variant.id, name: variant.name.trim(), rootNodeId: variant.rootNodeId };
  });
  if (new Set(variants.map((variant) => variant.id)).size !== variants.length ||
    new Set(variants.map((variant) => variant.name.toLocaleLowerCase())).size !== variants.length) {
    throw new Error('Component variant identifiers and names must be unique.');
  }
  return { id: value.id, name: value.name.trim(), variants };
}

function parsePrototypeFlow(value: unknown): DesignPrototypeFlow {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 ||
    typeof value.name !== 'string' || value.name.trim().length === 0 || Array.from(value.name).length > 128 ||
    typeof value.pageId !== 'string' || value.pageId.length === 0 || value.pageId.length > 128 ||
    typeof value.startNodeId !== 'string' || value.startNodeId.length === 0 || value.startNodeId.length > 128) {
    throw new Error('A prototype flow is invalid.');
  }
  return { id: value.id, name: value.name.trim(), pageId: value.pageId, startNodeId: value.startNodeId };
}

function parsePrototypeInteraction(value: unknown): DesignPrototypeInteraction {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 ||
    typeof value.sourceNodeId !== 'string' || value.sourceNodeId.length === 0 || value.sourceNodeId.length > 128 ||
    typeof value.targetNodeId !== 'string' || value.targetNodeId.length === 0 || value.targetNodeId.length > 128 ||
    (value.trigger !== 'click' && value.trigger !== 'hover') || (value.presentation !== undefined && value.presentation !== 'navigate' && value.presentation !== 'overlay') || (value.transition !== 'instant' && value.transition !== 'dissolve' && value.transition !== 'slide') ||
    typeof value.durationMs !== 'number' || !Number.isInteger(value.durationMs) || value.durationMs < 0 || value.durationMs > 2_000 ||
    (value.transition === 'instant' && value.durationMs !== 0) || (value.transition !== 'instant' && value.durationMs === 0)) {
    throw new Error('A prototype interaction is invalid.');
  }
  return { id: value.id, sourceNodeId: value.sourceNodeId, targetNodeId: value.targetNodeId, trigger: value.trigger, presentation: value.presentation === 'overlay' ? 'overlay' : 'navigate', transition: value.transition, durationMs: value.durationMs };
}

function parseDesignMotionTrack(value: unknown): DesignMotionTrack {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 ||
    typeof value.nodeId !== 'string' || value.nodeId.length === 0 || value.nodeId.length > 128 ||
    !isDesignMotionProperty(value.property) ||
    !Array.isArray(value.keyframes) || value.keyframes.length < 2 || value.keyframes.length > 100) {
    throw new Error('A design motion track is invalid.');
  }
  const property = value.property;
  const keyframes = value.keyframes.map((keyframe): DesignMotionKeyframe => {
    if (!isRecord(keyframe) || typeof keyframe.id !== 'string' || keyframe.id.length === 0 || keyframe.id.length > 128 ||
      !Number.isSafeInteger(keyframe.timeMs) || typeof keyframe.timeMs !== 'number' || keyframe.timeMs < 0 || keyframe.timeMs > 10_000 ||
      typeof keyframe.value !== 'number' || !Number.isFinite(keyframe.value) ||
      !isDesignMotionEasing(keyframe.easing)) {
      throw new Error('A design motion keyframe is invalid.');
    }
    const inRange = property === 'opacity'
      ? keyframe.value >= 0 && keyframe.value <= 1
      : property === 'scale'
        ? keyframe.value >= 0.01 && keyframe.value <= 100
        : property === 'rotation'
          ? Math.abs(keyframe.value) <= 3600
          : Math.abs(keyframe.value) <= Number.MAX_SAFE_INTEGER;
    if (!inRange) throw new Error('A design motion keyframe value is out of range.');
    return { id: keyframe.id, timeMs: keyframe.timeMs, value: keyframe.value, easing: keyframe.easing };
  });
  if (keyframes[0]?.timeMs !== 0 || new Set(keyframes.map((keyframe) => keyframe.id)).size !== keyframes.length ||
    keyframes.some((keyframe, index) => index > 0 && keyframe.timeMs <= (keyframes[index - 1]?.timeMs ?? -1))) {
    throw new Error('Design motion keyframes must have unique identifiers and increasing times beginning at zero.');
  }
  return { id: value.id, nodeId: value.nodeId, property, keyframes };
}

function isDesignMotionProperty(value: unknown): value is DesignMotionProperty {
  return value === 'x' || value === 'y' || value === 'rotation' || value === 'opacity' || value === 'scale';
}

function isDesignMotionEasing(value: unknown): value is DesignMotionEasing {
  return value === 'linear' || value === 'ease-in' || value === 'ease-out' || value === 'ease-in-out';
}

function parseDesignComment(value: unknown): DesignComment {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 ||
    typeof value.pageId !== 'string' || value.pageId.length === 0 || value.pageId.length > 128 ||
    !(value.nodeId === null || (typeof value.nodeId === 'string' && value.nodeId.length <= 128)) ||
    typeof value.text !== 'string' || value.text.trim().length === 0 || Array.from(value.text).length > 2_000 ||
    typeof value.createdAt !== 'number' || !Number.isFinite(value.createdAt) ||
    typeof value.resolved !== 'boolean') throw new Error('A saved design comment is invalid.');
  let point: DesignComment['point'] = null;
  if (value.point !== null) {
    if (!isRecord(value.point) || typeof value.point.x !== 'number' || !Number.isFinite(value.point.x) ||
      typeof value.point.y !== 'number' || !Number.isFinite(value.point.y) ||
      value.point.x < 0 || value.point.x > 1 || value.point.y < 0 || value.point.y > 1) {
      throw new Error('A saved design comment position is invalid.');
    }
    point = { x: value.point.x, y: value.point.y };
  }
  return { id: value.id, pageId: value.pageId, nodeId: value.nodeId, text: value.text, point, createdAt: value.createdAt, resolved: value.resolved };
}

function parseNode(value: unknown, defaultPageId: string, allowLegacyPageId: boolean): DesignNode {
  if (
    !isRecord(value) ||
    typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 ||
    typeof value.name !== 'string' || Array.from(value.name).length > 256 || value.name.includes('\0') ||
    typeof value.x !== 'number' ||
    !Number.isFinite(value.x) || Math.abs(value.x) > Number.MAX_SAFE_INTEGER ||
    typeof value.y !== 'number' ||
    !Number.isFinite(value.y) || Math.abs(value.y) > Number.MAX_SAFE_INTEGER ||
    typeof value.width !== 'number' ||
    !Number.isFinite(value.width) ||
    value.width <= 0 || value.width > 1_000_000 ||
    typeof value.height !== 'number' ||
    !Number.isFinite(value.height) ||
    value.height <= 0 || value.height > 1_000_000 ||
    !(value.parentId === null || typeof value.parentId === 'string') ||
    typeof value.rotation !== 'number' ||
    !Number.isFinite(value.rotation) || Math.abs(value.rotation) > 360_000 ||
    typeof value.visible !== 'boolean' ||
    typeof value.locked !== 'boolean' ||
    typeof value.type !== 'string'
  ) {
    throw new Error('A design node is invalid.');
  }
  const base: NodeBase = {
    id: value.id,
    pageId: value.pageId === undefined && allowLegacyPageId ? defaultPageId : readRequiredId(value.pageId, 'A design page identifier is invalid.'),
    name: value.name,
    x: value.x,
    y: value.y,
    width: value.width,
    height: value.height,
    parentId: value.parentId,
    rotation: value.rotation,
    visible: value.visible,
    locked: value.locked,
    effects: value.effects === undefined ? [] : parseDesignEffects(value.effects),
    ...(value.constraints === undefined ? {} : { constraints: parseDesignConstraints(value.constraints) }),
    ...(value.componentRef === undefined ? {} : { componentRef: parseDesignComponentReference(value.componentRef) }),
    ...(readOptionalId(value.textStyleId, 'A text style identifier is invalid.') ? { textStyleId: readOptionalId(value.textStyleId, 'A text style identifier is invalid.') } : {}),
    ...(value.responsiveOverrides === undefined ? {} : { responsiveOverrides: parseResponsiveOverrides(value.responsiveOverrides) }),
  };
  switch (value.type) {
    case 'ellipse':
      if (typeof value.fill !== 'string' || !isColor(value.fill)) throw new Error('A node fill is invalid.');
      return { ...base, type: value.type, fill: value.fill, ...(value.fillStyle === undefined ? {} : { fillStyle: parseDesignFillStyle(value.fillStyle) }), ...(readOptionalId(value.fillVariableId, 'A fill variable identifier is invalid.') ? { fillVariableId: readOptionalId(value.fillVariableId, 'A fill variable identifier is invalid.') } : {}), ...parseStroke(value) };
    case 'frame':
      if (typeof value.fill !== 'string' || !isColor(value.fill) || (value.scrollable !== undefined && typeof value.scrollable !== 'boolean')) throw new Error('A frame fill is invalid.');
      return {
        ...base,
        type: value.type,
        fill: value.fill,
        ...(value.fillStyle === undefined ? {} : { fillStyle: parseDesignFillStyle(value.fillStyle) }),
        ...(value.radius === undefined ? {} : { radius: parseRadius(value.radius) }),
        ...(value.cornerRadii === undefined ? {} : { cornerRadii: parseDesignCornerRadii(value.cornerRadii) }),
        scrollable: value.scrollable === true,
        ...(value.autoLayout === undefined ? {} : { autoLayout: parseDesignFrameAutoLayout(value.autoLayout) }),
        ...(readOptionalId(value.fillVariableId, 'A fill variable identifier is invalid.') ? { fillVariableId: readOptionalId(value.fillVariableId, 'A fill variable identifier is invalid.') } : {}),
        ...parseStroke(value),
      };
    case 'rectangle':
      if (typeof value.fill !== 'string' || !isColor(value.fill) || typeof value.radius !== 'number' || !Number.isFinite(value.radius) || value.radius < 0) {
        throw new Error('A rectangle node is invalid.');
      }
      return {
        ...base,
        type: value.type,
        fill: value.fill,
        ...(value.fillStyle === undefined ? {} : { fillStyle: parseDesignFillStyle(value.fillStyle) }),
        radius: value.radius,
        ...(value.cornerRadii === undefined ? {} : { cornerRadii: parseDesignCornerRadii(value.cornerRadii) }),
        ...(readOptionalId(value.fillVariableId, 'A fill variable identifier is invalid.') ? { fillVariableId: readOptionalId(value.fillVariableId, 'A fill variable identifier is invalid.') } : {}),
        ...parseStroke(value),
      };
    case 'path':
      if (typeof value.fill !== 'string' || !isColor(value.fill) || typeof value.closed !== 'boolean') throw new Error('A vector path is invalid.');
      return { ...base, type: 'path', points: parseDesignPathPoints(value.points, value.closed), closed: value.closed, fill: value.fill, ...(value.fillStyle === undefined ? {} : { fillStyle: parseDesignFillStyle(value.fillStyle) }), ...(readOptionalId(value.fillVariableId, 'A fill variable identifier is invalid.') ? { fillVariableId: readOptionalId(value.fillVariableId, 'A fill variable identifier is invalid.') } : {}), ...parseStroke(value) };
    case 'text':
      if (
        typeof value.content !== 'string' ||
        Array.from(value.content).length > 100_000 ||
        value.content.includes('\0') ||
        typeof value.color !== 'string' ||
        !isColor(value.color) ||
        typeof value.fontSize !== 'number' ||
        !Number.isFinite(value.fontSize) || value.fontSize <= 0 || value.fontSize > 4096 ||
        typeof value.fontWeight !== 'number' ||
        !Number.isInteger(value.fontWeight) || value.fontWeight < 100 || value.fontWeight > 900 ||
        (value.fontFamily !== undefined && value.fontFamily !== 'sans-serif' && value.fontFamily !== 'serif' && value.fontFamily !== 'monospace') ||
        (value.lineHeight !== undefined && (typeof value.lineHeight !== 'number' || !Number.isFinite(value.lineHeight) || value.lineHeight < 0.5 || value.lineHeight > 4)) ||
        (value.letterSpacing !== undefined && (typeof value.letterSpacing !== 'number' || !Number.isFinite(value.letterSpacing) || value.letterSpacing < -100 || value.letterSpacing > 100)) ||
        (value.textAlign !== undefined && value.textAlign !== 'left' && value.textAlign !== 'center' && value.textAlign !== 'right')
      ) {
        throw new Error('A text node is invalid.');
      }
      return {
        ...base,
        type: value.type,
        content: value.content,
        color: value.color,
        fontSize: value.fontSize,
        fontWeight: value.fontWeight,
        ...(value.fontFamily === undefined ? {} : { fontFamily: value.fontFamily }),
        ...(value.lineHeight === undefined ? {} : { lineHeight: value.lineHeight }),
        ...(value.letterSpacing === undefined ? {} : { letterSpacing: value.letterSpacing }),
        ...(value.textAlign === undefined ? {} : { textAlign: value.textAlign }),
        ...(readOptionalId(value.colorVariableId, 'A text color variable identifier is invalid.') ? { colorVariableId: readOptionalId(value.colorVariableId, 'A text color variable identifier is invalid.') } : {}),
      };
    case 'imported-svg':
      if (typeof value.svg !== 'string') throw new Error('An imported vector node is invalid.');
      {
        const tokens = parseDesignCanvasTokens(value.tokens);
        return {
          ...base,
          type: value.type,
          svg: sanitizeDesignCanvasSvg(value.svg, base.width, base.height, tokens),
          tokens,
        };
      }
    case 'image':
      if (
        typeof value.assetId !== 'string' || !/^[0-9a-f]{32}$/.test(value.assetId) ||
        !(value.mimeType === 'image/png' || value.mimeType === 'image/jpeg' || value.mimeType === 'image/webp') ||
        typeof value.sizeBytes !== 'number' || !Number.isSafeInteger(value.sizeBytes) || value.sizeBytes < 1 || value.sizeBytes > 32 * 1024 * 1024 ||
        (value.fit !== 'cover' && value.fit !== 'contain') ||
        typeof value.altText !== 'string' || Array.from(value.altText).length > 256 || Array.from(value.altText).some((character) => character.charCodeAt(0) < 32)
      ) throw new Error('A design image node is invalid.');
      return { ...base, type: 'image', assetId: value.assetId, mimeType: value.mimeType, sizeBytes: value.sizeBytes, fit: value.fit, altText: value.altText };
    default:
      throw new Error('A design node type is unsupported.');
  }
}

export function parseDesignNode(value: unknown, pageId: string): DesignNode {
  return parseNode(value, pageId, false);
}

function parseResponsiveOverrides(value: unknown): NonNullable<NodeBase['responsiveOverrides']> {
  if (value === undefined) return {};
  if (!isRecord(value) || Object.keys(value).some((key) => key !== 'mobile' && key !== 'tablet')) {
    throw new Error('A responsive layout map is invalid.');
  }
  const layouts: NonNullable<NodeBase['responsiveOverrides']> = {};
  for (const breakpoint of ['mobile', 'tablet'] as const) {
    const entry = value[breakpoint];
    if (entry === undefined) continue;
    if (!isRecord(entry) ||
      typeof entry.x !== 'number' || !Number.isFinite(entry.x) || Math.abs(entry.x) > Number.MAX_SAFE_INTEGER ||
      typeof entry.y !== 'number' || !Number.isFinite(entry.y) || Math.abs(entry.y) > Number.MAX_SAFE_INTEGER ||
      typeof entry.width !== 'number' || !Number.isFinite(entry.width) || entry.width < 1 || entry.width > 1_000_000 ||
      typeof entry.height !== 'number' || !Number.isFinite(entry.height) || entry.height < 1 || entry.height > 1_000_000) {
      throw new Error('A responsive layout override is invalid.');
    }
    layouts[breakpoint] = { x: entry.x, y: entry.y, width: entry.width, height: entry.height };
  }
  return layouts;
}

export function parseDesignPathPoints(value: unknown, closed: boolean): Array<{ x: number; y: number }> {
  if (!Array.isArray(value) || value.length < (closed ? 3 : 2) || value.length > 512) {
    throw new Error('A vector path must contain a bounded set of points.');
  }
  return value.map((point) => {
    if (!isRecord(point) || typeof point.x !== 'number' || !Number.isFinite(point.x) || Math.abs(point.x) > 1_000_000 ||
      typeof point.y !== 'number' || !Number.isFinite(point.y) || Math.abs(point.y) > 1_000_000 ||
      Object.keys(point).some((key) => key !== 'x' && key !== 'y')) {
      throw new Error('A vector path point is invalid.');
    }
    return { x: point.x, y: point.y };
  });
}

export function designPathData(points: readonly { x: number; y: number }[], closed: boolean): string {
  const [first, ...rest] = points;
  if (!first) return '';
  return [`M ${first.x} ${first.y}`, ...rest.map((point) => `L ${point.x} ${point.y}`), ...(closed ? ['Z'] : [])].join(' ');
}

function parseDesignComponentReference(value: unknown): DesignComponentReference {
  if (!isRecord(value) || typeof value.componentId !== 'string' || value.componentId.length === 0 || value.componentId.length > 128 ||
    typeof value.sourceNodeId !== 'string' || value.sourceNodeId.length === 0 || value.sourceNodeId.length > 128 ||
    typeof value.variantId !== 'string' || value.variantId.length === 0 || value.variantId.length > 128 ||
    (value.role !== 'master' && value.role !== 'variant' && value.role !== 'instance')) {
    throw new Error('A design component reference is invalid.');
  }
  return { componentId: value.componentId, sourceNodeId: value.sourceNodeId, variantId: value.variantId, role: value.role };
}

export function parseDesignEffects(value: unknown): DesignShadowEffect[] {
  if (!Array.isArray(value) || value.length > 8) throw new Error('The design effects are invalid.');
  return value.map((effect) => {
    if (!isRecord(effect) || effect.type !== 'shadow' || typeof effect.color !== 'string' || !isColor(effect.color) ||
        typeof effect.offsetX !== 'number' || !Number.isFinite(effect.offsetX) || Math.abs(effect.offsetX) > 4096 ||
        typeof effect.offsetY !== 'number' || !Number.isFinite(effect.offsetY) || Math.abs(effect.offsetY) > 4096 ||
        typeof effect.blur !== 'number' || !Number.isFinite(effect.blur) || effect.blur < 0 || effect.blur > 4096) {
      throw new Error('A design shadow effect is invalid.');
    }
    return { type: 'shadow', color: effect.color, offsetX: effect.offsetX, offsetY: effect.offsetY, blur: effect.blur };
  });
}

export function parseDesignFrameAutoLayout(value: unknown): DesignFrameAutoLayout {
  if (!isRecord(value) || (value.direction !== 'horizontal' && value.direction !== 'vertical') ||
      (value.alignment !== 'start' && value.alignment !== 'center' && value.alignment !== 'end' && value.alignment !== 'stretch') ||
      typeof value.gap !== 'number' || !Number.isFinite(value.gap) || value.gap < 0 || value.gap > 4096 ||
      typeof value.padding !== 'number' || !Number.isFinite(value.padding) || value.padding < 0 || value.padding > 4096) {
    throw new Error('The frame auto layout is invalid.');
  }
  return { direction: value.direction, alignment: value.alignment, gap: value.gap, padding: value.padding };
}

export function parseDesignConstraints(value: unknown): DesignConstraints {
  if (!isRecord(value) || !isDesignConstraintMode(value.horizontal) || !isDesignConstraintMode(value.vertical)) {
    throw new Error('The design constraints are invalid.');
  }
  return { horizontal: value.horizontal, vertical: value.vertical };
}

function isDesignConstraintMode(value: unknown): value is DesignConstraintMode {
  return value === 'start' || value === 'end' || value === 'start-end' || value === 'center' || value === 'scale';
}

function parsePage(value: unknown): DesignPage {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 || value.id.includes('\0') ||
      typeof value.name !== 'string' || value.name.trim().length === 0 || Array.from(value.name).length > 128 || Array.from(value.name).some((character) => character.charCodeAt(0) < 32)) {
    throw new Error('A design page is invalid.');
  }
  return { id: value.id, name: value.name.trim() };
}

function readRequiredId(value: unknown, message: string): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 128 || value.includes('\0')) throw new Error(message);
  return value;
}

function parseRadius(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 4096) throw new Error('A design corner radius is invalid.');
  return value;
}

function parseDesignCornerRadii(value: unknown): DesignCornerRadii {
  if (!isRecord(value)) throw new Error('Design corner radii are invalid.');
  return {
    topLeft: parseRadius(value.topLeft),
    topRight: parseRadius(value.topRight),
    bottomRight: parseRadius(value.bottomRight),
    bottomLeft: parseRadius(value.bottomLeft),
  };
}

function parseDesignFillStyle(value: unknown): DesignFillStyle {
  if (!isRecord(value)) throw new Error('A design fill style is invalid.');
  if (value.type === 'gradient') {
    if (typeof value.endColor !== 'string' || !isColor(value.endColor) || typeof value.angle !== 'number' || !Number.isFinite(value.angle) || Math.abs(value.angle) > 360) {
      throw new Error('A design gradient is invalid.');
    }
    if (value.stops !== undefined && (!Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 8)) throw new Error('A design gradient stop is invalid.');
    const stops = Array.isArray(value.stops) ? value.stops.map((stop) => {
      if (!isRecord(stop) || typeof stop.position !== 'number' || !Number.isFinite(stop.position) || stop.position < 0 || stop.position > 100 || typeof stop.color !== 'string' || !isColor(stop.color)) {
        throw new Error('A design gradient stop is invalid.');
      }
      return { position: stop.position, color: stop.color };
    }) : undefined;
    if (stops?.some((stop, index) => {
      const previous = stops[index - 1];
      return index > 0 && previous !== undefined && stop.position <= previous.position;
    })) throw new Error('Design gradient stops must be ordered by position.');
    return { type: 'gradient', endColor: value.endColor, angle: value.angle, ...(stops ? { stops } : {}) };
  }
  if (value.type === 'image' && typeof value.sizeBytes === 'number' && Number.isSafeInteger(value.sizeBytes) && value.sizeBytes > 0 && value.sizeBytes <= 32 * 1024 * 1024 && (value.mimeType === 'image/png' || value.mimeType === 'image/jpeg' || value.mimeType === 'image/webp') && (value.fit === 'cover' || value.fit === 'contain')) {
    return { type: 'image', assetId: readRequiredId(value.assetId, 'A design fill image identifier is invalid.'), mimeType: value.mimeType, sizeBytes: value.sizeBytes, fit: value.fit };
  }
  throw new Error('A design fill style is invalid.');
}

function parseStroke(value: Record<string, unknown>): Pick<DesignFrameNode, 'stroke' | 'strokeWidth'> {
  const stroke = value.stroke === undefined ? '#000000' : value.stroke;
  const strokeWidth = value.strokeWidth === undefined ? 0 : value.strokeWidth;
  if (typeof stroke !== 'string' || !isColor(stroke) || typeof strokeWidth !== 'number' || !Number.isFinite(strokeWidth) || strokeWidth < 0 || strokeWidth > 4096) {
    throw new Error('A design stroke is invalid.');
  }
  return { stroke, strokeWidth };
}

function parseColorVariable(value: unknown): DesignColorVariable {
  if (
    !isRecord(value) ||
    typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 || value.id.includes('\0') ||
    typeof value.name !== 'string' || value.name.trim().length === 0 || Array.from(value.name).length > 128 || value.name.includes('\0') ||
    typeof value.value !== 'string' || !isColor(value.value)
  ) {
    throw new Error('A design color variable is invalid.');
  }
  return { id: value.id, name: value.name, value: value.value };
}

function readOptionalId(value: unknown, message: string): string | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== 'string' || value.length === 0 || value.length > 128 || value.includes('\0')) {
    throw new Error(message);
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isColor(value: string): boolean {
  return /^#[\da-f]{6}$/i.test(value);
}
