import { parseToolActivities } from '@features/chat/components/message/messageData';
import { readProjectSkillIds, type ToolPermissionMode } from '@features/projects/data/projectPreferences';
import type { ChatModel } from '@entities/model';
import { modelRouteKey, readModelPreferences } from '@features/provider-settings/data/modelPreferences';
import { readAssistantExecutionPreferences } from '@features/app-settings/data/assistantExecutionPreferences';
import { personalMemoryContextForConversation } from '@features/app-settings/data/personalMemory';
import { readSharedInstructions } from '@features/app-settings/data/sharedInstructions';
import { LocalServiceError, localServiceClient } from '@shared/lib/platform/local-service';
import { invoke } from '@shared/lib/platform/tauri';
import type { PendingChatAttachment } from './chatAttachments';
import type { ChatMessage } from './chatStorage';

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
  usageMode: 'estimated' | 'provider_reported_plus_estimate';
  estimateSource: 'model_tokenizer' | 'heuristic';
}

const opaqueCheckpointEstimate = 4096;

export async function loadContextUsageConfiguration(
  model: ChatModel,
  conversationId: string | null,
  projectId: string | null,
  toolPermissionMode: ToolPermissionMode | 'plan',
  isTemporaryChat = false,
): Promise<ContextUsageConfiguration | null> {
  try {
    const sharedInstructions = readSharedInstructions() ?? '';
    const personalMemory = isTemporaryChat
      ? { context: '', factIds: [] }
      : await personalMemoryContextForConversation(conversationId);
    const assistantPreferences = readAssistantExecutionPreferences();
    const projectSkillIds =
      projectId === null || !assistantPreferences.includeProjectSkills ? [] : readProjectSkillIds(projectId);
    const customInstructions = [sharedInstructions, personalMemory.context]
      .map((value) => value.trim())
      .filter((value) => value.length > 0)
      .join('\n\n');
    const configuredContextLimit = readModelPreferences().contextLimits[modelRouteKey(model)];
    const contextWindow =
      configuredContextLimit === undefined
        ? model.contextWindow
        : model.contextWindow === null
          ? configuredContextLimit
          : Math.min(configuredContextLimit, model.contextWindow);
    const response = await localServiceClient.call('chat.context.usage.estimate', {
      providerId: model.providerId,
      modelId: model.modelId,
      contextWindow,
      supportsTools: model.supportsTools ?? null,
      ...(model.providerId === 'copilot' && model.requestProtocol ? { requestProtocol: model.requestProtocol } : {}),
      conversationId,
      projectId: conversationId === null ? projectId : null,
      customInstructions: customInstructions.length === 0 ? null : customInstructions,
      planMode: toolPermissionMode === 'plan',
      projectSkillIds,
    });
    return parseContextUsageConfiguration(response);
  } catch (cause) {
    if (isUnsupportedContextUsage(cause)) return null;
    await recordContextUsageFailure('contextConfiguration', cause);
    throw cause;
  }
}

export async function inspectContextUsageMemory(conversationId: string): Promise<ContextUsageMemory> {
  try {
    const response = await localServiceClient.call('chat.memory.inspect', { conversationId });
    return parseContextUsageMemory(response);
  } catch (cause) {
    await recordContextUsageFailure('memoryInspection', cause);
    throw cause;
  }
}

async function recordContextUsageFailure(
  operation: 'contextConfiguration' | 'fullEstimate' | 'memoryInspection',
  cause: unknown,
) {
  const code =
    cause instanceof LocalServiceError && /^[a-z0-9_]{1,96}$/.test(cause.code)
      ? cause.code
      : cause instanceof Error
        ? 'client_error'
        : 'unknown_error';
  try {
    await invoke('record_context_usage_failure', { operation, code });
  } catch (diagnosticCause) {
    globalThis.reportError(
      new Error('Context usage failure diagnostic could not be recorded.', { cause: diagnosticCause }),
    );
  }
}

/**
 * Fetches a backend context usage snapshot. Supported model identifiers use
 * their registered tokenizer; unknown models use an explicitly approximate
 * fallback. The backend accounts for messages, tool activity, attachments,
 * draft text, and compaction summary.
 */
export async function loadFullContextUsage(
  model: ChatModel,
  conversationId: string | null,
  draft: string,
  pendingAttachments: readonly PendingChatAttachment[],
  configuration: ContextUsageConfiguration | null,
  memory: ContextUsageMemory | null,
): Promise<ContextUsageSnapshot> {
  try {
    const result = await localServiceClient.call('chat.context.usage.full_estimate', {
      modelId: model.modelId,
      providerId: model.providerId,
      connectionId: model.connectionId ?? null,
      workspaceId: model.workspaceId ?? null,
      conversationId,
      draft: draft || null,
      attachments: pendingAttachments.map((a) => ({
        name: a.name,
        kind: a.kind,
        sizeBytes: a.sizeBytes,
      })),
    });
    return parseFullContextUsageSnapshot(result, configuration, memory);
  } catch (cause) {
    if (!isUnsupportedContextUsage(cause)) await recordContextUsageFailure('fullEstimate', cause);
    throw cause;
  }
}

