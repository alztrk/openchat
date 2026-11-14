import type { ChatModel } from '@entities/model';
import { loadRouteParameters } from '@entities/model/modelRouteParameters';
import { localServiceClient, type ServiceEvent, type ServiceOperation } from '@shared/lib/platform/local-service';

import { maximumDesignNodeCount, parseDesignNode, validateDesignDocument, type DesignDocument, type DesignImageNode, type DesignNode } from './designDocument';
import { removeDesignNodes, synchronizeDesignComponentDependents } from './designOperations';

const maximumGeneratedNodes = 400;
const maximumPromptLength = 4_000;
const maximumProjectInstructionsBytes = 16 * 1024;

export interface DesignGenerationRequest {
  prompt: string;
  mode?: 'create' | 'revise' | 'variant';
  reasoningEffort?: string | null;
  targetNodeIds?: readonly string[];
  model: ChatModel;
  document: DesignDocument;
  references?: readonly DesignGenerationReference[];
  projectContext?: { id: string; name: string; instructions?: string } | null;
  allowFrameInspection?: boolean;
  onOperations?: (operations: DesignCanvasOperation[]) => void;
}

export interface DesignGenerationReference {
  id: string;
  name: string;
  mimeType: 'image/png' | 'image/jpeg' | 'image/webp';
  bytes: Uint8Array;
}

export interface DesignGenerationOperation {
  cancel: ServiceOperation['cancel'];
  result: Promise<DesignCanvasOperation[]>;
}

export type DesignCanvasOperation =
  | { type: 'create'; node: DesignNode }
  | { type: 'update'; node: DesignNode }
  | { type: 'delete'; nodeId: string }
  | { type: 'document'; patch: DesignDocumentPatch }
  | { type: 'create-image-from-reference'; referenceId: string; node: DesignImageNode }
  | { type: 'inspect-frame'; nodeId: string };

type DesignDocumentPatch = Partial<Pick<DesignDocument, 'name' | 'pages' | 'activePageId' | 'variables' | 'textStyles' | 'comments' | 'prototypeFlows' | 'prototypeInteractions' | 'motionTracks' | 'components'>>;

