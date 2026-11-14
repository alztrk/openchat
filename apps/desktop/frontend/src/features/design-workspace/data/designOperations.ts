import { parseDesignPathPoints, validateDesignDocument, type DesignComponent, type DesignConstraints, type DesignDocument, type DesignFrameAutoLayout, type DesignMotionEasing, type DesignMotionProperty, type DesignNode, type DesignPrototypeFlow, type DesignPrototypeInteraction, type DesignResponsiveBreakpoint, type DesignResponsiveGeometry, type DesignShadowEffect, type DesignTextNode, type DesignTextStyle } from './designDocument';

export function createDesignTextStyle(document: DesignDocument, name: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const normalizedName = name.trim();
  const selected = validated.selectedNodeIds.length === 1
    ? validated.nodes.find((node) => node.id === validated.selectedNodeIds[0])
    : undefined;
  if (selected?.type !== 'text' || selected.locked || !normalizedName || Array.from(normalizedName).length > 128 || normalizedName.includes('\0') ||
    validated.textStyles.length >= 2_000 || validated.textStyles.some((style) => style.name.toLocaleLowerCase() === normalizedName.toLocaleLowerCase())) {
    throw new Error('Select one unlocked text item and provide a unique style name.');
  }
  const style: DesignTextStyle = {
    id: crypto.randomUUID(), name: normalizedName, color: selected.color, fontSize: selected.fontSize, fontWeight: selected.fontWeight,
    fontFamily: selected.fontFamily, lineHeight: selected.lineHeight, letterSpacing: selected.letterSpacing, textAlign: selected.textAlign,
  };
  return validateDesignDocument({ ...validated, textStyles: [...validated.textStyles, style] });
}

export function applyDesignTextStyle(document: DesignDocument, styleId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const style = validated.textStyles.find((item) => item.id === styleId);
  const selected = new Set(validated.selectedNodeIds);
  if (!style || !validated.nodes.some((node) => node.type === 'text' && selected.has(node.id) && !node.locked)) {
    throw new Error('Select an unlocked text item and choose an available text style.');
  }
  const nodes = validated.nodes.map((node): DesignNode => {
    if (node.type !== 'text' || !selected.has(node.id) || node.locked) return node;
    const { colorVariableId: _colorVariableId, ...unlinked } = node;
    return { ...unlinked, textStyleId: style.id, color: style.color, fontSize: style.fontSize, fontWeight: style.fontWeight,
      fontFamily: style.fontFamily, lineHeight: style.lineHeight, letterSpacing: style.letterSpacing, textAlign: style.textAlign };
  });
  return validateDesignDocument({ ...validated, nodes });
}

export function importSharedDesignTextStyle(document: DesignDocument, style: DesignTextStyle): DesignDocument {
  const validated = validateDesignDocument(document);
  const existing = validated.textStyles.find((item) => item.id === style.id);
  let next = validated;
  if (existing) {
    next = updateDesignTextStyle(validated, style.id, {
      name: style.name,
      color: style.color,
      fontSize: style.fontSize,
      fontWeight: style.fontWeight,
      fontFamily: style.fontFamily,
      lineHeight: style.lineHeight,
      letterSpacing: style.letterSpacing,
      textAlign: style.textAlign,
    });
  } else {
    if (validated.textStyles.length >= 2_000 || validated.textStyles.some((item) => item.name.toLocaleLowerCase() === style.name.toLocaleLowerCase())) {
      throw new Error('The shared text style conflicts with an existing document style.');
    }
    next = validateDesignDocument({ ...validated, textStyles: [...validated.textStyles, style] });
  }
  return applyDesignTextStyle(next, style.id);
}

export function updateDesignTextStyle(document: DesignDocument, styleId: string, patch: Pick<DesignTextStyle, 'name' | 'color' | 'fontSize' | 'fontWeight' | 'fontFamily' | 'lineHeight' | 'letterSpacing' | 'textAlign'>): DesignDocument {
  const validated = validateDesignDocument(document);
  const name = patch.name.trim();
  if (!validated.textStyles.some((style) => style.id === styleId) || !name || Array.from(name).length > 128 || name.includes('\0') ||
    !/^#[\da-f]{6}$/i.test(patch.color) || !Number.isFinite(patch.fontSize) || patch.fontSize < 1 || patch.fontSize > 4096 ||
    !Number.isInteger(patch.fontWeight) || patch.fontWeight < 100 || patch.fontWeight > 900 ||
    validated.textStyles.some((style) => style.id !== styleId && style.name.toLocaleLowerCase() === name.toLocaleLowerCase())) {
    throw new Error('The reusable text style values are invalid.');
  }
  const style = { id: styleId, ...patch, name };
  const nodes = validated.nodes.map((node): DesignNode => node.type === 'text' && node.textStyleId === styleId
    ? { ...node, color: style.color, fontSize: style.fontSize, fontWeight: style.fontWeight,
      fontFamily: style.fontFamily, lineHeight: style.lineHeight, letterSpacing: style.letterSpacing, textAlign: style.textAlign }
    : node);
  return validateDesignDocument({ ...validated, textStyles: validated.textStyles.map((item) => item.id === styleId ? style : item), nodes });
}

