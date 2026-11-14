import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';
import {
  parseWorkflowTemplates,
  WorkflowTemplateValidationError,
  type WorkflowTemplate,
} from './workflowTemplateCore';

export * from './workflowTemplateCore';

const storageKey = 'library.workflow-templates.v1';

export class WorkflowTemplateReadError extends Error {
  readonly kind: 'unavailable' | 'corrupt';
  readonly rawValue: string | null;

  constructor(kind: 'unavailable' | 'corrupt', rawValue: string | null = null) {
    super(kind === 'corrupt' ? 'Saved workflow template data is invalid.' : 'Workflow template storage is unavailable.');
    this.name = 'WorkflowTemplateReadError';
    this.kind = kind;
    this.rawValue = rawValue;
  }
}

export function readWorkflowTemplates(): WorkflowTemplate[] {
  let encoded: string | null;
  try {
    encoded = unifiedStorage.getItem(storageKey);
  } catch {
    throw new WorkflowTemplateReadError('unavailable');
  }
  if (encoded === null) return [];

  let value: unknown;
  try {
    value = JSON.parse(encoded);
  } catch {
    throw new WorkflowTemplateReadError('corrupt', encoded);
  }

  try {
    return parseWorkflowTemplates(value);
  } catch (cause: unknown) {
    if (cause instanceof WorkflowTemplateValidationError) {
      throw new WorkflowTemplateReadError('corrupt', encoded);
    }
    throw cause;
  }
}

export async function writeWorkflowTemplates(templates: WorkflowTemplate[]): Promise<void> {
  const parsed = parseWorkflowTemplates(templates);
  if (parsed.length === 0) {
    await unifiedStorage.removePersistedItem(storageKey);
    return;
  }
  await unifiedStorage.setPersistedItem(storageKey, JSON.stringify(parsed));
}

export async function resetWorkflowTemplates(): Promise<void> {
  await unifiedStorage.removePersistedItem(storageKey);
}
