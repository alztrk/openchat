import { File, FileWarning, LoaderCircle, MessageSquare } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Input, Select } from '@/design-system';
import {
  listConversationAttachments,
  type ConversationAttachmentSummary,
} from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';

const pageSize = 50;
type AvailabilityFilter = 'all' | 'available' | 'missing';
type LoadState = 'loading' | 'ready' | 'error';

function parseAvailabilityFilter(value: string): AvailabilityFilter {
  if (value === 'available' || value === 'missing') return value;
  return 'all';
}

export function AttachmentLibraryTab() {
  const { i18n, t } = useTranslation();
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<AvailabilityFilter>('all');
  const [items, setItems] = useState<ConversationAttachmentSummary[]>([]);
  const [nextOffset, setNextOffset] = useState<number | null>(null);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const openConversation = useChatStore((state) => state.selectConversation);

  useEffect(() => {
    let cancelled = false;
    setLoadState('loading');
    setItems([]);
    setNextOffset(null);
    const timeoutId = window.setTimeout(() => {
      void listConversationAttachments(0, pageSize, query, filter)
        .then((page) => {
          if (cancelled) return;
          setItems(page.items);
          setNextOffset(page.nextOffset);
          setLoadState('ready');
        })
        .catch(() => {
          if (cancelled) return;
          setLoadState('error');
        });
    }, query.trim() ? 180 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timeoutId);
    };
  }, [filter, loadAttempt, query]);

  const loadMore = async () => {
    if (nextOffset === null || isLoadingMore) return;
    setIsLoadingMore(true);
    try {
      const page = await listConversationAttachments(nextOffset, pageSize, query, filter);
      setItems((current) => [...current, ...page.items]);
      setNextOffset(page.nextOffset);
    } catch {
      setLoadState('error');
    } finally {
      setIsLoadingMore(false);
    }
  };

  const locale = i18n.resolvedLanguage ?? i18n.language;
  const filterOptions = [
    { value: 'all', label: t('attachmentLibraryAllFilter') },
    { value: 'available', label: t('attachmentLibraryAvailableFilter') },
    { value: 'missing', label: t('attachmentLibraryMissingFilter') },
  ] as const;

  return (
    <section aria-labelledby="attachment-library-heading" className="grid gap-4 p-4 sm:p-6">
      <div>
        <h2 className="text-lg font-semibold text-text" id="attachment-library-heading">
          {t('attachmentLibraryTitle')}
        </h2>
        <p className="mt-1 text-sm text-secondary-text">{t('attachmentLibraryDescription')}</p>
      </div>
      <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_14rem] sm:items-end">
        <Input
          aria-label={t('attachmentLibrarySearchHint')}
          label={t('attachmentLibrarySearchHint')}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('attachmentLibrarySearchHint')}
          value={query}
        />
        <Select
          label={t('attachmentLibraryAvailability')}
          onChange={(event) => setFilter(parseAvailabilityFilter(event.target.value))}
          options={filterOptions}
          value={filter}
        />
      </div>

      {loadState === 'loading' ? (
        <p aria-live="polite" className="flex items-center gap-2 py-6 text-sm text-secondary-text" role="status">
          <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          {t('attachmentLibraryLoading')}
        </p>
      ) : loadState === 'error' ? (
        <div className="flex flex-wrap items-center justify-between gap-3" role="alert">
          <p className="text-sm text-destructive">{t('attachmentLibraryLoadFailed')}</p>
          <Button onClick={() => setLoadAttempt((attempt) => attempt + 1)} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : items.length === 0 ? (
        <Card className="p-5" role="status">
          <p className="text-sm text-secondary-text">{t('attachmentLibraryEmpty')}</p>
        </Card>
      ) : (
        <>
          <div className="grid gap-3">
            {items.map((item) => (
              <Card className="grid min-w-0 gap-3 p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center" key={`${item.conversationId}:${item.messageId}:${item.attachmentId}`}>
                <div className="flex min-w-0 items-start gap-3">
                  {item.isAvailable ? (
                    <File aria-hidden="true" className="mt-0.5 shrink-0 text-accent-icon" size={18} />
                  ) : (
                    <FileWarning aria-hidden="true" className="mt-0.5 shrink-0 text-destructive" size={18} />
                  )}
                  <div className="min-w-0">
                    <p className="break-all font-medium text-text">{item.fileName}</p>
                    <p className="mt-1 truncate text-sm text-secondary-text">{item.conversationTitle}</p>
                    <p className="mt-1 text-xs text-secondary-text">
                      {new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(item.sizeBytes / 1024)} KB
                      {' · '}
                      {new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(item.createdAt)}
                      {' · '}
                      {t(item.isAvailable ? 'attachmentLibraryAvailableFilter' : 'attachmentLibraryUnavailable')}
                    </p>
                  </div>
                </div>
                <Button
                  className="justify-self-start sm:justify-self-end"
                  onClick={() => void openConversation(item.conversationId, item.messageId)}
                  size="small"
                  variant="secondary"
                >
                  <MessageSquare aria-hidden="true" size={15} />
                  {t('attachmentLibraryOpenConversation')}
                </Button>
              </Card>
            ))}
          </div>
          {nextOffset !== null ? (
            <Button
              className="justify-self-center"
              disabled={isLoadingMore}
              onClick={() => void loadMore()}
              size="small"
              variant="secondary"
            >
              {isLoadingMore ? <LoaderCircle aria-hidden="true" className="animate-spin" size={15} /> : null}
              {isLoadingMore ? t('attachmentLibraryLoading') : t('outputsLoadMore')}
            </Button>
          ) : null}
        </>
      )}
    </section>
  );
}
