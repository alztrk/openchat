import type { StoreApi } from 'zustand';
import { i18n } from '@/localization/i18n';
import { encodeBase64, type PendingChatAttachment } from '@/modules/conversations/data/chatAttachments';
import {
  type ChatMessage,
  createConversation,
  deleteConversation,
  listConversations,
  saveMessage,
} from '@/modules/conversations/data/chatStorage';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { loadRouteParameters } from '@/modules/providers/data/modelRouteParameters';
import { type LocalServiceClient, LocalServiceError } from '@/platform/local-service';
import type { ChatState } from './chatStore';
import { applyStreamEvent, numberOrNull, titleFromMessage, upsertMessage } from './chatStoreUtils';
import { createStreamMessagePublisher } from './streamMessagePublisher';

export interface TemporaryChatSession {
  id: string;
  route: ChatModel | null;
  disclosedProviderIds: string[];
  attachmentsByMessageId: Record<string, PendingChatAttachment[]>;
}

type ChatStoreSetState = StoreApi<ChatState>['setState'];
type ChatStoreGetState = StoreApi<ChatState>['getState'];

export function createTemporaryChatActions(
  set: ChatStoreSetState,
  get: ChatStoreGetState,
): Pick<
  ChatState,
  | 'startTemporaryChat'
  | 'discloseTemporaryProvider'
  | 'saveTemporaryChat'
  | 'discardTemporaryChat'
  | 'setPendingAttachmentCount'
> {
  return {
    startTemporaryChat() {
      const current = get();
      if (current.isSending) {
        set({ errorCode: 'temporaryChatStopBeforeLeaving' });
        return false;
      }
      if (current.temporaryChat) {
        set({ errorCode: 'temporaryChatExitRequired' });
        return false;
      }
      if (current.pendingAttachmentCount > 0) {
        set({ errorCode: 'temporaryChatRemoveAttachmentsFirst' });
        return false;
      }
      const selectedModel = current.models.find((model) => model.key === current.selectedModelKey) ?? null;
      set({
        temporaryChat: {
          id: createTemporaryChatId(),
          route: selectedModel,
          disclosedProviderIds: [],
          attachmentsByMessageId: {},
        },
        activeConversationId: null,
        activeGoal: null,
        goalStatus: 'ready',
        isChangingGoal: false,
        pendingProjectId: null,
        messages: [],
        isLoadingMessages: false,
        searchTargetMessageId: null,
        searchHighlightMessageId: null,
        errorCode: null,
      });
      return true;
    },
    discloseTemporaryProvider(providerId) {
      const session = get().temporaryChat;
      if (!session || session.disclosedProviderIds.includes(providerId)) return;
      set({
        temporaryChat: {
          ...session,
          disclosedProviderIds: [...session.disclosedProviderIds, providerId],
        },
      });
    },
    saveTemporaryChat: createSaveTemporaryChatAction(set, get),
    discardTemporaryChat() {
      const session = get().temporaryChat;
      if (!session || get().isSending) return;
      for (const attachments of Object.values(session.attachmentsByMessageId)) {
        for (const attachment of attachments) {
          if (attachment.previewUrl?.startsWith('blob:')) URL.revokeObjectURL(attachment.previewUrl);
        }
      }
      set({
        temporaryChat: null,
        activeConversationId: null,
        messages: [],
        isLoadingMessages: false,
        searchTargetMessageId: null,
        searchHighlightMessageId: null,
        errorCode: null,
      });
    },
    setPendingAttachmentCount(count) {
      set({ pendingAttachmentCount: Number.isFinite(count) ? Math.max(0, Math.floor(count)) : 0 });
    },
  };
}

export function createTemporaryChatId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  const suffix = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
  return `tmp_${suffix}`;
}

