import {
  CalendarDays,
  FileSearch,
  FolderOpen,
  LoaderCircle,
  MessageSquare,
  Save,
  Search,
  Trash2,
} from 'lucide-react';
import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { type TranslationKey, providerNameForModel } from '@entities/model';
import { Button, Dialog, Input, Select } from '@shared/components';
import {
  emptyHistorySearchFilters,
  deleteHistorySearch,
  historySearchDateRange,
  HistorySearchInputError,
  loadHistorySearchFilterOptions,
  loadSavedHistorySearches,
  searchHistory,
  SavedHistorySearchError,
  saveHistorySearch,
  type HistoryResultType,
  type SavedHistorySearch,
  type HistorySearchFilterOptions,
  type HistorySearchFilters,
  type HistorySearchResult,
  type HistorySortOrder,
} from '@features/chat/data';
import { useChatStore } from '@features/chat/state';

type FilterOptionsStatus = 'idle' | 'loading' | 'ready' | 'error';
type SavedSearchStatus = 'idle' | 'loading' | 'ready' | 'error';

export interface HistorySearchDialogProps {
  onFiltersActiveChange(active: boolean): void;
  onOpenChange(open: boolean): void;
  onSelectConversation(conversationId: string, messageId?: string | null, searchQuery?: string | null): void;
  onSelectProject?: (projectId: string) => void;
  open: boolean;
}

const resultTypes: readonly HistoryResultType[] = ['all', 'chats', 'projects', 'messages', 'conversations'];
const sortOrders: readonly HistorySortOrder[] = ['relevance', 'newest', 'oldest'];
const resultTypeKeys: Record<HistoryResultType, TranslationKey> = {
  all: 'historySearchAllResultTypes',
  chats: 'searchFilterChats',
  projects: 'searchFilterProjects',
  messages: 'historySearchMessagesOnly',
  conversations: 'historySearchConversationsOnly',
};
const sortOrderKeys: Record<HistorySortOrder, TranslationKey> = {
  relevance: 'historySearchSortRelevance',
  newest: 'historySearchSortNewest',
  oldest: 'historySearchSortOldest',
};

