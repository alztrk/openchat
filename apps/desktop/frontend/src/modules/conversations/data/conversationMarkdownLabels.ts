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

function safeFormatDateTime(value: number | string | Date | null | undefined, locale: string): string {
  if (value === null || value === undefined) return '';
  try {
    const date = typeof value === 'object' ? value : new Date(value);
    if (Number.isNaN(date.getTime())) return '';
    return new Intl.DateTimeFormat(locale, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }).format(date);
  } catch {
    return String(value);
  }
}

function safeFormatTime(value: number | string | Date | null | undefined, locale: string): string {
  if (value === null || value === undefined) return '';
  try {
    const date = typeof value === 'object' ? value : new Date(value);
    if (Number.isNaN(date.getTime())) return '';
    return new Intl.DateTimeFormat(locale, {
      hour: '2-digit',
      minute: '2-digit',
    }).format(date);
  } catch {
    return '';
  }
}

export function conversationMarkdownLabels(
  conversation: ChatConversation,
  t: Translate,
  locale = 'tr',
): ConversationMarkdownLabels {
  const providerKey = conversation.providerId
    ? (providerTranslationKeys[conversation.providerId] ?? 'chatGptProvider')
    : 'chatGptProvider';
  const effectiveLocale = locale.trim() || 'tr';

  return {
    locale: effectiveLocale,
    provider: t(providerKey),
    modelUnavailable: t('messageModelUnavailable'),
    user: t('userMessage'),
    assistant: t('conversationExportAssistant'),
    exportProvider: t('conversationExportProvider'),
    exportModel: t('conversationExportModel'),
    exportCreated: t('conversationExportCreated'),
    exportStatus: t('conversationExportStatus'),
    exportToolActivity: t('conversationExportToolActivity'),
    exportMessagesCount: t('conversationExportMessagesCount'),
    exportTitleBadge: t('conversationExportTitleBadge'),
    exportInput: t('conversationExportInput'),
    exportOutput: t('conversationExportOutput'),
    exportTargetPath: t('conversationExportTargetPath'),
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
    formatDateTime: (val) => safeFormatDateTime(val, effectiveLocale),
    formatTime: (val) => safeFormatTime(val, effectiveLocale),
  };
}
