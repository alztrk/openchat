import { ArrowUp, Folder, Paperclip, Square, X } from 'lucide-react';
import { type FormEvent, type KeyboardEvent, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Select } from '@/design-system';
import {
  type ChatAttachmentErrorCode,
  ChatAttachmentInputError,
  maximumAttachmentBytes,
  maximumAttachmentFileCount,
  type PendingChatAttachment,
  readAttachmentFile,
  validateAttachmentSelection,
} from '@/modules/conversations/data/chatAttachments';
import { readChatGptFastMode, writeChatGptFastMode } from '@/modules/conversations/data/chatGptFastMode';
import type { ChatModelFavorite } from '@/modules/conversations/data/chatStorage';
import { renameConversation } from '@/modules/conversations/data/chatStorage';
import { useChatStreaming } from '@/modules/conversations/hooks/useChatStreaming';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { favoriteSourceConnection } from '@/modules/conversations/stores/chatStoreUtils';
import { ToolPermissionModeSelector } from '@/modules/projects/components/ToolPermissionModeSelector';
import type { ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { modelRouteKey } from '@/modules/providers/data/modelPreferences';
import { keyboardShortcutMatches, useKeyboardShortcutsStore } from '@/modules/settings/stores/keyboardShortcutsStore';
import { localServiceClient } from '@/platform/local-service';
import { type ChatSlashCommand, ChatSlashCommandMenu, matchingSlashCommands } from './ChatSlashCommandMenu';
import { ChatSlashModelMenu } from './ChatSlashModelMenu';
import { ContextUsageIndicator } from './ContextUsageIndicator';
import { ModelPicker } from './ModelPicker';
import { PendingAttachmentGallery } from './PendingAttachmentGallery';

export interface ChatComposerProps {
  onOpenSettings(): void;
  onRequestTemporaryExit(action: () => void | Promise<void>): void;
  onBeforeSend?: (providerId: string, providerName: string) => Promise<boolean>;
  isTemporaryChat?: boolean;
}

type ChatCommandFeedbackKey =
  | 'chatRequestFailed'
  | 'chatGptFastModeSettingsLoadFailed'
  | 'chatGptFastModeSettingsSaveFailed'
  | 'conversationModelSaveFailed'
  | 'conversationTitleSaveFailed'
  | 'messageHistoryLoadFailed'
  | 'slashCommandCompactBusy'
  | 'slashCommandCompactFailed'
  | 'slashCommandCompactNoArguments'
  | 'slashCommandCompactNoContextWindow'
  | 'slashCommandCompactNoConversation'
  | 'slashCommandCompactNothing'
  | 'slashCommandCompactSuccess'
  | 'slashCommandModelAmbiguous'
  | 'slashCommandModelNameRequired'
  | 'slashCommandModelNoMatch'
  | 'slashCommandModelUnavailable'
  | 'slashCommandReasoningLevelRequired'
  | 'slashCommandReasoningUnsupported'
  | 'slashCommandRenameNoConversation'
  | 'slashCommandRenameSuccess'
  | 'slashCommandRenameTitleRequired'
  | 'slashCommandSummarizeLoading'
  | 'slashCommandSummarizeNoMessages'
  | 'slashCommandSummarizeUnavailable';

export function ChatComposer({
  isTemporaryChat = false,
  onBeforeSend,
  onOpenSettings,
  onRequestTemporaryExit,
}: ChatComposerProps) {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const filePickerRef = useRef<HTMLInputElement>(null);
  const isReadingAttachmentsRef = useRef(false);
  const pendingAttachmentsRef = useRef<PendingChatAttachment[]>([]);
  const [pendingAttachments, setPendingAttachments] = useState<PendingChatAttachment[]>([]);
  const [messageDraft, setMessageDraft] = useState('');
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
  const initializeKeyboardShortcuts = useKeyboardShortcutsStore((state) => state.initialize);
  const models = useChatStore((state) => state.models);
  const modelProviders = useChatStore((state) => state.modelProviders);
  const conversations = useChatStore((state) => state.conversations);
  const projects = useChatStore((state) => state.projects);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const pendingProjectId = useChatStore((state) => state.pendingProjectId);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const selectedModelKey = useChatStore((state) => state.selectedModelKey);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const pendingQuestionSendBlock = useChatStore((state) => state.pendingQuestionSendBlock);
  const compactingConversationId = useChatStore((state) => state.compactingConversationId);
  const isCreatingMessageBranch = useChatStore((state) => state.isCreatingMessageBranch);
  const messages = useChatStore((state) => state.messages);
  const isLoadingMessages = useChatStore((state) => state.isLoadingMessages);
  const isUpdatingConversationModel = useChatStore((state) => state.isUpdatingConversationModel);
  const selectedReasoningEffort = useChatStore((state) => state.selectedReasoningEffort);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelFavorites = useChatStore((state) => state.modelFavorites);
  const toggleFavoriteModel = useChatStore((state) => state.toggleFavoriteModel);
  const removeModelFavorite = useChatStore((state) => state.removeModelFavorite);
  const selectConversationModel = useChatStore((state) => state.selectConversationModel);
  const selectModel = useChatStore((state) => state.selectModel);
  const beginConversationCompaction = useChatStore((state) => state.beginConversationCompaction);
  const finishConversationCompaction = useChatStore((state) => state.finishConversationCompaction);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const setPendingAttachmentCount = useChatStore((state) => state.setPendingAttachmentCount);
  const selectReasoningEffort = useChatStore((state) => state.selectReasoningEffort);
  const startNewChat = useChatStore((state) => state.startNewChat);
  const setPendingProjectId = useChatStore((state) => state.setPendingProjectId);
  const refreshConversations = useChatStore((state) => state.refreshConversations);
  const { isSending, sendMessage, stopResponse } = useChatStreaming();
  const chatErrorCode = useChatStore((state) => state.errorCode);
  const modelError = useChatStore((state) => state.modelError);
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
        (command) => !isTemporaryChat || command === 'new' || command === 'search' || command === 'model',
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
  const contextWindow = selectedModel === undefined ? null : effectiveContextWindow(selectedModel, modelContextLimits);
  const activeConversation = conversations.find((conversation) => conversation.id === activeConversationId);
  const contextProjectId = activeConversation?.projectId ?? pendingProjectId;
  const contextToolPermissionMode = composerMode === 'plan' ? 'plan' : selectedPermissionMode;
  const isChatGptRoute = selectedModel?.providerId === 'chatgpt' || selectedModel?.providerId === 'chatgpt_api';
  const fastModeAvailable = selectedModel?.providerId === 'chatgpt_api' || selectedModel?.supportsFastMode === true;

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

  useEffect(() => initializeKeyboardShortcuts(), [initializeKeyboardShortcuts]);

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
    if (!isSending) return;
    const stopWithShortcut = (event: globalThis.KeyboardEvent) => {
      const target = event.target;
      if (!(target instanceof HTMLElement) || !formRef.current?.contains(target)) return;
      if (!keyboardShortcutMatches(event, keyboardShortcuts.stopResponse)) return;
      event.preventDefault();
      void stopResponse();
    };
    window.addEventListener('keydown', stopWithShortcut);
    return () => window.removeEventListener('keydown', stopWithShortcut);
  }, [isSending, keyboardShortcuts.stopResponse, stopResponse]);

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

  const sendContent = async (
    content: string,
    mode: 'normal' | 'plan' | 'goal',
    attachments: readonly PendingChatAttachment[],
    clearAttachmentsOnSuccess = true,
  ) => {
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
      ...(currentReasoningEffort ? { reasoningEffort: currentReasoningEffort } : {}),
      ...(!isTemporaryChat && isChatGptRoute && fastModeEnabled ? { fastMode: true } : {}),
    };
    const saved = await sendMessage(content, attachments, undefined, undefined, options);
    if (!saved) return false;
    clearDraft();
    if (clearAttachmentsOnSuccess) clearPendingAttachments();
    setComposerMode('normal');
    setCommandFeedbackKey(null);
    return true;
  };

  const openHistorySearch = (query: string) => {
    window.dispatchEvent(new CustomEvent('openchat-history-search', { detail: { query } }));
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
      case 'search':
        openHistorySearch(args);
        clearDraft();
        return;
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
        if (isSending || isUpdatingConversationModel || isSendBlocked || !selectedModelKey) {
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
        if (isSending || isUpdatingConversationModel || modelStatus === 'loading') {
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
        const changed = await selectConversationModel(selectedModel.key);
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
        if (isSending || isUpdatingConversationModel || isSendBlocked) {
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
          if (useChatStore.getState().activeConversationId !== conversationId) return;
          setCommandFeedbackKey(
            result.compacted === true ? 'slashCommandCompactSuccess' : 'slashCommandCompactNothing',
          );
          clearDraft();
        } catch {
          if (useChatStore.getState().activeConversationId === conversationId) {
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
      command === 'search' ||
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
    if (isSending || isUpdatingConversationModel) return;
    setSlashModelMenuOpen(false);
    const changed = await selectConversationModel(model.key);
    if (!changed) {
      setCommandFeedbackKey('slashCommandModelUnavailable');
      return;
    }
    clearDraft();
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
      validateAttachmentSelection(existingAttachments, attachments);
      const selectedModel = models.find((model) => model.key === selectedModelKey);
      if (attachments.some((attachment) => attachment.kind === 'image') && selectedModel?.supportsImages !== true) {
        setAttachmentErrorKey('modelDoesNotSupportImages');
        return;
      }
      const nextAttachments = [
        ...existingAttachments,
        ...attachments.map((attachment) => ({
          ...attachment,
          previewUrl: attachment.kind === 'image' ? URL.createObjectURL(attachment.file) : null,
        })),
      ];
      replacePendingAttachments(nextAttachments);
    } catch (error) {
      if (isMountedRef.current) {
        setAttachmentErrorKey(error instanceof ChatAttachmentInputError ? error.code : 'attachmentReadFailed');
      }
    } finally {
      isReadingAttachmentsRef.current = false;
      if (isMountedRef.current) setIsReadingAttachments(false);
    }
  };

  const removeAttachment = (id: string) => {
    const currentAttachments = pendingAttachmentsRef.current;
    const attachment = currentAttachments.find((item) => item.id === id);
    if (attachment?.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
    const nextAttachments = currentAttachments.filter((item) => item.id !== id);
    replacePendingAttachments(nextAttachments);
  };

  const send = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!inputRef.current || isSending || isReadingAttachments || isUpdatingConversationModel) return;
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
    if (keyboardShortcutMatches(event.nativeEvent, keyboardShortcuts.sendMessage)) {
      event.preventDefault();
      event.currentTarget.form?.requestSubmit();
      return;
    }
    if (keyboardShortcutMatches(event.nativeEvent, keyboardShortcuts.insertNewLine)) {
      event.preventDefault();
      const input = event.currentTarget;
      input.setRangeText('\n', input.selectionStart, input.selectionEnd, 'end');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
  };

  const successFeedback =
    commandFeedbackKey === 'slashCommandCompactSuccess' || commandFeedbackKey === 'slashCommandRenameSuccess';
  const selectedSlashCommand = slashCommands[slashCommandIndex];
  const selectedSlashModel = slashModelCandidates[slashModelIndex];
  const selectFavoriteModel = async (favorite: ChatModelFavorite): Promise<boolean> => {
    const initialState = useChatStore.getState();
    const targetConversationId = initialState.activeConversationId;
    const targetTemporaryChatId = initialState.temporaryChat?.id ?? null;
    const isTargetStillActive = () => {
      const current = useChatStore.getState();
      return (
        current.activeConversationId === targetConversationId &&
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
    const updated = await selectConversationModel(model.key);
    setCommandFeedbackKey(updated ? null : 'slashCommandModelUnavailable');
    return updated;
  };

  return (
    <div className="shrink-0 px-4 pb-5 pt-3 sm:px-8">
      <div className="composer-container mx-auto max-w-composer">
        {models.length === 0 ? (
          <div className="mb-3 flex flex-wrap items-center justify-between gap-3 rounded-card border border-border bg-surface px-4 py-3 text-sm text-secondary-text">
            <span>{t('noModelConnectedBody')}</span>
            <Button onClick={onOpenSettings} size="small" variant="secondary">
              {t('settings')}
            </Button>
          </div>
        ) : selectableModels.length === 0 && activeConversationId === null ? (
          <div className="mb-3 flex flex-wrap items-center justify-between gap-3 rounded-card border border-border bg-surface px-4 py-3 text-sm text-secondary-text">
            <span>{t('noVisibleModels')}</span>
            <Button onClick={onOpenSettings} size="small" variant="secondary">
              {t('settings')}
            </Button>
          </div>
        ) : null}
        <div className="composer-context-row flex min-w-0 items-center justify-between gap-3">
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <Select
              aria-label={t('selectProject')}
              className="min-h-8 max-w-64 border-0 bg-transparent px-2 text-xs hover:bg-hover focus-visible:ring-0"
              disabled={
                isTemporaryChat ||
                activeConversationId !== null ||
                isSending ||
                projectsStatus !== 'ready' ||
                projects.length === 0
              }
              label={t('selectProject')}
              onChange={(event) => setPendingProjectId(event.target.value || null)}
              options={
                projects.length === 0
                  ? [
                      {
                        value: '',
                        label: t(projectsStatus === 'error' ? 'projectLoadFailed' : 'noProjects'),
                        disabled: true,
                      },
                    ]
                  : [
                      { value: '', label: t('noProjectSelected') },
                      ...projects.map((project) => ({ value: project.id, label: project.name })),
                    ]
              }
              value={contextProjectId ?? ''}
              visuallyHiddenLabel
            />
          </div>
          {selectedModel && contextToolPermissionMode !== null ? (
            <ContextUsageIndicator
              contextWindow={contextWindow}
              conversationId={activeConversationId}
              draft={messageDraft}
              isSending={isSending}
              messages={messages}
              model={selectedModel}
              pendingAttachments={pendingAttachments}
              projectId={contextProjectId}
              toolPermissionMode={contextToolPermissionMode}
            />
          ) : selectedModel ? (
            <p className="max-w-48 text-xs text-destructive" role="alert">
              {t('toolPermissionSettingsLoadFailed')}
            </p>
          ) : null}
        </div>
        <form className="composer-surface rounded-composer" onSubmit={send} ref={formRef}>
          <input
            accept={
              selectedModel?.supportsImages === true
                ? `${imageAttachmentExtensions},${textAttachmentExtensions}`
                : textAttachmentExtensions
            }
            aria-label={t('attachmentChooseFile')}
            className="sr-only"
            disabled={pickerModels.length === 0 || isSending || isReadingAttachments || isUpdatingConversationModel}
            multiple
            onChange={(event) => {
              const files = Array.from(event.currentTarget.files ?? []);
              event.currentTarget.value = '';
              if (files.length > 0) void addAttachments(files);
            }}
            ref={filePickerRef}
            type="file"
          />
          {pendingAttachments.length > 0 ? (
            <PendingAttachmentGallery
              attachments={pendingAttachments}
              disabled={isSending || isReadingAttachments}
              onRemove={removeAttachment}
            />
          ) : null}
          {!isTemporaryChat && composerMode !== 'normal' ? (
            <div className="mb-3 flex items-center gap-2">
              <Button
                aria-label={t(composerMode === 'plan' ? 'toolPermissionPlanOff' : 'composerGoalModeOff')}
                className="min-h-9"
                onClick={() => setComposerMode('normal')}
                size="small"
                variant="secondary"
              >
                <span>{t(composerMode === 'plan' ? 'toolPermissionPlan' : 'composerGoalModeLabel')}</span>
                <X aria-hidden="true" size={14} />
              </Button>
            </div>
          ) : null}
          <div className="relative">
            {slashModelMenuOpen ? (
              <ChatSlashModelMenu
                canSelect={!isSending && !isUpdatingConversationModel}
                isLoading={modelStatus === 'loading'}
                models={slashModelCandidates}
                onSelect={(model) => void selectSlashModel(model)}
                providerLabel={(model) => providerNameForModel(model, t)}
                selectedIndex={slashModelIndex}
              />
            ) : slashCommands.length > 0 ? (
              <ChatSlashCommandMenu
                commands={slashCommands}
                onSelect={selectSlashCommand}
                selectedIndex={slashCommandIndex}
              />
            ) : null}
            {messageDraft.length === 0 ? (
              <span
                aria-hidden="true"
                className="pointer-events-none absolute left-0 top-0 text-composer-input text-secondary-text"
              >
                {t('messageHint')}
              </span>
            ) : null}
            <textarea
              aria-activedescendant={
                slashModelMenuOpen
                  ? selectedSlashModel
                    ? `chat-slash-model-${encodeURIComponent(selectedSlashModel.key)}`
                    : undefined
                  : selectedSlashCommand
                    ? `chat-slash-command-${selectedSlashCommand}`
                    : undefined
              }
              aria-autocomplete={slashModelMenuOpen || slashCommands.length > 0 ? 'list' : 'none'}
              aria-controls={
                slashModelMenuOpen
                  ? 'chat-slash-model-list'
                  : slashCommands.length > 0
                    ? 'chat-slash-command-list'
                    : undefined
              }
              role="combobox"
              aria-expanded={slashModelMenuOpen || slashCommands.length > 0}
              aria-label={t('messageHint')}
              className="composer-message-input w-full bg-transparent text-composer-input text-text outline-none focus-visible:outline-none focus-visible:ring-0 focus-visible:ring-offset-0"
              disabled={pickerModels.length === 0 || isSending || isUpdatingConversationModel}
              id="chat-message"
              onChange={(event) => {
                const input = event.currentTarget;
                setMessageDraft(input.value);
                setSlashSuggestionsDismissed(slashModelMenuOpen);
                setSlashCommandIndex(0);
                if (slashModelMenuOpen) setSlashModelIndex(0);
                setCommandFeedbackKey(null);
                input.style.height = 'auto';
                input.style.height = `${input.scrollHeight}px`;
              }}
              onKeyDown={handleKeyDown}
              ref={inputRef}
              rows={1}
              value={messageDraft}
            />
          </div>
          <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <Button
                aria-label={t('attachmentChooseFile')}
                disabled={pickerModels.length === 0 || isSending || isReadingAttachments}
                onClick={() => filePickerRef.current?.click()}
                size="icon"
                type="button"
                variant="ghost"
              >
                <Paperclip aria-hidden="true" size={18} />
              </Button>
              <ModelPicker
                canSelectProvider={activeConversationId === null}
                showFastModeControl={!isTemporaryChat}
                fastModeAvailable={fastModeAvailable}
                fastModeEnabled={fastModeEnabled}
                fastModeLoading={fastModePreferenceState === 'loading'}
                fastModeSaving={fastModePreferenceState === 'saving'}
                showReasoningSelector={!isTemporaryChat && (selectedModel?.supportsReasoning ?? false)}
                reasoningDisabled={isSending || isUpdatingConversationModel}
                reasoningEffort={selectedReasoningEffort}
                reasoningLevels={selectedModel?.reasoningLevels ?? []}
                emptyModelsLabel={t(modelError ? 'modelsUnavailable' : 'noModelsAvailable')}
                hiddenModelKeys={hiddenModelKeys}
                isOpen={modelPickerOpen}
                knownModels={models}
                modelFavorites={modelFavorites}
                modelProviders={modelProviders}
                models={pickerModels}
                disabled={isSending || isUpdatingConversationModel}
                onOpenChange={(open) => {
                  setModelPickerOpen(open);
                }}
                onQueryChange={setModelQuery}
                onSelect={(model) => {
                  setModelPickerOpen(false);
                  setCommandFeedbackKey(null);
                  void selectConversationModel(model.key).then((updated) => {
                    if (!updated) setCommandFeedbackKey('slashCommandModelUnavailable');
                    else setCommandFeedbackKey(null);
                  });
                }}
                onRemoveFavorite={(favorite) => {
                  void removeModelFavorite(favorite);
                }}
                onSelectFavorite={selectFavoriteModel}
                onSelectProvider={() => {
                  if (activeConversationId === null) selectModel(null);
                  setCommandFeedbackKey(null);
                  setModelQuery('');
                }}
                onFastModeToggle={toggleFastMode}
                onSelectReasoningEffort={(value) => void selectReasoningEffort(value)}
                onToggleFavorite={(model) => {
                  void toggleFavoriteModel(model);
                }}
                query={modelQuery}
                selectedModel={selectedModel ?? null}
              />
              <ToolPermissionModeSelector onModeChange={setSelectedPermissionMode} />
            </div>
            {isSending ? (
              <Button
                aria-label={t('stop')}
                className="size-10"
                onClick={() => void stopResponse()}
                size="icon"
                variant="secondary"
              >
                <Square aria-hidden="true" fill="currentColor" size={14} />
              </Button>
            ) : (
              <Button
                aria-label={t('send')}
                className="size-10"
                disabled={
                  pickerModels.length === 0 ||
                  !selectedModelKey ||
                  messageDraft.trim().length === 0 ||
                  isReadingAttachments ||
                  isUpdatingConversationModel ||
                  isSendBlocked
                }
                size="icon"
                type="submit"
                variant="primary"
              >
                <ArrowUp aria-hidden="true" size={20} />
              </Button>
            )}
          </div>
        </form>
        {attachmentErrorKey ? (
          <p className="mt-2 text-sm text-destructive" role="alert">
            {t(attachmentErrorKey)}
          </p>
        ) : null}
        {commandFeedbackKey ? (
          <p
            className={`mt-2 text-sm ${successFeedback ? 'text-success' : 'text-destructive'}`}
            role={successFeedback ? 'status' : 'alert'}
          >
            {t(commandFeedbackKey)}
          </p>
        ) : null}
        {chatErrorCode && chatErrorCode !== 'searchMessageUnavailable' ? (
          <p className="mt-2 text-sm text-destructive" role="alert">
            {t(chatErrorCode)}
          </p>
        ) : null}
        {modelError ? (
          <p className="mt-2 text-sm text-warning" role="status">
            {t('modelCatalogSourcesUnavailable')}
          </p>
        ) : null}
      </div>
    </div>
  );
}

function effectiveContextWindow(model: ChatModel, limits: Record<string, number>): number | null {
  const configured = limits[modelRouteKey(model)];
  if (configured === undefined) return model.contextWindow;
  if (model.contextWindow === null) return configured;
  return Math.min(configured, model.contextWindow);
}

type TranslationKey = keyof typeof import('@/localization/locales/en.json');

const providerTranslationKeys: Partial<Record<string, TranslationKey>> = {
  opencode: 'openCodeProvider',
  copilot: 'providerNameCopilot',
  chatgpt: 'chatGptProvider',
  chatgpt_api: 'chatGptProvider',
  gemini: 'geminiProvider',
  groq: 'groqProvider',
  cerebras: 'cerebrasProvider',
  openrouter: 'openRouterProvider',
  mistral: 'mistralProvider',
  deepseek: 'deepSeekProvider',
  xai: 'xAiProvider',
  perplexity: 'perplexityProvider',
  together: 'togetherProvider',
  sambanova: 'sambaNovaProvider',
  anthropic: 'anthropicProvider',
  fireworks: 'fireworksProvider',
  deepinfra: 'deepInfraProvider',
  dashscope: 'dashScopeProvider',
  cohere: 'cohereProvider',
  llama_cpp: 'localEngines',
};

const imageAttachmentExtensions = '.png,.jpg,.jpeg,.webp';
const textAttachmentExtensions =
  '.txt,.md,.csv,.json,.jsonl,.xml,.html,.css,.js,.jsx,.ts,.tsx,.dart,.rs,.py,.go,.java,.kt,.swift,.c,.h,.cpp,.hpp,.cs,.sh,.ps1,.yaml,.yml,.toml,.ini,.sql,.log';

function isMessageSendingCommand(command: ChatSlashCommand): boolean {
  return command === 'summarize' || command === 'compact' || command === 'plan' || command === 'goal';
}

function providerNameForModel(model: ChatModel, t: (key: TranslationKey) => string): string {
  if (model.customProvider?.name) return model.customProvider.name;
  return providerNameForId(model.providerId, t);
}

function providerNameForId(providerId: string, t: (key: TranslationKey) => string): string {
  const key = providerTranslationKeys[providerId];
  return key ? t(key) : providerId;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parseSlashCommand(value: string): { command: ChatSlashCommand; arguments: string } | null {
  const match = /^\/(new|search|summarize|rename|model|compact|reasoning|plan|goal|hedef)(?:\s+([\s\S]*))?$/i.exec(
    value.trim(),
  );
  if (!match) return null;
  const name = match[1]?.toLocaleLowerCase('en');
  if (!name) return null;
  let command: ChatSlashCommand;
  switch (name) {
    case 'new':
      command = 'new';
      break;
    case 'search':
      command = 'search';
      break;
    case 'summarize':
      command = 'summarize';
      break;
    case 'rename':
      command = 'rename';
      break;
    case 'model':
      command = 'model';
      break;
    case 'compact':
      command = 'compact';
      break;
    case 'reasoning':
      command = 'reasoning';
      break;
    case 'plan':
      command = 'plan';
      break;
    case 'goal':
    case 'hedef':
      command = 'goal';
      break;
    default:
      return null;
  }
  return { command, arguments: match[2] ?? '' };
}
