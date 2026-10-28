import {
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Download,
  Image as ImageIcon,
  LoaderCircle,
  Minus,
  Plus,
  RotateCcw,
  Sparkles,
  Trash2,
} from 'lucide-react';
import {
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';

import { Button, ColorPicker, Dialog, Select, Textarea } from '@/design-system';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import {
  addFrameComment,
  createFrame,
  type DesignCanvasBreakpoint,
  type DesignCanvasExportFormat,
  type DesignCanvasFrame,
  type DesignCanvasState,
  type DesignCanvasTokens,
  designCanvasDataUrl,
  designCanvasFontFamilies,
  designCanvasFrameForViewport,
  designCanvasSpacingUnits,
  designCanvasTypeScales,
  emptyDesignCanvas,
  exportDesignCanvasElement,
  exportDesignCanvasFrame,
  frameAsAssistantMessage,
  type GeneratedDesignCanvasFrame,
  listDesignCanvasElements,
  parseGeneratedDesignCanvas,
  readDesignCanvas,
  responsiveBreakpointForWidth,
  restoreResponsiveLayoutRevision,
  reviseFrame,
  saveDesignCanvas,
  setResponsiveLayout,
  settleDesignCanvasRequest,
  updateFrameTokens,
} from '@/modules/design/data/designCanvas';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { loadRouteParameters } from '@/modules/providers/data/modelRouteParameters';
import { type JsonObject, localServiceClient, type ServiceOperation } from '@/platform/local-service';

import './DesignCanvas.css';

interface DesignCanvasProps {
  onOpenImageStudio(): void;
  onOpenSettings(): void;
}

type CanvasLoadState = { kind: 'ready'; canvas: DesignCanvasState } | { kind: 'error' };
type CanvasFeedback =
  | 'none'
  | 'cancelled'
  | 'cancelFailed'
  | 'commentLimit'
  | 'commentRequired'
  | 'invalidOutput'
  | 'modelRequired'
  | 'promptRequired'
  | 'requestFailed'
  | 'saveFailed';
type CanvasFeedbackTranslationKey =
  | 'designCanvasCancelled'
  | 'designCanvasCancelFailed'
  | 'designCanvasCommentLimit'
  | 'designCanvasCommentRequired'
  | 'designCanvasInvalidOutput'
  | 'designCanvasModelRequired'
  | 'designCanvasPromptRequired'
  | 'designCanvasRequestFailed'
  | 'designCanvasSaveFailed';
type ExportTranslationKey = 'designCanvasExportSvg' | 'designCanvasExportHtml' | 'designCanvasExportReact';
type CanvasColorTokenKey = 'background' | 'surface' | 'text' | 'primary' | 'accent' | 'border';
type CanvasTokenTranslationKey =
  | 'designCanvasTokenBackground'
  | 'designCanvasTokenSurface'
  | 'designCanvasTokenText'
  | 'designCanvasTokenPrimary'
  | 'designCanvasTokenAccent'
  | 'designCanvasTokenBorder';

interface Camera {
  x: number;
  y: number;
  scale: number;
}

interface CanvasCancellationAttempt {
  operationId: number;
  result: Promise<boolean>;
}

type CanvasDrag =
  | { kind: 'pan'; pointerId: number; startX: number; startY: number; cameraX: number; cameraY: number }
  | {
      kind: 'frame';
      pointerId: number;
      frameId: string;
      startX: number;
      startY: number;
      frameX: number;
      frameY: number;
      moved: boolean;
    };

const defaultCamera: Camera = { x: 40, y: 64, scale: 0.8 };
const maximumPromptLength = 4_000;
const tokenKeys: CanvasColorTokenKey[] = ['background', 'surface', 'text', 'primary', 'accent', 'border'];
const tokenLabelKeys: Record<CanvasColorTokenKey, CanvasTokenTranslationKey> = {
  background: 'designCanvasTokenBackground',
  surface: 'designCanvasTokenSurface',
  text: 'designCanvasTokenText',
  primary: 'designCanvasTokenPrimary',
  accent: 'designCanvasTokenAccent',
  border: 'designCanvasTokenBorder',
};

export function DesignCanvas({ onOpenImageStudio, onOpenSettings }: DesignCanvasProps) {
  const { i18n, t } = useTranslation();
  const [loadState, setLoadState] = useState<CanvasLoadState>(() => loadCanvas());
  const canvas = loadState.kind === 'ready' ? loadState.canvas : emptyDesignCanvas();
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
  const [feedback, setFeedback] = useState<CanvasFeedback>('none');
  const [camera, setCamera] = useState<Camera>(defaultCamera);
  const [prompt, setPrompt] = useState('');
  const [comment, setComment] = useState('');
  const [commentPoint, setCommentPoint] = useState<{ x: number; y: number } | null>(null);
  const [isPlacingComment, setIsPlacingComment] = useState(false);
  const [selectedCommentId, setSelectedCommentId] = useState<string | null>(null);
  const [selectedModelKey, setSelectedModelKey] = useState<string | null>(null);
  const [operation, setOperation] = useState<ServiceOperation | null>(null);
  const [requestMode, setRequestMode] = useState<'generate' | 'reflow' | 'revise' | null>(null);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [tokenDraft, setTokenDraft] = useState<DesignCanvasTokens | null>(null);
  const [selectedElementId, setSelectedElementId] = useState<string | null>(null);
  const [responsivePreviewWidth, setResponsivePreviewWidth] = useState(360);
  const [modelRefreshFailed, setModelRefreshFailed] = useState(false);
  const canvasRef = useRef(canvas);
  const cameraRef = useRef(camera);
  const dragRef = useRef<CanvasDrag | null>(null);
  const activeOperationRef = useRef<ServiceOperation | null>(null);
  const cancellationAttemptRef = useRef<CanvasCancellationAttempt | null>(null);
  const mountedRef = useRef(true);
  const cancelRequestedRef = useRef(false);

  const models = useChatStore((state) => state.models);
  const selectedChatModelKey = useChatStore((state) => state.selectedModelKey);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelError = useChatStore((state) => state.modelError);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const selectedModel = models.find((model) => model.key === selectedModelKey) ?? null;
  const selectedFrame = canvas.frames.find((frame) => frame.id === canvas.selectedFrameId) ?? null;
  const activeBreakpoint = responsiveBreakpointForWidth(responsivePreviewWidth);
  const selectedViewportFrame = useMemo(
    () => (selectedFrame ? designCanvasFrameForViewport(selectedFrame, responsivePreviewWidth) : null),
    [selectedFrame, responsivePreviewWidth],
  );
  const selectedElements = useMemo(
    () => (selectedViewportFrame ? listDesignCanvasElements(selectedViewportFrame.svg) : []),
    [selectedViewportFrame],
  );
  const selectedElement = selectedElements.find((element) => element.id === selectedElementId) ?? null;
  const selectedTokens = selectedFrame ? (tokenDraft ?? selectedViewportFrame?.tokens ?? selectedFrame.tokens) : null;
  const previewFrame = selectedViewportFrame
    ? tokenDraft
      ? updateFrameTokens(selectedViewportFrame, tokenDraft)
      : selectedViewportFrame
    : null;
  const visibleRevisions =
    activeBreakpoint && selectedFrame?.responsiveLayouts[activeBreakpoint]
      ? selectedFrame.responsiveLayouts[activeBreakpoint].revisions
      : (selectedFrame?.revisions ?? []);
  const isSaving = saveState === 'saving';
  const isBusy = operation !== null || requestMode !== null;

  useEffect(() => {
    canvasRef.current = canvas;
  }, [canvas]);

  useEffect(() => {
    cameraRef.current = camera;
  }, [camera]);

  useEffect(() => {
    if (
      selectedModelKey === null &&
      selectedChatModelKey &&
      models.some((model) => model.key === selectedChatModelKey)
    ) {
      setSelectedModelKey(selectedChatModelKey);
    }
  }, [models, selectedChatModelKey, selectedModelKey]);

  useEffect(() => {
    if (modelStatus !== 'idle') return;
    void refreshModels().catch(() => {
      if (mountedRef.current) setModelRefreshFailed(true);
    });
  }, [modelStatus, refreshModels]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      const activeOperation = activeOperationRef.current;
      if (activeOperation) {
        void activeOperation.cancel().catch((cause: unknown) => {
          globalThis.reportError(
            new Error('The design canvas request could not be cancelled during navigation.', { cause }),
          );
        });
      }
    };
  }, []);

  const commitCanvas = async (next: DesignCanvasState): Promise<boolean> => {
    canvasRef.current = next;
    setLoadState({ kind: 'ready', canvas: next });
    setSaveState('saving');
    setFeedback('none');
    try {
      await saveDesignCanvas(next);
      if (mountedRef.current) setSaveState('saved');
      return true;
    } catch {
      if (mountedRef.current) {
        setSaveState('error');
        setFeedback('saveFailed');
      }
      return false;
    }
  };

  const updateFrame = async (frame: DesignCanvasFrame): Promise<boolean> => {
    const next = {
      ...canvasRef.current,
      frames: canvasRef.current.frames.map((item) => (item.id === frame.id ? frame : item)),
    };
    return commitCanvas(next);
  };

  const beginCancellation = (request: ServiceOperation): Promise<boolean> => {
    const activeAttempt = cancellationAttemptRef.current;
    if (activeAttempt?.operationId === request.id) return activeAttempt.result;

    const attempt: CanvasCancellationAttempt = {
      operationId: request.id,
      result: Promise.resolve(false),
    };
    attempt.result = Promise.resolve()
      .then(() => request.cancel())
      .then((cancelled) => {
        if (!cancelled) {
          cancelRequestedRef.current = false;
          if (mountedRef.current) setFeedback('cancelFailed');
        }
        return cancelled;
      })
      .catch(() => {
        cancelRequestedRef.current = false;
        if (mountedRef.current) setFeedback('cancelFailed');
        return false;
      });
    cancellationAttemptRef.current = attempt;
    void attempt.result.then((cancelled) => {
      if (!cancelled && cancellationAttemptRef.current === attempt) {
        cancellationAttemptRef.current = null;
      }
    });
    return attempt.result;
  };

  const sendCanvasRequest = async (
    model: ChatModel,
    messages: Array<{ role: 'user' | 'assistant'; content: string }>,
  ): Promise<string | null> => {
    if (model.providerId === 'chatgpt' && (!model.connectionId || !model.workspaceId)) {
      setFeedback('modelRequired');
      return null;
    }
    let routeParameters: JsonObject;
    try {
      routeParameters = loadRouteParameters(model);
    } catch {
      setFeedback('modelRequired');
      return null;
    }
    const conversationId = `tmp_${crypto.randomUUID().replaceAll('-', '')}`;
    let operationId: number | null = null;
    try {
      const request = await localServiceClient.startOperation('chat.temporary.send', {
        conversationId,
        providerId: model.providerId,
        modelId: model.modelId,
        messages,
        ...routeParameters,
        ...(model.connectionId ? { connectionId: model.connectionId } : {}),
        ...(model.workspaceId ? { workspaceId: model.workspaceId } : {}),
      });
      if (!mountedRef.current) {
        await request.cancel();
        return null;
      }
      operationId = request.id;
      activeOperationRef.current = request;
      setOperation(request);
      if (cancelRequestedRef.current) void beginCancellation(request);
      const outcome = await settleDesignCanvasRequest(request.result, () => {
        const cancellation = cancellationAttemptRef.current;
        return cancellation?.operationId === request.id ? cancellation.result : null;
      });
      if (!mountedRef.current) return null;
      if (outcome.kind === 'cancelled') {
        setFeedback('cancelled');
        return null;
      }
      if (outcome.kind === 'failed') {
        setFeedback('requestFailed');
        return null;
      }
      const response = outcome.value;
      if (response.conversationId !== conversationId || typeof response.content !== 'string') {
        setFeedback('invalidOutput');
        return null;
      }
      return response.content;
    } catch {
      if (!mountedRef.current) return null;
      setFeedback('requestFailed');
      return null;
    } finally {
      activeOperationRef.current = null;
      if (operationId !== null && cancellationAttemptRef.current?.operationId === operationId) {
        cancellationAttemptRef.current = null;
      }
      if (mountedRef.current) setOperation(null);
      cancelRequestedRef.current = false;
    }
  };

  const generateFrame = async () => {
    const description = prompt.trim();
    if (!description) {
      setFeedback('promptRequired');
      return;
    }
    if (!selectedModel) {
      setFeedback('modelRequired');
      return;
    }
    if (canvasRef.current.frames.length >= 16) {
      setFeedback('saveFailed');
      return;
    }
    setFeedback('none');
    setRequestMode('generate');
    const content = await sendCanvasRequest(selectedModel, [{ role: 'user', content: buildDesignPrompt(description) }]);
    if (!mountedRef.current) return;
    setRequestMode(null);
    if (content === null) return;
    let generated: GeneratedDesignCanvasFrame;
    try {
      generated = parseGeneratedDesignCanvas(content);
    } catch {
      setFeedback('invalidOutput');
      return;
    }
    const offset = canvasRef.current.frames.length * 28;
    const frame = createFrame(generated, description, offset, offset);
    await commitCanvas({
      ...canvasRef.current,
      frames: [...canvasRef.current.frames, frame],
      selectedFrameId: frame.id,
    });
    if (!mountedRef.current) return;
    setPrompt('');
    setTokenDraft(null);
    setCamera(defaultCamera);
  };

  const addRevision = async () => {
    const note = comment.trim();
    if (!note) {
      setFeedback('commentRequired');
      return;
    }
    if (!selectedFrame || !selectedModel) {
      setFeedback('modelRequired');
      return;
    }
    const responsiveLayout = activeBreakpoint ? selectedFrame.responsiveLayouts[activeBreakpoint] : undefined;
    const revisionTarget = responsiveLayout
      ? designCanvasFrameForViewport(selectedFrame, responsivePreviewWidth)
      : selectedFrame;
    let nextComment: DesignCanvasFrame;
    const previousNote = selectedFrame.comments.at(-1);
    const samePoint =
      previousNote?.point === null && commentPoint === null
        ? true
        : previousNote?.point !== null &&
          previousNote?.point !== undefined &&
          commentPoint !== null &&
          previousNote.point.x === commentPoint.x &&
          previousNote.point.y === commentPoint.y;
    if (previousNote?.text === note && samePoint) {
      nextComment = selectedFrame;
    } else {
      try {
        nextComment = addFrameComment(selectedFrame, note, commentPoint);
      } catch {
        setFeedback('commentLimit');
        return;
      }
    }
    const savedComment = await updateFrame(nextComment);
    if (!savedComment || !mountedRef.current) return;
    setRequestMode('revise');
    const message = buildRevisionMessage(note, revisionTarget);
    const content = await sendCanvasRequest(selectedModel, [
      { role: 'user', content: buildDesignPrompt(selectedFrame.prompt) },
      { role: 'assistant', content: frameAsAssistantMessage(revisionTarget) },
      { role: 'user', content: message },
    ]);
    if (!mountedRef.current) return;
    setRequestMode(null);
    if (content === null) return;
    let generated: GeneratedDesignCanvasFrame;
    try {
      generated = parseGeneratedDesignCanvas(content);
    } catch {
      setFeedback('invalidOutput');
      return;
    }
    if (generated.width !== revisionTarget.width || generated.height !== revisionTarget.height) {
      setFeedback('invalidOutput');
      return;
    }
    const revised =
      activeBreakpoint && responsiveLayout
        ? setResponsiveLayout(nextComment, activeBreakpoint, generated)
        : reviseFrame(nextComment, generated);
    const savedRevision = await updateFrame(revised);
    if (savedRevision && mountedRef.current) {
      setComment('');
      setCommentPoint(null);
      setSelectedCommentId(null);
    }
  };

  const cancelRequest = async () => {
    cancelRequestedRef.current = true;
    const activeOperation = activeOperationRef.current;
    if (!activeOperation) return;
    await beginCancellation(activeOperation);
  };

  const retrySave = async () => {
    setSaveState('saving');
    setFeedback('none');
    try {
      await saveDesignCanvas(canvasRef.current);
      if (mountedRef.current) setSaveState('saved');
    } catch {
      if (mountedRef.current) {
        setSaveState('error');
        setFeedback('saveFailed');
      }
    }
  };

  const selectFrame = async (frameId: string) => {
    if (frameId === canvasRef.current.selectedFrameId || isSaving || isBusy) return;
    setTokenDraft(null);
    setCommentPoint(null);
    setIsPlacingComment(false);
    setSelectedCommentId(null);
    setSelectedElementId(null);
    await commitCanvas({ ...canvasRef.current, selectedFrameId: frameId });
  };

  const handleFrameClick = (event: ReactMouseEvent<HTMLButtonElement>, frame: DesignCanvasFrame) => {
    if (!isPlacingComment) {
      void selectFrame(frame.id);
      return;
    }
    if (frame.id !== canvasRef.current.selectedFrameId || !(event.target instanceof HTMLImageElement)) return;
    const bounds = event.target.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return;
    setCommentPoint({
      x: Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)),
      y: Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height)),
    });
    setIsPlacingComment(false);
  };

  const updateTokenDraft = <K extends keyof DesignCanvasTokens>(key: K, value: DesignCanvasTokens[K]) => {
    if (!selectedFrame) return;
    setTokenDraft({
      ...(tokenDraft ?? selectedViewportFrame?.tokens ?? selectedFrame.tokens),
      [key]: value,
    });
  };

  const updateColorTokenDraft = (key: CanvasColorTokenKey, value: string) => {
    const color = value.toUpperCase();
    switch (key) {
      case 'background':
        updateTokenDraft('background', color);
        break;
      case 'surface':
        updateTokenDraft('surface', color);
        break;
      case 'text':
        updateTokenDraft('text', color);
        break;
      case 'primary':
        updateTokenDraft('primary', color);
        break;
      case 'accent':
        updateTokenDraft('accent', color);
        break;
      case 'border':
        updateTokenDraft('border', color);
        break;
    }
  };

  const saveTokenDraft = async () => {
    if (!selectedFrame || !tokenDraft) return;
    const saved = await updateFrame(updateFrameTokens(selectedFrame, tokenDraft));
    if (saved && mountedRef.current) setTokenDraft(null);
  };

  const moveSelectedFrame = async (dx: number, dy: number) => {
    if (!selectedFrame || isSaving) return;
    const moved = {
      ...selectedFrame,
      x: clampPosition(selectedFrame.x + dx),
      y: clampPosition(selectedFrame.y + dy),
    };
    await updateFrame(moved);
  };

  const deleteSelectedFrame = async () => {
    if (!selectedFrame) return;
    const remaining = canvasRef.current.frames.filter((frame) => frame.id !== selectedFrame.id);
    await commitCanvas({
      ...canvasRef.current,
      frames: remaining,
      selectedFrameId: remaining[0]?.id ?? null,
    });
    if (!mountedRef.current) return;
    setTokenDraft(null);
    setDeleteDialogOpen(false);
  };

  const restoreRevision = async (index: number) => {
    if (!selectedFrame || isSaving) return;
    if (activeBreakpoint && selectedFrame.responsiveLayouts[activeBreakpoint]) {
      await updateFrame(restoreResponsiveLayoutRevision(selectedFrame, activeBreakpoint, index));
      return;
    }
    const revision = selectedFrame.revisions[index];
    if (!revision) return;
    const next = reviseFrame(selectedFrame, revision);
    next.revisions = selectedFrame.revisions.filter((_, revisionIndex) => revisionIndex !== index);
    await updateFrame(next);
  };

  const downloadFrame = (format: DesignCanvasExportFormat) => {
    if (!selectedFrame) return;
    const content = exportDesignCanvasFrame(
      designCanvasFrameForViewport(selectedFrame, responsivePreviewWidth),
      format,
    );
    const filenameBase = filename(selectedFrame.title);
    const extension = format === 'react' ? 'tsx' : format === 'html' ? 'html' : 'svg';
    const mimeType = format === 'svg' ? 'image/svg+xml' : format === 'html' ? 'text/html' : 'text/plain';
    const url = URL.createObjectURL(new Blob([content], { type: `${mimeType};charset=utf-8` }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `${filenameBase}.${extension}`;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
  };

  const downloadSelectedElement = () => {
    if (!selectedFrame || !selectedViewportFrame || !selectedElement) return;
    const content = exportDesignCanvasElement(selectedViewportFrame, selectedElement.id);
    const url = URL.createObjectURL(new Blob([content], { type: 'image/svg+xml;charset=utf-8' }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `${filename(selectedFrame.title)}-${selectedElement.id}.svg`;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
  };

  const generateResponsiveLayout = async () => {
    if (!selectedFrame || !activeBreakpoint) return;
    if (!selectedModel) {
      setFeedback('modelRequired');
      return;
    }
    const sourceFrame = designCanvasFrameForViewport(selectedFrame, responsivePreviewWidth);
    const width = activeBreakpoint === 'mobile' ? 390 : 768;
    const height = Math.max(320, Math.min(1600, Math.round((sourceFrame.height * width) / sourceFrame.width)));
    setFeedback('none');
    setRequestMode('reflow');
    const content = await sendCanvasRequest(selectedModel, [
      { role: 'user', content: buildDesignPrompt(selectedFrame.prompt) },
      { role: 'assistant', content: frameAsAssistantMessage(sourceFrame) },
      { role: 'user', content: buildResponsiveReflowMessage(activeBreakpoint, width, height) },
    ]);
    if (!mountedRef.current) return;
    setRequestMode(null);
    if (content === null) return;
    let generated: GeneratedDesignCanvasFrame;
    try {
      generated = parseGeneratedDesignCanvas(content);
    } catch {
      setFeedback('invalidOutput');
      return;
    }
    if (generated.width !== width || generated.height !== height) {
      setFeedback('invalidOutput');
      return;
    }
    const updated = setResponsiveLayout(selectedFrame, activeBreakpoint, generated);
    const saved = await updateFrame(updated);
    if (saved && mountedRef.current) setTokenDraft(null);
  };

  const onCanvasPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget || event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      kind: 'pan',
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      cameraX: cameraRef.current.x,
      cameraY: cameraRef.current.y,
    };
  };

  const onFramePointerDown = (event: ReactPointerEvent<HTMLButtonElement>, frame: DesignCanvasFrame) => {
    if (event.button !== 0 || isBusy || isSaving || isPlacingComment) return;
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      kind: 'frame',
      pointerId: event.pointerId,
      frameId: frame.id,
      startX: event.clientX,
      startY: event.clientY,
      frameX: frame.x,
      frameY: frame.y,
      moved: false,
    };
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement | HTMLButtonElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (drag.kind === 'pan') {
      setCamera((current) => ({
        ...current,
        x: drag.cameraX + event.clientX - drag.startX,
        y: drag.cameraY + event.clientY - drag.startY,
      }));
      return;
    }
    const scale = cameraRef.current.scale;
    const deltaX = (event.clientX - drag.startX) / scale;
    const deltaY = (event.clientY - drag.startY) / scale;
    if (Math.abs(deltaX) < 1 && Math.abs(deltaY) < 1) return;
    drag.moved = true;
    const x = clampPosition(drag.frameX + deltaX);
    const y = clampPosition(drag.frameY + deltaY);
    const next: DesignCanvasState = {
      ...canvasRef.current,
      frames: canvasRef.current.frames.map((frame) => (frame.id === drag.frameId ? { ...frame, x, y } : frame)),
    };
    canvasRef.current = next;
    setLoadState({ kind: 'ready', canvas: next });
  };

  const onPointerUp = () => {
    const drag = dragRef.current;
    if (!drag) return;
    dragRef.current = null;
    if (drag.kind === 'frame' && drag.moved) void retrySave();
  };

  const onCanvasWheel = (event: ReactWheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    const viewport = event.currentTarget.getBoundingClientRect();
    const current = cameraRef.current;
    const nextScale = Math.max(0.35, Math.min(2.5, current.scale * (event.deltaY < 0 ? 1.08 : 1 / 1.08)));
    const pointX = event.clientX - viewport.left;
    const pointY = event.clientY - viewport.top;
    const worldX = (pointX - current.x) / current.scale;
    const worldY = (pointY - current.y) / current.scale;
    setCamera({ x: pointX - worldX * nextScale, y: pointY - worldY * nextScale, scale: nextScale });
  };

  const feedbackMessage = feedbackKey(feedback);

  if (loadState.kind === 'error') {
    return (
      <div className="grid h-full place-items-center bg-background p-6">
        <section aria-labelledby="design-canvas-load-title" className="grid max-w-lg gap-4 text-center">
          <h1 className="text-section-title font-semibold" id="design-canvas-load-title">
            {t('designCanvasTitle')}
          </h1>
          <p className="text-sm text-destructive" role="alert">
            {t('designCanvasLoadFailed')}
          </p>
          <Button
            onClick={() => {
              try {
                setLoadState({ kind: 'ready', canvas: readDesignCanvas() });
                setSaveState('idle');
              } catch {
                setLoadState({ kind: 'error' });
              }
            }}
            variant="secondary"
          >
            <RotateCcw aria-hidden="true" size={16} />
            {t('retry')}
          </Button>
        </section>
      </div>
    );
  }

  return (
    <div className="design-vector-editor">
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-surface px-4 py-3 sm:px-6">
        <div className="min-w-0">
          <h1 className="text-section-title font-semibold text-text">{t('designCanvasTitle')}</h1>
          <p className="mt-1 max-w-3xl text-sm text-secondary-text">{t('designCanvasDescription')}</p>
        </div>
        <Button onClick={onOpenImageStudio} size="small" variant="secondary">
          <ImageIcon aria-hidden="true" size={16} />
          {t('designCanvasImageStudio')}
        </Button>
      </header>

      <div className="design-vector-layout">
        <main className="design-vector-main min-w-0">
          <div className="design-vector-prompt-row grid shrink-0 gap-3 border-b border-border bg-raised p-3 sm:p-4">
            <label className="grid min-w-0 gap-2 text-sm font-medium text-text" htmlFor="design-canvas-prompt">
              {t('designCanvasPrompt')}
              <Textarea
                aria-describedby="design-canvas-prompt-hint"
                className="min-h-20 w-full resize-y rounded-control border border-control-border bg-surface px-3 py-2 text-sm font-normal leading-relaxed text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
                disabled={isBusy || isSaving}
                id="design-canvas-prompt"
                maxLength={maximumPromptLength}
                onChange={(event) => setPrompt(event.target.value)}
                placeholder={t('designCanvasPromptPlaceholder')}
                value={prompt}
              />
              <span className="text-xs font-normal text-secondary-text" id="design-canvas-prompt-hint">
                {t('designCanvasPromptHint', { count: prompt.length, max: maximumPromptLength })}
              </span>
            </label>
            <div className="grid content-start gap-3">
              <Select
                disabled={isBusy || isSaving || models.length === 0}
                label={t('designCanvasModel')}
                onChange={(event) => {
                  setSelectedModelKey(event.target.value || null);
                  setFeedback('none');
                }}
                options={models.map((model) => ({
                  value: model.key,
                  label: `${model.displayName} · ${model.sourceLabel ?? model.providerId}`,
                }))}
                placeholder={t('designCanvasChooseModel')}
                value={selectedModelKey ?? ''}
              />
              {modelStatus === 'loading' ? (
                <p aria-live="polite" className="flex items-center gap-2 text-xs text-secondary-text" role="status">
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={14} />
                  {t('designCanvasLoadingModels')}
                </p>
              ) : null}
              {modelStatus === 'error' || modelError || modelRefreshFailed ? (
                <div className="grid gap-2" role="alert">
                  <p className="text-xs text-destructive">{t('designCanvasModelsFailed')}</p>
                  <Button
                    onClick={() => {
                      setModelRefreshFailed(false);
                      void refreshModels(true).catch(() => {
                        if (mountedRef.current) setModelRefreshFailed(true);
                      });
                    }}
                    size="small"
                    variant="secondary"
                  >
                    <RotateCcw aria-hidden="true" size={14} />
                    {t('retry')}
                  </Button>
                </div>
              ) : null}
              {models.length === 0 && modelStatus === 'ready' ? (
                <div className="grid gap-2">
                  <p className="text-xs text-secondary-text">{t('designCanvasNoModels')}</p>
                  <Button onClick={onOpenSettings} size="small" variant="secondary">
                    {t('designCanvasOpenSettings')}
                  </Button>
                </div>
              ) : null}
              <div className="flex flex-wrap gap-2">
                {isBusy ? (
                  <Button loading size="small" variant="primary">
                    {t(
                      requestMode === 'revise'
                        ? 'designCanvasRevising'
                        : requestMode === 'reflow'
                          ? 'designCanvasReflowing'
                          : 'designCanvasGenerating',
                    )}
                  </Button>
                ) : (
                  <Button
                    disabled={!selectedModel || isSaving || canvas.frames.length >= 16}
                    onClick={() => void generateFrame()}
                    size="small"
                    variant="primary"
                  >
                    <Sparkles aria-hidden="true" size={16} />
                    {t('designCanvasGenerate')}
                  </Button>
                )}
                {isBusy ? (
                  <Button onClick={() => void cancelRequest()} size="small" variant="secondary">
                    {t('designCanvasCancel')}
                  </Button>
                ) : null}
                {canvas.frames.length >= 16 ? (
                  <p className="basis-full text-xs text-secondary-text">{t('designCanvasFrameLimit')}</p>
                ) : null}
              </div>
              {feedbackMessage ? (
                <p
                  aria-live="polite"
                  className={feedback === 'saveFailed' ? 'text-sm text-destructive' : 'text-sm text-secondary-text'}
                  role="status"
                >
                  {t(feedbackMessage)}
                </p>
              ) : null}
            </div>
          </div>

          <section
            aria-label={t('designCanvasWorkspace')}
            className="design-vector-canvas"
            onPointerDown={onCanvasPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerUp}
            onWheel={onCanvasWheel}
          >
            <div className="absolute right-3 top-3 z-10 flex items-center gap-1 rounded-control border border-border bg-raised p-1 shadow-soft">
              <Button
                aria-label={t('designCanvasZoomOut')}
                disabled={camera.scale <= 0.35}
                onClick={() => setCamera((current) => ({ ...current, scale: Math.max(0.35, current.scale / 1.15) }))}
                size="icon"
                variant="ghost"
              >
                <Minus aria-hidden="true" size={16} />
              </Button>
              <span aria-live="polite" className="min-w-12 text-center text-xs tabular-nums text-secondary-text">
                {Math.round(camera.scale * 100)}%
              </span>
              <Button
                aria-label={t('designCanvasZoomIn')}
                disabled={camera.scale >= 2.5}
                onClick={() => setCamera((current) => ({ ...current, scale: Math.min(2.5, current.scale * 1.15) }))}
                size="icon"
                variant="ghost"
              >
                <Plus aria-hidden="true" size={16} />
              </Button>
              <Button
                aria-label={t('designCanvasResetView')}
                onClick={() => setCamera(defaultCamera)}
                size="icon"
                variant="ghost"
              >
                <RotateCcw aria-hidden="true" size={15} />
              </Button>
            </div>

            {canvas.frames.length === 0 ? (
              <div className="pointer-events-none absolute inset-0 grid place-items-center p-6 text-center">
                <div className="max-w-sm">
                  <p className="text-sm font-medium text-text">{t('designCanvasEmptyTitle')}</p>
                  <p className="mt-2 text-sm text-secondary-text">{t('designCanvasEmptyDescription')}</p>
                </div>
              </div>
            ) : null}

            <div
              className="absolute left-0 top-0 origin-top-left"
              style={{ transform: `translate(${camera.x}px, ${camera.y}px) scale(${camera.scale})` }}
            >
              {canvas.frames.map((frame) => (
                <button
                  aria-label={t('designCanvasSelectFrame', { title: frame.title })}
                  aria-pressed={frame.id === canvas.selectedFrameId}
                  className="design-vector-frame absolute block overflow-hidden border bg-surface p-0 text-left focus-visible:z-10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  key={frame.id}
                  onClick={(event) => handleFrameClick(event, frame)}
                  onPointerDown={(event) => onFramePointerDown(event, frame)}
                  style={{
                    left: frame.x,
                    top: frame.y,
                    width: frame.id === selectedFrame?.id ? responsivePreviewWidth : 360,
                  }}
                  type="button"
                >
                  <img
                    alt=""
                    className="block h-auto w-full select-none"
                    draggable={false}
                    src={designCanvasDataUrl(
                      frame.id === selectedFrame?.id ? (previewFrame?.svg ?? frame.svg) : frame.svg,
                    )}
                  />
                  <span className="block truncate border-t border-border px-3 py-2 text-left text-xs font-medium text-text">
                    {frame.title}
                  </span>
                </button>
              ))}
              {canvas.frames.flatMap((frame) =>
                frame.comments.flatMap((item, index) => {
                  if (!item.point) return [];
                  const isSelectedFrame = frame.id === selectedFrame?.id;
                  const renderedFrame = isSelectedFrame ? (previewFrame ?? frame) : frame;
                  const renderedWidth = isSelectedFrame ? responsivePreviewWidth : 360;
                  return [
                    <button
                      aria-label={t('designCanvasCommentMarker', { number: index + 1, text: item.text })}
                      className="design-vector-comment-marker absolute z-10 grid size-7 place-items-center rounded-full border-2 border-surface bg-accent text-xs font-semibold text-accent-foreground shadow-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                      key={item.id}
                      onClick={() => {
                        if (frame.id === canvasRef.current.selectedFrameId) {
                          setSelectedCommentId(item.id);
                          return;
                        }
                        void selectFrame(frame.id).then(() => setSelectedCommentId(item.id));
                      }}
                      style={{
                        left: frame.x + item.point.x * renderedWidth,
                        top: frame.y + item.point.y * ((renderedWidth * renderedFrame.height) / renderedFrame.width),
                        transform: 'translate(-50%, -50%)',
                      }}
                      type="button"
                    >
                      {index + 1}
                    </button>,
                  ];
                }),
              )}
            </div>
          </section>
        </main>

        <aside aria-label={t('designCanvasInspector')} className="design-vector-sidebar">
          <div className="flex items-center justify-between gap-3 border-b border-border pb-3">
            <div className="min-w-0">
              <p className="text-sm font-semibold text-text">{selectedFrame?.title ?? t('designCanvasInspector')}</p>
              <p className="mt-1 text-xs text-secondary-text">
                {t('designCanvasFrameCount', { count: canvas.frames.length })}
              </p>
            </div>
            {selectedFrame ? (
              <Button
                aria-label={t('designCanvasDeleteFrame')}
                disabled={isBusy || isSaving}
                onClick={() => setDeleteDialogOpen(true)}
                size="icon"
                variant="ghost"
              >
                <Trash2 aria-hidden="true" size={16} />
              </Button>
            ) : null}
          </div>

          {saveState === 'saving' ? (
            <p aria-live="polite" className="flex items-center gap-2 text-xs text-secondary-text" role="status">
              <LoaderCircle aria-hidden="true" className="animate-spin" size={14} />
              {t('designCanvasSaving')}
            </p>
          ) : null}
          {saveState === 'saved' ? <p className="text-xs text-secondary-text">{t('designCanvasSaved')}</p> : null}
          {saveState === 'error' ? (
            <div className="grid gap-2" role="alert">
              <p className="text-xs text-destructive">{t('designCanvasSaveFailed')}</p>
              <Button onClick={() => void retrySave()} size="small" variant="secondary">
                <RotateCcw aria-hidden="true" size={14} />
                {t('retry')}
              </Button>
            </div>
          ) : null}

          {selectedFrame && previewFrame ? (
            <div className="grid gap-5">
              <section aria-labelledby="design-canvas-viewport-title" className="grid gap-2">
                <h2 className="text-sm font-semibold text-text" id="design-canvas-viewport-title">
                  {t('designCanvasViewport')}
                </h2>
                <label className="grid gap-2 text-sm text-secondary-text" htmlFor="design-canvas-viewport-width">
                  <span className="flex items-center justify-between gap-2">
                    <span>{t('designCanvasViewportWidth')}</span>
                    <output className="tabular-nums text-text" htmlFor="design-canvas-viewport-width">
                      {responsivePreviewWidth}px
                    </output>
                  </span>
                  <input
                    className="w-full accent-accent"
                    id="design-canvas-viewport-width"
                    max={1280}
                    min={320}
                    onChange={(event) => setResponsivePreviewWidth(Number(event.target.value))}
                    type="range"
                    value={responsivePreviewWidth}
                  />
                </label>
                <p aria-live="polite" className="text-xs text-secondary-text" role="status">
                  {activeBreakpoint
                    ? t(
                        selectedFrame.responsiveLayouts[activeBreakpoint]
                          ? 'designCanvasBreakpointSaved'
                          : 'designCanvasBreakpointFallback',
                      )
                    : t('designCanvasBreakpointBaseShown')}
                </p>
                {activeBreakpoint ? (
                  <Button
                    disabled={isBusy || isSaving || !selectedModel}
                    loading={requestMode === 'reflow'}
                    onClick={() => void generateResponsiveLayout()}
                    size="small"
                    variant="secondary"
                  >
                    <Sparkles aria-hidden="true" size={14} />
                    {t(
                      activeBreakpoint === 'mobile'
                        ? 'designCanvasGenerateMobileLayout'
                        : 'designCanvasGenerateTabletLayout',
                    )}
                  </Button>
                ) : null}
              </section>

              <section aria-labelledby="design-canvas-elements-title" className="grid gap-2">
                <h2 className="text-sm font-semibold text-text" id="design-canvas-elements-title">
                  {t('designCanvasElements')}
                </h2>
                {selectedElements.length > 0 ? (
                  <ol className="grid max-h-40 gap-1 overflow-y-auto">
                    {selectedElements.map((element, index) => (
                      <li key={element.id}>
                        <Button
                          aria-pressed={element.id === selectedElementId}
                          className="w-full justify-start"
                          onClick={() => setSelectedElementId(element.id)}
                          size="small"
                          variant={element.id === selectedElementId ? 'elevated' : 'ghost'}
                        >
                          {t('designCanvasElementLabel', { number: index + 1 })}
                        </Button>
                      </li>
                    ))}
                  </ol>
                ) : (
                  <p className="text-xs text-secondary-text">{t('designCanvasNoElements')}</p>
                )}
                <Button
                  disabled={isBusy || !selectedElement}
                  onClick={downloadSelectedElement}
                  size="small"
                  variant="secondary"
                >
                  <Download aria-hidden="true" size={14} />
                  {t('designCanvasExportElement')}
                </Button>
              </section>

              <section aria-labelledby="design-canvas-tokens-title" className="grid gap-3">
                <div className="flex items-center justify-between gap-3">
                  <h2 className="text-sm font-semibold text-text" id="design-canvas-tokens-title">
                    {t('designCanvasTokens')}
                  </h2>
                  {tokenDraft ? (
                    <Button disabled={isSaving} onClick={() => void saveTokenDraft()} size="small" variant="secondary">
                      {t('designCanvasSaveTokens')}
                    </Button>
                  ) : null}
                </div>
                <div className="grid grid-cols-2 gap-3">
                  {tokenKeys.map((key) => (
                    <ColorPicker
                      disabled={isBusy || isSaving}
                      key={key}
                      label={t(tokenLabelKeys[key])}
                      onChange={(value) => updateColorTokenDraft(key, value)}
                      value={selectedTokens?.[key] ?? ''}
                    />
                  ))}
                </div>
                <Select
                  disabled={isBusy || isSaving}
                  label={t('designCanvasFontFamily')}
                  onChange={(event) => {
                    const fontFamily = designCanvasFontFamilies.find((candidate) => candidate === event.target.value);
                    if (fontFamily) updateTokenDraft('fontFamily', fontFamily);
                  }}
                  options={designCanvasFontFamilies.map((fontFamily) => ({ value: fontFamily, label: fontFamily }))}
                  value={selectedTokens?.fontFamily ?? designCanvasFontFamilies[0]}
                />
                <Select
                  disabled={isBusy || isSaving}
                  label={t('designCanvasTypeScale')}
                  onChange={(event) => {
                    const typeScale = designCanvasTypeScales.find(
                      (candidate) => String(candidate) === event.target.value,
                    );
                    if (typeScale !== undefined) updateTokenDraft('typeScale', typeScale);
                  }}
                  options={designCanvasTypeScales.map((typeScale) => ({
                    value: String(typeScale),
                    label: `${typeScale} px`,
                  }))}
                  value={String(selectedTokens?.typeScale ?? designCanvasTypeScales[1])}
                />
                <Select
                  disabled={isBusy || isSaving}
                  label={t('designCanvasSpacingUnit')}
                  onChange={(event) => {
                    const spacingUnit = designCanvasSpacingUnits.find(
                      (candidate) => String(candidate) === event.target.value,
                    );
                    if (spacingUnit !== undefined) updateTokenDraft('spacingUnit', spacingUnit);
                  }}
                  options={designCanvasSpacingUnits.map((spacingUnit) => ({
                    value: String(spacingUnit),
                    label: `${spacingUnit} px`,
                  }))}
                  value={String(selectedTokens?.spacingUnit ?? designCanvasSpacingUnits[1])}
                />
                <p className="text-xs text-secondary-text">{t('designCanvasTokensDescription')}</p>
              </section>

              <section aria-labelledby="design-canvas-move-title" className="grid gap-2">
                <h2 className="text-sm font-semibold text-text" id="design-canvas-move-title">
                  {t('designCanvasPosition')}
                </h2>
                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    aria-label={t('designCanvasMoveLeft')}
                    disabled={isBusy || isSaving}
                    onClick={() => void moveSelectedFrame(-24, 0)}
                    size="icon"
                    variant="secondary"
                  >
                    <ArrowLeft aria-hidden="true" size={16} />
                  </Button>
                  <Button
                    aria-label={t('designCanvasMoveUp')}
                    disabled={isBusy || isSaving}
                    onClick={() => void moveSelectedFrame(0, -24)}
                    size="icon"
                    variant="secondary"
                  >
                    <ArrowUp aria-hidden="true" size={16} />
                  </Button>
                  <Button
                    aria-label={t('designCanvasMoveDown')}
                    disabled={isBusy || isSaving}
                    onClick={() => void moveSelectedFrame(0, 24)}
                    size="icon"
                    variant="secondary"
                  >
                    <ArrowDown aria-hidden="true" size={16} />
                  </Button>
                  <Button
                    aria-label={t('designCanvasMoveRight')}
                    disabled={isBusy || isSaving}
                    onClick={() => void moveSelectedFrame(24, 0)}
                    size="icon"
                    variant="secondary"
                  >
                    <ArrowRight aria-hidden="true" size={16} />
                  </Button>
                  <span className="text-xs tabular-nums text-secondary-text">
                    {Math.round(selectedFrame.x)}, {Math.round(selectedFrame.y)}
                  </span>
                </div>
              </section>

              <section aria-labelledby="design-canvas-export-title" className="grid gap-2">
                <h2 className="text-sm font-semibold text-text" id="design-canvas-export-title">
                  {t('designCanvasExport')}
                </h2>
                <div className="grid grid-cols-1 gap-2 sm:grid-cols-3 2xl:grid-cols-1">
                  {(['svg', 'html', 'react'] as const).map((format) => (
                    <Button
                      key={format}
                      disabled={isBusy}
                      onClick={() => downloadFrame(format)}
                      size="small"
                      variant="secondary"
                    >
                      <Download aria-hidden="true" size={14} />
                      {t(exportLabel(format))}
                    </Button>
                  ))}
                </div>
              </section>

              <section aria-labelledby="design-canvas-comments-title" className="grid gap-3">
                <h2 className="text-sm font-semibold text-text" id="design-canvas-comments-title">
                  {t('designCanvasComments')}
                </h2>
                <label className="grid gap-2 text-sm font-medium text-text" htmlFor="design-canvas-comment">
                  {t('designCanvasCommentLabel')}
                  <Textarea
                    className="min-h-20 w-full resize-y rounded-control border border-control-border bg-surface px-3 py-2 text-sm font-normal leading-relaxed text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
                    disabled={isBusy || isSaving}
                    id="design-canvas-comment"
                    maxLength={2_000}
                    onChange={(event) => setComment(event.target.value)}
                    placeholder={t('designCanvasCommentPlaceholder')}
                    value={comment}
                  />
                </label>
                <Button
                  aria-pressed={isPlacingComment}
                  disabled={isBusy || isSaving}
                  onClick={() => {
                    setCommentPoint(null);
                    setIsPlacingComment((placing) => !placing);
                  }}
                  size="small"
                  variant={isPlacingComment ? 'elevated' : 'secondary'}
                >
                  {t(isPlacingComment ? 'designCanvasCancelPlacement' : 'designCanvasPlaceNote')}
                </Button>
                {isPlacingComment ? (
                  <p aria-live="polite" className="text-xs text-secondary-text" role="status">
                    {t('designCanvasPlacementHint')}
                  </p>
                ) : commentPoint ? (
                  <p aria-live="polite" className="text-xs text-secondary-text" role="status">
                    {t('designCanvasNotePinned')}
                  </p>
                ) : null}
                <Button
                  disabled={isBusy || isSaving || !selectedModel}
                  loading={isBusy}
                  onClick={() => void addRevision()}
                  size="small"
                  variant="primary"
                >
                  <Sparkles aria-hidden="true" size={15} />
                  {t('designCanvasRevise')}
                </Button>
                {selectedFrame.comments.length > 0 ? (
                  <ol aria-label={t('designCanvasComments')} className="grid max-h-40 gap-2 overflow-y-auto">
                    {selectedFrame.comments.map((item) => (
                      <li
                        className={`rounded-control border bg-surface px-3 py-2 text-sm text-text ${selectedCommentId === item.id ? 'border-accent' : 'border-border'}`}
                        key={item.id}
                      >
                        <p className="whitespace-pre-wrap break-words">{item.text}</p>
                        {item.point ? (
                          <span className="mt-1 block text-xs text-secondary-text">{t('designCanvasPinnedNote')}</span>
                        ) : null}
                        <time
                          className="mt-1 block text-xs text-secondary-text"
                          dateTime={new Date(item.createdAt).toISOString()}
                        >
                          {new Date(item.createdAt).toLocaleString(i18n.language)}
                        </time>
                      </li>
                    ))}
                  </ol>
                ) : (
                  <p className="text-xs text-secondary-text">{t('designCanvasNoComments')}</p>
                )}
              </section>

              {visibleRevisions.length > 0 ? (
                <section aria-labelledby="design-canvas-history-title" className="grid gap-2">
                  <h2 className="text-sm font-semibold text-text" id="design-canvas-history-title">
                    {t('designCanvasHistory')}
                  </h2>
                  <ol className="grid gap-2">
                    {visibleRevisions.map((revision, index) => (
                      <li className="flex items-center justify-between gap-2" key={revision.id}>
                        <span className="min-w-0 truncate text-xs text-secondary-text">
                          {t('designCanvasRevisionLabel', { number: visibleRevisions.length - index })}
                        </span>
                        <Button
                          disabled={isBusy || isSaving}
                          onClick={() => void restoreRevision(index)}
                          size="small"
                          variant="ghost"
                        >
                          <RotateCcw aria-hidden="true" size={13} />
                          {t('designCanvasRestoreRevision')}
                        </Button>
                      </li>
                    ))}
                  </ol>
                </section>
              ) : null}
            </div>
          ) : (
            <div className="grid gap-2 py-3">
              <p className="text-sm font-medium text-text">{t('designCanvasNoSelection')}</p>
              <p className="text-sm text-secondary-text">{t('designCanvasNoSelectionDescription')}</p>
            </div>
          )}
        </aside>
      </div>

      <Dialog onOpenChange={setDeleteDialogOpen} open={deleteDialogOpen} title={t('designCanvasDeleteTitle')}>
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('designCanvasDeleteDescription')}</p>
          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={() => setDeleteDialogOpen(false)} variant="secondary">
              {t('cancel')}
            </Button>
            <Button disabled={isSaving} onClick={() => void deleteSelectedFrame()} variant="destructive">
              <Trash2 aria-hidden="true" size={15} />
              {t('designCanvasDeleteConfirm')}
            </Button>
          </div>
        </div>
      </Dialog>
    </div>
  );

  function feedbackKey(value: CanvasFeedback): CanvasFeedbackTranslationKey | null {
    switch (value) {
      case 'none':
        return null;
      case 'cancelled':
        return 'designCanvasCancelled';
      case 'cancelFailed':
        return 'designCanvasCancelFailed';
      case 'commentLimit':
        return 'designCanvasCommentLimit';
      case 'commentRequired':
        return 'designCanvasCommentRequired';
      case 'invalidOutput':
        return 'designCanvasInvalidOutput';
      case 'modelRequired':
        return 'designCanvasModelRequired';
      case 'promptRequired':
        return 'designCanvasPromptRequired';
      case 'requestFailed':
        return 'designCanvasRequestFailed';
      case 'saveFailed':
        return 'designCanvasSaveFailed';
    }
  }
}

