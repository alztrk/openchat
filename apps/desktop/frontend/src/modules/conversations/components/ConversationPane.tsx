import { ChevronDown, ChevronUp, Clock3, FileDiff, FileUp, MessageSquare, X } from 'lucide-react';
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Toast } from '@/design-system';
import { type ChatFileChange, listConversationFileChanges } from '@/modules/conversations/data/chatFileChanges';
import { type ChatMessage, getMessages } from '@/modules/conversations/data/chatStorage';
import { findInChatMatches, type InChatFilterType } from '@/modules/conversations/data/inChatSearch';
import { getSidebarDraggingConversationId } from '@/modules/conversations/data/sidebarPreferences';
import { useChatSession } from '@/modules/conversations/hooks/useChatSession';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { isSameRoute, upsertMessage } from '@/modules/conversations/stores/chatStoreUtils';
import { modelRouteKey } from '@/modules/providers/data/modelPreferences';
import { isObject } from '@/shared/utils/json';
import { ChatComposer } from './ChatComposer';
import type { RightWorkspaceTab, WorkspaceFileRequest, WorkspaceFileSelection } from './ChatFileChangesPanel';
import { ConversationEmptyState } from './ConversationEmptyState';
import { EditMessageBranchDialog } from './EditMessageBranchDialog';
import { GoalStatusBar } from './GoalStatusBar';
import { InChatSearchBar } from './InChatSearchBar';
import { parseToolActivities } from './message/messageData';
import { ToolPermissionCard } from './ToolPermissionCard';

const MessageList = lazy(() => import('./MessageList').then((module) => ({ default: module.MessageList })));
const ChatFileChangesPanel = lazy(() =>
  import('./ChatFileChangesPanel').then((module) => ({ default: module.ChatFileChangesPanel })),
);

function isChatMessage(value: unknown): value is ChatMessage {
  return isObject(value) && typeof value.id === 'string' && typeof value.conversationId === 'string';
}

export interface ConversationPaneProps {
  conversationId?: string | null;
  header?:
    | React.ReactNode
    | ((controls: { isRightPanelOpen: boolean; toggleRightPanel: () => void }) => React.ReactNode);
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
  onDropConversation?(conversationId: string): void;
  isRightPanelOpen?: boolean;
  onToggleRightPanel?(): void;
  onOpenRightPanel?(tab?: RightWorkspaceTab): void;
  onOpenWorkspaceFile?(file: WorkspaceFileSelection): void;
  hideInternalRightPanel?: boolean;
}

