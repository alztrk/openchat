import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  CircleCheck,
  Cpu,
  Download,
  File,
  GitBranch,
  Heart,
  Info,
  LoaderCircle,
  LockKeyhole,
  RefreshCw,
  Scale,
  Search,
  X,
} from 'lucide-react';
import { type FormEvent, type ReactNode, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Input, Select } from '@/design-system';
import type {
  HubDetails,
  HubFile,
  HubModel,
  LoadState,
  ModelSort,
  SearchErrorCode,
} from '@/modules/model-library/data/modelCatalogData';
import {
  componentKey,
  componentKinds,
  downloadErrorMessageKey,
  formatBytes,
  formatCompact,
  HubResponseError,
  isActive,
  isModelSort,
  modelDisplayName,
  parseHubDetails,
  parseHubModel,
  publisherName,
  readmeErrorMessageKey,
  readmeMessageKey,
  searchErrorFor,
  sortLabelKey,
} from '@/modules/model-library/data/modelCatalogData';
import { useModelDownloadStore } from '@/modules/model-library/stores/modelDownloadStore';
import { LocalEngineIcon } from '@/modules/providers/components/EngineIcon';
import { useModelDirectoryStore } from '@/modules/settings/stores/modelDirectoryStore';
import { localServiceClient, localServiceErrorDetails } from '@/platform/local-service';
import { ModelReadmeMarkdown } from './ModelReadmeMarkdown';

