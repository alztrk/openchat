import { LoaderCircle } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { ChatFileChange } from '@features/chat/data/chatFileChanges';
import { startsNewDay } from '@features/chat/data/messageGrouping';
import { type ChatMessage, getActiveBranchMessages, getMessageSiblings } from '@features/chat/data';
import { useInChatSearchHighlight } from '@features/chat/hooks/useInChatSearchHighlight';
import { useVirtualMessageList } from '@features/chat/hooks/useVirtualMessageList';
import { useChatStore } from '@features/chat/state';
import { Button, ProviderIcon } from '@shared/components';
import { useChatDisplayPreferencesStore } from '@shared/lib/preferences/chatDisplayPreferences';
import type { WorkspaceFileSelection } from './ChatFileChangesPanel';
import { MessageItem } from './MessageItem';
import { parseToolActivities } from './message/messageData';

export interface MessageListProps {
  assistantModelFallback?: string | null;
  assistantProviderFallback?: string | null;
  showAssistantLoading?: boolean;
  onRetryMessage?: (message: ChatMessage) => void;
  onEditMessage?: (message: ChatMessage) => void;
  onSelectResponseVersion?: (message: ChatMessage, versionIndex: number) => void;
  latestFileChanges?: ChatFileChange[];
  onOpenFileChanges?: () => void;
  onOpenWorkspaceFile?: (file: WorkspaceFileSelection) => void;
  onOpenConnections?: () => void;
  onCheckpointRestored?: () => void;
  inChatTargetMessageId?: string | null;
  inChatSearchQuery?: string;
  messagesOverride?: ChatMessage[];
  conversationIdOverride?: string | null;
  isLoadingMessagesOverride?: boolean;
}

