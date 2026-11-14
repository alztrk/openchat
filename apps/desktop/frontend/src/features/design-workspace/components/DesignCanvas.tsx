import { Blend, Brain, Check, ChevronDown, Circle, CodeXml, Download, Eye, EyeOff, FilePlus2, FileText, Folder, FolderMinus, Frame, Image as ImageIcon, ImageDown, Layers3, LoaderCircle, LockKeyhole, MessageSquare, Minus, MoreHorizontal, MousePointer2, Move, PaintBucket, Paperclip, PenTool, Pencil, Play, Plus, Radius, RectangleHorizontal, Save, Scaling, Send, Square, Trash2, Type, UnlockKeyhole, X } from 'lucide-react';
import {
  type CSSProperties as ReactCSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MutableRefObject,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
  useEffect,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { createPortal } from 'react-dom';

import { Button, ColorPicker, Dialog, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger, Input, Select, Textarea } from '@shared/components';
import { toast } from '@shared/lib/toast';
import { localServiceErrorDetails } from '@shared/lib/platform/local-service';
import { useChatStore } from '@features/chat/state';
import { favoriteSourceConnection } from '@features/chat/state';
import { ModelPicker } from '@features/chat/components/ModelPicker';
import { PendingAttachmentGallery } from '@features/chat/components/PendingAttachmentGallery';
import { type PendingChatAttachment, ChatAttachmentInputError, readAttachmentFile } from '@features/chat/data/chatAttachments';
import type { ChatModelFavorite } from '@features/chat/data';
import { CanvasZoomControls } from '@features/design-workspace/components/CanvasZoomControls';
import { applyDesignOperations, startDesignGeneration, type DesignCanvasOperation, type DesignGenerationOperation, type DesignGenerationReference } from '@features/design-workspace/data/designGeneration';
import { applyDesignTextStyle, arrangeDesignSelection, createDesignComponent, createDesignComponentInstance, createDesignComponentVariant, createDesignTextStyle, designNodesForResponsiveBreakpoint, expandDesignMoveSelection, groupDesignSelection, reflowDesignFrameChildren, reflowDesignFrameConstraints, removeDesignNodes, removeDesignPrototypeInteraction, renameDesignNode, setDesignPrototypeFlow, setDesignPrototypeInteraction, snapDesignMove, synchronizeDesignComponentDependents, type DesignSnapGuides } from '@features/design-workspace/data/designOperations';
import { listSharedDesignTextStyles, type SharedDesignTextStyle } from '@features/design-workspace/data/designStyleLibrary';
import {
  addDesignPage,
  addDesignComment,
  createDesignNode,
  designDocumentImageAssetsForExport,
  designNodeDataUrl,
  emptyDesignDocument,
  exportDesignDocumentSvg,
  exportDesignDocumentHtml,
  maximumDesignNodeCount,
  removeDesignPage,
  removeDesignComment,
  readDesignDocument,
  setDesignCommentResolved,
  renameDesignPage,
  type DesignDocument,
  type DesignDocumentReadResult,
  type DesignPrototypeInteraction,
  type DesignResponsiveBreakpoint,
  type DesignCornerRadii,
  type DesignFillStyle,
  type DesignGradientStop,
  type DesignFrameNode,
  type DesignImageNode,
  type DesignNode,
} from '@features/design-workspace/data/designDocument';
import {
  getDesignDocument,
  listDesignDocuments,
  listDesignDocumentRevisions,
  saveDesignDocumentRevision,
  type DesignDocumentRevision,
  type DesignDocumentSummary,
} from '@features/design-workspace/data/designDocumentRepository';
import {
  DESIGN_PANEL_DEFAULT_WIDTH,
  DESIGN_PANEL_MAX_WIDTH,
  DESIGN_PANEL_MIN_WIDTH,
  DESIGN_INSPECTOR_DEFAULT_WIDTH,
  DESIGN_INSPECTOR_MAX_WIDTH,
  DESIGN_INSPECTOR_MIN_WIDTH,
  clampDesignInspectorWidth,
  clampDesignPanelWidth,
  readDesignInspectorWidth,
  readDesignPanelWidth,
  writeDesignInspectorWidth,
  writeDesignPanelWidth,
} from '@features/design-workspace/data';
import { modelRouteKey } from '@entities/model/modelPreferences';
import { reasoningLevelLabel } from '@entities/model/composerUtils';
import { importDesignImage, readDesignImage } from '@features/design-workspace/data/imageGeneration';
import { createDesignPdf } from '@features/design-workspace/data/designPdf';
import { encodeDesignDocumentArchive } from '@features/design-workspace/data/designDocumentArchive';

import './DesignCanvas.css';

interface Camera {
  x: number;
  y: number;
  scale: number;
}

interface DragState {
  pointerId: number;
  startX: number;
  startY: number;
  nodes: DesignNode[];
  didMove: boolean;
  beforeDocument?: DesignDocument;
  responsiveBreakpoint?: DesignResponsiveBreakpoint;
  resizeHandle?: ResizeHandle;
  constraintChildren?: DesignNode[];
}

type ResizeHandle = 'north-west' | 'north-east' | 'south-west' | 'south-east';
type DesignNodeTool = 'frame' | 'rectangle' | 'ellipse' | 'path' | 'text';

const designNodeToolLabelKeys = {
  frame: 'designEditorFrame',
  rectangle: 'designEditorRectangle',
  ellipse: 'designEditorEllipse',
  path: 'designEditorPath',
  text: 'designEditorTextNode',
} as const;

function designNodeTypeLabelKey(node: DesignNode) {
  if (node.type === 'frame') return 'designEditorFrame';
  if (node.type === 'rectangle') return 'designEditorRectangle';
  if (node.type === 'ellipse') return 'designEditorEllipse';
  if (node.type === 'path' || node.type === 'imported-svg') return 'designEditorPath';
  if (node.type === 'text') return 'designEditorTextNode';
  return 'designEditorImageNode';
}

function DesignLayerIcon({ type }: { type: DesignNode['type'] }) {
  if (type === 'frame') return <Frame aria-hidden="true" size={16} />;
  if (type === 'rectangle') return <RectangleHorizontal aria-hidden="true" size={16} />;
  if (type === 'ellipse') return <Circle aria-hidden="true" size={16} />;
  if (type === 'path') return <PenTool aria-hidden="true" size={16} />;
  if (type === 'text') return <Type aria-hidden="true" size={16} />;
  if (type === 'image') return <ImageIcon aria-hidden="true" size={16} />;
  return <CodeXml aria-hidden="true" size={16} />;
}

function designImageAssets(nodes: readonly DesignNode[], pageId: string) {
  const assets = nodes.filter((node) => node.pageId === pageId && node.visible).flatMap((node) => {
    const result: Array<{ assetId: string; mimeType: DesignImageNode['mimeType']; sizeBytes: number }> = [];
    if (node.type === 'image') result.push({ assetId: node.assetId, mimeType: node.mimeType, sizeBytes: node.sizeBytes });
    if ('fillStyle' in node && node.fillStyle?.type === 'image') result.push(node.fillStyle);
    return result;
  });
  return [...new Map(assets.map((asset) => [asset.assetId, asset])).values()];
}

function isSupportedDesignImageMimeType(value: string): value is DesignImageNode['mimeType'] {
  return value === 'image/png' || value === 'image/jpeg' || value === 'image/webp';
}

function designFillStyle(node: DesignFrameNode | Extract<DesignNode, { type: 'rectangle' | 'ellipse' | 'path' }>, variablesById: ReadonlyMap<string, string>, imageUrls: ReadonlyMap<string, string>): ReactCSSProperties {
  const color = resolveDesignColor(variablesById, node.fillVariableId, node.fill);
  if (node.fillStyle?.type === 'gradient') {
    const stops = designGradientStops(color, node.fillStyle);
    return { backgroundImage: `linear-gradient(${node.fillStyle.angle}deg, ${stops.map((stop) => `${stop.color} ${stop.position}%`).join(', ')})` };
  }
  if (node.fillStyle?.type === 'image') {
    const imageUrl = imageUrls.get(node.fillStyle.assetId);
    return imageUrl ? { backgroundImage: `url("${imageUrl}")`, backgroundPosition: 'center', backgroundRepeat: 'no-repeat', backgroundSize: node.fillStyle.fit } : { backgroundColor: color };
  }
  return { backgroundColor: color };
}

function designFillPreviewStyle(node: Extract<DesignNode, { type: 'frame' | 'rectangle' | 'ellipse' }>, imageUrls: ReadonlyMap<string, string>): ReactCSSProperties {
  if (node.fillStyle?.type === 'gradient') {
    const stops = designGradientStops(node.fill, node.fillStyle);
    return { backgroundImage: `linear-gradient(${node.fillStyle.angle}deg, ${stops.map((stop) => `${stop.color} ${stop.position}%`).join(', ')})` };
  }
  if (node.fillStyle?.type === 'image') {
    const imageUrl = imageUrls.get(node.fillStyle.assetId);
    return imageUrl ? { backgroundImage: `url("${imageUrl}")`, backgroundPosition: 'center', backgroundSize: 'cover' } : { backgroundColor: node.fill };
  }
  return { backgroundColor: node.fill };
}

function designGradientStops(startColor: string, fillStyle: Extract<DesignFillStyle, { type: 'gradient' }>): DesignGradientStop[] {
  return fillStyle.stops ?? [{ position: 0, color: startColor }, { position: 100, color: fillStyle.endColor }];
}

function updateDesignGradientStops<T extends DesignFrameNode | Extract<DesignNode, { type: 'rectangle' | 'ellipse' }>>(node: T, stops: DesignGradientStop[]): T {
  const firstStop = stops[0];
  const lastStop = stops[stops.length - 1];
  if (!firstStop || !lastStop) return node;
  return {
    ...node,
    fill: firstStop.color,
    fillStyle: {
      type: 'gradient',
      endColor: lastStop.color,
      angle: node.fillStyle?.type === 'gradient' ? node.fillStyle.angle : 0,
      stops,
    },
  };
}

function interpolateHexColor(start: string, end: string, amount: number): string {
  const startChannels = [1, 3, 5].map((offset) => Number.parseInt(start.slice(offset, offset + 2), 16));
  const endChannels = [1, 3, 5].map((offset) => Number.parseInt(end.slice(offset, offset + 2), 16));
  const channels = startChannels.map((channel, index) => Math.round(channel + ((endChannels[index] ?? channel) - channel) * amount));
  return `#${channels.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`;
}

function designNodeCornerRadii(node: Extract<DesignNode, { type: 'frame' | 'rectangle' }>): DesignCornerRadii {
  if (node.cornerRadii) return node.cornerRadii;
  const radius = node.radius ?? 0;
  return { topLeft: radius, topRight: radius, bottomRight: radius, bottomLeft: radius };
}

function designCornerRadiusCss(node: Extract<DesignNode, { type: 'frame' | 'rectangle' }>): string {
  const radii = designNodeCornerRadii(node);
  return `${radii.topLeft}px ${radii.topRight}px ${radii.bottomRight}px ${radii.bottomLeft}px`;
}

interface ResizeStart {
  pointerX: number;
  width: number;
}

interface CanvasViewport {
  width: number;
  height: number;
}

const initialCamera: Camera = { x: 48, y: 64, scale: 0.8 };
const emptyPast: DesignDocument[] = [];
const emptyDocumentFallback = emptyDesignDocument();

export function DesignCanvas() {
  const { t } = useTranslation();
  const [sidebarWidth, setSidebarWidth] = useState(readDesignPanelWidth);
  const [isResizing, setIsResizing] = useState(false);
  const [inspectorWidth, setInspectorWidth] = useState(readDesignInspectorWidth);
  const [isResizingInspector, setIsResizingInspector] = useState(false);
  const [loadState, setLoadState] = useState<DesignDocumentReadResult>({ kind: 'loading' });
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [documents, setDocuments] = useState<DesignDocumentSummary[]>([]);
  const [documentMenuWidth, setDocumentMenuWidth] = useState<number | null>(null);
  const [, setRevisions] = useState<DesignDocumentRevision[]>([]);
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const [isNewDocumentDialogOpen, setIsNewDocumentDialogOpen] = useState(false);
  const [newDocumentName, setNewDocumentName] = useState('');
  const [isDesignSystemDialogOpen, setIsDesignSystemDialogOpen] = useState(false);
  const [designSystemDialogKind, setDesignSystemDialogKind] = useState<'color' | 'text' | 'component' | null>(null);
  const [designSystemNameDraft, setDesignSystemNameDraft] = useState('');
  const revisionListRequestRef = useRef(0);


  const [, setStrokeColorDraft] = useState('#000000');
  const [, setStrokeWidthDraft] = useState('0');




  const [, setImageAltTextDraft] = useState('');
  const [, setImageFitDraft] = useState<'cover' | 'contain'>('cover');
  const [, setPathPointsDraft] = useState('');
  const [, setTextColorDraft] = useState('#23272C');
  const [, setTextFontSizeDraft] = useState('24');
  const [, setTextFontWeightDraft] = useState('500');
  const [, setTextStyleNameDraft] = useState('');
  const [, setSelectedTextStyleId] = useState('');
  const [, setSharedTextStyles] = useState<SharedDesignTextStyle[]>([]);


  const [pageNameDraft, setPageNameDraft] = useState('');
  const [renamingPageId, setRenamingPageId] = useState<string | null>(null);
  const [isRenamingDocument, setIsRenamingDocument] = useState(false);
  const [documentNameDraft, setDocumentNameDraft] = useState('');
  const [renamingNodeId, setRenamingNodeId] = useState<string | null>(null);
  const [editingTextNodeId, setEditingTextNodeId] = useState<string | null>(null);
  const editingTextRef = useRef<HTMLTextAreaElement>(null);
  const [nodeNameDraft, setNodeNameDraft] = useState('');
  const [isSwitchingDocument, setIsSwitchingDocument] = useState(false);
  const [camera, setCamera] = useState(initialCamera);
  const [snapGuides, setSnapGuides] = useState<DesignSnapGuides>({ vertical: null, horizontal: null });
  const [canvasViewport, setCanvasViewport] = useState<CanvasViewport>({ width: 0, height: 0 });
  const [past, setPast] = useState<DesignDocument[]>(emptyPast);
  const [future, setFuture] = useState<DesignDocument[]>(emptyPast);
  const [selectedNodeTool, setSelectedNodeTool] = useState<DesignNodeTool>('frame');
  const [prompt, setPrompt] = useState('');
  const [designReasoningEffort, setDesignReasoningEffort] = useState<string | null | undefined>(undefined);
  const [designReferences, setDesignReferences] = useState<PendingChatAttachment[]>([]);
  const designReferencesRef = useRef(designReferences);
  const [isReadingDesignReferences, setIsReadingDesignReferences] = useState(false);
  const isReadingDesignReferencesRef = useRef(false);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [modelQuery, setModelQuery] = useState('');
  const filePickerRef = useRef<HTMLInputElement>(null);
  const fillImageInputRef = useRef<HTMLInputElement>(null);
  const gradientTrackRef = useRef<HTMLFieldSetElement>(null);
  const [activeGradientStop, setActiveGradientStop] = useState(0);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isAddingGeneratedNodes, setIsAddingGeneratedNodes] = useState(false);
  const [generationCursor, setGenerationCursor] = useState<{ x: number; y: number } | null>(null);
  const [imageUrls, setImageUrls] = useState<ReadonlyMap<string, string>>(new Map());
  const [activeFillType, setActiveFillType] = useState<'solid' | 'gradient' | 'image'>('solid');
  const [designModelKey, setDesignModelKey] = useState<string | null>(null);
  const [contextMenuPosition, setContextMenuPosition] = useState<{ x: number; y: number } | null>(null);
  const contextMenuRef = useRef<HTMLDivElement>(null);
  const designSystemNameRef = useRef<HTMLInputElement>(null);
  const newDocumentNameRef = useRef<HTMLInputElement>(null);
  const documentSwitcherRef = useRef<HTMLDivElement>(null);
  const documentMenuTriggerRef = useRef<HTMLButtonElement>(null);
  const [prototypeOverlayId, setPrototypeOverlayId] = useState<string | null>(null);
  const prototypeOverlayCloseRef = useRef<HTMLButtonElement>(null);
  const prototypePreviousFocusRef = useRef<HTMLElement | null>(null);
  const [isPrototypePreview, setIsPrototypePreview] = useState(false);
  const [prototypeStartNodeId, setPrototypeStartNodeId] = useState<string | null>(null);
  const [prototypeDraft, setPrototypeDraft] = useState<{ targetNodeId: string; trigger: DesignPrototypeInteraction['trigger']; presentation: DesignPrototypeInteraction['presentation']; transition: DesignPrototypeInteraction['transition'] }>({ targetNodeId: '', trigger: 'click', presentation: 'navigate', transition: 'dissolve' });
  const [prototypeTransition, setPrototypeTransition] = useState<DesignPrototypeInteraction['transition']>('dissolve');
  const [isPrototypeDissolveVisible, setIsPrototypeDissolveVisible] = useState(true);
  const [responsivePreview, ] = useState<'base' | DesignResponsiveBreakpoint>('base');
  const [isCommentPlacementActive, setIsCommentPlacementActive] = useState(false);
  const [commentComposer, setCommentComposer] = useState<{ frameId: string; point: { x: number; y: number } } | null>(null);
  const [commentDraft, setCommentDraft] = useState('');
  const [selectedCommentId, setSelectedCommentId] = useState<string | null>(null);
  const [isCommentsPanelOpen, setIsCommentsPanelOpen] = useState(false);
  const projects = useChatStore((state) => state.projects);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const projectListLoadError = useChatStore((state) => state.projectListLoadError);
  const models = useChatStore((state) => state.models);
  const modelProviders = useChatStore((state) => state.modelProviders);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const chatSelectedModelKey = useChatStore((state) => state.selectedModelKey);
  const modelReasoningEfforts = useChatStore((state) => state.modelReasoningEfforts);
  const modelFavorites = useChatStore((state) => state.modelFavorites);
  const chatGptWorkspaceSelectionRequired = useChatStore((state) => state.chatGptWorkspaceSelectionRequired);
  const toggleFavoriteModel = useChatStore((state) => state.toggleFavoriteModel);
  const removeModelFavorite = useChatStore((state) => state.removeModelFavorite);
  const sidebarWidthRef = useRef(sidebarWidth);
  const resizeStartRef = useRef<ResizeStart | null>(null);
  const layoutRef = useRef<HTMLDivElement>(null);
  const separatorRef = useRef<HTMLDivElement>(null);
  const inspectorSeparatorRef = useRef<HTMLDivElement>(null);
  const inspectorResizeStartRef = useRef<ResizeStart | null>(null);
  const canvasRef = useRef<HTMLElement>(null);
  const cameraRef = useRef(camera);
  const documentRef = useRef(loadState.kind === 'ready' ? loadState.document : emptyDocumentFallback);
  const documentRevisionRef = useRef<number | null>(null);
  const saveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const saveFailedRef = useRef(false);
  const dragRef = useRef<DragState | null>(null);
  const lastNodePointerDownRef = useRef<{ nodeId: string; at: number; x: number; y: number } | null>(null);
  const generationRunRef = useRef(0);
  const prototypeTransitionFrameRef = useRef<number | null>(null);
  const generationControllerRef = useRef<AbortController | null>(null);
  const activeGenerationRef = useRef<DesignGenerationOperation | null>(null);
  const generationBaseRef = useRef<DesignDocument | null>(null);
  const visibleModels = models.filter((model) => !hiddenModelKeys.includes(modelRouteKey(model)));
  const imageAssetKey = loadState.kind === 'ready'
    ? designImageAssets(loadState.document.nodes, loadState.document.activePageId)
      .map((asset) => `${asset.assetId}:${asset.mimeType}:${asset.sizeBytes}`)
      .join('|')
    : '';
  useEffect(() => {
    if (prototypeOverlayId !== null) {
      prototypePreviousFocusRef.current = window.document.activeElement instanceof HTMLElement ? window.document.activeElement : null;
      prototypeOverlayCloseRef.current?.focus();
      return () => {
        prototypePreviousFocusRef.current?.focus();
        prototypePreviousFocusRef.current = null;
      };
    }
    return undefined;
  }, [prototypeOverlayId]);

  useEffect(() => {
    if (loadState.kind !== 'ready') {
      setPathPointsDraft('');
      return;
    }
    const selectedPath = loadState.document.nodes.find((node) => node.type === 'path' && loadState.document.selectedNodeIds.includes(node.id));
    setPathPointsDraft(selectedPath?.type === 'path' ? selectedPath.points.map((point) => `${point.x}, ${point.y}`).join('\n') : '');
  }, [loadState]);
  useEffect(() => {
    if (editingTextNodeId) editingTextRef.current?.focus();
  }, [editingTextNodeId]);
  useEffect(() => {
    if (!imageAssetKey) {
      setImageUrls(new Map());
      return;
    }
    let cancelled = false;
    const createdUrls: string[] = [];
    const currentDocument = documentRef.current;
    const unique = designImageAssets(currentDocument.nodes, currentDocument.activePageId);
    const limited: Array<{ assetId: string; mimeType: DesignImageNode['mimeType']; sizeBytes: number }> = [];
    let totalBytes = 0;
    for (const node of unique) {
      if (limited.length >= 16 || totalBytes + node.sizeBytes > 32 * 1024 * 1024) continue;
      limited.push(node);
      totalBytes += node.sizeBytes;
    }
    void Promise.all(limited.map(async (node) => {
      try {
        const image = await readDesignImage(node.assetId, node.mimeType, node.sizeBytes);
        const bytes = image.bytes.buffer.slice(image.bytes.byteOffset, image.bytes.byteOffset + image.bytes.byteLength);
        if (!(bytes instanceof ArrayBuffer)) return null;
        const url = URL.createObjectURL(new Blob([bytes], { type: image.mimeType }));
        if (cancelled) {
          URL.revokeObjectURL(url);
          return null;
        }
        createdUrls.push(url);
        return [node.assetId, url] as const;
      } catch {
        return null;
      }
    })).then((results) => {
      if (cancelled) return;
      setImageUrls(new Map(results.filter((entry): entry is readonly [string, string] => entry !== null)));
    });
    return () => {
      cancelled = true;
      createdUrls.forEach((url) => { URL.revokeObjectURL(url); });
    };
  }, [imageAssetKey]);
  useEffect(() => {
    let active = true;
    void listSharedDesignTextStyles().then((styles) => {
      if (active) setSharedTextStyles(styles);
    }).catch(() => {
      if (active) toast.error(t('designEditorSharedStyleLoadFailed'));
    });
    return () => { active = false; };
  }, [t]);
  useEffect(() => {
    if (loadState.kind !== 'ready') {
      setSelectedTextStyleId('');
      return;
    }
    const selectedText = loadState.document.nodes.find((node) => node.type === 'text' && loadState.document.selectedNodeIds.includes(node.id));
    if (selectedText?.type !== 'text') {
      setSelectedTextStyleId('');
      return;
    }
    setTextColorDraft(selectedText.color);
    setTextFontSizeDraft(String(selectedText.fontSize));
    setTextFontWeightDraft(String(selectedText.fontWeight));
    setSelectedTextStyleId(selectedText.textStyleId ?? '');
    setTextStyleNameDraft(selectedText.textStyleId
      ? loadState.document.textStyles.find((style) => style.id === selectedText.textStyleId)?.name ?? ''
      : '');
  }, [loadState]);
  useEffect(() => {
    if (loadState.kind !== 'ready' || loadState.document.selectedNodeIds.length !== 1) {
      setImageAltTextDraft('');
      setImageFitDraft('cover');
      return;
    }
    const selectedImage = loadState.document.nodes.find((node) => node.id === loadState.document.selectedNodeIds[0]);
    if (selectedImage?.type !== 'image') {
      setImageAltTextDraft('');
      setImageFitDraft('cover');
      return;
    }
    setImageAltTextDraft(selectedImage.altText);
    setImageFitDraft(selectedImage.fit);
  }, [loadState]);
  const selectedModelKey =
    (designModelKey && visibleModels.some((model) => model.key === designModelKey) ? designModelKey : null) ??
    (chatSelectedModelKey && visibleModels.some((model) => model.key === chatSelectedModelKey)
      ? chatSelectedModelKey
      : visibleModels[0]?.key ?? '');
  const selectedDesignModel = visibleModels.find((model) => model.key === selectedModelKey);
  const selectedDesignModelForPicker = selectedDesignModel ?? null;
  const storedDesignReasoningEffort = selectedDesignModel
    ? modelReasoningEfforts[modelRouteKey(selectedDesignModel)] ?? null
    : null;
  const requestedDesignReasoningEffort = designReasoningEffort === undefined
    ? storedDesignReasoningEffort
    : designReasoningEffort;
  const selectedDesignReasoningEffort = selectedDesignModel?.supportsReasoning &&
    requestedDesignReasoningEffort !== null &&
    selectedDesignModel.reasoningLevels.includes(requestedDesignReasoningEffort)
    ? requestedDesignReasoningEffort
    : null;
  const currentDocumentId = loadState.kind === 'ready' ? loadState.document.id : null;
  const selectedStrokeNode = loadState.kind === 'ready'
    ? loadState.document.nodes.find((node) => loadState.document.selectedNodeIds.includes(node.id) && (node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' || node.type === 'path'))
    : undefined;
  const selectedStrokeColor = selectedStrokeNode && 'stroke' in selectedStrokeNode ? selectedStrokeNode.stroke : null;
  const selectedStrokeWidth = selectedStrokeNode && 'strokeWidth' in selectedStrokeNode ? selectedStrokeNode.strokeWidth : null;
  const selectedNode = loadState.kind === 'ready' && loadState.document.selectedNodeIds.length === 1
    ? loadState.document.nodes.find((node) => node.id === loadState.document.selectedNodeIds[0])
    : undefined;
  const selectedPrototypeInteraction = selectedNode && loadState.kind === 'ready'
    ? loadState.document.prototypeInteractions.find((interaction) => interaction.sourceNodeId === selectedNode.id)
    : undefined;
  const prototypeDraftSourceKey = `${selectedNode?.id ?? ''}|${selectedPrototypeInteraction?.id ?? ''}`;
  useEffect(() => {
    const sourceNodeId = prototypeDraftSourceKey.slice(0, prototypeDraftSourceKey.indexOf('|'));
    const interaction = documentRef.current.prototypeInteractions.find((item) => item.sourceNodeId === sourceNodeId);
    setPrototypeDraft(interaction
      ? { targetNodeId: interaction.targetNodeId, trigger: interaction.trigger, presentation: interaction.presentation, transition: interaction.transition }
      : { targetNodeId: '', trigger: 'click', presentation: 'navigate', transition: 'dissolve' });
  }, [prototypeDraftSourceKey]);
  const selectedFillType = selectedNode && 'fillStyle' in selectedNode ? selectedNode.fillStyle?.type : undefined;
  const selectedFillSelectionKey = `${selectedNode?.id ?? ''}|${selectedFillType ?? ''}`;
  const selectedGradientStops = selectedNode && 'fill' in selectedNode && selectedNode.fillStyle?.type === 'gradient'
    ? designGradientStops(selectedNode.fill, selectedNode.fillStyle)
    : [];
  const selectedGradientStop = selectedGradientStops[Math.min(activeGradientStop, Math.max(0, selectedGradientStops.length - 1))];
  const selectedImageFill = selectedNode && 'fillStyle' in selectedNode && selectedNode.fillStyle?.type === 'image' ? selectedNode.fillStyle : undefined;

  const addGradientStopAtPointer = (event: ReactPointerEvent<HTMLFieldSetElement>) => {
    if (!selectedNode || selectedGradientStops.length >= 8) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    const requestedPosition = clamp(((event.clientX - bounds.left) / bounds.width) * 100, 0, 100);
    const rightIndex = selectedGradientStops.findIndex((stop) => stop.position > requestedPosition);
    const leftIndex = Math.max(0, rightIndex < 0 ? selectedGradientStops.length - 2 : rightIndex - 1);
    const left = selectedGradientStops[leftIndex];
    const right = selectedGradientStops[leftIndex + 1];
    if (!left || !right || right.position - left.position < 2) return;
    const position = clamp(requestedPosition, left.position + 1, right.position - 1);
    const ratio = (position - left.position) / (right.position - left.position);
    const stops = [...selectedGradientStops.slice(0, leftIndex + 1), { position, color: interpolateHexColor(left.color, right.color, ratio) }, ...selectedGradientStops.slice(leftIndex + 1)];
    updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' ? updateDesignGradientStops(node, stops) : node);
    setActiveGradientStop(leftIndex + 1);
  };

  const moveGradientStop = (index: number, clientX: number) => {
    if (!selectedNode || !gradientTrackRef.current) return;
    const bounds = gradientTrackRef.current.getBoundingClientRect();
    const minimum = (selectedGradientStops[index - 1]?.position ?? 0) + 1;
    const maximum = (selectedGradientStops[index + 1]?.position ?? 100) - 1;
    const position = clamp(((clientX - bounds.left) / bounds.width) * 100, minimum, maximum);
    const stops = selectedGradientStops.map((stop, stopIndex) => stopIndex === index ? { ...stop, position } : stop);
    updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' ? updateDesignGradientStops(node, stops) : node);
  };

  useEffect(() => {
    const fillType = selectedFillSelectionKey.slice(selectedFillSelectionKey.lastIndexOf('|') + 1);
    setActiveFillType(fillType === 'gradient' || fillType === 'image' ? fillType : 'solid');
    setActiveGradientStop(0);
  }, [selectedFillSelectionKey]);


  const activePage = loadState.kind === 'ready' ? loadState.document.pages.find((page) => page.id === loadState.document.activePageId) : undefined;

  useEffect(() => {
    setPageNameDraft(activePage?.name ?? '');
  }, [activePage?.name]);

  useEffect(() => {
    if (selectedStrokeColor === null || selectedStrokeWidth === null) return;
    setStrokeColorDraft(selectedStrokeColor);
    setStrokeWidthDraft(String(selectedStrokeWidth));
  }, [selectedStrokeColor, selectedStrokeWidth]);


  useEffect(() => {
    cameraRef.current = camera;
  }, [camera]);

  useEffect(() => {
    if (!contextMenuPosition) return;
    const menu = contextMenuRef.current;
    if (!menu) return;
    const bounds = menu.getBoundingClientRect();
    const x = clamp(contextMenuPosition.x, 8, Math.max(8, window.innerWidth - bounds.width - 8));
    const y = clamp(contextMenuPosition.y, 8, Math.max(8, window.innerHeight - bounds.height - 8));
    if (x !== contextMenuPosition.x || y !== contextMenuPosition.y) {
      setContextMenuPosition({ x, y });
    }
    menu.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus();
  }, [contextMenuPosition]);

  useEffect(() => {
    if (!contextMenuPosition) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!(event.target instanceof window.Node) || !contextMenuRef.current?.contains(event.target)) setContextMenuPosition(null);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      setContextMenuPosition(null);
    };
    window.document.addEventListener('pointerdown', closeOnOutsidePointer);
    window.document.addEventListener('keydown', closeOnEscape);
    return () => {
      window.document.removeEventListener('pointerdown', closeOnOutsidePointer);
      window.document.removeEventListener('keydown', closeOnEscape);
    };
  }, [contextMenuPosition]);

  useEffect(() => {
    designReferencesRef.current = designReferences;
  }, [designReferences]);

  useEffect(() => () => {
    designReferencesRef.current.forEach((attachment) => {
      if (attachment.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
    });
  }, []);

  useEffect(() => () => {
    if (prototypeTransitionFrameRef.current !== null) window.cancelAnimationFrame(prototypeTransitionFrameRef.current);
  }, []);

  useEffect(() => {
    if (!currentDocumentId) return;
    let active = true;
    const request = ++revisionListRequestRef.current;
    void listDesignDocumentRevisions(currentDocumentId)
      .then((items) => { if (active && request === revisionListRequestRef.current) setRevisions(items); })
      .catch(() => { if (active) toast.error(t('designEditorHistoryLoadFailed')); });
    return () => { active = false; revisionListRequestRef.current += 1; };
  }, [currentDocumentId, t]);

  useEffect(() => {
    let active = true;
    if (loadAttempt > 0) documentRevisionRef.current = null;
    void (async () => {
      try {
        const summaries = await listDesignDocuments();
        if (active) setDocuments(summaries);
        const firstSummary = summaries[0];
        if (firstSummary) {
          const stored = await getDesignDocument(firstSummary.id);
          if (!active) return;
          const revision = stored.needsMigration
            ? await saveDesignDocumentRevision(stored.document, stored.revision)
            : stored.revision;
          if (!active) return;
          documentRevisionRef.current = revision;
          documentRef.current = stored.document;
          setDocuments(summaries);
          setLoadState({ kind: 'ready', document: stored.document, needsMigration: false });
          return;
        }
        const legacy = readDesignDocument();
        if (legacy.kind !== 'ready') {
          if (active) setLoadState(legacy);
          return;
        }
        const revision = await saveDesignDocumentRevision(legacy.document, null);
        const stored = await getDesignDocument(legacy.document.id);
        if (!active) return;
        documentRevisionRef.current = revision;
        documentRef.current = stored.document;
        setDocuments([stored]);
        setLoadState({ kind: 'ready', document: stored.document, needsMigration: false });
      } catch {
        if (active) setLoadState({ kind: 'unavailable' });
      }
    })();
    return () => { active = false; };
  }, [loadAttempt]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: The canvas mounts after recovery and must be observed when loading succeeds.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      setCanvasViewport({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [loadState.kind]);

  useEffect(() => () => {
    generationRunRef.current += 1;
    generationControllerRef.current?.abort();
    generationControllerRef.current = null;
    const operation = activeGenerationRef.current;
    activeGenerationRef.current = null;
    if (operation) {
      void operation.cancel().catch((cause: unknown) => {
        globalThis.reportError(new Error('Failed to cancel a design generation after the editor closed.', { cause }));
      });
    }
  }, []);

  useEffect(() => {
    layoutRef.current?.style.setProperty('--design-control-panel-width', `${sidebarWidth}px`);
  }, [sidebarWidth]);

  useEffect(() => {
    layoutRef.current?.style.setProperty('--design-inspector-width', `${selectedNode ? inspectorWidth : 0}px`);
  }, [inspectorWidth, selectedNode]);

  useEffect(() => {
    if (!isResizingInspector) return;
    const move = (event: PointerEvent) => {
      const start = inspectorResizeStartRef.current;
      if (!start) return;
      const width = clampDesignInspectorWidth(start.width + start.pointerX - event.clientX);
      layoutRef.current?.style.setProperty('--design-inspector-width', `${width}px`);
      inspectorSeparatorRef.current?.setAttribute('aria-valuenow', String(width));
    };
    const finish = () => {
      const width = clampDesignInspectorWidth(Number.parseFloat(layoutRef.current?.style.getPropertyValue('--design-inspector-width') ?? String(DESIGN_INSPECTOR_DEFAULT_WIDTH)));
      setInspectorWidth(width);
      void writeDesignInspectorWidth(width).catch(() => toast.error(t('designEditorSaveFailed')));
      inspectorResizeStartRef.current = null;
      setIsResizingInspector(false);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', finish);
    window.addEventListener('pointercancel', finish);
    return () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', finish);
      window.removeEventListener('pointercancel', finish);
      inspectorResizeStartRef.current = null;
    };
  }, [isResizingInspector, t]);

  useEffect(() => {
    if (!isResizing) return;
    const move = (event: PointerEvent) => {
      const start = resizeStartRef.current;
      if (!start) return;
      const width = clampDesignPanelWidth(start.width + event.clientX - start.pointerX);
      sidebarWidthRef.current = width;
      layoutRef.current?.style.setProperty('--design-control-panel-width', `${width}px`);
      separatorRef.current?.setAttribute('aria-valuenow', String(width));
    };
    const finish = () => {
      const width = sidebarWidthRef.current;
      setSidebarWidth(width);
      void writeDesignPanelWidth(width).catch(() => toast.error(t('designEditorSaveFailed')));
      resizeStartRef.current = null;
      setIsResizing(false);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', finish);
    window.addEventListener('pointercancel', finish);
    return () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', finish);
      window.removeEventListener('pointercancel', finish);
      resizeStartRef.current = null;
    };
  }, [isResizing, t]);

  const persist = (next: DesignDocument) => {
    const write = saveQueueRef.current.then(async () => {
      const revision = await saveDesignDocumentRevision(next, documentRevisionRef.current);
      documentRevisionRef.current = revision;
      saveFailedRef.current = false;
      revisionListRequestRef.current += 1;
      setRevisions((items) => [
        { revision, document: next, createdAtUnixMs: Date.now() },
        ...items.filter((item) => item.revision !== revision),
      ].slice(0, 30));
    });
    const reportedWrite = write.catch((cause: unknown) => {
      saveFailedRef.current = true;
      toast.error(t('designEditorSaveFailed'));
      throw cause;
    });
    saveQueueRef.current = reportedWrite.catch(() => undefined);
    return reportedWrite;
  };

  const publish = (next: DesignDocument, recordHistory = true) => {
    const previous = documentRef.current;
    const synchronized = synchronizeDesignComponentDependents(previous, next);
    documentRef.current = synchronized;
    if (recordHistory) {
      setPast((items) => [...items.slice(-99), previous]);
      setFuture([]);
    }
    setLoadState({ kind: 'ready', document: synchronized, needsMigration: false });
    return persist(synchronized);
  };

  const selectNode = (nodeId: string, additive: boolean) => {
    const current = documentRef.current;
    const selectedNodeIds = additive
      ? current.selectedNodeIds.includes(nodeId)
        ? current.selectedNodeIds.filter((id) => id !== nodeId)
        : [...current.selectedNodeIds, nodeId]
      : [nodeId];
    void publish({ ...current, selectedNodeIds }, false);
    if (!additive) {
      const selected = current.nodes.find((node) => node.id === nodeId);
      if (selected) {
        const scale = cameraRef.current.scale;
        setCamera({
          scale,
          x: canvasViewport.width / 2 - (selected.x + selected.width / 2) * scale,
          y: canvasViewport.height / 2 - (selected.y + selected.height / 2) * scale,
        });
      }
    }
  };

  const startTextEditing = (nodeId: string) => {
    const previousClick = lastNodePointerDownRef.current;
    const current = documentRef.current;
    const node = current.nodes.find((entry) => entry.id === nodeId);
    if (node?.type === 'text' && previousClick?.nodeId === nodeId && (node.x !== previousClick.x || node.y !== previousClick.y)) {
      void publish({ ...current, nodes: current.nodes.map((entry) => entry.id === nodeId && entry.type === 'text'
        ? { ...entry, x: previousClick.x, y: previousClick.y }
        : entry) }, false);
    }
    lastNodePointerDownRef.current = null;
    setEditingTextNodeId(nodeId);
  };

  const openNodeContextMenu = (event: ReactMouseEvent<HTMLButtonElement>, node: DesignNode) => {
    if (isPrototypePreview) return;
    event.preventDefault();
    event.stopPropagation();
    const current = documentRef.current;
    if (!current.selectedNodeIds.includes(node.id)) {
      void publish({ ...current, selectedNodeIds: [node.id] }, false);
    }
    const bounds = event.currentTarget.getBoundingClientRect();
    setContextMenuPosition({
      x: event.clientX || bounds.left,
      y: event.clientY || bounds.bottom,
    });
  };

  const handleContextMenuKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]'));
    const activeElement = window.document.activeElement;
    const currentIndex = activeElement instanceof HTMLButtonElement ? items.indexOf(activeElement) : -1;
    let nextIndex: number | null = null;
    if (event.key === 'ArrowDown') nextIndex = (currentIndex + 1) % items.length;
    if (event.key === 'ArrowUp') nextIndex = (currentIndex - 1 + items.length) % items.length;
    if (event.key === 'Home') nextIndex = 0;
    if (event.key === 'End') nextIndex = items.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    items[nextIndex]?.focus();
  };

  const selectProject = (projectId: string | null) => {
    const current = documentRef.current;
    if (current.projectId === projectId) return;
    void publish({ ...current, projectId });
    setDocuments((items) => items.map((item) => item.id === current.id ? { ...item, projectId } : item));
  };

  const selectDocument = async (id: string) => {
    if (id === documentRef.current.id || isSwitchingDocument) return;
    setIsSwitchingDocument(true);
    try {
      await saveQueueRef.current;
      if (saveFailedRef.current) await persist(documentRef.current);
      const stored = await getDesignDocument(id);
      documentRevisionRef.current = stored.revision;
      documentRef.current = stored.document;
      setPast([]);
      setFuture([]);
      setLoadState({ kind: 'ready', document: stored.document, needsMigration: false });
      setCamera(initialCamera);
    } catch {
      toast.error(t('designEditorLoadFailed'));
    } finally {
      setIsSwitchingDocument(false);
    }
  };

  const saveDocumentName = () => {
    if (loadState.kind !== 'ready' || isGenerating || isSwitchingDocument) return;
    const name = documentNameDraft.trim();
    if (!name || Array.from(name).length > 256) return;
    void publish({ ...loadState.document, name });
    setDocuments((items) => items.map((item) => item.id === loadState.document.id ? { ...item, name } : item));
    setIsRenamingDocument(false);
  };

  const createDocument = async (documentName: string) => {
    const name = documentName.trim();
    if (isSwitchingDocument || !name || Array.from(name).length > 256) return;
    setIsSwitchingDocument(true);
    try {
      await saveQueueRef.current;
      if (saveFailedRef.current) await persist(documentRef.current);
      const next = { ...emptyDesignDocument(t('designEditorPageName', { count: 1 })), name, projectId: documentRef.current.projectId };
      const revision = await saveDesignDocumentRevision(next, null);
      const stored = await getDesignDocument(next.id);
      documentRevisionRef.current = revision;
      revisionListRequestRef.current += 1;
      setRevisions([{ revision, document: next, createdAtUnixMs: Date.now() }]);
      documentRef.current = stored.document;
      setDocuments((items) => [stored, ...items]);
      setPast([]);
      setFuture([]);
      setLoadState({ kind: 'ready', document: stored.document, needsMigration: false });
      setCamera(initialCamera);
      setIsNewDocumentDialogOpen(false);
      setNewDocumentName('');
    } catch {
      toast.error(t('designEditorSaveFailed'));
    } finally {
      setIsSwitchingDocument(false);
    }
  };

  const updateNodeState = (nodeId: string, patch: Partial<Pick<DesignNode, 'visible' | 'locked'>> & { scrollable?: boolean }) => {
    const current = documentRef.current;
    void publish({
      ...current,
      nodes: current.nodes.map((node) => {
        if (node.id !== nodeId) return node;
        if (node.type === 'frame') return { ...node, ...patch };
        const nodePatch: Partial<Pick<DesignNode, 'visible' | 'locked'>> = {
          ...(patch.visible === undefined ? {} : { visible: patch.visible }),
          ...(patch.locked === undefined ? {} : { locked: patch.locked }),
        };
        return { ...node, ...nodePatch };
      }),
    });
  };

  const updateNode = (nodeId: string, update: (node: DesignNode) => DesignNode) => {
    const current = documentRef.current;
    void publish({ ...current, nodes: current.nodes.map((node) => {
      if (node.id !== nodeId) return node;
      const updated = update(node);
      if (node.type !== 'text' || updated.type !== 'text' || !node.textStyleId || updated.textStyleId !== node.textStyleId) return updated;
      const styleChanged = node.color !== updated.color || node.fontSize !== updated.fontSize || node.fontWeight !== updated.fontWeight ||
        node.fontFamily !== updated.fontFamily || node.lineHeight !== updated.lineHeight || node.letterSpacing !== updated.letterSpacing || node.textAlign !== updated.textAlign;
      if (!styleChanged) return updated;
      const { textStyleId: _textStyleId, ...unlinked } = updated;
      return unlinked;
    }) });
  };

  const openDesignSystemDialog = (kind: 'color' | 'text' | 'component', name: string) => {
    setDesignSystemDialogKind(kind);
    setDesignSystemNameDraft(name);
    setIsDesignSystemDialogOpen(true);
  };

  const saveDesignSystemItem = () => {
    if (loadState.kind !== 'ready' || !designSystemDialogKind || !designSystemNameDraft.trim()) return;
    try {
      let next = documentRef.current;
      if (designSystemDialogKind === 'text') {
        next = createDesignTextStyle(next, designSystemNameDraft);
        const style = next.textStyles.at(-1);
        if (!style) throw new Error('The text style was not created.');
        next = applyDesignTextStyle(next, style.id);
      } else if (designSystemDialogKind === 'component') {
        next = createDesignComponent(next, designSystemNameDraft);
      } else {
        const selected = next.nodes.find((node) => node.id === next.selectedNodeIds[0]);
        const supportsColorToken = selected?.type === 'text' || (selected?.type === 'frame' || selected?.type === 'rectangle' || selected?.type === 'ellipse' || selected?.type === 'path') && !selected.fillStyle;
        const color = selected?.type === 'text' ? selected.color : supportsColorToken && selected && 'fill' in selected ? selected.fill : null;
        if (!selected || !color || next.variables.length >= 1000 || next.variables.some((variable) => variable.name.toLocaleLowerCase() === designSystemNameDraft.trim().toLocaleLowerCase())) {
          throw new Error('Select a layer with a unique color variable name.');
        }
        const variableId = crypto.randomUUID();
        next = {
          ...next,
          variables: [...next.variables, { id: variableId, name: designSystemNameDraft.trim(), value: color }],
          nodes: next.nodes.map((node) => node.id !== selected.id ? node
            : node.type === 'text' ? { ...node, colorVariableId: variableId }
              : 'fill' in node ? { ...node, fillVariableId: variableId }
                : node),
        };
      }
      void publish(next);
      setIsDesignSystemDialogOpen(false);
      setDesignSystemDialogKind(null);
    } catch {
      toast.error(t(designSystemDialogKind === 'text' ? 'designEditorTextStyleSaveFailed' : designSystemDialogKind === 'component' ? 'designEditorComponentCreateFailed' : 'designEditorColorVariableSaveFailed'));
    }
  };

  const updateNodeGeometry = (nodeId: string, patch: Partial<Pick<DesignNode, 'name' | 'x' | 'y' | 'width' | 'height' | 'rotation'>>) => {
    updateNode(nodeId, (node) => ({ ...node, ...patch }));
  };

  const updateNodeFill = (nodeId: string, fill: string) => {
    updateNode(nodeId, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' || node.type === 'path'
      ? { ...node, fill, fillVariableId: undefined }
      : node);
  };

  const addPage = () => {
    if (loadState.kind !== 'ready' || isGenerating || isSwitchingDocument) return;
    try {
      const name = t('designEditorPageName', { count: loadState.document.pages.length + 1 });
      void publish(addDesignPage(loadState.document, name));
    } catch {
      toast.error(t('designEditorPageChangeFailed'));
    }
  };

  const savePageName = (pageId: string) => {
    if (loadState.kind !== 'ready' || isGenerating) return;
    try {
      void publish(renameDesignPage(loadState.document, pageId, pageNameDraft));
      setRenamingPageId(null);
    } catch {
      toast.error(t('designEditorPageNameInvalid'));
    }
  };

  const deletePage = (pageId: string) => {
    if (loadState.kind !== 'ready' || loadState.document.pages.length < 2 || isGenerating) return;
    try {
      void publish(removeDesignPage(loadState.document, pageId));
    } catch {
      toast.error(t('designEditorPageChangeFailed'));
    }
  };

  const switchPage = (pageId: string) => {
    if (loadState.kind !== 'ready' || pageId === loadState.document.activePageId) return;
    void publish({ ...loadState.document, activePageId: pageId, selectedNodeIds: [] }, false);
  };

  const cancelGeneration = () => {
    generationRunRef.current += 1;
    generationControllerRef.current?.abort();
    generationControllerRef.current = null;
    const base = generationBaseRef.current;
    if (base) {
      documentRef.current = base;
      setLoadState({ kind: 'ready', document: base, needsMigration: false });
    }
    generationBaseRef.current = null;
    const operation = activeGenerationRef.current;
    activeGenerationRef.current = null;
    setIsGenerating(false);
    setIsAddingGeneratedNodes(false);
    if (operation) {
      void operation.cancel().catch(() => toast.error(t('designEditorCancelFailed')));
    }
  };

  const generateDesign = async () => {
    if (isGenerating) return;
    const normalizedPrompt = prompt.trim();
    const model = visibleModels.find((item) => item.key === selectedModelKey);
    if (normalizedPrompt.length === 0) {
      toast.warning(t('designEditorGenerationRequired'));
      return;
    }
    if (!model) {
      toast.warning(t('designEditorNoModels'));
      return;
    }
    if (designReferences.length > 0 && !model.supportsImages) {
      toast.warning(t('designEditorReferenceModelUnsupported'));
      return;
    }
    const runId = generationRunRef.current + 1;
    generationRunRef.current = runId;
    const controller = new AbortController();
    generationControllerRef.current = controller;
    const base = documentRef.current;
    let generatedDocument: DesignDocument | null = null;
    generationBaseRef.current = base;
    setIsGenerating(true);
    try {
      const references: DesignGenerationReference[] = designReferences.map((attachment) => {
        if (attachment.mimeType !== 'image/png' && attachment.mimeType !== 'image/jpeg' && attachment.mimeType !== 'image/webp') {
          throw new Error('The selected design reference format is not supported.');
        }
        return {
          id: attachment.id,
          name: attachment.name,
          mimeType: attachment.mimeType,
          bytes: attachment.bytes,
        };
      });
      const importedReferenceImages = new Map<string, Awaited<ReturnType<typeof importDesignImage>>>();
      const applyGeneratedOperation = async (current: DesignDocument, generated: DesignCanvasOperation): Promise<DesignDocument> => {
        if (generated.type !== 'create-image-from-reference') return applyDesignOperations(current, [generated]);
        const reference = references.find((entry) => entry.id === generated.referenceId);
        if (!reference) throw new Error('The generated image reference is unavailable.');
        let image = importedReferenceImages.get(reference.id);
        if (!image) {
          image = await importDesignImage(reference.mimeType, reference.bytes);
          importedReferenceImages.set(reference.id, image);
        }
        return applyDesignOperations(current, [{
          type: 'create',
          node: { ...generated.node, assetId: image.id, mimeType: reference.mimeType, sizeBytes: image.sizeBytes },
        }]);
      };
      if (generationRunRef.current !== runId) return;
      const projectContext = base.projectId === null ? null : projects.find((project) => project.id === base.projectId);
      let preview = base;
      let passPrompt = normalizedPrompt;
      let passMode: 'create' | 'revise' = 'create';
      let passTargets = base.selectedNodeIds;
      let passReferences = references;
      let inspectedFrameCount = 0;
      for (let passIndex = 0; passIndex < 4; passIndex += 1) {
        const passBase = preview;
        const frameRequests = new Set<string>();
        let streamFailure: unknown = null;
        let streamQueue = Promise.resolve();
        try {
          const operation = await startDesignGeneration({
            prompt: passPrompt,
            mode: passMode,
            reasoningEffort: selectedDesignReasoningEffort,
            targetNodeIds: passTargets,
            model,
            document: passBase,
            references: passReferences,
            projectContext: projectContext ? { id: projectContext.id, name: projectContext.name } : null,
            allowFrameInspection: model.supportsImages && passIndex < 3,
            onOperations: (operations) => {
              for (const streamedOperation of operations) {
                if (streamedOperation.type === 'inspect-frame') frameRequests.add(streamedOperation.nodeId);
              }
              setIsAddingGeneratedNodes(true);
              streamQueue = streamQueue.then(async () => {
                for (const streamedOperation of operations) {
                  const targetNode = 'node' in streamedOperation ? streamedOperation.node : null;
                  if (targetNode) {
                    setGenerationCursor({
                      x: camera.x + (targetNode.x + Math.min(targetNode.width / 2, 18)) * camera.scale,
                      y: camera.y + (targetNode.y + Math.min(targetNode.height / 2, 18)) * camera.scale,
                    });
                  }
                  await waitForDesignStep(controller.signal, targetNode ? 190 : 70);
                  if (generationRunRef.current !== runId) return;
                  preview = await applyGeneratedOperation(preview, streamedOperation);
                  documentRef.current = preview;
                  setLoadState({ kind: 'ready', document: preview, needsMigration: false });
                }
              }).catch((error: unknown) => {
                streamFailure = error;
              });
            },
          });
          if (generationRunRef.current !== runId) {
            await operation.cancel();
            return;
          }
          activeGenerationRef.current = operation;
          const generatedOperations = await operation.result;
          await streamQueue;
          if (streamFailure) throw streamFailure;
          if (generationRunRef.current !== runId) return;
          setIsAddingGeneratedNodes(true);
          for (const nextOperation of generatedOperations) {
            if (nextOperation.type === 'inspect-frame') frameRequests.add(nextOperation.nodeId);
            const targetNode = 'node' in nextOperation ? nextOperation.node : null;
            if (targetNode) {
              setGenerationCursor({
                x: camera.x + (targetNode.x + Math.min(targetNode.width / 2, 18)) * camera.scale,
                y: camera.y + (targetNode.y + Math.min(targetNode.height / 2, 18)) * camera.scale,
              });
            }
            await waitForDesignStep(controller.signal, targetNode ? 190 : 70);
            if (generationRunRef.current !== runId) return;
            preview = await applyGeneratedOperation(preview, nextOperation);
            documentRef.current = preview;
            setLoadState({ kind: 'ready', document: preview, needsMigration: false });
          }
          activeGenerationRef.current = null;
        } catch (error) {
          if (passIndex === 0 || controller.signal.aborted) throw error;
          preview = passBase;
          documentRef.current = preview;
          setLoadState({ kind: 'ready', document: preview, needsMigration: false });
          toast.warning(t('designEditorFrameReviewFailed'));
          break;
        }
        if (generationRunRef.current !== runId || frameRequests.size === 0 || passIndex === 3) break;
        const screenshots: DesignGenerationReference[] = [];
        const targetIds = new Set<string>();
        let screenshotBytes = 0;
        for (const frameId of frameRequests) {
          if (controller.signal.aborted || generationRunRef.current !== runId) break;
          if (inspectedFrameCount >= 3) break;
          try {
            const screenshot = await captureDesignFrameScreenshot(preview, frameId);
            const frame = preview.nodes.find((node) => node.id === frameId && node.type === 'frame');
            if (!frame || screenshot.length > 10 * 1024 * 1024 || screenshotBytes + screenshot.length > 14 * 1024 * 1024) throw new Error('The frame screenshot is unavailable or too large.');
            screenshots.push({ id: `frame-review-${crypto.randomUUID()}`, name: `${frame.name} rendered screenshot.png`, mimeType: 'image/png', bytes: screenshot });
            screenshotBytes += screenshot.length;
            for (const nodeId of designFrameNodeIds(preview, frameId)) targetIds.add(nodeId);
            inspectedFrameCount += 1;
          } catch {
            toast.warning(t('designEditorFrameCaptureFailed'));
          }
        }
        if (controller.signal.aborted || generationRunRef.current !== runId || screenshots.length === 0 || targetIds.size === 0) break;
        passMode = 'revise';
        passTargets = [...targetIds];
        passReferences = screenshots;
        passPrompt = `Review the attached rendered screenshot(s) requested by you. Compare each frame's actual rendered appearance with the original design brief: ${normalizedPrompt}. Make only corrections needed to improve visual fidelity, hierarchy, alignment, spacing, contrast, or clipping. Preserve correct existing details. Do not add a replacement frame. If visual corrections are complete, return normal design operations without asking for another screenshot.`;
      }
      if (generationRunRef.current !== runId) return;
      const completed = preview;
      generatedDocument = completed;
      documentRef.current = completed;
      setPast((items) => [...items.slice(-99), base]);
      setFuture([]);
      setLoadState({ kind: 'ready', document: completed, needsMigration: false });
      await persist(completed);
      setPrompt('');
    } catch (error: unknown) {
      if (generationRunRef.current === runId) {
        if (generatedDocument === null) {
          documentRef.current = base;
          setLoadState({ kind: 'ready', document: base, needsMigration: false });
          const details = localServiceErrorDetails(error) ?? readSafeDesignGenerationError(error);
          toast.error(details ? `${t('designEditorGenerationFailed')}\n${details}` : t('designEditorGenerationFailed'), { durationMs: 12_000 });
        } else if (saveFailedRef.current) {
          documentRef.current = generatedDocument;
          setLoadState({ kind: 'ready', document: generatedDocument, needsMigration: false });
        } else {
          documentRef.current = generatedDocument;
          setLoadState({ kind: 'ready', document: generatedDocument, needsMigration: false });
          toast.error(t('designEditorGenerationFailed'));
        }
      }
    } finally {
      if (generationRunRef.current === runId) {
        activeGenerationRef.current = null;
        generationControllerRef.current = null;
        generationBaseRef.current = null;
        setIsGenerating(false);
        setIsAddingGeneratedNodes(false);
        setGenerationCursor(null);
      }
    }
  };

  const addDesignReferenceFiles = async (files: File[]) => {
    if (files.length === 0 || isGenerating || isReadingDesignReferencesRef.current) return;
    if (designReferences.length + files.length > 3) {
      toast.warning(t('designEditorReferenceLimit'));
      return;
    }
    const selectedBytes = designReferences.reduce((total, attachment) => total + attachment.sizeBytes, 0);
    const incomingBytes = files.reduce((total, file) => total + file.size, 0);
    if (files.some((file) => file.size > 10 * 1024 * 1024) || selectedBytes + incomingBytes > 14 * 1024 * 1024) {
      toast.warning(t('designEditorReferenceTooLarge'));
      return;
    }
    isReadingDesignReferencesRef.current = true;
    setIsReadingDesignReferences(true);
    try {
      const attachments = await Promise.all(files.map(readAttachmentFile));
      if (attachments.some((attachment) => attachment.mimeType !== 'image/png' && attachment.mimeType !== 'image/jpeg' && attachment.mimeType !== 'image/webp')) {
        toast.warning(t('attachmentUnsupportedType'));
        return;
      }
      setDesignReferences((current) => [
        ...current,
        ...attachments.map((attachment) => ({ ...attachment, id: crypto.randomUUID(), previewUrl: URL.createObjectURL(attachment.file) })),
      ]);
    } catch (error) {
      toast.error(t(error instanceof ChatAttachmentInputError ? error.code : 'attachmentReadFailed'));
    } finally {
      isReadingDesignReferencesRef.current = false;
      setIsReadingDesignReferences(false);
    }
  };

  const removeDesignReference = (id: string) => {
    setDesignReferences((current) => {
      const attachment = current.find((item) => item.id === id);
      if (attachment?.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
      return current.filter((item) => item.id !== id);
    });
  };

  const addNode = (type: DesignNodeTool) => {
    const current = documentRef.current;
    if (current.nodes.length >= maximumDesignNodeCount) {
      toast.warning(t('designEditorNodeLimit', { count: maximumDesignNodeCount }));
      return;
    }
    const point = screenToCanvas(160, 120, cameraRef.current);
    const node = createDesignNode(type, point.x, point.y, t(designNodeToolLabelKeys[type]), t('designEditorDefaultText'), current.activePageId);
    void publish({ ...current, nodes: [...current.nodes, node], selectedNodeIds: [node.id] });
  };

  const selectAndAddNode = (type: DesignNodeTool) => {
    setSelectedNodeTool(type);
    addNode(type);
  };

  const undo = () => {
    const previous = past.at(-1);
    if (!previous) return;
    setPast(past.slice(0, -1));
    setFuture([...future, documentRef.current]);
    documentRef.current = previous;
    setLoadState({ kind: 'ready', document: previous, needsMigration: false });
    void persist(previous);
  };

  const redo = () => {
    const next = future.at(-1);
    if (!next) return;
    setFuture(future.slice(0, -1));
    setPast([...past, documentRef.current]);
    documentRef.current = next;
    setLoadState({ kind: 'ready', document: next, needsMigration: false });
    void persist(next);
  };

  const removeSelected = () => {
    const current = documentRef.current;
    if (current.selectedNodeIds.length === 0) return;
    const selected = new Set(current.selectedNodeIds);
    let hasNewDescendant = true;
    while (hasNewDescendant) {
      hasNewDescendant = false;
      for (const node of current.nodes) {
        if (node.parentId !== null && selected.has(node.parentId) && !selected.has(node.id)) {
          selected.add(node.id);
          hasNewDescendant = true;
        }
      }
    }
    void publish(removeDesignNodes(current, selected));
  };

  const activatePrototypeNode = (node: DesignNode, trigger: DesignPrototypeInteraction['trigger'] = 'click') => {
    const interaction = documentRef.current.prototypeInteractions.find((item) => item.sourceNodeId === node.id);
    if (interaction?.trigger !== trigger) return;
    const target = interaction && documentRef.current.nodes.find((item) => item.id === interaction.targetNodeId);
    if (!target || !interaction) return;
    setPrototypeTransition(interaction.transition);
    if (interaction.presentation === 'overlay') {
      setPrototypeOverlayId(target.id);
      return;
    }
    setPrototypeOverlayId(null);
    if (prototypeTransitionFrameRef.current !== null) window.cancelAnimationFrame(prototypeTransitionFrameRef.current);
    if (interaction.transition === 'dissolve') {
      setIsPrototypeDissolveVisible(false);
      prototypeTransitionFrameRef.current = window.requestAnimationFrame(() => {
        focusCanvasNode(target);
        setIsPrototypeDissolveVisible(true);
        prototypeTransitionFrameRef.current = null;
      });
      return;
    }
    focusCanvasNode(target);
  };

  const startPrototypePreview = () => {
    const current = documentRef.current;
    const pageFrames = current.nodes.filter((node): node is DesignFrameNode => node.pageId === current.activePageId && node.type === 'frame');
    const selectedFrame = pageFrames.find((node) => current.selectedNodeIds.includes(node.id));
    const flow = current.prototypeFlows.find((item) => item.pageId === current.activePageId);
    const startFrame = selectedFrame ?? pageFrames.find((node) => node.id === flow?.startNodeId) ?? pageFrames[0];
    if (!startFrame) {
      toast.info(t('designEditorPrototypeNeedsFrame'));
      return;
    }
    try {
      const hasFlow = current.prototypeFlows.some((item) => item.startNodeId === startFrame.id);
      if (!hasFlow) {
        const name = nextUniqueDesignName(t('designEditorPrototypeFlowName'), current.prototypeFlows);
        void publish(setDesignPrototypeFlow(current, name, startFrame.id));
      }
      setPrototypeStartNodeId(startFrame.id);
      setPrototypeOverlayId(null);
      setIsPrototypeDissolveVisible(true);
      setIsPrototypePreview(true);
      focusCanvasNode(startFrame);
    } catch {
      toast.error(t('designEditorPrototypePreviewFailed'));
    }
  };

  const stopPrototypePreview = () => {
    const startFrame = prototypeStartNodeId ? documentRef.current.nodes.find((node) => node.id === prototypeStartNodeId) : undefined;
    setIsPrototypePreview(false);
    setPrototypeOverlayId(null);
    setPrototypeStartNodeId(null);
    if (startFrame) focusCanvasNode(startFrame);
  };

  const savePrototypeConnection = () => {
    if (!selectedNode || !prototypeDraft.targetNodeId) return;
    try {
      void publish(setDesignPrototypeInteraction(
        documentRef.current,
        selectedNode.id,
        prototypeDraft.targetNodeId,
        prototypeDraft.transition,
        prototypeDraft.trigger,
        prototypeDraft.presentation,
      )).then(() => toast.success(t('designEditorPrototypeConnectionSaved')), () => undefined);
    } catch {
      toast.error(t('designEditorPrototypeConnectionFailed'));
    }
  };

  const removePrototypeConnection = () => {
    if (!selectedNode) return;
    try {
      void publish(removeDesignPrototypeInteraction(documentRef.current, selectedNode.id));
      setPrototypeDraft({ targetNodeId: '', trigger: 'click', presentation: 'navigate', transition: 'dissolve' });
    } catch {
      toast.error(t('designEditorPrototypeConnectionFailed'));
    }
  };

  const focusCanvasNode = (node: DesignNode) => {
    setCamera((current) => ({
      ...current,
      x: canvasViewport.width / 2 - (node.x + node.width / 2) * current.scale,
      y: canvasViewport.height / 2 - (node.y + node.height / 2) * current.scale,
    }));
  };

  const placeCommentAt = (clientX: number, clientY: number, nodeId?: string) => {
    if (!isCommentPlacementActive) return false;
    const current = documentRef.current;
    const clicked = nodeId ? current.nodes.find((node) => node.id === nodeId) : undefined;
    let frame = clicked?.type === 'frame' ? clicked : undefined;
    let parentId = clicked?.parentId ?? null;
    while (!frame && parentId !== null) {
      const parent = current.nodes.find((node) => node.id === parentId);
      if (!parent) break;
      if (parent.type === 'frame') frame = parent;
      parentId = parent.parentId;
    }
    frame ??= current.nodes.find((node): node is Extract<DesignNode, { type: 'frame' }> => node.type === 'frame' && current.selectedNodeIds.includes(node.id));
    const bounds = canvasRef.current?.getBoundingClientRect();
    if (frame?.type !== 'frame' || !bounds) {
      toast.info(t('designEditorCommentNeedsFrame'));
      setIsCommentPlacementActive(false);
      return true;
    }
    const worldPoint = screenToCanvas(clientX - bounds.left, clientY - bounds.top, cameraRef.current);
    setCommentComposer({
      frameId: frame.id,
      point: {
        x: clamp((worldPoint.x - frame.x) / frame.width, 0, 1),
        y: clamp((worldPoint.y - frame.y) / frame.height, 0, 1),
      },
    });
    setCommentDraft('');
    setIsCommentPlacementActive(false);
    return true;
  };

  const submitComment = () => {
    if (!commentComposer) return;
    try {
      const next = addDesignComment(documentRef.current, commentComposer.frameId, commentDraft, commentComposer.point);
      const addedComment = next.comments.at(-1);
      void publish(next);
      setCommentComposer(null);
      setCommentDraft('');
      if (addedComment) setSelectedCommentId(addedComment.id);
      toast.success(t('designEditorCommentAdded'));
    } catch {
      toast.error(t('designEditorCommentFailed'));
    }
  };

  const toggleCommentResolution = (commentId: string, resolved: boolean) => {
    try {
      void publish(setDesignCommentResolved(documentRef.current, commentId, resolved));
    } catch {
      toast.error(t('designEditorCommentFailed'));
    }
  };

  const deleteComment = (commentId: string) => {
    try {
      void publish(removeDesignComment(documentRef.current, commentId));
      setSelectedCommentId(null);
    } catch {
      toast.error(t('designEditorCommentFailed'));
    }
  };

  const saveNodeName = (nodeId: string) => {
    try {
      void publish(renameDesignNode(documentRef.current, nodeId, nodeNameDraft));
      setRenamingNodeId(null);
      setNodeNameDraft('');
    } catch {
      toast.error(t('designEditorInvalidLayerName'));
    }
  };

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const target = event.target;
    const isEditing = target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA'].includes(target.tagName));
    if (isEditing) return;
    const isHistoryShortcut = (event.ctrlKey || event.metaKey) && ['z', 'y'].includes(event.key.toLowerCase());
    const isMovementKey = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key);
    if (responsivePreview !== 'base' && !isHistoryShortcut && event.key !== 'Escape' && event.key !== 'F2' && !isMovementKey) return;
    if (event.key === 'F2') {
      const selectedNodeId = documentRef.current.selectedNodeIds[0];
      const selectedNode = documentRef.current.nodes.find((node) => node.id === selectedNodeId);
      if (documentRef.current.selectedNodeIds.length !== 1 || !selectedNode) return;
      event.preventDefault();
      setNodeNameDraft(selectedNode.name);
      setRenamingNodeId(selectedNode.id);
    } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
      event.preventDefault();
      if (event.shiftKey) redo();
      else undo();
    } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'y') {
      event.preventDefault();
      redo();
    } else if (event.key === 'Delete' || event.key === 'Backspace') {
      event.preventDefault();
      removeSelected();
    } else if (event.shiftKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
      const current = documentRef.current;
      if (current.selectedNodeIds.length === 0) return;
      event.preventDefault();
      const step = event.key === 'ArrowRight' ? 15 : -15;
      const selected = new Set(current.selectedNodeIds);
      void publish({
        ...current,
        nodes: current.nodes.map((node) => selected.has(node.id)
          ? { ...node, rotation: ((node.rotation + step) % 360 + 360) % 360 }
          : node),
      });
    } else if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) {
      const current = documentRef.current;
      if (current.selectedNodeIds.length === 0) return;
      event.preventDefault();
      const amount = event.altKey ? 10 : 1;
      const delta = {
        x: event.key === 'ArrowLeft' ? -amount : event.key === 'ArrowRight' ? amount : 0,
        y: event.key === 'ArrowUp' ? -amount : event.key === 'ArrowDown' ? amount : 0,
      };
      const selected = new Set(current.selectedNodeIds);
      if (responsivePreview !== 'base') {
        const displayedNodes = new Map(designNodesForResponsiveBreakpoint(current.nodes, responsivePreview).map((node) => [node.id, node]));
        void publish({
          ...current,
          nodes: current.nodes.map((node) => {
            if (!selected.has(node.id) || node.locked) return node;
            const displayed = displayedNodes.get(node.id);
            if (!displayed) return node;
            return {
              ...node,
              responsiveOverrides: {
                ...node.responsiveOverrides,
                [responsivePreview]: { x: displayed.x + delta.x, y: displayed.y + delta.y, width: displayed.width, height: displayed.height },
              },
            };
          }),
        });
        return;
      }
      void publish({
        ...current,
        nodes: current.nodes.map((node) => selected.has(node.id)
          ? { ...node, x: node.x + delta.x, y: node.y + delta.y }
          : node),
      });
    } else if (event.key === 'Escape' && prototypeOverlayId) {
      event.preventDefault();
      setPrototypeOverlayId(null);
    } else if (event.key === 'Escape' && isPrototypePreview) {
      event.preventDefault();
      stopPrototypePreview();
    } else if (event.key === 'Escape') {
      const current = documentRef.current;
      void publish({ ...current, selectedNodeIds: [] }, false);
    }
  };

  const onCanvasPointerDown = (event: ReactPointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    if (placeCommentAt(event.clientX, event.clientY)) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (event.target instanceof Element && event.target.closest('.design-editor-node, .design-editor-floating-dock')) return;
    lastNodePointerDownRef.current = null;
    event.currentTarget.setPointerCapture(event.pointerId);
    setLoadState((current) => {
      if (current.kind !== 'ready' || current.document.selectedNodeIds.length === 0) return current;
      const next = { ...current.document, selectedNodeIds: [] };
      documentRef.current = next;
      return { ...current, document: next };
    });
    const start = cameraRef.current;
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX - start.x,
      startY: event.clientY - start.y,
      nodes: [],
      didMove: false,
    };
  };

  const onNodePointerDown = (event: ReactPointerEvent<HTMLButtonElement>, node: DesignNode) => {
    if (event.button !== 0) return;
    if (placeCommentAt(event.clientX, event.clientY, node.id)) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (node.locked) return;
    const now = Date.now();
    const previousClick = lastNodePointerDownRef.current;
    if (node.type === 'text' && event.detail > 1 && previousClick?.nodeId === node.id) {
      event.preventDefault();
      event.stopPropagation();
      startTextEditing(node.id);
      return;
    }
    const currentNode = documentRef.current.nodes.find((entry) => entry.id === node.id);
    if (node.type === 'text' && currentNode?.type === 'text') lastNodePointerDownRef.current = { nodeId: node.id, at: now, x: currentNode.x, y: currentNode.y };
    else lastNodePointerDownRef.current = null;
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    const target = event.target;
    const handleValue = target instanceof Element
      ? target.closest<HTMLElement>('[data-resize-handle]')?.dataset.resizeHandle
      : undefined;
    const resizeHandle = isResizeHandle(handleValue) ? handleValue : undefined;
    const additive = event.shiftKey;
    const current = documentRef.current;
    const selectedIds = resizeHandle
      ? [node.id]
      : additive
      ? current.selectedNodeIds.includes(node.id)
        ? current.selectedNodeIds
        : [...current.selectedNodeIds, node.id]
      : current.selectedNodeIds.includes(node.id)
        ? current.selectedNodeIds
        : [node.id];
    const next = { ...current, selectedNodeIds: selectedIds };
    documentRef.current = next;
    setLoadState({ kind: 'ready', document: next, needsMigration: false });
    const responsiveBreakpoint = responsivePreview === 'base' ? undefined : responsivePreview;
    const expandedSelection = resizeHandle
      ? current.nodes.filter((entry) => selectedIds.includes(entry.id))
      : expandDesignMoveSelection(current, selectedIds);
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      nodes: responsiveBreakpoint
        ? designNodesForResponsiveBreakpoint(expandedSelection, responsiveBreakpoint)
        : expandedSelection,
      didMove: false,
      beforeDocument: next,
      ...(responsiveBreakpoint ? { responsiveBreakpoint } : {}),
      ...(resizeHandle ? { resizeHandle } : {}),
      ...(resizeHandle && node.type === 'frame'
        ? { constraintChildren: responsiveBreakpoint
            ? designNodesForResponsiveBreakpoint(current.nodes.filter((entry) => entry.parentId === node.id), responsiveBreakpoint)
            : current.nodes.filter((entry) => entry.parentId === node.id) }
        : {}),
    };
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLElement | HTMLButtonElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (!drag.didMove && Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) < 4) return;
    drag.didMove = true;
    const isCanvas = event.currentTarget === canvasRef.current;
    if (drag.nodes.length === 0 && isCanvas) {
      setCamera((current) => ({
        ...current,
        x: event.clientX - drag.startX,
        y: event.clientY - drag.startY,
      }));
      return;
    }
    if (drag.nodes.length === 0) return;
    const scale = cameraRef.current.scale;
    const rawDeltaX = (event.clientX - drag.startX) / scale;
    const rawDeltaY = (event.clientY - drag.startY) / scale;
    let deltaX = event.altKey ? rawDeltaX : Math.round(rawDeltaX / 8) * 8;
    let deltaY = event.altKey ? rawDeltaY : Math.round(rawDeltaY / 8) * 8;
    if (!drag.resizeHandle && !event.altKey) {
      const currentDocument = documentRef.current;
      const targets = designNodesForResponsiveBreakpoint(
        currentDocument.nodes.filter((node) => node.pageId === currentDocument.activePageId),
        drag.responsiveBreakpoint ?? null,
      );
      const snap = snapDesignMove(drag.nodes, targets, deltaX, deltaY, 6 / scale);
      deltaX = snap.deltaX;
      deltaY = snap.deltaY;
      setSnapGuides(snap.guides);
    } else {
      setSnapGuides({ vertical: null, horizontal: null });
    }
    const movedNodes = drag.nodes.map((original) => {
      if (!drag.resizeHandle) return { ...original, x: original.x + deltaX, y: original.y + deltaY };
      const movesWest = drag.resizeHandle.endsWith('west');
      const movesNorth = drag.resizeHandle.startsWith('north');
      const width = Math.max(24, original.width + (movesWest ? -deltaX : deltaX));
      const height = Math.max(24, original.height + (movesNorth ? -deltaY : deltaY));
      return {
        ...original,
        x: movesWest ? original.x + original.width - width : original.x,
        y: movesNorth ? original.y + original.height - height : original.y,
        width,
        height,
      };
    });
    const movedById = new Map(movedNodes.map((node) => [node.id, node]));
    let next: DesignDocument = {
      ...documentRef.current,
      nodes: documentRef.current.nodes.map((node) => {
        const moved = movedById.get(node.id);
        if (!moved) return node;
        if (drag.responsiveBreakpoint) {
          return {
            ...node,
            responsiveOverrides: {
              ...node.responsiveOverrides,
              [drag.responsiveBreakpoint]: { x: moved.x, y: moved.y, width: moved.width, height: moved.height },
            },
          };
        }
        return { ...node, x: moved.x, y: moved.y, width: moved.width, height: moved.height };
      }),
    };
    if (drag.resizeHandle && !drag.responsiveBreakpoint) {
      for (const original of drag.nodes) {
        if (original.type !== 'frame') continue;
        if (original.autoLayout) next = reflowDesignFrameChildren(next, original.id);
        else if (drag.constraintChildren) next = reflowDesignFrameConstraints(next, original, drag.constraintChildren);
      }
    } else if (drag.resizeHandle && drag.responsiveBreakpoint) {
      const responsiveBreakpoint = drag.responsiveBreakpoint;
      const reflowSource: DesignDocument = {
        ...next,
        nodes: designNodesForResponsiveBreakpoint(next.nodes, responsiveBreakpoint),
      };
      let reflowed = reflowSource;
      for (const original of drag.nodes) {
        if (original.type !== 'frame') continue;
        if (original.autoLayout) reflowed = reflowDesignFrameChildren(reflowed, original.id);
        else if (drag.constraintChildren) reflowed = reflowDesignFrameConstraints(reflowed, original, drag.constraintChildren);
      }
      const reflowedById = new Map(reflowed.nodes.map((node) => [node.id, node]));
      next = {
        ...next,
        nodes: next.nodes.map((node) => {
          const displayed = reflowedById.get(node.id);
          if (!displayed) return node;
          const currentDisplayed = designNodesForResponsiveBreakpoint([node], responsiveBreakpoint)[0];
          if (!currentDisplayed || (displayed.x === currentDisplayed.x && displayed.y === currentDisplayed.y && displayed.width === currentDisplayed.width && displayed.height === currentDisplayed.height)) return node;
          return {
            ...node,
            responsiveOverrides: {
              ...node.responsiveOverrides,
              [responsiveBreakpoint]: { x: displayed.x, y: displayed.y, width: displayed.width, height: displayed.height },
            },
          };
        }),
      };
    }
    if (movedById.size > 0) {
      documentRef.current = next;
      setLoadState({ kind: 'ready', document: next, needsMigration: false });
    }
  };

  const finishPointer = () => {
    const drag = dragRef.current;
    dragRef.current = null;
    setSnapGuides({ vertical: null, horizontal: null });
    if (drag?.nodes.length) {
      const visibleNodes = drag.responsiveBreakpoint
        ? designNodesForResponsiveBreakpoint(documentRef.current.nodes, drag.responsiveBreakpoint)
        : documentRef.current.nodes;
      const moved = drag.nodes.some((node) => {
        const current = visibleNodes.find((entry) => entry.id === node.id);
        return current && (
          current.x !== node.x || current.y !== node.y ||
          current.width !== node.width || current.height !== node.height
        );
      });
      if (moved) {
        const origins = new Map(drag.nodes.map((node) => [node.id, node]));
        const before = drag.beforeDocument ?? {
          ...documentRef.current,
          nodes: documentRef.current.nodes.map((node) => origins.get(node.id) ?? node),
        };
        setPast((items) => [...items.slice(-99), before]);
        setFuture([]);
        void persist(documentRef.current);
      }
    }
  };

  const onWheel = (event: ReactWheelEvent<HTMLElement>) => {
    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    const current = cameraRef.current;
    const scale = clamp(current.scale * (event.deltaY < 0 ? 1.08 : 1 / 1.08), 0.05, 8);
    const pointX = event.clientX - bounds.left;
    const pointY = event.clientY - bounds.top;
    const worldX = (pointX - current.x) / current.scale;
    const worldY = (pointY - current.y) / current.scale;
    setCamera({ x: pointX - worldX * scale, y: pointY - worldY * scale, scale });
  };

  const exportRecoveryData = () => {
    if (loadState.kind !== 'recoveryRequired') return;
    let url: string | null = null;
    try {
      url = URL.createObjectURL(new Blob([loadState.rawData], { type: 'application/json;charset=utf-8' }));
      const anchor = window.document.createElement('a');
      anchor.href = url;
      anchor.download = 'OpenChat-Design-recovery.json';
      window.document.body.append(anchor);
      anchor.click();
      anchor.remove();
      URL.revokeObjectURL(url);
    } catch {
      if (url) URL.revokeObjectURL(url);
      toast.error(t('designEditorRecoveryExportFailed'));
    }
  };

  const exportDocumentSvg = async (targetDocument = loadState.kind === 'ready' ? loadState.document : emptyDocumentFallback) => {
    const assetUrls = new Map<string, string>();
    const assets = designDocumentImageAssetsForExport(targetDocument);
    let totalBytes = 0;
    for (const asset of assets) {
      totalBytes += asset.sizeBytes;
      if (totalBytes > 32 * 1024 * 1024) throw new Error('The selected image assets exceed the export limit.');
      const image = await readDesignImage(asset.assetId, asset.mimeType, asset.sizeBytes);
      assetUrls.set(asset.assetId, `data:${image.mimeType};base64,${encodeBase64(image.bytes)}`);
    }
    return exportDesignDocumentSvg(targetDocument, assetUrls);
  };

  const captureDesignFrameScreenshot = async (targetDocument: DesignDocument, frameId: string): Promise<Uint8Array> => {
    const frame = targetDocument.nodes.find((node) => node.id === frameId && node.type === 'frame');
    if (!frame) throw new Error('The requested frame is unavailable.');
    const frameDocument = { ...targetDocument, activePageId: frame.pageId, selectedNodeIds: [frame.id] };
    const assetUrls = new Map<string, string>();
    const assets = designDocumentImageAssetsForExport(frameDocument);
    let totalBytes = 0;
    for (const asset of assets) {
      totalBytes += asset.sizeBytes;
      if (totalBytes > 32 * 1024 * 1024) throw new Error('The frame image assets exceed the screenshot limit.');
      const image = await readDesignImage(asset.assetId, asset.mimeType, asset.sizeBytes);
      assetUrls.set(asset.assetId, `data:${image.mimeType};base64,${encodeBase64(image.bytes)}`);
    }
    const svg = exportDesignDocumentSvg(frameDocument, assetUrls);
    const openingTag = /^<svg\b[^>]*>/.exec(svg)?.[0];
    if (!openingTag) throw new Error('The frame screenshot could not be rendered.');
    const croppedSvg = svg.replace(openingTag, `<svg xmlns="http://www.w3.org/2000/svg" width="${frame.width}" height="${frame.height}" viewBox="${frame.x} ${frame.y} ${frame.width} ${frame.height}">`);
    const scale = Math.min(1, 2048 / frame.width, 2048 / frame.height);
    let bitmap: ImageBitmap | null = null;
    let canvas: HTMLCanvasElement | null = null;
    try {
      bitmap = await createImageBitmap(new Blob([croppedSvg], { type: 'image/svg+xml;charset=utf-8' }), {
        resizeWidth: Math.max(1, Math.floor(frame.width * scale)),
        resizeHeight: Math.max(1, Math.floor(frame.height * scale)),
        resizeQuality: 'high',
      });
      canvas = window.document.createElement('canvas');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Canvas drawing is unavailable.');
      context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
      const blob = await new Promise<Blob>((resolve, reject) => {
        canvas?.toBlob((result) => result ? resolve(result) : reject(new Error('Frame screenshot encoding failed.')), 'image/png');
      });
      if (blob.size > 10 * 1024 * 1024) throw new Error('The frame screenshot is too large to review.');
      return new Uint8Array(await blob.arrayBuffer());
    } finally {
      bitmap?.close();
      if (canvas) {
        canvas.width = 0;
        canvas.height = 0;
      }
    }
  };

  const exportDocument = async (targetDocument?: DesignDocument) => {
    const documentToExport = targetDocument ?? (loadState.kind === 'ready' ? loadState.document : null);
    if (!documentToExport) return;
    try {
      const svg = await exportDocumentSvg(documentToExport);
      downloadDesignBlob(new Blob([svg], { type: 'image/svg+xml;charset=utf-8' }), designExportFilename(documentToExport.name, 'svg'));
    } catch {
      toast.error(t('designEditorExportFailed'));
    }
  };

  const exportDocumentCode = async (targetDocument?: DesignDocument) => {
    const documentToExport = targetDocument ?? (loadState.kind === 'ready' ? loadState.document : null);
    if (!documentToExport) return;
    try {
      const assetUrls = new Map<string, string>();
      const assets = designDocumentImageAssetsForExport(documentToExport);
      let totalBytes = 0;
      for (const asset of assets) {
        totalBytes += asset.sizeBytes;
        if (totalBytes > 32 * 1024 * 1024) throw new Error('The selected image assets exceed the export limit.');
        const image = await readDesignImage(asset.assetId, asset.mimeType, asset.sizeBytes);
        assetUrls.set(asset.assetId, `data:${image.mimeType};base64,${encodeBase64(image.bytes)}`);
      }
      const html = exportDesignDocumentHtml(documentToExport, assetUrls);
      downloadDesignBlob(new Blob([html], { type: 'text/html;charset=utf-8' }), designExportFilename(documentToExport.name, 'html'));
    } catch {
      toast.error(t('designEditorExportFailed'));
    }
  };

  const exportDocumentArchive = async (targetDocument?: DesignDocument) => {
    const documentToExport = targetDocument ?? (loadState.kind === 'ready' ? loadState.document : null);
    if (!documentToExport) return;
    try {
      const assets = designDocumentImageAssetsForExport(documentToExport);
      const imageBytes = new Map<string, Uint8Array>();
      let totalBytes = 0;
      for (const asset of assets) {
        totalBytes += asset.sizeBytes;
        if (totalBytes > 16 * 1024 * 1024) throw new Error('The Design document image assets exceed the export limit.');
        const image = await readDesignImage(asset.assetId, asset.mimeType, asset.sizeBytes);
        imageBytes.set(asset.assetId, image.bytes);
      }
      const archive = encodeDesignDocumentArchive(documentToExport, imageBytes);
      downloadDesignBlob(new Blob([archive], { type: 'application/vnd.openchat.design+json;charset=utf-8' }), designExportFilename(documentToExport.name, 'ocdesign'));
    } catch {
      toast.error(t('designEditorArchiveExportFailed'));
    }
  };

  const exportDocumentPng = async (targetDocument?: DesignDocument) => {
    const documentToExport = targetDocument ?? (loadState.kind === 'ready' ? loadState.document : null);
    if (!documentToExport) return;
    let bitmap: ImageBitmap | null = null;
    let canvas: HTMLCanvasElement | null = null;
    try {
      const svg = await exportDocumentSvg(documentToExport);
      const dimensions = readDesignSvgDimensions(svg);
      const scale = Math.min(1, 4096 / dimensions.width, 4096 / dimensions.height);
      bitmap = await createImageBitmap(new Blob([svg], { type: 'image/svg+xml;charset=utf-8' }), {
        resizeWidth: Math.max(1, Math.floor(dimensions.width * scale)),
        resizeHeight: Math.max(1, Math.floor(dimensions.height * scale)),
        resizeQuality: 'high',
      });
      canvas = window.document.createElement('canvas');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Canvas drawing is unavailable.');
      context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
      const blob = await new Promise<Blob>((resolve, reject) => {
        canvas?.toBlob((result) => result ? resolve(result) : reject(new Error('PNG encoding failed.')), 'image/png');
      });
      downloadDesignBlob(blob, designExportFilename(documentToExport.name, 'png'));
    } catch {
      toast.error(t('designEditorExportFailed'));
    } finally {
      bitmap?.close();
      if (canvas) {
        canvas.width = 0;
        canvas.height = 0;
      }
    }
  };

  const exportDocumentPdf = async (targetDocument?: DesignDocument) => {
    const documentToExport = targetDocument ?? (loadState.kind === 'ready' ? loadState.document : null);
    if (!documentToExport) return;
    let bitmap: ImageBitmap | null = null;
    let canvas: HTMLCanvasElement | null = null;
    try {
      const svg = await exportDocumentSvg(documentToExport);
      const dimensions = readDesignSvgDimensions(svg);
      const scale = Math.min(1, 4096 / dimensions.width, 4096 / dimensions.height);
      bitmap = await createImageBitmap(new Blob([svg], { type: 'image/svg+xml;charset=utf-8' }), {
        resizeWidth: Math.max(1, Math.floor(dimensions.width * scale)),
        resizeHeight: Math.max(1, Math.floor(dimensions.height * scale)),
        resizeQuality: 'high',
      });
      canvas = window.document.createElement('canvas');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Canvas drawing is unavailable.');
      context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
      const blob = await new Promise<Blob>((resolve, reject) => {
        canvas?.toBlob((result) => result ? resolve(result) : reject(new Error('JPEG encoding failed.')), 'image/jpeg', 0.92);
      });
      const pdf = createDesignPdf(new Uint8Array(await blob.arrayBuffer()), canvas.width, canvas.height);
      const pdfBytes = new ArrayBuffer(pdf.byteLength);
      new Uint8Array(pdfBytes).set(pdf);
      downloadDesignBlob(new Blob([pdfBytes], { type: 'application/pdf' }), designExportFilename(documentToExport.name, 'pdf'));
    } catch {
      toast.error(t('designEditorExportFailed'));
    } finally {
      bitmap?.close();
      if (canvas) {
        canvas.width = 0;
        canvas.height = 0;
      }
    }
  };

  if (loadState.kind !== 'ready') {
    return (
      <div className="grid h-full place-items-center bg-background p-6">
        <section className="grid max-w-lg gap-4 text-center" aria-labelledby="design-editor-error-title">
          <h1 className="text-section-title font-semibold" id="design-editor-error-title">{t('designEditorTitle')}</h1>
          {loadState.kind === 'loading' ? (
            <p className="text-sm text-muted-foreground" role="status">{t('commandPaletteLoading')}</p>
          ) : (
            <p className="text-sm text-destructive" role="alert">{t(loadState.kind === 'unavailable' ? 'designEditorLoadFailed' : 'designEditorRecoveryRequired')}</p>
          )}
          {loadState.kind === 'recoveryRequired' && (
            <Button onClick={exportRecoveryData} variant="secondary"><Download aria-hidden="true" size={16} />{t('designEditorExportRecovery')}</Button>
          )}
          {loadState.kind !== 'loading' && <Button onClick={() => {
            setLoadState({ kind: 'loading' });
            setLoadAttempt((attempt) => attempt + 1);
          }} variant="secondary">{t('retry')}</Button>}
        </section>
      </div>
    );
  }

  const document = loadState.document;
  const documentsInSelectedProject = documents.filter((item) => item.projectId === document.projectId);
  const pageNodes = document.nodes.filter((node) => node.pageId === document.activePageId);
  const displayedPageNodes = designNodesForResponsiveBreakpoint(pageNodes, responsivePreview === 'base' ? null : responsivePreview);
  const visibleNodes = displayedPageNodes.filter((node) => node.visible && isNodeInViewport(node, camera, canvasViewport));
  const pageComments = document.comments.filter((comment) => comment.pageId === document.activePageId);
  const selectedComment = pageComments.find((comment) => comment.id === selectedCommentId);
  const variablesById = new Map(document.variables.map((variable) => [variable.id, variable.value]));
  const prototypeOverlayFrame = prototypeOverlayId === null ? undefined : pageNodes.find((node) => node.id === prototypeOverlayId && node.type === 'frame');
  const prototypeOverlayNodes = (() => {
    if (prototypeOverlayFrame?.type !== 'frame') return [];
    const included = new Set([prototypeOverlayFrame.id]);
    let foundDescendant = true;
    while (foundDescendant) {
      foundDescendant = false;
      for (const node of pageNodes) {
        if (node.parentId !== null && included.has(node.parentId) && !included.has(node.id)) {
          included.add(node.id);
          foundDescendant = true;
        }
      }
    }
    return pageNodes.filter((node) => included.has(node.id));
  })();
  const prototypeOverlayContentHeight = prototypeOverlayFrame?.type === 'frame'
    ? Math.max(prototypeOverlayFrame.height, ...prototypeOverlayNodes.map((node) => node.y - prototypeOverlayFrame.y + node.height))
    : 0;

  return (
    <section aria-labelledby="design-canvas-title" className="design-vector-editor" onKeyDown={onKeyDown}>
      <h1 className="sr-only" id="design-canvas-title">{t('designEditorTitle')}</h1>
      <div className={`design-vector-layout${isResizingInspector ? ' is-resizing-inspector' : ''}`} ref={layoutRef}>
        <aside aria-label={t('designEditorSidebar')} className="design-vector-controls">
          <div className="design-editor-sidebar-content">
            <div className="design-editor-document-controls">
              <div className="design-editor-document-switcher" ref={documentSwitcherRef}>
                {isRenamingDocument ? (
                  <div className="design-editor-document-rename">
                    <Input
                      aria-label={t('designEditorDocument')}
                      autoFocus
                      className="min-w-0"
                      density="compact"
                      maxLength={256}
                      onChange={(event) => setDocumentNameDraft(event.currentTarget.value)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') saveDocumentName();
                        if (event.key === 'Escape') setIsRenamingDocument(false);
                      }}
                      value={documentNameDraft}
                    />
                    <Button aria-label={t('save')} disabled={!documentNameDraft.trim()} onClick={saveDocumentName} size="icon" title={t('save')} variant="ghost"><Save aria-hidden="true" size={14} /></Button>
                    <Button aria-label={t('cancel')} onClick={() => setIsRenamingDocument(false)} size="icon" title={t('cancel')} variant="ghost"><X aria-hidden="true" size={14} /></Button>
                  </div>
                ) : (
                  <button
                    className="design-editor-document-name"
                    disabled={isGenerating || isSwitchingDocument}
                    onClick={() => { setDocumentNameDraft(document.name); setIsRenamingDocument(true); }}
                    title={t('designEditorDocument')}
                    type="button"
                  >
                    <FileText aria-hidden="true" size={15} />
                    <span className="truncate">{document.name || t('designEditorTitle')}</span>
                  </button>
                )}
                <DropdownMenu onOpenChange={(open) => {
                  if (!open) {
                    setDocumentMenuWidth(null);
                    return;
                  }
                  const switcherBounds = documentSwitcherRef.current?.getBoundingClientRect();
                  const triggerBounds = documentMenuTriggerRef.current?.getBoundingClientRect();
                  if (switcherBounds && triggerBounds) {
                    setDocumentMenuWidth(Math.max(0, triggerBounds.right - switcherBounds.left));
                  }
                }}>
                  <DropdownMenuTrigger asChild>
                    <Button aria-label={t('designEditorDocument')} disabled={isGenerating || isSwitchingDocument} ref={documentMenuTriggerRef} size="icon" title={t('designEditorDocument')} variant="ghost"><ChevronDown aria-hidden="true" size={15} /></Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent
                    align="end"
                    className="design-editor-document-menu"
                    style={documentMenuWidth === null ? undefined : { width: `${documentMenuWidth}px` }}
                  >
                    {documentsInSelectedProject.map((item) => (
                      <DropdownMenuItem key={item.id} onSelect={() => void selectDocument(item.id)}>
                        {item.id === document.id && <Check aria-hidden="true" size={14} />}
                        <span className="truncate">{item.name || t('designEditorTitle')}</span>
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuContent>
                </DropdownMenu>
                <Button aria-label={t('designEditorNewDocument')} disabled={isGenerating || isSwitchingDocument} onClick={() => { setNewDocumentName(''); setIsNewDocumentDialogOpen(true); }} size="icon" title={t('designEditorNewDocument')} variant="secondary"><FilePlus2 aria-hidden="true" size={15} /></Button>
              </div>
            </div>
            <div className="design-editor-sidebar-section-heading design-editor-pages-heading">
              <span>{t('designEditorPages')}</span>
              <Button aria-label={t('designEditorAddPage')} disabled={isGenerating || isSwitchingDocument || document.pages.length >= 100} onClick={addPage} size="icon" title={t('designEditorAddPage')} variant="ghost"><FilePlus2 aria-hidden="true" size={15} /></Button>
            </div>
            <ul aria-label={t('designEditorPages')} className="design-editor-page-list">
              {document.pages.map((page) => (
                <li className="design-editor-page-list-item" key={page.id}>
                  {renamingPageId === page.id ? (
                    <div className="design-editor-page-rename">
                      <Input
                        aria-label={t('designEditorRenamePage')}
                        autoFocus
                        className="min-w-0"
                        density="compact"
                        maxLength={128}
                        onChange={(event) => setPageNameDraft(event.currentTarget.value)}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter') savePageName(page.id);
                          if (event.key === 'Escape') setRenamingPageId(null);
                        }}
                        value={pageNameDraft}
                      />
                      <Button aria-label={t('designEditorSavePageName')} disabled={!pageNameDraft.trim() || pageNameDraft === page.name} onClick={() => savePageName(page.id)} size="icon" title={t('designEditorSavePageName')} variant="ghost"><Save aria-hidden="true" size={14} /></Button>
                      <Button aria-label={t('cancel')} onClick={() => setRenamingPageId(null)} size="icon" title={t('cancel')} variant="ghost"><X aria-hidden="true" size={14} /></Button>
                    </div>
                  ) : (
                    <button
                      aria-current={document.activePageId === page.id ? 'page' : undefined}
                      className="design-editor-page-item"
                      onClick={() => switchPage(page.id)}
                      onDoubleClick={() => { setPageNameDraft(page.name); setRenamingPageId(page.id); }}
                      type="button"
                    >
                      <FileText aria-hidden="true" size={15} />
                      <span className="truncate">{page.name}</span>
                    </button>
                  )}
                  {renamingPageId !== page.id && (
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button aria-label={t('moreOptions')} className="design-editor-page-menu-trigger" disabled={isGenerating || isSwitchingDocument} size="icon" title={t('moreOptions')} variant="ghost"><MoreHorizontal aria-hidden="true" size={15} /></Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onSelect={() => { setPageNameDraft(page.name); setRenamingPageId(page.id); }}><Pencil aria-hidden="true" size={14} />{t('designEditorRenamePage')}</DropdownMenuItem>
                        <DropdownMenuSeparator />
                        <DropdownMenuItem disabled={document.pages.length < 2} onSelect={() => deletePage(page.id)}><Trash2 aria-hidden="true" size={14} />{t('designEditorDeletePage')}</DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  )}
                </li>
              ))}
            </ul>
            <div className="design-editor-sidebar-section-heading is-layer-heading">
              <div className="flex min-w-0 items-center gap-2"><Layers3 aria-hidden="true" size={15} /><span>{t('designEditorLayers')}</span></div>
              <span className="design-editor-layer-count">{pageNodes.length}</span>
            </div>
            <ul className="design-editor-layer-list" aria-label={t('designEditorLayers')}>
              {orderDesignLayerTree(pageNodes).map(({ node, depth }) => (
                <li key={node.id} style={designLayerStyle(depth)}>
                  <div className="design-editor-layer-row">
                    {renamingNodeId === node.id ? (
                      <div className="flex min-w-0 flex-1 items-center gap-1">
                        <Input
                          aria-label={t('designEditorRenameLayer')}
                          autoFocus
                          className="min-w-0"
                          density="compact"
                          maxLength={256}
                          onChange={(event) => setNodeNameDraft(event.target.value)}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter') saveNodeName(node.id);
                            if (event.key === 'Escape') setRenamingNodeId(null);
                          }}
                          value={nodeNameDraft}
                        />
                        <Button aria-label={t('designEditorSaveLayerName')} onClick={() => saveNodeName(node.id)} size="icon" title={t('designEditorSaveLayerName')} variant="ghost"><Check aria-hidden="true" size={15} /></Button>
                      </div>
                    ) : (
                      <button
                        aria-pressed={document.selectedNodeIds.includes(node.id)}
                        className="design-editor-layer"
                        onClick={(event) => selectNode(node.id, event.shiftKey)}
                        type="button"
                      >
                        <DesignLayerIcon type={node.type} />
                        <span className="design-editor-layer-copy">
                          <span className="truncate">{node.name}</span>
                        </span>
                      </button>
                    )}
                    {renamingNodeId !== node.id && (
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button aria-label={t('moreOptions')} className="design-editor-layer-action" size="icon" title={t('moreOptions')} variant="ghost"><MoreHorizontal aria-hidden="true" size={15} /></Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onSelect={() => { setNodeNameDraft(node.name); setRenamingNodeId(node.id); }}><Pencil aria-hidden="true" size={14} />{t('designEditorRenameLayer')}</DropdownMenuItem>
                          <DropdownMenuSeparator />
                          <DropdownMenuItem onSelect={() => updateNodeState(node.id, { visible: !node.visible })}>
                            {node.visible ? <EyeOff aria-hidden="true" size={14} /> : <Eye aria-hidden="true" size={14} />}
                            {t(node.visible ? 'designEditorHideLayer' : 'designEditorShowLayer', { name: node.name })}
                          </DropdownMenuItem>
                          <DropdownMenuItem onSelect={() => updateNodeState(node.id, { locked: !node.locked })}>
                            {node.locked ? <UnlockKeyhole aria-hidden="true" size={14} /> : <LockKeyhole aria-hidden="true" size={14} />}
                            {t(node.locked ? 'designEditorUnlockLayer' : 'designEditorLockLayer', { name: node.name })}
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    )}
                  </div>
                </li>
              ))}
              {pageNodes.length === 0 && <li className="design-editor-empty-layers">{t('designEditorEmptyLayers')}</li>}
            </ul>
          </div>
          {/* biome-ignore lint/a11y/useSemanticElements: Interactive resize separator needs aria-value and keyboard support. */}
          <div
            aria-label={t('resizeSidebar')}
            aria-orientation="vertical"
            aria-valuemax={DESIGN_PANEL_MAX_WIDTH}
            aria-valuemin={DESIGN_PANEL_MIN_WIDTH}
            aria-valuenow={sidebarWidth}
            className="design-vector-resizer group/design-vector-resizer"
            onDoubleClick={() => {
              setSidebarWidth(DESIGN_PANEL_DEFAULT_WIDTH);
              sidebarWidthRef.current = DESIGN_PANEL_DEFAULT_WIDTH;
              void writeDesignPanelWidth(DESIGN_PANEL_DEFAULT_WIDTH).catch(() => toast.error(t('designEditorSaveFailed')));
            }}
            onKeyDown={(event) => resizeSidebarByKey(event, sidebarWidth, setSidebarWidth, sidebarWidthRef, () => toast.error(t('designEditorSaveFailed')))}
            onPointerDown={(event) => {
              if (event.button !== 0) return;
              event.preventDefault();
              resizeStartRef.current = { pointerX: event.clientX, width: sidebarWidthRef.current };
              setIsResizing(true);
            }}
            ref={separatorRef}
            role="separator"
            tabIndex={0}
          >
            <span className={isResizing ? 'design-vector-resizer-line is-active' : 'design-vector-resizer-line'} />
          </div>
        </aside>
        <div className="design-vector-main min-w-0">
          <section
            aria-label={t('designEditorCanvas')}
            className="design-vector-canvas"
            style={{
              backgroundPosition: `${camera.x}px ${camera.y}px`,
              backgroundSize: `${24 * camera.scale}px ${24 * camera.scale}px`,
            }}
            onPointerDown={onCanvasPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={finishPointer}
            onPointerCancel={finishPointer}
            onWheel={onWheel}
            ref={canvasRef}
          >
            <div className={`absolute left-0 top-0 origin-top-left${isPrototypePreview ? ` design-editor-preview-motion is-transition-${prototypeTransition}${isPrototypeDissolveVisible ? '' : ' is-dissolving'}` : ''}`} style={{ transform: `translate(${camera.x}px, ${camera.y}px) scale(${camera.scale})` }}>
              {visibleNodes.filter((node) => !prototypeOverlayNodes.some((overlayNode) => overlayNode.id === node.id)).map((node) => renderNode(node, document.selectedNodeIds.includes(node.id) && !isPrototypePreview, variablesById, imageUrls, onNodePointerDown, onPointerMove, finishPointer, isPrototypePreview, activatePrototypeNode, (hoveredNode) => activatePrototypeNode(hoveredNode, 'hover'), openNodeContextMenu, editingTextNodeId, startTextEditing, (nodeId, content) => {
                updateNode(nodeId, (current) => current.type === 'text' ? { ...current, content } : current);
                setEditingTextNodeId(null);
              }, editingTextRef))}
              {pageComments.map((comment) => {
                const frame = comment.nodeId ? document.nodes.find((node) => node.id === comment.nodeId && node.type === 'frame') : undefined;
                if (frame?.type !== 'frame') return null;
                return (
                  <button
                    aria-label={t('designEditorCommentMarker', { text: comment.text })}
                    aria-pressed={selectedCommentId === comment.id}
                    className={`design-editor-comment-marker${comment.resolved ? ' is-resolved' : ''}`}
                    key={comment.id}
                    onPointerDown={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      setSelectedCommentId(comment.id);
                      setIsCommentsPanelOpen(true);
                    }}
                    style={{ left: frame.x + frame.width * (comment.point?.x ?? 0.5), top: frame.y + frame.height * (comment.point?.y ?? 0.5) }}
                    type="button"
                  >{comment.resolved ? <Check aria-hidden="true" size={13} /> : <MessageSquare aria-hidden="true" size={13} />}</button>
                );
              })}
            </div>
            {snapGuides.vertical !== null && <div aria-hidden="true" className="design-editor-snap-guide is-vertical" style={{ left: camera.x + snapGuides.vertical * camera.scale }} />}
            {snapGuides.horizontal !== null && <div aria-hidden="true" className="design-editor-snap-guide is-horizontal" style={{ top: camera.y + snapGuides.horizontal * camera.scale }} />}
            {isGenerating && generationCursor && <div aria-hidden="true" className="design-editor-generation-cursor" style={{ left: generationCursor.x, top: generationCursor.y }}><MousePointer2 size={22} strokeWidth={2.5} /></div>}
            {prototypeOverlayFrame?.type === 'frame' && (
              <div aria-label={prototypeOverlayFrame.name} aria-modal="true" className="design-editor-prototype-overlay" onKeyDown={(event) => {
                if (event.key !== 'Tab') return;
                const focusable = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])'));
                if (focusable.length === 0) { event.preventDefault(); return; }
                const first = focusable[0];
                const last = focusable[focusable.length - 1];
                if (event.shiftKey && window.document.activeElement === first) { event.preventDefault(); last?.focus(); }
                else if (!event.shiftKey && window.document.activeElement === last) { event.preventDefault(); first?.focus(); }
              }} onPointerDown={(event) => {
                event.stopPropagation();
                if (event.target === event.currentTarget) setPrototypeOverlayId(null);
              }} onWheel={(event) => event.stopPropagation()} role="dialog">
                <div className={`design-editor-prototype-overlay-frame${prototypeTransition === 'instant' ? '' : ` is-transition-${prototypeTransition}`} is-opening`} style={{ width: prototypeOverlayFrame.width, height: prototypeOverlayFrame.height }}>
                  <Button aria-label={t('designEditorPrototypeCloseOverlay')} className="design-editor-prototype-overlay-close" onClick={() => setPrototypeOverlayId(null)} ref={prototypeOverlayCloseRef} size="icon" title={t('designEditorPrototypeCloseOverlay')} variant="secondary"><X aria-hidden="true" size={16} /></Button>
                  <div className={prototypeOverlayFrame.scrollable ? 'design-editor-prototype-overlay-scroll is-scrollable' : 'design-editor-prototype-overlay-scroll'} style={{ width: '100%', height: '100%' }}>
                    <div className="design-editor-prototype-overlay-content" style={{ width: prototypeOverlayFrame.width, height: prototypeOverlayContentHeight }}>
                    {prototypeOverlayNodes.map((sourceNode) => renderNode({ ...sourceNode, x: sourceNode.x - prototypeOverlayFrame.x, y: sourceNode.y - prototypeOverlayFrame.y }, false, variablesById, imageUrls, onNodePointerDown, onPointerMove, finishPointer, true, activatePrototypeNode, (hoveredNode) => activatePrototypeNode(hoveredNode, 'hover'), openNodeContextMenu, null, () => undefined, () => undefined, editingTextRef))}
                    </div>
                  </div>
                </div>
              </div>
            )}
            <div className="design-editor-bottom-controls" onPointerDown={(event) => event.stopPropagation()} onWheel={(event) => event.stopPropagation()}>
              {isCommentsPanelOpen && (
                <section aria-label={t('designEditorComments')} className="design-editor-comments-panel">
                  <div className="flex items-center justify-between gap-2">
                    <h2 className="text-sm font-semibold">{t('designEditorComments')}</h2>
                    <Button aria-label={t('designEditorPlaceComment')} disabled={isPrototypePreview} onClick={() => {
                      setCommentComposer(null);
                      setIsCommentPlacementActive((active) => !active);
                    }} size="small" variant={isCommentPlacementActive ? 'primary' : 'secondary'}>
                      <MessageSquare aria-hidden="true" size={14} />{t(isCommentPlacementActive ? 'designEditorCancelCommentPlacement' : 'designEditorPlaceComment')}
                    </Button>
                  </div>
                  {isCommentPlacementActive && <p className="m-0 text-xs text-secondary-text">{t('designEditorCommentPlacementHint')}</p>}
                  {commentComposer && (
                    <div className="grid gap-2">
                      <Textarea
                        aria-label={t('designEditorCommentDraft')}
                        autoFocus
                        className="design-editor-comment-input"
                        maxLength={2000}
                        onChange={(event) => setCommentDraft(event.currentTarget.value)}
                        onKeyDown={(event) => {
                          if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') submitComment();
                          if (event.key === 'Escape') setCommentComposer(null);
                        }}
                        placeholder={t('designEditorCommentDraftPlaceholder')}
                        rows={3}
                        value={commentDraft}
                      />
                      <div className="flex justify-end gap-2">
                        <Button onClick={() => setCommentComposer(null)} size="small" variant="ghost">{t('cancel')}</Button>
                        <Button disabled={!commentDraft.trim()} onClick={submitComment} size="small" variant="primary">{t('designEditorAddComment')}</Button>
                      </div>
                    </div>
                  )}
                  {pageComments.length === 0 ? (
                    <p className="m-0 text-xs text-secondary-text">{t('designEditorNoComments')}</p>
                  ) : (
                    <ul aria-label={t('designEditorComments')} className="design-editor-comment-list">
                      {pageComments.map((comment) => (
                        <li className={selectedCommentId === comment.id ? 'is-selected' : ''} key={comment.id}>
                          <button className="design-editor-comment-text" onClick={() => {
                            setSelectedCommentId(comment.id);
                            const frame = comment.nodeId ? document.nodes.find((node) => node.id === comment.nodeId) : undefined;
                            if (frame) focusCanvasNode(frame);
                          }} type="button">
                            <span>{comment.text}</span>
                            {comment.resolved && <span className="text-xs text-secondary-text">{t('designEditorCommentResolved')}</span>}
                          </button>
                          <Button aria-label={t(comment.resolved ? 'designEditorReopenComment' : 'designEditorResolveComment')} onClick={() => toggleCommentResolution(comment.id, !comment.resolved)} size="icon" title={t(comment.resolved ? 'designEditorReopenComment' : 'designEditorResolveComment')} variant="ghost"><Check aria-hidden="true" size={14} /></Button>
                          <Button aria-label={t('designEditorDeleteComment')} onClick={() => deleteComment(comment.id)} size="icon" title={t('designEditorDeleteComment')} variant="ghost"><Trash2 aria-hidden="true" size={14} /></Button>
                        </li>
                      ))}
                    </ul>
                  )}
                </section>
              )}
              {selectedComment && !isCommentsPanelOpen && (
                <section aria-live="polite" className="design-editor-selected-comment">
                  <p>{selectedComment.text}</p>
                  <Button onClick={() => toggleCommentResolution(selectedComment.id, !selectedComment.resolved)} size="small" variant="secondary">{t(selectedComment.resolved ? 'designEditorReopenComment' : 'designEditorResolveComment')}</Button>
                  <Button aria-label={t('designEditorDeleteComment')} onClick={() => deleteComment(selectedComment.id)} size="icon" title={t('designEditorDeleteComment')} variant="ghost"><Trash2 aria-hidden="true" size={14} /></Button>
                </section>
              )}
              <div className="design-editor-ai-composer">
                <div className="design-editor-project-row">
                  <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                  <DropdownMenu
                    onOpenChange={setProjectMenuOpen}
                    open={projectMenuOpen}
                  >
                    <DropdownMenuTrigger asChild>
                      <Button
                        aria-label={t('selectProject')}
                        className="openchat-dropdown-trigger openchat-dropdown-trigger--subtle openchat-dropdown-trigger--compact max-w-64 min-w-0 justify-start text-xs"
                        disabled={isGenerating || projectsStatus !== 'ready' || projectListLoadError}
                        title={projects.find((project) => project.id === document.projectId)?.name ?? t('noProjectSelected')}
                        variant="ghost"
                      >
                        <span className="min-w-0 flex-1 truncate text-left">
                          {projects.find((project) => project.id === document.projectId)?.name ?? t('noProjectSelected')}
                        </span>
                        <ChevronDown aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start" className="composer-project-menu p-1.5" side="top" sideOffset={8}>
                      <div className="composer-project-list">
                        {projects.length > 0 ? projects.map((project) => {
                          const isSelected = project.id === document.projectId;
                          return (
                            <DropdownMenuItem
                              asChild
                              className={`w-full min-h-8 gap-2 px-2 py-1 text-left text-xs ${isSelected ? 'bg-selected text-text' : ''}`}
                              key={project.id}
                              onSelect={() => selectProject(project.id)}
                              textValue={project.name}
                            >
                              <Button
                                aria-current={isSelected ? 'true' : undefined}
                                className="min-h-8 w-full justify-start gap-2 px-2 py-1 text-left text-xs"
                                size="small"
                                type="button"
                                variant="ghost"
                              >
                                <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
                                <span className="min-w-0 flex-1 truncate">{project.name}</span>
                                {isSelected ? <Check aria-hidden="true" className="shrink-0 text-accent" size={14} /> : null}
                              </Button>
                            </DropdownMenuItem>
                          );
                        }) : (
                          <p className="px-2 py-3 text-xs text-secondary-text" role="status">{t('noProjects')}</p>
                        )}
                      </div>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem
                        asChild
                        className="w-full min-h-8 gap-2 px-2 py-1 text-left text-xs"
                        disabled={document.projectId === null}
                        onSelect={() => selectProject(null)}
                      >
                        <Button
                          className="min-h-8 w-full justify-start gap-2 px-2 py-1 text-left text-xs"
                          disabled={document.projectId === null}
                          size="small"
                          type="button"
                          variant="ghost"
                        >
                          <FolderMinus aria-hidden="true" className="shrink-0 text-secondary-icon" size={14} />
                          <span>{t('workWithoutProject')}</span>
                        </Button>
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                  {document.selectedNodeIds.length >= 2 && <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button aria-label={t('designEditorArrange')} size="small" title={t('designEditorArrange')} variant="ghost"><Move aria-hidden="true" size={15} /><ChevronDown aria-hidden="true" size={13} /></Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start" side="top">
                      <DropdownMenuItem onSelect={() => {
                        try {
                          void publish(groupDesignSelection(document, t('designEditorGroupName')));
                        } catch {
                          toast.error(t('designEditorGroupFailed'));
                        }
                      }}>{t('designEditorGroup')}</DropdownMenuItem>
                      <DropdownMenuSeparator />
                      {([
                        ['align-left', 'designEditorAlignLeft'], ['align-center-x', 'designEditorAlignCenterX'], ['align-right', 'designEditorAlignRight'],
                        ['align-top', 'designEditorAlignTop'], ['align-center-y', 'designEditorAlignCenterY'], ['align-bottom', 'designEditorAlignBottom'],
                      ] as const).map(([action, labelKey]) => <DropdownMenuItem key={action} onSelect={() => void publish(arrangeDesignSelection(document, action))}>{t(labelKey)}</DropdownMenuItem>)}
                      <DropdownMenuItem disabled={document.selectedNodeIds.length < 3} onSelect={() => void publish(arrangeDesignSelection(document, 'distribute-horizontal'))}>{t('designEditorDistributeHorizontal')}</DropdownMenuItem>
                      <DropdownMenuItem disabled={document.selectedNodeIds.length < 3} onSelect={() => void publish(arrangeDesignSelection(document, 'distribute-vertical'))}>{t('designEditorDistributeVertical')}</DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>}
                </div>
              <section aria-label={t('designEditorPromptCard')} className="design-editor-prompt-card">
                <Input
                  appearance="bare"
                  aria-label={t('designEditorPrompt')}
                  className="design-editor-prompt-input design-editor-composer-prompt"
                  fieldLayout="none"
                  maxLength={4000}
                  onChange={(event) => setPrompt(event.currentTarget.value)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter' && !event.shiftKey) {
                      event.preventDefault();
                      void generateDesign();
                    }
                  }}
                  placeholder={t('designEditorPromptPlaceholder')}
                  type="text"
                  value={prompt}
                />
                <PendingAttachmentGallery
                  attachments={designReferences}
                  disabled={isGenerating || isSwitchingDocument || isReadingDesignReferences}
                  onRemove={removeDesignReference}
                />
                <div className="design-editor-composer-toolbar">
                  <input
                    accept=".png,.jpg,.jpeg,.webp"
                    aria-label={t('attachmentChooseFile')}
                    className="sr-only"
                    disabled={isGenerating || isSwitchingDocument || isReadingDesignReferences}
                    multiple
                    onChange={(event) => {
                      const files = Array.from(event.currentTarget.files ?? []);
                      event.currentTarget.value = '';
                      void addDesignReferenceFiles(files);
                    }}
                    ref={filePickerRef}
                    type="file"
                  />
                  <Button
                    aria-label={t('attachmentChooseFile')}
                    disabled={isGenerating || isSwitchingDocument || isReadingDesignReferences || designReferences.length >= 3}
                    onClick={() => filePickerRef.current?.click()}
                    size="icon"
                    type="button"
                    variant="ghost"
                  >
                    <Paperclip aria-hidden="true" size={18} />
                  </Button>
                  <div className="design-editor-model-picker">
                    <ModelPicker
                    canSelectProvider
                    chatGptWorkspaceSelectionRequired={chatGptWorkspaceSelectionRequired}
                    disabled={isGenerating || modelStatus !== 'ready' || visibleModels.length === 0}
                    emptyModelsLabel={t('designEditorNoModels')}
                    fastModeAvailable={false}
                    fastModeEnabled={false}
                    fastModeLoading={false}
                    fastModeSaving={false}
                    hiddenModelKeys={hiddenModelKeys}
                    isOpen={modelPickerOpen}
                    knownModels={models}
                    modelFavorites={modelFavorites}
                    modelProviders={modelProviders}
                    models={visibleModels}
                    onFastModeToggle={() => undefined}
                    onOpenChange={setModelPickerOpen}
                    onQueryChange={setModelQuery}
                    onRemoveFavorite={(favorite) => { void removeModelFavorite(favorite); }}
                    onSelect={(model) => {
                      setDesignModelKey(model.key);
                      setModelPickerOpen(false);
                    }}
                    onSelectFavorite={async (favorite: ChatModelFavorite) => {
                      const model = models.find((candidate) =>
                        candidate.providerId === favorite.providerId &&
                        candidate.modelId === favorite.modelId &&
                        favoriteSourceConnection(candidate) === favorite.sourceConnectionId &&
                        !hiddenModelKeys.includes(modelRouteKey(candidate)),
                      );
                      if (!model) return false;
                      setDesignModelKey(model.key);
                      setModelPickerOpen(false);
                      return true;
                    }}
                    onSelectProvider={() => setModelQuery('')}
                    onToggleFavorite={(model) => { void toggleFavoriteModel(model); }}
                    query={modelQuery}
                    selectedModel={selectedDesignModelForPicker}
                    showFastModeControl={false}
                    />
                  </div>
                {selectedDesignModel?.supportsReasoning ? (
                  <Select
                    aria-label={t('reasoning')}
                    className="design-editor-reasoning-select openchat-dropdown-trigger--subtle min-w-0 max-w-full py-2 pl-2.5 pr-2 text-sm"
                    disabled={isGenerating || isSwitchingDocument}
                    label={t('reasoning')}
                    leadingIcon={<Brain aria-hidden="true" size={16} />}
                    onChange={(event) => setDesignReasoningEffort(event.currentTarget.value || null)}
                    options={[
                      {
                        value: '',
                        label: t('reasoningDefault'),
                        icon: <Brain aria-hidden="true" size={16} />,
                      },
                      ...selectedDesignModel.reasoningLevels.map((level) => ({
                        value: level,
                        label: reasoningLevelLabel(level, t),
                        icon: <Brain aria-hidden="true" size={16} />,
                      })),
                    ]}
                    value={selectedDesignReasoningEffort ?? ''}
                    visuallyHiddenLabel
                  />
                ) : null}
                <div className="design-editor-prompt-actions">
                  <span aria-live="polite" className="design-editor-generation-status" role="status">
                    {isGenerating && <><LoaderCircle aria-hidden="true" className="design-editor-generation-spinner" size={15} />{t(isAddingGeneratedNodes ? 'designEditorApplyingChanges' : 'designEditorGenerating')}</>}
                  </span>
                  {isGenerating ? (
                    <Button aria-label={t('designEditorCancelGeneration')} onClick={cancelGeneration} size="icon" title={t('designEditorCancelGeneration')} variant="ghost"><Square aria-hidden="true" size={15} /></Button>
                  ) : (
                    <Button aria-label={t('designEditorGenerate')} disabled={modelStatus !== 'ready' || visibleModels.length === 0} onClick={() => void generateDesign()} size="icon" title={t('designEditorGenerate')} variant="primary"><Send aria-hidden="true" size={16} /></Button>
                  )}
                </div>
                </div>

                {projectListLoadError && <p className="design-editor-prompt-error" role="status">{t('designEditorProjectsFailed')}</p>}
              </section>
              </div>
              <div className="design-editor-floating-dock">
                <div aria-label={t('designEditorTools')} className="design-editor-dock-tools" role="toolbar">
                  <Button
                    aria-label={t(isPrototypePreview ? 'designEditorPrototypeStop' : 'designEditorPrototypePreview')}
                    aria-pressed={isPrototypePreview}
                    disabled={isGenerating || isSwitchingDocument}
                    onClick={isPrototypePreview ? stopPrototypePreview : startPrototypePreview}
                    size="icon"
                    title={t(isPrototypePreview ? 'designEditorPrototypeStop' : 'designEditorPrototypePreview')}
                    variant={isPrototypePreview ? 'secondary' : 'ghost'}
                  >
                    {isPrototypePreview ? <Square aria-hidden="true" size={15} /> : <Play aria-hidden="true" size={15} />}
                  </Button>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button aria-label={t(designNodeToolLabelKeys[selectedNodeTool])} className="w-20 min-w-0 shrink-0 justify-start gap-1 px-1.5" disabled={isGenerating} size="small" title={t(designNodeToolLabelKeys[selectedNodeTool])} variant="ghost">
                        {selectedNodeTool === 'frame' ? <Frame aria-hidden="true" size={16} /> : null}
                        {selectedNodeTool === 'rectangle' ? <RectangleHorizontal aria-hidden="true" size={16} /> : null}
                        {selectedNodeTool === 'ellipse' ? <Circle aria-hidden="true" size={16} /> : null}
                        {selectedNodeTool === 'path' ? <PenTool aria-hidden="true" size={16} /> : null}
                        {selectedNodeTool === 'text' ? <Type aria-hidden="true" size={16} /> : null}
                        <span className="min-w-0 flex-1 truncate text-left">{t(designNodeToolLabelKeys[selectedNodeTool])}</span>
                        <ChevronDown aria-hidden="true" className="shrink-0 text-secondary-icon" size={13} />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start" side="top">
                      <DropdownMenuItem onSelect={() => selectAndAddNode('frame')}><Frame aria-hidden="true" size={15} />{t('designEditorAddFrame')}</DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => selectAndAddNode('rectangle')}><RectangleHorizontal aria-hidden="true" size={15} />{t('designEditorAddRectangle')}</DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => selectAndAddNode('ellipse')}><Circle aria-hidden="true" size={15} />{t('designEditorAddEllipse')}</DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => selectAndAddNode('path')}><PenTool aria-hidden="true" size={15} />{t('designEditorAddPath')}</DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => selectAndAddNode('text')}><Type aria-hidden="true" size={15} />{t('designEditorAddText')}</DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
                <span aria-hidden="true" className="design-editor-toolbar-divider" />
                <CanvasZoomControls
                  onResetView={() => setCamera(initialCamera)}
                  onSetZoom={(scale) => setCamera((current) => ({ ...current, scale: clamp(scale, 0.05, 8) }))}
                  onZoomIn={() => setCamera((current) => ({ ...current, scale: clamp(current.scale * 1.15, 0.05, 8) }))}
                  onZoomOut={() => setCamera((current) => ({ ...current, scale: clamp(current.scale / 1.15, 0.05, 8) }))}
                  scale={camera.scale}
                />
              </div>
            </div>
          </section>
        </div>
        <aside aria-hidden={!selectedNode} className={`design-editor-frame-inspector${selectedNode ? ' is-open' : ''}`} inert={!selectedNode}>
          {/* biome-ignore lint/a11y/useSemanticElements: The resizable inspector edge uses separator semantics with keyboard support. */}
          <div
            aria-label={t('resizeSidebar')}
            aria-orientation="vertical"
            aria-valuemax={DESIGN_INSPECTOR_MAX_WIDTH}
            aria-valuemin={DESIGN_INSPECTOR_MIN_WIDTH}
            aria-valuenow={inspectorWidth}
            className="design-editor-inspector-resizer"
            onKeyDown={(event) => resizeInspectorByKey(event, inspectorWidth, setInspectorWidth, (width) => {
              void writeDesignInspectorWidth(width).catch(() => toast.error(t('designEditorSaveFailed')));
            })}
            onPointerDown={(event) => {
              if (event.button !== 0) return;
              event.preventDefault();
              inspectorResizeStartRef.current = { pointerX: event.clientX, width: inspectorWidth };
              setIsResizingInspector(true);
            }}
            ref={inspectorSeparatorRef}
            role="separator"
            tabIndex={selectedNode ? 0 : -1}
          >
            <span aria-hidden="true" className="design-editor-inspector-resizer-line" />
          </div>
          {selectedNode && <div className="design-editor-inspector-content">
            <div className="design-editor-heading">
              <DesignLayerIcon type={selectedNode.type} />
              <span>{t(designNodeTypeLabelKey(selectedNode))}</span>
            </div>
            <Input density="compact" label={t('designEditorName')} maxLength={256} onChange={(event) => updateNodeGeometry(selectedNode.id, { name: event.currentTarget.value })} value={selectedNode.name} />
            <div className="design-editor-inspector-group">
              <div className="design-editor-inspector-group-heading"><Move aria-hidden="true" size={15} /><span>{t('designEditorPosition')}</span></div>
              <div className="design-editor-number-grid">
                <Input density="compact" label={t('designEditorX')} onChange={(event) => updateNodeGeometry(selectedNode.id, { x: readFiniteNumber(event.currentTarget.value, selectedNode.x) })} type="number" value={selectedNode.x} />
                <Input density="compact" label={t('designEditorY')} onChange={(event) => updateNodeGeometry(selectedNode.id, { y: readFiniteNumber(event.currentTarget.value, selectedNode.y) })} type="number" value={selectedNode.y} />
              </div>
            </div>
            <div className="design-editor-inspector-group">
              <div className="design-editor-inspector-group-heading"><Scaling aria-hidden="true" size={15} /><span>{t('designEditorSize')}</span></div>
              <div className="design-editor-number-grid">
                <Input density="compact" label={t('designEditorWidth')} min={1} onChange={(event) => updateNodeGeometry(selectedNode.id, { width: Math.max(1, readFiniteNumber(event.currentTarget.value, selectedNode.width)) })} type="number" value={selectedNode.width} />
                <Input density="compact" label={t('designEditorHeight')} min={1} onChange={(event) => updateNodeGeometry(selectedNode.id, { height: Math.max(1, readFiniteNumber(event.currentTarget.value, selectedNode.height)) })} type="number" value={selectedNode.height} />
              </div>
            </div>
            <section aria-label={t('designEditorPrototypeSettings')} className="design-editor-inspector-group">
              <div className="design-editor-inspector-group-heading"><Play aria-hidden="true" size={15} /><span>{t('designEditorPrototypeSettings')}</span></div>
              <Select
                label={t('designEditorPrototypeTarget')}
                onChange={(event) => setPrototypeDraft((draft) => ({ ...draft, targetNodeId: event.currentTarget.value }))}
                options={pageNodes.filter((node): node is DesignFrameNode => node.type === 'frame' && node.id !== selectedNode.id).map((node) => ({ value: node.id, label: node.name }))}
                placeholder={t('designEditorPrototypeChooseTarget')}
                value={prototypeDraft.targetNodeId}
              />
              <Select
                label={t('designEditorPrototypeTrigger')}
                onChange={(event) => setPrototypeDraft((draft) => ({ ...draft, trigger: event.currentTarget.value === 'hover' ? 'hover' : 'click' }))}
                options={[{ value: 'click', label: t('designEditorPrototypeClick') }, { value: 'hover', label: t('designEditorPrototypeHover') }]}
                value={prototypeDraft.trigger}
              />
              <Select
                label={t('designEditorPrototypePresentation')}
                onChange={(event) => setPrototypeDraft((draft) => ({ ...draft, presentation: event.currentTarget.value === 'overlay' ? 'overlay' : 'navigate' }))}
                options={[{ value: 'navigate', label: t('designEditorPrototypeNavigate') }, { value: 'overlay', label: t('designEditorPrototypeOverlay') }]}
                value={prototypeDraft.presentation}
              />
              <Select
                label={t('designEditorPrototypeTransition')}
                onChange={(event) => setPrototypeDraft((draft) => ({ ...draft, transition: event.currentTarget.value === 'instant' || event.currentTarget.value === 'slide' ? event.currentTarget.value : 'dissolve' }))}
                options={[{ value: 'instant', label: t('designEditorPrototypeInstant') }, { value: 'dissolve', label: t('designEditorPrototypeDissolve') }, { value: 'slide', label: t('designEditorPrototypeSlide') }]}
                value={prototypeDraft.transition}
              />
              <div className="flex gap-2">
                <Button className="min-w-0 flex-1" disabled={!prototypeDraft.targetNodeId || selectedNode.locked || isPrototypePreview} onClick={savePrototypeConnection} size="small" variant="secondary">{t('designEditorPrototypeConnect')}</Button>
                {selectedPrototypeInteraction && <Button aria-label={t('designEditorPrototypeRemove')} disabled={isPrototypePreview} onClick={removePrototypeConnection} size="icon" title={t('designEditorPrototypeRemove')} variant="ghost"><Trash2 aria-hidden="true" size={14} /></Button>}
              </div>
            </section>
            {selectedNode.type === 'text' && <Textarea
              aria-label={t('designEditorText')}
              className="design-editor-text-content"
              label={t('designEditorText')}
              maxLength={100000}
              onChange={(event) => {
                const content = event.currentTarget.value;
                updateNode(selectedNode.id, (node) => node.type === 'text' ? { ...node, content } : node);
              }}
              rows={4}
              value={selectedNode.content}
            />}
            {selectedNode.type === 'text' && <>
              <div className="design-editor-number-grid">
                <Input density="compact" label={t('designEditorFontSize')} max={4096} min={1} onChange={(event) => {
                  const fontSize = readFiniteNumber(event.currentTarget.value, selectedNode.fontSize);
                  updateNode(selectedNode.id, (node) => node.type === 'text' ? { ...node, fontSize: clamp(fontSize, 1, 4096) } : node);
                }} type="number" value={selectedNode.fontSize} />
                <Input density="compact" label={t('designEditorFontWeight')} max={900} min={100} onChange={(event) => {
                  const fontWeight = Math.round(readFiniteNumber(event.currentTarget.value, selectedNode.fontWeight) / 100) * 100;
                  updateNode(selectedNode.id, (node) => node.type === 'text' ? { ...node, fontWeight: clamp(fontWeight, 100, 900) } : node);
                }} step={100} type="number" value={selectedNode.fontWeight} />
              </div>
              <div className="design-editor-number-grid">
                <Select
                  label={t('designEditorFontFamily')}
                  onChange={(event) => updateNode(selectedNode.id, (node) => node.type === 'text' ? { ...node, fontFamily: event.currentTarget.value === 'serif' || event.currentTarget.value === 'monospace' ? event.currentTarget.value : 'sans-serif' } : node)}
                  options={[{ value: 'sans-serif', label: t('designEditorFontSans') }, { value: 'serif', label: t('designEditorFontSerif') }, { value: 'monospace', label: t('designEditorFontMonospace') }]}
                  value={selectedNode.fontFamily ?? 'sans-serif'}
                />
                <Select
                  label={t('designEditorTextAlign')}
                  onChange={(event) => updateNode(selectedNode.id, (node) => node.type === 'text' ? { ...node, textAlign: event.currentTarget.value === 'center' || event.currentTarget.value === 'right' ? event.currentTarget.value : 'left' } : node)}
                  options={[{ value: 'left', label: t('designEditorTextAlignLeft') }, { value: 'center', label: t('designEditorTextAlignCenter') }, { value: 'right', label: t('designEditorTextAlignRight') }]}
                  value={selectedNode.textAlign ?? 'left'}
                />
              </div>
              <div className="design-editor-number-grid">
                <Input density="compact" label={t('designEditorLineHeight')} max={4} min={0.5} onChange={(event) => {
                  const lineHeight = readFiniteNumber(event.currentTarget.value, selectedNode.lineHeight ?? 1.2);
                  updateNode(selectedNode.id, (node) => node.type === 'text' ? { ...node, lineHeight: clamp(lineHeight, 0.5, 4) } : node);
                }} step={0.1} type="number" value={selectedNode.lineHeight ?? 1.2} />
                <Input density="compact" label={t('designEditorLetterSpacing')} max={100} min={-100} onChange={(event) => {
                  const letterSpacing = readFiniteNumber(event.currentTarget.value, selectedNode.letterSpacing ?? 0);
                  updateNode(selectedNode.id, (node) => node.type === 'text' ? { ...node, letterSpacing: clamp(letterSpacing, -100, 100) } : node);
                }} step={0.1} type="number" value={selectedNode.letterSpacing ?? 0} />
              </div>
              <div className="design-editor-number-grid">
                <Select
                  label={t('designEditorTextStyles')}
                  onChange={(event) => {
                    try {
                      void publish(applyDesignTextStyle(document, event.currentTarget.value));
                    } catch {
                      toast.error(t('designEditorTextStyleApplyFailed'));
                    }
                  }}
                  options={document.textStyles.length > 0 ? document.textStyles.map((style) => ({ value: style.id, label: style.name })) : [{ value: '', label: t('designEditorNoTextStyles'), disabled: true }]}
                  value={selectedNode.textStyleId ?? ''}
                />
                <Button className="self-end" disabled={selectedNode.locked || document.textStyles.length >= 2000} onClick={() => openDesignSystemDialog('text', nextUniqueDesignName(t('designEditorTextStyleDefaultName', { count: document.textStyles.length + 1 }), document.textStyles))} size="small" variant="secondary">
                  <Save aria-hidden="true" size={14} />{t('designEditorCreateTextStyle')}
                </Button>
              </div>
            </>}
            {(selectedNode.type === 'frame' || selectedNode.type === 'rectangle' || selectedNode.type === 'ellipse') && <div className="design-editor-fill-controls">
              <div className="design-editor-inspector-group-heading"><PaintBucket aria-hidden="true" size={15} /><span>{t('designEditorFill')}</span></div>
              <ColorPicker
                label={t('designEditorFill')}
                onChange={(value) => {
                  if (selectedNode.fillStyle?.type === 'gradient' && selectedGradientStop) {
                    updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse'
                      ? updateDesignGradientStops(node, selectedGradientStops.map((stop, index) => index === activeGradientStop ? { ...stop, color: value } : stop))
                      : node);
                    return;
                  }
                  updateNodeFill(selectedNode.id, value);
                }}
                showColorControls={activeFillType !== 'image'}
                swatchStyle={designFillPreviewStyle(selectedNode, imageUrls)}
                valueLabel={activeFillType === 'image' ? t('designEditorImageFill') : undefined}
                value={selectedGradientStop?.color ?? selectedNode.fill}
              >
                <fieldset className="design-editor-fill-mode-switch">
                  <legend className="sr-only">{t('designEditorFillType')}</legend>
                  <button aria-label={t('designCustomBackgroundSolid')} aria-pressed={activeFillType === 'solid'} className="design-editor-fill-mode" onClick={() => {
                    setActiveFillType('solid');
                    updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' ? { ...node, fillStyle: undefined } : node);
                  }} title={t('designCustomBackgroundSolid')} type="button"><PaintBucket aria-hidden="true" size={15} /><span>{t('designCustomBackgroundSolid')}</span></button>
                  <button aria-label={t('designCustomBackgroundGradient')} aria-pressed={activeFillType === 'gradient'} className="design-editor-fill-mode" onClick={() => {
                    setActiveFillType('gradient');
                    setActiveGradientStop(0);
                    updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse'
                      ? updateDesignGradientStops(node, node.fillStyle?.type === 'gradient' ? designGradientStops(node.fill, node.fillStyle) : [{ position: 0, color: node.fill }, { position: 100, color: '#ffffff' }])
                      : node);
                  }} title={t('designCustomBackgroundGradient')} type="button"><Blend aria-hidden="true" size={15} /><span>{t('designCustomBackgroundGradient')}</span></button>
                  <button aria-label={t('designEditorImageFill')} aria-pressed={activeFillType === 'image'} className="design-editor-fill-mode" onClick={() => {
                    setActiveFillType('image');
                    fillImageInputRef.current?.click();
                  }} title={t('designEditorImageFill')} type="button"><ImageIcon aria-hidden="true" size={15} /><span>{t('designEditorImageFill')}</span></button>
                </fieldset>
                {selectedNode.fillStyle?.type === 'gradient' && <div className="design-editor-gradient-controls">
                  <fieldset
                    className="design-editor-gradient-track"
                    onPointerDown={(event) => {
                      if (event.target instanceof Element && event.target.closest('.design-editor-gradient-stop-handle')) return;
                      event.preventDefault();
                      addGradientStopAtPointer(event);
                    }}
                    onPointerMove={(event) => {
                      if (event.buttons === 1) moveGradientStop(activeGradientStop, event.clientX);
                    }}
                    ref={gradientTrackRef}
                  >
                    <legend className="sr-only">{t('designEditorGradientStops')}</legend>
                    <div aria-hidden="true" className="design-editor-gradient-track-fill" style={{ backgroundImage: `linear-gradient(${selectedNode.fillStyle.angle}deg, ${selectedGradientStops.map((stop) => `${stop.color} ${stop.position}%`).join(', ')})` }} />
                    {selectedGradientStops.map((stop, index) => <button
                      aria-label={`${index === 0 ? t('designEditorGradientStart') : index === selectedGradientStops.length - 1 ? t('designEditorGradientEnd') : t('designEditorGradientStop')} ${Math.round(stop.position)}%`}
                      aria-pressed={activeGradientStop === index}
                      className="design-editor-gradient-stop-handle"
                      key={`${stop.position}-${stop.color}`}
                      onClick={() => setActiveGradientStop(index)}
                      onPointerDown={(event) => {
                        event.stopPropagation();
                        gradientTrackRef.current?.setPointerCapture(event.pointerId);
                        setActiveGradientStop(index);
                      }}
                      style={{ left: `${stop.position}%`, backgroundColor: stop.color }}
                      type="button"
                    />)}
                  </fieldset>
                  <div className="design-editor-gradient-stop-actions">
                    <span aria-live="polite" className="design-editor-gradient-stop-position">{t('designEditorGradientStop')} · {Math.round(selectedGradientStop?.position ?? 0)}%</span>
                    <Button aria-label={t('designEditorAddGradientStop')} disabled={selectedGradientStops.length >= 8} onClick={() => {
                      const leftIndex = Math.min(activeGradientStop, selectedGradientStops.length - 2);
                      const left = selectedGradientStops[leftIndex];
                      const right = selectedGradientStops[leftIndex + 1];
                      if (!left || !right || right.position - left.position < 2) return;
                      const position = (left.position + right.position) / 2;
                      const ratio = (position - left.position) / (right.position - left.position);
                      const stops = [...selectedGradientStops.slice(0, leftIndex + 1), { position, color: interpolateHexColor(left.color, right.color, ratio) }, ...selectedGradientStops.slice(leftIndex + 1)];
                      updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' ? updateDesignGradientStops(node, stops) : node);
                      setActiveGradientStop(leftIndex + 1);
                    }} size="icon" title={t('designEditorAddGradientStop')} variant="ghost"><Plus aria-hidden="true" size={14} /></Button>
                    <Button aria-label={t('designEditorRemoveGradientStop')} disabled={selectedGradientStops.length <= 2} onClick={() => {
                      if (selectedGradientStops.length <= 2) return;
                      const nextStops = selectedGradientStops.filter((_, index) => index !== activeGradientStop);
                      updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' ? updateDesignGradientStops(node, nextStops) : node);
                      setActiveGradientStop(Math.max(0, activeGradientStop - 1));
                    }} size="icon" title={t('designEditorRemoveGradientStop')} variant="ghost"><Minus aria-hidden="true" size={14} /></Button>
                  </div>
                  {selectedGradientStop && <>
                    <label className="design-editor-gradient-angle" htmlFor="design-gradient-stop-position"><span>{t('designEditorGradientStopPosition')}</span><span>{Math.round(selectedGradientStop.position)}%</span></label>
                    <input className="design-editor-gradient-angle-input" id="design-gradient-stop-position" max={Math.max((selectedGradientStops[activeGradientStop + 1]?.position ?? 100) - 1, selectedGradientStop.position)} min={Math.min((selectedGradientStops[activeGradientStop - 1]?.position ?? 0) + 1, selectedGradientStop.position)} onChange={(event) => {
                      const nextStops = selectedGradientStops.map((stop, index) => index === activeGradientStop ? { ...stop, position: Number(event.currentTarget.value) } : stop);
                      updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' ? updateDesignGradientStops(node, nextStops) : node);
                    }} type="range" value={selectedGradientStop.position} />
                  </>}
                  <label className="design-editor-gradient-angle" htmlFor="design-gradient-angle"><span>{t('designEditorGradientAngle')}</span><span>{selectedNode.fillStyle.angle}°</span></label>
                  <input className="design-editor-gradient-angle-input" id="design-gradient-angle" max={360} min={0} onChange={(event) => updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse' ? { ...node, fillStyle: node.fillStyle?.type === 'gradient' ? { ...node.fillStyle, angle: Number(event.currentTarget.value) } : node.fillStyle } : node)} type="range" value={selectedNode.fillStyle.angle} />
                </div>}
                {activeFillType === 'image' && <div className="design-editor-image-fill-settings">
                  {selectedImageFill && imageUrls.has(selectedImageFill.assetId) && <div aria-hidden="true" className="design-editor-image-fill-preview" style={{ backgroundImage: `url("${imageUrls.get(selectedImageFill.assetId)}")` }} />}
                  <Button className="w-full justify-start gap-2" onClick={() => fillImageInputRef.current?.click()} size="small" variant="ghost"><ImageIcon aria-hidden="true" size={15} />{t(selectedImageFill ? 'designEditorReplaceImage' : 'designEditorChooseImage')}</Button>
                  {selectedImageFill && <Select
                    aria-label={t('designEditorImageFit')}
                    label={t('designEditorImageFit')}
                    onChange={(event) => updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse'
                      ? { ...node, fillStyle: node.fillStyle?.type === 'image' ? { ...node.fillStyle, fit: event.currentTarget.value === 'contain' ? 'contain' : 'cover' } : node.fillStyle }
                      : node)}
                    options={[{ value: 'cover', label: t('designEditorImageFitCover') }, { value: 'contain', label: t('designEditorImageFitContain') }]}
                    value={selectedImageFill.fit}
                  />}
                </div>}
                <input aria-label={t('designEditorImageFit')} className="sr-only" ref={fillImageInputRef} accept="image/png,image/jpeg,image/webp" onChange={(event) => {
                  const file = event.currentTarget.files?.[0];
                  event.currentTarget.value = '';
                  if (!file || !isSupportedDesignImageMimeType(file.type) || file.size > 16 * 1024 * 1024) {
                    toast.error(t('designEditorImageImportFailed'));
                    return;
                  }
                  const mimeType = file.type;
                  void file.arrayBuffer().then((buffer) => importDesignImage(mimeType, new Uint8Array(buffer))).then((image) => {
                    if (!isSupportedDesignImageMimeType(image.mimeType)) throw new Error('The imported image type is invalid.');
                    updateNode(selectedNode.id, (node) => node.type === 'frame' || node.type === 'rectangle' || node.type === 'ellipse'
                      ? { ...node, fillStyle: { type: 'image', assetId: image.id, mimeType, sizeBytes: image.sizeBytes, fit: 'cover' } }
                      : node);
                  }).catch(() => toast.error(t('designEditorImageImportFailed')));
                }} type="file" />
              </ColorPicker>
            </div>}
            {selectedNode.type === 'path' && <ColorPicker label={t('designEditorFill')} onChange={(value) => updateNodeFill(selectedNode.id, value)} value={selectedNode.fill} />}
            {selectedNode.type === 'text' && <ColorPicker label={t('designEditorTextColor')} onChange={(color) => updateNode(selectedNode.id, (node) => node.type === 'text' ? { ...node, color, colorVariableId: undefined } : node)} value={selectedNode.color} />}
            {selectedNode.type === 'frame' && <div className="grid gap-2">
              {selectedNode.componentRef?.role === 'master' && <Button disabled={selectedNode.locked} onClick={() => {
                const component = document.components.find((item) => item.id === selectedNode.componentRef?.componentId);
                if (!component) return;
                try {
                  void publish(createDesignComponentVariant(document, component.id, t('designEditorComponentVariantDefaultName', { count: component.variants.length + 1 })));
                } catch {
                  toast.error(t('designEditorComponentVariantFailed'));
                }
              }} size="small" variant="secondary"><Plus aria-hidden="true" size={14} />{t('designEditorComponentVariant')}</Button>}
              {document.components.length > 0 && <Select
                label={t('designEditorComponents')}
                onChange={(event) => {
                  const [componentId, variantId] = event.currentTarget.value.split(':');
                  if (!componentId || !variantId) return;
                  try {
                    void publish(createDesignComponentInstance(document, componentId, variantId));
                  } catch {
                    toast.error(t('designEditorComponentInstanceFailed'));
                  }
                }}
                options={document.components.flatMap((component) => component.variants.map((variant) => ({ value: `${component.id}:${variant.id}`, label: `${component.name} / ${variant.name}` })))}
                placeholder={t('designEditorComponents')}
                value=""
              />}
            </div>}
            {(selectedNode.type === 'frame' || selectedNode.type === 'rectangle') && <div className="design-editor-corner-radii">
              <div className="design-editor-inspector-group-heading"><Radius aria-hidden="true" size={15} /><span>{t('designEditorCornerRadius')}</span></div>
              <div className="design-editor-radius-grid">
                {([
                  ['topLeft', 'designEditorRadiusTopLeft'],
                  ['topRight', 'designEditorRadiusTopRight'],
                  ['bottomLeft', 'designEditorRadiusBottomLeft'],
                  ['bottomRight', 'designEditorRadiusBottomRight'],
                ] as const).map(([corner, labelKey]) => (
                  <Input
                    density="compact"
                    key={corner}
                    label={t(labelKey)}
                    min={0}
                    onChange={(event) => updateNode(selectedNode.id, (node) => {
                      if (node.type !== 'frame' && node.type !== 'rectangle') return node;
                      const radius = Math.max(0, readFiniteNumber(event.currentTarget.value, 0));
                      return { ...node, radius: corner === 'topLeft' ? radius : node.radius ?? radius, cornerRadii: { ...designNodeCornerRadii(node), [corner]: radius } };
                    })}
                    type="number"
                    value={designNodeCornerRadii(selectedNode)[corner]}
                  />
                ))}
              </div>
            </div>}
          </div>}
        </aside>
      </div>
      {contextMenuPosition && createPortal(
        <div
          aria-label={t('designEditorFrame')}
          className="design-editor-node-context-menu"
          onKeyDown={handleContextMenuKeyDown}
          ref={contextMenuRef}
          role="menu"
          style={{ left: contextMenuPosition.x, top: contextMenuPosition.y }}
        >
          <button onClick={() => { setContextMenuPosition(null); void exportDocument(); }} role="menuitem" tabIndex={-1} type="button"><Download aria-hidden="true" size={15} />{t('designEditorExport')}</button>
          <button onClick={() => { setContextMenuPosition(null); void exportDocumentPng(); }} role="menuitem" tabIndex={-1} type="button"><ImageDown aria-hidden="true" size={15} />{t('designEditorExportPng')}</button>
          <button onClick={() => { setContextMenuPosition(null); void exportDocumentPdf(); }} role="menuitem" tabIndex={-1} type="button"><FileText aria-hidden="true" size={15} />{t('designEditorExportPdf')}</button>
          <button onClick={() => { setContextMenuPosition(null); void exportDocumentCode(); }} role="menuitem" tabIndex={-1} type="button"><CodeXml aria-hidden="true" size={15} />{t('designEditorExportCode')}</button>
          <span aria-hidden="true" className="design-editor-context-menu-separator" />
          <button disabled={isGenerating || isSwitchingDocument} onClick={() => { setContextMenuPosition(null); void exportDocumentArchive(); }} role="menuitem" tabIndex={-1} type="button"><Save aria-hidden="true" size={15} />{t('designEditorExportArchive')}</button>
        </div>,
        window.document.body,
      )}
      <Dialog
        initialFocusRef={designSystemNameRef}
        onOpenChange={(open) => {
          setIsDesignSystemDialogOpen(open);
          if (!open) setDesignSystemDialogKind(null);
        }}
        open={isDesignSystemDialogOpen}
        title={t(designSystemDialogKind === 'text' ? 'designEditorTextStyles' : designSystemDialogKind === 'component' ? 'designEditorCreateComponent' : 'designEditorCreateColorVariable')}
      >
        <form className="design-editor-new-document-form" onSubmit={(event) => { event.preventDefault(); saveDesignSystemItem(); }}>
          <Input
            label={t(designSystemDialogKind === 'color' ? 'designEditorColorVariableNameLabel' : designSystemDialogKind === 'text' ? 'designEditorTextStyleName' : 'designEditorName')}
            maxLength={128}
            onChange={(event) => setDesignSystemNameDraft(event.currentTarget.value)}
            ref={designSystemNameRef}
            value={designSystemNameDraft}
          />
          <div className="flex justify-end gap-2">
            <Button onClick={() => setIsDesignSystemDialogOpen(false)} type="button" variant="ghost">{t('cancel')}</Button>
            <Button disabled={!designSystemNameDraft.trim()} type="submit" variant="primary">{t(designSystemDialogKind === 'color' ? 'designEditorSaveColorVariable' : designSystemDialogKind === 'text' ? 'designEditorCreateTextStyle' : 'designEditorCreateComponent')}</Button>
          </div>
        </form>
      </Dialog>
      <Dialog
        initialFocusRef={newDocumentNameRef}
        onOpenChange={(open) => {
          setIsNewDocumentDialogOpen(open);
          if (!open) setNewDocumentName('');
        }}
        open={isNewDocumentDialogOpen}
        title={t('designEditorNewDocument')}
      >
        <form
          className="design-editor-new-document-form"
          onSubmit={(event) => {
            event.preventDefault();
            void createDocument(newDocumentName);
          }}
        >
          <Input
            label={t('designEditorName')}
            maxLength={256}
            onChange={(event) => setNewDocumentName(event.currentTarget.value)}
            ref={newDocumentNameRef}
            value={newDocumentName}
          />
          <div className="flex justify-end gap-2">
            <Button disabled={isSwitchingDocument} onClick={() => setIsNewDocumentDialogOpen(false)} type="button" variant="ghost">{t('cancel')}</Button>
            <Button disabled={isSwitchingDocument || !newDocumentName.trim()} type="submit" variant="primary">{t('createProjectSubmit')}</Button>
          </div>
        </form>
      </Dialog>
    </section>
  );
}

