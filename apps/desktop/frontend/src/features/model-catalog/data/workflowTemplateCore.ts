const maximumTemplates = 50;
const maximumNameLength = 64;
const maximumDescriptionLength = 256;
const maximumPromptBytes = 32 * 1024;
const maximumParameters = 16;
const maximumTools = 32;

export type WorkflowTemplateValidationErrorKind = 'invalid' | 'duplicate';

export class WorkflowTemplateValidationError extends Error {
  readonly kind: WorkflowTemplateValidationErrorKind;

  constructor(kind: WorkflowTemplateValidationErrorKind) {
    super(kind === 'duplicate' ? 'Workflow templates are duplicated.' : 'Workflow templates are invalid.');
    this.name = 'WorkflowTemplateValidationError';
    this.kind = kind;
  }
}

export interface WorkflowParameter {
  name: string;
  label?: string;
  defaultValue?: string;
  required?: boolean;
}

export interface WorkflowTemplate {
  id: string;
  name: string;
  description?: string;
  startingPrompt: string;
  parameters: WorkflowParameter[];
  providerId?: string;
  modelId?: string;
  systemPrompt?: string;
  tools?: string[];
  createdAt: number;
  updatedAt: number;
}

export interface WorkflowTemplateDraft {
  id: string | null;
  name: string;
  description?: string;
  startingPrompt: string;
  parameters?: WorkflowParameter[];
  providerId?: string;
  modelId?: string;
  systemPrompt?: string;
  tools?: string[];
}

export function parseWorkflowTemplates(value: unknown): WorkflowTemplate[] {
  if (!Array.isArray(value) || value.length > maximumTemplates) {
    throw new WorkflowTemplateValidationError('invalid');
  }
  const templates = value.map(parseWorkflowTemplate);
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const template of templates) {
    const normalizedName = template.name.toLocaleLowerCase('en-US');
    if (ids.has(template.id) || names.has(normalizedName)) {
      throw new WorkflowTemplateValidationError('duplicate');
    }
    ids.add(template.id);
    names.add(normalizedName);
  }
  return templates;
}

export function parseWorkflowTemplate(value: unknown): WorkflowTemplate {
  if (!isRecord(value)) throw new WorkflowTemplateValidationError('invalid');
  const {
    id,
    name,
    description,
    startingPrompt,
    prompt,
    parameters: rawParams,
    providerId,
    modelId,
    systemPrompt,
    tools: rawTools,
    createdAt,
    updatedAt,
  } = value;

  const resolvedPrompt = typeof startingPrompt === 'string' ? startingPrompt : typeof prompt === 'string' ? prompt : '';

  if (
    typeof id !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(id) ||
    typeof name !== 'string' ||
    name.trim() !== name ||
    name.length === 0 ||
    name.length > maximumNameLength ||
    typeof resolvedPrompt !== 'string' ||
    resolvedPrompt.trim().length === 0 ||
    new TextEncoder().encode(resolvedPrompt).byteLength > maximumPromptBytes ||
    !isTimestamp(createdAt) ||
    !isTimestamp(updatedAt)
  ) {
    throw new WorkflowTemplateValidationError('invalid');
  }

  if (
    description !== undefined &&
    (typeof description !== 'string' || description.length > maximumDescriptionLength)
  ) {
    throw new WorkflowTemplateValidationError('invalid');
  }

  if (providerId !== undefined && (typeof providerId !== 'string' || providerId.length > 64)) {
    throw new WorkflowTemplateValidationError('invalid');
  }

  if (modelId !== undefined && (typeof modelId !== 'string' || modelId.length > 256)) {
    throw new WorkflowTemplateValidationError('invalid');
  }

  if (systemPrompt !== undefined && (typeof systemPrompt !== 'string' || systemPrompt.length > 32 * 1024)) {
    throw new WorkflowTemplateValidationError('invalid');
  }

  let tools: string[] | undefined;
  if (rawTools !== undefined) {
    if (!Array.isArray(rawTools) || rawTools.length > maximumTools || !rawTools.every((t) => typeof t === 'string')) {
      throw new WorkflowTemplateValidationError('invalid');
    }
    tools = [...rawTools];
  }

  const detectedParamNames = workflowParameterNames(resolvedPrompt);
  if (detectedParamNames.length > maximumParameters) throw new WorkflowTemplateValidationError('invalid');

  const parameters: WorkflowParameter[] = [];
  if (Array.isArray(rawParams)) {
    for (const p of rawParams) {
      if (!isRecord(p) || typeof p.name !== 'string' || !p.name.trim()) {
        throw new WorkflowTemplateValidationError('invalid');
      }
      parameters.push({
        name: p.name.trim(),
        label: typeof p.label === 'string' ? p.label : undefined,
        defaultValue: typeof p.defaultValue === 'string' ? p.defaultValue : undefined,
        required: p.required === true,
      });
    }
  } else {
    for (const pName of detectedParamNames) {
      parameters.push({ name: pName });
    }
  }

  return {
    id,
    name,
    description: description || undefined,
    startingPrompt: resolvedPrompt,
    parameters,
    providerId: providerId || undefined,
    modelId: modelId || undefined,
    systemPrompt: systemPrompt || undefined,
    tools,
    createdAt,
    updatedAt,
  };
}

