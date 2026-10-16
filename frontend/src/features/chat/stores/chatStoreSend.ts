import type { StoreApi } from 'zustand';

import {
  createConversation,
  listConversations,
  saveMessage,
  type ChatConversation,
  type ChatMessage,
} from '@/features/chat/data/chatStorage';
import { isTerminalChatGoalStatus, parseChatGoal } from '@/features/chat/data/chatGoals';
import { loadRouteCredentials } from '@/features/chat/data/modelRouteCredentials';
import { getActiveGlobalMcpInstructions } from '@/features/library/data/mcpStorage';
import { notifyAssistantResponse, notifyPendingQuestion } from '@/features/settings/data/notificationRuntime';
import { readSharedInstructions } from '@/features/settings/data/sharedInstructions';
import {
  personalMemoryContextForConversation,
  setPersonalMemoryExcludedForConversation,
} from '@/features/settings/data/personalMemory';
import {
  type ProjectToolPermissionRule,
  readProjectSkillIds,
  readProjectToolPermissionRules,
  readToolPermissionMode,
  type ToolPermissionMode,
} from '@/features/chat/data/projectPreferences';
import { modelRouteKey } from '@/features/settings/data/modelPreferences';
import {
  writeProviderRequestOutcomeRecord,
  type ProviderRequestOutcome,
} from '@/features/settings/data/providerRequestOutcome';
import {
  applyStreamEvent,
  numberOrNull,
  titleFromMessage,
  upsertMessage,
} from './chatStoreUtils';
import {
  LocalServiceError,
  type LocalServiceClient,
} from '@/shared/services/LocalServiceClient';
import type { ChatState } from './chatStore';
import { handleToolPermissionEvent } from './chatStoreToolPermissions';
import { loadTitleGenerationRequest } from './chatStoreTitleGeneration';

type ChatStoreSetState = StoreApi<ChatState>['setState'];
type ChatStoreGetState = StoreApi<ChatState>['getState'];