function renderNode(
  node: DesignNode,
  selected: boolean,
  variablesById: ReadonlyMap<string, string>,
  imageUrls: ReadonlyMap<string, string>,
  onPointerDown: (event: ReactPointerEvent<HTMLButtonElement>, node: DesignNode) => void,
  onPointerMove: (event: ReactPointerEvent<HTMLElement | HTMLButtonElement>) => void,
  onPointerUp: () => void,
  isPrototypePreview: boolean,
  onPrototypeNodeActivate: (node: DesignNode) => void,
  onPrototypeNodeHover: (node: DesignNode) => void,
  onContextMenu: (event: ReactMouseEvent<HTMLButtonElement>, node: DesignNode) => void,
  editingTextNodeId: string | null,
  onStartTextEditing: (nodeId: string) => void,
  onFinishTextEditing: (nodeId: string, content: string) => void,
  editingTextRef: MutableRefObject<HTMLTextAreaElement | null>,
) {
  const style = {
    left: node.x,
    top: node.y,
    width: node.width,
    height: node.height,
    transform: `rotate(${node.rotation}deg)`,
    filter: node.effects.map((effect) => `drop-shadow(${effect.offsetX}px ${effect.offsetY}px ${effect.blur}px ${effect.color})`).join(' '),
  };
  const content = (() => {
    switch (node.type) {
      case 'frame':
        return <span className="design-editor-node-frame" style={{ ...designFillStyle(node, variablesById, imageUrls), borderRadius: designCornerRadiusCss(node), border: node.strokeWidth > 0 ? `${node.strokeWidth}px solid ${node.stroke}` : 'none' }} />;
      case 'rectangle':
        return <span className="design-editor-node-rectangle" style={{ ...designFillStyle(node, variablesById, imageUrls), borderRadius: designCornerRadiusCss(node), border: node.strokeWidth > 0 ? `${node.strokeWidth}px solid ${node.stroke}` : 'none' }} />;
      case 'ellipse':
        return <span className="design-editor-node-ellipse" style={{ ...designFillStyle(node, variablesById, imageUrls), border: node.strokeWidth > 0 ? `${node.strokeWidth}px solid ${node.stroke}` : 'none' }} />;
      case 'path':
        return <svg aria-hidden="true" className="design-editor-node-path" preserveAspectRatio="none" viewBox={`0 0 ${node.width} ${node.height}`}><polygon fill={node.closed ? resolveDesignColor(variablesById, node.fillVariableId, node.fill) : 'none'} points={node.points.map((point) => `${point.x},${point.y}`).join(' ')} stroke={node.strokeWidth > 0 ? node.stroke : 'none'} strokeLinecap="round" strokeLinejoin="round" strokeWidth={node.strokeWidth} /></svg>;
      case 'text':
        return <span className="design-editor-node-text" style={{ color: resolveDesignColor(variablesById, node.colorVariableId, node.color), fontFamily: node.fontFamily ?? 'sans-serif', fontSize: node.fontSize, fontWeight: node.fontWeight, lineHeight: node.lineHeight ?? 1.2, letterSpacing: node.letterSpacing ?? 0, textAlign: node.textAlign ?? 'left' }}>{node.content}</span>;
      case 'imported-svg':
        return <img alt="" className="design-editor-node-svg" draggable={false} src={designNodeDataUrl(node)} />;
      case 'image': {
        const src = imageUrls.get(node.assetId);
        return src
          ? <img alt={node.altText} className="design-editor-node-svg" draggable={false} src={src} style={{ objectFit: node.fit }} />
          : <span className="design-editor-node-text" role="img" aria-label={node.altText}>{node.altText}</span>;
      }
    }
  })();
  if (node.type === 'text' && editingTextNodeId === node.id && !isPrototypePreview) {
    return <div className={`design-editor-node is-text${selected ? ' is-selected' : ''}`} data-design-node-id={node.id} key={node.id} style={style}>
      <textarea
        aria-label={node.name}
        className="design-editor-text-edit-input"
        defaultValue={node.content}
        onBlur={(event) => onFinishTextEditing(node.id, event.currentTarget.value)}
        onPointerDown={(event) => event.stopPropagation()}
        ref={editingTextRef}
        rows={1}
        spellCheck
        style={{ color: resolveDesignColor(variablesById, node.colorVariableId, node.color), fontFamily: node.fontFamily ?? 'sans-serif', fontSize: node.fontSize, fontWeight: node.fontWeight, lineHeight: node.lineHeight ?? 1.2, letterSpacing: node.letterSpacing ?? 0, textAlign: node.textAlign ?? 'left' }}
      />
      {selected && !node.locked && (['north-west', 'north-east', 'south-west', 'south-east'] as const).map((handle) => (
        <span aria-hidden="true" className={`design-editor-resize-handle is-${handle}`} data-resize-handle={handle} key={handle} />
      ))}
    </div>;
  }
  return (
    <button
      aria-label={node.name}
      aria-pressed={selected}
      className={`design-editor-node is-${node.type}${selected ? ' is-selected' : ''}${node.locked ? ' is-locked' : ''}`}
      data-design-node-id={node.id}
      key={node.id}
      onPointerDown={(event) => {
        if (isPrototypePreview) {
          event.preventDefault();
          event.stopPropagation();
          onPrototypeNodeActivate(node);
          return;
        }
        onPointerDown(event, node);
      }}
      onPointerEnter={() => {
        if (isPrototypePreview) onPrototypeNodeHover(node);
      }}
      onFocus={() => {
        if (isPrototypePreview) onPrototypeNodeHover(node);
      }}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onContextMenu={(event) => onContextMenu(event, node)}
      onDoubleClick={(event) => {
        if (node.type !== 'text' || node.locked || isPrototypePreview) return;
        event.preventDefault();
        event.stopPropagation();
        onStartTextEditing(node.id);
      }}
      style={style}
      type="button"
    >
      {content}
      {selected && !node.locked && (['north-west', 'north-east', 'south-west', 'south-east'] as const).map((handle) => (
        <span aria-hidden="true" className={`design-editor-resize-handle is-${handle}`} data-resize-handle={handle} key={handle} />
      ))}
    </button>
  );
}