export function ModelCatalogTab() {
  const { i18n, t } = useTranslation();
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState<ModelSort>('downloads');
  const [models, setModels] = useState<HubModel[]>([]);
  const [pageIndex, setPageIndex] = useState(0);
  const [pageCursors, setPageCursors] = useState<Array<string | null>>([null]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [failedCursor, setFailedCursor] = useState<string | null>(null);
  const [failedPageIndex, setFailedPageIndex] = useState<number | null>(null);
  const [state, setState] = useState<LoadState>('idle');
  const [errorCode, setErrorCode] = useState<SearchErrorCode | null>(null);
  const [errorDetails, setErrorDetails] = useState<string | null>(null);
  const [selectedModel, setSelectedModel] = useState<HubModel | null>(null);
  const [details, setDetails] = useState<HubDetails | null>(null);
  const [detailsState, setDetailsState] = useState<LoadState>('idle');
  const [detailsErrorCode, setDetailsErrorCode] = useState<SearchErrorCode | null>(null);
  const [detailsErrorDetails, setDetailsErrorDetails] = useState<string | null>(null);
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
  const [visibleComponentCounts, setVisibleComponentCounts] = useState<Record<string, number>>({});
  const [downloadedComponentKeys, setDownloadedComponentKeys] = useState<Set<string>>(new Set());
  const searchGeneration = useRef(0);
  const detailsGeneration = useRef(0);
  const searchDebounce = useRef<number | undefined>(undefined);
  const downloadStatus = useModelDownloadStore((store) => store.status);
  const activeRepoId = useModelDownloadStore((store) => store.activeRepoId);
  const downloadProgress = useModelDownloadStore((store) => store.progress);
  const progressUnavailable = useModelDownloadStore((store) => store.progressUnavailable);
  const downloadErrorCode = useModelDownloadStore((store) => store.errorCode);
  const downloadErrorDetails = useModelDownloadStore((store) => store.errorDetails);
  const downloadResult = useModelDownloadStore((store) => store.result);
  const startDownload = useModelDownloadStore((store) => store.start);
  const cancelDownload = useModelDownloadStore((store) => store.cancel);
  const modelDirectory = useModelDirectoryStore((store) => store.directory);
  const modelDirectoryPreferenceAvailable = useModelDirectoryStore((store) => store.available);

  const loadPage = useCallback(
    async (cursor: string | null, targetPageIndex: number, resetPagination: boolean) => {
      const generation = ++searchGeneration.current;
      setState('loading');
      setErrorCode(null);
      setErrorDetails(null);
      setFailedCursor(null);
      setFailedPageIndex(null);
      setSelectedModel(null);
      setDetails(null);
      setDetailsErrorCode(null);
      setSelectedGroupId(null);
      detailsGeneration.current++;
      if (resetPagination) {
        setModels([]);
        setPageCursors([null]);
        setPageIndex(0);
        setNextCursor(null);
      }

      try {
        const response = await localServiceClient.call(
          'models.hub.search',
          {
            query: query.trim(),
            format: 'gguf',
            sort,
            cursor,
          },
          50_000,
        );
        if (generation !== searchGeneration.current) return;
        if (!Array.isArray(response.models) || response.freshness !== 'current') {
          throw new HubResponseError();
        }
        const responseNextCursor = response.nextCursor;
        if (
          responseNextCursor !== null &&
          (typeof responseNextCursor !== 'string' ||
            responseNextCursor.length === 0 ||
            responseNextCursor.length > 4096)
        ) {
          throw new HubResponseError();
        }
        const page = response.models.map(parseHubModel);
        setModels(page);
        setNextCursor(responseNextCursor);
        setPageIndex(targetPageIndex);
        setPageCursors((current) => {
          const next = resetPagination ? [null] : [...current];
          while (next.length <= targetPageIndex) next.push(null);
          next[targetPageIndex] = cursor;
          if (next.length === targetPageIndex + 1) next.push(responseNextCursor);
          else next[targetPageIndex + 1] = responseNextCursor;
          return next;
        });
        setState('ready');
      } catch (error) {
        if (generation !== searchGeneration.current) return;
        setErrorCode(searchErrorFor(error));
        setErrorDetails(localServiceErrorDetails(error));
        setFailedCursor(cursor);
        setFailedPageIndex(targetPageIndex);
        setState('error');
      }
    },
    [query, sort],
  );

  useEffect(() => {
    searchDebounce.current = window.setTimeout(() => {
      searchDebounce.current = undefined;
      void loadPage(null, 0, true);
    }, 250);
    return () => {
      if (searchDebounce.current !== undefined) {
        window.clearTimeout(searchDebounce.current);
        searchDebounce.current = undefined;
      }
      searchGeneration.current++;
    };
  }, [loadPage]);

  const openModel = async (model: HubModel) => {
    setSelectedModel(model);
    setDetails(null);
    setSelectedGroupId(null);
    setVisibleComponentCounts({});
    setDetailsState('loading');
    setDetailsErrorCode(null);
    setDetailsErrorDetails(null);
    const generation = ++detailsGeneration.current;
    try {
      const response = await localServiceClient.call(
        'models.hub.files',
        { repoId: model.repoId, format: 'gguf' },
        50_000,
      );
      if (generation !== detailsGeneration.current) return;
      const nextDetails = parseHubDetails(response);
      setDetails(nextDetails);
      setSelectedGroupId(nextDetails.downloadGroups[0]?.id ?? null);
      setDetailsState('ready');
    } catch (error) {
      if (generation !== detailsGeneration.current) return;
      setDetailsErrorCode(searchErrorFor(error));
      setDetailsErrorDetails(localServiceErrorDetails(error));
      setDetailsState('error');
    }
  };

  const submitSearch = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (searchDebounce.current !== undefined) {
      window.clearTimeout(searchDebounce.current);
      searchDebounce.current = undefined;
    }
    void loadPage(null, 0, true);
  };

  const selectSort = (value: string) => {
    if (isModelSort(value)) setSort(value);
  };

  const loadPreviousPage = () => {
    if (state === 'loading' || pageIndex === 0) return;
    void loadPage(pageCursors[pageIndex - 1] ?? null, pageIndex - 1, false);
  };

  const loadNextPage = () => {
    if (state === 'loading' || nextCursor === null) return;
    void loadPage(nextCursor, pageIndex + 1, false);
  };

  const retrySearch = () => {
    const retryPageIndex = failedPageIndex ?? 0;
    void loadPage(failedCursor, retryPageIndex, retryPageIndex === 0);
  };

  const selectedGroup = details?.downloadGroups.find((group) => group.id === selectedGroupId) ?? null;
  const isDownloadActive = isActive(downloadStatus);
  const accessBlocked = details?.gated === true || details?.private === true;
  const canDownloadGroup = Boolean(
    details && selectedGroup?.canDownload && !accessBlocked && !isDownloadActive && modelDirectoryPreferenceAvailable,
  );
  const canDownloadComponents = Boolean(
    details && selectedGroup?.canDownload && !accessBlocked && !isDownloadActive && modelDirectoryPreferenceAvailable,
  );

  const downloadSelectedGroup = async () => {
    if (!selectedModel || !details || !selectedGroup?.canDownload || accessBlocked || isDownloadActive) return;
    await startDownload({
      repoId: details.repoId,
      revision: details.revision,
      format: 'gguf',
      groupId: selectedGroup.id,
      modelDirectory: modelDirectory ?? undefined,
    });
  };

  const downloadComponent = async (file: HubFile) => {
    if (
      !selectedModel ||
      !details ||
      !selectedGroup?.canDownload ||
      accessBlocked ||
      isDownloadActive ||
      !modelDirectoryPreferenceAvailable ||
      file.kind === 'model' ||
      file.sizeBytes === null
    ) {
      return;
    }
    await startDownload({
      repoId: details.repoId,
      revision: details.revision,
      format: 'gguf',
      groupId: selectedGroup.id,
      componentPath: file.path,
      modelDirectory: modelDirectory ?? undefined,
    });
    const result = useModelDownloadStore.getState().result;
    if (
      result?.repoId === details.repoId &&
      result.revision === details.revision &&
      result.componentPath === file.path
    ) {
      setDownloadedComponentKeys((current) => {
        const next = new Set(current);
        next.add(componentKey(details.repoId, details.revision, file.path));
        return next;
      });
    }
  };

  const renderFileRow = (file: HubFile, allowComponentDownload: boolean) => {
    const isComponent = file.kind !== 'model';
    const key = selectedModel && details ? componentKey(details.repoId, details.revision, file.path) : file.path;
    const downloaded =
      downloadedComponentKeys.has(key) ||
      (downloadStatus === 'completed' &&
        downloadResult?.repoId === details?.repoId &&
        downloadResult?.revision === details?.revision &&
        downloadResult?.componentPath === file.path);
    return (
      <tr className="border-t border-border first:border-t-0" key={file.path}>
        <th className="max-w-0 px-3 py-2.5 text-left font-normal" scope="row">
          <span className="flex min-w-0 items-center gap-2">
            <File aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
            <span className="min-w-0 truncate font-mono text-xs text-text" title={file.path}>
              {file.path}
            </span>
          </span>
        </th>
        <td className="whitespace-nowrap px-3 py-2.5 text-right text-xs text-secondary-text">
          {file.sizeBytes === null ? t('modelUnknownDownloadSize') : formatBytes(file.sizeBytes, i18n.language)}
        </td>
        <td className="w-12 px-2 py-1 text-right">
          {isComponent ? (
            <Button
              aria-label={downloaded ? t('modelComponentDownloaded') : t('modelDownloadComponentButton')}
              disabled={!allowComponentDownload || file.sizeBytes === null || downloaded}
              onClick={() => void downloadComponent(file)}
              size="icon"
              title={downloaded ? t('modelComponentDownloaded') : t('modelDownloadComponentButton')}
              variant="ghost"
            >
              {downloaded ? <CircleCheck aria-hidden="true" size={17} /> : <Download aria-hidden="true" size={17} />}
            </Button>
          ) : null}
        </td>
      </tr>
    );
  };

  const componentGroups = details
    ? [
        {
          key: 'vision',
          label: t('modelVisionComponentsLabel'),
          files: details.files.filter((file) => file.kind === 'vision'),
        },
        {
          key: 'mtp',
          label: t('modelMtpComponentsLabel'),
          files: details.files.filter((file) => file.kind === 'mtp'),
        },
        {
          key: 'auxiliary',
          label: t('modelAuxiliaryComponentsLabel'),
          files: details.files.filter((file) => file.kind === 'auxiliary' || !componentKinds.has(file.kind)),
        },
      ]
    : [];
  const modelFiles = details?.files.filter((file) => file.kind === 'model') ?? [];

  return (
    <div className="library-model-page flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="library-model-content flex min-h-0 min-w-0 flex-1 flex-col">
        {!modelDirectoryPreferenceAvailable ? (
          <p className="px-5 text-sm text-destructive sm:px-8" role="alert">
            {t('localModelActionError')}
          </p>
        ) : null}

        {isDownloadActive ? (
          <div className="px-5 sm:px-8">
            <ModelDownloadStatus
              activeRepoId={activeRepoId}
              cancelDownload={() => void cancelDownload()}
              locale={i18n.language}
              progress={downloadProgress}
              progressUnavailable={progressUnavailable}
              status={downloadStatus}
            />
          </div>
        ) : null}

        <div className="library-model-controls shrink-0 border-b border-border bg-navigation px-5 py-4 sm:px-8">
          <div className="flex flex-wrap items-center gap-3">
            <Button
              aria-pressed={true}
              className="min-h-10 border-accent/50 bg-selected px-3 text-text hover:bg-selected"
              size="small"
              type="button"
              variant="secondary"
            >
              <LocalEngineIcon engineId="llama_cpp" size={17} tone="accent" />
              {t('modelFormatGguf')}
            </Button>
            <div className="relative min-w-0 flex-model-sort">
              <Select
                aria-label={t(sortLabelKey(sort))}
                className="min-w-0"
                label={t('modelSortDownloads')}
                onChange={(event) => selectSort(event.target.value)}
                options={[
                  {
                    value: 'downloads',
                    label: t('modelSortDownloads'),
                    icon: <Download aria-hidden="true" size={15} />,
                  },
                  { value: 'likes', label: t('modelSortLikes'), icon: <Heart aria-hidden="true" size={15} /> },
                  {
                    value: 'recently_updated',
                    label: t('modelSortRecentlyUpdated'),
                    icon: <RefreshCw aria-hidden="true" size={15} />,
                  },
                ]}
                value={sort}
                visuallyHiddenLabel
              />
            </div>
          </div>
          <form className="mt-3 w-full" onSubmit={submitSearch}>
            <div className="relative">
              <Search
                aria-hidden="true"
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-secondary-icon"
                size={16}
              />
              <Input
                aria-label={t('searchModels')}
                className="pl-9 pr-12"
                id="model-catalog-search"
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t('huggingFaceModelSearchHint')}
                value={query}
              />
              <Button
                aria-label={t('modelSearchRefresh')}
                className="absolute right-1 top-1/2 size-8 -translate-y-1/2"
                disabled={state === 'loading'}
                size="icon"
                type="submit"
                variant="ghost"
              >
                {state === 'loading' ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={18} />
                ) : (
                  <RefreshCw aria-hidden="true" size={18} />
                )}
              </Button>
            </div>
          </form>
        </div>

        <div className="model-catalog-layout grid min-h-0 min-w-0 flex-1 gap-3 p-4 sm:p-5 library:grid-cols-library">
          <Card className="model-catalog-results-panel flex min-h-0 min-w-0 flex-col overflow-hidden p-0">
            <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
              <h2 className="font-semibold text-text">{t('models')}</h2>
              {state === 'loading' && models.length > 0 ? (
                <LoaderCircle aria-label={t('modelsLoading')} className="animate-spin text-secondary-icon" size={17} />
              ) : null}
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto p-2">
              {state === 'loading' && models.length === 0 ? (
                <PanelMessage
                  icon={<LoaderCircle aria-hidden="true" className="animate-spin" size={22} />}
                  text={t('modelsLoading')}
                />
              ) : null}
              {state === 'ready' && models.length === 0 ? (
                <PanelMessage icon={<Search aria-hidden="true" size={22} />} text={t('modelSearchEmpty')} />
              ) : null}
              {state === 'error' ? (
                <PanelMessage
                  icon={<CircleAlert aria-hidden="true" size={22} />}
                  onRetry={retrySearch}
                  role="alert"
                  details={errorDetails}
                  text={t(errorCode ?? 'modelSearchFailed')}
                />
              ) : null}
              {state !== 'error' && models.length > 0 ? (
                <div aria-label={t('models')} className="grid gap-1" role="listbox">
                  {models.map((model) => {
                    const selected = model.repoId === selectedModel?.repoId;
                    const publisher = publisherName(model.repoId);
                    return (
                      <button
                        aria-selected={selected}
                        className={[
                          'flex w-full items-center gap-3 rounded-control border p-3 text-left transition-colors',
                          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus',
                          selected
                            ? 'border-accent bg-selected'
                            : 'border-transparent hover:border-border hover:bg-hover',
                        ].join(' ')}
                        key={model.repoId}
                        onClick={() => void openModel(model)}
                        role="option"
                        type="button"
                      >
                        <PublisherAvatar publisher={publisher} />
                        <span className="min-w-0 flex-1">
                          <span className="block line-clamp-2 break-words font-medium text-text">
                            {modelDisplayName(model.repoId)}
                          </span>
                          <span className="mt-1 block truncate text-xs text-secondary-text">
                            {publisher ? `${publisher} · ` : ''}
                            {t('modelDownloadsLabel')}: {formatCompact(model.downloads, i18n.language)}
                          </span>
                        </span>
                        {model.gated || model.private ? (
                          <LockKeyhole
                            aria-label={model.private ? t('modelPrivateBadge') : t('modelGatedBadge')}
                            className="shrink-0 text-secondary-icon"
                            size={17}
                          />
                        ) : null}
                        {selected ? (
                          <CircleCheck aria-hidden="true" className="shrink-0 text-accent-icon" size={18} />
                        ) : null}
                      </button>
                    );
                  })}
                </div>
              ) : null}
            </div>
            {state !== 'error' && models.length > 0 ? (
              <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border px-2 py-1">
                <Button
                  disabled={state === 'loading' || pageIndex === 0}
                  onClick={loadPreviousPage}
                  size="small"
                  variant="ghost"
                >
                  <ChevronLeft aria-hidden="true" size={17} />
                  {t('modelPreviousPage')}
                </Button>
                <span aria-live="polite" className="text-xs text-secondary-text">
                  {t('modelPageLabel', { page: pageIndex + 1 })}
                </span>
                <Button
                  disabled={state === 'loading' || nextCursor === null}
                  onClick={loadNextPage}
                  size="small"
                  variant="ghost"
                >
                  {t('modelNextPage')}
                  <ChevronRight aria-hidden="true" size={17} />
                </Button>
              </div>
            ) : null}
          </Card>

          <div className="model-catalog-detail-layout grid min-h-0 min-w-0 gap-3">
            <Card className="model-catalog-details-panel min-h-0 min-w-0 overflow-hidden p-0">
              <div className="model-catalog-details-content flex h-full min-h-0 flex-col overflow-hidden p-5 sm:p-6">
                {selectedModel === null ? (
                  <PanelMessage icon={<Cpu aria-hidden="true" size={24} />} text={t('modelChooseForDetails')} />
                ) : null}
                {selectedModel !== null && detailsState === 'loading' ? (
                  <PanelMessage
                    icon={<LoaderCircle aria-hidden="true" className="animate-spin" size={24} />}
                    text={t('modelDetailsLoading')}
                  />
                ) : null}
                {selectedModel !== null && detailsState === 'error' ? (
                  <PanelMessage
                    icon={<CircleAlert aria-hidden="true" size={24} />}
                    onRetry={() => void openModel(selectedModel)}
                    role="alert"
                    details={detailsErrorDetails}
                    text={t(detailsErrorCode ?? 'modelSearchFailed')}
                  />
                ) : null}
                {selectedModel !== null && details !== null && detailsState === 'ready' ? (
                  <div className="model-details-content-inner flex min-h-0 flex-1 flex-col gap-4">
                    <header className="model-details-header flex shrink-0 flex-wrap items-center gap-x-5 gap-y-3 border-b border-border pb-4">
                      <span className="flex size-10 shrink-0 items-center justify-center rounded-control border border-border bg-composer text-accent-icon">
                        <LocalEngineIcon engineId="llama_cpp" size={20} />
                      </span>
                      <div className="model-details-title min-w-0 flex-1">
                        <h2 className="break-words text-lg font-semibold text-text">{selectedModel.repoId}</h2>
                        <p className="mt-1 text-sm text-secondary-text">
                          {publisherName(selectedModel.repoId) ?? t('models')}
                        </p>
                      </div>
                      {details.private || details.gated ? (
                        <span className="rounded-control border border-warning/40 bg-surface px-2 py-1 text-xs text-warning">
                          {details.private ? t('modelPrivateBadge') : t('modelGatedBadge')}
                        </span>
                      ) : null}
                      <dl className="model-details-stats ml-auto flex flex-wrap justify-end gap-x-5 gap-y-2">
                        <MetadataItem
                          icon={<Download aria-hidden="true" size={14} />}
                          label={t('modelDownloadsLabel')}
                          value={formatCompact(selectedModel.downloads, i18n.language)}
                        />
                        <MetadataItem
                          icon={<Heart aria-hidden="true" size={14} />}
                          label={t('modelLikesLabel')}
                          value={formatCompact(selectedModel.likes, i18n.language)}
                        />
                        {details.license ? (
                          <MetadataItem
                            icon={<Scale aria-hidden="true" size={14} />}
                            label={t('modelLicenseLabel')}
                            value={details.license}
                          />
                        ) : null}
                        <MetadataItem
                          icon={<GitBranch aria-hidden="true" size={14} />}
                          label={t('modelRevisionLabel')}
                          value={details.revision.slice(0, 12)}
                        />
                      </dl>
                    </header>

                    {accessBlocked ? (
                      <InlineNotice
                        icon={<LockKeyhole aria-hidden="true" size={17} />}
                        text={t('modelDownloadAccessNeeded')}
                        tone="warning"
                      />
                    ) : null}

                    <section
                      aria-label={t('modelReadmeLabel')}
                      className="model-readme-section flex min-h-0 flex-1 flex-col"
                    >
                      {details.readmeMarkdown !== null ? (
                        <div className="model-readme-content min-h-0 flex-1 overflow-auto rounded-control border border-border bg-surface p-4 text-sm leading-6">
                          <ModelReadmeMarkdown
                            content={details.readmeMarkdown}
                            repoId={details.repoId}
                            revision={details.revision}
                          />
                        </div>
                      ) : (
                        <div className="model-readme-content grid min-h-0 flex-1 content-start gap-3 overflow-auto rounded-control border border-border bg-surface p-4">
                          <p className="text-sm text-secondary-text">
                            {t(
                              details.readmeStatus === 'unavailable'
                                ? readmeErrorMessageKey(details.readmeErrorCode)
                                : readmeMessageKey(details.readmeStatus),
                            )}
                          </p>
                          {details.readmeStatus === 'unavailable' ? (
                            <Button
                              className="justify-self-start"
                              onClick={() => void openModel(selectedModel)}
                              size="small"
                              variant="secondary"
                            >
                              <RefreshCw aria-hidden="true" size={15} />
                              {t('retry')}
                            </Button>
                          ) : null}
                        </div>
                      )}
                    </section>
                  </div>
                ) : null}
              </div>
            </Card>

            <Card
              aria-labelledby="model-files-heading"
              className="model-catalog-files-panel flex min-h-0 min-w-0 flex-col overflow-hidden p-0"
            >
              <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border px-4 py-3">
                <h2 className="font-semibold text-text" id="model-files-heading">
                  {t('modelFilesLabel')}
                </h2>
              </div>
              <div className="model-catalog-files-content min-h-0 flex-1 overflow-y-auto p-4">
                {selectedModel === null ? (
                  <PanelMessage icon={<File aria-hidden="true" size={22} />} text={t('modelChooseForDetails')} />
                ) : null}
                {selectedModel !== null && detailsState === 'loading' ? (
                  <PanelMessage
                    icon={<LoaderCircle aria-hidden="true" className="animate-spin" size={22} />}
                    text={t('modelDetailsLoading')}
                  />
                ) : null}
                {selectedModel !== null && detailsState === 'error' ? (
                  <PanelMessage
                    icon={<CircleAlert aria-hidden="true" size={22} />}
                    onRetry={() => void openModel(selectedModel)}
                    role="alert"
                    details={detailsErrorDetails}
                    text={t(detailsErrorCode ?? 'modelSearchFailed')}
                  />
                ) : null}
                {selectedModel !== null && details !== null && detailsState === 'ready' ? (
                  <div className="grid gap-5">
                    {details.files.length === 0 ? (
                      <p className="py-5 text-center text-sm text-secondary-text">{t('modelNoFiles')}</p>
                    ) : (
                      <div className="overflow-x-auto rounded-control border border-border bg-surface">
                        <table className="w-full border-collapse text-sm">
                          <caption className="sr-only">{t('modelFilesLabel')}</caption>
                          <thead className="bg-navigation text-secondary-text">
                            <tr>
                              <th className="px-3 py-2 text-left text-xs font-medium" scope="col">
                                {t('modelFileName')}
                              </th>
                              <th className="px-3 py-2 text-right text-xs font-medium" scope="col">
                                {t('modelFileSize')}
                              </th>
                              <th className="w-12 px-2 py-2" scope="col">
                                <span className="sr-only">{t('modelDownloadComponentButton')}</span>
                              </th>
                            </tr>
                          </thead>
                          {modelFiles.length > 0 ? (
                            <tbody>
                              {modelFiles.slice(0, 12).map((file) => renderFileRow(file, false))}
                              {modelFiles.length > 12 ? (
                                <tr className="border-t border-border">
                                  <td className="px-3 py-2 text-xs text-secondary-text" colSpan={3}>
                                    {t('fileChangesMoreFiles', { count: modelFiles.length - 12 })}
                                  </td>
                                </tr>
                              ) : null}
                            </tbody>
                          ) : null}
                          {componentGroups.map((group) => {
                            if (group.files.length === 0) return null;
                            const groupKey = `${selectedModel.repoId}\\n${details.revision}\\n${group.key}`;
                            const visibleCount = visibleComponentCounts[groupKey] ?? 12;
                            return (
                              <tbody key={group.key}>
                                <tr className="border-t border-border bg-navigation/70">
                                  <th
                                    className="px-3 py-2 text-left text-xs font-semibold text-secondary-text"
                                    colSpan={3}
                                    scope="rowgroup"
                                  >
                                    {group.label}
                                  </th>
                                </tr>
                                {group.files
                                  .slice(0, visibleCount)
                                  .map((file) => renderFileRow(file, canDownloadComponents))}
                                {group.files.length > visibleCount ? (
                                  <tr className="border-t border-border">
                                    <td className="px-2 py-1" colSpan={3}>
                                      <Button
                                        onClick={() =>
                                          setVisibleComponentCounts((current) => ({
                                            ...current,
                                            [groupKey]: Math.min(visibleCount + 12, group.files.length),
                                          }))
                                        }
                                        size="small"
                                        variant="ghost"
                                      >
                                        <ChevronDown aria-hidden="true" size={16} />
                                        {t('modelShowMoreComponents')}
                                      </Button>
                                    </td>
                                  </tr>
                                ) : null}
                              </tbody>
                            );
                          })}
                        </table>
                      </div>
                    )}

                    <section
                      aria-labelledby="model-download-options-heading"
                      className="grid gap-3 border-t border-border pt-4"
                    >
                      <h3 className="font-semibold text-text" id="model-download-options-heading">
                        {t('modelDownloadOptionsLabel')}
                      </h3>
                      {details.downloadGroups.length === 0 ? (
                        <p className="text-sm text-secondary-text">{t('modelNoCompatibleFiles')}</p>
                      ) : (
                        <fieldset className="grid gap-2">
                          <legend className="sr-only">{t('modelDownloadGroupLabel')}</legend>
                          {details.downloadGroups.map((group) => {
                            const firstFile = group.files[0];
                            return (
                              <label
                                className="flex min-w-0 items-start gap-2 rounded-control border border-border bg-surface p-2.5 hover:bg-hover has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-60"
                                key={group.id}
                              >
                                <input
                                  checked={selectedGroupId === group.id}
                                  className="mt-1 accent-accent"
                                  disabled={!group.canDownload || isDownloadActive}
                                  name="model-download-group"
                                  onChange={() => setSelectedGroupId(group.id)}
                                  type="radio"
                                  value={group.id}
                                />
                                <span className="min-w-0 flex-1">
                                  <span className="block break-words text-sm font-medium text-text">
                                    {group.displayName}
                                  </span>
                                  <span className="mt-1 block break-all text-xs text-secondary-text">
                                    {firstFile?.path ?? t('modelNoFiles')}
                                    {group.files.length > 1
                                      ? ` · ${t('fileChangesMoreFiles', { count: group.files.length - 1 })}`
                                      : ''}
                                  </span>
                                  <span className="mt-1 block text-xs text-secondary-text">
                                    {t('modelDownloadSizeLabel')}
                                    {': '}
                                    {group.totalBytes === null
                                      ? t('modelUnknownDownloadSize')
                                      : formatBytes(group.totalBytes, i18n.language)}
                                  </span>
                                </span>
                              </label>
                            );
                          })}
                        </fieldset>
                      )}
                      <p className="text-xs text-secondary-text">{t('modelSavedToFolder')}</p>
                      {!accessBlocked && selectedGroup === null ? (
                        <InlineNotice icon={<Info aria-hidden="true" size={17} />} text={t('modelNoCompatibleFiles')} />
                      ) : null}
                      {!accessBlocked && selectedGroup?.canDownload === false ? (
                        <InlineNotice
                          icon={<Info aria-hidden="true" size={17} />}
                          text={t('modelUnknownDownloadSize')}
                        />
                      ) : null}
                      {downloadStatus === 'completed' &&
                      downloadResult?.repoId === details.repoId &&
                      downloadResult?.revision === details.revision &&
                      downloadResult?.componentPath === null ? (
                        <InlineNotice
                          icon={<CircleCheck aria-hidden="true" size={17} />}
                          text={t('modelDownloadComplete')}
                          tone="success"
                        />
                      ) : null}
                      {downloadStatus === 'cancelled' && activeRepoId === details.repoId ? (
                        <InlineNotice icon={<Info aria-hidden="true" size={17} />} text={t('modelDownloadCancelled')} />
                      ) : null}
                      {downloadStatus === 'failed' && activeRepoId === details.repoId ? (
                        <InlineNotice
                          icon={<CircleAlert aria-hidden="true" size={17} />}
                          text={t(downloadErrorMessageKey(downloadErrorCode))}
                          details={downloadErrorDetails}
                          tone="error"
                        />
                      ) : null}
                      <Button
                        className="w-full"
                        disabled={!canDownloadGroup}
                        onClick={() => void downloadSelectedGroup()}
                        variant="primary"
                      >
                        {isDownloadActive && activeRepoId === details.repoId ? (
                          <LoaderCircle aria-hidden="true" className="animate-spin" size={17} />
                        ) : (
                          <Download aria-hidden="true" size={17} />
                        )}
                        {t('modelDownloadButton')}
                      </Button>
                    </section>
                  </div>
                ) : null}
              </div>
            </Card>
          </div>
        </div>
      </div>
    </div>
  );
}

