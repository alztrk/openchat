import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const storageKey = 'design.vector_canvas.v1';
const maximumCanvasBytes = 2 * 1024 * 1024;
const maximumFrameCount = 16;
const maximumSvgBytes = 32 * 1024;
const maximumSvgNodeCount = 512;
const maximumRevisionCount = 3;
const maximumCommentCount = 40;
const svgNamespace = 'http://www.w3.org/2000/svg';
export const designCanvasFontFamilies = ['Source Sans 3', 'Arial', 'Georgia', 'monospace'] as const;
export const designCanvasTypeScales = [14, 16, 18] as const;
export const designCanvasSpacingUnits = [4, 8, 12] as const;
const allowedTypographyReferences = new Set([
  'var(--oc-type-small)',
  'var(--oc-type-body)',
  'var(--oc-type-heading)',
  'var(--oc-type-display)',
]);
const allowedSpacingReferences = new Set([
  'var(--oc-space-1)',
  'var(--oc-space-2)',
  'var(--oc-space-3)',
  'var(--oc-space-4)',
  'var(--oc-space-6)',
]);
const allowedElements = new Set([
  'svg',
  'g',
  'rect',
  'circle',
  'ellipse',
  'line',
  'polyline',
  'polygon',
  'path',
  'text',
  'tspan',
]);
const allowedColors = new Set([
  'var(--oc-background)',
  'var(--oc-surface)',
  'var(--oc-text)',
  'var(--oc-primary)',
  'var(--oc-accent)',
  'var(--oc-border)',
  'none',
  'transparent',
]);

export interface DesignCanvasTokens {
  background: string;
  surface: string;
  text: string;
  primary: string;
  accent: string;
  border: string;
  fontFamily: DesignCanvasFontFamily;
  typeScale: DesignCanvasTypeScale;
  spacingUnit: DesignCanvasSpacingUnit;
}

export type DesignCanvasFontFamily = (typeof designCanvasFontFamilies)[number];
export type DesignCanvasTypeScale = (typeof designCanvasTypeScales)[number];
export type DesignCanvasSpacingUnit = (typeof designCanvasSpacingUnits)[number];

export interface DesignCanvasRevision {
  id: string;
  title: string;
  width: number;
  height: number;
  svg: string;
  tokens: DesignCanvasTokens;
  createdAt: number;
}

export type DesignCanvasBreakpoint = 'mobile' | 'tablet';

export interface DesignCanvasResponsiveLayout extends DesignCanvasRevision {
  revisions: DesignCanvasRevision[];
}

export interface DesignCanvasComment {
  id: string;
  text: string;
  point: { x: number; y: number } | null;
  createdAt: number;
}

export interface DesignCanvasFrame extends DesignCanvasRevision {
  id: string;
  x: number;
  y: number;
  prompt: string;
  comments: DesignCanvasComment[];
  revisions: DesignCanvasRevision[];
  responsiveLayouts: Partial<Record<DesignCanvasBreakpoint, DesignCanvasResponsiveLayout>>;
}

export interface DesignCanvasState {
  version: 1;
  frames: DesignCanvasFrame[];
  selectedFrameId: string | null;
}

export interface GeneratedDesignCanvasFrame {
  title: string;
  width: number;
  height: number;
  svg: string;
  tokens: DesignCanvasTokens;
}

export interface DesignCanvasElement {
  id: string;
  kind: 'circle' | 'ellipse' | 'line' | 'path' | 'polygon' | 'polyline' | 'rect' | 'text';
}

export type DesignCanvasExportFormat = 'svg' | 'html' | 'react';

export type DesignCanvasRequestOutcome<T> =
  | { kind: 'completed'; value: T }
  | { kind: 'cancelled' }
  | { kind: 'failed'; error: unknown };

export async function settleDesignCanvasRequest<T>(
  result: Promise<T>,
  getCancellationAttempt: () => Promise<boolean> | null,
): Promise<DesignCanvasRequestOutcome<T>> {
  const response = await result.then(
    (value) => ({ kind: 'completed', value }) as const,
    (error: unknown) => ({ kind: 'failed', error }) as const,
  );
  const cancellation = getCancellationAttempt();
  if (cancellation && (await cancellation)) return { kind: 'cancelled' };
  return response;
}

