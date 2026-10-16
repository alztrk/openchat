import type { CustomProviderSummary } from '@/features/settings/data/providerCredentials';
import type { ChatRoute } from './chatStorage';

export interface ChatModel extends ChatRoute {
  key: string;
  displayName: string;
  description: string | null;
  supportsReasoning: boolean;
  supportsImages: boolean;
  supportsTools?: boolean | null;
  supportsFastMode: boolean;
  reasoningLevels: string[];
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