function screenToCanvas(x: number, y: number, camera: Camera): { x: number; y: number } {
  return { x: (x - camera.x) / camera.scale, y: (y - camera.y) / camera.scale };
}

function resolveDesignColor(variablesById: ReadonlyMap<string, string>, variableId: string | undefined, fallback: string): string {
  return variableId ? variablesById.get(variableId) ?? fallback : fallback;
}

function orderDesignLayerTree(nodes: readonly DesignNode[]): Array<{ node: DesignNode; depth: number }> {
  const childrenByParent = new Map<string | null, DesignNode[]>();
  for (const node of nodes) {
    const children = childrenByParent.get(node.parentId) ?? [];
    children.push(node);
    childrenByParent.set(node.parentId, children);
  }

  const roots = childrenByParent.get(null) ?? [];
  const pending = roots.map((node) => ({ node, depth: 0 }));
  const ordered: Array<{ node: DesignNode; depth: number }> = [];
  while (pending.length > 0) {
    const entry = pending.pop();
    if (!entry) break;
    ordered.push(entry);
    const children = childrenByParent.get(entry.node.id) ?? [];
    for (const child of children) pending.push({ node: child, depth: entry.depth + 1 });
  }
  return ordered;
}

function readSafeDesignGenerationError(error: unknown): string | null {
  if (!(error instanceof Error)) return null;
  const safePrefixes = [
    'The design request is ',
    'The design references ',
    'The selected model ',
    'The selected reasoning ',
    'Select at least one ',
    'The selected project instructions ',
    'The streamed design response ',
    'The model ',
    'A generated ',
    'An attached image reference ',
  ];
  return safePrefixes.some((prefix) => error.message.startsWith(prefix)) ? error.message : null;
}