export function emptyDesignCanvas(): DesignCanvasState {
  return { version: 1, frames: [], selectedFrameId: null };
}

export function readDesignCanvas(): DesignCanvasState {
  const encoded = unifiedStorage.getItem(storageKey);
  if (encoded === null) return emptyDesignCanvas();
  if (new TextEncoder().encode(encoded).byteLength > maximumCanvasBytes) {
    throw new Error('The saved design canvas exceeds its storage limit.');
  }
  const value: unknown = JSON.parse(encoded);
  if (!isRecord(value) || value.version !== 1 || !Array.isArray(value.frames)) {
    throw new Error('The saved design canvas is invalid.');
  }
  if (value.frames.length > maximumFrameCount) throw new Error('The saved design canvas has too many frames.');
  const frames = value.frames.map(parseFrame);
  const selectedFrameId = value.selectedFrameId;
  if (
    selectedFrameId !== null &&
    (typeof selectedFrameId !== 'string' || !frames.some((frame) => frame.id === selectedFrameId))
  ) {
    throw new Error('The selected design frame is invalid.');
  }
  return { version: 1, frames, selectedFrameId };
}

export async function saveDesignCanvas(state: DesignCanvasState): Promise<void> {
  const encoded = JSON.stringify(state);
  if (new TextEncoder().encode(encoded).byteLength > maximumCanvasBytes) {
    throw new Error('The design canvas reached its storage limit. Remove older frames or revisions to continue.');
  }
  await unifiedStorage.setPersistedItem(storageKey, encoded);
}

export function parseGeneratedDesignCanvas(content: string): GeneratedDesignCanvasFrame {
  if (new TextEncoder().encode(content).byteLength > maximumSvgBytes + 32 * 1024) {
    throw new Error('The model response exceeds its design limit.');
  }
  const trimmed = content.trim();
  const json = trimmed.startsWith('```') ? trimmed.replace(/^```(?:json)?\s*|\s*```$/g, '') : trimmed;
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    throw new Error('The model did not return valid design data.');
  }
  if (!isRecord(value)) throw new Error('The model returned invalid design data.');
  const title = parseText(value.title, 80);
  if (title.trim().length === 0) throw new Error('The model returned an empty frame title.');
  const width = parseDimension(value.width);
  const height = parseDimension(value.height);
  const tokens = parseTokens(value.tokens);
  if (typeof value.svg !== 'string') throw new Error('The model did not return an SVG frame.');
  const svg = sanitizeSvg(value.svg, width, height, tokens);
  return { title, width, height, svg, tokens };
}

export function createFrame(
  generated: GeneratedDesignCanvasFrame,
  prompt: string,
  x: number,
  y: number,
  now = Date.now(),
): DesignCanvasFrame {
  return {
    ...generated,
    id: crypto.randomUUID(),
    x: clampPosition(x),
    y: clampPosition(y),
    prompt: parseText(prompt, 4_000),
    comments: [],
    revisions: [],
    responsiveLayouts: {},
    createdAt: now,
  };
}

export function updateFrameTokens(frame: DesignCanvasFrame, tokens: DesignCanvasTokens): DesignCanvasFrame {
  const responsiveLayouts = { ...frame.responsiveLayouts };
  for (const breakpoint of ['mobile', 'tablet'] as const) {
    const layout = responsiveLayouts[breakpoint];
    if (!layout) continue;
    responsiveLayouts[breakpoint] = {
      ...layout,
      svg: sanitizeSvg(layout.svg, layout.width, layout.height, tokens),
      tokens,
    };
  }
  const svg = sanitizeSvg(frame.svg, frame.width, frame.height, tokens);
  return { ...frame, svg, tokens, responsiveLayouts, createdAt: Date.now() };
}

export function responsiveBreakpointForWidth(width: number): DesignCanvasBreakpoint | null {
  if (!Number.isSafeInteger(width) || width < 320 || width > 1280) {
    throw new Error('The design viewport width is outside the supported range.');
  }
  if (width <= 640) return 'mobile';
  if (width <= 1024) return 'tablet';
  return null;
}

