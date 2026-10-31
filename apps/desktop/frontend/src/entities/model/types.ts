/** Canonical domain types for models and routes */

export interface ChatRoute {
  providerId: string;
  modelId: string;
  connectionId?: string;
  workspaceId?: string;
  apiKeyConnectionId?: string;
}

export interface CustomProviderInfo {
  id: `custom_${string}`;
  name: string;
  baseUrl: string;
  protocol: 'openAi' | 'anthropic';
  allowInsecureHttp: boolean;
  reasoningEfforts: string[];
  enableThinking: boolean;
}

export interface ChatModel extends ChatRoute {
  key: string;
  displayName: string;
  description: string | null;
  supportsReasoning: boolean;
  supportsImages: boolean;
  supportsTools?: boolean | null;
  supportsFastMode: boolean;
  reasoningLevels: string[];
  requestProtocol?: 'chatCompletions' | 'responses' | 'messages';
  contextWindow: number | null;
  catalogStale: boolean;
  sourceLabel?: string;
  groupId?: string;
  customProvider?: CustomProviderInfo;
}

export interface ChatModelProvider {
  providerId: string;
  name: string | null;
  available: boolean;
}