export function HistorySearchDialog({
  onFiltersActiveChange,
  onOpenChange,
  onSelectConversation,
  onSelectProject,
  open,
}: HistorySearchDialogProps) {
  const { i18n, t } = useTranslation();
  const [query, setQuery] = useState('');
  const [filters, setFilters] = useState<HistorySearchFilters>(emptyHistorySearchFilters);
  const [fromDate, setFromDate] = useState('');
  const [throughDate, setThroughDate] = useState('');
  const [filterOptions, setFilterOptions] = useState<HistorySearchFilterOptions>({ providerIds: [], modelIds: [] });
  const [filterOptionsStatus, setFilterOptionsStatus] = useState<FilterOptionsStatus>('idle');
  const [savedSearches, setSavedSearches] = useState<SavedHistorySearch[]>([]);
  const [savedSearchStatus, setSavedSearchStatus] = useState<SavedSearchStatus>('idle');
  const [selectedSavedSearchName, setSelectedSavedSearchName] = useState('');
  const [savedSearchName, setSavedSearchName] = useState('');
  const [savedSearchError, setSavedSearchError] = useState<SavedHistorySearchError['reason'] | null>(null);
  const [isSavedSearchMutationPending, setIsSavedSearchMutationPending] = useState(false);
  const [results, setResults] = useState<HistorySearchResult[]>([]);
  const [hasSearched, setHasSearched] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const searchGeneration = useRef(0);
  const savedSearchMutationInFlight = useRef(false);
  const searchDraftRevision = useRef(0);
  const queryInputRef = useRef<HTMLInputElement>(null);
  const projects = useChatStore((state) => state.projects);
  const models = useChatStore((state) => state.models);
  const modelProviders = useChatStore((state) => state.modelProviders);

  const loadFilterOptions = useCallback(async () => {
    setFilterOptionsStatus('loading');
    try {
      setFilterOptions(await loadHistorySearchFilterOptions());
      setFilterOptionsStatus('ready');
    } catch {
      setFilterOptionsStatus('error');
    }
  }, []);

  const loadSavedSearches = useCallback(() => {
    setSavedSearchStatus('loading');
    try {
      setSavedSearches(loadSavedHistorySearches());
      setSavedSearchStatus('ready');
      setSavedSearchError(null);
    } catch (error) {
      if (!(error instanceof SavedHistorySearchError)) throw error;
      setSavedSearches([]);
      setSavedSearchStatus('error');
      setSavedSearchError('loadFailed');
    }
  }, []);

  useEffect(() => {
    if (open && filterOptionsStatus === 'idle') void loadFilterOptions();
  }, [filterOptionsStatus, loadFilterOptions, open]);

  useEffect(() => {
    if (open && savedSearchStatus === 'idle') loadSavedSearches();
  }, [loadSavedSearches, open, savedSearchStatus]);

  const providerOptions = useMemo(() => {
    const options = filterOptions.providerIds.map((providerId) => {
      const catalogName = modelProviders.find((provider) => provider.providerId === providerId)?.name;
      const model = models.find((candidate) => candidate.providerId === providerId);
      const name = catalogName ?? (model ? providerNameForModel(model, t) : providerId);
      return { value: providerId, label: name };
    });
    return [{ value: '', label: t('historySearchAllProviders') }, ...options];
  }, [filterOptions.providerIds, modelProviders, models, t]);

  const modelOptions = useMemo(() => {
    const options = filterOptions.modelIds.map((modelId) => {
      const model = models.find((candidate) => candidate.modelId === modelId);
      const label = model ? `${model.displayName} (${providerNameForModel(model, t)})` : modelId;
      return { value: modelId, label };
    });
    return [{ value: '', label: t('historySearchAllModels') }, ...options];
  }, [filterOptions.modelIds, models, t]);

  const projectOptions = useMemo(
    () => [{ value: '', label: t('historySearchAllProjects') }, ...projects.map((project) => ({ value: project.id, label: project.name }))],
    [projects, t],
  );

  const resultTypeOptions = useMemo(
    () => resultTypes.map((value) => ({ value, label: t(resultTypeKeys[value]) })),
    [t],
  );

  const sortOptions = useMemo(
    () => sortOrders.map((value) => ({ value, label: t(sortOrderKeys[value]) })),
    [t],
  );

  const savedSearchOptions = useMemo(
    () => [
      { value: '', label: t('savedHistorySearchesEmpty') },
      ...savedSearches.map((search) => ({ value: search.name, label: search.name })),
    ],
    [savedSearches, t],
  );

  const archivedOptions = useMemo(
    () => [
      { value: 'all', label: t('historySearchAllStatuses') },
      { value: 'active', label: t('historySearchActiveConversations') },
      { value: 'archived', label: t('historySearchArchivedConversations') },
    ],
    [t],
  );

  const resetSearchForInputChange = () => {
    searchDraftRevision.current += 1;
    setSelectedSavedSearchName('');
    searchGeneration.current += 1;
    setIsSearching(false);
    setResults([]);
    setHasSearched(false);
    setSearchError(null);
  };

  const updateFilters = (patch: Partial<HistorySearchFilters>) => {
    resetSearchForInputChange();
    setFilters((current) => ({ ...current, ...patch }));
  };

  const handleQueryChange = (value: string) => {
    resetSearchForInputChange();
    setQuery(value);
  };

  const handleSearch = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setSearchError(null);
    const dateRange = historySearchDateRange(fromDate, throughDate);
    if (!dateRange) {
      setSearchError(t('historySearchInvalidDateRange'));
      return;
    }
    const generation = searchGeneration.current + 1;
    searchGeneration.current = generation;
    setIsSearching(true);
    setHasSearched(true);
    setResults([]);
    try {
      const nextResults = await searchHistory(query, filters, dateRange.fromUnixMs, dateRange.throughUnixMs);
      if (searchGeneration.current === generation) setResults(nextResults);
    } catch (error) {
      if (searchGeneration.current !== generation) return;
      setSearchError(
        error instanceof HistorySearchInputError
          ? error.reason === 'tooShort'
            ? t('searchMessagesTooShort')
            : error.reason === 'tooLong'
              ? t('searchMessagesTooLong')
              : t('historySearchInvalidDateRange')
          : t('searchMessagesFailed'),
      );
    } finally {
      if (searchGeneration.current === generation) setIsSearching(false);
    }
  };

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      searchGeneration.current += 1;
      setIsSearching(false);
    }
    onOpenChange(nextOpen);
  };

  const clearFilters = () => {
    resetSearchForInputChange();
    setFilters(emptyHistorySearchFilters);
    setFromDate('');
    setThroughDate('');
  };

  const loadSavedSearch = (search: SavedHistorySearch) => {
    resetSearchForInputChange();
    setSelectedSavedSearchName(search.name);
    setQuery(search.query);
    setFilters(search.filters);
    setFromDate(search.fromUnixMs === null ? '' : localDateInputValue(search.fromUnixMs));
    setThroughDate(search.throughUnixMs === null ? '' : localDateInputValue(search.throughUnixMs - 1));
  };

  const handleSavedSearchSelection = (name: string) => {
    setSelectedSavedSearchName(name);
    const search = savedSearches.find((candidate) => candidate.name === name);
    if (search) loadSavedSearch(search);
  };

  const saveDateRange = historySearchDateRange(fromDate, throughDate);
  const canSaveSearch =
    savedSearchStatus === 'ready' &&
    savedSearchName.trim().length > 0 &&
    Array.from(query.trim()).length >= 2 &&
    Array.from(query.trim()).length <= 512 &&
    saveDateRange !== null;

  const handleSaveSearch = async () => {
    if (!canSaveSearch || !saveDateRange || savedSearchMutationInFlight.current) return;
    savedSearchMutationInFlight.current = true;
    setIsSavedSearchMutationPending(true);
    const draftRevision = searchDraftRevision.current;
    const savedName = savedSearchName.trim();
    try {
      const nextSearches = await saveHistorySearch({
        name: savedName,
        query: query.trim(),
        filters,
        fromUnixMs: saveDateRange.fromUnixMs,
        throughUnixMs: saveDateRange.throughUnixMs,
      });
      setSavedSearches(nextSearches);
      if (searchDraftRevision.current === draftRevision) setSelectedSavedSearchName(savedName);
      setSavedSearchName('');
      setSavedSearchError(null);
    } catch (error) {
      if (error instanceof SavedHistorySearchError) {
        setSavedSearchError(error.reason);
      } else {
        globalThis.reportError(new Error('Failed to save a history search.', { cause: error }));
        setSavedSearchError('saveFailed');
      }
    } finally {
      savedSearchMutationInFlight.current = false;
      setIsSavedSearchMutationPending(false);
    }
  };

  const handleDeleteSavedSearch = async () => {
    if (!selectedSavedSearchName || savedSearchMutationInFlight.current) return;
    savedSearchMutationInFlight.current = true;
    setIsSavedSearchMutationPending(true);
    try {
      setSavedSearches(await deleteHistorySearch(selectedSavedSearchName));
      setSelectedSavedSearchName('');
      setSavedSearchError(null);
    } catch (error) {
      if (error instanceof SavedHistorySearchError) {
        setSavedSearchError(error.reason);
      } else {
        globalThis.reportError(new Error('Failed to delete a saved history search.', { cause: error }));
        setSavedSearchError('deleteFailed');
      }
    } finally {
      savedSearchMutationInFlight.current = false;
      setIsSavedSearchMutationPending(false);
    }
  };

  const filtersActive =
    filters.providerId !== null ||
    filters.modelId !== null ||
    filters.projectId !== null ||
    filters.isArchived !== null ||
    filters.resultType !== 'all' ||
    filters.sortOrder !== 'relevance' ||
    fromDate.length > 0 ||
    throughDate.length > 0;

  useEffect(() => {
    onFiltersActiveChange(filtersActive);
  }, [filtersActive, onFiltersActiveChange]);

  const selectResult = (result: HistorySearchResult) => {
    if (result.kind === 'project') {
      if (result.projectId) onSelectProject?.(result.projectId);
    } else {
      onSelectConversation(
        result.conversationId,
        result.messageId,
        result.messageId === null ? null : query.trim(),
      );
    }
    handleOpenChange(false);
  };

  const formatResultDate = (timestamp: number) =>
    new Date(timestamp).toLocaleString(i18n.language, { dateStyle: 'medium', timeStyle: 'short' });

  const updateSelectFilter = (key: 'providerId' | 'modelId' | 'projectId', value: string) => {
    updateFilters({ [key]: value.length > 0 ? value : null });
  };

  return (
    <Dialog
        initialFocusRef={queryInputRef}
        onOpenChange={handleOpenChange}
        open={open}
        size="wide"
        title={t('historySearchTitle')}
      >
        <form className="grid gap-4" onSubmit={(event) => void handleSearch(event)}>
          <Input
            autoComplete="off"
            label={t('searchChats')}
            maxLength={512}
            onChange={(event) => handleQueryChange(event.target.value)}
            ref={queryInputRef}
            value={query}
          />
          <section aria-label={t('savedHistorySearchesTitle')} className="grid gap-2">
            <div className="grid items-end gap-2 sm:grid-cols-2">
              <Select
                disabled={
                  isSavedSearchMutationPending || savedSearchStatus !== 'ready' || savedSearches.length === 0
                }
                label={t('savedHistorySearchesTitle')}
                onChange={(event) => handleSavedSearchSelection(event.target.value)}
                options={savedSearchOptions}
                value={selectedSavedSearchName}
              />
              <Button
                aria-label={`${t('deleteSavedHistorySearch')}${selectedSavedSearchName ? `: ${selectedSavedSearchName}` : ''}`}
                disabled={
                  isSavedSearchMutationPending || !selectedSavedSearchName || savedSearchStatus !== 'ready'
                }
                onClick={handleDeleteSavedSearch}
                size="small"
                type="button"
                variant="ghost"
              >
                <Trash2 aria-hidden="true" size={14} />
                {t('deleteSavedHistorySearch')}
              </Button>
            </div>
            <div className="grid items-end gap-2 sm:grid-cols-2">
              <Input
                disabled={isSavedSearchMutationPending}
                label={t('savedHistorySearchName')}
                maxLength={60}
                onChange={(event) => setSavedSearchName(event.target.value)}
                value={savedSearchName}
              />
              <Button
                disabled={isSavedSearchMutationPending || !canSaveSearch}
                onClick={handleSaveSearch}
                size="small"
                type="button"
                variant="secondary"
              >
                <Save aria-hidden="true" size={14} />
                {t('saveCurrentHistorySearch')}
              </Button>
            </div>
            {savedSearchError ? (
              <div className="flex items-center justify-between gap-3">
                <p className="text-xs text-destructive" role="alert">
                  {t(savedSearchErrorTranslationKey(savedSearchError))}
                </p>
                {savedSearchError === 'loadFailed' ? (
                  <Button onClick={loadSavedSearches} size="small" type="button" variant="secondary">
                    {t('retry')}
                  </Button>
                ) : null}
              </div>
            ) : null}
          </section>
          <div className="grid gap-3 sm:grid-cols-2">
            <Select
              label={t('historySearchResultTypeFilter')}
              onChange={(event) => {
                const value = resultTypes.find((resultType) => resultType === event.target.value);
                if (value) updateFilters({ resultType: value });
              }}
              options={resultTypeOptions}
              value={filters.resultType}
            />
            <Select
              disabled={filterOptionsStatus !== 'ready'}
              label={t('historySearchProviderFilter')}
              onChange={(event) => updateSelectFilter('providerId', event.target.value)}
              options={providerOptions}
              value={filters.providerId ?? ''}
            />
            <Select
              disabled={filterOptionsStatus !== 'ready'}
              label={t('historySearchModelFilter')}
              onChange={(event) => updateSelectFilter('modelId', event.target.value)}
              options={modelOptions}
              value={filters.modelId ?? ''}
            />
            <Select
              label={t('historySearchProjectFilter')}
              onChange={(event) => updateSelectFilter('projectId', event.target.value)}
              options={projectOptions}
              value={filters.projectId ?? ''}
            />
            <Select
              label={t('historySearchArchiveFilter')}
              onChange={(event) =>
                updateFilters({
                  isArchived: event.target.value === 'all' ? null : event.target.value === 'archived',
                })
              }
              options={archivedOptions}
              value={filters.isArchived === null ? 'all' : filters.isArchived ? 'archived' : 'active'}
            />
            <Select
              label={t('historySearchSortFilter')}
              onChange={(event) => {
                const value = sortOrders.find((sortOrder) => sortOrder === event.target.value);
                if (value) updateFilters({ sortOrder: value });
              }}
              options={sortOptions}
              value={filters.sortOrder}
            />
            <Input
              label={t('historySearchFromDate')}
              onChange={(event) => {
                resetSearchForInputChange();
                setFromDate(event.target.value);
              }}
              type="date"
              value={fromDate}
            />
            <Input
              label={t('historySearchThroughDate')}
              onChange={(event) => {
                resetSearchForInputChange();
                setThroughDate(event.target.value);
              }}
              type="date"
              value={throughDate}
            />
          </div>
          {filterOptionsStatus === 'loading' ? (
            <p className="text-sm text-secondary-text" role="status">
              {t('historySearchFiltersLoading')}
            </p>
          ) : null}
          {filterOptionsStatus === 'error' ? (
            <div className="flex items-center justify-between gap-3 rounded-control border border-destructive/40 p-3">
              <p className="text-sm text-destructive" role="alert">
                {t('historySearchFiltersLoadFailed')}
              </p>
              <Button
                onClick={() => void loadFilterOptions()}
                size="small"
                type="button"
                variant="secondary"
              >
                {t('retry')}
              </Button>
            </div>
          ) : null}
          <p className="text-xs text-secondary-text">{t('historySearchRouteFilterNote')}</p>
          {filtersActive ? (
            <p className="text-xs text-secondary-text" role="status">
              {t('historySearchFiltersActive')}
            </p>
          ) : null}
          {searchError ? (
            <p className="text-sm text-destructive" role="alert">
              {searchError}
            </p>
          ) : null}
          {isSearching ? (
            <p className="flex items-center gap-2 text-sm text-secondary-text" role="status">
              <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
              {t('searchMessagesLoading')}
            </p>
          ) : null}
          {hasSearched && !isSearching && results.length === 0 && searchError === null ? (
            <p className="text-sm text-secondary-text" role="status">
              {t('searchMessagesNoResults')}
            </p>
          ) : null}
          {results.length > 0 ? (
            <section aria-label={t('searchMessagesHeader')} className="grid gap-2">
              <h3 className="text-sm font-medium text-text">{t('searchMessagesHeader')}</h3>
              <ul className="openchat-scroll-area grid max-h-64 gap-1 overflow-y-auto">
                {results.map((result) => (
                  <li key={historyResultKey(result)}>
                    <Button
                      aria-label={
                        result.kind === 'project'
                          ? t('projectNewChatButtonTitle', { name: result.conversationTitle })
                          : t('selectConversation', { title: result.conversationTitle })
                      }
                      className="h-auto min-h-12 w-full justify-start gap-3 rounded-control px-3 py-2 text-left"
                      disabled={result.kind === 'project' && !onSelectProject}
                      onClick={() => selectResult(result)}
                      size="small"
                      type="button"
                      variant="ghost"
                    >
                      {result.kind === 'project' ? (
                        <FolderOpen aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                      ) : result.kind === 'message' ? (
                        <FileSearch aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                      ) : (
                        <MessageSquare aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                      )}
                      <span className="grid min-w-0 flex-1 gap-0.5">
                        <span className="flex min-w-0 items-center justify-between gap-3">
                          <span className="truncate font-medium text-text">{result.conversationTitle}</span>
                          <span className="shrink-0 text-xs text-secondary-text">
                            <CalendarDays aria-hidden="true" className="mr-1 inline" size={12} />
                            {formatResultDate(result.createdAtUnixMs)}
                          </span>
                        </span>
                        <span className="truncate text-xs text-secondary-text">
                          {result.role ? `${t(result.role === 'user' ? 'searchFilterUser' : 'searchFilterAssistant')}: ` : ''}
                          {result.excerpt}
                        </span>
                      </span>
                    </Button>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
          <div className="flex flex-wrap justify-between gap-2 border-t border-border pt-3">
            <Button onClick={clearFilters} size="small" type="button" variant="ghost">
              {t('historySearchClearFilters')}
            </Button>
            <div className="flex gap-2">
              <Button onClick={() => handleOpenChange(false)} size="small" type="button" variant="secondary">
                {t('cancel')}
              </Button>
              <Button disabled={isSearching} size="small" type="submit" variant="primary">
                <Search aria-hidden="true" size={14} />
                {t('historySearchRun')}
              </Button>
            </div>
          </div>
        </form>
    </Dialog>
  );
}

function historyResultKey(result: HistorySearchResult): string {
  if (result.kind === 'project') return `project:${result.projectId ?? result.conversationTitle}`;
  return `${result.conversationId}:${result.messageId ?? 'conversation'}`;
}

function localDateInputValue(timestamp: number): string {
  const date = new Date(timestamp);
  const year = String(date.getFullYear()).padStart(4, '0');
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function savedSearchErrorTranslationKey(reason: SavedHistorySearchError['reason']):
  | 'savedHistorySearchesLoadFailed'
  | 'savedHistorySearchSaveFailed'
  | 'savedHistorySearchDeleteFailed'
  | 'savedHistorySearchLimitReached' {
  if (reason === 'loadFailed') return 'savedHistorySearchesLoadFailed';
  if (reason === 'limitReached') return 'savedHistorySearchLimitReached';
  if (reason === 'deleteFailed') return 'savedHistorySearchDeleteFailed';
  return 'savedHistorySearchSaveFailed';
}