export function createTemporarySendMessageAction(
  set: ChatStoreSetState,
  get: ChatStoreGetState,
  localServiceClient: LocalServiceClient,
): ChatState['sendMessage'] {
  return async (rawContent, attachments = [], resumeRunId, resumeAssistantMessageId, options) => {
    const content = rawContent.trim();
    const state = get();
    const session = state.temporaryChat;
    if (!session || state.isSending || state.isUpdatingConversationModel) return false;
    if (resumeRunId || resumeAssistantMessageId || options?.mode) {
      set({ errorCode: 'chatRequestFailed' });
      return false;
    }
    if (!content && attachments.length === 0) return false;

    const selectedModel = state.models.find((model) => model.key === state.selectedModelKey);
    if (!selectedModel) {
      set({ errorCode: 'modelRequired' });
      return false;
    }
    if (selectedModel.providerId === 'chatgpt' && (!selectedModel.connectionId || !selectedModel.workspaceId)) {
      set({ errorCode: 'temporaryChatConnectionRequired' });
      return false;
    }
    if (!session.disclosedProviderIds.includes(selectedModel.providerId)) {
      set({ errorCode: 'temporaryChatExitRequired' });
      return false;
    }

    const transcript = state.messages;
    if (transcript.length + 1 > 100) {
      set({ errorCode: 'temporaryChatMessageLimitReached' });
      return false;
    }
    const attachmentBytes =
      Object.values(session.attachmentsByMessageId)
        .flat()
        .reduce((total, attachment) => total + attachment.sizeBytes, 0) +
      attachments.reduce((total, attachment) => total + attachment.sizeBytes, 0);
    const transcriptTextBytes = new TextEncoder().encode(
      [...transcript.map((message) => message.content), content].join(''),
    ).length;
    if (attachmentBytes + transcriptTextBytes > 16 * 1024 * 1024 || transcriptTextBytes > 768 * 1024) {
      set({ errorCode: 'temporaryChatMessageLimitReached' });
      return false;
    }

    let routeParameters: Record<string, unknown>;
    try {
      routeParameters = loadRouteParameters(selectedModel);
    } catch (error) {
      set({
        errorCode:
          error instanceof LocalServiceError && error.code === 'provider_credentials_missing'
            ? 'providerCredentialMissing'
            : 'chatRequestFailed',
      });
      return false;
    }

    const userMessage: ChatMessage = {
      id: crypto.randomUUID().replaceAll('-', ''),
      conversationId: session.id,
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
      personalMemoryFactIds: '[]',
      reasoningSummaries: '[]',
      toolActivities: '[]',
      status: 'completed',
      failureCode: null,
    };
    const nextAttachmentsByMessageId = {
      ...session.attachmentsByMessageId,
      ...(attachments.length > 0 ? { [userMessage.id]: [...attachments] } : {}),
    };
    const nextSession: TemporaryChatSession = {
      ...session,
      route: selectedModel,
      attachmentsByMessageId: nextAttachmentsByMessageId,
    };
    set((current) => ({
      temporaryChat: nextSession,
      messages: [...current.messages, userMessage],
      isSending: true,
      errorCode: null,
    }));

    const serializedMessages = [...transcript, userMessage].map((message) => {
      const attachments = nextAttachmentsByMessageId[message.id];
      return {
        role: message.role,
        content:
          message.content ||
          (message.status === 'failed' || message.status === 'stopped' ? i18n.t('temporaryChatResponseFailed') : ''),
        ...(attachments?.length
          ? {
              attachments: attachments.map((attachment) => ({
                id: attachment.id,
                name: attachment.name,
                mimeType: attachment.mimeType,
                sizeBytes: attachment.sizeBytes,
                kind: attachment.kind,
                data: encodeBase64(attachment.bytes),
              })),
            }
          : {}),
      };
    });
    let assistantMessage: ChatMessage | null = null;
    const streamUi = createStreamMessagePublisher((message) => {
      if (get().temporaryChat?.id === session.id) {
        set((current) => ({ messages: upsertMessage(current.messages, message) }));
      }
    });
    let operation: Awaited<ReturnType<LocalServiceClient['startOperation']>> | null = null;
    try {
      const parameters = {
        conversationId: session.id,
        providerId: selectedModel.providerId,
        modelId: selectedModel.modelId,
        messages: serializedMessages,
        ...routeParameters,
        ...(selectedModel.connectionId ? { connectionId: selectedModel.connectionId } : {}),
        ...(selectedModel.workspaceId ? { workspaceId: selectedModel.workspaceId } : {}),
      };
      operation = await localServiceClient.startOperation('chat.temporary.send', parameters, (event) => {
        const message = applyStreamEvent(event, assistantMessage, {
          conversationId: session.id,
          providerId: selectedModel.providerId,
          modelId: selectedModel.modelId,
        });
        if (!message) return;
        assistantMessage = message;
        streamUi.enqueue(message, event.name === 'chat.text.delta');
      });
      set({ activeOperation: operation });
      const result = await operation.result;
      streamUi.flush();
      if (
        result.conversationId !== session.id ||
        typeof result.messageId !== 'string' ||
        typeof result.content !== 'string' ||
        typeof result.createdAtUnixMs !== 'number' ||
        !Number.isFinite(result.createdAtUnixMs)
      ) {
        throw new LocalServiceError('invalid_service_response', 'The temporary response was invalid.');
      }
      const finalContent = result.content.trim().length > 0 ? result.content : i18n.t('temporaryChatResponseFailed');
      const completedMessage: ChatMessage = {
        id: result.messageId,
        conversationId: session.id,
        role: 'assistant',
        content: finalContent,
        attachments: [],
        createdAt: result.createdAtUnixMs,
        outputTokens: numberOrNull(result.outputTokens),
        tokensPerSecond: numberOrNull(result.tokensPerSecond),
        elapsedMicroseconds: numberOrNull(result.elapsedMicroseconds),
        providerId: selectedModel.providerId,
        modelId: selectedModel.modelId,
        citationSources: '[]',
        personalMemoryFactIds: '[]',
        reasoningSummaries: '[]',
        toolActivities: '[]',
        status: result.content.trim().length > 0 ? 'completed' : 'failed',
        failureCode: result.content.trim().length > 0 ? null : 'empty_response',
      };
      assistantMessage = completedMessage;
      if (get().temporaryChat?.id === session.id) {
        set((current) => ({ messages: upsertMessage(current.messages, completedMessage) }));
      }
      return completedMessage.status === 'completed';
    } catch (error) {
      streamUi.discard();
      const failureCode = error instanceof LocalServiceError ? error.code : 'service_error';
      const failureMessage: ChatMessage = assistantMessage
        ? {
            ...assistantMessage,
            status: failureCode === 'operation_cancelled' || failureCode === 'request_cancelled' ? 'stopped' : 'failed',
            failureCode,
          }
        : {
            id: crypto.randomUUID().replaceAll('-', ''),
            conversationId: session.id,
            role: 'assistant',
            content: i18n.t('temporaryChatResponseFailed'),
            attachments: [],
            createdAt: Date.now(),
            outputTokens: null,
            tokensPerSecond: null,
            elapsedMicroseconds: null,
            providerId: selectedModel.providerId,
            modelId: selectedModel.modelId,
            citationSources: '[]',
            personalMemoryFactIds: '[]',
            reasoningSummaries: '[]',
            toolActivities: '[]',
            status: failureCode === 'operation_cancelled' || failureCode === 'request_cancelled' ? 'stopped' : 'failed',
            failureCode,
          };
      if (get().temporaryChat?.id === session.id) {
        set((current) => ({
          messages: upsertMessage(current.messages, failureMessage),
          errorCode: failureCode === 'provider_credentials_missing' ? 'providerCredentialMissing' : 'chatRequestFailed',
        }));
      }
      return false;
    } finally {
      streamUi.dispose();
      if (get().temporaryChat?.id === session.id) {
        set({ isSending: false, activeOperation: null });
      } else if (get().activeOperation === operation) {
        set({ isSending: false, activeOperation: null });
      }
    }
  };
}

