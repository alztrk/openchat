import { BookmarkCheck, LoaderCircle } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@/design-system';
import { type SavedOutputSummary, listSavedOutputs } from '@/modules/conversations/data/chatStorage';
import { SavedResponseWorkspaceDialog } from '@/modules/conversations/components/SavedResponseWorkspaceDialog';

const pageSize = 30;
type LoadState = 'loading' | 'ready' | 'error';

export function SavedResponsesTab() {
  const { i18n, t } = useTranslation();
  const [outputs, setOutputs] = useState<SavedOutputSummary[]>([]);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [error, setError] = useState(false);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [target, setTarget] = useState<SavedOutputSummary | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoadState('loading');
    setError(false);
    void listSavedOutputs(0, pageSize)
      .then((page) => {
        if (cancelled) return;
        setOutputs(page);
        setHasMore(page.length === pageSize);
        setLoadState('ready');
      })
      .catch(() => {
        if (cancelled) return;
        setLoadState('error');
        setError(true);
      });
    return () => {
      cancelled = true;
    };
  }, [loadAttempt]);

  const loadMore = async () => {
    if (isLoadingMore || !hasMore || outputs.length === 0) return;
    setIsLoadingMore(true);
    setError(false);
    try {
      const page = await listSavedOutputs(outputs.length, pageSize);
      setOutputs((current) => [...current, ...page]);
      setHasMore(page.length === pageSize);
    } catch {
      setError(true);
    } finally {
      setIsLoadingMore(false);
    }
  };

  const canLoadMore = outputs.length > 0 && hasMore;
  const locale = i18n.resolvedLanguage ?? i18n.language;

  return (
    <div className="grid gap-4 p-4 sm:p-6">
      <div>
        <p className="text-sm text-secondary-text">{t('outputsDescription')}</p>
      </div>
      {loadState === 'loading' ? (
        <p aria-live="polite" className="flex items-center gap-2 text-sm text-secondary-text" role="status">
          <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          {t('outputsLoading')}
        </p>
      ) : loadState === 'error' ? (
        <div className="flex flex-wrap items-center justify-between gap-3" role="alert">
          <p className="text-sm text-destructive">{t('outputsLoadFailed')}</p>
          <Button onClick={() => setLoadAttempt((attempt) => attempt + 1)} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : outputs.length === 0 ? (
        <Card className="grid gap-2 p-5" role="status">
          <h2 className="font-semibold text-text">{t('outputsEmptyTitle')}</h2>
          <p className="text-sm text-secondary-text">{t('outputsEmptyDescription')}</p>
        </Card>
      ) : (
        <>
          <div className="grid gap-3">
            {outputs.map((output) => (
              <Card className="grid min-w-0 gap-3 p-4" key={`${output.conversationId}:${output.messageId}`}>
                <div className="flex min-w-0 items-start gap-3">
                  <BookmarkCheck aria-hidden="true" className="mt-0.5 shrink-0 text-accent-icon" size={17} />
                  <div className="min-w-0 flex-1">
                    <h2 className="truncate font-medium text-text">{output.conversationTitle}</h2>
                    <p className="mt-1 line-clamp-3 whitespace-pre-wrap break-words text-sm text-secondary-text">
                      {output.excerpt}
                    </p>
                    <p className="mt-2 text-xs text-secondary-text">
                      {t('outputSavedAt', { date: new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(output.createdAtUnixMs) })}
                      {' · '}
                      {t('outputWorkspaceVersion', { version: output.selectedVersion })}
                      {output.versionCount > 1 ? ` · ${t('outputWorkspaceVersionCount', { count: output.versionCount })}` : ''}
                    </p>
                  </div>
                </div>
                <Button className="justify-self-start" onClick={() => setTarget(output)} size="small" variant="secondary">
                  {t('outputWorkspaceOpen')}
                </Button>
              </Card>
            ))}
          </div>
          {error ? (
            <p className="text-sm text-destructive" role="alert">{t('outputsLoadFailed')}</p>
          ) : null}
          {canLoadMore ? (
            <Button className="justify-self-center" disabled={isLoadingMore} onClick={() => void loadMore()} size="small" variant="secondary">
              {isLoadingMore ? <LoaderCircle aria-hidden="true" className="animate-spin" size={15} /> : null}
              {isLoadingMore ? t('outputsLoading') : t('outputsLoadMore')}
            </Button>
          ) : null}
        </>
      )}
      <SavedResponseWorkspaceDialog
        onOpenChange={(open) => {
          if (!open) setTarget(null);
        }}
        open={target !== null}
        output={target}
      />
    </div>
  );
}