export function designCanvasFrameForViewport(frame: DesignCanvasFrame, width: number): DesignCanvasFrame {
  const breakpoint = responsiveBreakpointForWidth(width);
  const layout = breakpoint ? frame.responsiveLayouts[breakpoint] : undefined;
  return layout ? { ...frame, ...layout } : frame;
}

export function setResponsiveLayout(
  frame: DesignCanvasFrame,
  breakpoint: DesignCanvasBreakpoint,
  generated: GeneratedDesignCanvasFrame,
  now = Date.now(),
): DesignCanvasFrame {
  const previous = frame.responsiveLayouts[breakpoint];
  const history = previous
    ? [...previous.revisions, responsiveLayoutRevision(previous)].slice(-maximumRevisionCount)
    : [];
  const layout: DesignCanvasResponsiveLayout = {
    ...generated,
    id: crypto.randomUUID(),
    svg: sanitizeSvg(generated.svg, generated.width, generated.height, generated.tokens),
    revisions: history,
    createdAt: now,
  };
  return {
    ...frame,
    responsiveLayouts: { ...frame.responsiveLayouts, [breakpoint]: layout },
  };
}

export function restoreResponsiveLayoutRevision(
  frame: DesignCanvasFrame,
  breakpoint: DesignCanvasBreakpoint,
  index: number,
  now = Date.now(),
): DesignCanvasFrame {
  const layout = frame.responsiveLayouts[breakpoint];
  const revision = layout?.revisions[index];
  if (!layout || !revision) throw new Error('The selected responsive layout revision no longer exists.');
  const revisions = layout.revisions.filter((_, revisionIndex) => revisionIndex !== index);
  revisions.push(responsiveLayoutRevision(layout));
  const restored: DesignCanvasResponsiveLayout = {
    ...revision,
    id: crypto.randomUUID(),
    revisions: revisions.slice(-maximumRevisionCount),
    createdAt: now,
  };
  return {
    ...frame,
    responsiveLayouts: { ...frame.responsiveLayouts, [breakpoint]: restored },
  };
}

export function addFrameComment(
  frame: DesignCanvasFrame,
  text: string,
  point: { x: number; y: number } | null,
  now = Date.now(),
): DesignCanvasFrame {
  const comment = parseText(text, 2_000);
  if (comment.trim().length === 0) throw new Error('A design comment cannot be empty.');
  if (frame.comments.length >= maximumCommentCount) throw new Error('The design has reached its comment limit.');
  return {
    ...frame,
    comments: [
      ...frame.comments,
      {
        id: crypto.randomUUID(),
        text: comment,
        point: point ? { x: clampUnitInterval(point.x), y: clampUnitInterval(point.y) } : null,
        createdAt: now,
      },
    ],
  };
}

export function reviseFrame(
  frame: DesignCanvasFrame,
  generated: GeneratedDesignCanvasFrame,
  now = Date.now(),
): DesignCanvasFrame {
  const previous: DesignCanvasRevision = {
    id: crypto.randomUUID(),
    title: frame.title,
    width: frame.width,
    height: frame.height,
    svg: frame.svg,
    tokens: frame.tokens,
    createdAt: frame.createdAt,
  };
  return {
    ...frame,
    ...generated,
    revisions: [...frame.revisions, previous].slice(-maximumRevisionCount),
    createdAt: now,
  };
}

export function frameAsAssistantMessage(frame: DesignCanvasFrame): string {
  return JSON.stringify({
    title: frame.title,
    width: frame.width,
    height: frame.height,
    tokens: frame.tokens,
    svg: frame.svg,
  });
}

export function exportDesignCanvasFrame(frame: DesignCanvasFrame, format: DesignCanvasExportFormat): string {
  if (format === 'svg') return frame.svg;
  if (format === 'html') {
    return [
      '<!doctype html>',
      '<html lang="en">',
      '  <head>',
      '    <meta charset="utf-8">',
      '    <meta name="viewport" content="width=device-width, initial-scale=1">',
      `    <title>${escapeHtml(frame.title)}</title>`,
      `    <style>:root{${tokenStyle(frame.tokens)}}body{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--oc-background);color:var(--oc-text);font-family:var(--oc-font-family);font-size:var(--oc-type-body)}.design-frame{width:min(100%, 960px);background:var(--oc-surface)}</style>`,
      '  </head>',
      '  <body>',
      `    <main class="design-frame mx-auto w-full max-w-5xl bg-white" aria-label="${escapeHtml(frame.title)}">`,
      indent(frame.svg, 6),
      '    </main>',
      '  </body>',
      '</html>',
    ].join('\n');
  }
  return exportReactComponent(frame);
}