export function removeDesignTextStyle(document: DesignDocument, styleId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  if (!validated.textStyles.some((style) => style.id === styleId)) throw new Error('The reusable text style does not exist.');
  const nodes = validated.nodes.map((node): DesignNode => {
    if (node.type !== 'text' || node.textStyleId !== styleId) return node;
    const { textStyleId: _textStyleId, ...detached } = node;
    return detached;
  });
  return validateDesignDocument({ ...validated, textStyles: validated.textStyles.filter((style) => style.id !== styleId), nodes });
}

export function saveDesignResponsiveOverrides(document: DesignDocument, breakpoint: DesignResponsiveBreakpoint): DesignDocument {
  const validated = validateDesignDocument(document);
  if (breakpoint !== 'mobile' && breakpoint !== 'tablet') throw new Error('The responsive breakpoint is invalid.');
  const selected = new Set(validated.selectedNodeIds);
  const editable = validated.nodes.filter((node) => selected.has(node.id) && !node.locked);
  if (editable.length === 0) throw new Error('Select an unlocked design item to save a responsive layout.');
  const nodes = validated.nodes.map((node) => selected.has(node.id) && !node.locked
    ? { ...node, responsiveOverrides: { ...node.responsiveOverrides, [breakpoint]: { x: node.x, y: node.y, width: node.width, height: node.height } } }
    : node);
  return validateDesignDocument({ ...validated, nodes });
}

export function updateDesignResponsiveGeometry(document: DesignDocument, breakpoint: DesignResponsiveBreakpoint, nodeId: string, geometry: DesignResponsiveGeometry): DesignDocument {
  const validated = validateDesignDocument(document);
  const node = validated.nodes.find((item) => item.id === nodeId);
  if (!node || node.locked) throw new Error('Select an unlocked design item to edit its responsive layout.');
  const nodes = validated.nodes.map((item) => item.id === nodeId
    ? { ...item, responsiveOverrides: { ...item.responsiveOverrides, [breakpoint]: geometry } }
    : item);
  return validateDesignDocument({ ...validated, nodes });
}

export function designNodesForResponsiveBreakpoint(nodes: DesignNode[], breakpoint: DesignResponsiveBreakpoint | null): DesignNode[] {
  if (breakpoint === null) return nodes;
  return nodes.map((node) => {
    const geometry = node.responsiveOverrides?.[breakpoint];
    return geometry ? { ...node, ...geometry } : node;
  });
}

export interface DesignSnapGuides {
  vertical: number | null;
  horizontal: number | null;
}

export function snapDesignMove(
  movingNodes: DesignNode[],
  targetNodes: DesignNode[],
  deltaX: number,
  deltaY: number,
  tolerance: number,
): { deltaX: number; deltaY: number; guides: DesignSnapGuides } {
  if (movingNodes.length === 0 || !Number.isFinite(tolerance) || tolerance < 0) {
    return { deltaX, deltaY, guides: { vertical: null, horizontal: null } };
  }
  const movingIds = new Set(movingNodes.map((node) => node.id));
  const targets = targetNodes.filter((node) => node.visible && !movingIds.has(node.id));
  if (targets.length === 0) return { deltaX, deltaY, guides: { vertical: null, horizontal: null } };

  const movingBounds = {
    left: Math.min(...movingNodes.map((node) => node.x)),
    top: Math.min(...movingNodes.map((node) => node.y)),
    right: Math.max(...movingNodes.map((node) => node.x + node.width)),
    bottom: Math.max(...movingNodes.map((node) => node.y + node.height)),
  };
  const movingX = [movingBounds.left, (movingBounds.left + movingBounds.right) / 2, movingBounds.right];
  const movingY = [movingBounds.top, (movingBounds.top + movingBounds.bottom) / 2, movingBounds.bottom];
  const targetX = targets.flatMap((node) => [node.x, node.x + node.width / 2, node.x + node.width]);
  const targetY = targets.flatMap((node) => [node.y, node.y + node.height / 2, node.y + node.height]);
  const closest = (moving: number[], targets: number[], delta: number) => {
    let result: { correction: number; guide: number } | null = null;
    for (const movingGuide of moving) {
      for (const targetGuide of targets) {
        const candidate = targetGuide - (movingGuide + delta);
        if (Math.abs(candidate) <= tolerance && (result === null || Math.abs(candidate) < Math.abs(result.correction))) result = { correction: candidate, guide: targetGuide };
      }
    }
    return result;
  };
  const snapX = closest(movingX, targetX, deltaX);
  const snapY = closest(movingY, targetY, deltaY);
  return {
    deltaX: deltaX + (snapX?.correction ?? 0),
    deltaY: deltaY + (snapY?.correction ?? 0),
    guides: {
      vertical: snapX?.guide ?? null,
      horizontal: snapY?.guide ?? null,
    },
  };
}