/** Starts a transcript-free provider request and validates its structured canvas output. */
export async function startDesignGeneration({ prompt, mode = 'create', reasoningEffort, targetNodeIds, model, document, references = [], projectContext, allowFrameInspection = false, onOperations }: DesignGenerationRequest): Promise<DesignGenerationOperation> {
  const normalizedPrompt = prompt.trim();
  if (normalizedPrompt.length === 0 || Array.from(normalizedPrompt).length > maximumPromptLength) {
    throw new Error('The design request is empty or too long.');
  }
  if (references.length > 3 || new Set(references.map((reference) => reference.id)).size !== references.length ||
    references.some((reference) => typeof reference.id !== 'string' || reference.id.length === 0 || reference.id.length > 128 ||
      typeof reference.name !== 'string' || reference.name.length === 0 || reference.name.length > 255 ||
      !['image/png', 'image/jpeg', 'image/webp'].includes(reference.mimeType) ||
      !(reference.bytes instanceof Uint8Array) || reference.bytes.length === 0 || reference.bytes.length > 10 * 1024 * 1024) ||
    references.reduce((total, reference) => total + reference.bytes.length, 0) > 14 * 1024 * 1024) {
    throw new Error('The design references exceed supported image limits.');
  }
  if (references.length > 0 && !model.supportsImages) throw new Error('The selected model does not support image references.');
  if (reasoningEffort && (!model.supportsReasoning || !model.reasoningLevels.includes(reasoningEffort))) {
    throw new Error('The selected reasoning level is not supported by this model.');
  }
  const visibleIds = new Set(document.nodes.filter((node) => node.pageId === document.activePageId && node.visible).slice(-80).map((node) => node.id));
  const scopedTargetIds = [...new Set(targetNodeIds ?? document.selectedNodeIds)].filter((id) => visibleIds.has(id));
  if (mode === 'revise' && scopedTargetIds.length === 0) throw new Error('Select at least one visible canvas object to revise.');

  let resolvedProjectContext = projectContext ?? null;
  if (resolvedProjectContext) {
    const response = await localServiceClient.call('project.instructions.get', { projectId: resolvedProjectContext.id });
    if (typeof response.instructions !== 'string' || new TextEncoder().encode(response.instructions).length > maximumProjectInstructionsBytes) {
      throw new Error('The selected project instructions are invalid or exceed their supported size.');
    }
    resolvedProjectContext = { ...resolvedProjectContext, instructions: response.instructions };
  }

  let streamedText = '';
  let streamedOperationCount = 0;
  let streamedResponseExceededLimit = false;
  const operation = await localServiceClient.startOperation('chat.temporary.send', {
    conversationId: `tmp_${crypto.randomUUID().replaceAll('-', '')}`,
    providerId: model.providerId,
    modelId: model.modelId,
    ...(reasoningEffort ? { reasoningEffort } : {}),
    ...(model.connectionId ? { connectionId: model.connectionId } : {}),
    ...(model.workspaceId ? { workspaceId: model.workspaceId } : {}),
    ...loadRouteParameters(model),
    designMode: true,
    allowDesignFrameInspection: allowFrameInspection,
    messages: [{
      role: 'user',
      content: buildDesignRequest(normalizedPrompt, mode, scopedTargetIds, document, resolvedProjectContext, references, allowFrameInspection),
      attachments: references.map((reference) => ({
        id: `design-${reference.id}`,
        name: reference.name,
        mimeType: reference.mimeType,
        sizeBytes: reference.bytes.length,
        kind: 'image',
        data: encodeReferenceBytes(reference.bytes),
      })),
    }],
  }, (event) => {
    const delta = readTextDelta(event);
    if (delta === null || !onOperations) return;
    streamedText += delta;
    if (streamedText.length > 512 * 1024) {
      streamedResponseExceededLimit = true;
      return;
    }
    const complete = extractCompleteOperations(streamedText);
    if (complete.length <= streamedOperationCount) return;
    try {
      const validated = readGeneratedOperations(JSON.stringify({ operations: complete }), document, mode, scopedTargetIds, references, allowFrameInspection);
      const next = validated.slice(streamedOperationCount);
      onOperations(next);
      streamedOperationCount = validated.length;
    } catch {
      // Stream events are provisional; validate and report failures from the final response instead.
    }
  });

  return {
    cancel: () => operation.cancel(),
    result: operation.result.then((response) => {
      if (streamedResponseExceededLimit) throw new Error('The streamed design response exceeded its limit.');
      const operations = readGeneratedOperations(response.content, document, mode, scopedTargetIds, references, allowFrameInspection);
      return operations.slice(streamedOperationCount);
    }),
  };
}

function readTextDelta(event: ServiceEvent): string | null {
  if (event.name !== 'chat.text.delta' || typeof event.data.delta !== 'string') return null;
  return event.data.delta;
}

/** Extracts only complete operation objects from the streamed JSON envelope. */
export function extractCompleteOperations(source: string): unknown[] {
  const start = findTopLevelOperationsArrayStart(source);
  if (start < 0) return [];
  const operations: unknown[] = [];
  let objectStart = -1;
  let objectDepth = 0;
  let inString = false;
  let escaped = false;
  for (let index = start + 1; index < source.length; index += 1) {
    const character = source[index];
    if (inString) {
      if (escaped) escaped = false;
      else if (character === '\\') escaped = true;
      else if (character === '"') inString = false;
      continue;
    }
    if (character === '"') {
      inString = true;
      continue;
    }
    if (character === '{') {
      if (objectDepth === 0) objectStart = index;
      objectDepth += 1;
    } else if (character === '}') {
      objectDepth -= 1;
      if (objectDepth < 0) return [];
      if (objectDepth === 0 && objectStart >= 0) {
        try {
          const parsed: unknown = JSON.parse(source.slice(objectStart, index + 1));
          operations.push(parsed);
        } catch {
          return [];
        }
        objectStart = -1;
        if (operations.length > maximumGeneratedNodes) throw new Error('The model returned too many design operations.');
      }
    } else if (character === ']' && objectDepth === 0) {
      return operations;
    }
  }
  return operations;
}

