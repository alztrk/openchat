import type { StoreApi } from 'zustand';

import { isTerminalChatGoalStatus, parseChatGoal } from '@features/chat/data/chatGoals';
import {
  type ChatConversation,
  type ChatMessage,
  createConversation,
  listConversations,
  recordChatMessagePersistenceFailure,
  recordChatRequestFailure,
  saveMessage,
} from '@features/chat/data/chatStorage';
import { notifyAssistantResponse, notifyPendingQuestion } from '@features/notifications/data/notificationRuntime';
import {
  loadProjectToolPermissionRules,
  loadToolPermissionMode,
} from '@features/projects/data/projectPermissionSettings';
import { readProjectSkillIds } from '@features/projects/data/projectPreferences';
import { modelRouteKey } from '@features/provider-settings/data/modelPreferences';
import { loadRouteParameters } from '@features/provider-settings/data/modelRouteParameters';
import { recordOAuthRateLimit } from '@features/provider-settings/data/oauthRateLimits';
import {
  type ProviderRequestOutcome,
  writeProviderRequestOutcomeRecord,
} from '@features/provider-settings/data/providerRequestOutcome';
import { readAssistantExecutionPreferences } from '@features/app-settings/data/assistantExecutionPreferences';
import {
  personalMemoryContextForConversation,
  setPersonalMemoryExcludedForConversation,
} from '@features/app-settings/data/personalMemory';
import { readSharedInstructions } from '@features/app-settings/data/sharedInstructions';
import { type LocalServiceClient, LocalServiceError } from '@shared/lib/platform/local-service';
import { getAudioContext, playAssistantResponseAudio } from '@features/notifications/audioFeedback';
import type { ChatState } from './chatStore';
import { getOpenCodeRetryAtUnixMs, serviceErrorCategoryMessageKey } from './chatStoreErrorDetails';
import { chatStorageRetryDelay } from '../data/chatStorageRetry';
import { refreshConversationListAfterResponse } from './chatStorePostResponse';
import { loadTitleGenerationRequest } from './chatStoreTitleGeneration';
import { handleToolPermissionEvent } from './chatStoreToolPermissions';
import { applyStreamEvent, isSameRoute, numberOrNull, titleFromMessage, upsertMessage } from './chatStoreUtils';
import { createStreamMessagePublisher } from './streamMessagePublisher';

