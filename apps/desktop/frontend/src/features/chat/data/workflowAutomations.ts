import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';
import {
  parseWorkflowAutomations,
  parseWorkflowRuns,
  WorkflowAutomationValidationError,
  type WorkflowAutomation,
  type WorkflowRun,
} from './workflowAutomationCore';

const workflowsKey = 'automation.workflows.v1';
const runsKey = 'automation.workflow-runs.v1';

export class WorkflowAutomationStorageError extends Error {
  readonly rawValue: string | null;

  constructor(rawValue: string | null = null) {
    super('Workflow automation storage is unavailable or invalid.');
    this.name = 'WorkflowAutomationStorageError';
    this.rawValue = rawValue;
  }
}

export function readWorkflowAutomations(): WorkflowAutomation[] {
  const encoded = readStoredValue(workflowsKey);
  if (encoded === null) return [];
  try {
    return parseWorkflowAutomations(JSON.parse(encoded));
  } catch (cause: unknown) {
    if (cause instanceof WorkflowAutomationValidationError || cause instanceof SyntaxError) {
      throw new WorkflowAutomationStorageError(encoded);
    }
    throw cause;
  }
}

export function readWorkflowRuns(): WorkflowRun[] {
  const encoded = readStoredValue(runsKey);
  if (encoded === null) return [];
  try {
    return parseWorkflowRuns(JSON.parse(encoded));
  } catch (cause: unknown) {
    if (cause instanceof WorkflowAutomationValidationError || cause instanceof SyntaxError) {
      throw new WorkflowAutomationStorageError(encoded);
    }
    throw cause;
  }
}

export async function writeWorkflowAutomations(workflows: WorkflowAutomation[]): Promise<void> {
  const parsed = parseWorkflowAutomations(workflows);
  await writeStoredValue(workflowsKey, parsed);
  notifyWorkflowAutomationUpdate();
}

export async function writeWorkflowRuns(runs: WorkflowRun[]): Promise<void> {
  const parsed = parseWorkflowRuns(runs.slice(-500));
  await writeStoredValue(runsKey, parsed);
  notifyWorkflowAutomationUpdate();
}

function notifyWorkflowAutomationUpdate(): void {
  if (typeof window !== 'undefined') window.dispatchEvent(new Event('openchat-workflow-automations-updated'));
}

function readStoredValue(key: string): string | null {
  try {
    return unifiedStorage.getItem(key);
  } catch {
    throw new WorkflowAutomationStorageError();
  }
}

async function writeStoredValue(key: string, value: unknown): Promise<void> {
  try {
    await unifiedStorage.setPersistedItem(key, JSON.stringify(value));
  } catch {
    throw new WorkflowAutomationStorageError();
  }
}