function designFrameNodeIds(document: DesignDocument, frameId: string): string[] {
  const frame = document.nodes.find((node) => node.id === frameId && node.type === 'frame');
  if (!frame) return [];
  const nodeIds = new Set([frame.id]);
  let hasNewDescendant = true;
  while (hasNewDescendant) {
    hasNewDescendant = false;
    for (const node of document.nodes) {
      if (node.pageId === frame.pageId && node.visible && node.parentId !== null && nodeIds.has(node.parentId) && !nodeIds.has(node.id)) {
        nodeIds.add(node.id);
        hasNewDescendant = true;
      }
    }
  }
  return [...nodeIds];
}

function designLayerStyle(depth: number): ReactCSSProperties & { '--design-layer-depth': string } {
  return { '--design-layer-depth': `${depth * 14}px` };
}

function isNodeInViewport(node: DesignNode, camera: Camera, viewport: CanvasViewport): boolean {
  const overscan = 320;
  const left = node.x * camera.scale + camera.x;
  const top = node.y * camera.scale + camera.y;
  const right = left + node.width * camera.scale;
  const bottom = top + node.height * camera.scale;
  return right >= -overscan && bottom >= -overscan && left <= viewport.width + overscan && top <= viewport.height + overscan;
}

function resizeSidebarByKey(
  event: ReactKeyboardEvent<HTMLDivElement>,
  width: number,
  setWidth: (width: number) => void,
  widthRef: MutableRefObject<number>,
  onSaveError: () => void,
) {
  let next: number | null = null;
  if (event.key === 'ArrowLeft') next = width - 12;
  if (event.key === 'ArrowRight') next = width + 12;
  if (event.key === 'Home') next = DESIGN_PANEL_MIN_WIDTH;
  if (event.key === 'End') next = DESIGN_PANEL_MAX_WIDTH;
  if (next === null) return;
  event.preventDefault();
  const clamped = clampDesignPanelWidth(next);
  widthRef.current = clamped;
  setWidth(clamped);
  void writeDesignPanelWidth(clamped).catch(onSaveError);
}