export function MessageList({
  assistantModelFallback = null,
  assistantProviderFallback = null,
  showAssistantLoading = false,
  onEditMessage,
  onRetryMessage,
  onSelectResponseVersion,
  latestFileChanges,
  onOpenFileChanges,
  onOpenWorkspaceFile,
  onOpenConnections,
  onCheckpointRestored,
  inChatTargetMessageId = null,
  inChatSearchQuery = '',
  messagesOverride,
  conversationIdOverride,
  isLoadingMessagesOverride,
}: MessageListProps) {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const storeMessages = useChatStore((state) => state.messages);
  const storeIsLoadingMessages = useChatStore((state) => state.isLoadingMessages);
  const olderMessagesCursor = useChatStore((state) => state.olderMessagesCursor);
  const isLoadingOlderMessages = useChatStore((state) => state.isLoadingOlderMessages);
  const olderMessagesLoadError = useChatStore((state) => state.olderMessagesLoadError);
  const loadOlderMessages = useChatStore((state) => state.loadOlderMessages);
  const storeSearchTargetMessageId = useChatStore((state) => state.searchTargetMessageId);
  const storeSearchHighlightMessageId = useChatStore((state) => state.searchHighlightMessageId);
  const storeSearchHighlightQuery = useChatStore((state) => state.searchHighlightQuery);
  const clearSearchTarget = useChatStore((state) => state.clearSearchTarget);
  const storeActiveConversationId = useChatStore((state) => state.activeConversationId);
  const temporaryChatId = useChatStore((state) => state.temporaryChat?.id ?? null);
  const providerCompactionConversationIds = useChatStore((state) => state.providerCompactionConversationIds);
  const isOverridden = messagesOverride !== undefined;
  const searchTargetMessageId = isOverridden ? null : storeSearchTargetMessageId;
  const searchHighlightMessageId = isOverridden ? null : storeSearchHighlightMessageId;
  const searchHighlightQuery = isOverridden ? '' : storeSearchHighlightQuery;
  const messages = messagesOverride ?? storeMessages;
  const isLoadingMessages = isLoadingMessagesOverride ?? storeIsLoadingMessages;
  const activeConversationId =
    conversationIdOverride !== undefined
      ? conversationIdOverride
      : (storeActiveConversationId ?? (isOverridden ? null : temporaryChatId));
  const canLoadOlderMessages =
    !isOverridden && activeConversationId === storeActiveConversationId && olderMessagesCursor !== null;
  const skipBottomScroll = useRef(false);
  const skippedCompletedResponseScroll = useRef(false);
  const activeConversation = useChatStore((state) =>
    state.conversations.find((c) => c.id === activeConversationId),
  );
  const switchMessageBranch = useChatStore((state) => state.switchMessageBranch);

  const activeBranchMessages = useMemo(
    () => getActiveBranchMessages(messages, activeConversation?.activeLeafMessageId),
    [messages, activeConversation?.activeLeafMessageId],
  );

  const displayRows = useMemo(
    () =>
      activeBranchMessages.map((message) => {
        const siblingInfo = getMessageSiblings(messages, message.id);
        return {
          message,
          responseVersionCount: siblingInfo.count,
          responseVersionIndex: siblingInfo.index,
          siblings: siblingInfo.siblings,
        };
      }),
    [activeBranchMessages, messages],
  );

  const visibleMessages = useMemo(() => displayRows.map((row) => row.message), [displayRows]);
  const hasStreamingAssistant =
    visibleMessages.at(-1)?.role === 'assistant' && visibleMessages.at(-1)?.status === 'streaming';
  const latestMessage = visibleMessages.at(-1);
  const latestToolActivities = useMemo(
    () => (latestMessage?.role === 'assistant' ? parseToolActivities(latestMessage.toolActivities) : []),
    [latestMessage?.toolActivities, latestMessage?.role],
  );
  const isWaitingForUserAnswer = latestToolActivities.some(
    (activity) => activity.name === 'ask_user' && activity.status === 'waitingForUser',
  );
  const isCompactingConversation =
    activeConversationId !== null && providerCompactionConversationIds.includes(activeConversationId);
  const compactionInsertionIndex = hasStreamingAssistant
    ? Math.max(visibleMessages.length - 1, 0)
    : visibleMessages.length;
  const compactionStatus = isCompactingConversation ? <CompactionStatusDivider label={t('chatCompacting')} /> : null;
  const shouldShowAssistantLoading =
    showAssistantLoading && !hasStreamingAssistant && !isWaitingForUserAnswer && !isCompactingConversation;
  const virtualList = useVirtualMessageList(visibleMessages, activeConversationId);
  const autoFollowResponses = useChatDisplayPreferencesStore((state) => state.preferences.autoFollowResponses);

  const effectiveHighlightQuery = inChatSearchQuery || searchHighlightQuery || '';
  const effectiveTargetId = inChatTargetMessageId || searchHighlightMessageId || searchTargetMessageId;

  useInChatSearchHighlight(virtualList.containerRef, effectiveHighlightQuery, effectiveTargetId, virtualList.rows);

  const selectResponseVersion = useCallback(
    (message: ChatMessage, versionIndex: number) => {
      const siblingInfo = getMessageSiblings(messages, message.id);
      if (versionIndex < 0 || versionIndex >= siblingInfo.siblings.length) return;
      const targetSibling = siblingInfo.siblings[versionIndex];
      if (!targetSibling) return;
      void switchMessageBranch(targetSibling.id);
      onSelectResponseVersion?.(targetSibling, versionIndex);
    },
    [messages, switchMessageBranch, onSelectResponseVersion],
  );

  useEffect(() => {
    if (isLoadingMessages || !searchTargetMessageId) return;
    if (!activeBranchMessages.some((m) => m.id === searchTargetMessageId)) {
      const exists = messages.some((m) => m.id === searchTargetMessageId);
      if (exists) {
        void switchMessageBranch(searchTargetMessageId);
      }
    }
  }, [activeBranchMessages, isLoadingMessages, messages, searchTargetMessageId, switchMessageBranch]);

  useEffect(() => {
    if (isLoadingMessages || !inChatTargetMessageId) return;
    if (!activeBranchMessages.some((m) => m.id === inChatTargetMessageId)) {
      const exists = messages.some((m) => m.id === inChatTargetMessageId);
      if (exists) {
        void switchMessageBranch(inChatTargetMessageId);
      }
    }
  }, [activeBranchMessages, inChatTargetMessageId, isLoadingMessages, messages, switchMessageBranch]);

  useEffect(() => {
    if (isLoadingMessages || !searchTargetMessageId) return;
    if (!virtualList.rows.some(({ message }) => message.id === searchTargetMessageId)) {
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
    searchTargetMessageId,
    virtualList.rows,
    virtualList.scrollToMessage,
    virtualList.stopFollowingLatest,
  ]);

  useEffect(() => {
    if (isLoadingMessages || !inChatTargetMessageId) return;
    if (!virtualList.rows.some(({ message }) => message.id === inChatTargetMessageId)) {
      virtualList.scrollToMessage(inChatTargetMessageId);
      return;
    }
    const target = document.getElementById(`openchat-message-${inChatTargetMessageId}`);
    if (!target) return;
    virtualList.stopFollowingLatest();
    skipBottomScroll.current = true;
    target.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }, [
    inChatTargetMessageId,
    isLoadingMessages,
    virtualList.rows,
    virtualList.scrollToMessage,
    virtualList.stopFollowingLatest,
  ]);

  // NOTE: We use refs to avoid re-triggering on every streaming content chunk update.
  // Only the message count change (new messages) needs to trigger scroll-to-bottom.
  const latestMessageContentRef = useRef<string | null>(null);
  const latestMessageStatusRef = useRef<string | null>(null);
  latestMessageContentRef.current = latestMessage?.content ?? null;
  latestMessageStatusRef.current = latestMessage?.status ?? null;

  // biome-ignore lint/correctness/useExhaustiveDependencies: Message, loading, and conversation changes intentionally trigger scroll-follow updates.
  useEffect(() => {
    if (isLoadingMessages || searchTargetMessageId || inChatTargetMessageId) return;
    if (skipBottomScroll.current) {
      skipBottomScroll.current = false;
      return;
    }
    if (hasStreamingAssistant) {
      if (!autoFollowResponses) {
        skippedCompletedResponseScroll.current = true;
        return;
      }
      skippedCompletedResponseScroll.current = false;
    } else if (skippedCompletedResponseScroll.current) {
      skippedCompletedResponseScroll.current = false;
      return;
    }
    if (!virtualList.isFollowingLatest()) return;
    virtualList.scrollToBottom();
  }, [
    activeConversationId,
    autoFollowResponses,
    isCompactingConversation,
    hasStreamingAssistant,
    inChatTargetMessageId,
    isLoadingMessages,
    messages.length,
    latestMessageContentRef,
    latestMessageStatusRef,
    searchTargetMessageId,
    shouldShowAssistantLoading,
    virtualList.isFollowingLatest,
    virtualList.scrollToBottom,
    virtualList.totalHeight,
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
      {canLoadOlderMessages ? (
        <div className="mx-auto flex w-full max-w-conversation flex-col items-center gap-2 py-3">
          {olderMessagesLoadError ? (
            <p className="text-sm text-destructive" role="alert">
              {t('messageHistoryOlderLoadFailed')}
            </p>
          ) : null}
          <Button
            loading={isLoadingOlderMessages}
            loadingContent={t('messageHistoryLoadingOlder')}
            onClick={() => void loadOlderMessages()}
            size="small"
            variant="secondary"
          >
            {olderMessagesLoadError ? t('messageHistoryRetryOlder') : t('messageHistoryLoadOlder')}
          </Button>
        </div>
      ) : null}
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
            visibleMessages[index - 1]?.role === 'user';
          return (
            <div className="min-w-0" key={message.id}>
              {isCompactingConversation && index === compactionInsertionIndex ? compactionStatus : null}
              <article
                className="min-w-0"
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
                  isSearchMatch={
                    message.id === (inChatTargetMessageId ?? searchHighlightMessageId ?? searchTargetMessageId)
                  }
                  message={message}
                  latestFileChanges={latestFileChanges}
                  onOpenConnections={onOpenConnections}
                  onCheckpointRestored={onCheckpointRestored}
                  onOpenFileChanges={onOpenFileChanges}
                  onOpenWorkspaceFile={onOpenWorkspaceFile}
                  onEditMessage={onEditMessage}
                  onRetryMessage={canRetry ? onRetryMessage : undefined}
                  onSelectResponseVersion={selectResponseVersion}
                  responseVersionCount={row?.responseVersionCount}
                  responseVersionIndex={row?.responseVersionIndex}
                />
              </article>
            </div>
          );
        })}
        {isCompactingConversation && compactionInsertionIndex === visibleMessages.length
          ? compactionStatus
          : null}
        {shouldShowAssistantLoading ? (
          <div className="min-w-0" style={{ paddingTop: loadingMessageGap(visibleMessages.at(-1) ?? null) }}>
            <AssistantLoadingSkeleton modelLabel={assistantModelFallback} providerId={assistantProviderFallback} />
          </div>
        ) : null}
      </div>
    </div>
  );
}

function CompactionStatusDivider({ label }: { label: string }) {
  return (
    <div aria-live="polite" className="my-5 flex min-h-6 items-center gap-3 text-xs text-secondary-text" role="status">
      <span aria-hidden="true" className="h-px flex-1 bg-border" />
      <span className="inline-flex shrink-0 items-center gap-2">
        <LoaderCircle
          aria-hidden="true"
          className="animate-spin text-secondary-icon motion-reduce:animate-none"
          size={16}
        />
        {label}
      </span>
      <span aria-hidden="true" className="h-px flex-1 bg-border" />
    </div>
  );
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
