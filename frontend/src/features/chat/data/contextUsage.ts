import { getActiveGlobalMcpInstructions } from '@/features/library/data/mcpStorage';
import {
  personalMemoryContextForConversation,
} from '@/features/settings/data/personalMemory';
import { readSharedInstructions } from '@/features/settings/data/sharedInstructions';
import {
  readProjectSkillIds,
  type ToolPermissionMode,
} from './projectPreferences';
import { localServiceClient } from '@/features/chat/stores/chatStore';
import type { ChatMessage } from './chatStorage';
import type { PendingChatAttachment } from './chatAttachments';
import type { ChatModel } from './chatModels';
import { parseToolActivities } from '@/features/chat/components/message/messageData';

export interface ContextUsageConfiguration {
  instructionsTokens: number;
  toolDefinitions: { name: string; tokens: number }[];
  projectContextUnavailable: boolean;
}

export interface ContextUsageMemory {
  compactionKind: string | null;
  compactionProviderId: string | null;
  compactionModelId: string | null;
  compactionConnectionId: string | null;
  compactionWorkspaceId: string | null;
  compactedThroughMessageId: string | null;
  summary: string | null;
  lastPrompt: {
    inputTokens: number;
    messageId: string;
    providerId: string;
    modelId: string;
    connectionId: string | null;
    workspaceId: string | null;
  } | null;
}

export interface ContextUsageAttachment {
  name: string;
  tokens: number;
  isDraft: boolean;
}

export interface ContextUsageSnapshot {
  totalTokens: number;
  instructionsTokens: number;
  toolDefinitionTokens: number;
  userMessageTokens: number;
  assistantMessageTokens: number;
  attachments: ContextUsageAttachment[];
  toolUsage: { name: string; tokens: number }[];
  memoryTokens: number;
  draftTokens: number;
  providerInputTokens: number | null;
}

const opaqueCheckpointEstimate = 4096;

export async function loadContextUsageConfiguration(
  model: ChatModel,
  conversationId: string | null,
  projectId: string | null,
  toolPermissionMode: ToolPermissionMode | 'plan',
): Promise<ContextUsageConfiguration> {
  const sharedInstructions = readSharedInstructions() ?? '';
  const personalMemory = personalMemoryContextForConversation(conversationId);
  const mcpInstructions = await getActiveGlobalMcpInstructions();
  const projectSkillIds = projectId === null ? [] : readProjectSkillIds(projectId);
  const customInstructions = [sharedInstructions, personalMemory.context, mcpInstructions]
    .map((value) => value.trim())
    .filter((value) => value.length > 0)
    .join('\n\n');
  const response = await localServiceClient.call('chat.context.usage.estimate', {
    providerId: model.providerId,
    modelId: model.modelId,
    supportsTools: model.supportsTools ?? null,
    conversationId,
    projectId: conversationId === null ? projectId : null,
    customInstructions: customInstructions.length === 0 ? null : customInstructions,
    toolPermissionMode,
    projectSkillIds,
  });
  return {
    ...parseContextUsageConfiguration(response),
  };
}

export async function inspectContextUsageMemory(conversationId: string): Promise<ContextUsageMemory> {
  const response = await localServiceClient.call('chat.memory.inspect', { conversationId });
  return parseContextUsageMemory(response);
}

export function estimateContextUsage(
  messages: readonly ChatMessage[],
  draft: string,
  pendingAttachments: readonly PendingChatAttachment[],
  model: ChatModel,
  configuration: ContextUsageConfiguration | null,
  memory: ContextUsageMemory | null,
): ContextUsageSnapshot {
  const compactionMatchesRoute = memory !== null &&
    memory.compactionKind !== null &&
    memory.compactionProviderId === model.providerId &&
    memory.compactionModelId === model.modelId &&
    memory.compactionConnectionId === (model.connectionId ?? null) &&
    memory.compactionWorkspaceId === (model.workspaceId ?? null);
  const boundaryIndex = compactionMatchesRoute && memory.compactedThroughMessageId !== null
    ? messages.findIndex((message) => message.id === memory.compactedThroughMessageId)
    : -1;
  const contextMessages = messages.slice(boundaryIndex >= 0 ? boundaryIndex + 1 : 0);
  const messageEstimate = estimateMessages(contextMessages);
  const attachments = [
    ...messageEstimate.attachments,
    ...pendingAttachments.map((attachment) => estimateAttachment(attachment.name, attachment.kind, attachment.sizeBytes, true)),
  ];
  const memoryTokens = compactionMatchesRoute && memory.compactionKind === 'summary'
    ? estimateText(memory.summary ?? '')
    : compactionMatchesRoute && memory.compactionKind === 'responses_checkpoint'
      ? opaqueCheckpointEstimate
      : 0;
  const draftTokens = estimateText(draft);
  const instructionsTokens = configuration?.instructionsTokens ?? 0;
  const toolDefinitionTokens = configuration?.toolDefinitions.reduce((sum, item) => sum + item.tokens, 0) ?? 0;
  const baseTotal = instructionsTokens + toolDefinitionTokens + messageEstimate.userTokens +
    messageEstimate.assistantTokens + attachments.reduce((sum, item) => sum + item.tokens, 0) +
    messageEstimate.toolUsage.reduce((sum, item) => sum + item.tokens, 0) + memoryTokens + draftTokens;

  const lastPrompt = memory?.lastPrompt ?? null;
  const matchingPrompt = lastPrompt !== null &&
    lastPrompt.providerId === model.providerId &&
    lastPrompt.modelId === model.modelId &&
    lastPrompt.connectionId === (model.connectionId ?? null) &&
    lastPrompt.workspaceId === (model.workspaceId ?? null);
  const promptMessageIndex = matchingPrompt && lastPrompt !== null
    ? messages.findIndex((message) => message.id === lastPrompt.messageId)
    : -1;
  const providerInputTokens = matchingPrompt && lastPrompt !== null ? lastPrompt.inputTokens : null;
  const providerAdditions = promptMessageIndex >= 0
    ? estimateMessages(messages.slice(promptMessageIndex + 1)).totalTokens
    : 0;
  return {
    totalTokens: providerInputTokens === null
      ? baseTotal
      : providerInputTokens + providerAdditions + draftTokens,
    instructionsTokens,
    toolDefinitionTokens,
    userMessageTokens: messageEstimate.userTokens,
    assistantMessageTokens: messageEstimate.assistantTokens,
    attachments,
    toolUsage: messageEstimate.toolUsage,
    memoryTokens,
    draftTokens,
    providerInputTokens,
  };
}

