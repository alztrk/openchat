import { type ChatFileChange, parseChatFileChange } from '@/modules/conversations/data/chatFileChanges';

export interface CitationSource {
  id: string;
  title: string;
  url: string;
  sourceType: string;
  snippet: string | null;
  retrievedAtUnixMs: number | null;
}

export interface ReasoningSummary {
  id: string;
  content: string;
  elapsedMicroseconds: number | null;
  isComplete: boolean;
}

export type ToolActivityStatus =
  | 'awaitingApproval'
  | 'waitingForUser'
  | 'running'
  | 'completed'
  | 'failed'
  | 'denied'
  | 'cancelled';

export interface ToolActivity {
  callId: string;
  name: string;
  arguments: unknown;
  output: unknown;
  targetPath: string | null;
  fileChanges: ChatFileChange[];
  fileChangesError: string | null;
  status: ToolActivityStatus;
}

export function parseCitationSources(encoded: string): CitationSource[] {
  const value = parseJson(encoded);
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const sources: CitationSource[] = [];
  for (const item of value) {
    if (!isRecord(item)) continue;
    const id = item.id;
    const title = item.title;
    const url = item.url;
    const sourceType = item.sourceType;
    if (
      typeof id !== 'string' ||
      !/^\[?[PSU]\d+(?:-[A-Za-z0-9]+)?\]?$/.test(id) ||
      typeof title !== 'string' ||
      title.trim().length === 0 ||
      typeof url !== 'string' ||
      !isSafeHttpUrl(url) ||
      typeof sourceType !== 'string' ||
      seen.has(id.replace(/^\[|\]$/g, ''))
    ) {
      continue;
    }
    seen.add(id.replace(/^\[|\]$/g, ''));
    sources.push({
      id: id.replace(/^\[|\]$/g, ''),
      title: title.trim(),
      url,
      sourceType,
      snippet: typeof item.snippet === 'string' && item.snippet.length > 0 ? item.snippet : null,
      retrievedAtUnixMs: isSafeTimestamp(item.retrievedAtUnixMs) ? item.retrievedAtUnixMs : null,
    });
    if (sources.length >= 32) break;
  }
  return sources;
}

export function parseReasoningSummaries(encoded: string): ReasoningSummary[] {
  const value = parseJson(encoded);
  if (!Array.isArray(value)) return [];
  const summaries: ReasoningSummary[] = [];
  for (const item of value) {
    if (!isRecord(item)) continue;
    const id = item.id;
    const content = item.content;
    const elapsedMicroseconds = item.elapsedMicroseconds;
    const isComplete = item.isComplete;
    if (
      typeof id !== 'string' ||
      id.length === 0 ||
      typeof content !== 'string' ||
      typeof isComplete !== 'boolean' ||
      (elapsedMicroseconds !== null &&
        elapsedMicroseconds !== undefined &&
        (!isFiniteNumber(elapsedMicroseconds) || elapsedMicroseconds < 0))
    ) {
      continue;
    }
    summaries.push({
      id,
      content,
      elapsedMicroseconds: isFiniteNumber(elapsedMicroseconds) ? elapsedMicroseconds : null,
      isComplete,
    });
  }
  return summaries;
}

export function parseToolActivities(encoded: string): ToolActivity[] {
  const value = parseJson(encoded);
  if (!Array.isArray(value)) return [];
  const activities: ToolActivity[] = [];
  for (const item of value) {
    if (!isRecord(item)) continue;
    const callId = item.callId;
    const name = item.name;
    const status = item.status;
    if (
      typeof callId !== 'string' ||
      callId.length === 0 ||
      typeof name !== 'string' ||
      name.length === 0 ||
      !isToolActivityStatus(status) ||
      !Object.hasOwn(item, 'arguments')
    ) {
      continue;
    }
    activities.push({
      callId,
      name,
      arguments: item.arguments,
      output: Object.hasOwn(item, 'output') ? item.output : null,
      targetPath: typeof item.targetPath === 'string' ? item.targetPath : null,
      fileChanges: Array.isArray(item.fileChanges)
        ? item.fileChanges.flatMap((change) => {
            const parsed = parseChatFileChange(change);
            return parsed ? [parsed] : [];
          })
        : [],
      fileChangesError: typeof item.fileChangesError === 'string' ? item.fileChangesError : null,
      status,
    });
  }
  return activities;
}

export function isSafeHttpUrl(value: string): boolean {
  if (value.length === 0 || value.length > 4096) return false;
  try {
    const url = new URL(value);
    return (
      (url.protocol === 'https:' || url.protocol === 'http:') &&
      url.hostname.length > 0 &&
      url.username.length === 0 &&
      url.password.length === 0
    );
  } catch {
    return false;
  }
}

export function formatJsonValue(value: unknown): string {
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value, null, 2) ?? String(value);
  } catch {
    return String(value);
  }
}

function parseJson(value: string): unknown {
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed;
  } catch {
    return null;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isSafeTimestamp(value: unknown): value is number {
  return isFiniteNumber(value) && value >= 0 && value <= 8640000000000000;
}

function isToolActivityStatus(value: unknown): value is ToolActivityStatus {
  return (
    value === 'awaitingApproval' ||
    value === 'waitingForUser' ||
    value === 'running' ||
    value === 'completed' ||
    value === 'failed' ||
    value === 'denied' ||
    value === 'cancelled'
  );
}
