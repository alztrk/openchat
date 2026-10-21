import { Bot, Clock, LoaderCircle, MessageSquare, Pin, Search, Trash2, User, X } from 'lucide-react';
import { type KeyboardEvent, type ReactNode, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  type ChatConversation,
  normalizeSearchText,
  parseSearchQuery,
  type StoredSearchMessageResult,
  searchStoredMessages,
} from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';

export interface TitleBarSearchProps {
  onSelectConversation(conversationId: string, messageId?: string | null, searchQuery?: string | null): void;
}

interface ThreadMessageItem {
  messageId: string;
  role: 'user' | 'assistant';
  excerpt: string;
  createdAtUnixMs: number;
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

const RECENT_SEARCHES_STORAGE_KEY = 'openchat:recent_searches';

function loadRecentSearches(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_SEARCHES_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : [];
  } catch {
    return [];
  }
}

function persistRecentSearch(term: string): string[] {
  const trimmed = term.trim();
  if (!trimmed) return loadRecentSearches();
  try {
    const existing = loadRecentSearches().filter((item) => item.toLowerCase() !== trimmed.toLowerCase());
    const updated = [trimmed, ...existing].slice(0, 5);
    localStorage.setItem(RECENT_SEARCHES_STORAGE_KEY, JSON.stringify(updated));
    return updated;
  } catch {
    return [];
  }
}

function deleteRecentSearch(term: string): string[] {
  try {
    const existing = loadRecentSearches().filter((item) => item !== term);
    localStorage.setItem(RECENT_SEARCHES_STORAGE_KEY, JSON.stringify(existing));
    return existing;
  } catch {
    return [];
  }
}

function clearAllRecentSearches(): void {
  try {
    localStorage.removeItem(RECENT_SEARCHES_STORAGE_KEY);
  } catch {
    // Storage may be unavailable in restricted environments.
  }
}

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