function isUnsupportedContextUsage(cause: unknown): boolean {
  return cause instanceof LocalServiceError && cause.code === 'service_capability_unavailable';
}

function parseFullContextUsageSnapshot(
  raw: Record<string, unknown>,
  configuration: ContextUsageConfiguration | null,
  _memory: ContextUsageMemory | null,
): ContextUsageSnapshot {
  const userMessageTokens = isFiniteNonNegativeInteger(raw.userMessageTokens) ? raw.userMessageTokens : 0;
  const assistantMessageTokens = isFiniteNonNegativeInteger(raw.assistantMessageTokens)
    ? raw.assistantMessageTokens
    : 0;
  const draftTokens = isFiniteNonNegativeInteger(raw.draftTokens) ? raw.draftTokens : 0;
  const memoryTokens = isFiniteNonNegativeInteger(raw.memoryTokens) ? raw.memoryTokens : 0;
  const providerInputTokens = isFiniteNonNegativeInteger(raw.providerInputTokens) ? raw.providerInputTokens : null;
  const usageMode =
    raw.usageMode === 'provider_reported_plus_estimate'
      ? 'provider_reported_plus_estimate'
      : raw.usageMode === 'estimated'
        ? 'estimated'
        : null;
  const estimateSource =
    raw.estimateSource === 'model_tokenizer'
      ? 'model_tokenizer'
      : raw.estimateSource === 'heuristic'
        ? 'heuristic'
        : null;
  if (usageMode === null || estimateSource === null) {
    throw new Error('The context usage estimate metadata is invalid.');
  }

  const backendAttachments: ContextUsageAttachment[] = [];
  if (Array.isArray(raw.attachments)) {
    for (const item of raw.attachments) {
      if (
        isObject(item) &&
        typeof item.name === 'string' &&
        isFiniteNonNegativeInteger(item.tokens) &&
        typeof item.isDraft === 'boolean'
      ) {
        backendAttachments.push({ name: item.name, tokens: item.tokens, isDraft: item.isDraft });
      }
    }
  }

  const toolUsage: { name: string; tokens: number }[] = [];
  if (Array.isArray(raw.toolUsage)) {
    for (const item of raw.toolUsage) {
      if (isObject(item) && typeof item.name === 'string' && isFiniteNonNegativeInteger(item.tokens)) {
        toolUsage.push({ name: item.name, tokens: item.tokens });
      }
    }
  }

  const instructionsTokens = configuration?.instructionsTokens ?? 0;
  const toolDefinitionTokens = configuration?.toolDefinitions.reduce((sum, item) => sum + item.tokens, 0) ?? 0;
  const rawTotalTokens = isFiniteNonNegativeInteger(raw.totalTokens) ? raw.totalTokens : 0;
  const totalTokens =
    rawTotalTokens + (providerInputTokens === null ? instructionsTokens + toolDefinitionTokens : 0);

  return {
    totalTokens,
    instructionsTokens,
    toolDefinitionTokens,
    userMessageTokens,
    assistantMessageTokens,
    attachments: backendAttachments,
    toolUsage,
    memoryTokens,
    draftTokens,
    providerInputTokens,
    usageMode,
    estimateSource,
  };
}

