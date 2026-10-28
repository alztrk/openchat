import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const storageKey = 'library.prompt-templates.v1';
const maximumTemplates = 50;
const maximumNameLength = 64;
const maximumPromptBytes = 32 * 1024;
const maximumParameters = 16;

export interface PromptTemplate {
  id: string;
  name: string;
  prompt: string;
  createdAt: number;
  updatedAt: number;
}

export interface PromptTemplateDraft {
  id: string | null;
  name: string;
  prompt: string;
}

export function readPromptTemplates(): PromptTemplate[] {
  const encoded = unifiedStorage.getItem(storageKey);
  if (encoded === null) return [];
  return parsePromptTemplates(JSON.parse(encoded));
}

export function writePromptTemplates(templates: PromptTemplate[]): void {
  const parsed = parsePromptTemplates(templates);
  if (parsed.length === 0) {
    unifiedStorage.removeItem(storageKey);
    return;
  }
  unifiedStorage.setItem(storageKey, JSON.stringify(parsed));
}

export function parsePromptTemplates(value: unknown): PromptTemplate[] {
  if (!Array.isArray(value) || value.length > maximumTemplates) {
    throw new Error('Prompt templates are invalid.');
  }
  const templates = value.map(parsePromptTemplate);
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const template of templates) {
    const normalizedName = template.name.toLocaleLowerCase('en-US');
    if (ids.has(template.id) || names.has(normalizedName)) {
      throw new Error('Prompt templates are duplicated.');
    }
    ids.add(template.id);
    names.add(normalizedName);
  }
  return templates;
}

export function parsePromptTemplate(value: unknown): PromptTemplate {
  if (!isRecord(value)) throw new Error('A prompt template is invalid.');
  const { id, name, prompt, createdAt, updatedAt } = value;
  if (
    typeof id !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(id) ||
    typeof name !== 'string' ||
    name.trim() !== name ||
    name.length === 0 ||
    name.length > maximumNameLength ||
    typeof prompt !== 'string' ||
    prompt.trim().length === 0 ||
    new TextEncoder().encode(prompt).byteLength > maximumPromptBytes ||
    !isTimestamp(createdAt) ||
    !isTimestamp(updatedAt)
  ) {
    throw new Error('A prompt template is invalid.');
  }
  const parameters = promptParameterNames(prompt);
  if (parameters.length > maximumParameters) throw new Error('A prompt template has too many parameters.');
  return { id, name, prompt, createdAt, updatedAt };
}

export function promptParameterNames(prompt: string): string[] {
  const names = new Set<string>();
  for (const match of prompt.matchAll(/\{\{([a-zA-Z][a-zA-Z0-9_]{0,47})\}\}/g)) {
    const name = match[1];
    if (name) names.add(name);
    if (names.size > maximumParameters) break;
  }
  return [...names];
}

export function expandPromptTemplate(prompt: string, values: Record<string, string>): string {
  return prompt.replace(/\{\{([a-zA-Z][a-zA-Z0-9_]{0,47})\}\}/g, (_, name: string) => values[name] ?? '');
}

export function isPromptTemplateNameAvailable(
  templates: PromptTemplate[],
  name: string,
  currentId: string | null,
): boolean {
  const normalizedName = name.trim().toLocaleLowerCase('en-US');
  return !templates.some(
    (template) => template.id !== currentId && template.name.toLocaleLowerCase('en-US') === normalizedName,
  );
}

export const promptTemplateLimits = {
  maximumNameLength,
  maximumPromptBytes,
  maximumTemplates,
} as const;

function isTimestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
