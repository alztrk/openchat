import { localServiceClient, type ServiceEvent } from '@/platform/local-service';

const maximumDisplayedTextLength = 64 * 1024;
const maximumIdentifierLength = 128;

export type SubagentStatus = 'running' | 'paused' | 'completed' | 'cancelled' | 'interrupted' | 'failed';

export interface SubagentProgress {
  phase: string | null;
  toolName: string | null;
}

export interface SubagentRun {
  runId: string;
  conversationId: string;
  status: SubagentStatus;
  agentName: string | null;
  providerId: string | null;
  modelId: string | null;
  reasoningEffort: string | null;
  objective: string | null;
  progress: SubagentProgress | null;
  text: string;
  summary: string | null;
}

export interface SubagentProgressUpdate {
  conversationId: string;
  runId: string;
  status: SubagentStatus;
  agentName: string | null;
  phase: string | null;
  toolName: string | null;
  delta: string | null;
}

export async function listSubagentRuns(conversationId: string): Promise<SubagentRun[]> {
  const response = await localServiceClient.call('chat.subagents.list', { conversationId, limit: 100 });
  if (!Array.isArray(response.runs)) throw new Error('The subagent list response is invalid.');
  return response.runs.map(parseSubagentRun);
}

export async function cancelSubagentRun(conversationId: string, runId: string): Promise<boolean> {
  const response = await localServiceClient.call('chat.subagents.cancel', { conversationId, runId });
  if (typeof response.accepted !== 'boolean') throw new Error('The subagent cancellation response is invalid.');
  return response.accepted;
}

export function parseSubagentProgressEvent(event: ServiceEvent): SubagentProgressUpdate | null {
  if (event.name !== 'chat.subagent.updated') return null;
  const data = event.data;
  if (
    !isIdentifier(data.conversationId) ||
    !isIdentifier(data.runId) ||
    !isSubagentStatus(data.status) ||
    !isOptionalString(data.agentName) ||
    !isOptionalString(data.phase) ||
    !isOptionalString(data.toolName) ||
    !isOptionalString(data.delta)
  ) {
    return null;
  }
  return {
    conversationId: data.conversationId,
    runId: data.runId,
    status: data.status,
    agentName: data.agentName ?? null,
    phase: data.phase ?? null,
    toolName: data.toolName ?? null,
    delta: data.delta ?? null,
  };
}

export function applySubagentProgressUpdate(runs: SubagentRun[], update: SubagentProgressUpdate): SubagentRun[] {
  const currentIndex = runs.findIndex((run) => run.runId === update.runId);
  const current = currentIndex < 0 ? null : runs[currentIndex];
  const next: SubagentRun = {
    runId: update.runId,
    conversationId: update.conversationId,
    status: update.status,
    agentName: update.agentName ?? current?.agentName ?? null,
    providerId: current?.providerId ?? null,
    modelId: current?.modelId ?? null,
    reasoningEffort: current?.reasoningEffort ?? null,
    objective: current?.objective ?? null,
    progress:
      update.phase === null && update.toolName === null
        ? (current?.progress ?? null)
        : { phase: update.phase, toolName: update.toolName },
    text: update.delta ? appendBoundedText(current?.text ?? '', update.delta) : (current?.text ?? ''),
    summary: current?.summary ?? null,
  };
  if (currentIndex < 0) return [next, ...runs];
  return runs.map((run, index) => (index === currentIndex ? next : run));
}

function parseSubagentRun(value: unknown): SubagentRun {
  if (!isRecord(value)) throw new Error('A subagent run record is invalid.');
  if (
    !isIdentifier(value.runId) ||
    !isIdentifier(value.conversationId) ||
    !isSubagentStatus(value.status) ||
    !isOptionalString(value.agentName) ||
    !isOptionalString(value.providerId) ||
    !isOptionalString(value.modelId) ||
    !isOptionalString(value.reasoningEffort) ||
    !isOptionalString(value.objective) ||
    !isOptionalString(value.text) ||
    !isOptionalString(value.summary)
  ) {
    throw new Error('A subagent run record is invalid.');
  }
  return {
    runId: value.runId,
    conversationId: value.conversationId,
    status: value.status,
    agentName: value.agentName ?? null,
    providerId: value.providerId ?? null,
    modelId: value.modelId ?? null,
    reasoningEffort: value.reasoningEffort ?? null,
    objective: value.objective ?? null,
    progress: parseProgress(value.progress),
    text: (value.text ?? '').slice(0, maximumDisplayedTextLength),
    summary: value.summary ?? null,
  };
}

function parseProgress(value: unknown): SubagentProgress | null {
  if (value === null || value === undefined) return null;
  if (!isRecord(value) || !isOptionalString(value.phase) || !isOptionalString(value.toolName)) {
    throw new Error('A subagent progress record is invalid.');
  }
  return { phase: value.phase ?? null, toolName: value.toolName ?? null };
}

function appendBoundedText(current: string, delta: string): string {
  return `${current}${delta}`.slice(-maximumDisplayedTextLength);
}

function isSubagentStatus(value: unknown): value is SubagentStatus {
  return (
    value === 'running' ||
    value === 'paused' ||
    value === 'completed' ||
    value === 'cancelled' ||
    value === 'interrupted' ||
    value === 'failed'
  );
}

function isOptionalString(value: unknown): value is string | null | undefined {
  return value === undefined || value === null || typeof value === 'string';
}

function isIdentifier(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= maximumIdentifierLength;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
