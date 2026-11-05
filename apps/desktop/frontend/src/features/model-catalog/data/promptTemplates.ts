import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';

const storageKey = 'library.prompt-templates.v1';
const maximumTemplates = 50;
const maximumNameLength = 64;
const maximumPromptBytes = 32 * 1024;
const maximumParameters = 16;

type PromptTemplateValidationErrorKind = 'invalid' | 'duplicate';

class PromptTemplateValidationError extends Error {
  constructor(readonly kind: PromptTemplateValidationErrorKind) {
    super(kind === 'duplicate' ? 'Prompt templates are duplicated.' : 'Prompt templates are invalid.');
    this.name = 'PromptTemplateValidationError';
  }
}

export class PromptTemplateReadError extends Error {
  constructor(
    readonly kind: 'unavailable' | 'corrupt',
    readonly rawValue: string | null = null,
  ) {
    super(kind === 'corrupt' ? 'Saved prompt template data is invalid.' : 'Prompt template storage is unavailable.');
    this.name = 'PromptTemplateReadError';
  }
}

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
  let encoded: string | null;
  try {
    encoded = unifiedStorage.getItem(storageKey);
  } catch {
    throw new PromptTemplateReadError('unavailable');
  }
  if (encoded === null) return [];

  let value: unknown;
  try {
    value = JSON.parse(encoded);
  } catch {
    throw new PromptTemplateReadError('corrupt', encoded);
  }

  try {
    return parsePromptTemplates(value);
  } catch (cause: unknown) {
    if (cause instanceof PromptTemplateValidationError) {
      throw new PromptTemplateReadError('corrupt', encoded);
    }
    throw cause;
  }
}

export async function writePromptTemplates(templates: PromptTemplate[]): Promise<void> {
  const parsed = parsePromptTemplates(templates);
  if (parsed.length === 0) {
    await unifiedStorage.removePersistedItem(storageKey);
    return;
  }
  await unifiedStorage.setPersistedItem(storageKey, JSON.stringify(parsed));
}

export async function resetPromptTemplates(): Promise<void> {
  await unifiedStorage.removePersistedItem(storageKey);
}

export function parsePromptTemplates(value: unknown): PromptTemplate[] {
  if (!Array.isArray(value) || value.length > maximumTemplates) {
    throw new PromptTemplateValidationError('invalid');
  }
  const templates = value.map(parsePromptTemplate);
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const template of templates) {
    const normalizedName = template.name.toLocaleLowerCase('en-US');
    if (ids.has(template.id) || names.has(normalizedName)) {
      throw new PromptTemplateValidationError('duplicate');
    }
    ids.add(template.id);
    names.add(normalizedName);
  }
  return templates;
}

export function parsePromptTemplate(value: unknown): PromptTemplate {
  if (!isRecord(value)) throw new PromptTemplateValidationError('invalid');
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
    throw new PromptTemplateValidationError('invalid');
  }
  const parameters = promptParameterNames(prompt);
  if (parameters.length > maximumParameters) throw new PromptTemplateValidationError('invalid');
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