export function createDesignComponent(document: DesignDocument, name: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const root = validated.nodes.find((node) => node.id === validated.selectedNodeIds[0]);
  const normalizedName = name.trim();
  if (validated.selectedNodeIds.length !== 1 || root?.type !== 'frame' || !normalizedName || Array.from(normalizedName).length > 128 ||
    validated.components.some((component) => component.name.toLocaleLowerCase() === normalizedName.toLocaleLowerCase())) {
    throw new Error('Select one frame and provide a unique component name.');
  }
  const nodes = expandDesignMoveSelection(validated, [root.id]);
  if (nodes.some((node) => node.componentRef)) throw new Error('Nested components must be detached before creating this component.');
  const componentId = crypto.randomUUID();
  const variantId = crypto.randomUUID();
  const component: DesignComponent = { id: componentId, name: normalizedName, variants: [{ id: variantId, name: 'Default', rootNodeId: root.id }] };
  return validateDesignDocument({
    ...validated,
    components: [...validated.components, component],
    nodes: validated.nodes.map((node) => nodes.some((member) => member.id === node.id)
      ? { ...node, componentRef: { componentId, sourceNodeId: node.id, variantId, role: 'master' as const } }
      : node),
  });
}

export function createDesignComponentVariant(document: DesignDocument, componentId: string, name: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const component = validated.components.find((item) => item.id === componentId);
  const normalizedName = name.trim();
  if (!component || !normalizedName || Array.from(normalizedName).length > 128 || component.variants.some((variant) => variant.name.toLocaleLowerCase() === normalizedName.toLocaleLowerCase()) ||
    validated.nodes.length + validated.nodes.filter((node) => node.componentRef?.componentId === componentId && node.componentRef.role === 'master').length > 10_000) {
    throw new Error('The component variant cannot be created.');
  }
  const root = validated.nodes.find((node) => node.id === component.variants[0]?.rootNodeId);
  if (root?.type !== 'frame') throw new Error('The component master frame is unavailable.');
  const sourceNodes = expandDesignMoveSelection(validated, [root.id]);
  const variantId = crypto.randomUUID();
  const idBySource = new Map(sourceNodes.map((node) => [node.id, crypto.randomUUID()]));
  const offsetX = root.width + 80;
  const cloneNodes = sourceNodes.map((node): DesignNode => {
    const id = idBySource.get(node.id);
    if (!id) throw new Error('A component source node is unavailable.');
    const sourceNodeId = node.componentRef?.sourceNodeId ?? node.id;
    return {
      ...node,
      id,
      pageId: validated.activePageId,
      name: node === root ? normalizedName : node.name,
      x: node.x + offsetX,
      parentId: node.parentId ? idBySource.get(node.parentId) ?? null : null,
      componentRef: { componentId, sourceNodeId, variantId, role: 'variant' },
    };
  });
  const cloneRoot = cloneNodes.find((node) => node.type === 'frame' && node.parentId === null);
  if (cloneRoot?.type !== 'frame') throw new Error('The component variant root is invalid.');
  return validateDesignDocument({
    ...validated,
    nodes: [...validated.nodes, ...cloneNodes],
    components: validated.components.map((item) => item.id === componentId
      ? { ...item, variants: [...item.variants, { id: variantId, name: normalizedName, rootNodeId: cloneRoot.id }] }
      : item),
  });
}

export function createDesignComponentInstance(document: DesignDocument, componentId: string, variantId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const component = validated.components.find((item) => item.id === componentId);
  const variant = component?.variants.find((item) => item.id === variantId);
  const root = variant && validated.nodes.find((node) => node.id === variant.rootNodeId);
  if (!component || !variant || root?.type !== 'frame' || validated.nodes.length >= 10_000) {
    throw new Error('The component instance cannot be created.');
  }
  const sources = expandDesignMoveSelection(validated, [root.id]);
  if (validated.nodes.length + sources.length > 10_000) throw new Error('The design has reached its node limit.');
  const idBySource = new Map(sources.map((node) => [node.id, crypto.randomUUID()]));
  const offsetX = root.width + 48;
  const offsetY = 48;
  const instances = sources.map((node): DesignNode => {
    const id = idBySource.get(node.id);
    if (!id) throw new Error('A component source node is unavailable.');
    return {
      ...node,
      id,
      pageId: validated.activePageId,
      name: node === root ? `${component.name} instance` : node.name,
      x: node.x + offsetX,
      y: node.y + offsetY,
      parentId: node.parentId ? idBySource.get(node.parentId) ?? null : null,
      componentRef: { componentId, sourceNodeId: node.componentRef?.role === 'master' ? node.id : node.componentRef?.sourceNodeId ?? node.id, variantId, role: 'instance' },
    };
  });
  return validateDesignDocument({ ...validated, nodes: [...validated.nodes, ...instances], selectedNodeIds: [instances[0]?.id ?? ''] });
}