function estimateMessages(messages: readonly ChatMessage[]): {
  userTokens: number;
  assistantTokens: number;
  attachments: ContextUsageAttachment[];
  toolUsage: { name: string; tokens: number }[];
  totalTokens: number;
} {
  let userTokens = 0;
  let assistantTokens = 0;
  const attachments: ContextUsageAttachment[] = [];
  const toolUsage = new Map<string, number>();
  for (const message of messages) {
    attachments.push(...message.attachments.map((attachment) => estimateAttachment(
      attachment.name,
      attachment.kind,
      attachment.sizeBytes,
      false,
    )));
    if (message.role === 'user') {
      userTokens += estimateText(message.content);
      continue;
    }
    assistantTokens += estimateText(message.content);
    for (const activity of parseToolActivities(message.toolActivities)) {
      if (activity.status !== 'completed') continue;
      let activityTokens = estimateText(JSON.stringify({
        type: 'function_call',
        call_id: activity.callId,
        name: activity.name,
        arguments: stringifyJson(activity.arguments),
      }));
      if (activity.output !== null) {
        activityTokens += estimateText(JSON.stringify({
          type: 'function_call_output',
          call_id: activity.callId,
          output: stringifyJson(activity.output),
        }));
      }
      toolUsage.set(activity.name, (toolUsage.get(activity.name) ?? 0) + activityTokens);
    }
  }
  const sortedToolUsage = [...toolUsage.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, tokens]) => ({ name, tokens }));
  return {
    userTokens,
    assistantTokens,
    attachments,
    toolUsage: sortedToolUsage,
    totalTokens: userTokens + assistantTokens + attachments.reduce((sum, item) => sum + item.tokens, 0) +
      sortedToolUsage.reduce((sum, item) => sum + item.tokens, 0),
  };
}

function estimateAttachment(name: string, kind: 'image' | 'text', sizeBytes: number, isDraft: boolean): ContextUsageAttachment {
  return {
    name,
    tokens: kind === 'image' ? 2048 : Math.ceil(sizeBytes / 4),
    isDraft,
  };
}

function estimateText(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function stringifyJson(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}

function parseContextUsageConfiguration(value: Record<string, unknown>): ContextUsageConfiguration {
  if (
    !isFiniteNonNegativeInteger(value.instructionsTokens) ||
    !Array.isArray(value.toolDefinitions) ||
    typeof value.projectContextUnavailable !== 'boolean'
  ) {
    throw new Error('The context usage configuration is invalid.');
  }
  const toolDefinitions = value.toolDefinitions.map((item) => {
    if (!isObject(item) || typeof item.name !== 'string' || !isFiniteNonNegativeInteger(item.tokens)) {
      throw new Error('The context usage tool definition is invalid.');
    }
    return { name: item.name, tokens: item.tokens };
  });
  return {
    instructionsTokens: value.instructionsTokens,
    toolDefinitions,
    projectContextUnavailable: value.projectContextUnavailable,
  };
}

function parseContextUsageMemory(value: Record<string, unknown>): ContextUsageMemory {
  const lastPrompt = value.lastPrompt === null ? null : parseLastPrompt(value.lastPrompt);
  return {
    compactionKind: nullableString(value.compactionKind),
    compactionProviderId: nullableString(value.compactionProviderId),
    compactionModelId: nullableString(value.compactionModelId),
    compactionConnectionId: nullableString(value.compactionConnectionId),
    compactionWorkspaceId: nullableString(value.compactionWorkspaceId),
    compactedThroughMessageId: nullableString(value.compactedThroughMessageId),
    summary: nullableString(value.summary),
    lastPrompt,
  };
}

function parseLastPrompt(value: unknown): ContextUsageMemory['lastPrompt'] {
  if (!isObject(value) || !isFiniteNonNegativeInteger(value.inputTokens) ||
    typeof value.messageId !== 'string' || typeof value.providerId !== 'string' ||
    typeof value.modelId !== 'string') {
    throw new Error('The context usage prompt is invalid.');
  }
  return {
    inputTokens: value.inputTokens,
    messageId: value.messageId,
    providerId: value.providerId,
    modelId: value.modelId,
    connectionId: nullableString(value.connectionId),
    workspaceId: nullableString(value.workspaceId),
  };
}

function nullableString(value: unknown): string | null {
  return value === null || value === undefined ? null : typeof value === 'string' ? value : null;
}

function isFiniteNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