export function designCanvasDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function exportReactComponent(frame: DesignCanvasFrame): string {
  const document = parseSvgDocument(frame.svg);
  const root = document.documentElement;
  const style = Object.entries(designCanvasCssVariables(frame.tokens))
    .map(([key, value]) => `'${key}': ${JSON.stringify(value)}`)
    .join(', ');
  const children = Array.from(root.childNodes)
    .map((node) => nodeToJsx(node, 2))
    .filter(Boolean)
    .join('\n');
  return [
    "import type { CSSProperties } from 'react';",
    '',
    `export default function ${componentName(frame.title)}() {`,
    '  return (',
    `    <svg xmlns="${svgNamespace}" width={${frame.width}} height={${frame.height}} viewBox={${JSON.stringify(`0 0 ${frame.width} ${frame.height}`)}} style={{ ${style} } as CSSProperties}>`,
    children,
    '    </svg>',
    '  );',
    '}',
    '',
  ].join('\n');
}

function nodeToJsx(node: Node, depth: number): string {
  const prefix = ' '.repeat(depth * 2);
  if (node.nodeType === Node.TEXT_NODE) {
    const content = node.textContent ?? '';
    return content.trim().length > 0 ? `${prefix}{${JSON.stringify(content)}}` : '';
  }
  if (!(node instanceof Element)) return '';
  const attributes = Array.from(node.attributes)
    .filter((attribute) => attribute.name !== 'xmlns')
    .map((attribute) => {
      const name = jsxAttributeName(attribute.name);
      if (attribute.name === 'style') return `style={${styleObject(attribute.value)}}`;
      return `${name}=${JSON.stringify(attribute.value)}`;
    })
    .join(' ');
  const children = Array.from(node.childNodes)
    .map((child) => nodeToJsx(child, depth + 1))
    .filter(Boolean);
  if (children.length === 0) return `${prefix}<${node.localName}${attributes ? ` ${attributes}` : ''} />`;
  return `${prefix}<${node.localName}${attributes ? ` ${attributes}` : ''}>\n${children.join('\n')}\n${prefix}</${node.localName}>`;
}

function styleObject(value: string): string {
  const entries = value
    .split(';')
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const separator = entry.indexOf(':');
      const name = entry.slice(0, separator);
      const color = entry.slice(separator + 1).trim();
      return `'${name}': ${JSON.stringify(color)}`;
    });
  return `{ ${entries.join(', ')} } as CSSProperties`;
}

function parseFrame(value: unknown): DesignCanvasFrame {
  if (!isRecord(value)) throw new Error('A saved design frame is invalid.');
  const title = parseText(value.title, 80);
  if (title.trim().length === 0) throw new Error('A saved design frame has an empty title.');
  const width = parseDimension(value.width);
  const height = parseDimension(value.height);
  const tokens = parseTokens(value.tokens, true);
  if (typeof value.svg !== 'string') throw new Error('A saved design frame has no SVG.');
  const svg = sanitizeSvg(value.svg, width, height, tokens);
  if (
    typeof value.id !== 'string' ||
    value.id.length > 64 ||
    typeof value.x !== 'number' ||
    typeof value.y !== 'number' ||
    typeof value.createdAt !== 'number' ||
    !Array.isArray(value.comments) ||
    !Array.isArray(value.revisions)
  ) {
    throw new Error('A saved design frame is invalid.');
  }
  const comments = value.comments.map(parseComment);
  const revisions = value.revisions.map((revision, index) => parseRevision(revision, index));
  const responsiveLayouts = parseResponsiveLayouts(value.responsiveLayouts);
  if (comments.length > maximumCommentCount || revisions.length > maximumRevisionCount) {
    throw new Error('A saved design frame exceeds its history limit.');
  }
  return {
    id: value.id,
    title,
    width,
    height,
    svg,
    tokens,
    x: clampPosition(value.x),
    y: clampPosition(value.y),
    prompt: parseText(value.prompt, 4_000),
    comments,
    revisions,
    responsiveLayouts,
    createdAt: parseTimestamp(value.createdAt),
  };
}

