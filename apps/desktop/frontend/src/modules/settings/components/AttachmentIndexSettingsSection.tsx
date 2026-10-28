import { LoaderCircle, RefreshCw, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@/design-system';
import {
  type AttachmentTextIndexStatus,
  clearAttachmentTextIndex,
  getAttachmentTextIndexStatus,
  rebuildAttachmentTextIndex,
} from '@/modules/conversations/data/chatStorage';

type LoadState = 'loading' | 'ready' | 'error';

export function AttachmentIndexSettingsSection() {
  const { i18n, t } = useTranslation();
  const [status, setStatus] = useState<AttachmentTextIndexStatus | null>(null);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [isWorking, setIsWorking] = useState(false);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  // biome-ignore lint/correctness/useExhaustiveDependencies: The attempt counter intentionally retriggers this load effect.
  useEffect(() => {
    let cancelled = false;
    setLoadState('loading');
    void getAttachmentTextIndexStatus()
      .then((nextStatus) => {
        if (cancelled) return;
        setStatus(nextStatus);
        setLoadState('ready');
      })
      .catch(() => {
        if (cancelled) return;
        setLoadState('error');
      });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  const rebuild = async () => {
    if (isWorking) return;
    setIsWorking(true);
    setFailed(false);
    try {
      setStatus(await rebuildAttachmentTextIndex());
    } catch {
      setFailed(true);
    } finally {
      setIsWorking(false);
    }
  };

  const clear = async () => {
    if (isWorking) return;
    setIsWorking(true);
    setFailed(false);
    try {
      setStatus(await clearAttachmentTextIndex());
    } catch {
      setFailed(true);
      try {
        setStatus(await getAttachmentTextIndexStatus());
      } catch {
        setLoadState('error');
      }
    } finally {
      setIsWorking(false);
    }
  };

  const locale = i18n.resolvedLanguage ?? i18n.language;

  return (
    <Card className="grid gap-4" padding="default">
      <div>
        <h3 className="font-medium text-text">{t('attachmentIndexTitle')}</h3>
        <p className="mt-1 text-sm text-secondary-text">{t('attachmentIndexDescription')}</p>
        <p className="mt-2 text-xs text-secondary-text">{t('attachmentIndexSupportedTypes')}</p>
      </div>
      {loadState === 'loading' ? (
        <p aria-live="polite" className="flex items-center gap-2 text-sm text-secondary-text" role="status">
          <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          {t('attachmentLibraryLoading')}
        </p>
      ) : loadState === 'error' ? (
        <div className="flex flex-wrap items-center gap-3" role="alert">
          <p className="text-sm text-destructive">{t('attachmentIndexFailed')}</p>
          <Button onClick={() => setAttempt((value) => value + 1)} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : (
        <>
          <p className="text-sm text-secondary-text">
            {t('attachmentIndexCount', { count: status?.indexedFileCount ?? 0 })}
            {status ? ` · ${new Intl.NumberFormat(locale).format(status.indexedBytes)} B` : ''}
          </p>
          {status?.truncated ? <p className="text-sm text-secondary-text">{t('attachmentIndexTruncated')}</p> : null}
          {failed ? (
            <p className="text-sm text-destructive" role="alert">
              {t('attachmentIndexFailed')}
            </p>
          ) : null}
          <div className="flex flex-wrap gap-2">
            <Button
              loading={isWorking}
              loadingContent={t('attachmentIndexWorking')}
              onClick={() => void rebuild()}
              size="small"
              variant="secondary"
            >
              <RefreshCw aria-hidden="true" size={16} />
              {t(status?.enabled ? 'attachmentIndexRebuild' : 'attachmentIndexBuild')}
            </Button>
            {status && (status.enabled || status.indexedFileCount > 0) ? (
              <Button disabled={isWorking} onClick={() => void clear()} size="small" variant="ghost">
                <Trash2 aria-hidden="true" size={16} />
                {t('attachmentIndexClear')}
              </Button>
            ) : null}
          </div>
        </>
      )}
    </Card>
  );
}
