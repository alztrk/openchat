import { Fragment, useEffect, useRef, useState, type ReactNode } from 'react';
import type { TFunction } from 'i18next';
import { Bookmark, BookmarkPlus, CalendarDays, CircleAlert, LoaderCircle, Search, Trash2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import {
  deleteHistorySearch,
  emptyHistorySearchFilters,
  HistorySearchInputError,
  loadHistorySearchFilterOptions,
  loadSavedHistorySearches,
  SavedHistorySearchError,
  saveHistorySearch,
  searchHistory,
  type HistoryResultType,
  type HistorySearchFilters,
  type HistorySearchResult,
  type HistorySortOrder,
  type SavedHistorySearch,
} from '@/features/chat/data/historySearch';
import type { ChatProject } from '@/features/chat/data/chatStorage';
import { Button, Dialog, Input, Select, type SelectOption } from '@/shared/ui';

interface ConversationSearchDialogProps {
  open: boolean;
  initialQuery?: string;
  projects: readonly ChatProject[];
  projectsStatus: 'idle' | 'loading' | 'ready' | 'error';
  onOpenChange(open: boolean): void;
  onSelectResult(result: HistorySearchResult): void;
}

type SearchState = 'idle' | 'loading' | 'ready' | 'error';
type FilterState = 'loading' | 'ready' | 'error';
type SearchErrorKey = 'searchMessagesTooShort' | 'searchMessagesTooLong' | 'searchMessagesFailed';
type SavedSearchErrorKey = 'savedHistorySearchesLoadFailed' | 'savedHistorySearchSaveFailed' | 'savedHistorySearchLimitReached';

export function ConversationSearchDialog({
  open,
  initialQuery = '',
  onOpenChange,
  onSelectResult,
  projects,
  projectsStatus,
}: ConversationSearchDialogProps) {
  const { i18n, t } = useTranslation();
  const [query, setQuery] = useState(initialQuery);
  const [filters, setFilters] = useState<HistorySearchFilters>(emptyHistorySearchFilters);
  const [draftFilters, setDraftFilters] = useState<HistorySearchFilters>(emptyHistorySearchFilters);
  const [fromDate, setFromDate] = useState('');
  const [throughDate, setThroughDate] = useState('');
  const [draftFromDate, setDraftFromDate] = useState('');
  const [draftThroughDate, setDraftThroughDate] = useState('');
  const [results, setResults] = useState<HistorySearchResult[]>([]);
  const [searchState, setSearchState] = useState<SearchState>('idle');
  const [searchErrorKey, setSearchErrorKey] = useState<SearchErrorKey | null>(null);
  const [retryGeneration, setRetryGeneration] = useState(0);
  const [filterState, setFilterState] = useState<FilterState>('loading');
  const [providerIds, setProviderIds] = useState<string[]>([]);
  const [modelIds, setModelIds] = useState<string[]>([]);
  const [tags, setTags] = useState<string[]>([]);
  const [savedSearches, setSavedSearches] = useState<SavedHistorySearch[]>([]);
  const [savedSearchError, setSavedSearchError] = useState<SavedSearchErrorKey | null>(null);
  const [saveName, setSaveName] = useState('');
  const [savedSearchesOpen, setSavedSearchesOpen] = useState(false);
  const [savePromptOpen, setSavePromptOpen] = useState(false);
  const searchGeneration = useRef(0);
  const filterGeneration = useRef(0);

  useEffect(() => {
    if (!open) {
      searchGeneration.current++;
      filterGeneration.current++;
      setQuery('');
      setFilters(emptyHistorySearchFilters);
      setDraftFilters(emptyHistorySearchFilters);
      setFromDate('');
      setThroughDate('');
      setDraftFromDate('');
      setDraftThroughDate('');
      setResults([]);
      setSearchState('idle');
      setSearchErrorKey(null);
      setSaveName('');
      setSavedSearchesOpen(false);
      setSavePromptOpen(false);
      return;
    }

    setQuery(initialQuery);
    const generation = ++filterGeneration.current;
    setFilterState('loading');
    setSavedSearchError(null);
    try {
      setSavedSearches(loadSavedHistorySearches());
    } catch (error) {
      setSavedSearchError(savedSearchErrorKey(error));
    }
    void loadHistorySearchFilterOptions().then((options) => {
      if (filterGeneration.current !== generation) return;
      setProviderIds(options.providerIds);
      setModelIds(options.modelIds);
      setTags(options.tags);
      setFilterState('ready');
    }).catch(() => {
      if (filterGeneration.current !== generation) return;
      setFilterState('error');
    });
    return () => { filterGeneration.current++; };
  }, [initialQuery, open]);

  useEffect(() => {
    if (!open) return;
    const normalizedQuery = query.trim();
    const queryLength = Array.from(normalizedQuery).length;
    if (normalizedQuery.length === 0) {
      searchGeneration.current++;
      setResults([]);
      setSearchState('idle');
      setSearchErrorKey(null);
      return;
    }
    if (queryLength < 2) {
      searchGeneration.current++;
      setResults([]);
      setSearchState('idle');
      setSearchErrorKey('searchMessagesTooShort');
      return;
    }
    if (queryLength > 512) {
      searchGeneration.current++;
      setResults([]);
      setSearchState('idle');
      setSearchErrorKey('searchMessagesTooLong');
      return;
    }

    const generation = ++searchGeneration.current;
    setSearchState('loading');
    setSearchErrorKey(null);
    const timeout = window.setTimeout(() => {
      const fromUnixMs = startOfLocalDay(fromDate);
      const throughUnixMs = endOfLocalDayExclusive(throughDate);
      void searchHistory(normalizedQuery, filters, fromUnixMs, throughUnixMs)
        .then((nextResults) => {
          if (searchGeneration.current !== generation) return;
          setResults(nextResults);
          setSearchState('ready');
        })
        .catch((error: unknown) => {
          if (searchGeneration.current !== generation) return;
          setSearchState('error');
          setResults([]);
          setSearchErrorKey(searchErrorKeyFor(error));
        });
    }, 250);
    return () => {
      window.clearTimeout(timeout);
      if (searchGeneration.current === generation) searchGeneration.current++;
    };
  }, [filters, fromDate, open, query, retryGeneration, throughDate]);

  const saveCurrentSearch = () => {
    const name = saveName.trim();
    if (name.length === 0 || name.length > 60 || Array.from(query.trim()).length < 2) return;
    try {
      const next = saveHistorySearch({
        name,
        query: query.trim(),
        filters,
        fromUnixMs: startOfLocalDay(fromDate),
        throughUnixMs: endOfLocalDayExclusive(throughDate),
      });
      setSavedSearches(next);
      setSaveName('');
      setSavedSearchError(null);
      setSavePromptOpen(false);
    } catch (error) {
      setSavedSearchError(savedSearchErrorKey(error));
    }
  };

  const removeSavedSearch = (name: string) => {
    try {
      setSavedSearches(deleteHistorySearch(name));
      setSavedSearchError(null);
    } catch (error) {
      setSavedSearchError(savedSearchErrorKey(error));
    }
  };

  const applySavedSearch = (saved: SavedHistorySearch) => {
    setQuery(saved.query);
    setFilters(saved.filters);
    setDraftFilters(saved.filters);
    const nextFromDate = dateInputValue(saved.fromUnixMs);
    const nextThroughDate = dateInputValue(saved.throughUnixMs === null ? null : saved.throughUnixMs - 1);
    setFromDate(nextFromDate);
    setThroughDate(nextThroughDate);
    setDraftFromDate(nextFromDate);
    setDraftThroughDate(nextThroughDate);
  };

  const applySavedSearchAndClose = (saved: SavedHistorySearch) => {
    applySavedSearch(saved);
    setSavedSearchesOpen(false);
  };

  const setQuickResultType = (resultType: Extract<HistoryResultType, 'all' | 'chats' | 'projects' | 'savedResponses'>) => {
    setFilters((current) => ({ ...current, resultType }));
    setDraftFilters((current) => ({ ...current, resultType }));
  };

  const filtersAreActive = !areFiltersEmpty(filters) || fromDate.length > 0 || throughDate.length > 0;
  const queryLength = Array.from(query.trim()).length;
  const canSaveSearch = queryLength >= 2 && queryLength <= 512;
  const canSubmitSave = canSaveSearch && saveName.trim().length > 0 && saveName.trim().length <= 60;
  const selectedQuickResultType = isQuickResultType(filters.resultType) ? filters.resultType : 'all';
  const handleMainOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      setSavedSearchesOpen(false);
      setSavePromptOpen(false);
    }
    onOpenChange(nextOpen);
  };

  return (
    <>
    <Dialog
      className="w-[min(58rem,calc(100vw-2rem))]"
      onOpenChange={handleMainOpenChange}
      open={open}
      title={t('searchMessagesHeader')}
    >
      <div className="grid gap-4">
        <div className="flex justify-end">
          <Button aria-label={t('cancel')} onClick={() => handleMainOpenChange(false)} size="icon" variant="ghost">
            <X aria-hidden="true" size={18} />
          </Button>
        </div>
        <div className="relative">
          <Search aria-hidden="true" className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-secondary-icon" size={17} />
          <Input
            aria-label={t('searchMessagesTooltip')}
            className="pl-10"
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('searchMessagesTooltip')}
            value={query}
          />
        </div>

        <details
          className="openchat-native-disclosure rounded-control border border-border bg-surface p-3"
          onToggle={(event) => {
            if (event.currentTarget.open) {
              setDraftFilters(filters);
              setDraftFromDate(fromDate);
              setDraftThroughDate(throughDate);
            }
          }}
        >
          <summary className="cursor-pointer select-none text-sm font-medium text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus">
            <span className="inline-flex items-center gap-2">
              <CalendarDays aria-hidden="true" size={16} />
              {t(filtersAreActive ? 'historySearchFiltersActive' : 'historySearchFiltersTitle')}
            </span>
          </summary>
          <div className="mt-4 grid gap-4">
            <p className="text-xs text-secondary-text">{t('historySearchRouteFilterNote')}</p>
            {filterState === 'loading' ? <p aria-live="polite" className="text-sm text-secondary-text">{t('historySearchFiltersLoading')}</p> : null}
            {filterState === 'error' ? <p className="text-sm text-destructive" role="alert">{t('searchMessagesFailed')}</p> : null}
            <div className="grid gap-3 sm:grid-cols-2">
              <Select
                label={t('historySearchResultTypeFilter')}
                onChange={(event) => setDraftFilters((current) => ({ ...current, resultType: parseResultType(event.target.value) }))}
                options={resultTypeOptions(t, draftFilters.resultType)}
                value={draftFilters.resultType}
              />
              <Select
                label={t('historySearchSortFilter')}
                onChange={(event) => setDraftFilters((current) => ({ ...current, sortOrder: parseSortOrder(event.target.value) }))}
                options={sortOrderOptions(t)}
                value={draftFilters.sortOrder}
              />
              <Select
                disabled={projectsStatus !== 'ready'}
                label={t('historySearchProjectFilter')}
                onChange={(event) => setDraftFilters((current) => ({ ...current, projectId: event.target.value || null }))}
                options={projectOptions(t('historySearchAllProjects'), projects)}
                value={draftFilters.projectId ?? ''}
              />
              <Select
                disabled={filterState !== 'ready'}
                label={t('historySearchProviderFilter')}
                onChange={(event) => setDraftFilters((current) => ({ ...current, providerId: event.target.value || null }))}
                options={stringOptions(t('historySearchAllProviders'), providerIds)}
                value={draftFilters.providerId ?? ''}
              />
              <Select
                disabled={filterState !== 'ready'}
                label={t('historySearchModelFilter')}
                onChange={(event) => setDraftFilters((current) => ({ ...current, modelId: event.target.value || null }))}
                options={stringOptions(t('historySearchAllModels'), modelIds)}
                value={draftFilters.modelId ?? ''}
              />
              <Select
                disabled={filterState !== 'ready'}
                label={t('historySearchTagFilter')}
                onChange={(event) => setDraftFilters((current) => ({ ...current, tag: event.target.value || null }))}
                options={stringOptions(t('historySearchAllTags'), tags)}
                value={draftFilters.tag ?? ''}
              />
              <Select
                label={t('historySearchArchiveFilter')}
                onChange={(event) => setDraftFilters((current) => ({
                  ...current,
                  isArchived: event.target.value === 'active' ? false : event.target.value === 'archived' ? true : null,
                }))}
                options={archiveOptions(t)}
                value={draftFilters.isArchived === true ? 'archived' : draftFilters.isArchived === false ? 'active' : 'all'}
              />
            </div>
            {projectsStatus === 'loading' || projectsStatus === 'idle' ? <p aria-live="polite" className="text-sm text-secondary-text">{t('historySearchFiltersLoading')}</p> : null}
            {projectsStatus === 'error' ? <p className="text-sm text-destructive" role="alert">{t('projectLoadFailed')}</p> : null}
            <fieldset className="grid gap-2">
                <legend className="text-sm font-medium text-text">{t('historySearchDateFilter')}</legend>
              <div className="grid gap-3 sm:grid-cols-2">
                <Input
                  aria-label={t('historySearchFromDate')}
                  label={t('historySearchFromDate')}
                  max={draftThroughDate || undefined}
                  onChange={(event) => setDraftFromDate(event.target.value)}
                  type="date"
                  value={draftFromDate}
                />
                <Input
                  aria-label={t('historySearchThroughDate')}
                  label={t('historySearchThroughDate')}
                  min={draftFromDate || undefined}
                  onChange={(event) => setDraftThroughDate(event.target.value)}
                  type="date"
                  value={draftThroughDate}
                />
              </div>
            </fieldset>
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                onClick={() => {
                  setDraftFilters(emptyHistorySearchFilters);
                  setDraftFromDate('');
                  setDraftThroughDate('');
                  setFilters(emptyHistorySearchFilters);
                  setFromDate('');
                  setThroughDate('');
                }}
                size="small"
                variant="ghost"
              >
                {t('historySearchClearFilters')}
              </Button>
              <Button
                onClick={() => {
                  setFilters(draftFilters);
                  setFromDate(draftFromDate);
                  setThroughDate(draftThroughDate);
                }}
                size="small"
                variant="primary"
              >
                {t('historySearchApplyFilters')}
              </Button>
            </div>
          </div>
        </details>

        <div className="flex flex-wrap items-center justify-between gap-2" role="group" aria-label={t('historySearchResultTypeFilter')}>
          <div className="flex flex-wrap gap-2">
            {quickResultTypeOptions(t).map((option) => (
              <Button
                aria-pressed={selectedQuickResultType === option.value}
                key={option.value}
                onClick={() => setQuickResultType(option.value)}
                size="small"
                variant={selectedQuickResultType === option.value ? 'primary' : 'secondary'}
              >
                {option.label}
              </Button>
            ))}
          </div>
          <Button
            aria-label={t('savedHistorySearchesTitle')}
            onClick={() => setSavedSearchesOpen(true)}
            size="icon"
            title={t('savedHistorySearchesTitle')}
            variant="ghost"
          >
            <Bookmark aria-hidden="true" size={17} />
          </Button>
        </div>

        {searchState === 'loading' ? (
          <p aria-live="polite" className="flex items-center gap-2 text-sm text-secondary-text">
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            {t('searchMessagesLoading')}
          </p>
        ) : null}
        {searchState === 'error' ? (
          <div className="flex items-center justify-between gap-3">
            <p className="flex items-center gap-2 text-sm text-destructive" role="alert"><CircleAlert aria-hidden="true" size={16} />{t(searchErrorKey ?? 'searchMessagesFailed')}</p>
            <Button onClick={() => setRetryGeneration((current) => current + 1)} size="small">{t('retry')}</Button>
          </div>
        ) : null}
        {searchState === 'ready' && results.length === 0 ? <p className="text-sm text-secondary-text">{t('searchMessagesNoResults')}</p> : null}
        {results.length > 0 ? (
          <ul aria-label={t('searchMessagesHeader')} className="openchat-scroll-area grid max-h-80 gap-2 overflow-auto">
            {results.map((result) => (
              <li key={`${result.kind}:${result.projectId ?? result.conversationId}:${result.messageId ?? 'title'}:${result.createdAtUnixMs}`}>
                <button
                  className="grid min-h-16 w-full gap-1 rounded-control border border-border bg-surface p-3 text-left hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  onClick={() => onSelectResult(result)}
                  type="button"
                >
                  <span className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
                    <span className="min-w-0 flex-1 truncate font-medium text-text">{result.conversationTitle}</span>
                    <time className="shrink-0 text-xs text-secondary-text" dateTime={new Date(result.createdAtUnixMs).toISOString()}>
                      {new Date(result.createdAtUnixMs).toLocaleString(i18n.language)}
                    </time>
                  </span>
                  <span className="line-clamp-3 break-words text-sm text-secondary-text">{renderHistoryExcerpt(result.excerpt)}</span>
                  {result.kind === 'project' ? <span className="text-xs text-secondary-text">{t('projects')}</span> : result.kind === 'savedOutput' ? <span className="text-xs text-secondary-text">{t('outputs')}</span> : result.role ? <span className="text-xs text-secondary-text">{t(result.role === 'user' ? 'conversationMemoryUserMessage' : 'conversationMemoryAssistantMessage')}</span> : null}
                </button>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </Dialog>

    <SavedHistorySearchesDialog
      canSaveSearch={canSaveSearch}
      errorKey={savedSearchError}
      onDelete={removeSavedSearch}
      onOpenChange={setSavedSearchesOpen}
      onSaveCurrent={() => {
        setSaveName(query.trim());
        setSavedSearchesOpen(false);
        setSavePromptOpen(true);
      }}
      onSelect={applySavedSearchAndClose}
      open={open && savedSearchesOpen}
      searches={savedSearches}
    />

    <Dialog
      className="w-[min(28rem,calc(100vw-2rem))]"
      onOpenChange={setSavePromptOpen}
      open={open && savePromptOpen}
      title={t('saveHistorySearchTitle')}
    >
      <div className="grid gap-4">
        <Input
          aria-label={t('savedHistorySearchName')}
          autoFocus
          maxLength={60}
          onChange={(event) => setSaveName(event.target.value)}
          placeholder={t('savedHistorySearchName')}
          value={saveName}
        />
        {savedSearchError ? <p className="text-sm text-destructive" role="alert">{t(savedSearchError)}</p> : null}
        <div className="flex justify-end gap-2">
          <Button onClick={() => setSavePromptOpen(false)} variant="ghost">{t('cancel')}</Button>
          <Button disabled={!canSubmitSave} onClick={saveCurrentSearch} variant="primary">
            {t('save')}
          </Button>
        </div>
      </div>
    </Dialog>
    </>
  );
}

interface SavedHistorySearchesDialogProps {
  canSaveSearch: boolean;
  errorKey: SavedSearchErrorKey | null;
  onDelete(name: string): void;
  onOpenChange(open: boolean): void;
  onSaveCurrent(): void;
  onSelect(search: SavedHistorySearch): void;
  open: boolean;
  searches: readonly SavedHistorySearch[];
}

function SavedHistorySearchesDialog({
  canSaveSearch,
  errorKey,
  onDelete,
  onOpenChange,
  onSaveCurrent,
  onSelect,
  open,
  searches,
}: SavedHistorySearchesDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog
      className="w-[min(32rem,calc(100vw-2rem))]"
      onOpenChange={onOpenChange}
      open={open}
      title={t('savedHistorySearchesTitle')}
    >
      <div className="grid gap-4">
        {errorKey ? <p className="text-sm text-destructive" role="alert">{t(errorKey)}</p> : null}
        {searches.length === 0 ? <p className="text-sm text-secondary-text">{t('savedHistorySearchesEmpty')}</p> : (
          <ul className="openchat-scroll-area grid max-h-80 gap-2 overflow-auto">
            {searches.map((saved) => (
              <li className="flex flex-wrap items-center gap-2 rounded-control border border-border bg-surface p-2" key={saved.name.toLocaleLowerCase('en-US')}>
                <button
                  className="min-w-0 flex-1 rounded-control px-2 py-1 text-left hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  onClick={() => onSelect(saved)}
                  type="button"
                >
                  <span className="block truncate text-sm font-medium text-text">{saved.name}</span>
                  <span className="block truncate text-xs text-secondary-text">{saved.query}</span>
                </button>
                <Button aria-label={`${t('deleteSavedHistorySearch')}: ${saved.name}`} onClick={() => onDelete(saved.name)} size="icon" variant="ghost">
                  <Trash2 aria-hidden="true" size={16} />
                </Button>
              </li>
            ))}
          </ul>
        )}
        <div className="flex flex-wrap justify-end gap-2">
          <Button onClick={() => onOpenChange(false)} variant="ghost">{t('close')}</Button>
          <Button disabled={!canSaveSearch} onClick={onSaveCurrent} variant="secondary">
            <BookmarkPlus aria-hidden="true" size={16} />
            {t('saveCurrentHistorySearch')}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

function quickResultTypeOptions(t: TFunction): Array<{ value: Extract<HistoryResultType, 'all' | 'chats' | 'projects' | 'savedResponses'>; label: string }> {
  return [
    { value: 'all', label: t('searchFilterAll') },
    { value: 'chats', label: t('searchFilterChats') },
    { value: 'projects', label: t('searchFilterProjects') },
    { value: 'savedResponses', label: t('searchFilterSavedResponses') },
  ];
}

function isQuickResultType(value: HistoryResultType): value is Extract<HistoryResultType, 'all' | 'chats' | 'projects' | 'savedResponses'> {
  return value === 'all' || value === 'chats' || value === 'projects' || value === 'savedResponses';
}

function renderHistoryExcerpt(excerpt: string): ReactNode {
  const opening = '[match]';
  const closing = '[/match]';
  const parts: ReactNode[] = [];
  let offset = 0;
  let partIndex = 0;
  while (offset < excerpt.length) {
    const openingIndex = excerpt.indexOf(opening, offset);
    if (openingIndex < 0) {
      parts.push(<Fragment key={`history-excerpt-${partIndex++}`}>{excerpt.slice(offset)}</Fragment>);
      break;
    }
    const closingIndex = excerpt.indexOf(closing, openingIndex + opening.length);
    if (closingIndex < 0) {
      parts.push(<Fragment key={`history-excerpt-${partIndex++}`}>{excerpt.slice(offset)}</Fragment>);
      break;
    }
    const matchText = excerpt.slice(openingIndex + opening.length, closingIndex);
    if (matchText.includes(opening) || matchText.includes(closing)) {
      parts.push(<Fragment key={`history-excerpt-${partIndex++}`}>{excerpt.slice(offset)}</Fragment>);
      break;
    }
    if (openingIndex > offset) {
      parts.push(<Fragment key={`history-excerpt-${partIndex++}`}>{excerpt.slice(offset, openingIndex)}</Fragment>);
    }
    parts.push(
      <mark className="rounded-sm bg-selected px-0.5 text-text" key={`history-excerpt-match-${partIndex++}`}>
        {matchText}
      </mark>,
    );
    offset = closingIndex + closing.length;
  }
  return parts.length > 0 ? parts : excerpt;
}

function resultTypeOptions(t: TFunction, selectedResultType: HistoryResultType): SelectOption[] {
  const options: SelectOption[] = [
    { value: 'all', label: t('searchFilterAll') },
    { value: 'chats', label: t('searchFilterChats') },
    { value: 'projects', label: t('searchFilterProjects') },
    { value: 'savedResponses', label: t('searchFilterSavedResponses') },
  ];
  if (selectedResultType === 'messages') options.push({ value: 'messages', label: t('historySearchMessagesOnly') });
  if (selectedResultType === 'conversations') options.push({ value: 'conversations', label: t('historySearchConversationsOnly') });
  return options;
}

function sortOrderOptions(t: TFunction): SelectOption[] {
  return [
    { value: 'relevance', label: t('historySearchSortRelevance') },
    { value: 'newest', label: t('historySearchSortNewest') },
    { value: 'oldest', label: t('historySearchSortOldest') },
  ];
}

function archiveOptions(t: TFunction): SelectOption[] {
  return [
    { value: 'all', label: t('historySearchAllStatuses') },
    { value: 'active', label: t('historySearchActiveConversations') },
    { value: 'archived', label: t('historySearchArchivedConversations') },
  ];
}

function stringOptions(allLabel: string, values: string[]): SelectOption[] {
  return [{ value: '', label: allLabel }, ...values.map((value) => ({ value, label: value }))];
}

function projectOptions(allLabel: string, projects: readonly ChatProject[]): SelectOption[] {
  return [
    { value: '', label: allLabel },
    ...projects.map((project) => ({ value: project.id, label: project.name })),
  ];
}

function parseResultType(value: string): HistoryResultType {
  if (
    value === 'chats' ||
    value === 'projects' ||
    value === 'savedResponses' ||
    value === 'messages' ||
    value === 'conversations'
  ) return value;
  return 'all';
}

function parseSortOrder(value: string): HistorySortOrder {
  return value === 'newest' || value === 'oldest' ? value : 'relevance';
}

function areFiltersEmpty(filters: HistorySearchFilters): boolean {
  return filters.providerId === null &&
    filters.modelId === null &&
    filters.projectId === null &&
    filters.isArchived === null &&
    filters.tag === null &&
    filters.resultType === 'all' &&
    filters.sortOrder === 'relevance';
}

function startOfLocalDay(value: string): number | null {
  if (!value) return null;
  const date = new Date(`${value}T00:00:00`);
  return Number.isFinite(date.getTime()) ? date.getTime() : null;
}

function endOfLocalDayExclusive(value: string): number | null {
  if (!value) return null;
  const date = new Date(`${value}T00:00:00`);
  if (!Number.isFinite(date.getTime())) return null;
  date.setDate(date.getDate() + 1);
  return date.getTime();
}

function dateInputValue(timestamp: number | null): string {
  if (timestamp === null) return '';
  const date = new Date(timestamp);
  if (!Number.isFinite(date.getTime())) return '';
  const year = date.getFullYear().toString().padStart(4, '0');
  const month = (date.getMonth() + 1).toString().padStart(2, '0');
  const day = date.getDate().toString().padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function searchErrorKeyFor(error: unknown): SearchErrorKey {
  if (error instanceof HistorySearchInputError) {
    if (error.reason === 'tooShort') return 'searchMessagesTooShort';
    if (error.reason === 'tooLong') return 'searchMessagesTooLong';
  }
  return 'searchMessagesFailed';
}

function savedSearchErrorKey(error: unknown): SavedSearchErrorKey {
  if (error instanceof SavedHistorySearchError) {
    if (error.reason === 'limitReached') return 'savedHistorySearchLimitReached';
    if (error.reason === 'saveFailed') return 'savedHistorySearchSaveFailed';
  }
  return error instanceof SavedHistorySearchError
    ? 'savedHistorySearchesLoadFailed'
    : 'savedHistorySearchSaveFailed';
}