const STREAM_PERSISTENCE_CHECKPOINT_BYTES = 256 * 1024;
const streamTextEncoder = new TextEncoder();
const CHAT_STORAGE_DETAIL_CODES = new Set([
  'database_queue_full',
  'database_queue_wait_timeout',
  'invalid_query',
  'message_content_too_large',
  'message_metadata_too_large',
  'sqlite_busy',
  'sqlite_busy_snapshot',
  'sqlite_busy_timeout',
  'sqlite_cannot_open',
  'sqlite_corrupt',
  'sqlite_disk_full',
  'sqlite_failure',
  'sqlite_io_error',
  'sqlite_locked',
  'sqlite_permission_denied',
  'sqlite_schema_changed',
  'sqlite_5',
  'sqlite_517',
  'storage_broker_timeout',
  'storage_conversion_failed',
  'storage_operation_failed',
  'stored_value_invalid',
]);

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
    let conversationId = options?.conversationId ?? state.activeConversationId;
    let conversationCreated = conversationId !== null;
    const isRequestConversationActive = () =>
      conversationCreated
        ? get().activeConversationId === conversationId
        : get().activeConversationId === state.activeConversationId;
    const setRequestError = (
      errorCode: ChatState['errorCode'],
      errorMessage: string | null = null,
      opencodeRetryAtUnixMs: number | null = null,
    ) => {
      if (isRequestConversationActive()) set({ errorCode, errorMessage, opencodeRetryAtUnixMs });
    };
    const targetConvId = conversationId;
    const pendingQuestionBlocksSend =
      resumeRunId === undefined &&
      targetConvId !== null &&
      state.pendingQuestionSendBlock?.conversationId === targetConvId &&
      state.pendingQuestionSendBlock.blocked;
    const conversationOperationBlocksSend =
      targetConvId !== null && (state.compactingConversationId === targetConvId || state.isCreatingMessageBranch);
    if (
      (!content && attachments.length === 0 && !resumeRunId && !isRetry && !userMessageAlreadySaved) ||
      state.isUpdatingConversationModel ||
      (targetConvId !== null && state.sendingConversationIds.includes(targetConvId)) ||
      pendingQuestionBlocksSend ||
      conversationOperationBlocksSend
    )
      return false;
    getAudioContext();
    const goalObjective = options?.mode === 'goal' && !resumeRunId ? content : null;
    if (options?.mode === 'goal' && !resumeRunId && !content) {
      set({ errorCode: 'goalObjectiveRequired' });
      return false;
    }
    if (goalObjective && new TextEncoder().encode(goalObjective).length > 16 * 1024) {
      set({ errorCode: 'goalObjectiveTooLong' });
      return false;
    }
    const targetConversation =
      conversationId === null
        ? undefined
        : state.conversations.find((conversation) => conversation.id === conversationId);
    const projectId = targetConversation?.projectId ?? state.pendingProjectId;
    let projectSkillIds: string[] = [];
    let assistantExecutionPreferences: ReturnType<typeof readAssistantExecutionPreferences>;
    try {
      assistantExecutionPreferences = readAssistantExecutionPreferences();
    } catch {
      set({ errorCode: 'assistantPreferencesLoadFailed' });
      return false;
    }
    try {
      await loadToolPermissionMode();
    } catch {
      set({ errorCode: 'toolPermissionModeLoadFailed' });
      return false;
    }
    if (projectId !== null && assistantExecutionPreferences.includeProjectSkills) {
      try {
        projectSkillIds = readProjectSkillIds(projectId);
      } catch {
        set({ errorCode: 'projectSkillsLoadFailed' });
        return false;
      }
      try {
        await loadProjectToolPermissionRules(projectId);
      } catch {
        set({ errorCode: 'projectToolRulesLoadFailed' });
        return false;
      }
    }
    const selectedModel = options?.conversationId
      ? state.models.find((model) => targetConversation && isSameRoute(model, targetConversation))
      : state.models.find((model) => model.key === state.selectedModelKey);
    if (!selectedModel) {
      set({ errorCode: 'modelRequired' });
      return false;
    }
    if (attachments.some((attachment) => attachment.kind === 'image') && !selectedModel.supportsImages) {
      set({ errorCode: 'modelDoesNotSupportImages' });
      return false;
    }
    if (resumeRunId && !conversationId) {
      set({ errorCode: 'userQuestionUnavailable' });
      return false;
    }

    set((current) => ({
      isSending: true,
      sendingConversationId: conversationId ?? current.sendingConversationId,
      sendingConversationIds: conversationId && !current.sendingConversationIds.includes(conversationId)
        ? [...current.sendingConversationIds, conversationId]
        : current.sendingConversationIds,
      interruptedRun:
        resumeRunId !== undefined && current.interruptedRun?.runId === resumeRunId
          ? null
          : current.interruptedRun,
      errorCode: null,
      errorMessage: null,
      opencodeRetryAtUnixMs: null,
    }));
    if (goalObjective && conversationId !== null) {
      try {
        const response = await localServiceClient.call('chat.goal.active', {
          conversationId,
        });
        if (!('goal' in response)) throw new Error('The active goal response is invalid.');
        const existingGoal = parseChatGoal(response.goal);
        if (existingGoal) {
          set((current) => ({
            isSending: current.sendingConversationIds.some((id) => id !== conversationId),
            sendingConversationIds: current.sendingConversationIds.filter((id) => id !== conversationId),
            sendingConversationId: current.sendingConversationIds.find((id) => id !== conversationId) ?? null,
          }));
          if (isRequestConversationActive()) {
            set({ activeGoal: existingGoal, goalStatus: 'ready', errorCode: 'goalAlreadyActive' });
          }
          return false;
        }
      } catch {
        set((current) => ({
          isSending: current.sendingConversationIds.some((id) => id !== conversationId),
          sendingConversationIds: current.sendingConversationIds.filter((id) => id !== conversationId),
          sendingConversationId: current.sendingConversationIds.find((id) => id !== conversationId) ?? null,
        }));
        if (isRequestConversationActive()) set({ goalStatus: 'error', errorCode: 'goalStateUnavailable' });
        return false;
      }
    }
    const excludePersonalMemory = conversationId === null && state.personalMemoryExcludedForNewConversation;
    let userMessageSaved = resumeRunId !== undefined || userMessageAlreadySaved || isRetry;
    let sendStage: 'conversationCreation' | 'userMessagePersistence' | 'requestExecution' = 'requestExecution';
    let personalMemoryFactIds: string[] = [];
    const resumedMessage = resumeAssistantMessageId
      ? (state.messages.find((message) => message.id === resumeAssistantMessageId) ?? null)
      : null;
    const assistantMessage: { current: ChatMessage | null } = { current: resumedMessage };
    const streamUi = createStreamMessagePublisher((message) => {
      if (get().activeConversationId === message.conversationId) {
        set((current) => ({ messages: upsertMessage(current.messages, message) }));
      }
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('openchat-message-stream', { detail: { message } }));
      }
    });
    let requestOutcome: ProviderRequestOutcome | null = null;
    let checkpointFailureLogged = false;
    let unpersistedTextBytes = 0;
    let persistenceQueue = Promise.resolve();
    let pendingPersistenceMessage: ChatMessage | null = null;
    let persistenceWorkerRunning = false;
    const enqueueLatestMessageSave = (message: ChatMessage): void => {
      pendingPersistenceMessage = message;
      if (persistenceWorkerRunning) return;
      persistenceWorkerRunning = true;
      persistenceQueue = persistenceQueue.then(async () => {
        // Coalesce snapshots while the asynchronous native write is in flight.
        while (pendingPersistenceMessage) {
          const pendingMessage = pendingPersistenceMessage;
          pendingPersistenceMessage = null;
          try {
            await saveMessageWithRetry(pendingMessage, [], { preserveToolActivities: true });
          } catch (error) {
            if (!checkpointFailureLogged) {
              checkpointFailureLogged = true;
              await logMessagePersistenceFailure('assistantCheckpoint', error, pendingMessage, [], false);
            }
          }
        }
        persistenceWorkerRunning = false;
      });
    };
    try {
      const routeParameters = loadRouteParameters(selectedModel);
      if (!conversationId && !resumeRunId) {
        conversationId = crypto.randomUUID().replaceAll('-', '');
        const conversationTitle = titleFromMessage(content, attachments[0]?.name);
        const agentSnapshot = state.temporaryChat === null ? state.pendingAgentSnapshot : null;
        sendStage = 'conversationCreation';
        try {
          await createConversation(
            conversationId,
            conversationTitle,
            selectedModel,
            state.pendingProjectId,
            agentSnapshot,
            state.selectedReasoningEffort,
          );
        } catch (cause) {
          globalThis.reportError(new Error('The new conversation could not be persisted.', { cause }));
          throw cause;
        }
        conversationCreated = true;
        const createdConversationId = conversationId;
        const now = Date.now();
        const conversation: ChatConversation = {
          id: createdConversationId,
          title: conversationTitle,
          titleSource: 'automatic',
          providerId: selectedModel.providerId,
          modelId: selectedModel.modelId,
          connectionId: selectedModel.connectionId ?? null,
          workspaceId: selectedModel.workspaceId ?? null,
          apiKeyConnectionId: selectedModel.apiKeyConnectionId ?? null,
          projectId: state.pendingProjectId,
          agent: agentSnapshot,
          reasoningEffort: state.selectedReasoningEffort,
          isPinned: false,
          isArchived: false,
          createdAt: now,
          updatedAt: now,
        };
        set((current) => {
          const shouldActivate = current.activeConversationId === state.activeConversationId;
          return {
            activeConversationId: shouldActivate ? createdConversationId : current.activeConversationId,
            sendingConversationId: createdConversationId,
            sendingConversationIds: current.sendingConversationIds.includes(createdConversationId)
              ? current.sendingConversationIds
              : [...current.sendingConversationIds, createdConversationId],
            pendingProjectId: shouldActivate ? null : current.pendingProjectId,
            pendingAgentSnapshot: shouldActivate ? null : current.pendingAgentSnapshot,
            conversations: [conversation, ...current.conversations],
            messages: shouldActivate ? [] : current.messages,
            personalMemoryExcludedForNewConversation: shouldActivate
              ? false
              : current.personalMemoryExcludedForNewConversation,
          };
        });
      }
      const targetConversationId = conversationId;
      if (!targetConversationId) {
        throw new LocalServiceError('conversation_unavailable', 'The conversation could not be opened.');
      }

      let personalMemoryContext = '';
      if (!resumeRunId) {
        try {
          if (excludePersonalMemory) {
            setPersonalMemoryExcludedForConversation(targetConversationId, true);
          }
          const personalMemory = await personalMemoryContextForConversation(
            targetConversationId,
            excludePersonalMemory,
          );
          personalMemoryContext = personalMemory.context;
          personalMemoryFactIds = personalMemory.factIds;
        } catch {
          setRequestError('personalMemoryLoadFailed');
          return false;
        }
      }

      if (!resumeRunId && !userMessageAlreadySaved && !isRetry) {
        const userMessage: ChatMessage = {
          id: crypto.randomUUID().replaceAll('-', ''),
          conversationId: targetConversationId,
          role: 'user',
          content,
          attachments: attachments.map(({ id, name, mimeType, sizeBytes, kind }) => ({
            id,
            name,
            mimeType,
            sizeBytes,
            kind,
          })),
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
        sendStage = 'userMessagePersistence';
        try {
          await saveMessageWithRetry(userMessage, attachments);
        } catch (cause) {
          await logMessagePersistenceFailure('userMessage', cause, userMessage, attachments);
          globalThis.reportError(new Error('The initial user message could not be persisted.', { cause }));
          throw cause;
        }
        userMessageSaved = true;
        if (get().activeConversationId === targetConversationId) {
          set((current) => ({ messages: [...current.messages, userMessage] }));
        }
        options?.onUserMessageCommitted?.(targetConversationId);
      }

      const sharedInstructions = readSharedInstructions();
      const storedConversation = get().conversations.find((item) => item.id === targetConversationId);
      const assistantProfile = state.temporaryChat === null ? (storedConversation?.agent ?? null) : null;
      const profileInstructions = assistantProfile
        ? [
            `Selected assistant agent: ${assistantProfile.name}.`,
            'The following agent instructions supplement OpenChat’s built-in assistant, tool, safety, and permission instructions. They do not replace or weaken those instructions.',
            `Response style: ${assistantProfile.responseStyle}.`,
            assistantProfile.instructions.trim(),
          ]
            .filter(Boolean)
            .join('\n\n')
        : null;
      const customInstructions = [sharedInstructions?.trim(), profileInstructions, personalMemoryContext]
        .filter((value): value is string => Boolean(value))
        .join('\n\n');
      const contextLimit = state.modelContextLimits[modelRouteKey(selectedModel)];
      const titleGeneration = await loadTitleGenerationRequest(state.models);
      sendStage = 'requestExecution';
      window.dispatchEvent(
        new CustomEvent('openchat-chat-request-started', { detail: { conversationId: targetConversationId } }),
      );
      const operation = await localServiceClient.startOperation(
        'chat.send',
        {
          conversationId: targetConversationId,
          ...(resumeRunId ? { resumeRunId } : {}),
          ...(isRetry ? { excludedAssistantMessageId: options?.excludedAssistantMessageId } : {}),
          ...(goalObjective ? { goal: true, goalObjective } : {}),
          ...(options?.reasoningEffort ? { reasoningEffort: options.reasoningEffort } : {}),
          ...(options?.fastMode === true ? { fastMode: true } : {}),
          ...routeParameters,
          ...(customInstructions ? { customInstructions } : {}),
          ...(assistantProfile?.toolAllowlist === null || assistantProfile === null
            ? {}
            : { profileToolAllowlist: assistantProfile.toolAllowlist }),
          titleGeneration,
          ...(projectSkillIds.length === 0 ? {} : { projectSkillIds }),
          planMode: options?.mode === 'plan',
          ...(contextLimit === undefined ? {} : { contextLimit }),
          availableSubagentModels: state.models.map((model) => ({
            key: model.key,
            providerId: model.providerId,
            modelId: model.modelId,
            displayName: model.displayName,
            ...(model.connectionId ? { connectionId: model.connectionId } : {}),
            ...(model.workspaceId ? { workspaceId: model.workspaceId } : {}),
            ...(model.apiKeyConnectionId ? { apiKeyConnectionId: model.apiKeyConnectionId } : {}),
            supportsTools: model.supportsTools ?? null,
            reasoningLevels: model.reasoningLevels,
            preferred:
              assistantExecutionPreferences.preferredDelegationModelRouteKey !== null &&
              modelRouteKey(model) === assistantExecutionPreferences.preferredDelegationModelRouteKey,
            requestProtocol:
              model.requestProtocol ??
              (model.providerId === 'chatgpt' || model.providerId === 'chatgpt_api'
                ? 'responses'
                : model.customProvider?.protocol === 'anthropic' || model.providerId === 'anthropic'
                  ? 'messages'
                  : 'chatCompletions'),
          })),
        },
        (event) => {
          handleToolPermissionEvent(event, targetConversationId, set, get);
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
          if (event.name === 'chat.title_updated') {
            const data = event.data;
            if (typeof data.conversationId === 'string' && typeof data.title === 'string') {
              get().applyTitleUpdate(data.conversationId, data.title);
            }
          }
        const previousContent = assistantMessage.current?.content ?? '';
          const message = applyStreamEvent(event, assistantMessage.current, {
            conversationId: targetConversationId,
            providerId: selectedModel.providerId,
            modelId: selectedModel.modelId,
          });
          if (!message) return;
          assistantMessage.current = message;
          streamUi.enqueue(message, event.name === 'chat.text.delta');
          if (!isRetry) {
            if (message.content !== previousContent) {
              const appendedContent = message.content.startsWith(previousContent)
                ? message.content.slice(previousContent.length)
                : message.content;
              unpersistedTextBytes += streamTextEncoder.encode(appendedContent).byteLength;
            }
            const shouldPersist =
              unpersistedTextBytes >= STREAM_PERSISTENCE_CHECKPOINT_BYTES ||
              ['chat.started', 'chat.citations.updated'].includes(event.name);
            if (!shouldPersist) return;
            unpersistedTextBytes = 0;
            enqueueLatestMessageSave(message);
          }
        },
      );
      set((current) => ({
        activeOperation: operation,
        activeOperationsByConversation: { ...current.activeOperationsByConversation, [targetConversationId]: operation },
      }));
      const result = await operation.result;
      streamUi.flush();
      await persistenceQueue;

      const resultStatus = result.status;
      const finalStatus =
        resultStatus === 'completed' || resultStatus === 'paused'
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
      requestOutcome =
        resultStatus === 'completed' ? 'completed' : resultStatus === 'paused' ? 'waitingForInput' : 'stopped';
      if (isRetry && finalStatus === 'stopped') setRequestError('responseRetryNotCompleted');
      const streamedMessage = assistantMessage.current;
      if (streamedMessage) {
        const completedMessage: ChatMessage = {
          ...streamedMessage,
          personalMemoryFactIds: resumeRunId
            ? streamedMessage.personalMemoryFactIds
            : JSON.stringify(personalMemoryFactIds),
          status: finalStatus,
          outputTokens: numberOrNull(result.outputTokens) ?? streamedMessage.outputTokens,
          elapsedMicroseconds: numberOrNull(result.elapsedMicroseconds) ?? streamedMessage.elapsedMicroseconds,
          tokensPerSecond: numberOrNull(result.tokensPerSecond) ?? streamedMessage.tokensPerSecond,
        };
        assistantMessage.current = completedMessage;
        if (finalStatus === 'completed' || (!isRetry && finalStatus === 'stopped')) {
          try {
            await saveMessageWithRetry(completedMessage, [], { preserveToolActivities: true });
          } catch (cause) {
            await logMessagePersistenceFailure('assistantFinal', cause, completedMessage, [], false);
            throw new LocalServiceError(
              'message_save_failed',
              'The assistant response could not be saved.',
              false,
              cause instanceof LocalServiceError ? (cause.diagnosticCode ?? cause.code) : null,
            );
          }
        }
        if (get().activeConversationId === targetConversationId) {
          if (isRetry && finalStatus !== 'completed') {
            set((current) => ({ messages: current.messages.filter((message) => message.id !== completedMessage.id) }));
          }
          set((current) => ({
            messages:
              finalStatus === 'completed' ? upsertMessage(current.messages, completedMessage) : current.messages,
            hasCompletedAssistantResponse:
              finalStatus === 'completed' || current.hasCompletedAssistantResponse === true,
          }));
        }
      }
      const conversations = await refreshConversationListAfterResponse(
        listConversations,
        (items) => set({ conversations: items }),
        (cause) => {
          globalThis.reportError(
            new Error('Conversation list refresh failed after the assistant response completed.', { cause }),
          );
        },
      );
      if (finalStatus === 'completed' && streamedMessage) {
        void playAssistantResponseAudio('completed', projectId);
        const conversation = conversations?.find((item) => item.id === targetConversationId);
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
      try {
        const detailCode =
          error instanceof LocalServiceError &&
          error.code === 'local_storage_failed' &&
          error.diagnosticCode !== null &&
          CHAT_STORAGE_DETAIL_CODES.has(error.diagnosticCode)
            ? error.diagnosticCode
            : null;
        await recordChatRequestFailure(
          sendStage,
          error instanceof LocalServiceError ? error.code : 'unexpected_error',
          detailCode,
        );
      } catch (diagnosticError) {
        globalThis.reportError(
          new Error('The chat request failure could not be recorded.', { cause: diagnosticError }),
        );
      }
      requestOutcome ??= providerRequestOutcomeForError(error);
      if (
        (selectedModel.providerId === 'chatgpt' || selectedModel.providerId === 'copilot') &&
        requestOutcome === 'rateLimited'
      ) {
        void recordOAuthRateLimit(selectedModel, state.models, error);
      }
      if (requestOutcome !== 'stopped') {
        void playAssistantResponseAudio('failed', projectId);
      }
      const code =
        !userMessageSaved && attachments.length > 0
          ? 'attachmentSaveFailed'
          : sendStage === 'conversationCreation'
            ? 'conversationCreateFailed'
            : !userMessageSaved && sendStage === 'userMessagePersistence'
              ? 'messageSaveFailed'
              : error instanceof LocalServiceError && error.code === 'message_save_failed'
                ? 'messageSaveFailed'
                : error instanceof LocalServiceError && error.code === 'storage_unavailable'
                  ? 'chatHistoryUnavailable'
                  : error instanceof LocalServiceError && error.code === 'invalid_request_params'
                    ? 'chatRequestFailed'
                    : error instanceof LocalServiceError && error.code === 'provider_credentials_missing'
                      ? 'providerCredentialMissing'
                      : error instanceof LocalServiceError && error.code === 'connection_unavailable'
                        ? 'providerConnectionUnavailable'
                        : error instanceof LocalServiceError
                          ? (serviceErrorCategoryMessageKey(error.category) ?? 'chatRequestFailed')
                          : 'chatRequestFailed';
      setRequestError(code, null, getOpenCodeRetryAtUnixMs(error, selectedModel.providerId));
      await persistenceQueue;
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
          streamUi.discard();
          if (conversationId !== null && get().activeConversationId === conversationId) {
            set((current) => ({ messages: current.messages.filter((message) => message.id !== failedMessage.id) }));
          }
        } else {
          streamUi.enqueue(failedMessage, false);
          try {
            await saveMessageWithRetry(failedMessage, [], { preserveToolActivities: true });
          } catch (cause) {
            await logMessagePersistenceFailure('assistantRecovery', cause, failedMessage, [], false);
            setRequestError('messageSaveFailed');
          }
        }
      }
      return userMessageSaved;
    } finally {
      window.dispatchEvent(new CustomEvent('openchat-chat-request-finished', { detail: { conversationId } }));
      streamUi.dispose();
      if (requestOutcome !== null) {
        try {
          writeProviderRequestOutcomeRecord(
            {
              providerId: selectedModel.providerId === 'chatgpt' ? 'chatgpt_oauth' : selectedModel.providerId,
              connectionId: selectedModel.apiKeyConnectionId ?? selectedModel.connectionId ?? selectedModel.providerId,
              workspaceId: selectedModel.providerId === 'chatgpt' ? (selectedModel.workspaceId ?? null) : null,
            },
            { outcome: requestOutcome, occurredAtUnixMs: Date.now() },
          );
        } catch {
          setRequestError('providerLastRequestSaveFailed');
        }
      }
      set((current) => {
        const activeOperationsByConversation = Object.fromEntries(
          Object.entries(current.activeOperationsByConversation).filter(([id]) => id !== conversationId),
        );
        const sendingConversationIds = current.sendingConversationIds.filter((id) => id !== conversationId);
        const pendingToolPermissionRequests = current.pendingToolPermissionRequests.filter(
          (request) => request.conversationId !== conversationId,
        );
        return {
          isSending: sendingConversationIds.length > 0,
          sendingConversationIds,
          sendingConversationId: sendingConversationIds[0] ?? null,
          activeOperationsByConversation,
          activeOperation: Object.values(activeOperationsByConversation)[0] ?? null,
          pendingToolPermissionRequests,
          pendingToolPermissionRequest: pendingToolPermissionRequests[0] ?? null,
          isRespondingToToolPermission: current.respondingToolPermissionIds.length > 0,
        };
      });
      const currentGoal = get().activeGoal;
      if (
        conversationId &&
        (goalObjective !== null || resumeRunId !== undefined || currentGoal?.conversationId === conversationId)
      ) {
        await get().refreshActiveGoal(conversationId);
      }
      if (conversationId && resumeRunId !== undefined) {
        await get().refreshInterruptedRun(conversationId);
      }
    }
  };
}

