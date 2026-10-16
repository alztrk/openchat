import type { JsonObject } from '@/shared/services/LocalServiceClient';
import {
  type ChatConversation,
  type ChatMessage,
} from '@/features/chat/data/chatStorage';
import type { ChatModel } from '@/features/chat/data/chatModels';
import { compatibleProviderIds, type CompatibleProviderId } from '@/features/settings/data/providerCredentials';

export function applyStreamEvent(
  event: { name: string; data: JsonObject },
  previous: ChatMessage | null,
  route: { conversationId: string; providerId: string; modelId: string },
): ChatMessage | null {
  const data = event.data;
  if (
    !['chat.started', 'chat.delta', 'chat.reasoning.delta', 'chat.tool.updated', 'chat.citations.updated'].includes(event.name) ||
    typeof data.messageId !== 'string' ||
    typeof data.content !== 'string' ||
    typeof data.createdAtUnixMs !== 'number'
  ) {
    return null;
  }
  const id = data.messageId;
  const base: ChatMessage = previous?.id === id
    ? previous
    : {
        id,
        conversationId: route.conversationId,
        role: 'assistant',
        content: '',
        attachments: [],
        createdAt: data.createdAtUnixMs,
        outputTokens: null,
        tokensPerSecond: null,
        elapsedMicroseconds: null,
        providerId: route.providerId,
        modelId: route.modelId,
        citationSources: '[]',
        personalMemoryFactIds: '[]',
        reasoningSummaries: '[]',
        toolActivities: '[]',
        status: 'streaming',
        failureCode: null,
      };

  return {
    ...base,
    content: data.content,
    createdAt: data.createdAtUnixMs,
    reasoningSummaries: Array.isArray(data.reasoningGroups)
      ? JSON.stringify(data.reasoningGroups)
      : base.reasoningSummaries,
    toolActivities: isObject(data.toolActivity)
      ? mergeToolActivity(base.toolActivities, data.toolActivity)
      : base.toolActivities,
    citationSources: Array.isArray(data.sources)
      ? JSON.stringify(data.sources)
      : base.citationSources,
  };
}

export function isSameRoute(model: ChatModel, conversation: ChatConversation): boolean {
  return model.providerId === conversation.providerId &&
    model.modelId === conversation.modelId &&
    (model.connectionId ?? null) === conversation.connectionId &&
    (model.workspaceId ?? null) === conversation.workspaceId &&
    (model.apiKeyConnectionId ?? null) === conversation.apiKeyConnectionId;
}

export function favoriteSourceConnection(model: ChatModel): string | null {
  if (
    model.providerId === 'chatgpt_api' ||
    isCompatibleProviderId(model.providerId) ||
    model.customProvider
  ) {
    return model.apiKeyConnectionId ?? model.connectionId ?? model.providerId;
  }
  return null;
}

export function upsertMessage(messages: ChatMessage[], message: ChatMessage): ChatMessage[] {
  const index = messages.findIndex((item) => item.id === message.id);
  if (index < 0) return [...messages, message];
  return messages.map((item, itemIndex) => itemIndex === index ? message : item);
}

export function titleFromMessage(content: string, attachmentName?: string): string {
  const firstLine = content.split(/\r?\n/, 1)[0]?.trim() ?? content.trim();
  return firstLine.slice(0, 500) || content.trim().slice(0, 500) || attachmentName?.slice(0, 500) || '';
}

export function numberOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

export function isObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function mergeToolActivity(encoded: string, activity: JsonObject): string {
  const parsed: unknown = JSON.parse(encoded);
  if (!Array.isArray(parsed)) {
    throw new Error('The stored tool activity list is invalid.');
  }
  const callId = activity.callId;
  const index = parsed.findIndex(
    (item) => isObject(item) && item.callId === callId,
  );
  if (index < 0) parsed.push(activity);
  else parsed[index] = activity;
  return JSON.stringify(parsed);
}

function isCompatibleProviderId(value: string): value is CompatibleProviderId {
  return compatibleProviderIds.some((providerId) => providerId === value);
}