function resizeInspectorByKey(event: ReactKeyboardEvent<HTMLDivElement>, width: number, setWidth: (width: number) => void, onSave: (width: number) => void) {
  let next: number | null = null;
  if (event.key === 'ArrowLeft') next = width + 12;
  if (event.key === 'ArrowRight') next = width - 12;
  if (event.key === 'Home') next = DESIGN_INSPECTOR_MIN_WIDTH;
  if (event.key === 'End') next = DESIGN_INSPECTOR_MAX_WIDTH;
  if (next === null) return;
  event.preventDefault();
  const clamped = clampDesignInspectorWidth(next);
  setWidth(clamped);
  onSave(clamped);
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}

function nextUniqueDesignName(preferred: string, entries: readonly { name: string }[]): string {
  const existingNames = new Set(entries.map((entry) => entry.name.trim().toLocaleLowerCase()));
  if (!existingNames.has(preferred.trim().toLocaleLowerCase())) return preferred;
  let suffix = 2;
  while (existingNames.has(`${preferred} ${suffix}`.trim().toLocaleLowerCase())) suffix += 1;
  return `${preferred} ${suffix}`;
}

function readFiniteNumber(value: string, currentValue: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : currentValue;
}

function designExportFilename(name: string, extension: 'svg' | 'png' | 'pdf' | 'html' | 'ocdesign'): string {
  const baseName = Array.from(name.trim(), (character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return codePoint < 32 || /[<>:"/\\|?*]/.test(character) ? '-' : character;
  }).join('').slice(0, 100) || 'Design';
  return `${baseName}.${extension}`;
}

function readDesignSvgDimensions(svg: string): { width: number; height: number } {
  const match = /<svg\b[^>]*\bwidth="([^"]+)"[^>]*\bheight="([^"]+)"/.exec(svg);
  const width = match?.[1] ? Number(match[1]) : Number.NaN;
  const height = match?.[2] ? Number(match[2]) : Number.NaN;
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    throw new Error('The exported SVG dimensions are invalid.');
  }
  return { width, height };
}

function encodeBase64(bytes: Uint8Array): string {
  const chunkLength = 0x8000;
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += chunkLength) {
    const chunk = bytes.subarray(offset, Math.min(offset + chunkLength, bytes.length));
    binary += String.fromCharCode(...chunk);
  }
  return window.btoa(binary);
}

function downloadDesignBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = window.document.createElement('a');
  try {
    anchor.href = url;
    anchor.download = filename;
    window.document.body.append(anchor);
    anchor.click();
  } finally {
    anchor.remove();
    URL.revokeObjectURL(url);
  }
}

function waitForDesignStep(signal: AbortSignal, durationMs = 55): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => {
      signal.removeEventListener('abort', cancel);
      resolve();
    }, durationMs);
    const cancel = () => {
      window.clearTimeout(timer);
      signal.removeEventListener('abort', cancel);
      reject(new DOMException('Design generation was cancelled.', 'AbortError'));
    };
    signal.addEventListener('abort', cancel, { once: true });
    if (signal.aborted) cancel();
  });
}

function isResizeHandle(value: string | undefined): value is ResizeHandle {
  return value === 'north-west' || value === 'north-east' || value === 'south-west' || value === 'south-east';
}