async function saveMessageWithRetry(
  message: ChatMessage,
  attachments: Parameters<typeof saveMessage>[1] = [],
  options: Parameters<typeof saveMessage>[2] = {},
): Promise<void> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      await saveMessage(message, attachments, options);
      return;
    } catch (error) {
      if (!(error instanceof LocalServiceError)) throw error;
      const retryDelay = chatStorageRetryDelay('messageSave', error, attempt);
      if (retryDelay === null) throw error;
      await new Promise<void>((resolve) => window.setTimeout(resolve, retryDelay));
    }
  }
}

async function logMessagePersistenceFailure(
  stage: 'userMessage' | 'assistantCheckpoint' | 'assistantFinal' | 'assistantRecovery',
  error: unknown,
  message: ChatMessage,
  attachments: Parameters<typeof saveMessage>[1] = [],
  includeToolActivities = true,
): Promise<void> {
  const code =
    error instanceof LocalServiceError && /^[a-z0-9_]{1,96}$/.test(error.code) ? error.code : 'unexpected_error';
  const detailCode =
    error instanceof LocalServiceError && error.diagnosticCode && /^[a-z0-9_]{1,96}$/.test(error.diagnosticCode)
      ? error.diagnosticCode
      : null;
  try {
    const metadataBytes = (value: string) => streamTextEncoder.encode(value).byteLength;
    let toolActivityCount: number | null = null;
    if (includeToolActivities) {
      try {
        const parsed: unknown = JSON.parse(message.toolActivities);
        if (Array.isArray(parsed)) toolActivityCount = parsed.length;
      } catch {
        // The byte count is still useful when the stored JSON itself is malformed.
      }
    }
    await recordChatMessagePersistenceFailure(stage, code, detailCode, {
      contentBytes: metadataBytes(message.content),
      citationSourceBytes: metadataBytes(message.citationSources),
      personalMemoryFactIdBytes: metadataBytes(message.personalMemoryFactIds),
      reasoningSummaryBytes: metadataBytes(message.reasoningSummaries),
      toolActivityBytes: includeToolActivities ? metadataBytes(message.toolActivities) : 0,
      toolActivityCount,
      attachmentCount: attachments.length,
      attachmentBytes: attachments.reduce((total, attachment) => total + attachment.bytes.byteLength, 0),
    });
  } catch (cause) {
    globalThis.reportError(new Error('The chat message persistence failure could not be recorded.', { cause }));
  }
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
    case 'opencode_free_usage_limit':
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