function findTopLevelOperationsArrayStart(source: string): number {
  let objectDepth = 0;
  let arrayDepth = 0;
  let inString = false;
  let escaped = false;
  let stringStart = -1;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    if (inString) {
      if (escaped) escaped = false;
      else if (character === '\\') escaped = true;
      else if (character === '"') {
        inString = false;
        if (objectDepth === 1 && arrayDepth === 0 && stringStart >= 0) {
          let next = index + 1;
          while (/\s/.test(source[next] ?? '')) next += 1;
          if (source.slice(stringStart, index + 1) === '"operations"' && source[next] === ':') {
            next += 1;
            while (/\s/.test(source[next] ?? '')) next += 1;
            if (source[next] === '[') return next;
          }
        }
      }
      continue;
    }
    if (character === '"') {
      inString = true;
      stringStart = index;
    } else if (character === '{') objectDepth += 1;
    else if (character === '}') objectDepth -= 1;
    else if (character === '[') arrayDepth += 1;
    else if (character === ']') arrayDepth -= 1;
  }
  return -1;
}

function encodeReferenceBytes(bytes: Uint8Array): string {
  let binary = '';
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, Math.min(offset + chunkSize, bytes.length)));
  }
  return window.btoa(binary);
}

function buildDesignRequest(prompt: string, mode: NonNullable<DesignGenerationRequest['mode']>, targetNodeIds: readonly string[], document: DesignDocument, projectContext: DesignGenerationRequest['projectContext'], references: readonly DesignGenerationReference[], allowFrameInspection: boolean): string {
  const currentNodes = document.nodes.filter((node) => node.pageId === document.activePageId && node.visible).slice(-80).map((node) => {
    if (node.type === 'text') return { ...node, content: node.content.slice(0, 4_000) };
    if (node.type === 'imported-svg') return { ...node, svg: '[Existing sanitized SVG; preserve this value when updating the node.]' };
    return node;
  });
  const variables = document.variables.map(({ id, name, value }) => ({ id, name, value }));

  return [
    mode === 'revise' ? `Mode: revise. Only update or delete selected node IDs (${JSON.stringify(targetNodeIds)}); create content only inside selected frames or as children of newly created frames.` : mode === 'variant' ? 'Mode: variant. Create a visually distinct alternative using new nodes only. Do not update or delete existing nodes. Place it beside the selected content and give it a clear name.' : 'Mode: create. Create a new design from the request; updates and deletions may target only the supplied existing node IDs.',
    'Use the callable Design functions supplied by the model provider to create and change editable canvas layers. Existing nodes may be updated or deleted only by their exact authorized IDs from the supplied context. Never return source code, SVG markup, or a verbal description in place of canvas changes.',
    'Follow these design principles: clear content hierarchy, consistent spacing, readable type, sufficient contrast, visible focus treatment, and controls with understandable labels. Keep mobile and desktop layouts intentional. Do not invent real product claims, prices, or user data; use only content supplied in the brief or concise neutral interface labels.',
    `If callable function tools are unavailable for the selected model, use this fallback format and create the requested layers as operations: {"operations":[...]}. Operations are {"type":"create","node":...}, {"type":"update","node":...}, {"type":"delete","nodeId":"existing-id"}, {"type":"document","patch":{...}}, {"type":"create-image-from-reference","referenceId":"provided-reference-id","node":{id,name,x,y,width,height,parentId,rotation,visible,locked,fit,altText}}${allowFrameInspection ? ', {"type":"inspect-frame","nodeId":"frame-id"}' : ''}. Nodes use the saved document node fields: every node requires id, type, name, x, y, width, height, parentId, rotation, visible, and locked. Frames also require fill, stroke, and strokeWidth; rectangles require fill, radius, stroke, and strokeWidth; ellipses require fill, stroke, and strokeWidth; paths require points, closed, fill, stroke, and strokeWidth; text requires content, color, fontSize, and fontWeight; image references require name, geometry, parentId, fit, and altText; imported SVG requires svg and tokens. Add effects, constraints, responsive overrides, per-corner radii, gradients, and other type-specific fields only in their documented shapes. Updates may contain only changed fields plus the existing node id; omitted fields are preserved. Document patches may update name, pages, activePageId, variables, textStyles, comments, prototypeFlows, prototypeInteractions, motionTracks, or components. Keep references valid; use only supplied image reference IDs or existing image assets. ${allowFrameInspection ? 'To inspect your own visual result, call design_inspect_frame with a frame node ID. The captured image will be sent in a follow-up review request.' : 'Frame screenshot inspection is unavailable in this review request.'} Create parent frames before children, use unique IDs, finite numbers, and positive dimensions. Return 1 to 400 operations.`,
    `Existing visible canvas nodes (up to 80; text content is capped at 4,000 characters per item): ${JSON.stringify(currentNodes)}`,
    `Current document structure: ${JSON.stringify({ name: document.name, pages: document.pages, activePageId: document.activePageId, variables, textStyles: document.textStyles, components: document.components, comments: document.comments.slice(-100), prototypeFlows: document.prototypeFlows, prototypeInteractions: document.prototypeInteractions, motionTracks: document.motionTracks })}`,
    `Available image references: ${JSON.stringify(references.map(({ id, name, mimeType, bytes }) => ({ id, name, mimeType, sizeBytes: bytes.length })))}`,
    `Available color variables: ${JSON.stringify(variables)}`,
    ...(projectContext ? [`Selected project context: ${JSON.stringify({ id: projectContext.id, name: projectContext.name, instructions: projectContext.instructions ?? '' })}`] : []),
    'Use attached image references as visual guidance. They are user-approved inputs for this request only and are not copied into the saved design unless a typed operation creates a design node from them.',
    `Design request: ${prompt}`,
  ].join('\n\n');
}

