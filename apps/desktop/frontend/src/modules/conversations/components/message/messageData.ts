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
  assistantTextBeforeByteOffset?: number | null;
  toolActivityCountBefore?: number | null;
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
  assistantTextBeforeByteOffset?: number | null;
  targetPath: string | null;
  fileChanges: ChatFileChange[];
  fileChangesError: string | null;
  recoveryCheckpointId: string | null;
  status: ToolActivityStatus;
}

export type AssistantTimelineSegment =
  | { kind: 'reasoning'; key: string; summaries: ReasoningSummary[] }
  | { kind: 'tools'; key: string; activities: ToolActivity[] }
  | { kind: 'text'; key: string; content: string };

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
      assistantTextBeforeByteOffset: isNonNegativeInteger(item.assistantTextBeforeByteOffset)
        ? item.assistantTextBeforeByteOffset
        : null,
      toolActivityCountBefore: isNonNegativeInteger(item.toolActivityCountBefore) ? item.toolActivityCountBefore : null,
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
      assistantTextBeforeByteOffset: isNonNegativeInteger(item.assistantTextBeforeByteOffset)
        ? item.assistantTextBeforeByteOffset
        : null,
      targetPath: typeof item.targetPath === 'string' ? item.targetPath : null,
      fileChanges: Array.isArray(item.fileChanges)
        ? item.fileChanges.flatMap((change) => {
            const parsed = parseChatFileChange(change);
            return parsed ? [parsed] : [];
          })
        : [],
      fileChangesError: typeof item.fileChangesError === 'string' ? item.fileChangesError : null,
      recoveryCheckpointId:
        typeof item.recoveryCheckpointId === 'string' && /^[0-9a-f]{32}$/.test(item.recoveryCheckpointId)
          ? item.recoveryCheckpointId
          : null,
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

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

const timelineEncoder = new TextEncoder();
const timelineDecoder = new TextDecoder('utf-8');

export function buildAssistantTimeline(
  rawContent: string,
  inBandReasoning: { visibleContent: string; reasoningContent: string },
  reasoningSummaries: readonly ReasoningSummary[],
  toolActivities: readonly ToolActivity[],
): AssistantTimelineSegment[] {
  const validSummaries = reasoningSummaries.filter((summary) => summary.content.trim().length > 0);
  const hasToolOffsets = toolActivities.some(
    (activity) =>
      activity.assistantTextBeforeByteOffset !== null && activity.assistantTextBeforeByteOffset !== undefined,
  );
  const hasReasoningOffsets = validSummaries.some(
    (summary) => summary.assistantTextBeforeByteOffset !== null && summary.assistantTextBeforeByteOffset !== undefined,
  );

  if (inBandReasoning.reasoningContent.trim().length > 0 || (!hasToolOffsets && !hasReasoningOffsets)) {
    const segments: AssistantTimelineSegment[] = [];
    if (validSummaries.length > 0) {
      segments.push({
        kind: 'reasoning',
        key: `reasoning-${validSummaries[0]?.id ?? '0'}`,
        summaries: [...validSummaries],
      });
    }
    if (toolActivities.length > 0) {
      segments.push({
        kind: 'tools',
        key: `tools-${toolActivities[0]?.callId ?? '0'}`,
        activities: [...toolActivities],
      });
    }
    if (inBandReasoning.visibleContent.length > 0) {
      segments.push({
        kind: 'text',
        key: 'text-0',
        content: inBandReasoning.visibleContent,
      });
    }
    return segments;
  }

  const encoded = timelineEncoder.encode(rawContent);
  const toolByteOffsets: number[] = [];
  let minToolOffset = 0;
  for (const activity of toolActivities) {
    const rawOffset = activity.assistantTextBeforeByteOffset ?? encoded.byteLength;
    const clamped = snapUtf8Boundary(encoded, clampOffset(rawOffset, encoded.byteLength));
    const monotonic = Math.max(minToolOffset, clamped);
    minToolOffset = monotonic;
    toolByteOffsets.push(monotonic);
  }

  const orderedSummaries = validSummaries
    .map((summary, index) => ({
      summary,
      index,
      byteOffset: snapUtf8Boundary(
        encoded,
        clampOffset(summary.assistantTextBeforeByteOffset ?? 0, encoded.byteLength),
      ),
      toolIndex: clampOffset(summary.toolActivityCountBefore ?? 0, toolActivities.length),
    }))
    .sort(
      (left, right) =>
        left.toolIndex - right.toolIndex || left.byteOffset - right.byteOffset || left.index - right.index,
    );

  const segments: AssistantTimelineSegment[] = [];
  let textCursor = 0;
  let summaryCursor = 0;

  const pushText = (targetByteOffset: number) => {
    const clamped = Math.max(textCursor, Math.min(encoded.byteLength, targetByteOffset));
    if (clamped <= textCursor) return;
    const slice = timelineDecoder.decode(encoded.subarray(textCursor, clamped));
    textCursor = clamped;
    if (slice.trim().length === 0) return;
    const previous = segments.at(-1);
    if (previous?.kind === 'text') {
      previous.content += slice;
      return;
    }
    segments.push({
      kind: 'text',
      key: `text-${clamped}`,
      content: slice,
    });
  };

  const pushReasoning = (summary: ReasoningSummary) => {
    const previous = segments.at(-1);
    if (previous?.kind === 'reasoning') {
      previous.summaries.push(summary);
      return;
    }
    segments.push({
      kind: 'reasoning',
      key: `reasoning-${summary.id}`,
      summaries: [summary],
    });
  };

  const pushTool = (activity: ToolActivity) => {
    const previous = segments.at(-1);
    if (previous?.kind === 'tools') {
      previous.activities.push(activity);
      return;
    }
    segments.push({
      kind: 'tools',
      key: `tools-${activity.callId}`,
      activities: [activity],
    });
  };

  for (let toolIdx = 0; toolIdx <= toolActivities.length; toolIdx += 1) {
    while (summaryCursor < orderedSummaries.length) {
      const candidate = orderedSummaries[summaryCursor];
      if (!candidate) break;
      if (toolIdx < toolActivities.length && candidate.toolIndex > toolIdx) break;
      pushText(candidate.byteOffset);
      pushReasoning(candidate.summary);
      summaryCursor += 1;
    }
    if (toolIdx < toolActivities.length) {
      const activity = toolActivities[toolIdx];
      const toolOffset = toolByteOffsets[toolIdx];
      if (activity !== undefined && toolOffset !== undefined) {
        pushText(toolOffset);
        pushTool(activity);
      }
    }
  }

  pushText(encoded.byteLength);
  return segments;
}

function clampOffset(offset: number, max: number): number {
  return Math.max(0, Math.min(max, offset));
}

function snapUtf8Boundary(bytes: Uint8Array, offset: number): number {
  let cursor = offset;
  while (cursor > 0 && cursor < bytes.byteLength && ((bytes[cursor] ?? 0) & 0b1100_0000) === 0b1000_0000) {
    cursor -= 1;
  }
  return cursor;
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
