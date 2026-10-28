import { type FormEvent, type KeyboardEvent, type RefObject, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  type ChatAttachmentErrorCode,
  ChatAttachmentInputError,
  maximumAttachmentBytes,
  maximumAttachmentFileCount,
  type PendingChatAttachment,
  readAttachmentFile,
  restoreStoredAttachment,
  validateAttachmentSelection,
} from '@/modules/conversations/data/chatAttachments';
import { readChatGptFastMode, writeChatGptFastMode } from '@/modules/conversations/data/chatGptFastMode';
import type {
  ChatMessage,
  ChatModelFavorite,
  ConversationAttachmentSummary,
} from '@/modules/conversations/data/chatStorage';
import { readMessageAttachment, renameConversation } from '@/modules/conversations/data/chatStorage';
import { useComposerPreferencesStore } from '@/modules/conversations/data/composerPreferences';
import {
  readConversationComposerDraft,
  writeConversationComposerDraft,
} from '@/modules/conversations/data/conversationComposerDrafts';
import { clearPendingComposerDraft, readPendingComposerDraft } from '@/modules/conversations/data/pendingComposerDraft';
import { useChatStreaming } from '@/modules/conversations/hooks/useChatStreaming';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { favoriteSourceConnection, isSameRoute } from '@/modules/conversations/stores/chatStoreUtils';
import type { ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { modelRouteKey } from '@/modules/providers/data/modelPreferences';
import { loadRouteParameters } from '@/modules/providers/data/modelRouteParameters';
import { keyboardShortcutMatches, useKeyboardShortcutsStore } from '@/modules/settings/stores/keyboardShortcutsStore';
import { LocalServiceError, localServiceClient, type ServiceOperation } from '@/platform/local-service';
import { type ChatSlashCommand, matchingSlashCommands } from './ChatSlashCommandMenu';
import {
  type ChatCommandFeedbackKey,
  isMessageSendingCommand,
  isObject,
  parseSlashCommand,
  providerNameForModel,
} from './composerUtils';
import { getGhostTextContext, parseGhostTextSuggestion } from './ghostTextSuggestions';

export interface UseComposerStateOptions {
  isTemporaryChat: boolean;
  onBeforeSend?: (providerId: string, providerName: string) => Promise<boolean>;
  onRequestTemporaryExit(action: () => void | Promise<void>): void;
  formRef: RefObject<HTMLFormElement | null>;
  droppedFiles?: { id: number; files: File[] } | null;
  targetConversationId?: string | null;
  messagesOverride?: readonly ChatMessage[];
  isLoadingMessagesOverride?: boolean;
}

export function useComposerState({
  isTemporaryChat,
  onBeforeSend,
  onRequestTemporaryExit,
  formRef,
  droppedFiles,
  targetConversationId,
  messagesOverride,
  isLoadingMessagesOverride,
}: UseComposerStateOptions) {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const filePickerRef = useRef<HTMLInputElement>(null);
  const isReadingAttachmentsRef = useRef(false);
  const pendingAttachmentsRef = useRef<PendingChatAttachment[]>([]);
  const [pendingAttachments, setPendingAttachments] = useState<PendingChatAttachment[]>([]);
  const [messageDraft, setMessageDraft] = useState(() =>
    targetConversationId
      ? (readConversationComposerDraft(targetConversationId) ?? '')
      : (readPendingComposerDraft() ?? ''),
  );
  useEffect(() => {
    if (!targetConversationId && messageDraft.length > 0) clearPendingComposerDraft(messageDraft);
  }, [messageDraft, targetConversationId]);
  useEffect(() => {
    if (targetConversationId) writeConversationComposerDraft(targetConversationId, messageDraft);
  }, [messageDraft, targetConversationId]);
  const [composerMode, setComposerMode] = useState<'normal' | 'plan' | 'goal'>('normal');
  const [selectedPermissionMode, setSelectedPermissionMode] = useState<ToolPermissionMode | null>(null);
  const [slashCommandIndex, setSlashCommandIndex] = useState(0);
  const [slashSuggestionsDismissed, setSlashSuggestionsDismissed] = useState(false);
  const [slashModelMenuOpen, setSlashModelMenuOpen] = useState(false);
  const [slashModelIndex, setSlashModelIndex] = useState(0);
  const [commandFeedbackKey, setCommandFeedbackKey] = useState<ChatCommandFeedbackKey | null>(null);
  const [attachmentErrorKey, setAttachmentErrorKey] = useState<
    ChatAttachmentErrorCode | 'modelDoesNotSupportImages' | null
  >(null);
  const [isReadingAttachments, setIsReadingAttachments] = useState(false);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [modelQuery, setModelQuery] = useState('');
  const [fastModeEnabled, setFastModeEnabled] = useState(false);
  const [fastModePreferenceState, setFastModePreferenceState] = useState<'loading' | 'ready' | 'saving' | 'error'>(
    'loading',
  );
  const temporaryDisclosureProviderIdRef = useRef<string | null>(null);
  const keyboardShortcuts = useKeyboardShortcutsStore((state) => state.bindings);
  const models = useChatStore((state) => state.models);
  const conversations = useChatStore((state) => state.conversations);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const storeSelectedModelKey = useChatStore((state) => state.selectedModelKey);
  const storeActiveConversationId = useChatStore((state) => state.activeConversationId);
  const activeConversationId = targetConversationId === undefined ? storeActiveConversationId : targetConversationId;
  const activeConversation = conversations.find((conversation) => conversation.id === activeConversationId);
  const selectedModelKey =
    targetConversationId === undefined
      ? storeSelectedModelKey
      : activeConversation
        ? (models.find((model) => isSameRoute(model, activeConversation))?.key ?? null)
        : null;
  const temporaryChat = useChatStore((state) => state.temporaryChat);
  const pendingQuestionSendBlock = useChatStore((state) => state.pendingQuestionSendBlock);
  const compactingConversationId = useChatStore((state) => state.compactingConversationId);
  const isCreatingMessageBranch = useChatStore((state) => state.isCreatingMessageBranch);
  const storeMessages = useChatStore((state) => state.messages);
  const messages = messagesOverride ?? storeMessages;
  const storeIsLoadingMessages = useChatStore((state) => state.isLoadingMessages);
  const isLoadingMessages = isLoadingMessagesOverride ?? storeIsLoadingMessages;
  const isUpdatingConversationModel = useChatStore((state) => state.isUpdatingConversationModel);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const selectConversationModel = useChatStore((state) => state.selectConversationModel);
  const beginConversationCompaction = useChatStore((state) => state.beginConversationCompaction);
  const finishConversationCompaction = useChatStore((state) => state.finishConversationCompaction);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const setPendingAttachmentCount = useChatStore((state) => state.setPendingAttachmentCount);
  const selectReasoningEffort = useChatStore((state) => state.selectReasoningEffort);
  const startNewChat = useChatStore((state) => state.startNewChat);
  const refreshConversations = useChatStore((state) => state.refreshConversations);
  const { isSending: isAnyConversationSending, sendMessage, stopResponse } = useChatStreaming();
  const sendingConversationId = useChatStore((state) => state.sendingConversationId);
  const isSending =
    isAnyConversationSending &&
    (targetConversationId === undefined
      ? sendingConversationId === storeActiveConversationId || sendingConversationId === temporaryChat?.id
      : targetConversationId !== null && sendingConversationId === targetConversationId);
  const modelContextLimits = useChatStore((state) => state.modelContextLimits);

  const selectableModels = models.filter((model) => !hiddenModelKeys.includes(modelRouteKey(model)));
  const pickerModels = activeConversationId
    ? models.filter((model) => !hiddenModelKeys.includes(modelRouteKey(model)) || model.key === selectedModelKey)
    : selectableModels;
  const selectedModel = models.find((model) => model.key === selectedModelKey);
  const pendingQuestionBlocksSend =
    activeConversationId !== null &&
    pendingQuestionSendBlock?.conversationId === activeConversationId &&
    pendingQuestionSendBlock.blocked;
  const isCompactingConversation = activeConversationId !== null && compactingConversationId === activeConversationId;
  const isSendBlocked = pendingQuestionBlocksSend || isCreatingMessageBranch || isCompactingConversation;
  const slashCommands = slashSuggestionsDismissed
    ? []
    : matchingSlashCommands(messageDraft).filter(
        (command) => !isTemporaryChat || command === 'new' || command === 'model',
      );
  const slashModelQuery = /^\/model(?:\s+([\s\S]*))?$/i.exec(messageDraft)?.[1]?.trim().toLocaleLowerCase('en') ?? '';
  const slashModelCandidates = selectableModels.filter((model) => {
    if (slashModelQuery.length === 0) return true;
    return [model.displayName, model.modelId, model.sourceLabel ?? ''].some((value) =>
      value.toLocaleLowerCase('en').includes(slashModelQuery),
    );
  });
  const previousConversationIdRef = useRef(activeConversationId);
  const isMountedRef = useRef(false);
  const isChatGptRoute = selectedModel?.providerId === 'chatgpt' || selectedModel?.providerId === 'chatgpt_api';
  const fastModeAvailable = selectedModel?.providerId === 'chatgpt_api' || selectedModel?.supportsFastMode === true;
  const ghostTextEnabled = useComposerPreferencesStore((state) => state.ghostTextEnabled);
  const ghostTextModelKey = useComposerPreferencesStore((state) => state.ghostTextModelKey);
  const [ghostTextDismissed, setGhostTextDismissed] = useState(false);
  const [generatedGhostText, setGeneratedGhostText] = useState<{ requestKey: string; text: string } | null>(null);
  const lastGhostTextRequestKeyRef = useRef<string | null>(null);

  const ghostTextContext = useMemo(() => getGhostTextContext(messages), [messages]);
  const ghostTextModel =
    ghostTextModelKey === 'auto' ? selectedModel : models.find((model) => model.key === ghostTextModelKey);
  const ghostTextConversationId = isTemporaryChat ? (temporaryChat?.id ?? null) : activeConversationId;
  const ghostTextRequestKey =
    ghostTextContext && ghostTextModel && ghostTextConversationId
      ? JSON.stringify([ghostTextConversationId, ghostTextContext.assistantMessageId, ghostTextModel.key])
      : null;

  // biome-ignore lint/correctness/useExhaustiveDependencies: Reset the dismissal when the conversation or latest completed message changes.
  useEffect(() => {
    setGhostTextDismissed(false);
  }, [ghostTextContext?.assistantMessageId, ghostTextConversationId]);

  useEffect(() => {
    if (
      !ghostTextEnabled ||
      !ghostTextContext ||
      !ghostTextModel ||
      !ghostTextRequestKey ||
      !ghostTextConversationId ||
      targetConversationId === null ||
      isLoadingMessages ||
      messageDraft.length > 0 ||
      isAnyConversationSending ||
      slashCommands.length > 0 ||
      generatedGhostText?.requestKey === ghostTextRequestKey ||
      lastGhostTextRequestKeyRef.current === ghostTextRequestKey
    ) {
      return;
    }

    lastGhostTextRequestKeyRef.current = ghostTextRequestKey;
    let disposed = false;
    let completed = false;
    let cancellationRequested = false;
    let cancellationSent = false;
    let operation: ServiceOperation | null = null;
    const temporaryConversationId = `tmp_${crypto.randomUUID().replaceAll('-', '')}`;
    const cancelOperation = () => {
      cancellationRequested = true;
      if (!operation || cancellationSent) return;
      cancellationSent = true;
      void operation.cancel().catch((cause: unknown) => {
        globalThis.reportError(new Error('The follow-up suggestion request could not be cancelled.', { cause }));
      });
    };
    const timeoutId = window.setTimeout(() => {
      if (disposed) return;
      disposed = true;
      setCommandFeedbackKey('ghostTextSuggestionUnavailable');
      cancelOperation();
    }, 20_000);

    void (async () => {
      try {
        const routeParameters = loadRouteParameters(ghostTextModel);
        operation = await localServiceClient.startOperation('chat.temporary.send', {
          conversationId: temporaryConversationId,
          providerId: ghostTextModel.providerId,
          modelId: ghostTextModel.modelId,
          messages: ghostTextContext.messages,
          ...routeParameters,
          ...(ghostTextModel.connectionId ? { connectionId: ghostTextModel.connectionId } : {}),
          ...(ghostTextModel.workspaceId ? { workspaceId: ghostTextModel.workspaceId } : {}),
        });
        if (cancellationRequested || disposed) {
          cancelOperation();
          return;
        }

        const result = await operation.result;
        if (disposed) return;
        completed = true;
        if (result.conversationId !== temporaryConversationId) {
          setCommandFeedbackKey('ghostTextSuggestionUnavailable');
          return;
        }
        const suggestion = parseGhostTextSuggestion(result.content);
        if (!suggestion) {
          setCommandFeedbackKey('ghostTextSuggestionUnavailable');
          return;
        }
        setGeneratedGhostText({ requestKey: ghostTextRequestKey, text: suggestion });
      } catch (error) {
        if (disposed) return;
        completed = true;
        setCommandFeedbackKey('ghostTextSuggestionUnavailable');
        if (!(error instanceof LocalServiceError)) {
          globalThis.reportError(new Error('The follow-up suggestion request failed.', { cause: error }));
        }
      } finally {
        window.clearTimeout(timeoutId);
      }
    })();

    return () => {
      disposed = true;
      window.clearTimeout(timeoutId);
      if (!completed && lastGhostTextRequestKeyRef.current === ghostTextRequestKey) {
        lastGhostTextRequestKeyRef.current = null;
      }
      if (!completed) cancelOperation();
    };
  }, [
    generatedGhostText?.requestKey,
    ghostTextContext,
    ghostTextEnabled,
    ghostTextModel,
    ghostTextRequestKey,
    ghostTextConversationId,
    isLoadingMessages,
    isAnyConversationSending,
    messageDraft.length,
    slashCommands.length,
    targetConversationId,
  ]);

  const ghostTextSuggestion = useMemo(() => {
    if (
      !ghostTextEnabled ||
      ghostTextDismissed ||
      messageDraft.length > 0 ||
      isAnyConversationSending ||
      !ghostTextRequestKey ||
      generatedGhostText?.requestKey !== ghostTextRequestKey
    ) {
      return null;
    }
    return generatedGhostText.text;
  }, [
    ghostTextEnabled,
    ghostTextDismissed,
    generatedGhostText,
    ghostTextRequestKey,
    messageDraft.length,
    isAnyConversationSending,
  ]);

  useEffect(() => {
    pendingAttachmentsRef.current = pendingAttachments;
  }, [pendingAttachments]);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      pendingAttachmentsRef.current.forEach((attachment) => {
        if (attachment.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
      });
      setPendingAttachmentCount(0);
    };
  }, [setPendingAttachmentCount]);

  useEffect(() => {
    try {
      setFastModeEnabled(readChatGptFastMode());
      setFastModePreferenceState('ready');
    } catch {
      setFastModePreferenceState('error');
      setCommandFeedbackKey('chatGptFastModeSettingsLoadFailed');
    }
  }, []);

  useEffect(() => {
    if (previousConversationIdRef.current === activeConversationId) return;
    previousConversationIdRef.current = activeConversationId;
    setComposerMode('normal');
    temporaryDisclosureProviderIdRef.current = null;
  }, [activeConversationId]);

  useEffect(() => {
    if (!isTemporaryChat) temporaryDisclosureProviderIdRef.current = null;
    else setComposerMode('normal');
  }, [isTemporaryChat]);

  useEffect(() => {
    const applyNotificationReply = (event: Event) => {
      if (!(event instanceof CustomEvent) || !isObject(event.detail)) return;
      if (event.detail.conversationId !== activeConversationId || typeof event.detail.text !== 'string') return;
      const input = inputRef.current;
      if (!input || input.value.trim() || !event.detail.text) return;
      input.value = event.detail.text;
      setMessageDraft(event.detail.text);
      input.style.height = 'auto';
      input.style.height = `${input.scrollHeight}px`;
      input.focus();
    };
    window.addEventListener('openchat-notification-reply-draft', applyNotificationReply);
    return () => window.removeEventListener('openchat-notification-reply-draft', applyNotificationReply);
  }, [activeConversationId]);

  useEffect(() => {
    const openModelPicker = () => setModelPickerOpen(true);
    window.addEventListener('openchat-open-model-picker', openModelPicker);
    return () => window.removeEventListener('openchat-open-model-picker', openModelPicker);
  }, []);

  useEffect(() => {
    if (!isSending) return;
    const stopWithShortcut = (event: globalThis.KeyboardEvent) => {
      if (event.defaultPrevented || event.repeat) return;
      const target = event.target;
      if (!(target instanceof HTMLElement) || !formRef.current?.contains(target)) return;
      if (!keyboardShortcutMatches(event, keyboardShortcuts.stopResponse)) return;
      event.preventDefault();
      void stopResponse();
    };
    window.addEventListener('keydown', stopWithShortcut);
    return () => window.removeEventListener('keydown', stopWithShortcut);
  }, [isSending, keyboardShortcuts.stopResponse, stopResponse, formRef]);

  useEffect(() => {
    setSlashCommandIndex((current) => Math.min(current, Math.max(0, slashCommands.length - 1)));
  }, [slashCommands.length]);

  useEffect(() => {
    setSlashModelIndex((current) => Math.min(current, Math.max(0, slashModelCandidates.length - 1)));
  }, [slashModelCandidates.length]);

  useEffect(() => {
    if (slashModelMenuOpen && !/^\/model(?:\s|$)/i.test(messageDraft)) {
      setSlashModelMenuOpen(false);
    }
  }, [messageDraft, slashModelMenuOpen]);

  const updateDraft = (value: string) => {
    setMessageDraft(value);
    setSlashSuggestionsDismissed(false);
    setSlashCommandIndex(0);
    const input = inputRef.current;
    if (!input) return;
    input.value = value;
    input.style.height = 'auto';
    input.style.height = `${input.scrollHeight}px`;
    input.setSelectionRange(value.length, value.length);
    input.focus();
  };

  const clearDraft = () => {
    setMessageDraft('');
    setSlashSuggestionsDismissed(false);
    setSlashCommandIndex(0);
    setSlashModelMenuOpen(false);
    setSlashModelIndex(0);
    if (!inputRef.current) return;
    inputRef.current.value = '';
    inputRef.current.style.height = 'auto';
  };

  const replacePendingAttachments = (attachments: PendingChatAttachment[]) => {
    pendingAttachmentsRef.current = attachments;
    setPendingAttachments(attachments);
    setPendingAttachmentCount(attachments.length);
  };

  const clearPendingAttachments = () => {
    pendingAttachmentsRef.current.forEach((attachment) => {
      if (attachment.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
    });
    replacePendingAttachments([]);
    setAttachmentErrorKey(null);
  };

  const selectModelForCurrentConversation = (key: string) =>
    selectConversationModel(key, targetConversationId ?? undefined);

  const sendContent = async (
    content: string,
    mode: 'normal' | 'plan' | 'goal',
    attachments: readonly PendingChatAttachment[],
    clearAttachmentsOnSuccess = true,
  ) => {
    const draftAtSubmission = inputRef.current?.value ?? messageDraft;
    if (
      isTemporaryChat &&
      onBeforeSend &&
      selectedModel &&
      temporaryDisclosureProviderIdRef.current !== selectedModel.providerId
    ) {
      const approved = await onBeforeSend(selectedModel.providerId, providerNameForModel(selectedModel, t));
      if (!approved) return false;
      temporaryDisclosureProviderIdRef.current = selectedModel.providerId;
    }
    const currentReasoningEffort = useChatStore.getState().selectedReasoningEffort;
    const options = {
      ...(mode === 'normal' ? {} : { mode }),
      ...(currentReasoningEffort &&
      selectedModel?.supportsReasoning &&
      selectedModel.reasoningLevels.includes(currentReasoningEffort)
        ? { reasoningEffort: currentReasoningEffort }
        : {}),
      ...(!isTemporaryChat && isChatGptRoute && fastModeEnabled ? { fastMode: true } : {}),
      ...(targetConversationId ? { conversationId: targetConversationId } : {}),
      onUserMessageCommitted: (conversationId: string) => {
        const currentState = useChatStore.getState();
        const currentConversationId = isTemporaryChat
          ? (currentState.temporaryChat?.id ?? null)
          : (targetConversationId ?? currentState.activeConversationId);
        if (currentConversationId !== conversationId || inputRef.current?.value !== draftAtSubmission) return;
        clearDraft();
      },
    };
    const saved = await sendMessage(content, attachments, undefined, undefined, options);
    if (!saved) {
      if (targetConversationId) setCommandFeedbackKey('chatRequestFailed');
      return false;
    }
    if (clearAttachmentsOnSuccess) clearPendingAttachments();
    setComposerMode('normal');
    setCommandFeedbackKey(null);
    return true;
  };

  const runSlashCommand = async (command: ChatSlashCommand, rawArguments: string) => {
    const args = rawArguments.trim();
    setCommandFeedbackKey(null);
    switch (command) {
      case 'new': {
        onRequestTemporaryExit(async () => {
          startNewChat();
          setComposerMode('normal');
          clearDraft();
          clearPendingAttachments();
          if (args) {
            try {
              if (!(await sendContent(args, 'normal', []))) updateDraft(args);
            } catch {
              setCommandFeedbackKey('chatRequestFailed');
            }
          }
        });
        return;
      }
      case 'summarize': {
        if (!activeConversationId) {
          setCommandFeedbackKey('slashCommandSummarizeNoMessages');
          return;
        }
        if (isLoadingMessages) {
          setCommandFeedbackKey('slashCommandSummarizeLoading');
          return;
        }
        if (messages.length === 0) {
          setCommandFeedbackKey('slashCommandSummarizeNoMessages');
          return;
        }
        if (isAnyConversationSending || isUpdatingConversationModel || isSendBlocked || !selectedModelKey) {
          setCommandFeedbackKey('slashCommandSummarizeUnavailable');
          return;
        }
        const prompt = t('slashCommandSummarizePrompt');
        try {
          const summaryRequest = args ? `${prompt}\n\n${args}` : prompt;
          if (!(await sendContent(summaryRequest, 'normal', [], false))) {
            setCommandFeedbackKey('slashCommandSummarizeUnavailable');
          }
        } catch {
          setCommandFeedbackKey('slashCommandSummarizeUnavailable');
        }
        return;
      }
      case 'rename': {
        if (!args) {
          setCommandFeedbackKey('slashCommandRenameTitleRequired');
          return;
        }
        if (!activeConversationId) {
          setCommandFeedbackKey('slashCommandRenameNoConversation');
          return;
        }
        try {
          await renameConversation(activeConversationId, args);
          await refreshConversations();
          setCommandFeedbackKey('slashCommandRenameSuccess');
          clearDraft();
        } catch {
          setCommandFeedbackKey('conversationTitleSaveFailed');
        }
        return;
      }
      case 'model': {
        if (!args) {
          setCommandFeedbackKey('slashCommandModelNameRequired');
          return;
        }
        if (isAnyConversationSending || isUpdatingConversationModel || modelStatus === 'loading') {
          setCommandFeedbackKey('slashCommandModelUnavailable');
          return;
        }
        const normalizedQuery = args.toLocaleLowerCase('en');
        const matches = pickerModels.filter(
          (model) =>
            model.modelId.toLocaleLowerCase('en') === normalizedQuery ||
            model.displayName.toLocaleLowerCase('en') === normalizedQuery,
        );
        if (matches.length === 0) {
          setCommandFeedbackKey('slashCommandModelNoMatch');
          return;
        }
        if (matches.length > 1) {
          setCommandFeedbackKey('slashCommandModelAmbiguous');
          return;
        }
        const selectedModel = matches[0];
        if (!selectedModel) {
          setCommandFeedbackKey('slashCommandModelNoMatch');
          return;
        }
        const changed = await selectModelForCurrentConversation(selectedModel.key);
        if (!changed) {
          const code = useChatStore.getState().errorCode;
          setCommandFeedbackKey(
            code === 'conversationModelSaveFailed' ? 'conversationModelSaveFailed' : 'slashCommandModelUnavailable',
          );
          return;
        }
        clearDraft();
        return;
      }
      case 'compact': {
        if (args) {
          setCommandFeedbackKey('slashCommandCompactNoArguments');
          return;
        }
        if (!activeConversationId) {
          setCommandFeedbackKey('slashCommandCompactNoConversation');
          return;
        }
        if (isAnyConversationSending || isUpdatingConversationModel || isSendBlocked) {
          setCommandFeedbackKey('slashCommandCompactBusy');
          return;
        }
        if (isLoadingMessages) {
          setCommandFeedbackKey('slashCommandSummarizeLoading');
          return;
        }
        const configuredLimit = selectedModel ? modelContextLimits[modelRouteKey(selectedModel)] : undefined;
        const modelLimit = selectedModel?.contextWindow ?? null;
        const contextWindow =
          configuredLimit === undefined
            ? modelLimit
            : modelLimit === null
              ? configuredLimit
              : Math.min(configuredLimit, modelLimit);
        if (contextWindow === null || contextWindow < 2048 || contextWindow > 2_000_000) {
          setCommandFeedbackKey('slashCommandCompactNoContextWindow');
          return;
        }
        const conversationId = activeConversationId;
        if (!beginConversationCompaction(conversationId)) {
          setCommandFeedbackKey('slashCommandCompactBusy');
          return;
        }
        try {
          const result = await localServiceClient.call('chat.memory.compact', { conversationId, contextWindow });
          if (!targetConversationId && useChatStore.getState().activeConversationId !== conversationId) return;
          setCommandFeedbackKey(
            result.compacted === true ? 'slashCommandCompactSuccess' : 'slashCommandCompactNothing',
          );
          clearDraft();
        } catch {
          if (targetConversationId || useChatStore.getState().activeConversationId === conversationId) {
            setCommandFeedbackKey('slashCommandCompactFailed');
          }
        } finally {
          finishConversationCompaction(conversationId);
        }
        return;
      }
      case 'reasoning': {
        if (!args) {
          setCommandFeedbackKey('slashCommandReasoningLevelRequired');
          return;
        }
        if (args.toLocaleLowerCase('en') === 'default' || args.toLocaleLowerCase('en') === 'auto') {
          selectReasoningEffort(null);
          clearDraft();
          return;
        }
        const matchingLevel = selectedModel?.reasoningLevels.find(
          (level) => level.toLocaleLowerCase('en') === args.toLocaleLowerCase('en'),
        );
        if (!selectedModel?.supportsReasoning || !matchingLevel) {
          setCommandFeedbackKey('slashCommandReasoningUnsupported');
          return;
        }
        selectReasoningEffort(matchingLevel);
        clearDraft();
        return;
      }
      case 'plan':
      case 'goal':
        setComposerMode(command);
        if (!args) {
          clearDraft();
          return;
        }
        if (isSendBlocked) {
          setCommandFeedbackKey('slashCommandSummarizeUnavailable');
          return;
        }
        try {
          if (!(await sendContent(args, command, pendingAttachments))) {
            setCommandFeedbackKey(
              useChatStore.getState().errorCode === 'goalObjectiveTooLong' ? 'slashCommandSummarizeUnavailable' : null,
            );
          }
        } catch {
          setCommandFeedbackKey('chatRequestFailed');
        }
    }
  };

  const selectSlashCommand = (command: ChatSlashCommand) => {
    setSlashSuggestionsDismissed(true);
    setCommandFeedbackKey(null);
    if (command === 'model') {
      updateDraft('/model ');
      setSlashSuggestionsDismissed(true);
      setSlashModelMenuOpen(true);
      setSlashModelIndex(0);
      void refreshModels(true).catch(() => setCommandFeedbackKey('slashCommandModelUnavailable'));
      return;
    }
    if (
      command === 'new' ||
      command === 'summarize' ||
      command === 'compact' ||
      command === 'plan' ||
      command === 'goal'
    ) {
      clearDraft();
      void runSlashCommand(command, '');
      return;
    }
    updateDraft(`/${command} `);
  };

  const selectSlashModel = async (model: ChatModel) => {
    if (isAnyConversationSending || isUpdatingConversationModel) return;
    setSlashModelMenuOpen(false);
    const changed = await selectModelForCurrentConversation(model.key);
    if (!changed) {
      setCommandFeedbackKey('slashCommandModelUnavailable');
      return;
    }
    clearDraft();
  };

  const appendAttachments = (attachments: PendingChatAttachment[]) => {
    const existingAttachments = pendingAttachmentsRef.current;
    validateAttachmentSelection(existingAttachments, attachments);
    if (attachments.some((attachment) => attachment.kind === 'image') && selectedModel?.supportsImages !== true) {
      setAttachmentErrorKey('modelDoesNotSupportImages');
      return false;
    }
    const nextAttachments = [
      ...existingAttachments,
      ...attachments.map((attachment) => ({
        ...attachment,
        previewUrl: attachment.kind === 'image' ? URL.createObjectURL(attachment.file) : null,
      })),
    ];
    replacePendingAttachments(nextAttachments);
    setAttachmentErrorKey(null);
    return true;
  };

  const addAttachments = async (files: File[]) => {
    if (isReadingAttachmentsRef.current) return;
    isReadingAttachmentsRef.current = true;
    setIsReadingAttachments(true);
    setAttachmentErrorKey(null);
    try {
      const existingAttachments = pendingAttachmentsRef.current;
      if (existingAttachments.length + files.length > maximumAttachmentFileCount) {
        throw new ChatAttachmentInputError('attachmentCountExceeded');
      }
      const selectedTotal =
        existingAttachments.reduce((total, attachment) => total + attachment.sizeBytes, 0) +
        files.reduce((total, file) => total + file.size, 0);
      if (selectedTotal > maximumAttachmentBytes) {
        throw new ChatAttachmentInputError('attachmentTotalTooLarge');
      }
      const attachments = await Promise.all(files.map(readAttachmentFile));
      if (!isMountedRef.current) return;
      appendAttachments(attachments);
    } catch (error) {
      if (!isMountedRef.current) return;
      setAttachmentErrorKey(error instanceof ChatAttachmentInputError ? error.code : 'attachmentReadFailed');
    } finally {
      isReadingAttachmentsRef.current = false;
      if (isMountedRef.current) setIsReadingAttachments(false);
    }
  };

  const addExistingAttachments = async (items: readonly ConversationAttachmentSummary[]) => {
    if (isReadingAttachmentsRef.current || items.length === 0) return false;
    isReadingAttachmentsRef.current = true;
    setIsReadingAttachments(true);
    setAttachmentErrorKey(null);
    try {
      const existingAttachments = pendingAttachmentsRef.current;
      const metadata = items.map((item) => ({
        id: item.attachmentId,
        name: item.fileName,
        mimeType: item.mimeType,
        sizeBytes: item.sizeBytes,
        kind: item.mimeType.startsWith('image/') ? ('image' as const) : ('text' as const),
      }));
      validateAttachmentSelection(existingAttachments, metadata);
      const attachments = await Promise.all(
        items.map(async (item, index) => {
          const bytes = await readMessageAttachment(item.conversationId, item.messageId, item.attachmentId);
          const itemMetadata = metadata[index];
          if (!itemMetadata) throw new ChatAttachmentInputError('attachmentReadFailed');
          return restoreStoredAttachment(itemMetadata, bytes);
        }),
      );
      if (!isMountedRef.current) return false;
      return appendAttachments(attachments);
    } catch (error) {
      if (!isMountedRef.current) return false;
      setAttachmentErrorKey(error instanceof ChatAttachmentInputError ? error.code : 'attachmentReadFailed');
      return false;
    } finally {
      isReadingAttachmentsRef.current = false;
      if (isMountedRef.current) setIsReadingAttachments(false);
    }
  };

  const addAttachmentsRef = useRef(addAttachments);
  useEffect(() => {
    addAttachmentsRef.current = addAttachments;
  });

  const lastHandledDropIdRef = useRef<number | null>(null);
  useEffect(() => {
    if (!droppedFiles || droppedFiles.files.length === 0) return;
    if (lastHandledDropIdRef.current === droppedFiles.id) return;
    lastHandledDropIdRef.current = droppedFiles.id;
    void addAttachmentsRef.current(droppedFiles.files);
  }, [droppedFiles]);

  useEffect(() => {
    const handleAddAttachmentsEvent = (event: Event) => {
      if (!(event instanceof CustomEvent) || !isObject(event.detail) || !Array.isArray(event.detail.files)) return;
      const files = event.detail.files.filter((file): file is File => file instanceof File);
      if (files.length === 0) return;
      void addAttachmentsRef.current(files);
    };
    window.addEventListener('openchat-composer-add-attachments', handleAddAttachmentsEvent);
    return () => window.removeEventListener('openchat-composer-add-attachments', handleAddAttachmentsEvent);
  }, []);

  const removeAttachment = (id: string) => {
    const currentAttachments = pendingAttachmentsRef.current;
    const attachment = currentAttachments.find((item) => item.id === id);
    if (attachment?.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
    const nextAttachments = currentAttachments.filter((item) => item.id !== id);
    replacePendingAttachments(nextAttachments);
  };

  const send = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!inputRef.current || isReadingAttachments || isUpdatingConversationModel) return;

    if (isAnyConversationSending && !isSending) return;
    if (isSending) {
      if (!messageDraft.trim() && pendingAttachments.length === 0) {
        void stopResponse();
        return;
      }
      void (async () => {
        try {
          await stopResponse();
          await new Promise((resolve) => setTimeout(resolve, 60));
        } catch {
          // Proceed with steer send
        }
        const slashCommand = parseSlashCommand(messageDraft);
        if (slashCommand) {
          if (isSendBlocked && isMessageSendingCommand(slashCommand.command)) return;
          void runSlashCommand(slashCommand.command, slashCommand.arguments);
          return;
        }
        if (isSendBlocked) return;
        if (!messageDraft.trim() && pendingAttachments.length === 0) return;
        void sendContent(messageDraft, composerMode, pendingAttachments)
          .catch(() => {
            setCommandFeedbackKey('chatRequestFailed');
          })
          .finally(() => inputRef.current?.focus());
      })();
      return;
    }

    const slashCommand = parseSlashCommand(messageDraft);
    if (slashCommand) {
      if (isSendBlocked && isMessageSendingCommand(slashCommand.command)) return;
      void runSlashCommand(slashCommand.command, slashCommand.arguments);
      return;
    }
    if (isSendBlocked) return;
    if (!messageDraft.trim() && pendingAttachments.length === 0) return;
    void sendContent(messageDraft, composerMode, pendingAttachments)
      .catch(() => {
        setCommandFeedbackKey('chatRequestFailed');
      })
      .finally(() => inputRef.current?.focus());
  };

  const toggleFastMode = () => {
    if (
      !isChatGptRoute ||
      !fastModeAvailable ||
      fastModePreferenceState === 'loading' ||
      fastModePreferenceState === 'saving'
    )
      return;
    const nextValue = !fastModeEnabled;
    setFastModePreferenceState('saving');
    try {
      writeChatGptFastMode(nextValue);
      setFastModeEnabled(nextValue);
      setFastModePreferenceState('ready');
      setCommandFeedbackKey(null);
    } catch {
      setFastModePreferenceState('error');
      setCommandFeedbackKey('chatGptFastModeSettingsSaveFailed');
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.nativeEvent.isComposing) return;
    if (slashModelMenuOpen) {
      if (event.key === 'Escape') {
        event.preventDefault();
        clearDraft();
        return;
      }
      if (slashModelCandidates.length > 0 && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
        event.preventDefault();
        setSlashModelIndex((current) => {
          const direction = event.key === 'ArrowDown' ? 1 : -1;
          return (current + direction + slashModelCandidates.length) % slashModelCandidates.length;
        });
        return;
      }
      if (
        slashModelCandidates.length > 0 &&
        event.key === 'Enter' &&
        !event.shiftKey &&
        !event.altKey &&
        !event.ctrlKey &&
        !event.metaKey
      ) {
        event.preventDefault();
        const selectedModel = slashModelCandidates[slashModelIndex] ?? slashModelCandidates[0];
        if (selectedModel) void selectSlashModel(selectedModel);
        return;
      }
    }
    if (ghostTextSuggestion && messageDraft.length === 0 && slashCommands.length === 0 && !slashModelMenuOpen) {
      if (event.key === 'Tab') {
        event.preventDefault();
        updateDraft(ghostTextSuggestion);
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        setGhostTextDismissed(true);
        return;
      }
    }
    if (slashCommands.length > 0 && event.key === 'ArrowDown') {
      event.preventDefault();
      setSlashCommandIndex((current) => (current + 1) % slashCommands.length);
      return;
    }
    if (slashCommands.length > 0 && event.key === 'ArrowUp') {
      event.preventDefault();
      setSlashCommandIndex((current) => (current - 1 + slashCommands.length) % slashCommands.length);
      return;
    }
    if (slashCommands.length > 0 && event.key === 'Escape') {
      event.preventDefault();
      setSlashSuggestionsDismissed(true);
      return;
    }
    if (slashCommands.length > 0 && event.key === 'Tab') {
      event.preventDefault();
      const selectedCommand = slashCommands[slashCommandIndex] ?? slashCommands[0];
      if (selectedCommand) selectSlashCommand(selectedCommand);
      return;
    }
    if (
      slashCommands.length > 0 &&
      event.key === 'Enter' &&
      !event.shiftKey &&
      !event.altKey &&
      !event.ctrlKey &&
      !event.metaKey
    ) {
      event.preventDefault();
      const selectedCommand = slashCommands[slashCommandIndex] ?? slashCommands[0];
      if (selectedCommand) selectSlashCommand(selectedCommand);
      return;
    }
    if (keyboardShortcutMatches(event, keyboardShortcuts.sendMessage)) {
      event.preventDefault();
      event.currentTarget.form?.requestSubmit();
      return;
    }
    if (keyboardShortcutMatches(event, keyboardShortcuts.insertNewLine)) {
      event.preventDefault();
      const input = event.currentTarget;
      input.setRangeText('\n', input.selectionStart, input.selectionEnd, 'end');
      input.dispatchEvent(new Event('input', { bubbles: true }));
      return;
    }
  };

  const selectFavoriteModel = async (favorite: ChatModelFavorite): Promise<boolean> => {
    const initialState = useChatStore.getState();
    const selectionConversationId =
      targetConversationId === undefined ? initialState.activeConversationId : targetConversationId;
    const targetTemporaryChatId = targetConversationId === undefined ? (initialState.temporaryChat?.id ?? null) : null;
    const isTargetStillActive = () => {
      const current = useChatStore.getState();
      if (targetConversationId !== undefined && selectionConversationId !== null) {
        return current.conversations.some((conversation) => conversation.id === selectionConversationId);
      }
      return (
        current.activeConversationId === selectionConversationId &&
        (current.temporaryChat?.id ?? null) === targetTemporaryChatId
      );
    };
    const findModel = () => {
      const current = useChatStore.getState();
      return current.models.find(
        (model) =>
          model.providerId === favorite.providerId &&
          model.modelId === favorite.modelId &&
          favoriteSourceConnection(model) === favorite.sourceConnectionId &&
          !current.hiddenModelKeys.includes(modelRouteKey(model)),
      );
    };
    let model = findModel();
    if (!model) {
      try {
        await refreshModels(true);
      } catch {
        setCommandFeedbackKey('slashCommandModelUnavailable');
        return false;
      }
      if (!isTargetStillActive()) return false;
      model = findModel();
    }
    if (!model || !isTargetStillActive()) {
      setCommandFeedbackKey('slashCommandModelUnavailable');
      return false;
    }
    const updated = await selectConversationModel(model.key, selectionConversationId ?? undefined);
    setCommandFeedbackKey(updated ? null : 'slashCommandModelUnavailable');
    return updated;
  };

  const successFeedback =
    commandFeedbackKey === 'slashCommandCompactSuccess' || commandFeedbackKey === 'slashCommandRenameSuccess';
  const selectedSlashCommand = slashCommands[slashCommandIndex];
  const selectedSlashModel = slashModelCandidates[slashModelIndex];

  return {
    inputRef,
    filePickerRef,
    messageDraft,
    setMessageDraft,
    composerMode,
    setComposerMode,
    selectedPermissionMode,
    setSelectedPermissionMode,
    pendingAttachments,
    isReadingAttachments,
    attachmentErrorKey,
    commandFeedbackKey,
    setCommandFeedbackKey,
    successFeedback,
    slashCommands,
    slashCommandIndex,
    setSlashCommandIndex,
    selectedSlashCommand,
    slashModelMenuOpen,
    slashModelIndex,
    setSlashModelIndex,
    slashModelCandidates,
    selectedSlashModel,
    modelPickerOpen,
    setModelPickerOpen,
    modelQuery,
    setModelQuery,
    fastModeEnabled,
    fastModeAvailable,
    fastModePreferenceState,
    toggleFastMode,
    addAttachments,
    addExistingAttachments,
    removeAttachment,
    selectSlashCommand,
    selectSlashModel,
    selectFavoriteModel,
    handleKeyDown,
    send,
    setSlashSuggestionsDismissed,
    pickerModels,
    selectedModel,
    isSendBlocked,
    isSending,
    isAnyConversationSending,
    ghostTextSuggestion,
  };
}
