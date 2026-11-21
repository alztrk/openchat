const maximumWorkflowCount = 50;
const maximumRunCount = 500;
const maximumStepCount = 16;
const maximumNameLength = 64;
const maximumPromptBytes = 32 * 1024;

export type WorkflowSchedule =
  | { cadence: 'daily'; hour: number; minute: number }
  | { cadence: 'weekly'; weekday: number; hour: number; minute: number };

export interface WorkflowStep {
  id: string;
  name: string;
  prompt: string;
}

export interface WorkflowAutomation {
  id: string;
  name: string;
  description?: string;
  enabled: boolean;
  schedule: WorkflowSchedule;
  steps: WorkflowStep[];
  inputDefaults: Record<string, string>;
  providerId: string;
  modelId: string;
  projectId?: string;
  createdAt: number;
  updatedAt: number;
  nextRunAt: number;
}

export type WorkflowRunStatus = 'running' | 'completed' | 'failed' | 'interrupted';

export interface WorkflowRun {
  id: string;
  workflowId: string;
  workflowName: string;
  status: WorkflowRunStatus;
  scheduledAt: number | null;
  startedAt: number;
  finishedAt?: number;
  conversationId?: string;
  completedSteps: number;
  totalSteps: number;
  error?: string;
}

export class WorkflowAutomationValidationError extends Error {
  constructor() {
    super('Workflow automation data is invalid.');
    this.name = 'WorkflowAutomationValidationError';
  }
}

export function parseWorkflowAutomations(value: unknown): WorkflowAutomation[] {
  if (!Array.isArray(value) || value.length > maximumWorkflowCount) {
    throw new WorkflowAutomationValidationError();
  }
  const workflows = value.map(parseWorkflowAutomation);
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const workflow of workflows) {
    const normalizedName = workflow.name.toLocaleLowerCase('en-US');
    if (ids.has(workflow.id) || names.has(normalizedName)) throw new WorkflowAutomationValidationError();
    ids.add(workflow.id);
    names.add(normalizedName);
  }
  return workflows;
}

export function parseWorkflowAutomation(value: unknown): WorkflowAutomation {
  if (!isRecord(value)) throw new WorkflowAutomationValidationError();
  const { id, name, description, enabled, schedule, steps, inputDefaults, providerId, modelId, projectId, createdAt, updatedAt, nextRunAt } = value;
  if (
    !isUuid(id) ||
    typeof name !== 'string' || name.trim() !== name || name.length === 0 || name.length > maximumNameLength ||
    (description !== undefined && (typeof description !== 'string' || description.length > 256)) ||
    typeof enabled !== 'boolean' ||
    typeof providerId !== 'string' || providerId.length === 0 || providerId.length > 64 ||
    typeof modelId !== 'string' || modelId.length === 0 || modelId.length > 256 ||
    (projectId !== undefined && (typeof projectId !== 'string' || projectId.length > 128)) ||
    !isTimestamp(createdAt) || !isTimestamp(updatedAt) || !isTimestamp(nextRunAt) ||
    !Array.isArray(steps) || steps.length === 0 || steps.length > maximumStepCount ||
    !isRecord(inputDefaults) || Object.keys(inputDefaults).length > 16 ||
    !Object.values(inputDefaults).every((item) => typeof item === 'string' && item.length <= 4096)
  ) {
    throw new WorkflowAutomationValidationError();
  }
  const parsedSchedule = parseWorkflowSchedule(schedule);
  const parsedSteps = steps.map(parseWorkflowStep);
  const parsedInputDefaults = Object.fromEntries(
    Object.entries(inputDefaults).flatMap(([key, item]) => typeof item === 'string' ? [[key, item]] : []),
  );
  const stepIds = new Set(parsedSteps.map((step) => step.id));
  if (stepIds.size !== parsedSteps.length) throw new WorkflowAutomationValidationError();
  return {
    id,
    name,
    description: description || undefined,
    enabled,
    schedule: parsedSchedule,
    steps: parsedSteps,
    inputDefaults: parsedInputDefaults,
    providerId,
    modelId,
    projectId: projectId || undefined,
    createdAt,
    updatedAt,
    nextRunAt,
  };
}

