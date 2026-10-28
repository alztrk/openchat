import { File, LoaderCircle } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Input } from '@/design-system';
import {
  type ConversationAttachmentSummary,
  listConversationAttachments,
} from '@/modules/conversations/data/chatStorage';

const pageSize = 30;

interface AttachmentLibraryDialogProps {
  open: boolean;
  disabled: boolean;
  onOpenChange(open: boolean): void;
  onSelect(item: ConversationAttachmentSummary): Promise<boolean>;
}

export function AttachmentLibraryDialog({ open, disabled, onOpenChange, onSelect }: AttachmentLibraryDialogProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [items, setItems] = useState<ConversationAttachmentSummary[]>([]);
  const [nextOffset, setNextOffset] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(false);
  const [selectionFailed, setSelectionFailed] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: loadAttempt deliberately retriggers the request when the user retries.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setError(false);
    setSelectionFailed(false);
    setItems([]);
    setNextOffset(null);
    const timeoutId = window.setTimeout(
      () => {
        void listConversationAttachments(0, pageSize, query, 'available')
          .then((page) => {
            if (cancelled) return;
            setItems(page.items);
            setNextOffset(page.nextOffset);
          })
          .catch(() => {
            if (!cancelled) setError(true);
          })
          .finally(() => {
            if (!cancelled) setLoading(false);
          });
      },
      query.trim() ? 180 : 0,
    );
    return () => {
      cancelled = true;
      window.clearTimeout(timeoutId);
    };
  }, [loadAttempt, open, query]);

  const loadMore = async () => {
    if (nextOffset === null || loading) return;
    setLoading(true);
    setError(false);
    try {
      const page = await listConversationAttachments(nextOffset, pageSize, query, 'available');
      setItems((current) => [...current, ...page.items]);
      setNextOffset(page.nextOffset);
    } catch {
      setError(true);
    } finally {
      setLoading(false);
    }
  };

  const select = async (item: ConversationAttachmentSummary) => {
    if (disabled || selectedId !== null) return;
    setSelectedId(item.attachmentId);
    setSelectionFailed(false);
    try {
      if (await onSelect(item)) onOpenChange(false);
      else setSelectionFailed(true);
    } catch {
      setSelectionFailed(true);
    } finally {
      setSelectedId(null);
    }
  };

  return (
    <Dialog
      className="w-full max-w-2xl"
      onOpenChange={(nextOpen) => {
        if (!disabled) onOpenChange(nextOpen);
      }}
      open={open}
      title={t('attachmentLibraryTitle')}
    >
      <div className="grid gap-4">
        <p className="text-sm text-secondary-text">{t('attachmentLibraryPickerDescription')}</p>
        <Input
          label={t('attachmentLibrarySearchHint')}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('attachmentLibrarySearchHint')}
          value={query}
        />
        {error ? (
          <div className="flex flex-wrap items-center gap-3" role="alert">
            <p className="text-sm text-destructive">{t('attachmentLibraryLoadFailed')}</p>
            <Button onClick={() => setLoadAttempt((attempt) => attempt + 1)} size="small" variant="secondary">
              {t('retry')}
            </Button>
          </div>
        ) : null}
        {selectionFailed ? (
          <p className="text-sm text-destructive" role="alert">
            {t('attachmentLibraryAddFailed')}
          </p>
        ) : null}
        {loading && items.length === 0 && !error ? (
          <p aria-live="polite" className="flex items-center gap-2 text-sm text-secondary-text" role="status">
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            {t('attachmentLibraryLoading')}
          </p>
        ) : null}
        {items.length === 0 && !loading && !error ? (
          <Card padding="default" role="status">
            <p className="text-sm text-secondary-text">{t('attachmentLibraryEmpty')}</p>
          </Card>
        ) : null}
        {items.length > 0 ? (
          <div className="grid gap-2">
            {items.map((item) => (
              <Card
                className="flex min-w-0 flex-wrap items-center justify-between gap-3"
                key={`${item.conversationId}:${item.messageId}:${item.attachmentId}`}
                padding="compact"
              >
                <div className="flex min-w-0 items-start gap-2">
                  <File aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={16} />
                  <div className="min-w-0">
                    <p className="break-all text-sm font-medium text-text">{item.fileName}</p>
                    <p className="mt-0.5 truncate text-xs text-secondary-text">{item.conversationTitle}</p>
                  </div>
                </div>
                <Button
                  disabled={disabled || selectedId !== null}
                  onClick={() => void select(item)}
                  size="small"
                  variant="secondary"
                >
                  {selectedId === item.attachmentId ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                  ) : null}
                  {t('attachmentLibraryUseFile')}
                </Button>
              </Card>
            ))}
          </div>
        ) : null}
        {nextOffset !== null ? (
          <Button
            className="justify-self-center"
            disabled={loading || disabled}
            onClick={() => void loadMore()}
            size="small"
            variant="secondary"
          >
            {loading ? <LoaderCircle aria-hidden="true" className="animate-spin" size={15} /> : null}
            {loading ? t('attachmentLibraryLoading') : t('loadMore')}
          </Button>
        ) : null}
      </div>
    </Dialog>
  );
}