interface PublisherAvatarProps {
  publisher: string | null;
}

function PublisherAvatar({ publisher }: PublisherAvatarProps) {
  const [failed, setFailed] = useState(false);
  const initials = publisher?.slice(0, 1).toUpperCase() || '?';
  return (
    <span className="flex size-7 shrink-0 items-center justify-center overflow-hidden rounded-full bg-selected text-xs font-semibold text-secondary-text">
      {publisher && !failed ? (
        <img
          alt=""
          className="size-full object-cover"
          onError={() => setFailed(true)}
          src={`https://huggingface.co/api/avatars/${encodeURIComponent(publisher)}`}
        />
      ) : (
        <span aria-hidden="true">{initials}</span>
      )}
    </span>
  );
}

interface MetadataItemProps {
  icon: ReactNode;
  label: string;
  value: string;
}

function MetadataItem({ icon, label, value }: MetadataItemProps) {
  return (
    <div className="model-metadata-item grid content-start gap-0.5">
      <dt className="flex items-center gap-1.5 text-xs text-secondary-text">
        <span aria-hidden="true" className="text-accent-icon">
          {icon}
        </span>
        {label}
      </dt>
      <dd className="break-words pl-5 font-medium text-text">{value}</dd>
    </div>
  );
}

interface InlineNoticeProps {
  icon: ReactNode;
  text: string;
  details?: string | null;
  tone?: 'default' | 'warning' | 'success' | 'error';
}