export function synchronizeDesignComponentDependents(previous: DesignDocument, next: DesignDocument): DesignDocument {
  const oldNodes = new Map(previous.nodes.map((node) => [node.id, node]));
  const changedMasters = next.nodes.filter((node) => {
    if (node.componentRef?.role !== 'master') return false;
    const previousNode = oldNodes.get(node.id);
    return previousNode && JSON.stringify(previousNode) !== JSON.stringify(node);
  });
  if (changedMasters.length === 0) return next;
  const masters = new Map(changedMasters.map((node) => [node.id, node]));
  return validateDesignDocument({
    ...next,
    nodes: next.nodes.map((node) => {
      const source = node.componentRef && node.componentRef.role !== 'master' ? masters.get(node.componentRef.sourceNodeId) : undefined;
      if (!source || source.type !== node.type || !node.componentRef) return node;
      return syncComponentNode(source, node, oldNodes.get(source.id));
    }),
  });
}

export function removeDesignNodes(document: DesignDocument, removedNodeIds: ReadonlySet<string>): DesignDocument {
  const validated = validateDesignDocument(document);
  const removed = new Set(removedNodeIds);
  const removedComponents = new Set<string>();
  const removedVariants = new Set<string>();
  for (const node of validated.nodes) {
    const reference = node.componentRef;
    if (!reference || !removed.has(node.id)) continue;
    if (reference.role === 'master') removedComponents.add(reference.componentId);
    if (reference.role === 'variant') removedVariants.add(`${reference.componentId}:${reference.variantId}`);
    if (reference.role === 'instance') {
      let root = node;
      while (root.parentId !== null) {
        const parent = validated.nodes.find((item) => item.id === root.parentId);
        if (parent?.componentRef?.role !== 'instance' || parent.componentRef.componentId !== reference.componentId || parent.componentRef.variantId !== reference.variantId) break;
        root = parent;
      }
      removed.add(root.id);
    }
  }
  let foundDescendant = true;
  while (foundDescendant) {
    foundDescendant = false;
    for (const node of validated.nodes) {
      if (node.parentId !== null && removed.has(node.parentId) && !removed.has(node.id)) {
        removed.add(node.id);
        foundDescendant = true;
      }
    }
  }
  for (const node of validated.nodes) {
    const reference = node.componentRef;
    if (!reference) continue;
    if (removedComponents.has(reference.componentId) || removedVariants.has(`${reference.componentId}:${reference.variantId}`)) {
      removed.add(node.id);
    }
  }
  const components = validated.components
    .filter((component) => !removedComponents.has(component.id))
    .map((component) => ({
      ...component,
      variants: component.variants.filter((variant) => !removedVariants.has(`${component.id}:${variant.id}`)),
    }))
    .filter((component) => component.variants.length > 0);
  return validateDesignDocument({
    ...validated,
    components,
    nodes: validated.nodes.filter((node) => !removed.has(node.id)),
    selectedNodeIds: validated.selectedNodeIds.filter((id) => !removed.has(id)),
    comments: validated.comments.map((comment) => comment.nodeId && removed.has(comment.nodeId) ? { ...comment, nodeId: null } : comment),
    prototypeFlows: validated.prototypeFlows.filter((flow) => !removed.has(flow.startNodeId)),
    prototypeInteractions: validated.prototypeInteractions.filter((item) => !removed.has(item.sourceNodeId) && !removed.has(item.targetNodeId)),
    motionTracks: validated.motionTracks.filter((track) => !removed.has(track.nodeId)),
  });
}

function syncComponentNode(source: DesignNode, dependent: DesignNode, original: DesignNode | undefined): DesignNode {
  const deltaX = original ? source.x - original.x : 0;
  const deltaY = original ? source.y - original.y : 0;
  const preserved = {
    ...source,
    id: dependent.id,
    pageId: dependent.pageId,
    name: dependent.name,
    x: dependent.x + deltaX,
    y: dependent.y + deltaY,
    width: source.width,
    height: source.height,
    parentId: dependent.parentId,
    componentRef: dependent.componentRef,
  };
  switch (source.type) {
    case 'frame':
      if (dependent.type !== 'frame') return dependent;
      return { ...preserved, type: 'frame', autoLayout: source.autoLayout, fill: source.fill, fillStyle: source.fillStyle, fillVariableId: source.fillVariableId, radius: source.radius, cornerRadii: source.cornerRadii, stroke: source.stroke, strokeWidth: source.strokeWidth };
    case 'rectangle':
      if (dependent.type !== 'rectangle') return dependent;
      return { ...preserved, type: 'rectangle', fill: source.fill, fillStyle: source.fillStyle, radius: source.radius, cornerRadii: source.cornerRadii, fillVariableId: source.fillVariableId, stroke: source.stroke, strokeWidth: source.strokeWidth };
    case 'ellipse':
      if (dependent.type !== 'ellipse') return dependent;
      return { ...preserved, type: 'ellipse', fill: source.fill, fillVariableId: source.fillVariableId, stroke: source.stroke, strokeWidth: source.strokeWidth };
    case 'path':
      if (dependent.type !== 'path') return dependent;
      return { ...preserved, type: 'path', points: source.points, closed: source.closed, fill: source.fill, fillVariableId: source.fillVariableId, stroke: source.stroke, strokeWidth: source.strokeWidth };
    case 'text':
      if (dependent.type !== 'text') return dependent;
      return { ...preserved, type: 'text', content: source.content, color: source.color, fontSize: source.fontSize, fontWeight: source.fontWeight, colorVariableId: source.colorVariableId };
    case 'imported-svg':
      if (dependent.type !== 'imported-svg') return dependent;
      return { ...preserved, type: 'imported-svg', svg: source.svg, tokens: source.tokens };
    case 'image':
      if (dependent.type !== 'image') return dependent;
      return { ...preserved, type: 'image', assetId: source.assetId, mimeType: source.mimeType, sizeBytes: source.sizeBytes, fit: source.fit, altText: source.altText };
  }
}

