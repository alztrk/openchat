import { LoaderCircle, RefreshCw, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@shared/components';
import { LocalServiceError } from '@shared/lib/platform/local-service';
import {
  type AttachmentTextIndexRebuildOperation,
  type AttachmentTextIndexStatus,
  clearAttachmentTextIndex,
  getActiveAttachmentTextIndexRebuild,
  getAttachmentTextIndexStatus,
  startAttachmentTextIndexRebuild,
} from '@features/chat/data';

type LoadState = 'loading' | 'ready' | 'error';
type MessageKey =
  | 'attachmentIndexFailed'
  | 'attachmentIndexCancelled'
  | 'attachmentIndexBusy'
  | 'attachmentIndexUncertain';

export function AttachmentIndexSettingsSection() {
  const { i18n, t } = useTranslation();
  const [status, setStatus] = useState<AttachmentTextIndexStatus | null>(null);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [operation, setOperation] = useState<AttachmentTextIndexRebuildOperation | null>(
    getActiveAttachmentTextIndexRebuild,
  );
  const [isClearing, setIsClearing] = useState(false);
  const [isCancelling, setIsCancelling] = useState(false);
  const [messageKey, setMessageKey] = useState<MessageKey | null>(null);
  const [attempt, setAttempt] = useState(0);
  const isWorking = operation !== null || isClearing || status?.operationInProgress === true;

  const reconcileStatus = async () => {
    try {
      const nextStatus = await getAttachmentTextIndexStatus();
      setStatus(nextStatus);
      setLoadState('ready');
      if (!nextStatus.operationInProgress) {
        setMessageKey((current) =>
          current === 'attachmentIndexBusy' || current === 'attachmentIndexUncertain' ? null : current,
        );
      }
    } catch {
      if (status === null) setLoadState('error');
    }
  };

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

  useEffect(() => {
    if (!isWorking) return;
    let stopped = false;
    let timer: number | undefined;
    const pollStatus = async () => {
      try {
        const nextStatus = await getAttachmentTextIndexStatus();
        if (!stopped) setStatus(nextStatus);
      } catch {
        // Keep the last confirmed operation state visible while the status request is unavailable.
      }
      if (!stopped) timer = window.setTimeout(() => void pollStatus(), 750);
    };
    timer = window.setTimeout(() => void pollStatus(), 250);
    return () => {
      stopped = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [isWorking]);

  const setOperationError = (error: unknown) => {
    if (error instanceof LocalServiceError) {
      if (error.code === 'operation_cancelled') {
        setMessageKey('attachmentIndexCancelled');
        return;
      }
      if (error.code === 'attachment_index_busy') {
        setMessageKey('attachmentIndexBusy');
        return;
      }
      if (error.code === 'service_timeout') {
        setMessageKey('attachmentIndexUncertain');
        return;
      }
    }
    setMessageKey('attachmentIndexFailed');
  };

  const rebuild = async () => {
    if (isWorking) return;
    setMessageKey(null);
    setIsCancelling(false);
    try {
      const nextOperation = await startAttachmentTextIndexRebuild();
      setOperation(nextOperation);
      setStatus((current) =>
        current
          ? { ...current, operationInProgress: true }
          : current,
      );
      const nextStatus = await nextOperation.result;
      setMessageKey(null);
      setStatus(nextStatus);
    } catch (error) {
      setOperationError(error);
    } finally {
      setOperation(getActiveAttachmentTextIndexRebuild());
      setIsCancelling(false);
      await reconcileStatus();
    }
  };

  const cancelRebuild = async () => {
    if (!operation || isCancelling) return;
    setIsCancelling(true);
    try {
      const accepted = await operation.cancel();
      if (!accepted) {
        setIsCancelling(false);
        setMessageKey('attachmentIndexUncertain');
        await reconcileStatus();
      }
    } catch {
      setIsCancelling(false);
      setMessageKey('attachmentIndexUncertain');
      await reconcileStatus();
    }
  };

  const clear = async () => {
    if (isWorking) return;
    setIsClearing(true);
    setMessageKey(null);
    try {
      setStatus(await clearAttachmentTextIndex());
    } catch (error) {
      setOperationError(error);
    } finally {
      setIsClearing(false);
      await reconcileStatus();
    }
  };

  const locale = i18n.resolvedLanguage ?? i18n.language;
  const formatNumber = (value: number) => new Intl.NumberFormat(locale).format(value);
  const progressMessage =
    status?.progress?.phase === 'scan'
      ? t('attachmentIndexProgressScan', {
          count: formatNumber(status.progress.completedMessages),
          total: formatNumber(status.progress.totalMessages),
        })
      : status?.progress?.phase === 'read'
        ? t('attachmentIndexProgressRead', {
            count: formatNumber(status.progress.completedFiles),
            total: formatNumber(status.progress.totalFiles),
            indexed: formatNumber(status.progress.indexedFiles),
          })
      : status?.progress?.phase === 'write'
        ? t('attachmentIndexProgressWrite', {
            count: formatNumber(status.progress.completedFiles),
            total: formatNumber(status.progress.totalFiles),
          })
        : t('attachmentIndexWorking');

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
            {status ? (
              <>
                {' · '}
                {new Intl.NumberFormat(locale).format(status.indexedBytes)}
                {' B'}
              </>
            ) : null}
          </p>
          {status?.truncated ? <p className="text-sm text-secondary-text">{t('attachmentIndexTruncated')}</p> : null}
          {isWorking ? (
            <p aria-live="polite" className="flex items-center gap-2 text-sm text-secondary-text" role="status">
              <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              {isCancelling ? t('attachmentIndexCancelling') : progressMessage}
            </p>
          ) : null}
          {messageKey ? (
            <p className="text-sm text-destructive" role="alert">
              {t(messageKey)}
            </p>
          ) : null}
          <div className="flex flex-wrap gap-2">
            <Button
              disabled={isWorking}
              loading={isWorking}
              loadingContent={progressMessage}
              onClick={() => void rebuild()}
              size="small"
              variant="secondary"
            >
              <RefreshCw aria-hidden="true" size={16} />
              {t(status?.enabled ? 'attachmentIndexRebuild' : 'attachmentIndexBuild')}
            </Button>
            {operation ? (
              <Button disabled={isCancelling} onClick={() => void cancelRebuild()} size="small" variant="ghost">
                {t(isCancelling ? 'attachmentIndexCancelling' : 'attachmentIndexCancel')}
              </Button>
            ) : null}
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
