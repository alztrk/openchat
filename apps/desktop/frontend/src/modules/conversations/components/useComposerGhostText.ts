import { useEffect, useMemo, useRef, useState } from 'react';
import type { ChatMessage } from '@/modules/conversations/data/chatStorage';
import { useComposerPreferencesStore } from '@/modules/conversations/data/composerPreferences';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { loadRouteParameters } from '@/modules/providers/data/modelRouteParameters';
import { localServiceClient, type ServiceOperation } from '@/platform/local-service';
import type { ChatSlashCommand } from './ChatSlashCommandMenu';
import type { ChatCommandFeedbackKey } from './composerUtils';
import { getGhostTextContext, parseGhostTextSuggestion } from './ghostTextSuggestions';

export interface UseComposerGhostTextOptions {
  activeConversationId: string | null;
  isAnyConversationSending: boolean;
  isLoadingMessages: boolean;
  isTemporaryChat: boolean;
  messageDraft: string;
  messages: readonly ChatMessage[];
  models: readonly ChatModel[];
  selectedModel: ChatModel | undefined;
  slashCommands: readonly ChatSlashCommand[];
  targetConversationId?: string | null;
  temporaryChatId?: string | null;
  onFeedbackKey?: (key: ChatCommandFeedbackKey | null) => void;
}

export interface UseComposerGhostTextReturn {
  ghostTextSuggestion: string | null;
  setGhostTextDismissed: (dismissed: boolean) => void;
}

export function useComposerGhostText({
  activeConversationId,
  isAnyConversationSending,
  isLoadingMessages,
  isTemporaryChat,
  messageDraft,
  messages,
  models,
  selectedModel,
  slashCommands,
  targetConversationId,
  temporaryChatId,
  onFeedbackKey,
}: UseComposerGhostTextOptions): UseComposerGhostTextReturn {
  const ghostTextEnabled = useComposerPreferencesStore((state) => state.ghostTextEnabled);
  const ghostTextModelKey = useComposerPreferencesStore((state) => state.ghostTextModelKey);
  const [ghostTextDismissed, setGhostTextDismissed] = useState(false);
  const [generatedGhostText, setGeneratedGhostText] = useState<{ requestKey: string; text: string } | null>(null);
  const lastGhostTextRequestKeyRef = useRef<string | null>(null);

  const ghostTextContext = useMemo(() => getGhostTextContext(messages), [messages]);
  const ghostTextModel =
    ghostTextModelKey === 'auto' ? selectedModel : models.find((model) => model.key === ghostTextModelKey);
  const ghostTextConversationId = isTemporaryChat ? (temporaryChatId ?? null) : activeConversationId;
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
      onFeedbackKey?.('ghostTextSuggestionUnavailable');
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
          onFeedbackKey?.('ghostTextSuggestionUnavailable');
          return;
        }
        const text = parseGhostTextSuggestion(result.message);
        if (text) {
          setGeneratedGhostText({ requestKey: ghostTextRequestKey, text });
        } else {
          onFeedbackKey?.('ghostTextSuggestionUnavailable');
        }
      } catch (cause: unknown) {
        if (!disposed && !cancellationRequested) {
          onFeedbackKey?.('ghostTextSuggestionUnavailable');
          globalThis.reportError(new Error('The follow-up suggestion could not be generated.', { cause }));
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
    ghostTextConversationId,
    ghostTextEnabled,
    ghostTextModel,
    ghostTextRequestKey,
    isAnyConversationSending,
    isLoadingMessages,
    messageDraft.length,
    onFeedbackKey,
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
    generatedGhostText,
    ghostTextDismissed,
    ghostTextEnabled,
    ghostTextRequestKey,
    isAnyConversationSending,
    messageDraft.length,
  ]);

  return {
    ghostTextSuggestion,
    setGhostTextDismissed,
  };
}