function loadCanvas(): CanvasLoadState {
  try {
    return { kind: 'ready', canvas: readDesignCanvas() };
  } catch {
    return { kind: 'error' };
  }
}

function buildDesignPrompt(description: string): string {
  return [
    'Create one polished, responsive interface design as a vector SVG frame.',
    'Return only a JSON object with exactly these fields: title, width, height, tokens, svg.',
    'Use integer width and height values from 320 to 1600. tokens must contain six hex colors (background, surface, text, primary, accent, border), fontFamily (Source Sans 3, Arial, Georgia, or monospace), typeScale (14, 16, or 18), and spacingUnit (4, 8, or 12).',
    'The SVG root must be <svg>. Use only svg, g, rect, circle, ellipse, line, polyline, polygon, path, text, and tspan elements.',
    'Use fill and stroke colors only as var(--oc-background), var(--oc-surface), var(--oc-text), var(--oc-primary), var(--oc-accent), var(--oc-border), none, or transparent.',
    'Use font-family="var(--oc-font-family)" and font-size values from var(--oc-type-small), var(--oc-type-body), var(--oc-type-heading), or var(--oc-type-display). Use var(--oc-space-1), var(--oc-space-2), var(--oc-space-3), var(--oc-space-4), or var(--oc-space-6) for spacing values where appropriate.',
    'Use only basic vector geometry and text. Do not include scripts, links, images, style attributes, IDs, references, external resources, or unsupported SVG attributes.',
    'Use Source Sans 3 for text where appropriate. Keep the layout readable at the full frame size and use the supplied description as the design brief.',
    `Design brief:\n${description}`,
  ].join('\n\n');
}

