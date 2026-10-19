import { Clock3, FileDiff } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog } from '@/design-system';
import { type ChatFileChange, listConversationFileChanges } from '@/modules/conversations/data/chatFileChanges';
import type { ChatMessage } from '@/modules/conversations/data/chatStorage';
import { useChatSession } from '@/modules/conversations/hooks/useChatSession';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { modelRouteKey } from '@/modules/providers/data/modelPreferences';
import { ChatComposer } from './ChatComposer';
import { ChatFileChangesPanel } from './ChatFileChangesPanel';
import { ConversationEmptyState } from './ConversationEmptyState';
import { EditMessageBranchDialog } from './EditMessageBranchDialog';
import { GoalStatusBar } from './GoalStatusBar';
import { MessageList } from './MessageList';
import { parseToolActivities } from './message/messageData';
import { ToolPermissionCard } from './ToolPermissionCard';
import { UserQuestionPanel } from './UserQuestionPanel';

export interface ConversationPaneProps {
  isLargeText: boolean;
  onOpenSettings(): void;
  onRequestTemporaryExit(action: () => void | Promise<void>): void;
  notificationFeedbackKey:
    | 'assistantResponseNotificationReplyEmpty'
    | 'assistantResponseNotificationReplyTooLong'
    | 'assistantResponseNotificationReplyUnavailable'
    | 'assistantResponseNotificationReplyNotSent'
    | 'userQuestionUnavailable'
    | 'notificationDeliveryFailed'
    | 'notificationTestFailed'
    | null;
  questionFocusTarget: { conversationId: string; groupId: string } | null;
  onDismissNotificationFeedback(): void;
  onQuestionFocusHandled(): void;
}