export function TitleBarSearch({ onSelectConversation }: TitleBarSearchProps) {
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState('');
  const [isOpen, setIsOpen] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isSearchingMessages, setIsSearchingMessages] = useState(false);
  const [messageResults, setMessageResults] = useState<StoredSearchMessageResult[]>([]);
  const [recentSearches, setRecentSearches] = useState<string[]>([]);

  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const conversations = useChatStore((state) => state.conversations);
  const projects = useChatStore((state) => state.projects);

  useEffect(() => {
    setRecentSearches(loadRecentSearches());
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
      setIsSearchingMessages(false);
      return;
    }

    let isCancelled = false;
    setIsSearchingMessages(true);

    const timeoutId = window.setTimeout(() => {
      searchStoredMessages(parsedQuery.cleanQuery, 50, {
        roleFilter: parsedQuery.roleFilter,
        hasCodeOnly: parsedQuery.hasCodeOnly,
      })
        .then((results) => {
          if (!isCancelled) {
            setMessageResults(results);
            setIsSearchingMessages(false);
          }
        })
        .catch(() => {
          if (!isCancelled) {
            setMessageResults([]);
            setIsSearchingMessages(false);
          }
        });
    }, 120);

    return () => {
      isCancelled = true;
      window.clearTimeout(timeoutId);
    };
  }, [shouldSearch, parsedQuery.cleanQuery, parsedQuery.roleFilter, parsedQuery.hasCodeOnly]);

  const searchGroups = useMemo<SearchConversationGroup[]>(() => {
    if (!shouldSearch) return [];

    const messagesByConversation = new Map<string, ThreadMessageItem[]>();
    for (const msg of messageResults) {
      if (!msg.id) continue;
      const existing = messagesByConversation.get(msg.conversationId) ?? [];
      existing.push({
        messageId: msg.id,
        role: msg.role === 'assistant' ? 'assistant' : 'user',
        excerpt: extractExcerpt(msg.content, parsedQuery.cleanQuery),
        createdAtUnixMs: msg.createdAt ?? 0,
      });
      messagesByConversation.set(msg.conversationId, existing);
    }

    const titleMatchingConversations = hasActiveFilter
      ? []
      : conversations.filter((conversation) => {
          const normTitle = normalizeSearchText(conversation.title);
          const matchesTitle = normTitle.includes(normalizedCleanQuery);
          const matchesTags = conversation.tags.some((tag) => normalizeSearchText(tag).includes(normalizedCleanQuery));
          return matchesTitle || matchesTags;
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
    if (!isOpen || navigableTargets.length === 0) {
      if (event.key === 'ArrowDown' && (shouldSearch || recentSearches.length > 0)) {
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
          setRecentSearches(persistRecentSearch(query.trim()));
        }
        if (selected.type === 'conversation') {
          onSelectConversation(selected.conversationId);
        } else {
          onSelectConversation(selected.conversationId, selected.messageId, parsedQuery.cleanQuery || query.trim());
        }
        setIsOpen(false);
        inputRef.current?.blur();
      }
    } else if (event.key === 'Escape') {
      event.preventDefault();
      setIsOpen(false);
    }
  };

  const handleSelectConversation = (conversationId: string) => {
    if (query.trim()) {
      setRecentSearches(persistRecentSearch(query.trim()));
    }
    onSelectConversation(conversationId);
    setIsOpen(false);
  };

  const handleSelectMessage = (conversationId: string, messageId: string) => {
    if (query.trim()) {
      setRecentSearches(persistRecentSearch(query.trim()));
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
    setRecentSearches(deleteRecentSearch(term));
  };

  const handleClearAllRecent = (event: React.MouseEvent) => {
    event.stopPropagation();
    clearAllRecentSearches();
    setRecentSearches([]);
  };

  const handleClear = () => {
    setQuery('');
    setMessageResults([]);
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
  const showRecentSearches = isOpen && !query.trim() && recentSearches.length > 0;

  return (
    <div
      className="relative flex min-w-0 w-full max-w-xs sm:max-w-sm md:max-w-md lg:max-w-lg items-center"
      data-titlebar-no-drag="true"
      ref={containerRef}
    >
      <div className="relative flex w-full items-center">
        <Search aria-hidden="true" className="pointer-events-none absolute left-2.5 text-secondary-icon" size={14} />
        <input
          aria-autocomplete="list"
          aria-expanded={isOpen}
          aria-label={t('searchChats')}
          className="h-7.5 w-full rounded-control border border-border/70 bg-surface/80 pl-8 pr-12 text-xs text-text placeholder:text-secondary-text transition-colors duration-150 hover:border-border focus:border-accent focus:bg-surface focus:outline-none focus:ring-1 focus:ring-accent"
          onChange={(event) => {
            const nextValue = event.target.value;
            setQuery(nextValue);
            setIsOpen(true);
          }}
          onFocus={() => {
            setRecentSearches(loadRecentSearches());
            setIsOpen(true);
          }}
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
            <button
              aria-label={t('settingsSearchClear')}
              className="text-secondary-icon transition-colors hover:text-text focus:outline-none"
              onClick={handleClear}
              type="button"
            >
              <X aria-hidden="true" size={13} />
            </button>
          ) : null}
        </div>
      </div>

      {showRecentSearches ? (
        <div
          aria-label="Recent searches"
          className="openchat-glass-popover absolute left-0 right-0 top-full z-50 mt-1 flex w-full origin-top flex-col overflow-hidden rounded-dialog border border-border p-1"
          role="listbox"
        >
          <div className="flex items-center justify-between px-2 py-1.5 text-[11px] font-medium text-secondary-text">
            <span>Son Aramalar</span>
            <button
              className="flex items-center gap-1 text-[11px] text-secondary-text transition-colors hover:text-destructive"
              onClick={handleClearAllRecent}
              type="button"
            >
              <Trash2 size={11} />
              <span>Temizle</span>
            </button>
          </div>
          <div className="flex flex-col gap-0.5">
            {recentSearches.map((term) => (
              <div
                className="group flex w-full items-center justify-between rounded-button px-2.5 py-1.5 text-xs text-text transition-colors hover:bg-hover cursor-pointer"
                key={term}
                onClick={() => handleSelectRecentSearch(term)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleSelectRecentSearch(term);
                }}
                role="option"
                tabIndex={0}
              >
                <div className="flex items-center gap-2 truncate">
                  <Clock aria-hidden="true" className="shrink-0 text-secondary-icon" size={12} />
                  <span className="truncate">{term}</span>
                </div>
                <button
                  aria-label="Aramayı sil"
                  className="opacity-0 group-hover:opacity-100 p-0.5 text-secondary-icon transition-opacity hover:text-text"
                  onClick={(e) => handleDeleteRecentSearch(term, e)}
                  type="button"
                >
                  <X size={12} />
                </button>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      {isOpen && shouldSearch ? (
        <div
          aria-label={t('searchChats')}
          className="openchat-glass-popover absolute left-0 right-0 top-full z-50 mt-1 flex w-full origin-top flex-col overflow-hidden rounded-dialog border border-border transition-all duration-150 ease-out"
          role="listbox"
        >
          {hasActiveFilter ? (
            <div className="flex items-center gap-1.5 border-b border-border/70 bg-surface-raised/40 px-2.5 py-1.5 text-[11px] text-secondary-text">
              <span className="font-medium">Filtre:</span>
              {parsedQuery.roleFilter ? (
                <span className="rounded-xs bg-accent/15 px-1.5 py-0.2 text-accent font-medium">
                  {parsedQuery.roleFilter === 'user' ? 'Kullanıcı' : 'Yapay Zeka'}
                </span>
              ) : null}
              {parsedQuery.hasCodeOnly ? (
                <span className="rounded-xs bg-accent/15 px-1.5 py-0.2 text-accent font-medium">Kod bloğu</span>
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
                    <button
                      aria-selected={isConversationSelected}
                      className={`flex w-full items-center justify-between gap-2.5 rounded-button px-2.5 py-1.5 text-left text-xs transition-colors duration-100 ${
                        isConversationSelected
                          ? 'bg-accent/15 font-medium text-text'
                          : 'text-text/90 hover:bg-hover active:bg-active'
                      }`}
                      onClick={() => handleSelectConversation(group.conversationId)}
                      onMouseEnter={() => setSelectedIndex(conversationTargetIndex)}
                      role="option"
                      type="button"
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
                    </button>

                    {group.messages.length > 0 ? (
                      <div className="relative my-0.5 ml-4.5 flex flex-col gap-1 border-l-2 border-border/70 pl-2.5 transition-all duration-150">
                        {group.messages.map((msg) => {
                          const messageTargetIndex = currentIndex++;
                          const isMessageSelected = messageTargetIndex === selectedIndex;

                          return (
                            <button
                              aria-selected={isMessageSelected}
                              className={`group flex w-full items-start gap-2 rounded-button px-2 py-1.5 text-left text-xs transition-all duration-100 ease-out hover:translate-x-0.5 ${
                                isMessageSelected
                                  ? 'bg-accent/15 font-medium text-text'
                                  : 'text-secondary-text hover:bg-hover hover:text-text'
                              }`}
                              key={msg.messageId}
                              onClick={() => handleSelectMessage(group.conversationId, msg.messageId)}
                              onMouseEnter={() => setSelectedIndex(messageTargetIndex)}
                              role="option"
                              type="button"
                            >
                              <span className="mt-0.5 shrink-0 text-secondary-icon transition-colors group-hover:text-accent">
                                {msg.role === 'user' ? (
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
                            </button>
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
      ) : null}
    </div>
  );
}