function InlineNotice({ details, icon, text, tone = 'default' }: InlineNoticeProps) {
  const { t } = useTranslation();
  const toneClass =
    tone === 'warning'
      ? 'text-warning'
      : tone === 'success'
        ? 'text-success'
        : tone === 'error'
          ? 'text-destructive'
          : 'text-secondary-text';
  return (
    <div className={['flex items-start gap-2 text-sm', toneClass].join(' ')} role={tone === 'error' ? 'alert' : 'status'}>
      <span className="mt-0.5 shrink-0">{icon}</span>
      <span className="grid min-w-0 gap-1">
        <span>{text}</span>
        {details ? (
          <span className="break-words whitespace-pre-wrap text-xs text-secondary-text">
            {t('chatErrorDetails')}: {details}
          </span>
        ) : null}
      </span>
    </div>
  );
}

interface PanelMessageProps {
  icon: ReactNode;
  text: string;
  details?: string | null;
  onRetry?: () => void;
  role?: 'status' | 'alert';
}

function PanelMessage({ details, icon, onRetry, role = 'status', text }: PanelMessageProps) {
  const { t } = useTranslation();
  return (
    <div
      aria-live={role === 'alert' ? 'assertive' : 'polite'}
      className="flex min-h-64 flex-col items-center justify-center gap-3 p-6 text-center"
      role={role}
    >
      <span className="text-secondary-icon">{icon}</span>
      <p className="max-w-sm text-sm text-secondary-text">{text}</p>
      {details ? (
        <p className="max-w-full break-words whitespace-pre-wrap text-xs text-secondary-text">
          {t('chatErrorDetails')}: {details}
        </p>
      ) : null}
      {onRetry ? (
        <Button onClick={onRetry} size="small" variant="secondary">
          <RefreshCw aria-hidden="true" size={15} />
          {t('retry')}
        </Button>
      ) : null}
    </div>
  );
}