function parseResponsiveLayouts(value: unknown): Partial<Record<DesignCanvasBreakpoint, DesignCanvasResponsiveLayout>> {
  if (value === undefined) return {};
  if (!isRecord(value) || Object.keys(value).some((key) => key !== 'mobile' && key !== 'tablet')) {
    throw new Error('A saved responsive design layout is invalid.');
  }
  const layouts: Partial<Record<DesignCanvasBreakpoint, DesignCanvasResponsiveLayout>> = {};
  for (const breakpoint of ['mobile', 'tablet'] as const) {
    const item = value[breakpoint];
    if (item === undefined) continue;
    if (!isRecord(item)) throw new Error('A saved responsive design layout is invalid.');
    const revision = parseRevision(item, 0);
    const rawRevisions = item.revisions;
    if (rawRevisions !== undefined && !Array.isArray(rawRevisions)) {
      throw new Error('A saved responsive layout history is invalid.');
    }
    const revisions = Array.isArray(rawRevisions)
      ? rawRevisions.map((entry, index) => parseRevision(entry, index))
      : [];
    if (revisions.length > maximumRevisionCount) {
      throw new Error('A saved responsive layout exceeds its history limit.');
    }
    layouts[breakpoint] = { ...revision, revisions };
  }
  return layouts;
}

function responsiveLayoutRevision(layout: DesignCanvasResponsiveLayout): DesignCanvasRevision {
  return {
    id: layout.id,
    title: layout.title,
    width: layout.width,
    height: layout.height,
    svg: layout.svg,
    tokens: layout.tokens,
    createdAt: layout.createdAt,
  };
}

function parseComment(value: unknown): DesignCanvasComment {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.length > 64 || typeof value.text !== 'string') {
    throw new Error('A saved design comment is invalid.');
  }
  return {
    id: value.id,
    text: parseText(value.text, 2_000),
    point: parsePoint(value.point),
    createdAt: parseTimestamp(value.createdAt),
  };
}

function parsePoint(value: unknown): { x: number; y: number } | null {
  if (value === undefined || value === null) return null;
  if (
    !isRecord(value) ||
    typeof value.x !== 'number' ||
    !Number.isFinite(value.x) ||
    value.x < 0 ||
    value.x > 1 ||
    typeof value.y !== 'number' ||
    !Number.isFinite(value.y) ||
    value.y < 0 ||
    value.y > 1
  ) {
    throw new Error('A saved design comment position is invalid.');
  }
  return { x: value.x, y: value.y };
}

function clampUnitInterval(value: number): number {
  if (!Number.isFinite(value)) throw new Error('A design comment position is invalid.');
  return Math.max(0, Math.min(1, value));
}

function parseRevision(value: unknown, index: number): DesignCanvasRevision {
  if (!isRecord(value) || typeof value.svg !== 'string') throw new Error('A saved design revision is invalid.');
  if (value.id !== undefined && (typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 64)) {
    throw new Error('A saved design revision has an invalid identifier.');
  }
  const width = parseDimension(value.width);
  const height = parseDimension(value.height);
  const tokens = parseTokens(value.tokens, true);
  const createdAt = parseTimestamp(value.createdAt);
  return {
    id: typeof value.id === 'string' ? value.id : `legacy-${createdAt}-${index}`,
    title: parseText(value.title, 80),
    width,
    height,
    svg: sanitizeSvg(value.svg, width, height, tokens),
    tokens,
    createdAt,
  };
}

