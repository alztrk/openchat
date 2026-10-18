import { LoaderCircle } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ChatFileChange } from '@/modules/conversations/data/chatFileChanges';
import type { ChatMessage } from '@/modules/conversations/data/chatStorage';
import { useVirtualMessageList } from '@/modules/conversations/hooks/useVirtualMessageList';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { ProviderIcon } from '@/modules/providers/components/ProviderIcon';
import { MessageItem } from './MessageItem';

export interface MessageListProps {
  assistantModelFallback?: string | null;
  assistantProviderFallback?: string | null;
  showAssistantLoading?: boolean;
  savedOutputMessageIds?: ReadonlySet<string>;
  pendingSavedOutputMessageIds?: ReadonlySet<string>;
  onToggleSavedOutput?: (message: ChatMessage) => void;
  onRetryMessage?: (message: ChatMessage) => void;
  onEditMessage?: (message: ChatMessage) => void;
  onSelectResponseVersion?: (message: ChatMessage, versionIndex: number) => void;
  latestFileChanges?: ChatFileChange[];
  onOpenFileChanges?: () => void;
  onOpenConnections?: () => void;
}

export function MessageList({
  assistantModelFallback = null,
  assistantProviderFallback = null,
  showAssistantLoading = false,
  onEditMessage,
  onRetryMessage,
  onSelectResponseVersion,
  onToggleSavedOutput,
  pendingSavedOutputMessageIds,
  savedOutputMessageIds,
  latestFileChanges,
  onOpenFileChanges,
  onOpenConnections,
}: MessageListProps) {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const messages = useChatStore((state) => state.messages);
  const isLoadingMessages = useChatStore((state) => state.isLoadingMessages);
  const searchTargetMessageId = useChatStore((state) => state.searchTargetMessageId);
  const searchHighlightMessageId = useChatStore((state) => state.searchHighlightMessageId);
  const clearSearchTarget = useChatStore((state) => state.clearSearchTarget);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const skipBottomScroll = useRef(false);
  const responseGroups = useMemo(() => groupResponses(messages), [messages]);
  const [selectedResponseVersions, setSelectedResponseVersions] = useState<Record<string, number>>({});
  const displayRows = useMemo(
    () =>
      responseGroups.map((group) => {
        const selectedIndex = normalizeResponseIndex(selectedResponseVersions[group.id], group.messages.length);
        const message = group.messages[selectedIndex];
        if (!message) throw new RangeError('Selected response version is outside its message group.');
        return {
          group,
          message,
          responseVersionCount: group.messages.length,
          responseVersionIndex: selectedIndex,
        };
      }),
    [responseGroups, selectedResponseVersions],
  );
  const visibleMessages = useMemo(() => displayRows.map((row) => row.message), [displayRows]);
  const hasStreamingAssistant =
    visibleMessages.at(-1)?.role === 'assistant' && visibleMessages.at(-1)?.status === 'streaming';
  const virtualList = useVirtualMessageList(visibleMessages, activeConversationId);
  const selectResponseVersion = (message: ChatMessage, versionIndex: number) => {
    const group = responseGroups.find((candidate) => candidate.messages.some((item) => item.id === message.id));
    if (!group || versionIndex < 0 || versionIndex >= group.messages.length) return;
    const selectedMessage = group.messages[versionIndex];
    if (!selectedMessage) return;
    setSelectedResponseVersions((current) => ({ ...current, [group.id]: versionIndex }));
    onSelectResponseVersion?.(selectedMessage, versionIndex);
  };

  useEffect(() => {
    if (isLoadingMessages || !searchTargetMessageId) return;
    const group = responseGroups.find((candidate) =>
      candidate.messages.some((message) => message.id === searchTargetMessageId),
    );
    if (!group) return;
    const selectedIndex = group.messages.findIndex((message) => message.id === searchTargetMessageId);
    if (selectedIndex < 0) return;
    setSelectedResponseVersions((current) =>
      current[group.id] === selectedIndex ? current : { ...current, [group.id]: selectedIndex },
    );
  }, [isLoadingMessages, responseGroups, searchTargetMessageId]);

  useEffect(() => {
    if (isLoadingMessages || !searchTargetMessageId) return;
    if (!virtualList.rows.some(({ message }) => message.id === searchTargetMessageId)) {
      if (responseGroups.some((group) => group.messages.some((message) => message.id === searchTargetMessageId)))
        return;
      if (!virtualList.scrollToMessage(searchTargetMessageId)) clearSearchTarget(false);
      return;
    }
    const target = document.getElementById(`openchat-message-${searchTargetMessageId}`);
    if (!target) return;
    virtualList.stopFollowingLatest();
    skipBottomScroll.current = true;
    target.scrollIntoView({ block: 'center', behavior: 'smooth' });
    clearSearchTarget(true);
  }, [
    clearSearchTarget,
    isLoadingMessages,
    responseGroups,
    searchTargetMessageId,
    virtualList.rows,
    virtualList.scrollToMessage,
    virtualList.stopFollowingLatest,
  ]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Message, loading, and conversation changes intentionally trigger scroll-follow updates.
  useEffect(() => {
    if (isLoadingMessages || searchTargetMessageId) return;
    if (skipBottomScroll.current) {
      skipBottomScroll.current = false;
      return;
    }
    if (!virtualList.isFollowingLatest()) return;
    virtualList.scrollToBottom();
  }, [
    activeConversationId,
    isLoadingMessages,
    messages.length,
    messages.at(-1)?.content,
    searchTargetMessageId,
    showAssistantLoading,
    virtualList.isFollowingLatest,
    virtualList.scrollToBottom,
  ]);

  if (isLoadingMessages) {
    return (
      <div
        aria-label={t('messageHistoryLoading')}
        className="flex min-h-0 flex-1 items-center justify-center"
        role="status"
      >
        <LoaderCircle
          aria-hidden="true"
          className="animate-spin text-secondary-icon motion-reduce:animate-none"
          size={20}
        />
        <span className="sr-only">{t('messageHistoryLoading')}</span>
      </div>
    );
  }

  return (
    <div
      aria-label={t('chats')}
      className="openchat-scroll-area min-h-0 flex-1 overflow-y-auto px-6"
      ref={virtualList.containerRef}
      role="log"
      aria-relevant="additions text"
    >
      <div
        className="mx-auto grid w-full max-w-conversation"
        style={{ paddingBottom: virtualList.bottomPadding, paddingTop: virtualList.topPadding }}
      >
        {virtualList.rows.map(({ index, message }) => {
          const row = displayRows[index];
          const previousMessage = visibleMessages[index - 1] ?? null;
          const canRetry =
            message.role === 'assistant' &&
            message.status !== 'streaming' &&
            index === visibleMessages.length - 1 &&
            visibleMessages[index - 1]?.role === 'user' &&
            row?.responseVersionCount === 1;
          return (
            <article
              className="min-w-0"
              key={message.id}
              ref={virtualList.rowRef(message.id)}
              style={{ paddingBottom: messageGap(previousMessage, message) }}
            >
              {startsNewDay(previousMessage?.createdAt ?? null, message.createdAt) ? (
                <p className="mb-3 mt-1 text-center text-xs text-secondary-text">
                  {formatDate(message.createdAt, locale, t('today'))}
                </p>
              ) : null}
              <MessageItem
                assistantModelFallback={assistantModelFallback}
                assistantProviderFallback={assistantProviderFallback}
                isSavedOutput={savedOutputMessageIds?.has(message.id) ?? false}
                isSavingSavedOutput={pendingSavedOutputMessageIds?.has(message.id) ?? false}
                isSearchMatch={message.id === searchHighlightMessageId}
                message={message}
                latestFileChanges={latestFileChanges}
                onOpenConnections={onOpenConnections}
                onOpenFileChanges={onOpenFileChanges}
                onEditMessage={onEditMessage}
                onRetryMessage={canRetry ? onRetryMessage : undefined}
                onSelectResponseVersion={(selectedMessage, versionIndex) => {
                  selectResponseVersion(selectedMessage, versionIndex);
                }}
                responseVersionCount={row?.responseVersionCount}
                responseVersionIndex={row?.responseVersionIndex}
                onToggleSavedOutput={onToggleSavedOutput}
              />
            </article>
          );
        })}
        {showAssistantLoading && !hasStreamingAssistant ? (
          <div className="min-w-0" style={{ paddingTop: loadingMessageGap(visibleMessages.at(-1) ?? null) }}>
            <AssistantLoadingSkeleton modelLabel={assistantModelFallback} providerId={assistantProviderFallback} />
          </div>
        ) : null}
      </div>
    </div>
  );
}

interface ResponseGroup {
  id: string;
  messages: [ChatMessage, ...ChatMessage[]];
}

function groupResponses(messages: ChatMessage[]): ResponseGroup[] {
  const groups: ResponseGroup[] = [];
  for (const message of messages) {
    const previous = groups.at(-1);
    if (message.role === 'assistant' && previous?.messages.at(-1)?.role === 'assistant') {
      previous.messages.push(message);
      continue;
    }
    groups.push({ id: message.id, messages: [message] });
  }
  return groups;
}

function normalizeResponseIndex(index: number | undefined, length: number): number {
  if (length < 1) return 0;
  return index !== undefined && index >= 0 && index < length ? index : length - 1;
}

function messageGap(previous: ChatMessage | null, current: ChatMessage): number {
  if (previous === null || startsNewDay(previous.createdAt, current.createdAt)) return 0;
  if (previous.role === 'user' && current.role === 'assistant') return 2;
  return 18;
}

function loadingMessageGap(previous: ChatMessage | null): number {
  if (previous === null) return 0;
  return previous.role === 'user' ? 2 : 18;
}

function AssistantLoadingSkeleton({
  modelLabel,
  providerId,
}: {
  modelLabel: string | null;
  providerId: string | null;
}) {
  const { t } = useTranslation();
  const displayModelLabel = modelLabel?.trim() || t('messageModelUnavailable');
  return (
    <div className="max-w-assistant-message" aria-label={t('responseInProgress')} role="status">
      <div className="flex min-h-5 items-center gap-2 text-settings-description font-semibold leading-5 text-text">
        <ProviderIcon className="text-secondary-icon" providerId={providerId ?? 'unknown'} size={18} />
        <span className="min-w-0 truncate">{displayModelLabel}</span>
      </div>
      <div className="mt-5 grid max-w-full gap-2">
        <span className="h-3 w-3/4 animate-pulse rounded bg-selected motion-reduce:animate-none" />
        <span className="h-3 w-11/12 animate-pulse rounded bg-selected motion-reduce:animate-none" />
        <span className="h-3 w-1/2 animate-pulse rounded bg-selected motion-reduce:animate-none" />
      </div>
    </div>
  );
}

function startsNewDay(previous: number | null, current: number | null): boolean {
  if (previous === null || current === null) return previous !== current;
  const previousDate = new Date(previous);
  const currentDate = new Date(current);
  return (
    previousDate.getFullYear() !== currentDate.getFullYear() ||
    previousDate.getMonth() !== currentDate.getMonth() ||
    previousDate.getDate() !== currentDate.getDate()
  );
}

function formatDate(timestamp: number | null, locale: string, todayLabel: string): string {
  if (timestamp === null) return '';
  const date = new Date(timestamp);
  const today = new Date();
  if (
    date.getFullYear() === today.getFullYear() &&
    date.getMonth() === today.getMonth() &&
    date.getDate() === today.getDate()
  ) {
    return todayLabel;
  }
  return new Intl.DateTimeFormat(locale, {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  }).format(date);
}
