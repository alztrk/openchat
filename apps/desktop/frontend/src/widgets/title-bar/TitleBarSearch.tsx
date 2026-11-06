import { Bot, Clock, File, FileSearch, LoaderCircle, MessageSquare, Pin, Search, Trash2, User, X } from 'lucide-react';
import {
  lazy,
  Suspense,
  type KeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@shared/components/Button';
import { Input } from '@shared/components/Input';
import { Tooltip } from '@shared/components/Tooltip';
import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';

import {
  type ChatConversation,
  listConversationAttachments,
  normalizeSearchText,
  parseSearchQuery,
  type StoredSearchMessageResult,
  searchStoredMessages,
} from '@features/chat/data';
import { useChatStore } from '@features/chat/state';
import {
  addRecentSearch,
  readRecentSearches,
  recentSearchLimits,
  removeRecentSearch,
  writeRecentSearches,
} from './recentSearches';

const LazyHistorySearchDialog = lazy(async () => {
  const { HistorySearchDialog } = await import('@features/chat/components/HistorySearchDialog');
  return { default: HistorySearchDialog };
});

export interface TitleBarSearchProps {
  onSelectConversation(conversationId: string, messageId?: string | null, searchQuery?: string | null): void;
  onSelectProject?: (projectId: string) => void;
}

interface ThreadMessageItem {
  messageId: string;
  attachmentId: string | null;
  role: 'user' | 'assistant';
  excerpt: string;
  createdAtUnixMs: number;
  isAttachment: boolean;
}

interface SearchConversationGroup {
  conversationId: string;
  conversationTitle: string;
  isPinned: boolean;
  isArchived: boolean;
  projectId: string | null;
  projectName: string | null;
  updatedAt: number;
  isTitleMatch: boolean;
  messages: ThreadMessageItem[];
}

type NavigableTarget =
  | { type: 'conversation'; conversationId: string }
  | { type: 'message'; conversationId: string; messageId: string };

type RecentSearchError = 'loadFailed' | 'saveFailed' | 'deleteFailed' | 'clearFailed';

function extractExcerpt(content: string, query: string, radius = 45): string {
  const normContent = normalizeSearchText(content);
  const normQuery = normalizeSearchText(query);
  const index = normContent.indexOf(normQuery);
  if (index === -1) {
    return content.length > 90 ? `${content.slice(0, 90)}…` : content;
  }
  const start = Math.max(0, index - radius);
  const end = Math.min(content.length, index + query.length + radius);
  const prefix = start > 0 ? '… ' : '';
  const suffix = end < content.length ? ' …' : '';
  return `${prefix}${content.slice(start, end).replace(/\s+/g, ' ')}${suffix}`;
}

function renderHighlightedExcerpt(text: string, query: string): ReactNode {
  if (!query) return text;
  const normText = normalizeSearchText(text);
  const normQuery = normalizeSearchText(query);
  const index = normText.indexOf(normQuery);
  if (index === -1) return text;

  const before = text.slice(0, index);
  const match = text.slice(index, index + query.length);
  const after = text.slice(index + query.length);

  return (
    <>
      {before}
      <mark className="rounded-xs bg-amber-400/40 px-0.5 font-medium text-inherit">{match}</mark>
      {after}
    </>
  );
}

export function TitleBarSearch({ onSelectConversation, onSelectProject }: TitleBarSearchProps) {
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState('');
  const [isOpen, setIsOpen] = useState(false);
  const [isHistorySearchDialogMounted, setIsHistorySearchDialogMounted] = useState(false);
  const [isHistorySearchDialogOpen, setIsHistorySearchDialogOpen] = useState(false);
  const [areHistorySearchFiltersActive, setAreHistorySearchFiltersActive] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isSearchingMessages, setIsSearchingMessages] = useState(false);
  const [messageResults, setMessageResults] = useState<StoredSearchMessageResult[]>([]);
  const [messageSearchLimited, setMessageSearchLimited] = useState(false);
  const [attachmentResults, setAttachmentResults] = useState<
    Awaited<ReturnType<typeof listConversationAttachments>>['items']
  >([]);
  const [recentSearches, setRecentSearches] = useState<string[]>([]);
  const [selectedRecentSearchIndex, setSelectedRecentSearchIndex] = useState(-1);
  const [recentSearchError, setRecentSearchError] = useState<RecentSearchError | null>(null);
  const [isSavingRecentSearches, setIsSavingRecentSearches] = useState(false);
  const recentSearchWriteRef = useRef(false);

  const searchId = useId();
  const recentSearchesListboxId = `${searchId}-recent-searches`;
  const searchResultsListboxId = `${searchId}-search-results`;
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const conversations = useChatStore((state) => state.conversations);
  const projects = useChatStore((state) => state.projects);

  const reloadRecentSearches = useCallback(() => {
    try {
      setRecentSearches(readRecentSearches());
      setSelectedRecentSearchIndex(-1);
      setRecentSearchError(null);
    } catch {
      setRecentSearchError('loadFailed');
    }
  }, []);

  useEffect(() => {
    if (unifiedStorage.isInitialized) {
      reloadRecentSearches();
      return;
    }
    window.addEventListener('openchat-storage-ready', reloadRecentSearches, { once: true });
    return () => window.removeEventListener('openchat-storage-ready', reloadRecentSearches);
  }, [reloadRecentSearches]);

  const persistRecentSearchList = async (next: string[], error: RecentSearchError = 'saveFailed') => {
    if (recentSearchWriteRef.current) return false;
    recentSearchWriteRef.current = true;
    setIsSavingRecentSearches(true);
    try {
      await writeRecentSearches(next);
      setRecentSearches(next);
      setSelectedRecentSearchIndex(-1);
      setRecentSearchError(null);
      return true;
    } catch {
      setRecentSearchError(error);
      return false;
    } finally {
      recentSearchWriteRef.current = false;
      setIsSavingRecentSearches(false);
    }
  };

  const saveRecentSearch = (term: string) => {
    if (recentSearchError === 'loadFailed') return Promise.resolve(false);
    return persistRecentSearchList(addRecentSearch(recentSearches, term));
  };

  useEffect(() => {
    const openSearch = () => {
      setIsOpen(true);
      window.requestAnimationFrame(() => inputRef.current?.focus());
    };
    window.addEventListener('openchat-open-title-search', openSearch);
    return () => window.removeEventListener('openchat-open-title-search', openSearch);
  }, []);

  const projectMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const project of projects) {
      map.set(project.id, project.name);
    }
    return map;
  }, [projects]);

  const conversationMap = useMemo(() => {
    const map = new Map<string, ChatConversation>();
    for (const conversation of conversations) {
      map.set(conversation.id, conversation);
    }
    return map;
  }, [conversations]);

  const parsedQuery = useMemo(() => parseSearchQuery(query), [query]);
  const normalizedCleanQuery = normalizeSearchText(parsedQuery.cleanQuery);
  const hasActiveFilter = Boolean(parsedQuery.roleFilter || parsedQuery.hasCodeOnly);
  const shouldSearch = normalizedCleanQuery.length >= 1 || hasActiveFilter;

  useEffect(() => {
    if (!shouldSearch) {
      setMessageResults([]);
      setMessageSearchLimited(false);
      setAttachmentResults([]);
      setIsSearchingMessages(false);
      return;
    }

    let isCancelled = false;
    setIsSearchingMessages(true);

    const timeoutId = window.setTimeout(() => {
      const messageSearch = searchStoredMessages(parsedQuery.cleanQuery, 50, {
        roleFilter: parsedQuery.roleFilter,
        hasCodeOnly: parsedQuery.hasCodeOnly,
      });
      const attachmentSearch =
        normalizedCleanQuery.length >= 2 && !parsedQuery.roleFilter && !parsedQuery.hasCodeOnly
          ? listConversationAttachments(0, 30, parsedQuery.cleanQuery, 'all')
          : Promise.resolve({ items: [], nextOffset: null });
      void Promise.all([messageSearch, attachmentSearch])
        .then(([messagePage, attachments]) => {
          if (!isCancelled) {
            setMessageResults(messagePage.messages);
            setMessageSearchLimited(messagePage.limitedByResponseSize);
            setAttachmentResults(attachments.items);
            setIsSearchingMessages(false);
          }
        })
        .catch(() => {
          if (!isCancelled) {
            setMessageResults([]);
            setMessageSearchLimited(false);
            setAttachmentResults([]);
            setIsSearchingMessages(false);
          }
        });
    }, 120);

    return () => {
      isCancelled = true;
      window.clearTimeout(timeoutId);
    };
  }, [shouldSearch, parsedQuery.cleanQuery, parsedQuery.roleFilter, parsedQuery.hasCodeOnly, normalizedCleanQuery]);

  const searchGroups = useMemo<SearchConversationGroup[]>(() => {
    if (!shouldSearch) return [];

    const messagesByConversation = new Map<string, ThreadMessageItem[]>();
    for (const msg of messageResults) {
      if (!msg.id) continue;
      const existing = messagesByConversation.get(msg.conversationId) ?? [];
      existing.push({
        messageId: msg.id,
        attachmentId: null,
        role: msg.role === 'assistant' ? 'assistant' : 'user',
        excerpt: extractExcerpt(msg.content, parsedQuery.cleanQuery),
        createdAtUnixMs: msg.createdAt ?? 0,
        isAttachment: false,
      });
      messagesByConversation.set(msg.conversationId, existing);
    }
    for (const attachment of attachmentResults) {
      const existing = messagesByConversation.get(attachment.conversationId) ?? [];
      existing.push({
        messageId: attachment.messageId,
        attachmentId: attachment.attachmentId,
        role: 'user',
        excerpt: attachment.fileName,
        createdAtUnixMs: attachment.createdAt,
        isAttachment: true,
      });
      messagesByConversation.set(attachment.conversationId, existing);
    }

    const titleMatchingConversations = hasActiveFilter
      ? []
      : conversations.filter((conversation) => {
          const normTitle = normalizeSearchText(conversation.title);
          const matchesTitle = normTitle.includes(normalizedCleanQuery);
          return matchesTitle;
        });

    const groups: SearchConversationGroup[] = [];
    const processedConversationIds = new Set<string>();

    for (const conversation of titleMatchingConversations) {
      processedConversationIds.add(conversation.id);
      const messages = messagesByConversation.get(conversation.id) ?? [];
      groups.push({
        conversationId: conversation.id,
        conversationTitle: conversation.title || t('newChat'),
        isPinned: conversation.isPinned,
        isArchived: conversation.isArchived,
        projectId: conversation.projectId,
        projectName: conversation.projectId ? (projectMap.get(conversation.projectId) ?? null) : null,
        updatedAt: conversation.updatedAt,
        isTitleMatch: true,
        messages,
      });
    }

    for (const [conversationId, messages] of messagesByConversation.entries()) {
      if (processedConversationIds.has(conversationId)) continue;
      processedConversationIds.add(conversationId);

      const conversation = conversationMap.get(conversationId);
      const title = conversation?.title || messages[0]?.excerpt.slice(0, 40) || t('newChat');

      groups.push({
        conversationId,
        conversationTitle: title,
        isPinned: conversation?.isPinned ?? false,
        isArchived: conversation?.isArchived ?? false,
        projectId: conversation?.projectId ?? null,
        projectName: conversation?.projectId ? (projectMap.get(conversation.projectId) ?? null) : null,
        updatedAt: conversation?.updatedAt ?? messages[0]?.createdAtUnixMs ?? 0,
        isTitleMatch: false,
        messages,
      });
    }

    groups.sort((a, b) => {
      if (a.isPinned !== b.isPinned) return a.isPinned ? -1 : 1;
      return b.updatedAt - a.updatedAt;
    });

    return groups;
  }, [
    conversations,
    conversationMap,
    hasActiveFilter,
    messageResults,
    attachmentResults,
    normalizedCleanQuery,
    parsedQuery.cleanQuery,
    projectMap,
    shouldSearch,
    t,
  ]);

  const navigableTargets = useMemo<NavigableTarget[]>(() => {
    const targets: NavigableTarget[] = [];
    for (const group of searchGroups) {
      targets.push({ type: 'conversation', conversationId: group.conversationId });
      for (const message of group.messages) {
        targets.push({
          type: 'message',
          conversationId: group.conversationId,
          messageId: message.messageId,
        });
      }
    }
    return targets;
  }, [searchGroups]);

  const showRecentSearches = isOpen && !query.trim() && (recentSearches.length > 0 || recentSearchError !== null);
  const showSearchResults = isOpen && shouldSearch;
  const isPopupVisible = showRecentSearches || showSearchResults;
  const activeSearchOptionId =
    showSearchResults && navigableTargets[selectedIndex]
      ? `${searchResultsListboxId}-option-${selectedIndex}`
      : undefined;
  const activeRecentSearchOptionId =
    showRecentSearches && recentSearches[selectedRecentSearchIndex]
      ? `${recentSearchesListboxId}-option-${selectedRecentSearchIndex}`
      : undefined;
  const recentSearchErrorMessage = recentSearchError ? t(recentSearchErrorTranslationKey(recentSearchError)) : null;

  // biome-ignore lint/correctness/useExhaustiveDependencies: Reset the selected result whenever the query changes.
  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && event.target instanceof Node && !containerRef.current.contains(event.target)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape' && isOpen) {
      event.preventDefault();
      setIsOpen(false);
      return;
    }

    if (!query.trim() && recentSearches.length > 0 && ['ArrowDown', 'ArrowUp', 'Enter'].includes(event.key)) {
      if (event.key === 'Enter' && isOpen) {
        event.preventDefault();
        const selected = recentSearches[selectedRecentSearchIndex];
        if (selected) {
          handleSelectRecentSearch(selected);
        }
        return;
      }

      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        if (!isOpen) {
          setSelectedRecentSearchIndex(event.key === 'ArrowDown' ? 0 : recentSearches.length - 1);
          setIsOpen(true);
        } else {
          const direction = event.key === 'ArrowDown' ? 1 : -1;
          setSelectedRecentSearchIndex((index) => {
            if (index < 0) return direction > 0 ? 0 : recentSearches.length - 1;
            return (index + direction + recentSearches.length) % recentSearches.length;
          });
        }
        return;
      }
    }

    if (!isOpen || navigableTargets.length === 0) {
      if (event.key === 'ArrowDown' && (shouldSearch || recentSearches.length > 0 || recentSearchError !== null)) {
        setIsOpen(true);
        event.preventDefault();
      }
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % navigableTargets.length);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelectedIndex((prev) => (prev - 1 + navigableTargets.length) % navigableTargets.length);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const selected = navigableTargets[selectedIndex];
      if (selected) {
        if (query.trim()) {
          void saveRecentSearch(query.trim());
        }
        if (selected.type === 'conversation') {
          onSelectConversation(selected.conversationId);
        } else {
          onSelectConversation(selected.conversationId, selected.messageId, parsedQuery.cleanQuery || query.trim());
        }
        setIsOpen(false);
        inputRef.current?.blur();
      }
    }
  };

  const handleSelectConversation = (conversationId: string) => {
    if (query.trim()) {
      void saveRecentSearch(query.trim());
    }
    onSelectConversation(conversationId);
    setIsOpen(false);
  };

  const handleSelectMessage = (conversationId: string, messageId: string) => {
    if (query.trim()) {
      void saveRecentSearch(query.trim());
    }
    onSelectConversation(conversationId, messageId, parsedQuery.cleanQuery || query.trim());
    setIsOpen(false);
  };

  const handleSelectRecentSearch = (term: string) => {
    setQuery(term);
    setIsOpen(true);
    inputRef.current?.focus();
  };

  const handleDeleteRecentSearch = (term: string, event: React.MouseEvent) => {
    event.stopPropagation();
    void persistRecentSearchList(removeRecentSearch(recentSearches, term), 'deleteFailed');
  };

  const handleClearAllRecent = (event: React.MouseEvent) => {
    event.stopPropagation();
    void persistRecentSearchList([], 'clearFailed');
  };

  const handleClear = () => {
    setQuery('');
    setMessageResults([]);
    setMessageSearchLimited(false);
    inputRef.current?.focus();
  };

  const formatDate = (timestamp: number) => {
    if (!timestamp) return '';
    const date = new Date(timestamp);
    const now = new Date();
    const isToday =
      date.getDate() === now.getDate() &&
      date.getMonth() === now.getMonth() &&
      date.getFullYear() === now.getFullYear();

    if (isToday) {
      return date.toLocaleTimeString(i18n.language, { hour: '2-digit', minute: '2-digit' });
    }
    return date.toLocaleDateString(i18n.language, { month: 'short', day: 'numeric' });
  };

  let currentIndex = 0;

  return (
    <div
      className="relative flex min-w-0 w-full max-w-xs items-center gap-1 sm:max-w-sm md:max-w-md lg:max-w-lg"
      data-titlebar-no-drag="true"
      ref={containerRef}
    >
      <div className="relative flex w-full items-center">
        <Search aria-hidden="true" className="pointer-events-none absolute left-2.5 text-secondary-icon" size={14} />
        <Input
          aria-activedescendant={activeSearchOptionId ?? activeRecentSearchOptionId}
          aria-autocomplete="list"
          aria-controls={
            showRecentSearches
              ? recentSearchesListboxId
              : showSearchResults
                ? searchResultsListboxId
                : undefined
          }
          aria-expanded={isPopupVisible}
          aria-label={t('searchChats')}
          className="w-full border-border/70 bg-surface/80 pl-8 pr-12 text-xs transition-colors duration-150 hover:border-border focus:bg-surface focus:ring-1 focus:ring-accent"
          density="compact"
          fieldClassName="w-full"
          fieldLayout="inline"
          maxLength={recentSearchLimits.maximumLength}
          onChange={(event) => {
            const nextValue = event.target.value;
            setQuery(nextValue);
            setIsOpen(true);
          }}
          onFocus={() => setIsOpen(true)}
          onKeyDown={handleKeyDown}
          placeholder={t('searchChats')}
          ref={inputRef}
          role="combobox"
          type="text"
          value={query}
        />

        <div className="absolute right-2 flex items-center gap-1">
          {isSearchingMessages ? (
            <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-icon" size={13} />
          ) : null}
          {query ? (
            <Button
              aria-label={t('settingsSearchClear')}
              className="size-6 text-secondary-icon transition-colors hover:text-text focus:outline-none"
              onClick={handleClear}
              size="icon"
              variant="ghost"
              type="button"
            >
              <X aria-hidden="true" size={13} />
            </Button>
          ) : null}
        </div>
      </div>

      <Tooltip label={t('historySearchOpenFilters')}>
        <Button
          aria-label={
            areHistorySearchFiltersActive
              ? `${t('historySearchOpenFilters')}: ${t('historySearchFiltersActive')}`
              : t('historySearchOpenFilters')
          }
          className={`size-8 shrink-0 ${areHistorySearchFiltersActive ? 'text-accent' : 'text-secondary-icon'}`}
          onClick={() => {
            setIsHistorySearchDialogMounted(true);
            setIsHistorySearchDialogOpen(true);
          }}
          size="icon"
          type="button"
          variant="ghost"
        >
          <FileSearch aria-hidden="true" size={16} />
        </Button>
      </Tooltip>
      <Suspense fallback={null}>
        {isHistorySearchDialogMounted ? (
          <LazyHistorySearchDialog
            onFiltersActiveChange={setAreHistorySearchFiltersActive}
            onOpenChange={setIsHistorySearchDialogOpen}
            onSelectConversation={onSelectConversation}
            onSelectProject={onSelectProject}
            open={isHistorySearchDialogOpen}
          />
        ) : null}
      </Suspense>

      {showRecentSearches ? (
        <div
          className="openchat-dropdown-surface absolute left-0 right-0 top-full z-50 mt-1 flex w-full origin-top flex-col overflow-hidden p-1"
        >
          <div className="flex items-center justify-between px-2 py-1.5 text-caption font-medium text-secondary-text">
            <span>{t('recentSearches')}</span>
            <Button
              className="min-h-0 gap-1 px-1 text-caption text-secondary-text transition-colors hover:text-destructive"
              disabled={isSavingRecentSearches}
              onClick={handleClearAllRecent}
              size="small"
              variant="ghost"
              type="button"
            >
              <Trash2 size={11} />
              <span>{t('clearRecentSearches')}</span>
            </Button>
          </div>
          <p className="px-2.5 pb-1 text-caption text-secondary-text">{t('recentSearchesPrivacyNotice')}</p>
          {recentSearchErrorMessage ? (
            <div className="flex items-center justify-between gap-2 px-2.5 py-1.5">
              <p className="text-caption text-destructive" role="alert">
                {recentSearchErrorMessage}
              </p>
              {recentSearchError === 'loadFailed' ? (
                <Button disabled={isSavingRecentSearches} onClick={reloadRecentSearches} size="small" variant="secondary">
                  {t('retry')}
                </Button>
              ) : null}
            </div>
          ) : null}
          <div className="flex min-w-0 gap-0.5">
            <div
              aria-label={t('recentSearches')}
              className="flex min-w-0 flex-1 flex-col gap-0.5"
              id={recentSearchesListboxId}
              role="listbox"
            >
              {recentSearches.map((term, index) => (
                <div
                  aria-selected={selectedRecentSearchIndex === index}
                  className="flex min-h-8 w-full cursor-pointer items-center gap-2 rounded-button px-2.5 py-1.5 text-xs text-text transition-colors hover:bg-hover aria-selected:bg-hover"
                  id={`${recentSearchesListboxId}-option-${index}`}
                  key={term}
                  onClick={() => handleSelectRecentSearch(term)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault();
                      handleSelectRecentSearch(term);
                    }
                  }}
                  role="option"
                  tabIndex={-1}
                >
                  <Clock aria-hidden="true" className="shrink-0 text-secondary-icon" size={12} />
                  <span className="truncate">{term}</span>
                </div>
              ))}
            </div>
            {recentSearches.length > 0 ? (
              <fieldset className="m-0 flex shrink-0 flex-col gap-0.5 border-0 p-0">
                <legend className="sr-only">{t('recentSearchesActions')}</legend>
                {recentSearches.map((term) => (
                  <Button
                    aria-label={t('deleteRecentSearch', { term })}
                    className="size-8 p-1 text-secondary-icon transition-colors hover:text-text"
                    disabled={isSavingRecentSearches}
                    key={term}
                    onClick={(event) => handleDeleteRecentSearch(term, event)}
                    size="icon"
                    variant="ghost"
                    type="button"
                  >
                    <X aria-hidden="true" size={12} />
                  </Button>
                ))}
              </fieldset>
            ) : null}
          </div>
        </div>
      ) : null}

      {recentSearchErrorMessage && !showRecentSearches && !showSearchResults ? (
        <p
          className="absolute left-0 right-0 top-full z-50 mt-1 rounded-card border border-destructive/30 bg-surface p-2 text-caption text-destructive"
          role="alert"
        >
          {recentSearchErrorMessage}
        </p>
      ) : null}

      {showSearchResults ? (
        <div className="absolute left-0 right-0 top-full z-50 mt-1 flex w-full flex-col gap-1">
          {messageSearchLimited ? (
            <p
              aria-live="polite"
              className="border border-border/70 bg-surface-raised px-2.5 py-1.5 text-caption text-secondary-text shadow-sm"
            >
              {t('searchMessagesResultsLimited')}
            </p>
          ) : null}
          <div
          aria-label={t('searchChats')}
          className="openchat-dropdown-surface flex w-full origin-top flex-col overflow-hidden transition-all duration-150 ease-out"
          id={searchResultsListboxId}
          role="listbox"
        >
          {recentSearchErrorMessage ? (
            <p className="px-2.5 py-1.5 text-caption text-destructive" role="alert">
              {recentSearchErrorMessage}
            </p>
          ) : null}
          {hasActiveFilter ? (
            <div className="flex items-center gap-1.5 border-b border-border/70 bg-surface-raised/40 px-2.5 py-1.5 text-caption text-secondary-text">
              <span className="font-medium">{t('searchFilterLabel')}</span>
              {parsedQuery.roleFilter ? (
                <span className="rounded-xs bg-accent/15 px-1.5 py-0.2 text-accent font-medium">
                  {t(parsedQuery.roleFilter === 'user' ? 'searchFilterUser' : 'searchFilterAssistant')}
                </span>
              ) : null}
              {parsedQuery.hasCodeOnly ? (
                <span className="rounded-xs bg-accent/15 px-1.5 py-0.2 text-accent font-medium">
                  {t('searchFilterCode')}
                </span>
              ) : null}
            </div>
          ) : null}

          <div
            className="openchat-scroll-area flex flex-col gap-1 overflow-y-auto p-1.5"
            style={{ height: 'min(340px, 48vh)' }}
          >
            {searchGroups.length === 0 && !isSearchingMessages ? (
              <div className="flex h-full flex-col items-center justify-center p-4 text-center text-xs text-secondary-text">
                {t('searchMessagesNoResults')}
              </div>
            ) : (
              searchGroups.map((group) => {
                const conversationTargetIndex = currentIndex++;
                const isConversationSelected = conversationTargetIndex === selectedIndex;
                const dateText = formatDate(group.updatedAt);

                return (
                  <div className="flex flex-col rounded-button transition-colors" key={group.conversationId}>
                    <Button
                      aria-selected={isConversationSelected}
                      className={`min-h-0 w-full justify-between gap-2.5 rounded-button px-2.5 py-1.5 text-left text-xs transition-colors duration-100 ${
                        isConversationSelected
                          ? 'bg-accent/15 font-medium text-text'
                          : 'text-text/90 hover:bg-hover active:bg-active'
                      }`}
                      id={`${searchResultsListboxId}-option-${conversationTargetIndex}`}
                      onClick={() => handleSelectConversation(group.conversationId)}
                      onMouseEnter={() => setSelectedIndex(conversationTargetIndex)}
                      role="option"
                      size="small"
                      type="button"
                      variant="ghost"
                    >
                      <div className="flex min-w-0 flex-1 items-center gap-2">
                        {group.isPinned ? (
                          <Pin aria-hidden="true" className="shrink-0 text-accent" size={13} />
                        ) : (
                          <MessageSquare aria-hidden="true" className="shrink-0 text-secondary-icon" size={13} />
                        )}
                        <span className="truncate">{group.conversationTitle}</span>
                      </div>

                      <div className="flex shrink-0 items-center gap-1.5 text-xs text-secondary-text">
                        {group.projectName ? (
                          <span className="rounded-xs bg-surface-raised px-1.5 py-0.5 text-secondary-text">
                            {group.projectName}
                          </span>
                        ) : null}
                        {dateText ? <span>{dateText}</span> : null}
                      </div>
                    </Button>

                    {group.messages.length > 0 ? (
                      <div className="relative my-0.5 ml-4.5 flex flex-col gap-1 border-l-2 border-border/70 pl-2.5 transition-all duration-150">
                        {group.messages.map((msg) => {
                          const messageTargetIndex = currentIndex++;
                          const isMessageSelected = messageTargetIndex === selectedIndex;

                          return (
                            <Button
                              aria-selected={isMessageSelected}
                              className={`group min-h-0 w-full items-start gap-2 rounded-button px-2 py-1.5 text-left text-xs transition-all duration-100 ease-out hover:translate-x-0.5 ${
                                isMessageSelected
                                  ? 'bg-accent/15 font-medium text-text'
                                  : 'text-secondary-text hover:bg-hover hover:text-text'
                              }`}
                              id={`${searchResultsListboxId}-option-${messageTargetIndex}`}
                              key={`${msg.messageId}:${msg.attachmentId ?? 'message'}`}
                              onClick={() => handleSelectMessage(group.conversationId, msg.messageId)}
                              onMouseEnter={() => setSelectedIndex(messageTargetIndex)}
                              role="option"
                              size="small"
                              type="button"
                              variant="ghost"
                            >
                              <span className="mt-0.5 shrink-0 text-secondary-icon transition-colors group-hover:text-accent">
                                {msg.isAttachment ? (
                                  <File aria-hidden="true" size={12} />
                                ) : msg.role === 'user' ? (
                                  <User aria-hidden="true" size={12} />
                                ) : (
                                  <Bot aria-hidden="true" size={12} />
                                )}
                              </span>
                              <div className="min-w-0 flex-1">
                                <p className="line-clamp-2 text-xs leading-relaxed text-text/85">
                                  {renderHighlightedExcerpt(msg.excerpt, parsedQuery.cleanQuery || query.trim())}
                                </p>
                              </div>
                            </Button>
                          );
                        })}
                      </div>
                    ) : null}
                  </div>
                );
              })
            )}
          </div>
        </div>
        </div>
      ) : null}
    </div>
  );
}

function recentSearchErrorTranslationKey(error: RecentSearchError):
  | 'recentSearchesLoadFailed'
  | 'recentSearchesSaveFailed'
  | 'recentSearchesDeleteFailed'
  | 'recentSearchesClearFailed' {
  if (error === 'loadFailed') return 'recentSearchesLoadFailed';
  if (error === 'deleteFailed') return 'recentSearchesDeleteFailed';
  if (error === 'clearFailed') return 'recentSearchesClearFailed';
  return 'recentSearchesSaveFailed';
}
