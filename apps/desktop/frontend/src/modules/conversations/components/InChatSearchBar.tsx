import { ChevronDown, ChevronUp, Search, X } from 'lucide-react';
import { type KeyboardEvent, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Tooltip } from '@/design-system';
import type { InChatFilterType, InChatMessageMatch } from '@/modules/conversations/data/inChatSearch';
import { cn } from '@/shared/utils/cn';

export interface InChatSearchBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  filter: InChatFilterType;
  onFilterChange: (filter: InChatFilterType) => void;
  matches: readonly InChatMessageMatch[];
  activeMatchIndex: number;
  onNext: () => void;
  onPrevious: () => void;
  onClose: () => void;
}

const FILTER_KEYS = [
  { key: 'all' as const, labelKey: 'inChatFilterAll' as const },
  { key: 'code' as const, labelKey: 'inChatFilterCode' as const },
  { key: 'error' as const, labelKey: 'inChatFilterErrors' as const },
  { key: 'user' as const, labelKey: 'inChatFilterUser' as const },
] as const;

export function InChatSearchBar({
  query,
  onQueryChange,
  filter,
  onFilterChange,
  matches,
  activeMatchIndex,
  onNext,
  onPrevious,
  onClose,
}: InChatSearchBarProps) {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    input.select();
  }, []);

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      if (event.shiftKey) {
        onPrevious();
      } else {
        onNext();
      }
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      onNext();
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      onPrevious();
      return;
    }
  };

  const hasFilterActive = filter !== 'all';
  const hasQuery = query.trim().length > 0;
  const showCount = hasQuery || hasFilterActive;

  let countText = '';
  if (showCount) {
    if (matches.length > 0) {
      countText = t('inChatSearchCount', {
        current: activeMatchIndex + 1,
        total: matches.length,
      });
    } else {
      countText = t('inChatSearchNoMatches');
    }
  }

  return (
    <div
      aria-label={t('inChatSearch')}
      className="openchat-in-chat-search absolute top-3 right-5 z-20 flex w-full max-w-[calc(100%-2.5rem)] flex-col gap-2 rounded-card border border-border bg-raised/95 p-2 shadow-lg backdrop-blur-md sm:w-auto sm:min-w-[420px]"
      role="search"
    >
      <style>{`
        ::highlight(openchat-search-match) {
          background-color: rgba(234, 179, 8, 0.4);
          color: inherit;
        }
        ::highlight(openchat-search-active) {
          background-color: var(--accent, #3b82f6);
          color: #ffffff;
        }
      `}</style>
      <div className="flex items-center gap-1.5">
        <Search aria-hidden="true" className="ml-1 shrink-0 text-secondary-icon" size={16} />
        <input
          aria-label={t('inChatSearchPlaceholder')}
          className="h-8 min-w-0 flex-1 bg-transparent px-2 text-sm text-text placeholder:text-secondary-text outline-none"
          onChange={(e) => onQueryChange(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('inChatSearchPlaceholder')}
          ref={inputRef}
          type="search"
          value={query}
        />

        {countText ? (
          <span
            aria-live="polite"
            className={cn(
              'shrink-0 px-1 text-xs font-medium tabular-nums',
              matches.length > 0 ? 'text-secondary-text' : 'text-destructive',
            )}
          >
            {countText}
          </span>
        ) : null}

        <div className="flex shrink-0 items-center gap-0.5">
          <Tooltip label={t('inChatPreviousMatch')}>
            <Button
              aria-label={t('inChatPreviousMatch')}
              className="size-7 text-secondary-icon hover:text-text"
              disabled={matches.length === 0}
              onClick={onPrevious}
              size="icon"
              variant="ghost"
            >
              <ChevronUp aria-hidden="true" size={15} />
            </Button>
          </Tooltip>
          <Tooltip label={t('inChatNextMatch')}>
            <Button
              aria-label={t('inChatNextMatch')}
              className="size-7 text-secondary-icon hover:text-text"
              disabled={matches.length === 0}
              onClick={onNext}
              size="icon"
              variant="ghost"
            >
              <ChevronDown aria-hidden="true" size={15} />
            </Button>
          </Tooltip>
          <Tooltip label={t('inChatCloseSearch')}>
            <Button
              aria-label={t('inChatCloseSearch')}
              className="size-7 text-secondary-icon hover:text-text"
              onClick={onClose}
              size="icon"
              variant="ghost"
            >
              <X aria-hidden="true" size={15} />
            </Button>
          </Tooltip>
        </div>
      </div>

      <div
        aria-label={t('inChatSearch')}
        className="flex flex-wrap items-center gap-1 border-t border-border/50 pt-1.5 text-xs"
        role="toolbar"
      >
        {FILTER_KEYS.map(({ key, labelKey }) => {
          const isSelected = filter === key;
          return (
            <button
              aria-pressed={isSelected}
              className={cn(
                'rounded-full px-2.5 py-0.5 font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus',
                isSelected
                  ? 'bg-accent text-accent-foreground shadow-xs'
                  : 'text-secondary-text hover:bg-hover hover:text-text',
              )}
              key={key}
              onClick={() => {
                onFilterChange(key);
                inputRef.current?.focus();
              }}
              type="button"
            >
              {t(labelKey)}
            </button>
          );
        })}
      </div>
    </div>
  );
}