export function setDesignPrototypeFlow(document: DesignDocument, name: string, startNodeId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const normalizedName = name.trim();
  const startNode = validated.nodes.find((node) => node.id === startNodeId);
  if (!normalizedName || Array.from(normalizedName).length > 128 || startNode?.type !== 'frame') {
    throw new Error('The prototype flow is invalid.');
  }
  const existing = validated.prototypeFlows.find((flow) => flow.startNodeId === startNodeId);
  if (validated.prototypeFlows.some((flow) => flow.id !== existing?.id && flow.name.toLocaleLowerCase() === normalizedName.toLocaleLowerCase())) {
    throw new Error('A prototype flow already uses that name.');
  }
  const flow: DesignPrototypeFlow = {
    id: existing?.id ?? crypto.randomUUID(),
    name: normalizedName,
    pageId: startNode.pageId,
    startNodeId,
  };
  return validateDesignDocument({
    ...validated,
    prototypeFlows: [...validated.prototypeFlows.filter((item) => item.startNodeId !== startNodeId), flow],
  });
}

export function setDesignPrototypeInteraction(
  document: DesignDocument,
  sourceNodeId: string,
  targetNodeId: string,
  transition: DesignPrototypeInteraction['transition'],
  trigger: DesignPrototypeInteraction['trigger'] = 'click',
  presentation: DesignPrototypeInteraction['presentation'] = 'navigate',
): DesignDocument {
  const validated = validateDesignDocument(document);
  const source = validated.nodes.find((node) => node.id === sourceNodeId);
  const target = validated.nodes.find((node) => node.id === targetNodeId);
  if (!source || target?.type !== 'frame' || source.id === target.id || source.pageId !== target.pageId ||
    !['instant', 'dissolve', 'slide'].includes(transition) || !['click', 'hover'].includes(trigger) || !['navigate', 'overlay'].includes(presentation)) throw new Error('The prototype connection is invalid.');
  const interaction: DesignPrototypeInteraction = {
    id: crypto.randomUUID(),
    sourceNodeId,
    targetNodeId,
    trigger,
    presentation,
    transition,
    durationMs: transition === 'instant' ? 0 : 240,
  };
  return validateDesignDocument({
    ...validated,
    prototypeInteractions: [...validated.prototypeInteractions.filter((item) => item.sourceNodeId !== sourceNodeId), interaction],
  });
}

export function removeDesignPrototypeInteraction(document: DesignDocument, sourceNodeId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  return validateDesignDocument({
    ...validated,
    prototypeInteractions: validated.prototypeInteractions.filter((item) => item.sourceNodeId !== sourceNodeId),
  });
}

export function setDesignMotionTrack(
  document: DesignDocument,
  nodeId: string,
  property: DesignMotionProperty,
  keyframes: readonly { timeMs: number; value: number; easing: DesignMotionEasing }[],
): DesignDocument {
  const validated = validateDesignDocument(document);
  if (!validated.nodes.some((node) => node.id === nodeId) || keyframes.length < 2 || keyframes.length > 100) {
    throw new Error('The motion track target or keyframes are invalid.');
  }
  const existing = validated.motionTracks.find((track) => track.nodeId === nodeId && track.property === property);
  const track = {
    id: existing?.id ?? crypto.randomUUID(),
    nodeId,
    property,
    keyframes: keyframes.map((keyframe) => ({ ...keyframe, id: crypto.randomUUID() })),
  };
  return validateDesignDocument({
    ...validated,
    motionTracks: [...validated.motionTracks.filter((item) => item.id !== existing?.id), track],
  });
}

export function removeDesignMotionTrack(document: DesignDocument, trackId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  if (!validated.motionTracks.some((track) => track.id === trackId)) throw new Error('The motion track no longer exists.');
  return validateDesignDocument({ ...validated, motionTracks: validated.motionTracks.filter((track) => track.id !== trackId) });
}

export type DesignArrangement =
  | 'align-left'
  | 'align-center-x'
  | 'align-right'
  | 'align-top'
  | 'align-center-y'
  | 'align-bottom'
  | 'distribute-horizontal'
  | 'distribute-vertical';