export function ConversationPane({
  isLargeText,
  onOpenSettings,
  onRequestTemporaryExit,
  notificationFeedbackKey,
  questionFocusTarget,
  onDismissNotificationFeedback,
  onQuestionFocusHandled,
}: ConversationPaneProps) {
  const { t } = useTranslation();
  const paneRef = useRef<HTMLElement | null>(null);
  const [paneSize, setPaneSize] = useState(() => ({
    width: Math.max(0, window.innerWidth - 60),
    height: Math.max(0, window.innerHeight - 48),
  }));
  const [disclosureRequest, setDisclosureRequest] = useState<{ providerId: string; providerName: string } | null>(null);
  const [isFileChangesPanelOpen, setIsFileChangesPanelOpen] = useState(false);
  const [fileChangesState, setFileChangesState] = useState<{
    conversationId: string | null;
    changes: ChatFileChange[];
  }>({ conversationId: null, changes: [] });
  const [fileChangesLoadError, setFileChangesLoadError] = useState(false);
  const [fileChangesLoadRevision, setFileChangesLoadRevision] = useState(0);
  const [editingMessage, setEditingMessage] = useState<ChatMessage | null>(null);
  const disclosureResolver = useRef<((approved: boolean) => void) | null>(null);
  const {
    activeConversation,
    activeConversationId,
    errorCode,
    initialize,
    messages,
    serviceStatus,
    isLoadingMessages,
  } = useChatSession();
  const models = useChatStore((state) => state.models);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelError = useChatStore((state) => state.modelError);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const selectedModelKey = useChatStore((state) => state.selectedModelKey);
  const hasConfiguredProvider = useChatStore((state) => state.hasConfiguredProvider);
  const hasCompletedAssistantResponse = useChatStore((state) => state.hasCompletedAssistantResponse);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const selectConversation = useChatStore((state) => state.selectConversation);
  const activeGoal = useChatStore((state) => state.activeGoal);
  const goalStatus = useChatStore((state) => state.goalStatus);
  const refreshActiveGoal = useChatStore((state) => state.refreshActiveGoal);
  const temporaryChat = useChatStore((state) => state.temporaryChat);
  const discloseTemporaryProvider = useChatStore((state) => state.discloseTemporaryProvider);
  const savedOutputMessageKeys = useChatStore((state) => state.savedOutputMessageKeys);
  const pendingSavedOutputMessageIds = useChatStore((state) => state.pendingSavedOutputMessageIds);
  const savedOutputError = useChatStore((state) => state.savedOutputError);
  const refreshSavedOutputMessageKeys = useChatStore((state) => state.refreshSavedOutputMessageKeys);
  const toggleSavedOutput = useChatStore((state) => state.toggleSavedOutput);
  const retryMessage = useChatStore((state) => state.retryMessage);
  const branchFromUserMessage = useChatStore((state) => state.branchFromUserMessage);
  const isCreatingBranch = useChatStore((state) => state.isCreatingMessageBranch);
  const pendingToolPermissionRequest = useChatStore((state) => state.pendingToolPermissionRequest);
  const isRespondingToToolPermission = useChatStore((state) => state.isRespondingToToolPermission);
  const toolPermissionError = useChatStore((state) => state.toolPermissionError);
  const respondToToolPermission = useChatStore((state) => state.respondToToolPermission);
  const isTemporaryChat = temporaryChat !== null;
  const isSending = useChatStore((state) => state.isSending);
  const isEmptyConversation =
    messages.length === 0 &&
    !isLoadingMessages &&
    !isSending &&
    errorCode !== 'messageHistoryLoadFailed' &&
    serviceStatus !== 'error';
  const centerEmptyConversation =
    isEmptyConversation && !isLargeText && paneSize.width >= 640 && paneSize.height >= 640;
  const latestAssistant = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i]?.role === 'assistant') {
        return messages[i];
      }
    }
    return undefined;
  }, [messages]);
  const latestToolActivitiesJson = latestAssistant?.toolActivities ?? '';
  const latestAssistantActivities = useMemo(
    () => parseToolActivities(latestToolActivitiesJson),
    [latestToolActivitiesJson],
  );
  const activityFileChanges = useMemo(
    () => latestAssistantActivities.flatMap((activity) => activity.fileChanges),
    [latestAssistantActivities],
  );
  const conversationFileChanges =
    fileChangesState.conversationId === activeConversationId ? fileChangesState.changes : [];
  const latestFileChanges = useMemo(
    () => mergeFileChanges(activityFileChanges, conversationFileChanges),
    [activityFileChanges, conversationFileChanges],
  );

  useEffect(() => {
    const pane = paneRef.current;
    if (!pane || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const nextSize = {
        width: Math.round(entry.contentRect.width),
        height: Math.round(entry.contentRect.height),
      };
      setPaneSize((current) =>
        current.width === nextSize.width && current.height === nextSize.height ? current : nextSize,
      );
    });
    observer.observe(pane);
    return () => observer.disconnect();
  }, []);
  const visibleModels = useMemo(() => {
    const hiddenSet = new Set(hiddenModelKeys);
    return models.filter((model) => !hiddenSet.has(modelRouteKey(model)));
  }, [models, hiddenModelKeys]);
  const selectedModel = useMemo(
    () => visibleModels.find((model) => model.key === selectedModelKey) ?? null,
    [visibleModels, selectedModelKey],
  );
  const assistantModelFallback =
    selectedModel?.displayName ?? activeConversation?.modelId ?? temporaryChat?.route?.displayName ?? null;
  const assistantProviderFallback =
    selectedModel?.providerId ?? activeConversation?.providerId ?? temporaryChat?.route?.providerId ?? null;
  const modelCatalogStale = selectedModel
    ? selectedModel.catalogStale
    : visibleModels.length > 0 && visibleModels.every((model) => model.catalogStale);
  const isLoadingModels = serviceStatus === 'idle' || serviceStatus === 'loading' || modelStatus === 'loading';
  const temporaryProviderModel = models.find((model) => model.key === selectedModelKey) ?? temporaryChat?.route ?? null;
  const temporaryProviderName =
    temporaryProviderModel?.customProvider?.name ??
    temporaryProviderModel?.sourceLabel ??
    temporaryProviderModel?.providerId ??
    '';
  const savedMessageIds = useMemo(() => {
    const set = new Set<string>();
    for (const key of savedOutputMessageKeys) {
      if (key.conversationId === activeConversationId) {
        set.add(key.messageId);
      }
    }
    return set;
  }, [savedOutputMessageKeys, activeConversationId]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Switching conversations resets transient editor and file-change panel state.
  useEffect(() => {
    setIsFileChangesPanelOpen(false);
    setEditingMessage(null);
  }, [activeConversationId]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: File-change revisions and new tool output are explicit refresh signals.
  useEffect(() => {
    if (activeConversationId === null || isTemporaryChat) {
      setFileChangesState({ conversationId: activeConversationId, changes: [] });
      setFileChangesLoadError(false);
      return;
    }
    let current = true;
    setFileChangesLoadError(false);
    void listConversationFileChanges(activeConversationId)
      .then((changes) => {
        if (current) {
          setFileChangesState({ conversationId: activeConversationId, changes });
          setFileChangesLoadError(false);
        }
      })
      .catch(() => {
        if (current) setFileChangesLoadError(true);
      });
    return () => {
      current = false;
    };
  }, [activeConversationId, fileChangesLoadRevision, isTemporaryChat, latestAssistant?.id, latestToolActivitiesJson]);

  const updateConversationFileChanges = useCallback(
    (conversationId: string, changes: ChatFileChange[]) => {
      if (activeConversationId === conversationId) {
        setFileChangesState({ conversationId, changes });
        setFileChangesLoadError(false);
      }
    },
    [activeConversationId],
  );

  useEffect(
    () => () => {
      disclosureResolver.current?.(false);
      disclosureResolver.current = null;
    },
    [],
  );

  const askBeforeTemporarySend = (providerId: string, providerName: string) => {
    const session = useChatStore.getState().temporaryChat;
    if (!session) return Promise.resolve(false);
    if (session.disclosedProviderIds.includes(providerId)) return Promise.resolve(true);
    return new Promise<boolean>((resolve) => {
      disclosureResolver.current = resolve;
      setDisclosureRequest({ providerId, providerName });
    });
  };

  const settleDisclosure = (approved: boolean) => {
    const request = disclosureRequest;
    const resolve = disclosureResolver.current;
    disclosureResolver.current = null;
    setDisclosureRequest(null);
    if (approved && request) discloseTemporaryProvider(request.providerId);
    resolve?.(approved);
  };

  const submitMessageBranch = async (content: string): Promise<boolean> => {
    if (!editingMessage || !activeConversation) return false;
    return branchFromUserMessage(
      editingMessage.id,
      content,
      t('conversationBranchTitle', { title: activeConversation.title }),
    );
  };

  return (
    <section
      aria-label={t('chats')}
      className="chat-conversation-pane flex min-h-0 min-w-0 flex-1 flex-row bg-background"
      ref={paneRef}
    >
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <div
          className="chat-content-workspace flex min-h-0 min-w-0 flex-1 flex-col"
          data-centered-empty={centerEmptyConversation ? 'true' : undefined}
        >
          <div
            className="chat-content-row flex min-h-0 min-w-0 flex-1"
            data-centered-empty={centerEmptyConversation ? 'true' : undefined}
            data-file-changes-open={isFileChangesPanelOpen && !isTemporaryChat}
          >
            <div className="chat-conversation-content flex min-h-0 min-w-0 flex-1 flex-col">
              {activeConversation && errorCode === 'messageHistoryLoadFailed' && !isLoadingMessages ? (
                <div className="flex min-h-0 flex-1 items-center justify-center px-6 py-8">
                  <div className="max-w-xl text-center">
                    <p className="text-base text-destructive" role="alert">
                      {t('messageHistoryLoadFailed')}
                    </p>
                    <Button
                      className="mt-4"
                      onClick={() => void selectConversation(activeConversation.id)}
                      size="small"
                    >
                      {t('retry')}
                    </Button>
                  </div>
                </div>
              ) : messages.length > 0 || isLoadingMessages || isSending ? (
                <MessageList
                  assistantModelFallback={assistantModelFallback}
                  assistantProviderFallback={assistantProviderFallback}
                  showAssistantLoading={isSending}
                  latestFileChanges={latestFileChanges}
                  onOpenConnections={onOpenSettings}
                  onOpenFileChanges={
                    activeConversationId && !isTemporaryChat ? () => setIsFileChangesPanelOpen(true) : undefined
                  }
                  onEditMessage={!isTemporaryChat ? (message) => setEditingMessage(message) : undefined}
                  onRetryMessage={!isTemporaryChat ? (message) => void retryMessage(message.id) : undefined}
                  onToggleSavedOutput={
                    activeConversationId ? (message) => void toggleSavedOutput(message.id) : undefined
                  }
                  pendingSavedOutputMessageIds={new Set(pendingSavedOutputMessageIds)}
                  savedOutputMessageIds={savedMessageIds}
                />
              ) : serviceStatus === 'error' ? (
                <div className="flex min-h-0 flex-1 items-center justify-center px-6 py-8">
                  <div className="max-w-xl text-center">
                    <p className="text-base text-destructive" role="alert">
                      {t('messageHistoryLoadFailed')}
                    </p>
                    <Button className="mt-4" onClick={() => void initialize()} size="small">
                      {t('retry')}
                    </Button>
                  </div>
                </div>
              ) : (
                <ConversationEmptyState
                  hasAvailableModels={visibleModels.length > 0}
                  hasConfiguredProvider={hasConfiguredProvider}
                  isLoading={isLoadingModels}
                  modelCatalogFailed={modelError && models.length === 0}
                  modelCatalogStale={modelCatalogStale}
                  onOpenConnections={onOpenSettings}
                  onRetryModels={() => void refreshModels(true)}
                  selectedModel={selectedModel !== undefined}
                  showSetupSteps={hasCompletedAssistantResponse === false}
                />
              )}
              {errorCode === 'searchMessageUnavailable' ? (
                <p className="shrink-0 px-6 py-3 text-sm text-destructive" role="alert">
                  {t(errorCode)}
                </p>
              ) : null}
              {savedOutputError ? (
                <div
                  className="mx-auto flex w-full max-w-conversation items-center justify-between gap-3 px-4 py-2 text-sm text-destructive"
                  role="alert"
                >
                  <p>{t('outputSaveFailed')}</p>
                  <Button className="shrink-0" onClick={() => void refreshSavedOutputMessageKeys()} size="small">
                    {t('retry')}
                  </Button>
                </div>
              ) : null}
            </div>
            {isFileChangesPanelOpen && activeConversationId && !isTemporaryChat ? (
              <ChatFileChangesPanel
                conversationId={activeConversationId}
                key={activeConversationId}
                onChangesUpdated={updateConversationFileChanges}
                onClose={() => setIsFileChangesPanelOpen(false)}
                refreshRevision={latestToolActivitiesJson}
              />
            ) : null}
          </div>
          <div className="shrink-0">
            {fileChangesLoadError && !isFileChangesPanelOpen && activeConversationId ? (
              <div
                className="mx-auto flex w-full max-w-composer items-center justify-between gap-3 px-4 pb-2 text-sm text-destructive sm:px-8"
                role="alert"
              >
                <p>{t('fileChangesLoadFailed')}</p>
                <Button
                  className="shrink-0"
                  onClick={() => setFileChangesLoadRevision((revision) => revision + 1)}
                  size="small"
                >
                  {t('retry')}
                </Button>
              </div>
            ) : null}
            {latestFileChanges.length > 0 && activeConversationId && !isTemporaryChat ? (
              <div className="flex justify-center px-4 pb-2">
                <Button
                  className="min-h-11"
                  onClick={() => setIsFileChangesPanelOpen(true)}
                  size="small"
                  variant="secondary"
                >
                  <FileDiff aria-hidden="true" size={16} />
                  <span>{t('fileChangesOpenButton')}</span>
                  {latestFileChanges.some((change) => change.addedLines === null || change.removedLines === null) ? (
                    <span className="text-xs text-secondary-text">{t('fileChangesSomeCountsUnavailable')}</span>
                  ) : (
                    <span className="flex gap-1 text-xs font-semibold">
                      <span className="text-success">
                        +
                        {latestFileChanges
                          .filter((change) => change.status === 'active')
                          .reduce((sum, change) => sum + (change.addedLines ?? 0), 0)}
                      </span>
                      <span className="text-destructive">
                        -
                        {latestFileChanges
                          .filter((change) => change.status === 'active')
                          .reduce((sum, change) => sum + (change.removedLines ?? 0), 0)}
                      </span>
                    </span>
                  )}
                </Button>
              </div>
            ) : null}
            <UserQuestionPanel
              conversationId={activeConversationId}
              onFocusHandled={onQuestionFocusHandled}
              questionFocusTarget={questionFocusTarget}
            />
            {pendingToolPermissionRequest?.conversationId === activeConversationId ? (
              <ToolPermissionCard
                errorKey={toolPermissionError}
                isResponding={isRespondingToToolPermission}
                onRespond={(approved) => void respondToToolPermission(approved)}
                request={pendingToolPermissionRequest}
              />
            ) : null}
            {goalStatus === 'error' && activeConversationId ? (
              <div
                className="mx-auto flex w-full max-w-composer items-center justify-between gap-3 px-4 pb-2 text-sm text-destructive sm:px-8"
                role="alert"
              >
                <p>{t('goalStateUnavailable')}</p>
                <Button className="shrink-0" onClick={() => void refreshActiveGoal(activeConversationId)} size="small">
                  {t('retry')}
                </Button>
              </div>
            ) : null}
            {notificationFeedbackKey ? (
              <div
                className="mx-auto flex w-full max-w-composer items-center justify-between gap-3 px-4 pb-2 text-sm text-destructive sm:px-8"
                role="alert"
              >
                <p>{t(notificationFeedbackKey)}</p>
                <Button
                  aria-label={t('close')}
                  className="shrink-0"
                  onClick={onDismissNotificationFeedback}
                  size="small"
                >
                  {t('close')}
                </Button>
              </div>
            ) : null}
            {activeGoal?.conversationId === activeConversationId ? <GoalStatusBar goal={activeGoal} /> : null}
            <ChatComposer
              isTemporaryChat={temporaryChat !== null}
              onBeforeSend={askBeforeTemporarySend}
              onOpenSettings={onOpenSettings}
              onRequestTemporaryExit={onRequestTemporaryExit}
            />
            {temporaryChat ? (
              <div className="temporary-chat-notice-width mx-auto mb-2 flex max-w-composer flex-wrap items-start gap-3 rounded-card border border-border bg-raised px-3 py-2.5">
                <Clock3 aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={17} />
                <p className="min-w-0 flex-1 basis-60 text-sm text-secondary-text">
                  {temporaryProviderName
                    ? t('temporaryChatNotice', { provider: temporaryProviderName })
                    : t('modelRequired')}
                </p>
                <Button
                  className="shrink-0"
                  onClick={() => onRequestTemporaryExit(() => useChatStore.getState().discardTemporaryChat())}
                  size="small"
                  variant="ghost"
                >
                  {t('temporaryChatEnd')}
                </Button>
              </div>
            ) : null}
          </div>
        </div>
        <Dialog
          onOpenChange={(open) => {
            if (!open) settleDisclosure(false);
          }}
          open={disclosureRequest !== null}
          title={t('temporaryChatDisclosureTitle', { provider: disclosureRequest?.providerName ?? '' })}
        >
          <div className="grid gap-5">
            <p className="text-sm text-secondary-text">
              {t('temporaryChatDisclosureBody', { provider: disclosureRequest?.providerName ?? '' })}
            </p>
            <div className="flex justify-end gap-2">
              <Button onClick={() => settleDisclosure(false)}>{t('cancel')}</Button>
              <Button onClick={() => settleDisclosure(true)} variant="primary">
                {t('continueLabel')}
              </Button>
            </div>
          </div>
        </Dialog>
        <EditMessageBranchDialog
          busy={isCreatingBranch}
          message={editingMessage}
          onOpenChange={(open) => {
            if (!open && !isCreatingBranch) setEditingMessage(null);
          }}
          onSubmit={submitMessageBranch}
          open={editingMessage !== null}
        />
      </div>
    </section>
  );
}

function mergeFileChanges(activityChanges: ChatFileChange[], conversationChanges: ChatFileChange[]): ChatFileChange[] {
  const changesById = new Map(activityChanges.map((change) => [change.id, change]));
  for (const change of conversationChanges) changesById.set(change.id, change);
  return [...changesById.values()];
}