export function ConversationPane({
  conversationId: propConversationId,
  header,
  isLargeText,
  onOpenSettings,
  onRequestTemporaryExit,
  notificationFeedbackKey,
  questionFocusTarget,
  onDismissNotificationFeedback,
  onQuestionFocusHandled,
  onDropConversation,
  isRightPanelOpen: propIsRightPanelOpen,
  onToggleRightPanel: propOnToggleRightPanel,
  onOpenRightPanel,
  onOpenWorkspaceFile,
  hideInternalRightPanel = false,
}: ConversationPaneProps) {
  const { t } = useTranslation();
  const paneRef = useRef<HTMLElement | null>(null);
  const [paneSize, setPaneSize] = useState(() => ({
    width: Math.max(0, window.innerWidth - 60),
    height: Math.max(0, window.innerHeight - 48),
  }));
  const [disclosureRequest, setDisclosureRequest] = useState<{ providerId: string; providerName: string } | null>(null);
  const [isFileChangesPanelOpen, setIsFileChangesPanelOpen] = useState(false);
  const [rightPanelTab, setRightPanelTab] = useState<RightWorkspaceTab>('changes');
  const [workspaceFileRequest, setWorkspaceFileRequest] = useState<WorkspaceFileRequest | null>(null);
  const workspaceFileRequestId = useRef(0);
  const [fileChangesState, setFileChangesState] = useState<{
    conversationId: string | null;
    changes: ChatFileChange[];
  }>({ conversationId: null, changes: [] });
  const [fileChangesLoadError, setFileChangesLoadError] = useState(false);
  const [fileChangesLoadRevision, setFileChangesLoadRevision] = useState(0);
  const [editingMessage, setEditingMessage] = useState<ChatMessage | null>(null);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchFilter, setSearchFilter] = useState<InChatFilterType>('all');
  const [activeMatchIndex, setActiveMatchIndex] = useState(0);
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const [droppedFiles, setDroppedFiles] = useState<{ id: number; files: File[] } | null>(null);
  const dragDepthRef = useRef(0);
  const disclosureResolver = useRef<((approved: boolean) => void) | null>(null);
  const {
    activeConversation: storeActiveConversation,
    activeConversationId: storeActiveConversationId,
    errorCode,
    messages: storeMessages,
    serviceStatus,
    isLoadingMessages: storeIsLoadingMessages,
  } = useChatSession();
  const conversations = useChatStore((state) => state.conversations);
  const chatGptWorkspaceSelectionRequired = useChatStore((state) => state.chatGptWorkspaceSelectionRequired);

  const isConversationOverride = propConversationId !== undefined;
  const isPlaceholder = isConversationOverride && !propConversationId;
  const resolvedTargetId = isPlaceholder ? null : (propConversationId ?? storeActiveConversationId);
  const isStoreActive = resolvedTargetId !== null && resolvedTargetId === storeActiveConversationId;

  const [secondaryMessages, setSecondaryMessages] = useState<ChatMessage[]>([]);
  const [secondaryLoading, setSecondaryLoading] = useState(false);
  const [secondaryLoadError, setSecondaryLoadError] = useState(false);
  const [secondaryLoadRevision, setSecondaryLoadRevision] = useState(0);

  // biome-ignore lint/correctness/useExhaustiveDependencies: The revision increments on retry to reload this conversation's history.
  useEffect(() => {
    if (!isConversationOverride || isStoreActive) return;
    if (!resolvedTargetId) {
      setSecondaryMessages([]);
      setSecondaryLoading(false);
      setSecondaryLoadError(false);
      return;
    }
    let current = true;
    setSecondaryLoading(true);
    setSecondaryLoadError(false);
    void getMessages(resolvedTargetId)
      .then((items) => {
        if (current) setSecondaryMessages(items);
      })
      .catch((error) => {
        if (current) {
          setSecondaryLoadError(true);
          globalThis.reportError(new Error('Failed to load panel conversation messages.', { cause: error }));
        }
      })
      .finally(() => {
        if (current) setSecondaryLoading(false);
      });

    const onStream = (event: Event) => {
      if (!(event instanceof CustomEvent) || !isObject(event.detail) || !isChatMessage(event.detail.message)) return;
      const { message } = event.detail;
      if (message.conversationId === resolvedTargetId) {
        setSecondaryMessages((prev) => upsertMessage(prev, message));
      }
    };
    window.addEventListener('openchat-message-stream', onStream);
    return () => {
      current = false;
      window.removeEventListener('openchat-message-stream', onStream);
    };
  }, [isConversationOverride, isStoreActive, resolvedTargetId, secondaryLoadRevision]);

  const activeConversationId = isConversationOverride ? resolvedTargetId : storeActiveConversationId;
  const activeConversation = useMemo(() => {
    if (isConversationOverride) {
      return resolvedTargetId ? (conversations.find((c) => c.id === resolvedTargetId) ?? null) : null;
    }
    return storeActiveConversation;
  }, [conversations, isConversationOverride, resolvedTargetId, storeActiveConversation]);

  const messages = isConversationOverride && !isStoreActive ? secondaryMessages : storeMessages;
  const isLoadingMessages = isConversationOverride && !isStoreActive ? secondaryLoading : storeIsLoadingMessages;
  const models = useChatStore((state) => state.models);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelError = useChatStore((state) => state.modelError);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const selectedModelKey = useChatStore((state) => state.selectedModelKey);
  const hasConfiguredProvider = useChatStore((state) => state.hasConfiguredProvider);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const activeGoal = useChatStore((state) => state.activeGoal);
  const goalStatus = useChatStore((state) => state.goalStatus);
  const refreshActiveGoal = useChatStore((state) => state.refreshActiveGoal);
  const temporaryChat = useChatStore((state) => (isConversationOverride ? null : state.temporaryChat));
  const discloseTemporaryProvider = useChatStore((state) => state.discloseTemporaryProvider);
  const retryMessage = useChatStore((state) => state.retryMessage);
  const branchFromUserMessage = useChatStore((state) => state.branchFromUserMessage);
  const isCreatingBranch = useChatStore((state) => state.isCreatingMessageBranch);
  const pendingToolPermissionRequest = useChatStore((state) => state.pendingToolPermissionRequest);
  const isRespondingToToolPermission = useChatStore((state) => state.isRespondingToToolPermission);
  const toolPermissionError = useChatStore((state) => state.toolPermissionError);
  const respondToToolPermission = useChatStore((state) => state.respondToToolPermission);
  const searchHighlightQuery = useChatStore((state) => state.searchHighlightQuery);
  const searchMatchIndex = useChatStore((state) => state.searchMatchIndex);
  const searchMatchCount = useChatStore((state) => state.searchMatchCount);
  const selectConversation = useChatStore((state) => state.selectConversation);
  const nextSearchMatch = useChatStore((state) => state.nextSearchMatch);
  const prevSearchMatch = useChatStore((state) => state.prevSearchMatch);
  const clearSearchHighlight = useChatStore((state) => state.clearSearchHighlight);
  const isTemporaryChat = temporaryChat !== null;
  const isSendingInStore = useChatStore((state) => state.isSending);
  const sendingConversationId = useChatStore((state) => state.sendingConversationId);
  const isSending = isConversationOverride
    ? isSendingInStore && sendingConversationId === activeConversationId
    : isSendingInStore;
  const isEmptyConversation = messages.length === 0 && !isLoadingMessages && !isSending;
  const centerEmptyConversation =
    isEmptyConversation && !isLargeText && paneSize.width >= 640 && paneSize.height >= 640;
  const [historyErrorToastVisible, setHistoryErrorToastVisible] = useState(false);

  useEffect(() => {
    if (errorCode === 'messageHistoryLoadFailed') {
      setHistoryErrorToastVisible(true);
      return;
    }
    setHistoryErrorToastVisible(false);
  }, [errorCode]);

  const handleDismissHistoryErrorToast = useCallback(() => {
    setHistoryErrorToastVisible(false);
    if (useChatStore.getState().errorCode === 'messageHistoryLoadFailed') {
      useChatStore.setState({ errorCode: null });
    }
  }, []);
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
    () =>
      isConversationOverride
        ? activeConversation
          ? (models.find((model) => isSameRoute(model, activeConversation)) ?? null)
          : null
        : (visibleModels.find((model) => model.key === selectedModelKey) ?? null),
    [activeConversation, isConversationOverride, models, selectedModelKey, visibleModels],
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
  // biome-ignore lint/correctness/useExhaustiveDependencies: Switching conversations resets transient editor, search, and file-change panel state.
  useEffect(() => {
    setIsFileChangesPanelOpen(false);
    setWorkspaceFileRequest(null);
    setEditingMessage(null);
    setIsSearchOpen(false);
    setSearchQuery('');
    setSearchFilter('all');
    setActiveMatchIndex(0);
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

  const openWorkspaceFile = useCallback(
    (file: WorkspaceFileSelection) => {
      if (onOpenWorkspaceFile) {
        onOpenWorkspaceFile(file);
        return;
      }
      setRightPanelTab('changes');
      setWorkspaceFileRequest({ ...file, requestId: ++workspaceFileRequestId.current });
      setIsFileChangesPanelOpen(true);
    },
    [onOpenWorkspaceFile],
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

  const inChatMatches = useMemo(
    () => (isSearchOpen ? findInChatMatches(messages, searchQuery, searchFilter) : []),
    [isSearchOpen, messages, searchQuery, searchFilter],
  );

  const clampedMatchIndex = inChatMatches.length > 0 ? Math.min(activeMatchIndex, inChatMatches.length - 1) : 0;
  const activeMatch = inChatMatches[clampedMatchIndex] ?? null;

  const handleNextMatch = useCallback(() => {
    setActiveMatchIndex((idx) => (inChatMatches.length > 0 ? (idx + 1) % inChatMatches.length : 0));
  }, [inChatMatches.length]);

  const handlePreviousMatch = useCallback(() => {
    setActiveMatchIndex((idx) =>
      inChatMatches.length > 0 ? (idx - 1 + inChatMatches.length) % inChatMatches.length : 0,
    );
  }, [inChatMatches.length]);

  const handleCloseSearch = useCallback(() => {
    setIsSearchOpen(false);
    setSearchQuery('');
    setSearchFilter('all');
    setActiveMatchIndex(0);
  }, []);

  const handleOpenSearch = useCallback(() => {
    const selection = window.getSelection()?.toString().trim();
    if (selection) {
      setSearchQuery(selection);
    }
    setIsSearchOpen(true);
    setActiveMatchIndex(0);
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') {
        if (document.querySelector('[role="dialog"]')) return;
        const active = document.activeElement;
        const isPaneFocused =
          paneRef.current?.contains(active) ||
          active === document.body ||
          active?.tagName === 'BODY' ||
          active === null;
        if (!isPaneFocused) return;
        if (messages.length === 0) return;

        event.preventDefault();
        handleOpenSearch();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleOpenSearch, messages.length]);

  const isConversationDrag = (event: React.DragEvent) => {
    if (getSidebarDraggingConversationId() !== null) return true;
    const types = Array.from(event.dataTransfer?.types ?? []);
    return types.includes('application/x-openchat-conversation') || types.includes('text/plain');
  };

  const handleDragEnter = (event: React.DragEvent) => {
    const isFiles = Array.from(event.dataTransfer?.types ?? []).includes('Files');
    const isConv = isConversationDrag(event);
    if (!isFiles && !isConv) return;
    event.preventDefault();
    if (isFiles) {
      dragDepthRef.current += 1;
      setIsDraggingOver(true);
    }
  };

  const handleDragOver = (event: React.DragEvent) => {
    const isFiles = Array.from(event.dataTransfer?.types ?? []).includes('Files');
    const isConv = isConversationDrag(event);
    if (!isFiles && !isConv) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  };

  const handleDragLeave = (event: React.DragEvent) => {
    const isFiles = Array.from(event.dataTransfer?.types ?? []).includes('Files');
    if (!isFiles) return;
    event.preventDefault();
    dragDepthRef.current -= 1;
    if (dragDepthRef.current <= 0) {
      dragDepthRef.current = 0;
      setIsDraggingOver(false);
    }
  };

  const handleDrop = (event: React.DragEvent) => {
    const isFiles = Array.from(event.dataTransfer?.types ?? []).includes('Files');
    if (isFiles) {
      event.preventDefault();
      dragDepthRef.current = 0;
      setIsDraggingOver(false);
      const files = Array.from(event.dataTransfer.files ?? []);
      if (files.length > 0) {
        setDroppedFiles({ id: Date.now(), files });
      }
      return;
    }
    if (isConversationDrag(event)) {
      event.preventDefault();
      const customData = event.dataTransfer.getData('application/x-openchat-conversation');
      const inMemoryId = getSidebarDraggingConversationId();
      const plainText = event.dataTransfer.getData('text/plain');
      const draggedId = customData || inMemoryId || plainText;
      if (draggedId && onDropConversation) {
        onDropConversation(draggedId);
      }
    }
  };

  useEffect(() => {
    const handleWindowDragEnd = () => {
      dragDepthRef.current = 0;
      setIsDraggingOver(false);
    };
    window.addEventListener('dragend', handleWindowDragEnd);
    window.addEventListener('drop', handleWindowDragEnd);
    return () => {
      window.removeEventListener('dragend', handleWindowDragEnd);
      window.removeEventListener('drop', handleWindowDragEnd);
    };
  }, []);

  return (
    <section
      aria-label={t('chats')}
      className="chat-conversation-pane relative flex h-full min-h-0 min-w-0 flex-1 flex-row overflow-hidden bg-background"
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      ref={paneRef}
    >
      {historyErrorToastVisible ? (
        <div className="fixed top-14 right-6 z-50 max-w-md shadow-lg">
          <Toast
            action={
              !isConversationOverride && activeConversationId
                ? { label: t('retry'), onClick: () => void selectConversation(activeConversationId) }
                : undefined
            }
            message={t('messageHistoryLoadFailed')}
            onClose={handleDismissHistoryErrorToast}
            variant="destructive"
          />
        </div>
      ) : null}
      <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        {typeof header === 'function'
          ? header({
              isRightPanelOpen: propIsRightPanelOpen !== undefined ? propIsRightPanelOpen : isFileChangesPanelOpen,
              toggleRightPanel: () => {
                if (propOnToggleRightPanel) {
                  propOnToggleRightPanel();
                  return;
                }
                setIsFileChangesPanelOpen((open) => {
                  if (!open && latestFileChanges.length === 0) {
                    setRightPanelTab('browser');
                  }
                  return !open;
                });
              },
            })
          : header}
        <div
          className="chat-content-workspace flex min-h-0 min-w-0 flex-1 flex-col"
          data-centered-empty={centerEmptyConversation ? 'true' : undefined}
        >
          <div
            className="chat-content-row flex min-h-0 min-w-0 flex-1"
            data-centered-empty={centerEmptyConversation ? 'true' : undefined}
            data-file-changes-open={
              (propIsRightPanelOpen !== undefined ? propIsRightPanelOpen : isFileChangesPanelOpen) && !isTemporaryChat
            }
          >
            <div className="chat-conversation-content relative flex min-h-0 min-w-0 flex-1 flex-col">
              {searchHighlightQuery && !isSearchOpen ? (
                <div className="openchat-glass-popover absolute top-3 left-1/2 -translate-x-1/2 z-20 flex items-center gap-2 rounded-full border border-border/80 px-3 py-1 animate-in fade-in slide-in-from-top-2 duration-150">
                  <span className="max-w-search-highlight truncate text-xs font-medium text-text">
                    "{searchHighlightQuery}"
                  </span>
                  <span className="text-caption text-secondary-text">
                    {searchMatchCount > 0
                      ? `${searchMatchIndex + 1} / ${searchMatchCount}`
                      : t('inChatSearchNoMatches')}
                  </span>
                  <div className="flex items-center gap-0.5">
                    <button
                      aria-label={t('inChatPreviousMatch')}
                      className="rounded-full p-1 text-secondary-icon transition-colors hover:bg-hover hover:text-text disabled:opacity-40"
                      disabled={searchMatchCount <= 1}
                      onClick={prevSearchMatch}
                      type="button"
                    >
                      <ChevronUp size={14} />
                    </button>
                    <button
                      aria-label={t('inChatNextMatch')}
                      className="rounded-full p-1 text-secondary-icon transition-colors hover:bg-hover hover:text-text disabled:opacity-40"
                      disabled={searchMatchCount <= 1}
                      onClick={nextSearchMatch}
                      type="button"
                    >
                      <ChevronDown size={14} />
                    </button>
                    <button
                      aria-label={t('inChatCloseSearch')}
                      className="ml-1 rounded-full p-1 text-secondary-icon transition-colors hover:bg-hover hover:text-text"
                      onClick={clearSearchHighlight}
                      type="button"
                    >
                      <X size={13} />
                    </button>
                  </div>
                </div>
              ) : null}
              {isSearchOpen ? (
                <InChatSearchBar
                  activeMatchIndex={clampedMatchIndex}
                  filter={searchFilter}
                  matches={inChatMatches}
                  onClose={handleCloseSearch}
                  onFilterChange={(nextFilter) => {
                    setSearchFilter(nextFilter);
                    setActiveMatchIndex(0);
                  }}
                  onNext={handleNextMatch}
                  onPrevious={handlePreviousMatch}
                  onQueryChange={(nextQuery) => {
                    setSearchQuery(nextQuery);
                    setActiveMatchIndex(0);
                  }}
                  query={searchQuery}
                />
              ) : null}
              {isPlaceholder ? (
                <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center text-secondary-text">
                  <MessageSquare aria-hidden="true" className="opacity-45" size={32} />
                  <p className="text-sm font-medium text-text">{t('conversationUnavailable')}</p>
                </div>
              ) : isConversationOverride && !isStoreActive && secondaryLoadError ? (
                <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center" role="alert">
                  <p className="text-sm text-destructive">{t('messageHistoryLoadFailed')}</p>
                  <Button onClick={() => setSecondaryLoadRevision((revision) => revision + 1)} size="small">
                    {t('retry')}
                  </Button>
                </div>
              ) : messages.length > 0 || isLoadingMessages || isSending ? (
                <Suspense fallback={null}>
                  <MessageList
                    assistantModelFallback={assistantModelFallback}
                    assistantProviderFallback={assistantProviderFallback}
                    conversationIdOverride={activeConversationId ?? undefined}
                    inChatSearchQuery={isSearchOpen ? searchQuery : ''}
                    inChatTargetMessageId={isSearchOpen ? (activeMatch?.messageId ?? null) : null}
                    isLoadingMessagesOverride={isConversationOverride && !isStoreActive ? secondaryLoading : undefined}
                    latestFileChanges={latestFileChanges}
                    messagesOverride={isConversationOverride && !isStoreActive ? secondaryMessages : undefined}
                    onEditMessage={!isTemporaryChat ? (message) => setEditingMessage(message) : undefined}
                    onOpenConnections={onOpenSettings}
                    onOpenFileChanges={
                      activeConversationId && !isTemporaryChat
                        ? () => {
                            if (onOpenRightPanel) {
                              onOpenRightPanel('changes');
                            } else {
                              setRightPanelTab('changes');
                              setIsFileChangesPanelOpen(true);
                            }
                          }
                        : undefined
                    }
                    onOpenWorkspaceFile={openWorkspaceFile}
                    onRetryMessage={!isTemporaryChat ? (message) => void retryMessage(message.id) : undefined}
                    showAssistantLoading={isSending}
                  />
                </Suspense>
              ) : (
                <ConversationEmptyState
                  hasAvailableModels={visibleModels.length > 0}
                  hasConfiguredProvider={hasConfiguredProvider}
                  isLoading={isLoadingModels}
                  modelCatalogFailed={modelError && models.length === 0}
                  modelCatalogStale={modelCatalogStale}
                  chatGptWorkspaceSelectionRequired={chatGptWorkspaceSelectionRequired}
                  onOpenConnections={onOpenSettings}
                  onRetryModels={() => void refreshModels(true)}
                  selectedModel={selectedModel !== undefined}
                />
              )}
              {errorCode === 'searchMessageUnavailable' ? (
                <p className="shrink-0 px-6 py-3 text-sm text-destructive" role="alert">
                  {t(errorCode)}
                </p>
              ) : null}
            </div>
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
            {activeConversationId && !isTemporaryChat && latestFileChanges.length > 0 ? (
              <div className="flex flex-wrap justify-center gap-2 px-4 pb-2">
                <Button
                  className="min-h-9"
                  onClick={() => {
                    setRightPanelTab('changes');
                    setIsFileChangesPanelOpen(true);
                  }}
                  size="small"
                  variant="secondary"
                >
                  <FileDiff aria-hidden="true" size={15} />
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
              droppedFiles={droppedFiles}
              isLoadingMessagesOverride={isLoadingMessages}
              isTemporaryChat={temporaryChat !== null}
              messagesOverride={messages}
              onBeforeSend={askBeforeTemporarySend}
              onOpenSettings={onOpenSettings}
              onQuestionFocusHandled={onQuestionFocusHandled}
              onRequestTemporaryExit={onRequestTemporaryExit}
              questionFocusTarget={questionFocusTarget}
              targetConversationId={isPlaceholder ? null : isConversationOverride ? activeConversationId : undefined}
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
      {!hideInternalRightPanel ? (
        <div
          aria-hidden={!isFileChangesPanelOpen || undefined}
          className="chat-right-panel-shell"
          data-open={isFileChangesPanelOpen ? 'true' : 'false'}
          inert={!isFileChangesPanelOpen}
        >
          {isFileChangesPanelOpen || workspaceFileRequest !== null ? (
            <Suspense fallback={null}>
              <ChatFileChangesPanel
                conversationId={activeConversationId}
                initialTab={rightPanelTab}
                fileRequest={workspaceFileRequest}
                key={activeConversationId ?? 'workspace-panel'}
                onChangesUpdated={updateConversationFileChanges}
                refreshRevision={latestToolActivitiesJson}
              />
            </Suspense>
          ) : null}
        </div>
      ) : null}
      {isDraggingOver ? (
        <section
          aria-label={t('dropFilesOverlayTitle')}
          className="pointer-events-none absolute inset-0 z-50 flex items-center justify-center bg-background/80 p-6 backdrop-blur-sm"
        >
          <div className="flex max-w-md flex-col items-center justify-center gap-3 rounded-card border-2 border-dashed border-accent bg-raised px-8 py-10 text-center shadow-elevation-3">
            <div className="flex size-14 items-center justify-center rounded-full bg-accent/10 text-accent">
              <FileUp aria-hidden="true" size={28} />
            </div>
            <div className="space-y-1">
              <p className="text-base font-semibold text-text">{t('dropFilesOverlayTitle')}</p>
              <p className="text-sm text-secondary-text">{t('dropFilesOverlayDescription')}</p>
            </div>
          </div>
        </section>
      ) : null}
    </section>
  );
}

function mergeFileChanges(activityChanges: ChatFileChange[], conversationChanges: ChatFileChange[]): ChatFileChange[] {
  const changesById = new Map(activityChanges.map((change) => [change.id, change]));
  for (const change of conversationChanges) changesById.set(change.id, change);
  return [...changesById.values()];
}
