import { AlertCircle, ChartNoAxesCombined, LoaderCircle } from 'lucide-react';
import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { PendingChatAttachment } from '@/modules/conversations/data/chatAttachments';
import type { ChatMessage } from '@/modules/conversations/data/chatStorage';
import {
  type ContextUsageConfiguration,
  type ContextUsageMemory,
  estimateContextUsage,
  inspectContextUsageMemory,
  loadContextUsageConfiguration,
} from '@/modules/conversations/data/contextUsage';
import type { ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { withStableKeys } from '@/shared/utils/reactKeys';

interface ContextUsageIndicatorProps {
  messages: readonly ChatMessage[];
  draft: string;
  pendingAttachments: readonly PendingChatAttachment[];
  model: ChatModel;
  contextWindow: number | null;
  conversationId: string | null;
  isTemporaryChat: boolean;
  isSending: boolean;
  projectId: string | null;
  toolPermissionMode: ToolPermissionMode | 'plan';
}

type LoadState = 'idle' | 'loading' | 'ready' | 'error';
const contextUsageSegments = Array.from({ length: 100 }, (_, index) => ({ id: `context-usage-${index}`, index }));

interface UsageCategory {
  key: string;
  label: string;
  tokens: number;
  color: string;
  details: string[];
}

export function ContextUsageIndicator({
  contextWindow,
  conversationId,
  isTemporaryChat,
  draft,
  isSending,
  messages,
  model,
  pendingAttachments,
  projectId,
  toolPermissionMode,
}: ContextUsageIndicatorProps) {
  const { i18n, t } = useTranslation();
  const popoverRef = useRef<HTMLDetailsElement | null>(null);
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);
  const [configuration, setConfiguration] = useState<ContextUsageConfiguration | null>(null);
  const [configurationState, setConfigurationState] = useState<LoadState>('idle');
  const [memory, setMemory] = useState<ContextUsageMemory | null>(null);
  const [memoryState, setMemoryState] = useState<LoadState>(conversationId === null ? 'ready' : 'idle');

  useEffect(() => {
    if (!isPopoverOpen) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && !popoverRef.current?.contains(event.target)) {
        if (popoverRef.current) popoverRef.current.open = false;
        setIsPopoverOpen(false);
      }
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    return () => document.removeEventListener('pointerdown', closeOnOutsidePointer);
  }, [isPopoverOpen]);

  useEffect(() => {
    let current = true;
    setConfiguration(null);
    setConfigurationState('loading');
    void loadContextUsageConfiguration(model, conversationId, projectId, toolPermissionMode, isTemporaryChat)
      .then((nextConfiguration) => {
        if (!current) return;
        setConfiguration(nextConfiguration);
        setConfigurationState('ready');
      })
      .catch((cause: unknown) => {
        if (!current) return;
        globalThis.reportError(new Error('Context usage configuration could not be loaded.', { cause }));
        setConfiguration(null);
        setConfigurationState('error');
      });
    return () => {
      current = false;
    };
  }, [conversationId, isTemporaryChat, model, projectId, toolPermissionMode]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Sending starts and finishes a turn, so it refreshes saved memory.
  useEffect(() => {
    let current = true;
    if (conversationId === null) {
      setMemory(null);
      setMemoryState('ready');
      return () => {
        current = false;
      };
    }
    setMemory(null);
    setMemoryState('loading');
    void inspectContextUsageMemory(conversationId)
      .then((nextMemory) => {
        if (!current) return;
        setMemory(nextMemory);
        setMemoryState('ready');
      })
      .catch((cause: unknown) => {
        if (!current) return;
        globalThis.reportError(new Error('Conversation memory inspection failed.', { cause }));
        setMemory(null);
        setMemoryState('error');
      });
    return () => {
      current = false;
    };
  }, [conversationId, isSending]);

  const snapshot = useMemo(
    () => estimateContextUsage(messages, draft, pendingAttachments, model, configuration, memory),
    [configuration, draft, memory, messages, model, pendingAttachments],
  );
  const usageFraction = contextWindow !== null && contextWindow > 0 ? snapshot.totalTokens / contextWindow : null;
  const percent = usageFraction === null ? null : Math.min(100, Math.round(usageFraction * 100));
  const numberFormat = useMemo(
    () => new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 0 }),
    [i18n.language],
  );
  const percentFormat = useMemo(
    () =>
      new Intl.NumberFormat(i18n.language, {
        style: 'percent',
        maximumFractionDigits: 2,
      }),
    [i18n.language],
  );
  const formatCount = (value: number) => numberFormat.format(Math.max(0, Math.round(value)));
  const formatPercent = (value: number) =>
    contextWindow === null || contextWindow <= 0 ? '—' : percentFormat.format(value / contextWindow);
  const attachmentTokens = snapshot.attachments.reduce((sum, item) => sum + item.tokens, 0);
  const messageTokens = snapshot.userMessageTokens + snapshot.assistantMessageTokens;
  const toolUsageTokens = snapshot.toolUsage.reduce((sum, item) => sum + item.tokens, 0);
  const detailsUnavailable =
    configurationState === 'error' || memoryState === 'error' || configuration?.projectContextUnavailable === true;
  const statusText =
    configurationState === 'loading'
      ? t('contextUsageConfigurationLoading')
      : detailsUnavailable
        ? t('contextUsageConfigurationUnavailable')
        : percent === null
          ? t('contextUsageNoModelLimit')
          : t('contextUsageSummary', {
              used: formatCount(snapshot.totalTokens),
              limit: formatCount(contextWindow ?? 0),
              percent: percentFormat.format(snapshot.totalTokens / (contextWindow ?? 1)),
            });
  const categories: UsageCategory[] = [
    {
      key: 'instructions',
      label: t('contextUsageInstructionsEstimate', {
        count: formatCount(snapshot.instructionsTokens),
        percent: formatPercent(snapshot.instructionsTokens),
      }),
      tokens: snapshot.instructionsTokens,
      color: 'rgb(var(--accent))',
      details: [],
    },
    {
      key: 'tool-definitions',
      label: t('contextUsageToolDefinitionsEstimate', {
        count: formatCount(snapshot.toolDefinitionTokens),
        percent: formatPercent(snapshot.toolDefinitionTokens),
      }),
      tokens: snapshot.toolDefinitionTokens,
      color: 'rgb(var(--info))',
      details: [],
    },
    {
      key: 'messages',
      label: t('contextUsageMessagesEstimate', {
        count: formatCount(messageTokens),
        percent: formatPercent(messageTokens),
      }),
      tokens: messageTokens,
      color: 'rgb(var(--secondary-icon))',
      details: [
        t('contextUsageUserMessagesEstimate', {
          count: formatCount(snapshot.userMessageTokens),
          percent: formatPercent(snapshot.userMessageTokens),
        }),
        t('contextUsageAssistantMessagesEstimate', {
          count: formatCount(snapshot.assistantMessageTokens),
          percent: formatPercent(snapshot.assistantMessageTokens),
        }),
      ],
    },
    {
      key: 'attachments',
      label: t('contextUsageAttachmentsEstimate', {
        count: formatCount(attachmentTokens),
        percent: formatPercent(attachmentTokens),
      }),
      tokens: attachmentTokens,
      color: 'rgb(var(--warning))',
      details: snapshot.attachments.map((attachment) =>
        attachment.isDraft
          ? t('contextUsageDraftAttachment', { name: attachment.name })
          : t('contextUsageAttachmentEstimate', {
              name: attachment.name,
              count: formatCount(attachment.tokens),
              percent: formatPercent(attachment.tokens),
            }),
      ),
    },
    {
      key: 'tools',
      label: t('contextUsageToolsEstimate', {
        count: formatCount(toolUsageTokens),
        percent: formatPercent(toolUsageTokens),
      }),
      tokens: toolUsageTokens,
      color: 'rgb(var(--success))',
      details: snapshot.toolUsage.map((item) =>
        t('contextUsageToolUsageEstimate', {
          name: item.name,
          count: formatCount(item.tokens),
          percent: formatPercent(item.tokens),
        }),
      ),
    },
    {
      key: 'memory',
      label: t('contextUsageMemoryEstimate', {
        count: formatCount(snapshot.memoryTokens),
        percent: formatPercent(snapshot.memoryTokens),
      }),
      tokens: snapshot.memoryTokens,
      color: 'rgb(var(--destructive))',
      details: [],
    },
    {
      key: 'draft',
      label: t('contextUsageDraftEstimate', {
        count: formatCount(snapshot.draftTokens),
        percent: formatPercent(snapshot.draftTokens),
      }),
      tokens: snapshot.draftTokens,
      color: 'rgb(var(--accent-hover))',
      details: [],
    },
  ];
  const usageCellBackgrounds = createUsageCellBackgrounds(categories, usageFraction);
  const ringColor =
    percent !== null && percent >= 90
      ? 'rgb(var(--destructive))'
      : percent !== null && percent >= 75
        ? 'rgb(var(--warning))'
        : 'rgb(var(--accent))';
  const modelLabel = `${model.providerId}/${model.modelId}`;
  const contextLimitLabel =
    contextWindow !== null && contextWindow > 0
      ? t('contextUsageModelLimit', { count: formatCount(contextWindow) })
      : t('contextUsageNoModelLimit');
  const contextCountLabel = `${formatCount(snapshot.totalTokens)} / ${
    contextWindow !== null && contextWindow > 0 ? formatCount(contextWindow) : '—'
  }`;
  const closePopoverOnEscape = (event: KeyboardEvent<HTMLDetailsElement>) => {
    if (event.key !== 'Escape' || !event.currentTarget.open) return;
    event.preventDefault();
    event.currentTarget.open = false;
    setIsPopoverOpen(false);
    event.currentTarget.querySelector('summary')?.focus();
  };

  return (
    <details
      className="context-usage-indicator"
      onKeyDown={closePopoverOnEscape}
      onToggle={(event) => setIsPopoverOpen(event.currentTarget.open)}
      ref={popoverRef}
    >
      <summary
        aria-label={`${t('contextUsageTitle')}: ${statusText}`}
        className="context-usage-trigger focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        title={`${contextCountLabel} · ${t('contextUsageTitle')}: ${statusText}`}
      >
        <span aria-hidden="true" className="context-usage-count">
          {contextCountLabel}
        </span>
        {configurationState === 'loading' ? (
          <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-icon" size={20} />
        ) : detailsUnavailable ? (
          <AlertCircle aria-hidden="true" className="text-destructive" size={20} />
        ) : percent === null ? (
          <ChartNoAxesCombined aria-hidden="true" className="text-secondary-icon" size={20} />
        ) : (
          <span
            aria-hidden="true"
            className="context-usage-ring"
            style={{ background: `conic-gradient(${ringColor} ${percent}%, rgb(var(--border)) ${percent}% 100%)` }}
          >
            <span className="context-usage-ring__value">{percentFormat.format(percent / 100)}</span>
          </span>
        )}
      </summary>
      <section
        aria-label={t('contextUsageTitle')}
        className="context-usage-popover openchat-dropdown-surface openchat-popover-enter openchat-popover-enter--bottom-right"
      >
        <header className="grid gap-1">
          <h2 className="font-semibold text-text">{t('contextUsageTitle')}</h2>
          <div className="flex flex-wrap items-center justify-between gap-2 text-secondary-text">
            <span className="min-w-0 truncate" title={modelLabel}>
              {modelLabel}
            </span>
            <span className="shrink-0">
              {t('contextUsageSummary', {
                used: formatCount(snapshot.totalTokens),
                limit: contextWindow !== null && contextWindow > 0 ? formatCount(contextWindow) : '—',
                percent: percent === null ? '—' : percentFormat.format(snapshot.totalTokens / (contextWindow ?? 1)),
              })}
            </span>
          </div>
          <p className="flex items-center gap-1 text-secondary-text">
            <ChartNoAxesCombined aria-hidden="true" size={14} />
            <span>{contextLimitLabel}</span>
          </p>
        </header>
        <div className="context-usage-overview">
          {contextWindow !== null && contextWindow > 0 ? (
            <div aria-hidden="true" className="context-usage-grid">
              {contextUsageSegments.map((segment) => {
                return (
                  <span
                    className="context-usage-cell"
                    key={segment.id}
                    style={{ backgroundImage: usageCellBackgrounds[segment.index] }}
                  />
                );
              })}
            </div>
          ) : null}
          <div className="context-usage-categories">
            {categories
              .filter((category) => category.tokens > 0)
              .map((category) => (
                <div className="grid gap-1" key={category.key}>
                  <div className="flex min-w-0 items-start gap-2">
                    <span
                      aria-hidden="true"
                      className="context-usage-category-dot"
                      style={{ background: category.color }}
                    />
                    <span className="min-w-0 break-words">{category.label}</span>
                  </div>
                  {withStableKeys(category.details, (detail) => detail, `${category.key}-detail`).map(
                    ({ item: detail, key }) => (
                      <div className="pl-5 text-secondary-text" key={key}>
                        {detail}
                      </div>
                    ),
                  )}
                </div>
              ))}
            {contextWindow !== null && contextWindow > 0 ? (
              <div className="flex min-w-0 items-start gap-2">
                <span aria-hidden="true" className="context-usage-category-dot context-usage-category-dot--empty" />
                <span>
                  {t('contextUsageFreeSpaceEstimate', {
                    count: formatCount(Math.max(0, contextWindow - snapshot.totalTokens)),
                    percent: formatPercent(Math.max(0, contextWindow - snapshot.totalTokens)),
                  })}
                </span>
              </div>
            ) : null}
          </div>
        </div>
        {configurationState === 'loading' ? (
          <p className="mt-3" role="status">
            {t('contextUsageConfigurationLoading')}
          </p>
        ) : null}
        {configurationState === 'error' || configuration?.projectContextUnavailable ? (
          <p className="mt-3 text-destructive" role="status">
            {t('contextUsageConfigurationUnavailable')}
          </p>
        ) : null}
        {memoryState === 'error' ? (
          <p className="mt-2 text-destructive" role="status">
            {t('contextUsageMeasurementUnavailable')}
          </p>
        ) : null}
        {contextWindow !== null && contextWindow > 0 && snapshot.totalTokens > contextWindow ? (
          <p className="mt-3 font-semibold text-destructive" role="status">
            {t('contextUsageOverLimit')}
          </p>
        ) : null}
        {snapshot.providerInputTokens !== null ? (
          <p className="mt-3 border-t border-border pt-2 text-secondary-text" role="status">
            {t('contextUsageProviderMeasurement', { count: formatCount(snapshot.providerInputTokens) })}
          </p>
        ) : null}
      </section>
    </details>
  );
}