export function readGeneratedOperations(content: unknown, document: DesignDocument, mode: NonNullable<DesignGenerationRequest['mode']> = 'create', targetNodeIds: readonly string[] = document.selectedNodeIds, references: readonly DesignGenerationReference[] = [], allowFrameInspection = false): DesignCanvasOperation[] {
  if (typeof content !== 'string' || content.length === 0 || content.length > 512 * 1024) {
    throw new Error('The model returned no usable design data.');
  }
  let value: unknown;
  try {
    value = JSON.parse(content);
  } catch {
    throw new Error('The model response was not valid design JSON.');
  }
  if (!isRecord(value) || !Array.isArray(value.operations) || value.operations.length === 0 || value.operations.length > maximumGeneratedNodes) {
    throw new Error('The model response did not contain a supported design.');
  }
  const authorizedIds = new Set(document.nodes.filter((node) => node.pageId === document.activePageId && node.visible).slice(-80).map((node) => node.id));
  const scopedTargetIds = new Set(targetNodeIds.filter((id) => authorizedIds.has(id)));
  const createdIds = new Set<string>();
  let workingDocument = document;
  const operations = value.operations.map((operation): DesignCanvasOperation => {
    if (!isRecord(operation) || typeof operation.type !== 'string') {
      throw new Error('The model returned an invalid design operation.');
    }
    switch (operation.type) {
      case 'create':
      {
        const node = normalizeGeneratedNode(operation.node, workingDocument.activePageId, workingDocument);
        if (mode === 'revise' && (node.parentId === null || (authorizedIds.has(node.parentId) && !scopedTargetIds.has(node.parentId)))) throw new Error('The model attempted to create content outside the selected revision scope.');
        if (authorizedIds.has(node.id) || createdIds.has(node.id)) {
          throw new Error('The model returned a duplicate new node identifier.');
        }
        if (node.parentId !== null && !authorizedIds.has(node.parentId) && !createdIds.has(node.parentId)) {
          throw new Error('The model attempted to attach a new node outside the available design context.');
        }
        createdIds.add(node.id);
        const nextOperation: DesignCanvasOperation = { type: 'create', node };
        workingDocument = applyDesignOperations(workingDocument, [nextOperation]);
        return nextOperation;
      }
      case 'update': {
        const nodeId = isRecord(operation.node) && typeof operation.node.id === 'string' ? operation.node.id : '';
        if (!authorizedIds.has(nodeId) || mode === 'variant' || (mode === 'revise' && !scopedTargetIds.has(nodeId))) throw new Error('The model attempted to update an unavailable node.');
        const original = workingDocument.nodes.find((node) => node.id === nodeId);
        if (!original || !isRecord(operation.node)) throw new Error('The model attempted to update an unavailable node.');
        const normalized = normalizeGeneratedNode({ ...original, ...operation.node, id: nodeId, pageId: original.pageId }, original.pageId, workingDocument);
        const updated = original?.componentRef ? { ...normalized, componentRef: original.componentRef } : normalized;
        if (!original || (updated.parentId !== original.parentId && updated.parentId !== null && !authorizedIds.has(updated.parentId) && !createdIds.has(updated.parentId))) {
          throw new Error('The model attempted to move a node outside the available design context.');
        }
        const nextOperation: DesignCanvasOperation = { type: 'update', node: updated };
        workingDocument = applyDesignOperations(workingDocument, [nextOperation]);
        return nextOperation;
      }
      case 'delete':
        if (typeof operation.nodeId !== 'string' || !authorizedIds.has(operation.nodeId) || mode === 'variant' || (mode === 'revise' && !scopedTargetIds.has(operation.nodeId))) {
          throw new Error('The model attempted to delete an unavailable node.');
        }
        {
          const nextOperation: DesignCanvasOperation = { type: 'delete', nodeId: operation.nodeId };
          workingDocument = applyDesignOperations(workingDocument, [nextOperation]);
          return nextOperation;
        }
      case 'document': {
        if (mode === 'variant') throw new Error('A design variant cannot change document-level settings.');
        const patch = normalizeGeneratedDocumentPatch(operation.patch, workingDocument);
        const nextOperation: DesignCanvasOperation = { type: 'document', patch };
        workingDocument = applyDesignOperations(workingDocument, [nextOperation]);
        return nextOperation;
      }
      case 'create-image-from-reference': {
        if (!isRecord(operation.node) || typeof operation.referenceId !== 'string') throw new Error('The model returned an invalid image reference operation.');
        const reference = references.find((entry) => entry.id === operation.referenceId);
        if (!reference) throw new Error('The model referenced an unavailable attached image.');
        const node = parseDesignNode({ ...operation.node, type: 'image', pageId: workingDocument.activePageId, assetId: '0'.repeat(32), mimeType: reference.mimeType, sizeBytes: reference.bytes.length }, workingDocument.activePageId);
        if (node.type !== 'image' || authorizedIds.has(node.id) || createdIds.has(node.id) ||
          (node.parentId !== null && !authorizedIds.has(node.parentId) && !createdIds.has(node.parentId)) ||
          (mode === 'revise' && (node.parentId === null || (authorizedIds.has(node.parentId) && !scopedTargetIds.has(node.parentId))))) {
          throw new Error('The model returned an invalid image node.');
        }
        createdIds.add(node.id);
        const nextOperation: DesignCanvasOperation = { type: 'create-image-from-reference', referenceId: reference.id, node };
        workingDocument = applyDesignOperations(workingDocument, [{ type: 'create', node }]);
        return nextOperation;
      }
      case 'inspect-frame': {
        if (!allowFrameInspection || typeof operation.nodeId !== 'string' || (!authorizedIds.has(operation.nodeId) && !createdIds.has(operation.nodeId))) {
          throw new Error('The model requested an unavailable frame screenshot.');
        }
        const target = workingDocument.nodes.find((node) => node.id === operation.nodeId && node.type === 'frame' && node.pageId === workingDocument.activePageId && node.visible);
        if (!target || (mode === 'revise' && !scopedTargetIds.has(target.id) && !createdIds.has(target.id))) {
          throw new Error('The model requested an unavailable frame screenshot.');
        }
        return { type: 'inspect-frame', nodeId: target.id };
      }
      default:
        throw new Error('The model returned an unsupported design operation.');
    }
  });
  validateDesignDocument(workingDocument);
  return operations;
}