export function workflowParameterNames(prompt: string): string[] {
  const names = new Set<string>();
  for (const match of prompt.matchAll(/\{\{([a-zA-Z][a-zA-Z0-9_]{0,47})\}\}/g)) {
    const name = match[1];
    if (name) names.add(name);
    if (names.size > maximumParameters) break;
  }
  return [...names];
}

export function expandWorkflowTemplate(prompt: string, values: Record<string, string>): string {
  return prompt.replace(/\{\{([a-zA-Z][a-zA-Z0-9_]{0,47})\}\}/g, (_, name: string) => values[name] ?? '');
}

export function isWorkflowTemplateNameAvailable(
  templates: WorkflowTemplate[],
  name: string,
  currentId: string | null,
): boolean {
  const normalizedName = name.trim().toLocaleLowerCase('en-US');
  return !templates.some(
    (template) => template.id !== currentId && template.name.toLocaleLowerCase('en-US') === normalizedName,
  );
}

export function exportWorkflowTemplatesJson(templates: WorkflowTemplate[]): string {
  return JSON.stringify(
    {
      version: 1,
      exportedAt: Date.now(),
      templates,
    },
    null,
    2,
  );
}

export function importWorkflowTemplatesJson(
  jsonText: string,
  existingTemplates: WorkflowTemplate[] = [],
): { imported: WorkflowTemplate[]; skippedCount: number } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(jsonText);
  } catch {
    throw new WorkflowTemplateValidationError('invalid');
  }

  const rawList = isRecord(parsed) && Array.isArray(parsed.templates) ? parsed.templates : parsed;
  if (!Array.isArray(rawList)) {
    throw new WorkflowTemplateValidationError('invalid');
  }

  const existingIds = new Set(existingTemplates.map((t) => t.id));
  const existingNames = new Set(existingTemplates.map((t) => t.name.toLocaleLowerCase('en-US')));
  const imported: WorkflowTemplate[] = [];
  let skippedCount = 0;

  for (const item of rawList) {
    try {
      const template = parseWorkflowTemplate(item);
      const normalizedName = template.name.toLocaleLowerCase('en-US');
      if (existingIds.has(template.id) || existingNames.has(normalizedName)) {
        skippedCount += 1;
        continue;
      }
      existingIds.add(template.id);
      existingNames.add(normalizedName);
      imported.push(template);
    } catch {
      skippedCount += 1;
    }
  }

  return { imported, skippedCount };
}

export const workflowTemplateLimits = {
  maximumNameLength,
  maximumDescriptionLength,
  maximumPromptBytes,
  maximumTemplates,
  maximumParameters,
  maximumTools,
} as const;

function isTimestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