interface ModelDownloadStatusProps {
  activeRepoId: string | null;
  cancelDownload(): void;
  locale: string;
  progress: ReturnType<typeof useModelDownloadStore.getState>['progress'];
  progressUnavailable: boolean;
  status: ReturnType<typeof useModelDownloadStore.getState>['status'];
}

function ModelDownloadStatus({
  activeRepoId,
  cancelDownload,
  locale,
  progress,
  progressUnavailable,
  status,
}: ModelDownloadStatusProps) {
  const { t } = useTranslation();
  const fraction =
    progress && progress.totalBytes > 0 ? Math.min(1, progress.downloadedBytes / progress.totalBytes) : null;
  return (
    <section
      aria-label={t('modelDownloadOptionsLabel')}
      aria-live="polite"
      className="grid gap-3 rounded-card border border-border bg-surface p-4"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="min-w-0 flex-1 break-all text-sm font-medium text-text">
          {progress
            ? t('modelDownloadRunning', {
                fileName: progress.fileName,
                fileIndex: progress.fileIndex,
                fileCount: progress.fileCount,
              })
            : activeRepoId}
        </p>
        <Button disabled={status === 'cancelling'} onClick={cancelDownload} size="small" variant="secondary">
          {status === 'cancelling' ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
          ) : (
            <X aria-hidden="true" size={15} />
          )}
          {t('modelCancelDownload')}
        </Button>
      </div>
      <progress
        aria-label={t('modelDownloadOptionsLabel')}
        className="h-2 w-full accent-accent"
        max={1}
        value={fraction ?? undefined}
      />
      {progressUnavailable ? (
        <p className="text-xs text-secondary-text">{t('modelDownloadProgressUnavailable')}</p>
      ) : null}
      {progress && progress.totalBytes > 0 ? (
        <p className="text-right text-xs text-secondary-text">
          {formatBytes(progress.downloadedBytes, locale)} / {formatBytes(progress.totalBytes, locale)}
        </p>
      ) : null}
    </section>
  );
}