export function applyDesignOperations(document: DesignDocument, operations: DesignCanvasOperation[]): DesignDocument {
  let current = validateDesignDocument(document);
  for (const operation of operations) {
    switch (operation.type) {
      case 'create':
        if (current.nodes.some((node) => node.id === operation.node.id)) {
          throw new Error('A generated node identifier already exists.');
        }
        current = { ...current, nodes: [...current.nodes, operation.node], selectedNodeIds: [operation.node.id] };
        break;
      case 'update':
        if (!current.nodes.some((node) => node.id === operation.node.id)) {
          throw new Error('A generated update references an unavailable node.');
        }
        {
          const previous = current;
          const next = {
            ...current,
            nodes: current.nodes.map((node) => node.id === operation.node.id ? operation.node : node),
            selectedNodeIds: [operation.node.id],
          };
          current = synchronizeDesignComponentDependents(previous, next);
        }
        break;
      case 'delete': {
        if (!current.nodes.some((node) => node.id === operation.nodeId)) {
          throw new Error('A generated deletion references an unavailable node.');
        }
        const removedIds = new Set([operation.nodeId]);
        let foundDescendant = true;
        while (foundDescendant) {
          foundDescendant = false;
          for (const node of current.nodes) {
            if (node.parentId !== null && removedIds.has(node.parentId) && !removedIds.has(node.id)) {
              removedIds.add(node.id);
              foundDescendant = true;
            }
          }
        }
        current = removeDesignNodes(current, removedIds);
        break;
      }
      case 'document':
        current = validateDesignDocument({ ...current, ...operation.patch });
        break;
      case 'create-image-from-reference':
        throw new Error('An attached image reference must be imported before applying the design operation.');
      case 'inspect-frame':
        break;
    }
    current = validateDesignDocument(current);
    if (current.nodes.length > maximumDesignNodeCount) {
      throw new Error('The generated design exceeds the document node limit.');
    }
  }
  return validateDesignDocument(current);
}