export function isDesignArrangement(value: string): value is DesignArrangement {
  return value === 'align-left' || value === 'align-center-x' || value === 'align-right' ||
    value === 'align-top' || value === 'align-center-y' || value === 'align-bottom' ||
    value === 'distribute-horizontal' || value === 'distribute-vertical';
}

/** Applies deterministic alignment or distribution to the selected nodes. */
export function arrangeDesignSelection(document: DesignDocument, action: DesignArrangement): DesignDocument {
  const validated = validateDesignDocument(document);
  const selected = validated.nodes.filter((node) => validated.selectedNodeIds.includes(node.id) && !node.locked);
  if (selected.length < 2 || (action.startsWith('distribute-') && selected.length < 3)) return validated;

  const positions = new Map(selected.map((node) => [node.id, { x: node.x, y: node.y }]));
  const minX = Math.min(...selected.map((node) => node.x));
  const maxX = Math.max(...selected.map((node) => node.x + node.width));
  const minY = Math.min(...selected.map((node) => node.y));
  const maxY = Math.max(...selected.map((node) => node.y + node.height));
  switch (action) {
    case 'align-left':
      for (const node of selected) positions.set(node.id, { x: minX, y: node.y });
      break;
    case 'align-center-x':
      for (const node of selected) positions.set(node.id, { x: (minX + maxX - node.width) / 2, y: node.y });
      break;
    case 'align-right':
      for (const node of selected) positions.set(node.id, { x: maxX - node.width, y: node.y });
      break;
    case 'align-top':
      for (const node of selected) positions.set(node.id, { x: node.x, y: minY });
      break;
    case 'align-center-y':
      for (const node of selected) positions.set(node.id, { x: node.x, y: (minY + maxY - node.height) / 2 });
      break;
    case 'align-bottom':
      for (const node of selected) positions.set(node.id, { x: node.x, y: maxY - node.height });
      break;
    case 'distribute-horizontal':
    case 'distribute-vertical': {
      const horizontal = action === 'distribute-horizontal';
      const sorted = selected.slice().sort((left, right) => horizontal ? left.x - right.x : left.y - right.y);
      const first = sorted[0];
      const last = sorted.at(-1);
      if (!first || !last) return validated;
      const totalSize = sorted.reduce((sum, node) => sum + (horizontal ? node.width : node.height), 0);
      const available = horizontal ? last.x + last.width - first.x : last.y + last.height - first.y;
      const gap = (available - totalSize) / (sorted.length - 1);
      let cursor = horizontal ? first.x : first.y;
      for (const node of sorted) {
        positions.set(node.id, horizontal ? { x: cursor, y: node.y } : { x: node.x, y: cursor });
        cursor += (horizontal ? node.width : node.height) + gap;
      }
      break;
    }
  }
  const nodes: DesignNode[] = validated.nodes.map((node) => {
    const position = positions.get(node.id);
    return position ? { ...node, ...position } : node;
  });
  return validateDesignDocument({ ...validated, nodes });
}

/** Moves the selected node block to the front or back of the canvas stacking order. */
export function reorderDesignSelection(document: DesignDocument, direction: 'front' | 'back'): DesignDocument {
  const validated = validateDesignDocument(document);
  if (validated.selectedNodeIds.length === 0) return validated;
  const selectedNodes = expandDesignMoveSelection(validated, validated.selectedNodeIds);
  const selectedIds = new Set(selectedNodes.map((node) => node.id));
  const selected = validated.nodes.filter((node) => selectedIds.has(node.id));
  const remaining = validated.nodes.filter((node) => !selectedIds.has(node.id));
  return validateDesignDocument({ ...validated, nodes: direction === 'front' ? [...remaining, ...selected] : [...selected, ...remaining] });
}

export function renameDesignNode(document: DesignDocument, nodeId: string, name: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const normalizedName = name.trim();
  if (!normalizedName || Array.from(normalizedName).length > 256 || normalizedName.includes('\0') || Array.from(normalizedName).some((character) => character.charCodeAt(0) < 32)) {
    throw new Error('The design layer name is invalid.');
  }
  if (!validated.nodes.some((node) => node.id === nodeId)) throw new Error('The design layer does not exist.');
  return validateDesignDocument({ ...validated, nodes: validated.nodes.map((node) => node.id === nodeId ? { ...node, name: normalizedName } : node) });
}

export function rotateDesignSelection(document: DesignDocument, degrees: number): DesignDocument {
  const validated = validateDesignDocument(document);
  if (!Number.isFinite(degrees) || degrees === 0) return validated;
  const selected = new Set(validated.selectedNodeIds);
  if (selected.size === 0) return validated;
  const nodes = validated.nodes.map((node) => {
    if (!selected.has(node.id) || node.locked) return node;
    const rotation = ((node.rotation + degrees) % 360 + 360) % 360;
    return { ...node, rotation };
  });
  return validateDesignDocument({ ...validated, nodes });
}

