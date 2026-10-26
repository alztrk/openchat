import { Bot, Check, CircleAlert, CircleX, LoaderCircle, RefreshCw, Square, TriangleAlert } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import {
  applySubagentProgressUpdate,
  cancelSubagentRun,
  listSubagentRuns,
  parseSubagentProgressEvent,
  type SubagentRun,
  type SubagentStatus,
} from '@/modules/conversations/data/subagents';
import { localServiceClient } from '@/platform/local-service';

interface SubagentsPanelProps {
  conversationId: string | null;
}

type LoadState = 'loading' | 'ready' | 'error';

export function SubagentsPanel({ conversationId }: SubagentsPanelProps) {
  const { t } = useTranslation();
  const [runs, setRuns] = useState<SubagentRun[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [isCancelling, setIsCancelling] = useState<string | null>(null);
  const [cancellationRequestedRunIds, setCancellationRequestedRunIds] = useState<Set<string>>(() => new Set());
  const [cancelError, setCancelError] = useState(false);
  const loadGeneration = useRef(0);
  const progressRevision = useRef(0);

  const refreshRuns = useCallback(async () => {
    const generation = ++loadGeneration.current;
    const revisionAtStart = progressRevision.current;
    if (!conversationId) {
      setRuns([]);
      setSelectedRunId(null);
      setLoadState('ready');
      return;
    }
    try {
      const updatedRuns = await listSubagentRuns(conversationId);
      if (generation !== loadGeneration.current || revisionAtStart !== progressRevision.current) return;
      setRuns(updatedRuns);
      setSelectedRunId((current) =>
        current && updatedRuns.some((run) => run.runId === current) ? current : (updatedRuns[0]?.runId ?? null),
      );
      setLoadState('ready');
    } catch {
      if (generation !== loadGeneration.current) return;
      setLoadState('error');
    }
  }, [conversationId]);

  useEffect(() => {
    setLoadState('loading');
    setRuns([]);
    setSelectedRunId(null);
    setCancellationRequestedRunIds(new Set());
    progressRevision.current = 0;
    void refreshRuns();
    const unsubscribe = localServiceClient.subscribe((event) => {
      const update = parseSubagentProgressEvent(event);
      if (!update || update.conversationId !== conversationId) return;
      progressRevision.current += 1;
      setRuns((current) => applySubagentProgressUpdate(current, update));
      setSelectedRunId((current) => current ?? update.runId);
      if (update.status !== 'running') {
        setCancellationRequestedRunIds((current) => {
          if (!current.has(update.runId)) return current;
          const next = new Set(current);
          next.delete(update.runId);
          return next;
        });
        void refreshRuns();
      } else if (update.phase === 'queued') void refreshRuns();
    });
    return () => {
      loadGeneration.current += 1;
      unsubscribe();
    };
  }, [conversationId, refreshRuns]);

  const selectedRun = useMemo(() => runs.find((run) => run.runId === selectedRunId) ?? null, [runs, selectedRunId]);

  const cancelRun = async (run: SubagentRun) => {
    if (run.status !== 'running' || isCancelling !== null) return;
    setIsCancelling(run.runId);
    setCancelError(false);
    try {
      const accepted = await cancelSubagentRun(run.conversationId, run.runId);
      if (accepted) {
        setCancellationRequestedRunIds((current) => new Set(current).add(run.runId));
      } else await refreshRuns();
    } catch {
      setCancelError(true);
    } finally {
      setIsCancelling(null);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex min-h-0 shrink-0 flex-col border-b border-border">
        <div className="flex items-center justify-between px-3 py-2">
          <h2 className="text-xs font-semibold text-text">{t('subagentsPanelTitle')}</h2>
          <Button
            aria-label={t('subagentsRefresh')}
            className="size-8"
            disabled={loadState === 'loading'}
            onClick={() => void refreshRuns()}
            size="icon"
            variant="ghost"
          >
            <RefreshCw aria-hidden="true" size={15} />
          </Button>
        </div>
        {loadState === 'loading' && runs.length === 0 ? (
          <div className="flex h-16 items-center justify-center" role="status">
            <LoaderCircle
              aria-label={t('subagentsLoading')}
              className="animate-spin text-accent motion-reduce:animate-none"
              size={18}
            />
          </div>
        ) : loadState === 'error' && runs.length === 0 ? (
          <div className="flex items-center justify-between gap-2 px-3 pb-3 text-xs text-destructive" role="alert">
            <span>{t('subagentsLoadFailed')}</span>
            <Button className="h-7 px-2 text-xs" onClick={() => void refreshRuns()} size="small" variant="secondary">
              {t('retry')}
            </Button>
          </div>
        ) : runs.length === 0 ? (
          <p className="px-3 pb-3 text-xs leading-5 text-secondary-text">{t('subagentsEmpty')}</p>
        ) : (
          <>
            {loadState === 'error' ? (
              <p className="px-3 pb-2 text-xs text-destructive" role="alert">
                {t('subagentsLoadFailed')}
              </p>
            ) : null}
            <ul
              aria-label={t('subagentsPanelTitle')}
              className="openchat-scroll-area max-h-64 overflow-y-auto px-2 pb-2"
            >
              {runs.map((run) => (
                <li key={run.runId}>
                  <Button
                    aria-pressed={run.runId === selectedRunId}
                    className={`min-h-0 w-full items-start justify-start gap-2 rounded-control px-2.5 py-2 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                      run.runId === selectedRunId
                        ? 'bg-hover text-text'
                        : 'text-secondary-text hover:bg-hover hover:text-text'
                    }`}
                    onClick={() => setSelectedRunId(run.runId)}
                    size="small"
                    type="button"
                    variant="ghost"
                  >
                    <SubagentStatusIcon status={run.status} />
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center justify-between gap-2">
                        <span className="truncate text-xs font-semibold">{run.agentName ?? t('subagentsUnnamed')}</span>
                        <span className="shrink-0 text-caption text-secondary-text">
                          {cancellationRequestedRunIds.has(run.runId)
                            ? t('subagentsCancelling')
                            : t(statusTranslationKey(run.status))}
                        </span>
                      </span>
                      <span className="mt-0.5 block truncate text-caption text-secondary-text">
                        {run.progress?.toolName ?? run.progress?.phase ?? run.objective ?? ''}
                      </span>
                    </span>
                  </Button>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>

      {selectedRun ? (
        <section
          aria-label={selectedRun.agentName ?? t('subagentsReadOnlyTitle')}
          className="flex min-h-0 flex-1 flex-col"
        >
          <header className="flex shrink-0 items-start justify-between gap-2 border-b border-border px-3 py-2.5">
            <div className="min-w-0">
              <h3 className="truncate text-sm font-semibold text-text">
                {selectedRun.agentName ?? t('subagentsUnnamed')}
              </h3>
              <p className="mt-0.5 text-caption text-secondary-text" role="status">
                {cancellationRequestedRunIds.has(selectedRun.runId)
                  ? t('subagentsCancelling')
                  : t(statusTranslationKey(selectedRun.status))}
              </p>
            </div>
            {selectedRun.status === 'running' ? (
              <Button
                className="h-8 shrink-0 gap-1.5 px-2 text-xs"
                disabled={isCancelling !== null || cancellationRequestedRunIds.has(selectedRun.runId)}
                onClick={() => void cancelRun(selectedRun)}
                size="small"
                variant="secondary"
              >
                {isCancelling === selectedRun.runId || cancellationRequestedRunIds.has(selectedRun.runId) ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={13} />
                ) : (
                  <Square aria-hidden="true" size={12} />
                )}
                <span>
                  {isCancelling === selectedRun.runId || cancellationRequestedRunIds.has(selectedRun.runId)
                    ? t('subagentsCancelling')
                    : t('subagentsCancel')}
                </span>
              </Button>
            ) : null}
          </header>

          {cancelError ? (
            <p
              className="mx-3 mt-2 rounded-control bg-destructive/10 px-2.5 py-2 text-xs text-destructive"
              role="alert"
            >
              {t('subagentsCancelFailed')}
            </p>
          ) : null}
          {selectedRun.objective ? (
            <div className="border-b border-border/70 px-3 py-2">
              <p className="mb-1 text-caption font-semibold text-secondary-text">{t('subagentsTask')}</p>
              <p className="whitespace-pre-wrap break-words text-xs leading-5 text-text">{selectedRun.objective}</p>
            </div>
          ) : null}
          <div className="flex shrink-0 items-center gap-2 border-b border-border/70 px-3 py-2 text-caption text-secondary-text">
            <Bot aria-hidden="true" size={13} />
            <span className="min-w-0 truncate">
              {formatSubagentModel(selectedRun, t('subagentsModelUnavailable'))}
              {selectedRun.reasoningEffort ? ` · ${selectedRun.reasoningEffort}` : ''}
            </span>
            {selectedRun.progress?.phase ? (
              <span className="ml-auto shrink-0 truncate">
                {selectedRun.progress.toolName ?? selectedRun.progress.phase}
              </span>
            ) : null}
          </div>
          <section
            aria-label={t('subagentsReadOnlyTitle')}
            className="openchat-scroll-area min-h-0 flex-1 overflow-y-auto px-3 py-3"
          >
            {selectedRun.text || selectedRun.summary ? (
              <p className="whitespace-pre-wrap break-words text-xs leading-5 text-text">
                {selectedRun.text || selectedRun.summary}
              </p>
            ) : selectedRun.status === 'running' ? (
              <p className="text-xs leading-5 text-secondary-text">{t('subagentsWaitingForText')}</p>
            ) : selectedRun.status === 'failed' ? (
              <p className="flex items-center gap-2 text-xs leading-5 text-destructive">
                <TriangleAlert aria-hidden="true" size={14} />
                <span>{t('subagentsFailed')}</span>
              </p>
            ) : selectedRun.status === 'cancelled' || selectedRun.status === 'interrupted' ? (
              <p className="flex items-center gap-2 text-xs leading-5 text-secondary-text">
                <CircleX aria-hidden="true" size={14} />
                <span>{t('subagentsStopped')}</span>
              </p>
            ) : (
              <p className="flex items-center gap-2 text-xs leading-5 text-secondary-text">
                <Check aria-hidden="true" size={14} />
                <span>{t('subagentsCompleted')}</span>
              </p>
            )}
          </section>
        </section>
      ) : null}
    </div>
  );
}

function formatSubagentModel(run: SubagentRun, unavailable: string): string {
  if (!run.modelId) return unavailable;
  return run.providerId ? `${run.providerId} · ${run.modelId}` : run.modelId;
}

function statusTranslationKey(status: SubagentStatus) {
  switch (status) {
    case 'running':
      return 'subagentsStatusRunning';
    case 'paused':
      return 'subagentsStatusPaused';
    case 'completed':
      return 'subagentsStatusCompleted';
    case 'cancelled':
      return 'subagentsStatusCancelled';
    case 'interrupted':
      return 'subagentsStatusInterrupted';
    case 'failed':
      return 'subagentsStatusFailed';
  }
}

function SubagentStatusIcon({ status }: { status: SubagentStatus }) {
  const sharedProps = { 'aria-hidden': true as const, className: 'mt-0.5 shrink-0', size: 14 };
  if (status === 'running')
    return (
      <LoaderCircle {...sharedProps} className="mt-0.5 shrink-0 animate-spin text-accent motion-reduce:animate-none" />
    );
  if (status === 'completed') return <Check {...sharedProps} className="mt-0.5 shrink-0 text-success" />;
  if (status === 'failed') return <TriangleAlert {...sharedProps} className="mt-0.5 shrink-0 text-destructive" />;
  if (status === 'cancelled' || status === 'interrupted')
    return <CircleX {...sharedProps} className="mt-0.5 shrink-0 text-secondary-icon" />;
  return <CircleAlert {...sharedProps} className="mt-0.5 shrink-0 text-secondary-icon" />;
}