export function parseWorkflowRuns(value: unknown): WorkflowRun[] {
  if (!Array.isArray(value) || value.length > maximumRunCount) throw new WorkflowAutomationValidationError();
  return value.map((run) => {
    if (
      !isRecord(run) || !isUuid(run.id) || !isUuid(run.workflowId) ||
      typeof run.workflowName !== 'string' || run.workflowName.length > maximumNameLength ||
      !isWorkflowRunStatus(run.status) ||
      (run.scheduledAt !== null && !isTimestamp(run.scheduledAt)) ||
      !isTimestamp(run.startedAt) ||
      (run.finishedAt !== undefined && !isTimestamp(run.finishedAt)) ||
      (run.conversationId !== undefined && (typeof run.conversationId !== 'string' || run.conversationId.length > 128)) ||
      typeof run.completedSteps !== 'number' || !Number.isInteger(run.completedSteps) || run.completedSteps < 0 ||
      typeof run.totalSteps !== 'number' || !Number.isInteger(run.totalSteps) || run.totalSteps < 1 || run.completedSteps > run.totalSteps ||
      (run.error !== undefined && (typeof run.error !== 'string' || run.error.length > 512))
    ) {
      throw new WorkflowAutomationValidationError();
    }
    return {
      id: run.id,
      workflowId: run.workflowId,
      workflowName: run.workflowName,
      status: run.status,
      scheduledAt: run.scheduledAt,
      startedAt: run.startedAt,
      finishedAt: run.finishedAt,
      conversationId: run.conversationId,
      completedSteps: run.completedSteps,
      totalSteps: run.totalSteps,
      error: run.error,
    };
  });
}

export function parseWorkflowStep(value: unknown): WorkflowStep {
  if (
    !isRecord(value) || !isUuid(value.id) ||
    typeof value.name !== 'string' || value.name.trim() !== value.name || value.name.length === 0 || value.name.length > maximumNameLength ||
    typeof value.prompt !== 'string' || value.prompt.trim().length === 0 ||
    new TextEncoder().encode(value.prompt).byteLength > maximumPromptBytes
  ) {
    throw new WorkflowAutomationValidationError();
  }
  return { id: value.id, name: value.name, prompt: value.prompt };
}

export function nextWorkflowOccurrence(schedule: WorkflowSchedule, after: number): number {
  if (!isTimestamp(after)) throw new WorkflowAutomationValidationError();
  schedule = parseWorkflowSchedule(schedule);
  const candidate = new Date(after);
  candidate.setHours(schedule.hour, schedule.minute, 0, 0);
  if (schedule.cadence === 'weekly') {
    candidate.setDate(candidate.getDate() + ((schedule.weekday - candidate.getDay() + 7) % 7));
  }
  if (candidate.getTime() <= after) {
    candidate.setDate(candidate.getDate() + (schedule.cadence === 'daily' ? 1 : 7));
  }
  return candidate.getTime();
}

export function completeWorkflowRun(
  run: WorkflowRun,
  status: Exclude<WorkflowRunStatus, 'running'>,
  finishedAt: number,
  completedSteps: number,
  conversationId?: string,
  error?: string,
): WorkflowRun {
  if (!isTimestamp(finishedAt) || !Number.isInteger(completedSteps) || completedSteps < 0 || completedSteps > run.totalSteps) {
    throw new WorkflowAutomationValidationError();
  }
  return {
    ...run,
    status,
    finishedAt,
    completedSteps,
    ...(conversationId ? { conversationId } : {}),
    ...(error ? { error: error.slice(0, 512) } : {}),
  };
}

function parseWorkflowSchedule(value: unknown): WorkflowSchedule {
  if (
    !isRecord(value) ||
    typeof value.hour !== 'number' || !Number.isInteger(value.hour) || value.hour < 0 || value.hour > 23 ||
    typeof value.minute !== 'number' || !Number.isInteger(value.minute) || value.minute < 0 || value.minute > 59
  ) {
    throw new WorkflowAutomationValidationError();
  }
  if (value.cadence === 'daily') return { cadence: 'daily', hour: value.hour, minute: value.minute };
  if (
    value.cadence === 'weekly' && typeof value.weekday === 'number' &&
    Number.isInteger(value.weekday) && value.weekday >= 0 && value.weekday <= 6
  ) {
    return { cadence: 'weekly', weekday: value.weekday, hour: value.hour, minute: value.minute };
  }
  throw new WorkflowAutomationValidationError();
}

function isWorkflowRunStatus(value: unknown): value is WorkflowRunStatus {
  return value === 'running' || value === 'completed' || value === 'failed' || value === 'interrupted';
}

function isTimestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isUuid(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export const workflowAutomationLimits = { maximumWorkflowCount, maximumRunCount, maximumStepCount } as const;