function parseTokens(value: unknown, allowLegacyTypography = false): DesignCanvasTokens {
  if (!isRecord(value)) throw new Error('The design token palette is invalid.');
  const tokens: DesignCanvasTokens = {
    background: parseColor(value.background),
    surface: parseColor(value.surface),
    text: parseColor(value.text),
    primary: parseColor(value.primary),
    accent: parseColor(value.accent),
    border: parseColor(value.border),
    fontFamily:
      value.fontFamily === undefined && allowLegacyTypography ? 'Source Sans 3' : parseFontFamily(value.fontFamily),
    typeScale: value.typeScale === undefined && allowLegacyTypography ? 16 : parseTypeScale(value.typeScale),
    spacingUnit: value.spacingUnit === undefined && allowLegacyTypography ? 8 : parseSpacingUnit(value.spacingUnit),
  };
  return tokens;
}

function parseFontFamily(value: unknown): DesignCanvasFontFamily {
  const fontFamily = designCanvasFontFamilies.find((candidate) => candidate === value);
  if (!fontFamily) throw new Error('The design token font family is unsupported.');
  return fontFamily;
}

function parseTypeScale(value: unknown): DesignCanvasTypeScale {
  const typeScale = designCanvasTypeScales.find((candidate) => candidate === value);
  if (typeScale === undefined) throw new Error('The design token type scale is unsupported.');
  return typeScale;
}

function parseSpacingUnit(value: unknown): DesignCanvasSpacingUnit {
  const spacingUnit = designCanvasSpacingUnits.find((candidate) => candidate === value);
  if (spacingUnit === undefined) throw new Error('The design token spacing unit is unsupported.');
  return spacingUnit;
}

function parseColor(value: unknown): string {
  if (typeof value !== 'string' || !/^#[\da-fA-F]{6}$/.test(value)) {
    throw new Error('The design token color is invalid.');
  }
  return value.toUpperCase();
}

function parseDimension(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 320 || value > 1_600) {
    throw new Error('The design frame dimensions are outside the supported range.');
  }
  return value;
}

function parseText(value: unknown, maximum: number): string {
  if (typeof value !== 'string' || Array.from(value).length > maximum || value.includes('\0')) {
    throw new Error('The design text is invalid.');
  }
  return value;
}

function parseTimestamp(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > 8_640_000_000_000_000) {
    throw new Error('The saved design timestamp is invalid.');
  }
  return value;
}

function clampPosition(value: number): number {
  if (!Number.isFinite(value)) throw new Error('The design frame position is invalid.');
  return Math.max(-100_000, Math.min(100_000, Math.round(value)));
}

export function listDesignCanvasElements(svg: string): DesignCanvasElement[] {
  const root = parseSvgDocument(svg).documentElement;
  let elementNumber = 0;
  return Array.from(root.querySelectorAll('*')).flatMap((element) => {
    const kind = parseDrawableKind(element.localName);
    if (!kind) return [];
    elementNumber += 1;
    return [{ id: `element-${elementNumber}`, kind }];
  });
}

export function exportDesignCanvasElement(frame: DesignCanvasFrame, elementId: string): string {
  const match = /^element-(\d+)$/.exec(elementId);
  const targetIndex = match ? Number(match[1]) - 1 : -1;
  const document = parseSvgDocument(frame.svg);
  const elements = Array.from(document.documentElement.querySelectorAll('*')).flatMap((element) =>
    parseDrawableKind(element.localName) ? [element] : [],
  );
  if (!Number.isSafeInteger(targetIndex) || targetIndex < 0 || targetIndex >= elements.length) {
    throw new Error('The selected design element no longer exists.');
  }
  for (const [index, element] of elements.entries()) {
    if (index !== targetIndex) element.remove();
  }
  return sanitizeSvg(
    new XMLSerializer().serializeToString(document.documentElement),
    frame.width,
    frame.height,
    frame.tokens,
  );
}