export function setDesignSelectionShadow(document: DesignDocument, effect: DesignShadowEffect | null): DesignDocument {
  const validated = validateDesignDocument(document);
  if (effect && (effect.type !== 'shadow' || !/^#[\da-f]{6}$/i.test(effect.color) || !Number.isFinite(effect.offsetX) || Math.abs(effect.offsetX) > 4096 || !Number.isFinite(effect.offsetY) || Math.abs(effect.offsetY) > 4096 || !Number.isFinite(effect.blur) || effect.blur < 0 || effect.blur > 4096)) {
    throw new Error('The design shadow effect is invalid.');
  }
  const selected = new Set(validated.selectedNodeIds);
  const nodes: DesignNode[] = validated.nodes.map((node) => {
    if (!selected.has(node.id) || node.locked) return node;
    return { ...node, effects: effect ? [effect] : [] };
  });
  return validateDesignDocument({ ...validated, nodes });
}

export function updateDesignTextSelection(
  document: DesignDocument,
  patch: Pick<DesignTextNode, 'content' | 'color' | 'fontSize' | 'fontWeight'>,
): DesignDocument {
  const validated = validateDesignDocument(document);
  if (Array.from(patch.content).length > 100_000 || patch.content.includes('\0') || !/^#[\da-f]{6}$/i.test(patch.color) ||
      !Number.isFinite(patch.fontSize) || patch.fontSize <= 0 || patch.fontSize > 4096 ||
      !Number.isInteger(patch.fontWeight) || patch.fontWeight < 100 || patch.fontWeight > 900) {
    throw new Error('The design text values are invalid.');
  }
  const selected = new Set(validated.selectedNodeIds);
  const nodes: DesignNode[] = validated.nodes.map((node) => {
    if (node.type !== 'text' || !selected.has(node.id) || node.locked) return node;
    const { textStyleId: _textStyleId, ...detached } = node;
    return { ...detached, ...patch };
  });
  return validateDesignDocument({ ...validated, nodes });
}

export function updateDesignImageSelection(document: DesignDocument, patch: { fit: 'cover' | 'contain'; altText: string }): DesignDocument {
  const validated = validateDesignDocument(document);
  const selectedId = validated.selectedNodeIds.length === 1 ? validated.selectedNodeIds[0] : undefined;
  const selected = validated.nodes.find((node) => node.id === selectedId);
  if (selected?.type !== 'image' || selected.locked || (patch.fit !== 'cover' && patch.fit !== 'contain') ||
    typeof patch.altText !== 'string' || Array.from(patch.altText).length > 256 || patch.altText.includes('\0')) {
    throw new Error('The selected design image properties are invalid.');
  }
  return validateDesignDocument({
    ...validated,
    nodes: validated.nodes.map((node) => node.id === selected.id ? { ...selected, ...patch } : node),
  });
}

export function updateDesignPathPoints(document: DesignDocument, nodeId: string, points: unknown): DesignDocument {
  const validated = validateDesignDocument(document);
  const path = validated.nodes.find((node) => node.id === nodeId);
  if (path?.type !== 'path' || path.locked) throw new Error('The selected design item is not an editable vector path.');
  const parsedPoints = parseDesignPathPoints(points, path.closed);
  return validateDesignDocument({
    ...validated,
    nodes: validated.nodes.map((node) => node.id === nodeId && node.type === 'path' ? { ...node, points: parsedPoints } : node),
  });
}

export function setDesignFrameAutoLayout(
  document: DesignDocument,
  frameId: string,
  autoLayout: DesignFrameAutoLayout | null,
): DesignDocument {
  const validated = validateDesignDocument(document);
  const frame = validated.nodes.find((node) => node.id === frameId);
  if (frame?.type !== 'frame') throw new Error('The design frame does not exist.');
  if (autoLayout && (!Number.isFinite(autoLayout.gap) || autoLayout.gap < 0 || autoLayout.gap > 4096 ||
      !Number.isFinite(autoLayout.padding) || autoLayout.padding < 0 || autoLayout.padding > 4096 ||
      (autoLayout.direction !== 'horizontal' && autoLayout.direction !== 'vertical') ||
      !['start', 'center', 'end', 'stretch'].includes(autoLayout.alignment))) {
    throw new Error('The frame auto layout is invalid.');
  }
  const nodes = validated.nodes.map((node) => node.id === frameId
    ? (autoLayout ? { ...frame, autoLayout } : withoutAutoLayout(frame))
    : node);
  return reflowDesignFrameChildren(validateDesignDocument({ ...validated, nodes }), frameId);
}

export function expandDesignMoveSelection(document: DesignDocument, selectedNodeIds: string[]): DesignNode[] {
  const validated = validateDesignDocument(document);
  const selected = new Set(selectedNodeIds);
  if (selected.size !== selectedNodeIds.length || selectedNodeIds.some((id) => !validated.nodes.some((node) => node.id === id))) {
    throw new Error('The selected design nodes are invalid.');
  }
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
  return validated.nodes.filter((node) => selected.has(node.id));
}

export function reflowDesignFrameChildren(document: DesignDocument, frameId: string): DesignDocument {
  const validated = validateDesignDocument(document);
  const frame = validated.nodes.find((node) => node.id === frameId);
  if (frame?.type !== 'frame' || !frame.autoLayout) return validated;
  const layout = frame.autoLayout;
  const children = validated.nodes.filter((node) => node.parentId === frameId && node.visible);
  const horizontal = layout.direction === 'horizontal';
  const crossStart = horizontal ? frame.y : frame.x;
  const crossSize = Math.max(1, (horizontal ? frame.height : frame.width) - layout.padding * 2);
  const mainOrigin = (horizontal ? frame.x : frame.y) + layout.padding;
  let cursor = mainOrigin;
  const childPositions = new Map<string, Pick<DesignNode, 'x' | 'y' | 'width' | 'height'>>();
  for (const child of children) {
    const childCrossSize = horizontal ? child.height : child.width;
    const adjustedCrossSize = layout.alignment === 'stretch' ? Math.max(24, crossSize) : childCrossSize;
    const crossOffset = layout.alignment === 'center' ? (crossSize - childCrossSize) / 2
      : layout.alignment === 'end' ? crossSize - childCrossSize : 0;
    const crossPosition = crossStart + layout.padding + (layout.alignment === 'stretch' ? 0 : crossOffset);
    const width = horizontal ? child.width : adjustedCrossSize;
    const height = horizontal ? adjustedCrossSize : child.height;
    childPositions.set(child.id, horizontal
      ? { x: cursor, y: crossPosition, width, height }
      : { x: crossPosition, y: cursor, width, height });
    cursor += (horizontal ? child.width : child.height) + layout.gap;
  }
  const nodes: DesignNode[] = validated.nodes.map((node) => {
    const position = childPositions.get(node.id);
    return position ? { ...node, ...position } : node;
  });
  return validateDesignDocument({ ...validated, nodes });
}

export function setDesignSelectionConstraints(document: DesignDocument, constraints: DesignConstraints): DesignDocument {
  const validated = validateDesignDocument(document);
  const modes = ['start', 'end', 'start-end', 'center', 'scale'];
  if (!modes.includes(constraints.horizontal) || !modes.includes(constraints.vertical)) {
    throw new Error('The design constraints are invalid.');
  }
  const selected = new Set(validated.selectedNodeIds);
  const nodes: DesignNode[] = validated.nodes.map((node) => selected.has(node.id) && node.parentId !== null
    ? { ...node, constraints }
    : node);
  return validateDesignDocument({ ...validated, nodes });
}

export function reflowDesignFrameConstraints(
  document: DesignDocument,
  originalFrame: Extract<DesignNode, { type: 'frame' }>,
  originalChildren: DesignNode[],
): DesignDocument {
  const validated = validateDesignDocument(document);
  const currentFrame = validated.nodes.find((node) => node.id === originalFrame.id);
  if (currentFrame?.type !== 'frame' || currentFrame.autoLayout) return validated;
  const currentNodes = new Map(validated.nodes.map((node) => [node.id, node]));
  const changes = new Map<string, Pick<DesignNode, 'x' | 'y' | 'width' | 'height'>>();
  for (const original of originalChildren) {
    if (original.parentId !== originalFrame.id) continue;
    const current = currentNodes.get(original.id);
    if (!current) continue;
    const constraints = original.constraints ?? { horizontal: 'start', vertical: 'start' };
    const horizontal = resizeAlongConstraint(
      constraints.horizontal,
      original.x,
      original.width,
      originalFrame.x,
      originalFrame.width,
      currentFrame.x,
      currentFrame.width,
    );
    const vertical = resizeAlongConstraint(
      constraints.vertical,
      original.y,
      original.height,
      originalFrame.y,
      originalFrame.height,
      currentFrame.y,
      currentFrame.height,
    );
    changes.set(original.id, { x: horizontal.position, y: vertical.position, width: horizontal.size, height: vertical.size });
  }
  return validateDesignDocument({
    ...validated,
    nodes: validated.nodes.map((node) => {
      const change = changes.get(node.id);
      return change ? { ...node, ...change } : node;
    }),
  });
}

function resizeAlongConstraint(
  mode: DesignConstraints['horizontal'],
  position: number,
  size: number,
  originalOrigin: number,
  originalParentSize: number,
  currentOrigin: number,
  currentParentSize: number,
): { position: number; size: number } {
  const originDelta = currentOrigin - originalOrigin;
  const sizeDelta = currentParentSize - originalParentSize;
  if (mode === 'end') return { position: position + originDelta + sizeDelta, size };
  if (mode === 'start-end') return { position: position + originDelta, size: Math.max(24, size + sizeDelta) };
  if (mode === 'center') return { position: position + originDelta + sizeDelta / 2, size };
  if (mode === 'scale') {
    const scale = currentParentSize / originalParentSize;
    return { position: currentOrigin + (position - originalOrigin) * scale, size: Math.max(24, size * scale) };
  }
  return { position: position + originDelta, size };
}

function withoutAutoLayout(frame: Extract<DesignNode, { type: 'frame' }>): DesignNode {
  const { autoLayout: _autoLayout, ...node } = frame;
  return node;
}
