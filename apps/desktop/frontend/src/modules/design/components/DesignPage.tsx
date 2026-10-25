import type { TFunction } from 'i18next';
import {
  ArrowDownToLine,
  ChevronDown,
  FileText,
  Filter,
  FolderOpen,
  Image,
  Images,
  LoaderCircle,
  MessageSquareText,
  PanelsTopLeft,
  RefreshCw,
  Scaling,
  Settings2,
  SlidersHorizontal,
  Sparkles,
  UserRound,
  WandSparkles,
} from 'lucide-react';
import {
  type ChangeEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';

import { Button, DatePicker, Dialog, Select } from '@/design-system';
import type { ConversationAttachmentSummary } from '@/modules/conversations/data/chatStorage';
import { listConversationAttachments, readMessageAttachment } from '@/modules/conversations/data/chatStorage';
import {
  clampDesignPanelWidth,
  DESIGN_PANEL_DEFAULT_WIDTH,
  DESIGN_PANEL_MAX_WIDTH,
  DESIGN_PANEL_MIN_WIDTH,
  readDesignPanelWidth,
  writeDesignPanelWidth,
} from '@/modules/design/data/designPreferences';
import {
  clearImageLibrary,
  type DesignImageRecord,
  type DesignImageTarget,
  defaultDesignImageTarget,
  deleteImageCollection,
  deleteImagePromptTemplate,
  generateDesignImages,
  type ImageBackground,
  type ImageCollection,
  type ImageGenerationOptions,
  type ImageGenerationResponse,
  type ImageGenerationRun,
  type ImageModeration,
  type ImageOutputFormat,
  type ImagePromptTemplate,
  type ImageQuality,
  listDesignImages,
  listImageCollections,
  listImageGenerationRuns,
  listImagePromptTemplates,
  loadDesignImageTargets,
  readDesignImage,
  removeImageFromLibrary,
  saveImageAnnotation,
  saveImageCollection,
  saveImagePromptTemplate,
  setImageCollectionItem,
} from '@/modules/design/data/imageGeneration';
import { type ChatGptAccountQuotaStatus, refreshChatGptAccountQuota } from '@/modules/providers/data/oauthRateLimits';
import { LocalServiceError, type ServiceOperation } from '@/platform/local-service/LocalServiceClient';

type GalleryImage =
  | { source: 'design'; key: string; createdAt: number; image: DesignImageRecord }
  | { source: 'chat'; key: string; createdAt: number; image: ConversationAttachmentSummary };

interface ImagePageCursor {
  design: number | null;
  chat: number | null;
}

interface DesignPageProps {
  onOpenSettings(): void;
}

type DesignServiceOperation = Omit<ServiceOperation, 'result'> & { result: Promise<ImageGenerationResponse> };

const galleryPageSize = 30;
const modelOptions = {
  oauth: [{ value: 'chatgpt-image-latest', labelKey: 'designModelChatGpt' }],
  api_key: [
    { value: 'gpt-image-2.5-flare', labelKey: 'designModelFast' },
    { value: 'gpt-image-2.5-sunburst', labelKey: 'designModelDetailed' },
    { value: 'gpt-image-2', labelKey: 'designModelTwo' },
  ],
} as const;
const qualityOptions = [
  { value: 'auto', labelKey: 'designQualityAuto' },
  { value: 'low', labelKey: 'designQualityLow' },
  { value: 'medium', labelKey: 'designQualityMedium' },
  { value: 'high', labelKey: 'designQualityHigh' },
  { value: 'xhigh', labelKey: 'designQualityXhigh' },
  { value: 'max', labelKey: 'designQualityMax' },
] as const;
const sizeOptions = [
  { value: 'auto', labelKey: 'designResolutionAuto' },
  { value: '1024x1024', labelKey: 'designSizeSquare1k' },
  { value: '1536x1024', labelKey: 'designSizeLandscape15k' },
  { value: '1024x1536', labelKey: 'designSizePortrait15k' },
  { value: '2048x2048', labelKey: 'designSizeSquare2k' },
  { value: '2048x1152', labelKey: 'designSizeLandscape2k' },
  { value: '1152x2048', labelKey: 'designSizePortrait2k' },
  { value: '3840x2160', labelKey: 'designSizeLandscape4k' },
  { value: '2160x3840', labelKey: 'designSizePortrait4k' },
] as const;
const standardSizeOptions = sizeOptions.slice(0, 4);
const customSizeOption = { value: 'custom', labelKey: 'designResolutionCustom' } as const;

export function DesignPage({ onOpenSettings }: DesignPageProps) {
  const { t } = useTranslation();
  const [controlPanelWidth, setControlPanelWidth] = useState(readDesignPanelWidth);
  const [isResizingControlPanel, setIsResizingControlPanel] = useState(false);
  const [targets, setTargets] = useState<DesignImageTarget[]>([]);
  const [targetsLoading, setTargetsLoading] = useState(true);
  const [targetsError, setTargetsError] = useState<'none' | 'partial' | 'failed'>('none');
  const [targetKey, setTargetKey] = useState('');
  const [prompt, setPrompt] = useState('');
  const [model, setModel] = useState('');
  const [size, setSize] = useState('1024x1024');
  const [quality, setQuality] = useState<ImageQuality>('auto');
  const [background, setBackground] = useState<ImageBackground>('auto');
  const [moderation, setModeration] = useState<ImageModeration>('auto');
  const [outputFormat, setOutputFormat] = useState<ImageOutputFormat>('png');
  const [outputCompression, setOutputCompression] = useState(90);
  const [count, setCount] = useState('1');
  const [customWidth, setCustomWidth] = useState('1536');
  const [customHeight, setCustomHeight] = useState('1024');
  const [accountQuota, setAccountQuota] = useState<ChatGptAccountQuotaStatus | null>(null);
  const [generationStarting, setGenerationStarting] = useState(false);
  const [activeOperation, setActiveOperation] = useState<DesignServiceOperation | null>(null);
  const [generationError, setGenerationError] = useState<string | null>(null);
  const [generationStatus, setGenerationStatus] = useState<string | null>(null);
  const [gallery, setGallery] = useState<GalleryImage[]>([]);
  const [query, setQuery] = useState('');
  const [queryDraft, setQueryDraft] = useState('');
  const [sort, setSort] = useState<'newest' | 'oldest'>('newest');
  const [sourceFilter, setSourceFilter] = useState<'all' | 'design' | 'chat'>('all');
  const [providerFilter, setProviderFilter] = useState<'all' | 'oauth' | 'api_key'>('all');
  const [modelFilter, setModelFilter] = useState('');
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [tagFilter, setTagFilter] = useState('');
  const [tagFilterDraft, setTagFilterDraft] = useState('');
  const [collectionFilter, setCollectionFilter] = useState('');
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const [collections, setCollections] = useState<ImageCollection[]>([]);
  const [templates, setTemplates] = useState<ImagePromptTemplate[]>([]);
  const [failedRuns, setFailedRuns] = useState<ImageGenerationRun[]>([]);
  const [libraryMetadataError, setLibraryMetadataError] = useState(false);
  const [libraryMetadataLoading, setLibraryMetadataLoading] = useState(true);
  const [templateName, setTemplateName] = useState('');
  const [tagDraft, setTagDraft] = useState('');
  const [libraryActionError, setLibraryActionError] = useState(false);
  const [libraryActionBusy, setLibraryActionBusy] = useState(false);
  const [libraryActionStatus, setLibraryActionStatus] = useState<string | null>(null);
  const [collectionName, setCollectionName] = useState('');
  const [selectedCollection, setSelectedCollection] = useState('');
  const [comparisonImages, setComparisonImages] = useState<Array<{ image: GalleryImage; src: string }> | null>(null);
  const [cursor, setCursor] = useState<ImagePageCursor>({ design: 0, chat: 0 });
  const [hasMore, setHasMore] = useState(false);
  const [galleryLoading, setGalleryLoading] = useState(true);
  const [galleryError, setGalleryError] = useState<'none' | 'partial' | 'all'>('none');
  const [selectedImage, setSelectedImage] = useState<{ image: GalleryImage; src: string } | null>(null);
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const controlPanelRef = useRef<HTMLElement>(null);
  const controlPanelResizerRef = useRef<HTMLDivElement>(null);
  const controlPanelWidthRef = useRef(controlPanelWidth);
  const controlPanelResizeStartRef = useRef<{ pointerX: number; width: number } | null>(null);
  const galleryScrollRef = useRef<HTMLDivElement>(null);
  const activeOperationRef = useRef<DesignServiceOperation | null>(null);
  const galleryRequestRef = useRef(0);
  const mountedRef = useRef(true);
  const selectedTarget = targets.find((target) => target.key === targetKey) ?? null;
  const activeModels = selectedTarget ? modelOptions[selectedTarget.kind] : [];
  const selectedModel = activeModels.find((option) => option.value === model) ?? activeModels[0];
  const isApiKeyRoute = selectedTarget?.kind === 'api_key';
  const supportsFlexibleSize = isApiKeyRoute && supportsCustomResolution(selectedModel?.value ?? '');
  const supportsExtendedQuality = isApiKeyRoute && supportsHighQualityTier(selectedModel?.value ?? '');
  const providerOptions = Array.from(
    new Map(
      targets.map((target) => [target.providerId, { value: target.providerId, label: target.providerName }]),
    ).values(),
  );
  const selectedProvider =
    providerOptions.find((option) => option.value === selectedTarget?.providerId) ?? providerOptions[0] ?? null;
  const providerTargets = selectedProvider
    ? targets.filter((target) => target.providerId === selectedProvider.value)
    : [];
  const activeSizeOptions = supportsFlexibleSize ? [...sizeOptions, customSizeOption] : standardSizeOptions;
  const customResolutionValid = size !== 'custom' || isValidImageSize(customWidth, customHeight);
  const selectedDesignImage = selectedImage?.image.source === 'design' ? selectedImage.image.image : null;
  const generationInProgress = generationStarting || activeOperation !== null;

  const loadTargets = useCallback(async () => {
    setTargetsLoading(true);
    try {
      const result = await loadDesignImageTargets();
      setTargets(result.targets);
      setTargetKey((current) =>
        result.targets.some((target) => target.key === current) ? current : defaultDesignImageTarget(result.targets),
      );
      setTargetsError(result.partialFailure ? 'partial' : 'none');
    } catch {
      setTargets([]);
      setTargetsError('failed');
    } finally {
      setTargetsLoading(false);
    }
  }, []);

  const loadGalleryPage = useCallback(
    async (nextCursor: ImagePageCursor, append: boolean) => {
      const requestId = galleryRequestRef.current + 1;
      galleryRequestRef.current = requestId;
      if (dateFrom && dateTo && dateFrom > dateTo) {
        setGallery([]);
        setGalleryLoading(false);
        setGalleryError('none');
        setCursor({ design: 0, chat: 0 });
        setHasMore(false);
        return;
      }
      setGalleryLoading(true);
      setGalleryError('none');
      try {
        const createdAfter = dateFrom ? new Date(`${dateFrom}T00:00:00`).getTime() : null;
        const createdBefore = dateTo ? new Date(`${dateTo}T23:59:59.999`).getTime() : null;
        const filters = {
          query,
          sort,
          createdAfter: Number.isFinite(createdAfter) ? createdAfter : null,
          createdBefore: Number.isFinite(createdBefore) ? createdBefore : null,
          modelId: modelFilter || null,
          providerRoute: providerFilter === 'all' ? null : providerFilter,
          favoritesOnly,
          tag: tagFilter.trim() || null,
          collectionId: collectionFilter || null,
        } as const;
        const designOffset = nextCursor.design;
        const chatOffset = nextCursor.chat;
        const shouldLoadDesign = designOffset !== null && sourceFilter !== 'chat';
        const shouldLoadChat = chatOffset !== null && sourceFilter !== 'design';
        const [designResult, chatResult] = await Promise.allSettled([
          shouldLoadDesign
            ? listDesignImages(designOffset, galleryPageSize, filters)
            : Promise.resolve({ items: [], nextOffset: null }),
          shouldLoadChat
            ? listConversationAttachments(chatOffset, galleryPageSize, query, 'available', {
                generatedImagesOnly: true,
                sort,
                createdAfter: filters.createdAfter,
                createdBefore: filters.createdBefore,
                modelId: filters.modelId,
                providerRoute: filters.providerRoute,
                favoritesOnly,
                tag: filters.tag,
                collectionId: filters.collectionId,
              })
            : Promise.resolve({ items: [], nextOffset: null }),
        ]);
        if (requestId !== galleryRequestRef.current) return;
        const designFailed = shouldLoadDesign && designResult.status === 'rejected';
        const chatFailed = shouldLoadChat && chatResult.status === 'rejected';
        const designPage = designResult.status === 'fulfilled' ? designResult.value : { items: [], nextOffset: null };
        const chatPage = chatResult.status === 'fulfilled' ? chatResult.value : { items: [], nextOffset: null };
        const designImages: GalleryImage[] = designPage.items.map((image) => ({
          source: 'design',
          key: `design:${image.id}`,
          createdAt: image.createdAt,
          image,
        }));
        const chatImages: GalleryImage[] = chatPage.items
          .filter((image) => image.mimeType.startsWith('image/'))
          .map((image) => ({
            source: 'chat',
            key: `chat:${image.conversationId}:${image.messageId}:${image.attachmentId}`,
            createdAt: image.createdAt,
            image,
          }));
        const merged = [...designImages, ...chatImages].sort((left, right) =>
          sort === 'oldest' ? left.createdAt - right.createdAt : right.createdAt - left.createdAt,
        );
        setGallery((current) => (append ? mergeUniqueImages(current, merged, sort) : merged));
        const next = {
          design: designFailed ? designOffset : designPage.nextOffset,
          chat: chatFailed ? chatOffset : chatPage.nextOffset,
        };
        setCursor(next);
        setHasMore(next.design !== null || next.chat !== null);
        const failedCount = Number(designFailed) + Number(chatFailed);
        const requestedCount = Number(shouldLoadDesign) + Number(shouldLoadChat);
        setGalleryError(failedCount === 0 ? 'none' : failedCount === requestedCount && !append ? 'all' : 'partial');
        if (designFailed && designResult.status === 'rejected') {
          reportGalleryLoadFailure('Design', designResult.reason);
        }
        if (chatFailed && chatResult.status === 'rejected') {
          reportGalleryLoadFailure('chat', chatResult.reason);
        }
      } catch {
        if (requestId === galleryRequestRef.current) {
          setGalleryError(append ? 'partial' : 'all');
          globalThis.reportError(new Error('The image library request could not be completed.'));
        }
      } finally {
        if (requestId === galleryRequestRef.current) setGalleryLoading(false);
      }
    },
    [
      collectionFilter,
      dateFrom,
      dateTo,
      favoritesOnly,
      modelFilter,
      providerFilter,
      query,
      sort,
      sourceFilter,
      tagFilter,
    ],
  );

  const refreshLibraryMetadata = useCallback(async () => {
    setLibraryMetadataLoading(true);
    const results = await Promise.allSettled([
      listImageCollections(),
      listImagePromptTemplates(),
      listImageGenerationRuns(),
    ]);
    if (!mountedRef.current) return;
    const [collectionResult, templateResult, runResult] = results;
    if (collectionResult.status === 'fulfilled') setCollections(collectionResult.value);
    if (templateResult.status === 'fulfilled') setTemplates(templateResult.value);
    if (runResult.status === 'fulfilled') {
      setFailedRuns(runResult.value.filter((run) => run.status === 'failed' || run.status === 'cancelled'));
    }
    setLibraryMetadataError(results.some((result) => result.status === 'rejected'));
    setLibraryMetadataLoading(false);
  }, []);

  useEffect(() => {
    void loadTargets();
    void loadGalleryPage({ design: 0, chat: 0 }, false);
    void refreshLibraryMetadata();
  }, [loadGalleryPage, loadTargets, refreshLibraryMetadata]);

  useEffect(() => {
    const timeout = window.setTimeout(() => setQuery(queryDraft), 250);
    return () => window.clearTimeout(timeout);
  }, [queryDraft]);

  useEffect(() => {
    const timeout = window.setTimeout(() => setTagFilter(tagFilterDraft), 250);
    return () => window.clearTimeout(timeout);
  }, [tagFilterDraft]);

  useEffect(() => {
    const firstCollection = collections[0];
    if (!selectedCollection && firstCollection) setSelectedCollection(firstCollection.id);
  }, [collections, selectedCollection]);

  useEffect(() => {
    if (activeOperation !== null) return;
    setModel((current) =>
      activeModels.some((option) => option.value === current) ? current : (activeModels[0]?.value ?? ''),
    );
  }, [activeModels, activeOperation]);

  useEffect(() => {
    if (selectedTarget?.kind !== 'oauth') {
      setAccountQuota(selectedTarget ? { state: 'not_applicable', usedPercent: null, resetAtUnixMs: null } : null);
      return;
    }
    if (selectedTarget.workspaceId === null) {
      setAccountQuota({
        state: 'unavailable',
        usedPercent: null,
        resetAtUnixMs: null,
        reason: 'workspace_unavailable',
      });
      return;
    }
    let current = true;
    setAccountQuota(null);
    void refreshChatGptAccountQuota(selectedTarget.connectionId, selectedTarget.workspaceId).then((status) => {
      if (current) setAccountQuota(status);
    });
    return () => {
      current = false;
    };
  }, [selectedTarget]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      galleryRequestRef.current += 1;
      const operation = activeOperationRef.current;
      activeOperationRef.current = null;
      if (operation) void operation.cancel();
    };
  }, []);

  useEffect(() => {
    controlPanelRef.current?.style.setProperty('--design-control-panel-width', `${controlPanelWidth}px`);
  }, [controlPanelWidth]);

  useEffect(() => {
    if (!isResizingControlPanel) return;
    const handlePointerMove = (event: PointerEvent) => {
      const start = controlPanelResizeStartRef.current;
      if (!start) return;
      const width = clampDesignPanelWidth(start.width + event.clientX - start.pointerX);
      controlPanelWidthRef.current = width;
      controlPanelRef.current?.style.setProperty('--design-control-panel-width', `${width}px`);
      controlPanelResizerRef.current?.setAttribute('aria-valuenow', String(width));
    };
    const finishResize = () => {
      const width = controlPanelWidthRef.current;
      setControlPanelWidth(width);
      writeDesignPanelWidth(width);
      controlPanelResizeStartRef.current = null;
      setIsResizingControlPanel(false);
    };
    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', finishResize);
    window.addEventListener('pointercancel', finishResize);
    return () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', finishResize);
      window.removeEventListener('pointercancel', finishResize);
      if (controlPanelResizeStartRef.current !== null) {
        writeDesignPanelWidth(controlPanelWidthRef.current);
        controlPanelResizeStartRef.current = null;
      }
    };
  }, [isResizingControlPanel]);

  const updateTarget = (event: { target: { value: string } }) => {
    const nextTarget = targets.find((target) => target.key === event.target.value) ?? null;
    setTargetKey(event.target.value);
    setAccountQuota(null);
    if (nextTarget?.kind === 'oauth') {
      setSize('1024x1024');
      setOutputFormat('png');
      setCount('1');
      setModeration('auto');
      setQuality((current) => (current === 'xhigh' || current === 'max' ? 'auto' : current));
    }
  };

  const updateControlPanelWidth = (width: number) => {
    const nextWidth = clampDesignPanelWidth(width);
    controlPanelWidthRef.current = nextWidth;
    setControlPanelWidth(nextWidth);
    writeDesignPanelWidth(nextWidth);
  };

  const handleControlPanelResizePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    controlPanelResizeStartRef.current = {
      pointerX: event.clientX,
      width: controlPanelWidthRef.current,
    };
    setIsResizingControlPanel(true);
  };

  const handleControlPanelResizeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    let nextWidth: number | null = null;
    if (event.key === 'ArrowLeft') nextWidth = controlPanelWidthRef.current - 12;
    if (event.key === 'ArrowRight') nextWidth = controlPanelWidthRef.current + 12;
    if (event.key === 'Home') nextWidth = DESIGN_PANEL_MIN_WIDTH;
    if (event.key === 'End') nextWidth = DESIGN_PANEL_MAX_WIDTH;
    if (nextWidth === null) return;
    event.preventDefault();
    updateControlPanelWidth(nextWidth);
  };

  const updateProvider = (event: { target: { value: string } }) => {
    const providerTargets = targets.filter((target) => target.providerId === event.target.value);
    const nextTarget = providerTargets.find((target) => target.selected) ?? providerTargets[0] ?? null;
    if (nextTarget) updateTarget({ target: { value: nextTarget.key } });
  };

  const runGeneration = async (options: ImageGenerationOptions) => {
    if (generationStarting || activeOperation !== null) return;
    if (options.prompt.trim().length === 0) {
      setGenerationError(t('designPromptRequired'));
      promptRef.current?.focus();
      return;
    }
    setGenerationStarting(true);
    setGenerationError(null);
    setGenerationStatus(null);
    let operation: DesignServiceOperation | null = null;
    try {
      const nextOperation = await generateDesignImages(options);
      operation = nextOperation;
      if (!mountedRef.current) {
        void nextOperation.cancel();
        return;
      }
      activeOperationRef.current = nextOperation;
      setActiveOperation(nextOperation);
      setGenerationStarting(false);
      const result = await nextOperation.result;
      if (!mountedRef.current) return;
      setGenerationStatus(t('designGenerationComplete', { count: result.images.length }));
      if (options.target.kind === 'oauth') setAccountQuota(result.accountQuota);
      await loadGalleryPage({ design: 0, chat: 0 }, false);
      if (options.target.kind === 'oauth' && options.target.workspaceId !== null) {
        void refreshChatGptAccountQuota(options.target.connectionId, options.target.workspaceId).then((status) => {
          if (mountedRef.current) setAccountQuota(status);
        });
      }
    } catch (error) {
      if (!mountedRef.current) return;
      if (error instanceof LocalServiceError && ['request_cancelled', 'operation_cancelled'].includes(error.code)) {
        setGenerationError(null);
        setGenerationStatus(t('designGenerationCancelled'));
      } else if (
        error instanceof LocalServiceError &&
        error.code === 'image_rate_limited' &&
        options.target.kind === 'oauth'
      ) {
        const refreshedQuota =
          options.target.workspaceId === null
            ? null
            : await refreshChatGptAccountQuota(options.target.connectionId, options.target.workspaceId);
        if (!mountedRef.current) return;
        if (refreshedQuota) setAccountQuota(refreshedQuota);
        setGenerationError(
          refreshedQuota?.state === 'exhausted' ? t('designQuotaExhausted') : t('designQuotaRequestLimited'),
        );
      } else if (error instanceof LocalServiceError && error.code === 'image_api_rate_limited') {
        setGenerationError(t('designApiRateLimited'));
      } else {
        setGenerationError(error instanceof Error ? error.message : t('designGenerationFailed'));
      }
    } finally {
      if (activeOperationRef.current === operation) activeOperationRef.current = null;
      if (mountedRef.current) {
        setActiveOperation(null);
        setGenerationStarting(false);
        void refreshLibraryMetadata();
      }
    }
  };

  const handleGenerate = async () => {
    if (!selectedTarget || !selectedModel || generationStarting || activeOperation !== null) return;
    if (selectedTarget.kind === 'oauth' && accountQuota?.state === 'exhausted') return;
    await runGeneration({
      prompt: prompt.trim(),
      model: selectedModel.value,
      size: size === 'custom' ? `${customWidth}x${customHeight}` : size,
      quality,
      background,
      moderation,
      outputFormat,
      outputCompression,
      count: Number(count),
      target: selectedTarget,
    });
  };

  const handleCancel = async () => {
    if (!activeOperation) return;
    try {
      const cancelled = await activeOperation.cancel();
      if (cancelled) setGenerationStatus(t('designGenerationCancelled'));
    } catch {
      setGenerationError(t('designCancelFailed'));
    }
  };

  const reusePrompt = (value: string) => {
    setPrompt(value);
    setSelectedImage(null);
    promptRef.current?.focus();
  };

  const changeFavorite = async (entry: GalleryImage, isFavorite: boolean) => {
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    setLibraryActionStatus(null);
    try {
      await saveImageAnnotation({
        sourceKind: entry.source,
        sourceKey: gallerySourceKey(entry),
        isFavorite,
        tags: entry.image.tags,
      });
      updateEntryAnnotations(entry, { isFavorite, tags: entry.image.tags });
      await loadGalleryPage({ design: 0, chat: 0 }, false);
      setLibraryActionStatus(t('designFavoriteSaved'));
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const addTag = async (entry: GalleryImage) => {
    const tag = tagDraft.trim().toLowerCase();
    if (!tag || Array.from(tag).length > 48 || entry.image.tags.includes(tag) || entry.image.tags.length >= 20) return;
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    setLibraryActionStatus(null);
    try {
      await saveImageAnnotation({
        sourceKind: entry.source,
        sourceKey: gallerySourceKey(entry),
        isFavorite: entry.image.isFavorite,
        tags: [...entry.image.tags, tag],
      });
      updateEntryAnnotations(entry, { isFavorite: entry.image.isFavorite, tags: [...entry.image.tags, tag] });
      setTagDraft('');
      await loadGalleryPage({ design: 0, chat: 0 }, false);
      setLibraryActionStatus(t('designTagSaved'));
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const removeTag = async (entry: GalleryImage, tag: string) => {
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    try {
      await saveImageAnnotation({
        sourceKind: entry.source,
        sourceKey: gallerySourceKey(entry),
        isFavorite: entry.image.isFavorite,
        tags: entry.image.tags.filter((item) => item !== tag),
      });
      updateEntryAnnotations(entry, {
        isFavorite: entry.image.isFavorite,
        tags: entry.image.tags.filter((item) => item !== tag),
      });
      await loadGalleryPage({ design: 0, chat: 0 }, false);
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const updateEntryAnnotations = (entry: GalleryImage, annotations: { isFavorite: boolean; tags: string[] }) => {
    const updated = withAnnotations(entry, annotations);
    setGallery((current) => current.map((item) => (item.key === entry.key ? updated : item)));
    setSelectedImage((current) => (current?.image.key === entry.key ? { ...current, image: updated } : current));
  };

  const changeCollectionMembership = async (entry: GalleryImage, included: boolean) => {
    if (!selectedCollection) return;
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    setLibraryActionStatus(null);
    try {
      await setImageCollectionItem({
        collectionId: selectedCollection,
        sourceKind: entry.source,
        sourceKey: gallerySourceKey(entry),
        included,
      });
      setLibraryActionStatus(included ? t('designCollectionAdded') : t('designCollectionRemoved'));
      await loadGalleryPage({ design: 0, chat: 0 }, false);
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const removeSelectedImage = async () => {
    if (!selectedImage || !window.confirm(t('designRemoveImageConfirm'))) return;
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    try {
      await removeImageFromLibrary(selectedImage.image.source, gallerySourceKey(selectedImage.image));
      setSelectedImage(null);
      await loadGalleryPage({ design: 0, chat: 0 }, false);
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const clearLibrary = async () => {
    if (!window.confirm(t('designClearLibraryConfirm'))) return;
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    try {
      await clearImageLibrary();
      setSelectedImage(null);
      setGallery([]);
      await loadGalleryPage({ design: 0, chat: 0 }, false);
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const createCollection = async () => {
    if (!collectionName.trim()) return;
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    try {
      const collection = await saveImageCollection(collectionName.trim());
      setCollectionName('');
      setCollections((current) => [...current.filter((item) => item.id !== collection.id), collection]);
      setSelectedCollection(collection.id);
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const deleteSelectedCollection = async () => {
    if (!selectedCollection || !window.confirm(t('designDeleteCollectionConfirm'))) return;
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    try {
      await deleteImageCollection(selectedCollection);
      setCollections((current) => current.filter((item) => item.id !== selectedCollection));
      setSelectedCollection('');
      if (collectionFilter === selectedCollection) setCollectionFilter('');
      await loadGalleryPage({ design: 0, chat: 0 }, false);
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const saveCurrentTemplate = async () => {
    if (!templateName.trim() || !prompt.trim()) return;
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    try {
      const template = await saveImagePromptTemplate({ name: templateName.trim(), prompt: prompt.trim() });
      setTemplates((current) => [template, ...current.filter((item) => item.id !== template.id)]);
      setTemplateName('');
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const deleteTemplate = async (id: string) => {
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    try {
      await deleteImagePromptTemplate(id);
      setTemplates((current) => current.filter((item) => item.id !== id));
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const retryGeneration = async (run: ImageGenerationRun) => {
    const target = targets.find(
      (item) =>
        item.kind === run.providerRoute &&
        item.connectionId === run.connectionId &&
        item.workspaceId === run.workspaceId,
    );
    if (!target) {
      setGenerationError(t('designRetryTargetUnavailable'));
      return;
    }
    if (target.kind === 'oauth') {
      const latestQuota =
        target.workspaceId === null
          ? { state: 'unavailable' as const, usedPercent: null, resetAtUnixMs: null }
          : await refreshChatGptAccountQuota(target.connectionId, target.workspaceId);
      setAccountQuota(latestQuota);
      if (latestQuota.state === 'exhausted') {
        setGenerationError(t('designQuotaExhausted'));
        return;
      }
    }
    const savedQuality = run.quality ?? '';
    const savedBackground = run.background ?? '';
    const savedModeration = run.moderation ?? '';
    const savedFormat = run.outputFormat ?? '';
    const options: ImageGenerationOptions = {
      prompt: run.prompt,
      model: run.model,
      size: run.size ?? 'auto',
      quality: isImageQuality(savedQuality) ? savedQuality : 'auto',
      background: isImageBackground(savedBackground) ? savedBackground : 'auto',
      moderation: isImageModeration(savedModeration) ? savedModeration : 'auto',
      outputFormat: isImageOutputFormat(savedFormat) ? savedFormat : 'png',
      outputCompression: run.outputCompression ?? 90,
      count: run.requestedCount,
      target,
    };
    setPrompt(options.prompt);
    setTargetKey(target.key);
    setModel(options.model);
    setSize(options.size);
    setQuality(options.quality);
    setBackground(options.background);
    setModeration(options.moderation);
    setOutputFormat(options.outputFormat);
    setOutputCompression(options.outputCompression);
    setCount(String(options.count));
    await runGeneration(options);
  };

  const openComparison = async (entry: GalleryImage) => {
    const variants = gallery.filter((item) => galleryGenerationKey(item) === galleryGenerationKey(entry));
    if (variants.length < 2) return;
    setLibraryActionBusy(true);
    setLibraryActionError(false);
    try {
      const images = await Promise.all(variants.map(async (image) => ({ image, src: await readGalleryImage(image) })));
      setComparisonImages(images);
    } catch {
      setLibraryActionError(true);
    } finally {
      setLibraryActionBusy(false);
    }
  };

  const canGenerate =
    selectedTarget !== null &&
    selectedModel !== undefined &&
    prompt.trim().length > 0 &&
    !generationStarting &&
    activeOperation === null &&
    customResolutionValid &&
    !(selectedTarget.kind === 'oauth' && accountQuota?.state === 'exhausted');
  const activeFilterCount = [
    queryDraft.trim() !== '',
    sourceFilter !== 'all',
    sort !== 'newest',
    providerFilter !== 'all',
    modelFilter !== '',
    tagFilter !== '',
    collectionFilter !== '',
    favoritesOnly,
    dateFrom !== '',
    dateTo !== '',
  ].filter(Boolean).length;

  return (
    <div
      className="design-workspace flex h-full min-h-0 min-w-0 flex-col overflow-hidden lg:flex-row"
      data-resizing={isResizingControlPanel ? 'true' : undefined}
    >
      <aside
        aria-label={t('designSettings')}
        className="design-control-panel relative flex min-h-0 shrink-0 flex-col gap-5 overflow-y-auto border-b border-border bg-navigation px-4 py-4 lg:border-b-0 lg:border-r lg:px-5 lg:py-6"
        data-resizing={isResizingControlPanel ? 'true' : undefined}
        ref={controlPanelRef}
      >
        <div className="flex items-center gap-3">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-control bg-selected text-accent">
            <WandSparkles aria-hidden="true" size={20} />
          </span>
          <div className="min-w-0">
            <h1 className="text-section-title font-semibold">{t('designTitle')}</h1>
          </div>
        </div>

        {targetsLoading ? (
          <p aria-live="polite" className="flex items-center gap-2 text-sm text-secondary-text" role="status">
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            {t('designConnectionsLoading')}
          </p>
        ) : null}
        {targetsError !== 'none' ? (
          <div className="grid gap-2" role="alert">
            <p className="text-sm text-destructive">
              {t(targetsError === 'partial' ? 'designConnectionsPartialFailed' : 'designConnectionsFailed')}
            </p>
            <Button className="w-full min-w-0" onClick={() => void loadTargets()} size="small" variant="secondary">
              <RefreshCw aria-hidden="true" size={15} />
              <span className="min-w-0 truncate">{t('retry')}</span>
            </Button>
          </div>
        ) : null}

        {targets.length > 0 ? (
          <>
            {providerOptions.length > 1 ? (
              <Select
                disabled={activeOperation !== null || generationStarting}
                leadingIcon={<PanelsTopLeft aria-hidden="true" size={16} />}
                label={t('designProvider')}
                onChange={updateProvider}
                options={providerOptions}
                value={selectedProvider?.value ?? ''}
              />
            ) : (
              <div className="flex min-w-0 items-center gap-2 text-sm">
                <PanelsTopLeft aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                <span className="shrink-0 text-secondary-text">{t('designProvider')}</span>
                <span className="truncate font-medium text-text">{selectedProvider?.label}</span>
              </div>
            )}
            <Select
              disabled={activeOperation !== null || generationStarting}
              leadingIcon={<UserRound aria-hidden="true" size={16} />}
              label={t('designAccount')}
              onChange={updateTarget}
              options={providerTargets.map((target) => ({
                value: target.key,
                label: imageGenerationAccountLabel(target, t),
              }))}
              value={targetKey}
            />

            <label className="grid gap-2 text-sm font-medium text-text" htmlFor="design-prompt">
              <span className="inline-flex items-center gap-2">
                <MessageSquareText aria-hidden="true" className="text-secondary-icon" size={16} />
                {t('designPrompt')}
              </span>
              <textarea
                aria-describedby="design-prompt-hint"
                className="min-h-32 w-full resize-y rounded-control border border-control-border bg-surface px-3 py-2 text-sm font-normal leading-relaxed text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
                id="design-prompt"
                maxLength={10_000}
                onChange={(event: ChangeEvent<HTMLTextAreaElement>) => setPrompt(event.target.value)}
                placeholder={t('designPromptPlaceholder')}
                ref={promptRef}
                value={prompt}
              />
              <span className="text-xs font-normal text-secondary-text" id="design-prompt-hint">
                {t('designPromptHint', { count: prompt.length })}
              </span>
            </label>

            <section
              aria-label={t('designPromptTemplates')}
              className="grid gap-2 rounded-control border border-border p-3"
            >
              <h2 className="flex items-center gap-2 text-sm font-semibold text-text">
                <FileText aria-hidden="true" className="text-secondary-icon" size={16} />
                {t('designPromptTemplates')}
              </h2>
              <div className="grid gap-1">
                {templates.map((template) => (
                  <div className="flex min-w-0 items-center gap-1" key={template.id}>
                    <button
                      className="min-h-9 min-w-0 flex-1 truncate rounded-control px-2 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                      onClick={() => reusePrompt(template.prompt)}
                      type="button"
                    >
                      {template.name}
                    </button>
                    <Button
                      aria-label={t('designDeleteTemplate', { name: template.name })}
                      className="min-w-0"
                      disabled={libraryActionBusy}
                      onClick={() => void deleteTemplate(template.id)}
                      size="small"
                      variant="ghost"
                    >
                      <span className="min-w-0 truncate">{t('designDeleteAction')}</span>
                    </Button>
                  </div>
                ))}
                {templates.length === 0 ? (
                  <p className="text-xs text-secondary-text">{t('designNoTemplates')}</p>
                ) : null}
              </div>
              <label className="grid gap-1.5 text-xs font-medium text-secondary-text" htmlFor="design-template-name">
                {t('designTemplateName')}
                <input
                  className="min-h-9 rounded-control border border-control-border bg-surface px-2 text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  id="design-template-name"
                  maxLength={80}
                  onChange={(event) => setTemplateName(event.target.value)}
                  value={templateName}
                />
              </label>
              <Button
                className="w-full min-w-0"
                disabled={libraryActionBusy || !templateName.trim() || !prompt.trim()}
                onClick={() => void saveCurrentTemplate()}
                size="small"
                variant="secondary"
              >
                <span className="min-w-0 truncate">{t('designSaveTemplate')}</span>
              </Button>
            </section>

            <section
              aria-label={t('designCollections')}
              className="grid gap-2 rounded-control border border-border p-3"
            >
              <h2 className="flex items-center gap-2 text-sm font-semibold text-text">
                <FolderOpen aria-hidden="true" className="text-secondary-icon" size={16} />
                {t('designCollections')}
              </h2>
              <label className="grid gap-1.5 text-xs font-medium text-secondary-text" htmlFor="design-collection-name">
                {t('designCollectionName')}
                <input
                  className="min-h-9 rounded-control border border-control-border bg-surface px-2 text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  id="design-collection-name"
                  maxLength={80}
                  onChange={(event) => setCollectionName(event.target.value)}
                  value={collectionName}
                />
              </label>
              <Button
                className="w-full min-w-0"
                disabled={libraryActionBusy || !collectionName.trim()}
                onClick={() => void createCollection()}
                size="small"
                variant="secondary"
              >
                <span className="min-w-0 truncate">{t('designCreateCollection')}</span>
              </Button>
              {collections.length > 0 ? (
                <>
                  <Select
                    label={t('designManageCollection')}
                    onChange={(event) => setSelectedCollection(event.target.value)}
                    options={collections.map((collection) => ({ value: collection.id, label: collection.name }))}
                    value={selectedCollection}
                  />
                  <Button
                    className="w-full min-w-0"
                    disabled={libraryActionBusy}
                    onClick={() => void deleteSelectedCollection()}
                    size="small"
                    variant="ghost"
                  >
                    <span className="min-w-0 truncate">{t('designDeleteCollection')}</span>
                  </Button>
                </>
              ) : null}
            </section>

            <h2 className="flex items-center gap-2 border-t border-border pt-4 text-sm font-semibold text-text">
              <SlidersHorizontal aria-hidden="true" className="text-secondary-icon" size={16} />
              {t('designImageSettings')}
            </h2>
            <Select
              disabled={!selectedTarget || activeOperation !== null || generationStarting}
              leadingIcon={<Image aria-hidden="true" size={16} />}
              label={t('designImageModel')}
              onChange={(event) => {
                const nextModel = event.target.value;
                setModel(nextModel);
                if (!supportsCustomResolution(nextModel)) {
                  setSize((current) =>
                    standardSizeOptions.some((option) => option.value === current) ? current : '1024x1024',
                  );
                }
                if (!supportsHighQualityTier(nextModel)) {
                  setQuality((current) => (current === 'xhigh' || current === 'max' ? 'auto' : current));
                }
              }}
              options={activeModels.map((option) => ({ value: option.value, label: t(option.labelKey) }))}
              value={selectedModel?.value}
            />
            <Select
              disabled={activeOperation !== null || generationStarting}
              leadingIcon={<Scaling aria-hidden="true" size={16} />}
              label={t('designResolution')}
              onChange={(event) => setSize(event.target.value)}
              options={activeSizeOptions.map((option) => ({ value: option.value, label: t(option.labelKey) }))}
              value={size}
            />
            {size === 'custom' ? (
              <fieldset className="grid gap-2 rounded-control border border-border bg-raised p-3">
                <legend className="px-1 text-sm font-medium text-text">{t('designResolutionCustom')}</legend>
                <div className="grid grid-cols-2 gap-3">
                  <label className="grid gap-1.5 text-sm text-secondary-text" htmlFor="design-image-width">
                    {t('designWidth')}
                    <input
                      aria-invalid={!customResolutionValid}
                      className="min-h-10 w-full rounded-control border border-control-border bg-surface px-3 text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                      id="design-image-width"
                      max={3840}
                      min={256}
                      onChange={(event) => setCustomWidth(event.target.value)}
                      step={16}
                      type="number"
                      value={customWidth}
                    />
                  </label>
                  <label className="grid gap-1.5 text-sm text-secondary-text" htmlFor="design-image-height">
                    {t('designHeight')}
                    <input
                      aria-invalid={!customResolutionValid}
                      className="min-h-10 w-full rounded-control border border-control-border bg-surface px-3 text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                      id="design-image-height"
                      max={3840}
                      min={256}
                      onChange={(event) => setCustomHeight(event.target.value)}
                      step={16}
                      type="number"
                      value={customHeight}
                    />
                  </label>
                </div>
                <p className="text-xs text-secondary-text">{t('designCustomResolutionHint')}</p>
                {!customResolutionValid ? (
                  <p className="text-xs text-destructive" role="alert">
                    {t('designCustomResolutionInvalid')}
                  </p>
                ) : null}
              </fieldset>
            ) : null}
            <Select
              disabled={activeOperation !== null || generationStarting}
              leadingIcon={<Sparkles aria-hidden="true" size={16} />}
              label={t('designQuality')}
              onChange={(event) => {
                if (isImageQuality(event.target.value)) setQuality(event.target.value);
              }}
              options={qualityOptions
                .filter((option) => supportsExtendedQuality || !['xhigh', 'max'].includes(option.value))
                .map((option) => ({ value: option.value, label: t(option.labelKey) }))}
              value={quality}
            />
            <Select
              disabled={activeOperation !== null || generationStarting}
              leadingIcon={<PanelsTopLeft aria-hidden="true" size={16} />}
              label={t('designBackground')}
              onChange={(event) => {
                const value = event.target.value;
                if (!isImageBackground(value)) return;
                setBackground(value);
                if (value === 'transparent' && outputFormat === 'jpeg') setOutputFormat('png');
              }}
              options={[
                { value: 'auto', label: t('designBackgroundAuto') },
                { value: 'opaque', label: t('designBackgroundOpaque') },
                { value: 'transparent', label: t('designBackgroundTransparent') },
              ]}
              value={background}
            />

            {isApiKeyRoute ? (
              <>
                <Select
                  disabled={activeOperation !== null || generationStarting}
                  label={t('designModeration')}
                  onChange={(event) => {
                    if (isImageModeration(event.target.value)) setModeration(event.target.value);
                  }}
                  options={[
                    { value: 'auto', label: t('designModerationAuto') },
                    { value: 'low', label: t('designModerationLow') },
                  ]}
                  value={moderation}
                />
                <p className="-mt-3 text-xs text-secondary-text">{t('designModerationHint')}</p>
                <Select
                  disabled={activeOperation !== null || generationStarting}
                  label={t('designOutputFormat')}
                  onChange={(event) => {
                    if (isImageOutputFormat(event.target.value)) setOutputFormat(event.target.value);
                  }}
                  options={[
                    { value: 'png', label: 'PNG' },
                    { value: 'jpeg', label: 'JPEG', disabled: background === 'transparent' },
                    { value: 'webp', label: 'WebP' },
                  ]}
                  value={outputFormat}
                />
                {outputFormat !== 'png' ? (
                  <label className="grid gap-2 text-sm font-medium text-text" htmlFor="design-compression">
                    {t('designCompression', { value: outputCompression })}
                    <input
                      className="w-full accent-accent"
                      disabled={activeOperation !== null || generationStarting}
                      id="design-compression"
                      max={100}
                      min={0}
                      onChange={(event) => setOutputCompression(Number(event.target.value))}
                      type="range"
                      value={outputCompression}
                    />
                    <span className="text-xs font-normal text-secondary-text">{t('designCompressionHint')}</span>
                  </label>
                ) : null}
                <Select
                  disabled={activeOperation !== null || generationStarting}
                  label={t('designImageCount')}
                  onChange={(event) => setCount(event.target.value)}
                  options={[1, 2, 3].map((value) => ({
                    value: String(value),
                    label: t('designImageCountOption', { count: value }),
                  }))}
                  value={count}
                />
              </>
            ) : null}
            {generationError ? (
              <p className="text-sm text-destructive" role="alert">
                {generationError}
              </p>
            ) : null}
            {generationStatus ? (
              <p aria-live="polite" className="text-sm text-secondary-text" role="status">
                {generationStatus}
              </p>
            ) : null}
            <div className="mt-auto grid gap-2 pt-2">
              {activeOperation ? (
                <Button className="w-full min-w-0" onClick={() => void handleCancel()} variant="secondary">
                  <span className="min-w-0 truncate">{t('designCancelGeneration')}</span>
                </Button>
              ) : (
                <Button
                  className="w-full min-w-0"
                  disabled={!canGenerate}
                  onClick={() => void handleGenerate()}
                  variant="primary"
                >
                  {generationStarting ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" size={17} />
                  ) : (
                    <Sparkles aria-hidden="true" size={17} />
                  )}
                  <span className="min-w-0 truncate">
                    {generationStarting ? t('designGenerationStarting') : t('designGenerate')}
                  </span>
                </Button>
              )}
            </div>
          </>
        ) : !targetsLoading && targetsError === 'none' ? (
          <div className="grid gap-3 rounded-control border border-border bg-raised p-4">
            <p className="text-sm text-secondary-text">{t('designNoProvider')}</p>
            <Button className="w-full min-w-0" onClick={onOpenSettings} variant="secondary">
              <Settings2 aria-hidden="true" size={16} />
              <span className="min-w-0 truncate">{t('designOpenSettings')}</span>
            </Button>
          </div>
        ) : null}
        {/* biome-ignore lint/a11y/useSemanticElements: Custom interactive resize separator for the design settings panel. */}
        <div
          aria-label={t('resizeSidebar')}
          aria-orientation="vertical"
          aria-valuemax={DESIGN_PANEL_MAX_WIDTH}
          aria-valuemin={DESIGN_PANEL_MIN_WIDTH}
          aria-valuenow={controlPanelWidth}
          className={`group/design-resizer absolute inset-y-0 right-0 z-20 hidden w-1.5 cursor-col-resize select-none focus-visible:outline-none lg:block`}
          onDoubleClick={() => updateControlPanelWidth(DESIGN_PANEL_DEFAULT_WIDTH)}
          onKeyDown={handleControlPanelResizeKeyDown}
          onPointerDown={handleControlPanelResizePointerDown}
          ref={controlPanelResizerRef}
          role="separator"
          tabIndex={0}
          title={t('resizeSidebar')}
        >
          <span
            className={`pointer-events-none absolute inset-y-0 right-0 w-0.5 transition-colors duration-quick ${
              isResizingControlPanel
                ? 'bg-accent'
                : 'bg-transparent group-hover/design-resizer:bg-accent/50 group-focus-visible/design-resizer:bg-accent'
            }`}
          />
        </div>
      </aside>

      <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-background">
        <header className="grid gap-4 border-b border-border px-5 py-4 sm:px-7">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 className="text-base font-semibold text-text">{t('designImageLibrary')}</h2>
              <p className="mt-1 text-sm text-secondary-text">{t('designImageLibraryDescription')}</p>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button
                disabled={galleryLoading}
                onClick={() => void loadGalleryPage({ design: 0, chat: 0 }, false)}
                size="small"
                variant="secondary"
              >
                {galleryLoading ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                ) : (
                  <RefreshCw aria-hidden="true" size={15} />
                )}
                {t('designRefreshLibrary')}
              </Button>
              <Button disabled={libraryActionBusy} onClick={() => void clearLibrary()} size="small" variant="secondary">
                {t('designClearLibrary')}
              </Button>
            </div>
          </div>
          <div className="flex min-w-0 flex-wrap items-end gap-3">
            <label
              className="grid min-w-0 flex-1 gap-1 text-xs font-medium text-secondary-text"
              htmlFor="design-library-search"
            >
              {t('designSearch')}
              <input
                className="min-h-10 w-full rounded-control border border-control-border bg-surface px-3 text-sm text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus"
                id="design-library-search"
                onChange={(event) => setQueryDraft(event.target.value)}
                placeholder={t('designSearchPlaceholder')}
                value={queryDraft}
              />
            </label>
            <details className="design-library-filter shrink-0">
              <summary
                aria-label={t(activeFilterCount > 0 ? 'designFiltersActive' : 'designFilters', {
                  count: activeFilterCount,
                })}
                className="design-filter-trigger min-h-10"
              >
                <Filter aria-hidden="true" size={15} />
                <span>{t('designFilters')}</span>
                {activeFilterCount > 0 ? (
                  <span aria-hidden="true" className="design-filter-count">
                    {activeFilterCount}
                  </span>
                ) : null}
                <ChevronDown aria-hidden="true" className="design-filter-chevron" size={15} />
              </summary>
              <div className="design-library-filter-menu openchat-dropdown-surface">
                <Select
                  label={t('designSource')}
                  onChange={(event) => {
                    if (isGallerySourceFilter(event.target.value)) setSourceFilter(event.target.value);
                  }}
                  options={[
                    { value: 'all', label: t('designSourceAll') },
                    { value: 'design', label: t('designSourceDesign') },
                    { value: 'chat', label: t('designSourceChat') },
                  ]}
                  value={sourceFilter}
                />
                <Select
                  label={t('designSort')}
                  onChange={(event) => {
                    if (isGallerySort(event.target.value)) setSort(event.target.value);
                  }}
                  options={[
                    { value: 'newest', label: t('designSortNewest') },
                    { value: 'oldest', label: t('designSortOldest') },
                  ]}
                  value={sort}
                />
                <Select
                  label={t('designProviderFilter')}
                  onChange={(event) => {
                    if (isGalleryProviderFilter(event.target.value)) setProviderFilter(event.target.value);
                  }}
                  options={[
                    { value: 'all', label: t('designProviderAll') },
                    { value: 'oauth', label: t('designProviderOAuth') },
                    { value: 'api_key', label: t('designProviderApi') },
                  ]}
                  value={providerFilter}
                />
                <Select
                  label={t('designModelFilter')}
                  onChange={(event) => setModelFilter(event.target.value)}
                  options={[
                    { value: '', label: t('designAllModels') },
                    ...Array.from(
                      new Set([...modelOptions.oauth, ...modelOptions.api_key].map((option) => option.value)),
                    ).map((value) => ({
                      value,
                      label: imageModelLabel(value, t),
                    })),
                  ]}
                  value={modelFilter}
                />
                <label
                  className="grid gap-1 text-xs font-medium text-secondary-text"
                  htmlFor="design-library-tag-filter"
                >
                  {t('designTagFilter')}
                  <input
                    className="min-h-10 w-full rounded-control border border-control-border bg-surface px-3 text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                    id="design-library-tag-filter"
                    maxLength={48}
                    onChange={(event) => setTagFilterDraft(event.target.value)}
                    value={tagFilterDraft}
                  />
                </label>
                <Select
                  label={t('designCollectionFilter')}
                  onChange={(event) => setCollectionFilter(event.target.value)}
                  options={[
                    { value: '', label: t('designAllCollections') },
                    ...collections.map((collection) => ({ value: collection.id, label: collection.name })),
                  ]}
                  value={collectionFilter}
                />
                <label
                  className="flex min-h-10 items-center gap-2 self-end text-sm text-text"
                  htmlFor="design-favorites-only"
                >
                  <input
                    checked={favoritesOnly}
                    id="design-favorites-only"
                    onChange={(event) => setFavoritesOnly(event.target.checked)}
                    type="checkbox"
                  />
                  {t('designFavoritesOnly')}
                </label>
                <DatePicker label={t('designDateFrom')} onChange={setDateFrom} value={dateFrom} />
                <DatePicker label={t('designDateTo')} onChange={setDateTo} value={dateTo} />
                <Button
                  className="design-filter-reset"
                  disabled={activeFilterCount === 0}
                  onClick={() => {
                    setQuery('');
                    setQueryDraft('');
                    setSort('newest');
                    setSourceFilter('all');
                    setProviderFilter('all');
                    setModelFilter('');
                    setFavoritesOnly(false);
                    setTagFilter('');
                    setTagFilterDraft('');
                    setCollectionFilter('');
                    setDateFrom('');
                    setDateTo('');
                  }}
                  size="small"
                  variant="ghost"
                >
                  {t('designClearFilters')}
                </Button>
              </div>
            </details>
          </div>
          {libraryActionError ? (
            <p className="text-sm text-destructive" role="alert">
              {t('designLibraryActionFailed')}
            </p>
          ) : null}
          {libraryMetadataError ? (
            <div className="flex flex-wrap items-center gap-2" role="alert">
              <p className="text-sm text-destructive">{t('designMetadataLoadFailed')}</p>
              <Button
                disabled={libraryMetadataLoading}
                onClick={() => void refreshLibraryMetadata()}
                size="small"
                variant="secondary"
              >
                {libraryMetadataLoading ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                ) : (
                  <RefreshCw aria-hidden="true" size={15} />
                )}
                {t('retry')}
              </Button>
            </div>
          ) : null}
          {libraryActionStatus ? (
            <p aria-live="polite" className="text-sm text-secondary-text" role="status">
              {libraryActionStatus}
            </p>
          ) : null}
          {dateFrom && dateTo && dateFrom > dateTo ? (
            <p className="text-sm text-destructive" role="alert">
              {t('designDateRangeInvalid')}
            </p>
          ) : null}
        </header>

        {failedRuns.length > 0 ? (
          <section
            aria-label={t('designRecentFailures')}
            className="grid gap-2 border-b border-border px-5 py-3 sm:px-7"
          >
            <h3 className="text-sm font-semibold text-text">{t('designRecentFailures')}</h3>
            {failedRuns.slice(0, 3).map((run) => (
              <div
                className="flex min-w-0 flex-wrap items-center justify-between gap-2 rounded-control bg-raised px-3 py-2"
                key={run.id}
              >
                <div className="min-w-0">
                  <p className="truncate text-sm text-text">{run.prompt}</p>
                  <p className="text-xs text-secondary-text">
                    {t(run.status === 'failed' ? 'designGenerationFailed' : 'designGenerationCancelled')}
                  </p>
                </div>
                {targets.some(
                  (target) => target.connectionId === run.connectionId && target.workspaceId === run.workspaceId,
                ) ? (
                  <Button
                    disabled={generationStarting || activeOperation !== null}
                    onClick={() => void retryGeneration(run)}
                    size="small"
                    variant="secondary"
                  >
                    {t('designRetryGeneration')}
                  </Button>
                ) : (
                  <Button onClick={() => reusePrompt(run.prompt)} size="small" variant="ghost">
                    {t('designReusePrompt')}
                  </Button>
                )}
              </div>
            ))}
          </section>
        ) : null}

        <div aria-live="polite" className="min-h-0 flex-1 overflow-y-auto px-4 py-5 sm:px-7" ref={galleryScrollRef}>
          {galleryError !== 'none' && gallery.length === 0 ? (
            <div className="mx-auto grid max-w-lg gap-3 py-12 text-center" role="alert">
              <p className="text-sm text-destructive">{t('designLibraryFailed')}</p>
              <Button
                className="justify-self-center"
                onClick={() => void loadGalleryPage({ design: 0, chat: 0 }, false)}
                size="small"
                variant="secondary"
              >
                {t('retry')}
              </Button>
            </div>
          ) : null}
          {gallery.length === 0 && galleryLoading && !generationInProgress ? (
            <p className="flex items-center justify-center gap-2 py-20 text-sm text-secondary-text" role="status">
              <LoaderCircle aria-hidden="true" className="animate-spin" size={17} />
              {t('designLibraryLoading')}
            </p>
          ) : null}
          {gallery.length === 0 && !galleryLoading && galleryError === 'none' && !generationInProgress ? (
            <div className="mx-auto grid max-w-lg justify-items-center gap-3 py-20 text-center">
              <span className="flex size-12 items-center justify-center rounded-full bg-raised text-secondary-icon">
                <Images aria-hidden="true" size={22} />
              </span>
              <h3 className="text-base font-semibold text-text">{t('designLibraryEmptyTitle')}</h3>
              <p className="max-w-sm text-sm leading-relaxed text-secondary-text">
                {t('designLibraryEmptyDescription')}
              </p>
            </div>
          ) : null}

          {gallery.length > 0 || generationInProgress ? (
            <>
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5">
                {generationInProgress ? (
                  <div
                    aria-label={t('designGenerationInProgress')}
                    className="design-image-tile relative grid aspect-square min-w-0 place-items-center overflow-hidden rounded-control border border-border bg-raised"
                    role="status"
                  >
                    <span className="openchat-skeleton absolute inset-0 bg-selected" />
                    <span aria-hidden="true" className="relative z-10 grid justify-items-center gap-3 text-accent">
                      <LoaderCircle className="openchat-ai-ring-spinner" size={28} />
                      <span className="text-sm font-medium">{t('designGenerationInProgress')}</span>
                    </span>
                    <span
                      aria-hidden="true"
                      className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/75 to-transparent px-3 pb-2 pt-8 text-xs font-medium text-white"
                    >
                      {t('designGenerationInProgressHint')}
                    </span>
                  </div>
                ) : null}
                {gallery.map((entry) => (
                  <ImageTile
                    entry={entry}
                    key={entry.key}
                    root={galleryScrollRef.current}
                    onSelect={(src) => setSelectedImage({ image: entry, src })}
                  />
                ))}
              </div>
              {hasMore ? (
                <div className="flex justify-center py-6">
                  <Button
                    disabled={galleryLoading}
                    onClick={() => void loadGalleryPage(cursor, true)}
                    variant="secondary"
                  >
                    {galleryLoading ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : null}
                    {t('loadMore')}
                  </Button>
                </div>
              ) : null}
            </>
          ) : null}
        </div>
      </main>

      <Dialog
        className="w-full max-w-5xl"
        onOpenChange={(open) => {
          if (!open) setSelectedImage(null);
        }}
        open={selectedImage !== null}
        title={t('designImageDetails')}
      >
        {selectedImage ? (
          <div className="grid gap-4">
            <img
              alt={galleryImageAlt(selectedImage.image)}
              className="design-image-preview w-full rounded-control bg-raised object-contain"
              src={selectedImage.src}
            />
            <p className="break-words text-sm text-secondary-text">{galleryImageCaption(selectedImage.image)}</p>
            {selectedDesignImage ? (
              <dl className="grid grid-cols-2 gap-3 rounded-control border border-border bg-raised p-3 text-sm sm:grid-cols-3">
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designSource')}</dt>
                  <dd className="mt-1 break-words font-medium text-text">{t('designSourceDesign')}</dd>
                </div>
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataModel')}</dt>
                  <dd className="mt-1 break-words font-medium text-text">
                    {imageModelLabel(selectedDesignImage.model, t)}
                  </dd>
                </div>
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataResolution')}</dt>
                  <dd className="mt-1 break-words font-medium text-text">
                    {selectedDesignImage.size ?? t('designResolutionAuto')}
                  </dd>
                </div>
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataQuality')}</dt>
                  <dd className="mt-1 break-words font-medium text-text">
                    {imageQualityLabel(selectedDesignImage.quality, t)}
                  </dd>
                </div>
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataBackground')}</dt>
                  <dd className="mt-1 break-words font-medium text-text">
                    {imageBackgroundLabel(selectedDesignImage.background, t)}
                  </dd>
                </div>
                {selectedDesignImage.moderation !== null ? (
                  <div className="min-w-0">
                    <dt className="text-xs text-secondary-text">{t('designMetadataModeration')}</dt>
                    <dd className="mt-1 break-words font-medium text-text">
                      {imageModerationLabel(selectedDesignImage.moderation, t)}
                    </dd>
                  </div>
                ) : null}
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataFormat')}</dt>
                  <dd className="mt-1 font-medium uppercase text-text">{selectedDesignImage.outputFormat}</dd>
                </div>
                {selectedDesignImage.outputCompression !== null ? (
                  <div className="min-w-0">
                    <dt className="text-xs text-secondary-text">{t('designMetadataCompression')}</dt>
                    <dd className="mt-1 font-medium text-text">
                      {t('designCompression', { value: selectedDesignImage.outputCompression })}
                    </dd>
                  </div>
                ) : null}
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataFileSize')}</dt>
                  <dd className="mt-1 font-medium text-text">{formatImageSize(selectedDesignImage.sizeBytes)}</dd>
                </div>
              </dl>
            ) : null}
            {selectedImage.image.source === 'chat' ? (
              <dl className="grid grid-cols-2 gap-3 rounded-control border border-border bg-raised p-3 text-sm sm:grid-cols-3">
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designSource')}</dt>
                  <dd className="mt-1 break-words font-medium text-text">{t('designSourceChat')}</dd>
                </div>
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataModel')}</dt>
                  <dd className="mt-1 break-words font-medium text-text">
                    {selectedImage.image.image.model
                      ? imageModelLabel(selectedImage.image.image.model, t)
                      : t('designMetadataUnavailable')}
                  </dd>
                </div>
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataProvider')}</dt>
                  <dd className="mt-1 font-medium text-text">
                    {providerRouteLabel(selectedImage.image.image.providerRoute, t)}
                  </dd>
                </div>
                <div className="min-w-0">
                  <dt className="text-xs text-secondary-text">{t('designMetadataFileSize')}</dt>
                  <dd className="mt-1 font-medium text-text">{formatImageSize(selectedImage.image.image.sizeBytes)}</dd>
                </div>
              </dl>
            ) : null}
            <section className="grid gap-3 rounded-control border border-border p-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h3 className="text-sm font-semibold text-text">{t('designLibraryOrganization')}</h3>
                <Button
                  disabled={libraryActionBusy}
                  onClick={() => void changeFavorite(selectedImage.image, !selectedImage.image.image.isFavorite)}
                  size="small"
                  variant={selectedImage.image.image.isFavorite ? 'primary' : 'secondary'}
                >
                  {selectedImage.image.image.isFavorite ? t('designRemoveFavorite') : t('designAddFavorite')}
                </Button>
              </div>
              <div className="flex flex-wrap gap-1">
                {selectedImage.image.image.tags.map((tag) => (
                  <button
                    aria-label={t('designRemoveTag', { tag })}
                    className="rounded-control bg-raised px-2 py-1 text-xs text-text hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                    key={tag}
                    onClick={() => void removeTag(selectedImage.image, tag)}
                    type="button"
                  >
                    {tag} ×
                  </button>
                ))}
              </div>
              <div className="flex flex-wrap gap-2">
                <label className="sr-only" htmlFor="design-tag-input">
                  {t('designAddTag')}
                </label>
                <input
                  className="min-h-9 min-w-0 flex-1 rounded-control border border-control-border bg-surface px-2 text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  id="design-tag-input"
                  maxLength={48}
                  onChange={(event) => setTagDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') {
                      event.preventDefault();
                      void addTag(selectedImage.image);
                    }
                  }}
                  placeholder={t('designTagPlaceholder')}
                  value={tagDraft}
                />
                <Button
                  disabled={libraryActionBusy || !tagDraft.trim()}
                  onClick={() => void addTag(selectedImage.image)}
                  size="small"
                  variant="secondary"
                >
                  {t('designAddTag')}
                </Button>
              </div>
              {collections.length > 0 ? (
                <div className="grid gap-2 sm:grid-cols-3">
                  <Select
                    label={t('designManageCollection')}
                    onChange={(event) => setSelectedCollection(event.target.value)}
                    options={collections.map((collection) => ({ value: collection.id, label: collection.name }))}
                    value={selectedCollection}
                  />
                  <Button
                    className="self-end"
                    disabled={libraryActionBusy || !selectedCollection}
                    onClick={() => void changeCollectionMembership(selectedImage.image, true)}
                    size="small"
                    variant="secondary"
                  >
                    {t('designCollectionAddImage')}
                  </Button>
                  <Button
                    className="self-end"
                    disabled={libraryActionBusy || !selectedCollection}
                    onClick={() => void changeCollectionMembership(selectedImage.image, false)}
                    size="small"
                    variant="ghost"
                  >
                    {t('designCollectionRemoveImage')}
                  </Button>
                </div>
              ) : null}
            </section>
            <div className="flex flex-wrap justify-end gap-2">
              {galleryVariants(gallery, selectedImage.image).length > 1 ? (
                <Button
                  disabled={libraryActionBusy}
                  onClick={() => void openComparison(selectedImage.image)}
                  size="small"
                  variant="secondary"
                >
                  {t('designCompareVariants', { count: galleryVariants(gallery, selectedImage.image).length })}
                </Button>
              ) : null}
              <Button onClick={() => reusePrompt(galleryPrompt(selectedImage.image))} size="small" variant="secondary">
                <WandSparkles aria-hidden="true" size={15} />
                {t('designReusePrompt')}
              </Button>
              <Button
                disabled={libraryActionBusy}
                onClick={() => void removeSelectedImage()}
                size="small"
                variant="secondary"
              >
                {t('designRemoveFromLibrary')}
              </Button>
              <a
                className="openchat-button inline-flex min-h-10 items-center justify-center gap-2 rounded-button border border-border px-3 text-sm font-medium text-text transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                download={`openchat-image.${galleryImageExtension(selectedImage.image)}`}
                href={selectedImage.src}
              >
                <ArrowDownToLine aria-hidden="true" size={15} />
                {t('designDownload')}
              </a>
            </div>
          </div>
        ) : null}
      </Dialog>
      <Dialog
        className="w-full max-w-6xl"
        onOpenChange={(open) => {
          if (!open) setComparisonImages(null);
        }}
        open={comparisonImages !== null}
        title={t('designCompareTitle')}
      >
        {comparisonImages ? (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {comparisonImages.map(({ image, src }) => (
              <figure className="grid min-w-0 gap-2" key={image.key}>
                <img
                  alt={galleryImageAlt(image)}
                  className="design-compare-image w-full rounded-control bg-raised object-contain"
                  src={src}
                />
                <figcaption className="break-words text-sm text-secondary-text">
                  {galleryImageCaption(image)}
                </figcaption>
              </figure>
            ))}
          </div>
        ) : null}
      </Dialog>
    </div>
  );
}

function ImageTile({
  entry,
  root,
  onSelect,
}: {
  entry: GalleryImage;
  root: Element | null;
  onSelect(src: string): void;
}) {
  const { t } = useTranslation();
  const tileRef = useRef<HTMLButtonElement>(null);
  const [inRange, setInRange] = useState(false);
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const element = tileRef.current;
    if (!element || typeof IntersectionObserver === 'undefined') {
      setInRange(true);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        setInRange(entries.some((item) => item.isIntersecting));
      },
      { root, rootMargin: '480px' },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [root]);

  useEffect(() => {
    if (!inRange) {
      setSrc(null);
      return;
    }
    let active = true;
    setFailed(false);
    void readGalleryImage(entry)
      .then((value) => {
        if (active) setSrc(value);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
    };
  }, [entry, inRange]);

  const label = galleryPrompt(entry);
  return (
    <button
      aria-label={t('designOpenImage', { label: label.slice(0, 100) })}
      className="group design-image-tile relative grid aspect-square min-w-0 overflow-hidden rounded-control border border-border bg-raised text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      onClick={() => {
        if (src) onSelect(src);
      }}
      ref={tileRef}
      type="button"
    >
      {src ? (
        <img
          alt={label}
          className="size-full object-cover transition-transform duration-quick"
          loading="lazy"
          src={src}
        />
      ) : failed ? (
        <span className="flex h-full items-center justify-center px-3 text-center text-xs text-secondary-text">
          {t('designImageUnavailable')}
        </span>
      ) : (
        <span className="flex h-full items-center justify-center text-secondary-icon">
          <LoaderCircle aria-label={t('designImageLoading')} className="animate-spin" size={18} />
        </span>
      )}
      <span className="absolute inset-x-0 bottom-0 flex items-end justify-between gap-2 bg-gradient-to-t from-black/80 via-black/45 to-transparent px-3 pb-2 pt-12">
        <span
          aria-hidden="true"
          className="pointer-events-none line-clamp-3 min-w-0 flex-1 break-words text-left text-xs font-medium leading-4 text-white/75 opacity-0 transition-opacity duration-quick group-hover:opacity-75 group-focus-visible:opacity-75 motion-reduce:transition-none"
        >
          {label}
        </span>
        <span className="max-w-1/2 shrink-0 truncate text-xs font-medium text-white/70">
          {entry.source === 'design' ? entry.image.model : entry.image.conversationTitle}
        </span>
      </span>
    </button>
  );
}

async function readGalleryImage(entry: GalleryImage): Promise<string> {
  if (entry.source === 'design') {
    const image = await readDesignImage(entry.image.id, entry.image.mimeType, entry.image.sizeBytes);
    return bytesToDataUrl(image.mimeType, image.bytes);
  }
  const bytes = await readMessageAttachment(
    entry.image.conversationId,
    entry.image.messageId,
    entry.image.attachmentId,
  );
  return bytesToDataUrl(entry.image.mimeType, bytes);
}

function bytesToDataUrl(mimeType: string, bytes: Uint8Array): string {
  let binary = '';
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return `data:${mimeType};base64,${window.btoa(binary)}`;
}

function mergeUniqueImages(current: GalleryImage[], next: GalleryImage[], sort: 'newest' | 'oldest'): GalleryImage[] {
  const images = new Map(current.map((image) => [image.key, image]));
  for (const image of next) images.set(image.key, image);
  return [...images.values()].sort((left, right) =>
    sort === 'oldest' ? left.createdAt - right.createdAt : right.createdAt - left.createdAt,
  );
}

function gallerySourceKey(entry: GalleryImage): string {
  return entry.source === 'design'
    ? entry.image.id
    : `${entry.image.conversationId}:${entry.image.messageId}:${entry.image.attachmentId}`;
}

function galleryGenerationKey(entry: GalleryImage): string {
  if (entry.source === 'design') return `generation:${entry.image.generationId}`;
  return entry.image.generationId
    ? `generation:${entry.image.generationId}`
    : `legacy-chat:${entry.image.conversationId}:${entry.image.messageId}`;
}

function galleryVariants(images: GalleryImage[], selected: GalleryImage): GalleryImage[] {
  return images.filter((image) => galleryGenerationKey(image) === galleryGenerationKey(selected));
}

function galleryPrompt(entry: GalleryImage): string {
  return entry.source === 'design' ? entry.image.prompt : (entry.image.prompt ?? entry.image.conversationTitle);
}

function withAnnotations(entry: GalleryImage, annotations: { isFavorite: boolean; tags: string[] }): GalleryImage {
  if (entry.source === 'design') return { ...entry, image: { ...entry.image, ...annotations } };
  return { ...entry, image: { ...entry.image, ...annotations } };
}

function providerRouteLabel(route: 'oauth' | 'api_key' | 'unknown' | null, t: TFunction): string {
  if (route === 'oauth') return t('designProviderOAuth');
  if (route === 'api_key') return t('designProviderApi');
  return t('designMetadataUnavailable');
}

function isGallerySourceFilter(value: string): value is 'all' | 'design' | 'chat' {
  return value === 'all' || value === 'design' || value === 'chat';
}

function isGallerySort(value: string): value is 'newest' | 'oldest' {
  return value === 'newest' || value === 'oldest';
}

function isGalleryProviderFilter(value: string): value is 'all' | 'oauth' | 'api_key' {
  return value === 'all' || value === 'oauth' || value === 'api_key';
}

function imageGenerationAccountLabel(target: DesignImageTarget, t: TFunction): string {
  const accountLabel =
    target.accountLabel ?? t(target.kind === 'api_key' ? 'designAccountApiKey' : 'designAccountDefault');
  const workspaceLabel = target.workspaceId === null ? null : (target.workspaceLabel ?? t('designWorkspaceDefault'));
  return workspaceLabel ? `${accountLabel} · ${workspaceLabel}` : accountLabel;
}

function extensionFromMime(mimeType: string): string {
  if (mimeType === 'image/jpeg') return 'jpeg';
  if (mimeType === 'image/webp') return 'webp';
  return 'png';
}

function galleryImageAlt(entry: GalleryImage): string {
  return entry.source === 'design' ? entry.image.prompt : entry.image.fileName;
}

function galleryImageCaption(entry: GalleryImage): string {
  return entry.source === 'design' ? entry.image.prompt : (entry.image.prompt ?? entry.image.conversationTitle);
}

function galleryImageExtension(entry: GalleryImage): string {
  return entry.source === 'design' ? entry.image.outputFormat : extensionFromMime(entry.image.mimeType);
}

function imageModelLabel(model: string, t: TFunction): string {
  const modelOption = [...modelOptions.oauth, ...modelOptions.api_key].find((option) => option.value === model);
  return modelOption ? t(modelOption.labelKey) : model;
}

function imageQualityLabel(quality: string | null, t: TFunction): string {
  const qualityOption = qualityOptions.find((option) => option.value === quality);
  return qualityOption ? t(qualityOption.labelKey) : t('designQualityAuto');
}

function imageBackgroundLabel(background: string | null, t: TFunction): string {
  if (background === 'transparent') return t('designBackgroundTransparent');
  if (background === 'opaque') return t('designBackgroundOpaque');
  return t('designBackgroundAuto');
}

function imageModerationLabel(moderation: ImageModeration, t: TFunction): string {
  return moderation === 'low' ? t('designModerationLow') : t('designModerationAuto');
}

function isImageQuality(value: string): value is ImageQuality {
  return qualityOptions.some((option) => option.value === value);
}

function isImageBackground(value: string): value is ImageBackground {
  return value === 'auto' || value === 'opaque' || value === 'transparent';
}

function isImageModeration(value: string): value is ImageModeration {
  return value === 'auto' || value === 'low';
}

function isImageOutputFormat(value: string): value is ImageOutputFormat {
  return value === 'png' || value === 'jpeg' || value === 'webp';
}

function supportsCustomResolution(model: string): boolean {
  return model === 'gpt-image-2' || model.startsWith('gpt-image-2.5-');
}

function supportsHighQualityTier(model: string): boolean {
  return model.startsWith('gpt-image-2.5-');
}

function isValidImageSize(widthValue: string, heightValue: string): boolean {
  if (!/^\d+$/.test(widthValue) || !/^\d+$/.test(heightValue)) return false;
  const width = Number(widthValue);
  const height = Number(heightValue);
  const pixels = width * height;
  return (
    Number.isSafeInteger(width) &&
    Number.isSafeInteger(height) &&
    width >= 256 &&
    height >= 256 &&
    width <= 3840 &&
    height <= 3840 &&
    width % 16 === 0 &&
    height % 16 === 0 &&
    pixels >= 655_360 &&
    pixels <= 8_294_400 &&
    Math.max(width, height) <= Math.min(width, height) * 3
  );
}

function formatImageSize(sizeBytes: number): string {
  const mebibytes = sizeBytes / (1024 * 1024);
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(mebibytes)} MiB`;
}

function reportGalleryLoadFailure(source: 'Design' | 'chat', error: unknown): void {
  const code = error instanceof LocalServiceError ? error.code : 'request_failed';
  globalThis.reportError(new Error(`${source} image library request failed (${code}).`));
}