function sanitizeSvg(svg: string, width: number, height: number, tokens: DesignCanvasTokens): string {
  if (new TextEncoder().encode(svg).byteLength > maximumSvgBytes || /<!DOCTYPE|<!ENTITY/i.test(svg)) {
    throw new Error('The generated SVG exceeds its safe size or structure limit.');
  }
  const document = parseSvgDocument(svg);
  const root = document.documentElement;
  const elements = [root, ...Array.from(root.querySelectorAll('*'))];
  if (elements.length > maximumSvgNodeCount) throw new Error('The generated SVG contains too many vector elements.');
  const expectedStyle = tokenStyle(tokens);
  for (const element of elements) {
    if (!allowedElements.has(element.localName)) throw new Error('The generated SVG contains an unsupported element.');
    for (const attribute of Array.from(element.attributes)) {
      if (element === root && attribute.name === 'xmlns' && attribute.value === svgNamespace) continue;
      if (
        element === root &&
        attribute.name === 'style' &&
        (attribute.value === expectedStyle || attribute.value === legacyTokenStyle(tokens))
      ) {
        continue;
      }
      validateSvgAttribute(attribute.name, attribute.value);
    }
  }
  root.setAttribute('xmlns', svgNamespace);
  root.setAttribute('width', String(width));
  root.setAttribute('height', String(height));
  root.setAttribute('viewBox', `0 0 ${width} ${height}`);
  root.setAttribute('style', expectedStyle);
  const output = new XMLSerializer().serializeToString(root);
  if (new TextEncoder().encode(output).byteLength > maximumSvgBytes) {
    throw new Error('The sanitized SVG exceeds its storage limit.');
  }
  return output;
}

function validateSvgAttribute(name: string, value: string): void {
  if (name === 'width' || name === 'height' || name === 'viewBox') return;
  if (name === 'fill' || name === 'stroke') {
    if (!allowedColors.has(value)) throw new Error('The generated SVG uses a color outside its token palette.');
    return;
  }
  if (
    ['x', 'y', 'x1', 'y1', 'x2', 'y2', 'cx', 'cy', 'r', 'rx', 'ry'].includes(name) &&
    allowedSpacingReferences.has(value)
  ) {
    return;
  }
  if (['x', 'y', 'x1', 'y1', 'x2', 'y2', 'cx', 'cy', 'r', 'rx', 'ry', 'opacity', 'fill-opacity'].includes(name)) {
    if (!/^-?\d{1,5}(?:\.\d{1,3})?$/.test(value)) throw new Error('The generated SVG contains an invalid number.');
    return;
  }
  if (name === 'font-size' && allowedTypographyReferences.has(value)) return;
  if (name === 'letter-spacing' && allowedSpacingReferences.has(value)) return;
  if (name === 'stroke-width' || name === 'font-size' || name === 'letter-spacing') {
    if (!/^\d{1,4}(?:\.\d{1,3})?$/.test(value)) throw new Error('The generated SVG contains an invalid size.');
    return;
  }
  if (name === 'd' || name === 'points') {
    if (value.length > 32_768 || !/^[MmZzLlHhVvCcSsQqTtAa0-9.,\s+-]*$/.test(value)) {
      throw new Error('The generated SVG contains invalid vector geometry.');
    }
    return;
  }
  if (name === 'transform') {
    if (value.length > 256 || !/^(?:(?:translate|scale|rotate)\([-+\d.,\s]+\)\s*)+$/.test(value)) {
      throw new Error('The generated SVG contains an invalid transform.');
    }
    return;
  }
  if (name === 'font-family') {
    if (
      value !== 'var(--oc-font-family)' &&
      !['Source Sans 3', 'Arial', 'Georgia', 'monospace', 'sans-serif'].includes(value)
    ) {
      throw new Error('The generated SVG uses an unsupported font family.');
    }
    return;
  }
  if (name === 'font-weight') {
    if (!['normal', 'bold', '400', '500', '600', '700'].includes(value)) {
      throw new Error('The generated SVG uses an unsupported font weight.');
    }
    return;
  }
  if (name === 'text-anchor') {
    if (!['start', 'middle', 'end'].includes(value)) throw new Error('The generated SVG has an invalid text anchor.');
    return;
  }
  if (name === 'dominant-baseline') {
    if (!['auto', 'middle', 'hanging', 'central'].includes(value)) {
      throw new Error('The generated SVG has an invalid text baseline.');
    }
    return;
  }
  if (name === 'stroke-linecap') {
    if (!['butt', 'round', 'square'].includes(value)) throw new Error('The generated SVG has an invalid line cap.');
    return;
  }
  if (name === 'stroke-linejoin') {
    if (!['miter', 'round', 'bevel'].includes(value)) throw new Error('The generated SVG has an invalid line join.');
    return;
  }
  throw new Error('The generated SVG contains an unsupported attribute.');
}

