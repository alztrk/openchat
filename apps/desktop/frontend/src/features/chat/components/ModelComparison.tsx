import { X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { ChatModel } from '@entities/model';
import { type ChatConversation, createConversation, deleteConversation } from '@features/chat/data';
import { loadModelComparisonMetrics, type ModelComparisonMetrics } from '@features/chat/data/comparisonMetrics';
import {
  getComparisonHistoryLoadState,
  canContinueComparisonConversation,
  hasValidComparisonModelSelection,
  maximumComparisonModels,
  toggleComparisonModelSelection,
} from '@features/chat/data/modelComparison';
import { ConversationPane } from '@features/chat/components/ConversationPane';
import { useChatStore } from '@features/chat/state';
import { Button, Checkbox, Dialog, toast } from '@shared/components';
import { providerNameForModel } from '@entities/model/composerUtils';
import { modelRouteKey } from '@entities/model/modelPreferences';
import { isObject } from '@shared/lib/utils/json';

import './ModelComparison.css';

interface ComparisonEntry {
  conversationId: string;
  model: ChatModel;
  reasoningEffort: string | null;
  startedAtUnixMs: number;
}

interface ComparisonMetricsState {
  status: 'loading' | 'loaded';
  metrics: ModelComparisonMetrics | null;
}

interface ModelComparisonProps {
  agent: ChatConversation['agent'];
  initialModelKeys: readonly string[];
  models: readonly ChatModel[];
  onClose(): void;
  onOpenSettings(): void;
  onRequestTemporaryExit(action: () => void | Promise<void>): void;
  onStarted(): void;
  projectId: string | null;
  prompt: string;
}

const noop = (): void => undefined;

export function ModelComparison({
  agent,
  initialModelKeys,
  models,
  onClose,
  onOpenSettings,
  onRequestTemporaryExit,
  onStarted,
  projectId,
  prompt,
}: ModelComparisonProps) {
  const { i18n, t } = useTranslation();
  const [comparisonPrompt] = useState(prompt);
  const [selectedKeys, setSelectedKeys] = useState<string[]>(() => {
    const availableKeys = new Set(models.map((model) => model.key));
    return [...new Set(initialModelKeys.filter((key) => availableKeys.has(key)))].slice(0, maximumComparisonModels);
  });
  const [entries, setEntries] = useState<ComparisonEntry[]>([]);
  const [isCreating, setIsCreating] = useState(false);
  const [isWorkspaceOpen, setIsWorkspaceOpen] = useState(false);
  const [phase, setPhase] = useState<'selecting' | 'loading' | 'sending' | 'active' | 'failed'>('selecting');
  const [readyConversationIds, setReadyConversationIds] = useState<ReadonlySet<string>>(() => new Set());
  const [failedConversationIds, setFailedConversationIds] = useState<ReadonlySet<string>>(() => new Set());
  const [completedConversationIds, setCompletedConversationIds] = useState<ReadonlySet<string>>(() => new Set());
  const [metricsByConversation, setMetricsByConversation] = useState<Record<string, ComparisonMetricsState>>({});
  const sendStartedForRef = useRef<string | null>(null);
  const closeBeforeSendStartedRef = useRef(false);
  const measuredMessageIdsRef = useRef<Set<string>>(new Set());
  const metricsFailureNotifiedRef = useRef(false);
  const refreshConversations = useChatStore((state) => state.refreshConversations);
  const selectConversation = useChatStore((state) => state.selectConversation);
  const sendMessage = useChatStore((state) => state.sendMessage);

  const selectedModels = selectedKeys
    .map((key) => models.find((model) => model.key === key))
    .filter((model): model is ChatModel => model !== undefined);
  const canStart = hasValidComparisonModelSelection(selectedKeys) && selectedModels.length === selectedKeys.length;
  const markConversationMessagesReady = useCallback((conversationId: string) => {
    setReadyConversationIds((current) => {
      if (current.has(conversationId)) return current;
      return new Set([...current, conversationId]);
    });
  }, []);
  const markConversationMessagesLoadFailed = useCallback((conversationId: string) => {
    setFailedConversationIds((current) => {
      if (current.has(conversationId)) return current;
      return new Set([...current, conversationId]);
    });
  }, []);

  useEffect(() => {
    const availableKeys = new Set(models.map((model) => model.key));
    setSelectedKeys((current) => current.filter((key) => availableKeys.has(key)));
  }, [models]);

  const removeCreatedConversations = useCallback(async (conversationIds: readonly string[]) => {
    const results = await Promise.allSettled(conversationIds.map((id) => deleteConversation(id)));
    const failedCleanup = results.some((result) => result.status === 'rejected');
    if (failedCleanup) {
      globalThis.reportError(new Error('One or more incomplete model comparison conversations could not be removed.'));
    }
    try {
      await refreshConversations();
    } catch (cause) {
      globalThis.reportError(new Error('The conversation list could not be refreshed after comparison cleanup.', { cause }));
    }
  }, [refreshConversations]);

  const startComparison = async () => {
    if (isCreating || !canStart || !comparisonPrompt.trim()) return;

    const currentState = useChatStore.getState();
    const currentModels = selectedKeys
      .map((key) => currentState.models.find((model) => model.key === key))
      .filter((model): model is ChatModel => model !== undefined);
    if (
      !hasValidComparisonModelSelection(selectedKeys) ||
      currentModels.length !== selectedKeys.length ||
      currentModels.some((model) => currentState.hiddenModelKeys.includes(modelRouteKey(model)))
    ) {
      toast.error(t('comparisonModelsChanged'));
      return;
    }

    const createdEntries = currentModels.map((model) => {
      const configuredEffort = currentState.modelReasoningEfforts[modelRouteKey(model)];
      return {
        conversationId: crypto.randomUUID().replaceAll('-', ''),
        model,
        startedAtUnixMs: Date.now(),
        reasoningEffort:
          model.supportsReasoning && configuredEffort && model.reasoningLevels.includes(configuredEffort)
            ? configuredEffort
            : null,
      };
    });
    setIsCreating(true);
    try {
      const creationResults = await Promise.allSettled(
        createdEntries.map(({ conversationId, model, reasoningEffort }) =>
          createConversation(
            conversationId,
            t('comparisonConversationTitle', { model: model.displayName }),
            model,
            projectId,
            agent,
            reasoningEffort,
          ),
        ),
      );
      const createdConversationIds = creationResults.flatMap((result, index) =>
        result.status === 'fulfilled' ? [createdEntries[index]?.conversationId].filter((id) => id !== undefined) : [],
      );
      if (creationResults.some((result) => result.status === 'rejected')) {
        await removeCreatedConversations(createdConversationIds);
        toast.error(t('comparisonStartFailed'));
        return;
      }

      try {
        await refreshConversations();
      } catch (cause) {
        globalThis.reportError(new Error('Model comparison conversations could not be loaded.', { cause }));
        await removeCreatedConversations(createdConversationIds);
        toast.error(t('comparisonStartFailed'));
        return;
      }

      setEntries(createdEntries);
      setPhase('loading');
      setIsWorkspaceOpen(true);
      onStarted();
    } catch (cause) {
      globalThis.reportError(new Error('Model comparison could not be started.', { cause }));
      await removeCreatedConversations(createdEntries.map((entry) => entry.conversationId));
      toast.error(t('comparisonStartFailed'));
    } finally {
      setIsCreating(false);
    }
  };

  const closeWorkspace = () => {
    if (entries.length > 0 && sendStartedForRef.current === null && !closeBeforeSendStartedRef.current) {
      closeBeforeSendStartedRef.current = true;
      void removeCreatedConversations(entries.map((entry) => entry.conversationId));
    }
    setIsWorkspaceOpen(false);
    onClose();
  };

  useEffect(() => {
    if (phase !== 'loading' || entries.length === 0) return;
    const loadState = getComparisonHistoryLoadState(
      entries.map(({ conversationId }) => conversationId),
      readyConversationIds,
      failedConversationIds,
    );
    if (loadState === 'ready') {
      setPhase('sending');
      return;
    }
    if (loadState === 'failed') {
      if (closeBeforeSendStartedRef.current) return;
      closeBeforeSendStartedRef.current = true;
      setPhase('failed');
      toast.error(t('comparisonStartFailed'));
      void removeCreatedConversations(entries.map(({ conversationId }) => conversationId));
      setIsWorkspaceOpen(false);
      onClose();
    }
  }, [entries, failedConversationIds, onClose, phase, readyConversationIds, removeCreatedConversations, t]);

  useEffect(() => {
    if (phase !== 'sending' || entries.length === 0) return;
    const comparisonKey = entries.map((entry) => entry.conversationId).join(':');
    if (sendStartedForRef.current === comparisonKey) return;
    sendStartedForRef.current = comparisonKey;

    const sends = entries.map(async ({ conversationId, model, reasoningEffort }) => {
      try {
        const sent = await sendMessage(comparisonPrompt, [], undefined, undefined, {
          conversationId,
          ...(reasoningEffort ? { reasoningEffort } : {}),
        });
        if (!sent) toast.error(t('comparisonModelSendFailed', { model: model.displayName }));
      } catch (cause) {
        globalThis.reportError(new Error(`The comparison request for ${model.displayName} failed to start.`, { cause }));
        toast.error(t('comparisonModelSendFailed', { model: model.displayName }));
      }
    });
    setPhase('active');
    void Promise.allSettled(sends).then((results) => {
      for (const result of results) {
        if (result.status === 'rejected') {
          globalThis.reportError(new Error('A model comparison request failed unexpectedly.', { cause: result.reason }));
        }
      }
    });
  }, [comparisonPrompt, entries, phase, sendMessage, t]);

  useEffect(() => {
    if (phase !== 'active' || entries.length === 0) return;
    const onMessageStream = (event: Event) => {
      if (!(event instanceof CustomEvent) || !isObject(event.detail) || !isObject(event.detail.message)) return;
      const message = event.detail.message;
      if (
        typeof message.id !== 'string' ||
        message.role !== 'assistant' ||
        message.status !== 'completed' ||
        typeof message.conversationId !== 'string'
      ) {
        return;
      }
      const entry = entries.find((item) => item.conversationId === message.conversationId);
      if (!entry || measuredMessageIdsRef.current.has(message.id)) return;
      measuredMessageIdsRef.current.add(message.id);
      setCompletedConversationIds((current) => new Set([...current, entry.conversationId]));
      setMetricsByConversation((current) => ({
        ...current,
        [entry.conversationId]: { status: 'loading', metrics: current[entry.conversationId]?.metrics ?? null },
      }));
      void loadModelComparisonMetrics({
        conversationId: entry.conversationId,
        providerId: entry.model.providerId,
        modelId: entry.model.modelId,
        fromUnixMs: entry.startedAtUnixMs,
      })
        .then((metrics) => {
          setMetricsByConversation((current) => ({
            ...current,
            [entry.conversationId]: { status: 'loaded', metrics },
          }));
        })
        .catch((cause: unknown) => {
          globalThis.reportError(new Error('Model comparison usage metrics could not be loaded.', { cause }));
          if (!metricsFailureNotifiedRef.current) {
            metricsFailureNotifiedRef.current = true;
            toast.error(t('comparisonMetricsLoadFailed'));
          }
          setMetricsByConversation((current) => ({
            ...current,
            [entry.conversationId]: { status: 'loaded', metrics: null },
          }));
        });
    };
    window.addEventListener('openchat-message-stream', onMessageStream);
    return () => window.removeEventListener('openchat-message-stream', onMessageStream);
  }, [entries, phase, t]);

  const continueWithComparison = async (conversationId: string) => {
    if (!canContinueComparisonConversation(phase, conversationId, completedConversationIds)) return;
    await selectConversation(conversationId);
    closeWorkspace();
  };

  return (
    <>
      <Dialog
        onOpenChange={(open) => {
          if (!open && !isCreating) onClose();
        }}
        open={!isWorkspaceOpen}
        size="wide"
        title={t('comparisonTitle')}
      >
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('comparisonDescription')}</p>
          <div className="max-h-28 overflow-y-auto rounded-card border border-border bg-raised p-3 text-sm whitespace-pre-wrap break-words">
            {comparisonPrompt}
          </div>
          <p className="text-sm text-secondary-text">
            {t('comparisonSelectedCount', { count: selectedModels.length, maximum: maximumComparisonModels })}
          </p>
          <fieldset
            aria-label={t('comparisonModels')}
            className="openchat-scroll-area m-0 grid max-h-64 min-w-0 gap-2 overflow-y-auto border-0 p-0"
          >
            {models.map((model) => {
              const checked = selectedKeys.includes(model.key);
              const limitReached = selectedKeys.length >= maximumComparisonModels && !checked;
              const checkboxId = `comparison-model-${encodeURIComponent(model.key)}`;
              return (
                <div
                  className="flex min-h-12 cursor-pointer items-center gap-3 rounded-card border border-border bg-background px-3 py-2 hover:bg-hover has-[:checked]:border-accent"
                  key={model.key}
                >
                  <Checkbox
                    checked={checked}
                    disabled={limitReached}
                    id={checkboxId}
                    onChange={() => setSelectedKeys((current) => toggleComparisonModelSelection(current, model.key))}
                  />
                  <label className="min-w-0 flex-1 cursor-pointer" htmlFor={checkboxId}>
                    <span className="block truncate text-sm font-medium text-text">{model.displayName}</span>
                    <span className="block truncate text-xs text-secondary-text">{providerNameForModel(model, t)}</span>
                  </label>
                </div>
              );
            })}
          </fieldset>
          {models.length === 0 ? <p className="text-sm text-secondary-text">{t('comparisonNoModels')}</p> : null}
          <div className="flex flex-wrap justify-end gap-2">
            <Button disabled={isCreating} onClick={onClose}>
              {t('cancel')}
            </Button>
            <Button
              disabled={!canStart || !comparisonPrompt.trim()}
              loading={isCreating}
              onClick={() => void startComparison()}
              variant="primary"
            >
              {t('comparisonStart')}
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog
        className="model-comparison-workspace-dialog"
        onOpenChange={(open) => {
          if (!open) closeWorkspace();
        }}
        open={isWorkspaceOpen}
        size="wide"
        title={t('comparisonWorkspaceTitle')}
      >
        <div className="model-comparison-workspace">
          <div className="model-comparison-toolbar">
            <p aria-live="polite" className="min-w-0 flex-1 truncate text-sm text-secondary-text">
              {t(phase === 'loading' ? 'comparisonPreparing' : 'comparisonWorkspaceDescription')}
            </p>
            <Button aria-label={t('close')} onClick={closeWorkspace} size="icon" variant="ghost">
              <X aria-hidden="true" size={18} />
            </Button>
          </div>
          <div className="model-comparison-panes" data-pane-count={entries.length}>
            {entries.map(({ conversationId, model }) => {
              const metricsState = metricsByConversation[conversationId];
              const metrics = metricsState?.metrics;
              return (
              <ConversationPane
                conversationId={conversationId}
                header={
                  <div className="grid gap-1 border-b border-border px-4 py-2">
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium text-text">{model.displayName}</p>
                      <p className="truncate text-xs text-secondary-text">{providerNameForModel(model, t)}</p>
                    </div>
                    <fieldset className="m-0 flex min-w-0 flex-wrap gap-x-3 gap-y-1 border-0 p-0 text-xs text-secondary-text">
                      <legend className="sr-only">{t('comparisonMetrics')}</legend>
                      <span>{t('comparisonMetricFirstToken', { value: formatLatency(metrics?.firstOutputLatencyMs ?? null, i18n.resolvedLanguage) })}</span>
                      <span>{t('comparisonMetricOutputRate', { value: formatDecimal(metrics?.outputTokensPerSecond ?? null, i18n.resolvedLanguage) })}</span>
                      <span>
                        {t('comparisonMetricTokens', {
                          input: formatCount(metrics?.inputTokens ?? null, i18n.resolvedLanguage),
                          output: formatCount(metrics?.outputTokens ?? null, i18n.resolvedLanguage),
                        })}
                      </span>
                      <span>{t('comparisonMetricCost', { value: formatUsd(metrics?.estimatedCostUsd ?? null, i18n.resolvedLanguage) })}</span>
                    </fieldset>
                    {metricsState?.status === 'loading' ? (
                      <span className="text-xs text-secondary-text">{t('comparisonMetricsLoading')}</span>
                    ) : null}
                    <Button
                      className="justify-self-start"
                      disabled={!canContinueComparisonConversation(phase, conversationId, completedConversationIds)}
                      onClick={() => void continueWithComparison(conversationId)}
                      size="small"
                      title={t('comparisonContinueWithModel', { model: model.displayName })}
                      variant="secondary"
                    >
                      {t('comparisonContinue')}
                    </Button>
                  </div>
                }
                isLargeText={false}
                key={conversationId}
                notificationFeedbackKey={null}
                onDismissNotificationFeedback={noop}
                onConversationMessagesLoadFailed={markConversationMessagesLoadFailed}
                onConversationMessagesReady={markConversationMessagesReady}
                onOpenSettings={onOpenSettings}
                onQuestionFocusHandled={noop}
                onRequestTemporaryExit={onRequestTemporaryExit}
                questionFocusTarget={null}
              />
              );
            })}
          </div>
        </div>
      </Dialog>
    </>
  );
}

function formatCount(value: number | null, locale: string | undefined): string {
  return value === null ? '—' : new Intl.NumberFormat(locale).format(value);
}

function formatDecimal(value: number | null, locale: string | undefined): string {
  return value === null ? '—' : new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
}

function formatLatency(value: number | null, locale: string | undefined): string {
  if (value === null) return '—';
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value)} ms`;
}

function formatUsd(value: number | null, locale: string | undefined): string {
  if (value === null) return '—';
  return new Intl.NumberFormat(locale, { style: 'currency', currency: 'USD', maximumFractionDigits: 6 }).format(value);
}
