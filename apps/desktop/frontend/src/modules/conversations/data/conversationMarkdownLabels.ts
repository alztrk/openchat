import type { ChatConversation } from './chatStorage';
import type { ConversationMarkdownLabels, ExportToolStatus } from './conversationMarkdownExport';

type TranslationKey = keyof typeof import('@/localization/locales/en.json');
type Translate = (key: TranslationKey, options?: { count?: number }) => string;

const providerTranslationKeys: Partial<Record<string, TranslationKey>> = {
  opencode: 'openCodeProvider',
  copilot: 'providerNameCopilot',
  gemini: 'geminiProvider',
  groq: 'groqProvider',
  cerebras: 'cerebrasProvider',
  openrouter: 'openRouterProvider',
  mistral: 'mistralProvider',
  deepseek: 'deepSeekProvider',
  xai: 'xAiProvider',
  perplexity: 'perplexityProvider',
  together: 'togetherProvider',
  sambanova: 'sambaNovaProvider',
  anthropic: 'anthropicProvider',
  fireworks: 'fireworksProvider',
  deepinfra: 'deepInfraProvider',
  dashscope: 'dashScopeProvider',
  cohere: 'cohereProvider',
};

const toolStatusTranslationKeys: Record<ExportToolStatus, TranslationKey> = {
  awaitingApproval: 'toolAwaitingApproval',
  waitingForUser: 'toolWaitingForUser',
  running: 'toolRunning',
  completed: 'toolCompleted',
  failed: 'toolFailed',
  denied: 'toolDenied',
  cancelled: 'toolCancelled',
};

export function conversationMarkdownLabels(conversation: ChatConversation, t: Translate): ConversationMarkdownLabels {
  const providerKey = conversation.providerId
    ? (providerTranslationKeys[conversation.providerId] ?? 'chatGptProvider')
    : 'chatGptProvider';
  return {
    provider: t(providerKey),
    modelUnavailable: t('messageModelUnavailable'),
    user: t('userMessage'),
    exportProvider: t('conversationExportProvider'),
    exportModel: t('conversationExportModel'),
    exportCreated: t('conversationExportCreated'),
    exportStatus: t('conversationExportStatus'),
    exportToolActivity: t('conversationExportToolActivity'),
    responseInProgress: t('responseInProgress'),
    responseFailed: t('responseFailed'),
    responseStopped: t('responseStopped'),
    reasoningSummary: t('reasoningSummary'),
    toolAwaitingApproval: t('toolAwaitingApproval'),
    toolWaitingForUser: t('toolWaitingForUser'),
    toolRunning: t('toolRunning'),
    toolCompleted: t('toolCompleted'),
    toolFailed: t('toolFailed'),
    toolDenied: t('toolDenied'),
    toolCancelled: t('toolCancelled'),
    toolStatus: (status) => t(toolStatusTranslationKeys[status]),
    secondsShort: (seconds) => t('secondsShort', { count: seconds }),
  };
}