function parseSvgDocument(svg: string): XMLDocument {
  const document = new DOMParser().parseFromString(svg, 'image/svg+xml');
  const root = document.documentElement;
  if (
    root.localName !== 'svg' ||
    (root.namespaceURI !== null && root.namespaceURI !== svgNamespace) ||
    document.querySelector('parsererror')
  ) {
    throw new Error('The generated SVG is not well-formed.');
  }
  return document;
}

function parseDrawableKind(name: string): DesignCanvasElement['kind'] | null {
  switch (name) {
    case 'circle':
    case 'ellipse':
    case 'line':
    case 'path':
    case 'polygon':
    case 'polyline':
    case 'rect':
    case 'text':
      return name;
    default:
      return null;
  }
}

function tokenStyle(tokens: DesignCanvasTokens): string {
  return Object.entries(designCanvasCssVariables(tokens))
    .map(([name, value]) => `${name}:${value}`)
    .join(';');
}

function legacyTokenStyle(tokens: DesignCanvasTokens): string {
  return [
    `--oc-background:${tokens.background.toUpperCase()}`,
    `--oc-surface:${tokens.surface.toUpperCase()}`,
    `--oc-text:${tokens.text.toUpperCase()}`,
    `--oc-primary:${tokens.primary.toUpperCase()}`,
    `--oc-accent:${tokens.accent.toUpperCase()}`,
    `--oc-border:${tokens.border.toUpperCase()}`,
  ].join(';');
}

function designCanvasCssVariables(tokens: DesignCanvasTokens): Record<string, string> {
  const spacing = tokens.spacingUnit;
  return {
    '--oc-background': tokens.background.toUpperCase(),
    '--oc-surface': tokens.surface.toUpperCase(),
    '--oc-text': tokens.text.toUpperCase(),
    '--oc-primary': tokens.primary.toUpperCase(),
    '--oc-accent': tokens.accent.toUpperCase(),
    '--oc-border': tokens.border.toUpperCase(),
    '--oc-font-family': fontFamilyCssValue(tokens.fontFamily),
    '--oc-type-small': `${tokens.typeScale - 2}px`,
    '--oc-type-body': `${tokens.typeScale}px`,
    '--oc-type-heading': `${Math.round(tokens.typeScale * 1.5)}px`,
    '--oc-type-display': `${tokens.typeScale * 2}px`,
    '--oc-space-1': `${spacing}px`,
    '--oc-space-2': `${spacing * 2}px`,
    '--oc-space-3': `${spacing * 3}px`,
    '--oc-space-4': `${spacing * 4}px`,
    '--oc-space-6': `${spacing * 6}px`,
  };
}

function fontFamilyCssValue(fontFamily: DesignCanvasFontFamily): string {
  switch (fontFamily) {
    case 'Source Sans 3':
      return '"Source Sans 3", sans-serif';
    case 'Arial':
      return 'Arial, sans-serif';
    case 'Georgia':
      return 'Georgia, serif';
    case 'monospace':
      return 'monospace';
  }
}

function componentName(title: string): string {
  const name = title
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .map((part) => `${part[0]?.toUpperCase() ?? ''}${part.slice(1)}`)
    .join('');
  return /^[A-Z]/.test(name) ? name : `Design${name || 'Frame'}`;
}

function jsxAttributeName(name: string): string {
  const names: Record<string, string> = {
    'fill-opacity': 'fillOpacity',
    'font-family': 'fontFamily',
    'font-size': 'fontSize',
    'font-weight': 'fontWeight',
    'letter-spacing': 'letterSpacing',
    'stroke-linecap': 'strokeLinecap',
    'stroke-linejoin': 'strokeLinejoin',
    'stroke-width': 'strokeWidth',
  };
  return names[name] ?? name;
}

function escapeHtml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}

function indent(value: string, spaces: number): string {
  return value
    .split('\n')
    .map((line) => `${' '.repeat(spaces)}${line}`)
    .join('\n');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