function normalizeGeneratedNode(value: unknown, pageId: string, document: DesignDocument): DesignNode {
  if (!isRecord(value)) throw new Error('The model returned an invalid design item.');
  const node = parseDesignNode({ ...value, pageId }, pageId);
  const availableImageAssets = new Set(document.nodes.flatMap((entry) => {
    const assetIds: string[] = [];
    if (entry.type === 'image') assetIds.push(entry.assetId);
    if ('fillStyle' in entry && entry.fillStyle?.type === 'image') assetIds.push(entry.fillStyle.assetId);
    return assetIds;
  }));
  if ((node.type === 'image' && !availableImageAssets.has(node.assetId)) ||
    ('fillStyle' in node && node.fillStyle?.type === 'image' && !availableImageAssets.has(node.fillStyle.assetId))) {
    throw new Error('The model referenced an image asset that is not available in this design.');
  }
  return node;
}

function normalizeGeneratedDocumentPatch(value: unknown, document: DesignDocument): DesignDocumentPatch {
  const allowedKeys = new Set(['name', 'pages', 'activePageId', 'variables', 'textStyles', 'comments', 'prototypeFlows', 'prototypeInteractions', 'motionTracks', 'components']);
  if (!isRecord(value) || Object.keys(value).some((key) => !allowedKeys.has(key))) {
    throw new Error('The model returned an invalid document operation.');
  }
  const validated = validateDesignDocument({ ...document, ...value });
  const patch: DesignDocumentPatch = {};
  if ('name' in value) patch.name = validated.name;
  if ('pages' in value) patch.pages = validated.pages;
  if ('activePageId' in value) patch.activePageId = validated.activePageId;
  if ('variables' in value) patch.variables = validated.variables;
  if ('textStyles' in value) patch.textStyles = validated.textStyles;
  if ('comments' in value) patch.comments = validated.comments;
  if ('prototypeFlows' in value) patch.prototypeFlows = validated.prototypeFlows;
  if ('prototypeInteractions' in value) patch.prototypeInteractions = validated.prototypeInteractions;
  if ('motionTracks' in value) patch.motionTracks = validated.motionTracks;
  if ('components' in value) patch.components = validated.components;
  return patch;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
