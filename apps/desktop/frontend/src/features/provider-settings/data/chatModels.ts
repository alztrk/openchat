import type { CustomProviderSummary } from './providerCredentials';
import type { ChatRoute } from './routes';

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
  customProvider?: Omit<CustomProviderSummary, 'isConfigured' | 'keySuffix'>;
}

export interface ChatModelProvider {
  providerId: string;
  name: string | null;
  available: boolean;
}