export function createSendMessageAction(
  set: ChatStoreSetState,
  get: ChatStoreGetState,
  localServiceClient: LocalServiceClient,
): ChatState['sendMessage'] {
  return async (rawContent, attachments = [], resumeRunId, resumeAssistantMessageId, options) => {
    const content = rawContent.trim();
    const isRetry = options?.excludedAssistantMessageId !== undefined;
    const userMessageAlreadySaved = options?.userMessageAlreadySaved === true;
    const state = get();
    const activeConversationId = state.activeConversationId;
    const pendingQuestionBlocksSend = resumeRunId === undefined &&
      activeConversationId !== null &&
      state.pendingQuestionSendBlock?.conversationId === activeConversationId &&
      state.pendingQuestionSendBlock.blocked;
    const conversationOperationBlocksSend = activeConversationId !== null && (
      state.compactingConversationId === activeConversationId ||
      state.isCreatingMessageBranch
    );
    if (
      (!content && attachments.length === 0 && !resumeRunId && !isRetry && !userMessageAlreadySaved) ||
      state.isSending ||
      state.isUpdatingConversationModel ||
      pendingQuestionBlocksSend ||
      conversationOperationBlocksSend
    ) return false;
    const goalObjective = options?.mode === 'goal' && !resumeRunId ? content : null;
    if (options?.mode === 'goal' && !resumeRunId && !content) {
      set({ errorCode: 'goalObjectiveRequired' });
      return false;
    }
    if (goalObjective && new TextEncoder().encode(goalObjective).length > 16 * 1024) {
      set({ errorCode: 'goalObjectiveTooLong' });
      return false;
    }
    const activeConversation = state.activeConversationId === null
      ? undefined
      : state.conversations.find((conversation) => conversation.id === state.activeConversationId);
    const projectId = activeConversation?.projectId ?? state.pendingProjectId;
    if (goalObjective && state.activeConversationId !== null) {
      try {
        const response = await localServiceClient.call('chat.goal.active', {
          conversationId: state.activeConversationId,
        });
        if (!('goal' in response)) throw new Error('The active goal response is invalid.');
        const existingGoal = parseChatGoal(response.goal);
        if (existingGoal) {
          set({ activeGoal: existingGoal, goalStatus: 'ready', errorCode: 'goalAlreadyActive' });
          return false;
        }
      } catch {
        set({ goalStatus: 'error', errorCode: 'goalStateUnavailable' });
        return false;
      }
    }
    let projectSkillIds: string[] = [];
    let projectToolRules: Record<string, ProjectToolPermissionRule> = {};
    let toolPermissionMode: ToolPermissionMode | 'plan';
    try {
      toolPermissionMode = options?.mode === 'plan' ? 'plan' : readToolPermissionMode();
    } catch {
      set({ errorCode: 'toolPermissionModeLoadFailed' });
      return false;
    }
    if (projectId !== null) {
      try {
        projectSkillIds = readProjectSkillIds(projectId);
      } catch {
        set({ errorCode: 'projectSkillsLoadFailed' });
        return false;
      }
      try {
        projectToolRules = readProjectToolPermissionRules(projectId);
      } catch {
        set({ errorCode: 'projectToolRulesLoadFailed' });
        return false;
      }
    }
    const selectedModel = state.models.find((model) => model.key === state.selectedModelKey);
    if (!selectedModel) {
      set({ errorCode: 'modelRequired' });
      return false;
    }
    if (attachments.some((attachment) => attachment.kind === 'image') && !selectedModel.supportsImages) {
      set({ errorCode: 'modelDoesNotSupportImages' });
      return false;
    }
    if (resumeRunId && !state.activeConversationId) {
      set({ errorCode: 'userQuestionUnavailable' });
      return false;
    }

    set({ isSending: true, errorCode: null });
    let conversationId = state.activeConversationId;
    const excludePersonalMemory = conversationId === null && state.personalMemoryExcludedForNewConversation;
    let userMessageSaved = resumeRunId !== undefined || userMessageAlreadySaved || isRetry;
    let personalMemoryFactIds: string[] = [];
    const resumedMessage = resumeAssistantMessageId
      ? state.messages.find((message) => message.id === resumeAssistantMessageId) ?? null
      : null;
    const assistantMessage: { current: ChatMessage | null } = { current: resumedMessage };
    let requestOutcome: ProviderRequestOutcome | null = null;
    try {
      const routeCredentials = await loadRouteCredentials(selectedModel);
      if (!conversationId && !resumeRunId) {
        conversationId = crypto.randomUUID().replaceAll('-', '');
        const conversationTitle = titleFromMessage(content, attachments[0]?.name);
        await createConversation(
          conversationId,
          conversationTitle,
          selectedModel,
          state.pendingProjectId,
        );
        const now = Date.now();
        const conversation: ChatConversation = {
          id: conversationId,
          title: conversationTitle,
          titleSource: 'automatic',
          providerId: selectedModel.providerId,
          modelId: selectedModel.modelId,
          connectionId: selectedModel.connectionId ?? null,
          workspaceId: selectedModel.workspaceId ?? null,
          apiKeyConnectionId: selectedModel.apiKeyConnectionId ?? null,
          projectId: state.pendingProjectId,
          tags: [],
          isPinned: false,
          isArchived: false,
          isBookmarked: false,
          createdAt: now,
          updatedAt: now,
        };
        set((current) => ({
          activeConversationId: conversationId,
          pendingProjectId: null,
          conversations: [conversation, ...current.conversations],
          messages: [],
          personalMemoryExcludedForNewConversation: false,
        }));
      }
      const targetConversationId = conversationId;
      if (!targetConversationId) {
        throw new LocalServiceError(
          'conversation_unavailable',
          'The conversation could not be opened.',
        );
      }

      let personalMemoryContext = '';
      if (!resumeRunId) {
        try {
          if (excludePersonalMemory) {
            setPersonalMemoryExcludedForConversation(targetConversationId, true);
          }
          const personalMemory = personalMemoryContextForConversation(targetConversationId, excludePersonalMemory);
          personalMemoryContext = personalMemory.context;
          personalMemoryFactIds = personalMemory.factIds;
        } catch {
          set({ errorCode: 'personalMemoryLoadFailed' });
          return false;
        }
      }

      if (!resumeRunId && !userMessageAlreadySaved && !isRetry) {
        const userMessage: ChatMessage = {
          id: crypto.randomUUID().replaceAll('-', ''),
          conversationId: targetConversationId,
          role: 'user',
          content,
          attachments: attachments.map(({ id, name, mimeType, sizeBytes, kind }) => ({ id, name, mimeType, sizeBytes, kind })),
          createdAt: Date.now(),
          outputTokens: null,
          tokensPerSecond: null,
          elapsedMicroseconds: null,
          providerId: selectedModel.providerId,
          modelId: selectedModel.modelId,
          citationSources: '[]',
          personalMemoryFactIds: JSON.stringify(personalMemoryFactIds),
          reasoningSummaries: '[]',
          toolActivities: '[]',
          status: 'completed',
          failureCode: null,
        };
        await saveMessage(userMessage, attachments);
        userMessageSaved = true;
        set((current) => ({ messages: [...current.messages, userMessage] }));
      }

      let persistenceError = false;
      let persistenceQueue = Promise.resolve();
      const [sharedInstructions, mcpInstructions] = await Promise.all([
        Promise.resolve(readSharedInstructions()),
        getActiveGlobalMcpInstructions(),
      ]);
      const customInstructions = [sharedInstructions?.trim(), personalMemoryContext, mcpInstructions.trim()]
        .filter((value): value is string => Boolean(value))
        .join('\n\n');
      const contextLimit = state.modelContextLimits[modelRouteKey(selectedModel)];
      const titleGeneration = await loadTitleGenerationRequest(state.models);
      const operation = await localServiceClient.startOperation(
        'chat.send',
        {
          conversationId: targetConversationId,
          ...(resumeRunId ? { resumeRunId } : {}),
          ...(isRetry ? { excludedAssistantMessageId: options?.excludedAssistantMessageId } : {}),
          ...(goalObjective ? { goal: true, goalObjective } : {}),
          ...(options?.reasoningEffort ? { reasoningEffort: options.reasoningEffort } : {}),
          ...(options?.fastMode === true ? { fastMode: true } : {}),
          ...routeCredentials,
          ...(customInstructions ? { customInstructions } : {}),
          titleGeneration,
          ...(projectSkillIds.length === 0 ? {} : { projectSkillIds }),
          toolPermissionMode,
          ...(Object.keys(projectToolRules).length === 0 ? {} : { toolPermissionRules: projectToolRules }),
          ...(contextLimit === undefined ? {} : { contextLimit }),
        },
        (event) => {
          handleToolPermissionEvent(event, set, get);
          if (event.name === 'chat.goal.updated') {
            try {
              const goal = parseChatGoal(event.data);
              if (!goal || goal.conversationId !== targetConversationId) throw new Error('The goal event is invalid.');
              if (get().activeConversationId === targetConversationId) {
                set({
                  activeGoal: isTerminalChatGoalStatus(goal.status) ? null : goal,
                  goalStatus: 'ready',
                });
              }
            } catch {
              if (get().activeConversationId === targetConversationId) set({ goalStatus: 'error' });
            }
          }
          if (event.name === 'chat.question.requested') {
            window.dispatchEvent(new CustomEvent('openchat-question-requested', { detail: event.data }));
            void notifyPendingQuestion(event.data).catch(() => {
              window.dispatchEvent(new CustomEvent('openchat-notification-error'));
            });
          }
          const message = applyStreamEvent(event, assistantMessage.current, {
            conversationId: targetConversationId,
            providerId: selectedModel.providerId,
            modelId: selectedModel.modelId,
          });
          if (!message) return;
          assistantMessage.current = message;
          set((current) => ({ messages: upsertMessage(current.messages, message) }));
          if (!isRetry) {
            persistenceQueue = persistenceQueue.then(async () => {
              try {
                await saveMessage(message);
              } catch {
                persistenceError = true;
              }
            });
          }
        },
      );
      set({ activeOperation: operation });
      const result = await operation.result;
      await persistenceQueue;
      if (persistenceError) {
        throw new LocalServiceError(
          'message_save_failed',
          'The assistant response could not be saved.',
        );
      }

      const resultStatus = result.status;
      const finalStatus = resultStatus === 'completed' || resultStatus === 'paused'
        ? 'completed'
        : resultStatus === 'stopped'
          ? 'stopped'
          : null;
      if (!finalStatus) {
        throw new LocalServiceError(
          'invalid_service_response',
          'The local service returned an invalid response status.',
        );
      }
      requestOutcome = resultStatus === 'completed'
        ? 'completed'
        : resultStatus === 'paused'
          ? 'waitingForInput'
          : 'stopped';
      if (isRetry && finalStatus === 'stopped') set({ errorCode: 'responseRetryNotCompleted' });
      const streamedMessage = assistantMessage.current;
      if (streamedMessage) {
        const completedMessage: ChatMessage = {
          ...streamedMessage,
          personalMemoryFactIds: resumeRunId
            ? streamedMessage.personalMemoryFactIds
            : JSON.stringify(personalMemoryFactIds),
          status: finalStatus,
          outputTokens: numberOrNull(result.outputTokens) ?? streamedMessage.outputTokens,
          elapsedMicroseconds:
            numberOrNull(result.elapsedMicroseconds) ?? streamedMessage.elapsedMicroseconds,
          tokensPerSecond:
            numberOrNull(result.tokensPerSecond) ?? streamedMessage.tokensPerSecond,
        };
        assistantMessage.current = completedMessage;
        if (finalStatus === 'completed') await saveMessage(completedMessage);
        if (isRetry && finalStatus !== 'completed') {
          set((current) => ({ messages: current.messages.filter((message) => message.id !== completedMessage.id) }));
        }
        set((current) => ({
          messages: finalStatus === 'completed'
            ? upsertMessage(current.messages, completedMessage)
            : current.messages,
          hasCompletedAssistantResponse:
            finalStatus === 'completed' || current.hasCompletedAssistantResponse === true,
        }));
      }
      const conversations = await listConversations();
      set({ conversations });
      if (finalStatus === 'completed' && streamedMessage) {
        const conversation = conversations.find((item) => item.id === targetConversationId);
        if (conversation) {
          const completedMessage = assistantMessage.current;
          if (completedMessage) {
            void notifyAssistantResponse(conversation, completedMessage).catch(() => {
              window.dispatchEvent(new CustomEvent('openchat-notification-error'));
            });
          }
        }
      }
      return true;
    } catch (error) {
      requestOutcome ??= providerRequestOutcomeForError(error);
      const code = !userMessageSaved && attachments.length > 0
        ? 'attachmentSaveFailed'
        : !userMessageSaved
          ? 'messageSaveFailed'
          : error instanceof LocalServiceError && error.code === 'message_save_failed'
            ? 'messageSaveFailed'
            : error instanceof LocalServiceError && error.code === 'invalid_request_params'
              ? 'modelRequired'
              : error instanceof LocalServiceError && error.code === 'provider_credentials_missing'
                ? 'providerCredentialMissing'
                : 'chatRequestFailed';
      set({ errorCode: code });
      const streamedMessage = assistantMessage.current;
      if (streamedMessage) {
        const failedMessage: ChatMessage = {
          ...streamedMessage,
          personalMemoryFactIds: resumeRunId
            ? streamedMessage.personalMemoryFactIds
            : JSON.stringify(personalMemoryFactIds),
          status: 'failed',
          failureCode: error instanceof LocalServiceError ? error.code : 'service_error',
        };
        assistantMessage.current = failedMessage;
        if (isRetry) {
          set((current) => ({ messages: current.messages.filter((message) => message.id !== failedMessage.id) }));
        } else {
          set((current) => ({ messages: upsertMessage(current.messages, failedMessage) }));
          try {
            await saveMessage(failedMessage);
          } catch {
            set({ errorCode: 'messageSaveFailed' });
          }
        }
      }
      return userMessageSaved;
    } finally {
      if (requestOutcome !== null) {
        try {
          writeProviderRequestOutcomeRecord(
            {
              providerId: selectedModel.providerId === 'chatgpt'
                ? 'chatgpt_oauth'
                : selectedModel.providerId,
              connectionId: selectedModel.apiKeyConnectionId ?? selectedModel.connectionId ?? selectedModel.providerId,
              workspaceId: selectedModel.providerId === 'chatgpt'
                ? selectedModel.workspaceId ?? null
                : null,
            },
            { outcome: requestOutcome, occurredAtUnixMs: Date.now() },
          );
        } catch {
          set({ errorCode: 'providerLastRequestSaveFailed' });
        }
      }
      set({
        isSending: false,
        activeOperation: null,
        pendingToolPermissionRequest: null,
        isRespondingToToolPermission: false,
      });
      const currentGoal = get().activeGoal;
      if (
        conversationId &&
        (goalObjective !== null || resumeRunId !== undefined || currentGoal?.conversationId === conversationId)
      ) {
        await get().refreshActiveGoal(conversationId);
      }
    }
  };
}

function providerRequestOutcomeForError(error: unknown): ProviderRequestOutcome {
  if (!(error instanceof LocalServiceError)) return 'failed';
  switch (error.code) {
    case 'operation_cancelled':
    case 'request_cancelled':
      return 'stopped';
    case 'authentication_required':
    case 'refresh_rejected':
    case 'invalid_api_key':
      return 'authenticationFailed';
    case 'rate_limited':
    case 'quota_exceeded':
    case 'insufficient_quota':
    case 'resource_exhausted':
      return 'rateLimited';
    case 'network_unavailable':
    case 'request_timeout':
    case 'provider_timeout':
      return 'networkFailed';
    default:
      return 'failed';
  }
}