function buildRevisionMessage(note: string, frame: DesignCanvasFrame): string {
  return [
    "Revise the current design to address the user's visual feedback. Preserve the existing frame dimensions and token names. Return only the same JSON structure: title, width, height, tokens, svg.",
    'Keep every SVG element, attribute, and color within the restrictions in the original request. Do not include code fences or explanation.',
    `User feedback:\n${note}`,
    `The current frame is ${frame.width} by ${frame.height}.`,
  ].join('\n\n');
}

function buildResponsiveReflowMessage(breakpoint: DesignCanvasBreakpoint, width: number, height: number): string {
  const label = breakpoint === 'mobile' ? 'phone' : 'tablet';
  return [
    `Create a separately reflowed ${label} layout for the same design at exactly ${width} by ${height} pixels.`,
    'Reorganize the content for this viewport instead of scaling the existing arrangement. Stack groups when needed, keep controls readable and reachable, preserve the content and visual identity, and avoid clipping or horizontal overflow.',
    'Return only the supported JSON structure: title, width, height, tokens, svg. Preserve the existing token names and keep every SVG element, attribute, and color within the restrictions in the original request. Do not include code fences or explanation.',
  ].join('\n\n');
}

function exportLabel(format: DesignCanvasExportFormat): ExportTranslationKey {
  switch (format) {
    case 'svg':
      return 'designCanvasExportSvg';
    case 'html':
      return 'designCanvasExportHtml';
    case 'react':
      return 'designCanvasExportReact';
  }
}

function filename(value: string): string {
  const safe = value
    .normalize('NFKD')
    .replace(/[^A-Za-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .toLowerCase();
  return safe || 'design-frame';
}

function clampPosition(value: number): number {
  return Math.max(-100_000, Math.min(100_000, Math.round(value)));
}
