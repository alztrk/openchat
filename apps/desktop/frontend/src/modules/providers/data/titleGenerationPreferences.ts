import type { ChatModel } from './chatModels';
import {
  type DefaultModelPreference,
  defaultPreferenceForModel,
  parseDefaultModelPreference,
} from './modelPreferences';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type TitleGenerationMode = 'conversation' | 'selectedModel' | 'lightweight';

export interface TitleGenerationPreference {
  mode: TitleGenerationMode;
  model: DefaultModelPreference | null;
}

const preferenceKey = 'chat.title_generation';

export function readTitleGenerationPreference(): TitleGenerationPreference {
  const encoded = unifiedStorage.getItem(preferenceKey);
  if (!encoded) return { mode: 'conversation', model: null };
  const value: unknown = JSON.parse(encoded);
  return parsePreference(value);
}

export function writeTitleGenerationPreference(preference: TitleGenerationPreference): void {
  if (preference.mode === 'selectedModel' && preference.model === null) {
    throw new Error('A model is required for selected title generation.');
  }
  const value: Record<string, unknown> = { mode: preference.mode };
  if (preference.model !== null) value.model = preference.model;
  unifiedStorage.setItem(preferenceKey, JSON.stringify(value));
}

export function titlePreferenceForModel(model: ChatModel): DefaultModelPreference {
  return defaultPreferenceForModel(model);
}

function parsePreference(value: unknown): TitleGenerationPreference {
  if (!isObject(value)) throw new Error('The title generation preference is invalid.');
  const mode = value.mode;
  if (mode !== 'conversation' && mode !== 'selectedModel' && mode !== 'lightweight') {
    throw new Error('The title generation mode is invalid.');
  }
  const model = value.model === undefined || value.model === null ? null : parseDefaultModelPreference(value.model);
  if (mode === 'conversation' && model !== null) {
    throw new Error('The conversation title preference cannot include a model.');
  }
  if (mode === 'selectedModel' && model === null) {
    throw new Error('The selected title model is missing.');
  }
  return { mode, model };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