function createUsageCellBackgrounds(
  categories: readonly UsageCategory[],
  usageFraction: number | null,
): Array<string | undefined> {
  const usagePercent = usageFraction === null ? 0 : Math.min(100, Math.max(0, usageFraction * 100));
  const categoryTokenTotal = categories.reduce((sum, category) => sum + category.tokens, 0);
  if (usagePercent === 0 || categoryTokenTotal === 0) return contextUsageSegments.map(() => undefined);

  let cumulativeTokens = 0;
  const categoryRanges = categories.flatMap((category) => {
    if (category.tokens <= 0) return [];
    const start = (cumulativeTokens / categoryTokenTotal) * usagePercent;
    cumulativeTokens += category.tokens;
    const end = (cumulativeTokens / categoryTokenTotal) * usagePercent;
    return [{ start, end, color: category.color }];
  });

  return contextUsageSegments.map(({ index }) => {
    const cellEnd = index + 1;
    const stops: string[] = [];
    for (const range of categoryRanges) {
      const start = Math.max(index, range.start);
      const end = Math.min(cellEnd, range.end);
      if (end <= start) continue;
      const startPercent = (start - index) * 100;
      const endPercent = (end - index) * 100;
      stops.push(`${range.color} ${startPercent}%`, `${range.color} ${endPercent}%`);
    }
    if (stops.length === 0) return undefined;

    const fillEnd = Math.min(100, Math.max(0, (usagePercent - index) * 100));
    if (fillEnd < 100) stops.push(`transparent ${fillEnd}%`, `transparent 100%`);
    return `linear-gradient(90deg, ${stops.join(', ')})`;
  });
}
