import type { ChatSlashCommand } from '@/modules/conversations/components/ChatSlashCommandMenu';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { modelRouteKey } from '@/modules/providers/data/modelPreferences';

export type ChatCommandFeedbackKey =
  | 'chatRequestFailed'
  | 'chatGptFastModeSettingsLoadFailed'
  | 'chatGptFastModeSettingsSaveFailed'
  | 'conversationModelSaveFailed'
  | 'conversationTitleSaveFailed'
  | 'ghostTextSuggestionUnavailable'
  | 'messageHistoryLoadFailed'
  | 'slashCommandCompactBusy'
  | 'slashCommandCompactFailed'
  | 'slashCommandCompactNoArguments'
  | 'slashCommandCompactNoContextWindow'
  | 'slashCommandCompactNoConversation'
  | 'slashCommandCompactNothing'
  | 'slashCommandCompactSuccess'
  | 'slashCommandModelAmbiguous'
  | 'slashCommandModelNameRequired'
  | 'slashCommandModelNoMatch'
  | 'slashCommandModelUnavailable'
  | 'slashCommandReasoningLevelRequired'
  | 'slashCommandReasoningUnsupported'
  | 'slashCommandRenameNoConversation'
  | 'slashCommandRenameSuccess'
  | 'slashCommandRenameTitleRequired'
  | 'slashCommandSummarizeLoading'
  | 'slashCommandSummarizeNoMessages'
  | 'slashCommandSummarizeUnavailable';

export type TranslationKey = keyof typeof import('@/localization/locales/en.json');

export const providerTranslationKeys: Partial<Record<string, TranslationKey>> = {
  opencode: 'openCodeProvider',
  copilot: 'providerNameCopilot',
  chatgpt: 'chatGptProvider',
  chatgpt_api: 'chatGptProvider',
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
  llama_cpp: 'localEngines',
};

export const imageAttachmentExtensions = '.png,.jpg,.jpeg,.webp,.gif';
export const textAttachmentExtensions =
  '.pdf,.docx,.xlsx,.txt,.md,.csv,.json,.jsonl,.xml,.html,.htm,.css,.js,.jsx,.ts,.tsx,.dart,.rs,.py,.go,.java,.kt,.swift,.c,.h,.cpp,.hpp,.cs,.sh,.ps1,.yaml,.yml,.toml,.ini,.sql,.log';

export function effectiveContextWindow(model: ChatModel, limits: Record<string, number>): number | null {
  const configured = limits[modelRouteKey(model)];
  if (configured === undefined) return model.contextWindow;
  if (model.contextWindow === null) return configured;
  return Math.min(configured, model.contextWindow);
}

export function isMessageSendingCommand(command: ChatSlashCommand): boolean {
  return command === 'summarize' || command === 'compact' || command === 'plan' || command === 'goal';
}

export function providerNameForModel(model: ChatModel, t: (key: TranslationKey) => string): string {
  if (model.customProvider?.name) return model.customProvider.name;
  return providerNameForId(model.providerId, t);
}

export function providerNameForId(providerId: string, t: (key: TranslationKey) => string): string {
  const key = providerTranslationKeys[providerId];
  return key ? t(key) : providerId;
}

const reasoningTranslationKeys: Partial<Record<string, TranslationKey>> = {
  minimal: 'reasoningMinimal',
  low: 'reasoningLow',
  medium: 'reasoningMedium',
  high: 'reasoningHigh',
  xhigh: 'reasoningExtraHigh',
  extra_high: 'reasoningExtraHigh',
  'extra-high': 'reasoningExtraHigh',
  max: 'reasoningMax',
  maximum: 'reasoningMax',
  ultra: 'reasoningUltra',
  enabled: 'reasoningEnabled',
};

export function reasoningLevelLabel(level: string, t: (key: TranslationKey) => string): string {
  const key = reasoningTranslationKeys[level.toLocaleLowerCase('en')];
  return key ? t(key) : level;
}

export function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function parseSlashCommand(value: string): { command: ChatSlashCommand; arguments: string } | null {
  const match = /^\/(new|summarize|rename|model|compact|reasoning|plan|goal|hedef)(?:\s+([\s\S]*))?$/i.exec(
    value.trim(),
  );
  if (!match) return null;
  const name = match[1]?.toLocaleLowerCase('en');
  if (!name) return null;
  let command: ChatSlashCommand;
  switch (name) {
    case 'new':
      command = 'new';
      break;
    case 'summarize':
      command = 'summarize';
      break;
    case 'rename':
      command = 'rename';
      break;
    case 'model':
      command = 'model';
      break;
    case 'compact':
      command = 'compact';
      break;
    case 'reasoning':
      command = 'reasoning';
      break;
    case 'plan':
      command = 'plan';
      break;
    case 'goal':
    case 'hedef':
      command = 'goal';
      break;
    default:
      return null;
  }
  return { command, arguments: match[2] ?? '' };
}