export function estimateContextUsage(
  messages: readonly ChatMessage[],
  draft: string,
  pendingAttachments: readonly PendingChatAttachment[],
  model: ChatModel,
  configuration: ContextUsageConfiguration | null,
  memory: ContextUsageMemory | null,
): ContextUsageSnapshot {
  const compactionMatchesRoute =
    memory !== null &&
    (memory.compactionKind === 'summary' ||
      (memory.compactionKind === 'responses_checkpoint' &&
        memory.compactionProviderId === model.providerId &&
        memory.compactionModelId === model.modelId &&
        memory.compactionConnectionId === (model.connectionId ?? null) &&
        memory.compactionWorkspaceId === (model.workspaceId ?? null)));
  const boundaryIndex =
    compactionMatchesRoute && memory.compactedThroughMessageId !== null
      ? messages.findIndex((message) => message.id === memory.compactedThroughMessageId)
      : -1;
  const contextMessages = messages.slice(boundaryIndex >= 0 ? boundaryIndex + 1 : 0);
  const messageEstimate = estimateMessages(contextMessages);
  const pendingAttachmentEstimates = pendingAttachments.map((attachment) =>
    estimateAttachment(attachment.name, attachment.kind, attachment.sizeBytes, true),
  );
  const pendingAttachmentTokens = pendingAttachmentEstimates.reduce((sum, attachment) => sum + attachment.tokens, 0);
  const attachments = [...messageEstimate.attachments, ...pendingAttachmentEstimates];
  const memoryTokens =
    compactionMatchesRoute && memory.compactionKind === 'summary'
      ? estimateText(memory.summary ?? '')
      : compactionMatchesRoute && memory.compactionKind === 'responses_checkpoint'
        ? opaqueCheckpointEstimate
        : 0;
  const draftTokens = estimateText(draft);
  const instructionsTokens = configuration?.instructionsTokens ?? 0;
  const toolDefinitionTokens = configuration?.toolDefinitions.reduce((sum, item) => sum + item.tokens, 0) ?? 0;
  const baseTotal =
    instructionsTokens +
    toolDefinitionTokens +
    messageEstimate.totalTokens +
    pendingAttachmentTokens +
    memoryTokens +
    draftTokens;

  const lastPrompt = memory?.lastPrompt ?? null;
  const matchingPrompt =
    lastPrompt !== null &&
    lastPrompt.providerId === model.providerId &&
    lastPrompt.modelId === model.modelId &&
    lastPrompt.connectionId === (model.connectionId ?? null) &&
    lastPrompt.workspaceId === (model.workspaceId ?? null);
  const promptMessageIndex =
    matchingPrompt && lastPrompt !== null ? messages.findIndex((message) => message.id === lastPrompt.messageId) : -1;
  const promptIsAfterCompactionBoundary =
    promptMessageIndex >= 0 && (boundaryIndex < 0 || promptMessageIndex > boundaryIndex);
  const providerInputTokens =
    matchingPrompt && lastPrompt !== null && promptIsAfterCompactionBoundary ? lastPrompt.inputTokens : null;
  const postPromptEstimate = estimateMessages(messages.slice(promptMessageIndex + 1));
  const providerAdditions = providerInputTokens === null
    ? 0
    : postPromptEstimate.totalTokens + pendingAttachmentTokens;
  return {
    totalTokens: providerInputTokens === null ? baseTotal : providerInputTokens + providerAdditions + draftTokens,
    instructionsTokens,
    toolDefinitionTokens,
    userMessageTokens: messageEstimate.userTokens,
    assistantMessageTokens: messageEstimate.assistantTokens,
    attachments,
    toolUsage: messageEstimate.toolUsage,
    memoryTokens,
    draftTokens,
    providerInputTokens,
    usageMode: providerInputTokens === null ? 'estimated' : 'provider_reported_plus_estimate',
    estimateSource: 'heuristic',
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
    attachments.push(
      ...message.attachments.map((attachment) =>
        estimateAttachment(attachment.name, attachment.kind, attachment.sizeBytes, false),
      ),
    );
    if (message.role === 'user') {
      userTokens += estimateText(message.content);
      continue;
    }
    assistantTokens += estimateText(message.content);
    for (const activity of parseToolActivities(message.toolActivities)) {
      if (activity.status !== 'completed') continue;
      let activityTokens = estimateText(
        JSON.stringify({
          type: 'function_call',
          call_id: activity.callId,
          name: activity.name,
          arguments: stringifyJson(activity.arguments),
        }),
      );
      if (activity.output !== null) {
        activityTokens += estimateText(
          JSON.stringify({
            type: 'function_call_output',
            call_id: activity.callId,
            output: stringifyJson(activity.output),
          }),
        );
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
    totalTokens:
      userTokens +
      assistantTokens +
      attachments.reduce((sum, item) => sum + item.tokens, 0) +
      sortedToolUsage.reduce((sum, item) => sum + item.tokens, 0),
  };
}

function estimateAttachment(
  name: string,
  kind: 'image' | 'text',
  sizeBytes: number,
  isDraft: boolean,
): ContextUsageAttachment {
  return {
    name,
    tokens: kind === 'image' ? 2048 : Math.ceil(sizeBytes / 4),
    isDraft,
  };
}

function estimateText(value: string): number {
  let asciiCharacters = 0;
  let punctuationCharacters = 0;
  let nonAsciiCharacters = 0;
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0;
    if (codePoint <= 0x7f) {
      asciiCharacters += 1;
      const isAsciiAlphanumeric =
        (codePoint >= 0x30 && codePoint <= 0x39) ||
        (codePoint >= 0x41 && codePoint <= 0x5a) ||
        (codePoint >= 0x61 && codePoint <= 0x7a);
      const isAsciiWhitespace = codePoint === 0x20 || (codePoint >= 0x09 && codePoint <= 0x0d);
      if (!isAsciiAlphanumeric && !isAsciiWhitespace) punctuationCharacters += 1;
    } else {
      nonAsciiCharacters += 1;
    }
  }
  const ordinaryAscii = asciiCharacters - punctuationCharacters;
  return Math.ceil(ordinaryAscii / 3) + punctuationCharacters + nonAsciiCharacters * 2;
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
  if (
    !isObject(value) ||
    !isFiniteNonNegativeInteger(value.inputTokens) ||
    typeof value.messageId !== 'string' ||
    typeof value.providerId !== 'string' ||
    typeof value.modelId !== 'string'
  ) {
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