export function createSaveTemporaryChatAction(
  set: ChatStoreSetState,
  get: ChatStoreGetState,
): ChatState['saveTemporaryChat'] {
  return async () => {
    const state = get();
    const session = state.temporaryChat;
    if (!session || state.isSending || state.messages.length === 0 || !session.route) return false;
    const firstUserMessage = state.messages.find((message) => message.role === 'user');
    if (!firstUserMessage) return false;
    const firstAttachment = session.attachmentsByMessageId[firstUserMessage.id]?.[0];
    const conversationId = crypto.randomUUID().replaceAll('-', '');
    const title = titleFromMessage(firstUserMessage.content, firstAttachment?.name);
    let conversationCreated = false;

    try {
      await createConversation(conversationId, title, session.route, null);
      conversationCreated = true;
      for (const message of state.messages) {
        const savedMessage = { ...message, conversationId };
        await saveMessage(savedMessage, session.attachmentsByMessageId[message.id] ?? []);
      }
      const conversations = await listConversations();
      set({
        temporaryChat: null,
        activeConversationId: conversationId,
        conversations,
        messages: state.messages.map((message) => ({ ...message, conversationId })),
        errorCode: null,
      });
      await get().selectConversation(conversationId);
      return true;
    } catch {
      let rollbackFailed = false;
      if (conversationCreated) {
        try {
          await deleteConversation(conversationId);
        } catch {
          rollbackFailed = true;
        }
      }
      set({
        errorCode: rollbackFailed ? 'temporaryChatSaveCleanupFailed' : 'temporaryChatSaveFailed',
      });
      return false;
    }
  };
}
