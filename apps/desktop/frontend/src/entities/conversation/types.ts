/** Canonical domain types for chat and conversations */

import type { ChatModel, ChatModelProvider } from '../model/types';
import type { ChatProject, ToolPermissionMode } from '../project/types';

export type ChatRole = 'user' | 'assistant';
export type ChatMessageStatus = 'streaming' | 'completed' | 'failed' | 'stopped';
export type ChatAttachmentKind = 'image' | 'text';
export type ExportFormat = 'md' | 'html' | 'json';
export type AssistantResponseStyle = 'concise' | 'balanced' | 'detailed';

export interface AssistantAgentSnapshot {
  id: string;
  name: string;
  instructions: string;
  responseStyle: AssistantResponseStyle;
  toolAllowlist: string[] | null;
}

export interface ChatAttachmentMetadata {
  id: string;
  name: string;
  mimeType: string;
  sizeBytes: number;
  kind: ChatAttachmentKind;
}

export interface ChatConversation {
  id: string;
  title: string;
  titleSource: string;
  providerId: string | null;
  modelId: string | null;
  connectionId: string | null;
  workspaceId: string | null;
  apiKeyConnectionId: string | null;
  projectId: string | null;
  agent: AssistantAgentSnapshot | null;
  isPinned: boolean;
  isArchived: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface ChatMessage {
  id: string;
  conversationId: string;
  role: ChatRole;
  content: string;
  attachments: ChatAttachmentMetadata[];
  createdAt: number | null;
  outputTokens: number | null;
  tokensPerSecond: number | null;
  elapsedMicroseconds: number | null;
  providerId: string | null;
  modelId: string | null;
  citationSources: string;
  personalMemoryFactIds: string;
  reasoningSummaries: string;
  toolActivities: string;
  status: ChatMessageStatus;
  failureCode: string | null;
}

export interface ChatModelFavorite {
  providerId: string;
  modelId: string;
  displayName: string;
  sourceConnectionId: string | null;
}

export interface ChatToolPermissionRequest {
  id: string;
  conversationId: string;
  toolName: string;
  targetPath: string;
  arguments: Record<string, unknown>;
}

export type { ChatModel, ChatModelProvider, ChatProject, ToolPermissionMode };
