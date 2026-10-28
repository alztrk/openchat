import type { TFunction } from 'i18next';
import type { JsonObject } from '@/platform/local-service';

export interface StatisticsSummary {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  conversationCount: number;
  estimatedCostUsd: number;
  estimatedCostRequests: number;
}

export interface StatisticsBreakdown {
  key: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  cachedInputTokens: number;
  cacheWriteTokens: number;
  cacheShareInputTokens: number;
  cacheShareCachedTokens: number;
  cacheShareRequests: number;
  estimatedCostUsd: number;
  estimatedCostRequests: number;
}

export interface StatisticsTrend {
  bucket: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  estimatedCostUsd: number;
  estimatedCostRequests: number;
}

export interface StatisticsConversation {
  conversationId: string;
  title: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  estimatedCostUsd: number;
  estimatedCostRequests: number;
  lastRequestAtUnixMs: number;
}

export interface StatisticsQuotaBucket {
  limitId: string;
  limit: number | null;
  remaining: number | null;
  usedPercent: number | null;
  windowSeconds: number | null;
  resetAtUnixMs: number | null;
}

export interface StatisticsQuotaSnapshot {
  source: 'chatgpt' | 'provider_response';
  providerId: string | null;
  modelId: string | null;
  connectionId: string | null;
  workspaceId: string | null;
  connectionName: string | null;
  workspaceName: string | null;
  fetchedAtUnixMs: number;
  freshness: 'current' | 'stale' | 'unknown';
  ordinaryUsageAllowed: boolean | null;
  buckets: StatisticsQuotaBucket[];
}

export interface LocalModelMeasurements {
  localRequests: number;
  measuredRequests: number;
  outputTokens: number;
  generationTimeMs: number;
  outputTokensPerSecond: number | null;
}

export interface StatisticsRequest {
  eventKind: string;
  startedAtUnixMs: number;
  elapsedTimeMs: number | null;
  firstOutputLatencyMs: number | null;
  providerFirstTokenLatencyMs: number | null;
  providerProcessingMs: number | null;
  providerQueueTimeMs: number | null;
  providerInputProcessingMs: number | null;
  providerOutputProcessingMs: number | null;
  providerOutputTokensPerSecond: number | null;
  conversationId: string;
  conversationTitle: string;
  connectionId: string | null;
  workspaceId: string | null;
  projectId: string | null;
  parentRunId: string | null;
  providerId: string | null;
  modelId: string | null;
  reasoningEffort: string | null;
  operation: string;
  status: string;
  completionReason: string | null;
  fastRequested: boolean | null;
  inputTokens: number | null;
  outputTokens: number | null;
  reasoningTokens: number | null;
  cachedInputTokens: number | null;
  cacheWriteTokens: number | null;
  estimatedCostUsd: number | null;
  usageSource: string;
  runId: string | null;
}

export interface StatisticsSnapshot {
  summary: StatisticsSummary;
  providers: StatisticsBreakdown[];
  models: StatisticsBreakdown[];
  dailyTrend: StatisticsTrend[];
  monthlyTrend: StatisticsTrend[];
  conversations: StatisticsConversation[];
  localModelMeasurements: LocalModelMeasurements;
  quotaHistory: StatisticsQuotaSnapshot[];
  requestDetailCount: number;
  requests: StatisticsRequest[];
  filterOptions: { providers: string[]; models: string[] };
}

export type LoadState = 'loading' | 'loaded' | 'failed';

export function statisticsQuery({
  fromDate,
  includeTotal,
  limit,
  modelId,
  offset,
  providerId,
  toDate,
}: {
  fromDate: Date;
  toDate: Date;
  providerId: string | null;
  modelId: string | null;
  offset: number;
  limit: number;
  includeTotal?: boolean;
}): JsonObject {
  const fromUnixMs = fromDate.getTime();
  const end = new Date(toDate);
  end.setDate(end.getDate() + 1);
  const toUnixMs = Math.min(end.getTime(), Date.now() + 1);
  return {
    fromUnixMs,
    toUnixMs: Math.max(fromUnixMs + 1, toUnixMs),
    providerId,
    modelId,
    offset,
    limit,
    ...(includeTotal === undefined ? {} : { includeTotal }),
  };
}

export function parseStatistics(value: JsonObject): StatisticsSnapshot {
  const summary = objectValue(value.summary, 'summary');
  return {
    summary: {
      inputTokens: requiredInteger(summary, 'inputTokens'),
      outputTokens: requiredInteger(summary, 'outputTokens'),
      totalTokens: requiredInteger(summary, 'totalTokens'),
      totalRequests: requiredInteger(summary, 'totalRequests'),
      successfulRequests: requiredInteger(summary, 'successfulRequests'),
      failedRequests: requiredInteger(summary, 'failedRequests'),
      conversationCount: requiredInteger(summary, 'conversationCount'),
      estimatedCostUsd: requiredNumber(summary, 'estimatedCostUsd'),
      estimatedCostRequests: requiredInteger(summary, 'estimatedCostRequests'),
    },
    providers: parseBreakdowns(value.providerBreakdown, 'providerId'),
    models: parseBreakdowns(value.modelBreakdown, 'modelId'),
    dailyTrend: parseTrends(value.dailyTrend, 'day'),
    monthlyTrend: parseTrends(value.monthlyTrend, 'month'),
    conversations: parseConversations(value.conversationRanking),
    localModelMeasurements: parseLocalModelMeasurements(value.localModelMeasurements),
    quotaHistory: parseQuotaHistory(value.quotaHistory),
    requestDetailCount: requiredInteger(value, 'requestDetailCount'),
    requests: parseRequests(value.requestDetails),
    filterOptions: parseFilterOptions(value.filterOptions),
  };
}

export function parseQuotaHistory(value: unknown): StatisticsQuotaSnapshot[] {
  if (!Array.isArray(value)) throw new Error('Statistics quota history is invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'quota snapshot');
    const source = item.source;
    if (source !== 'chatgpt' && source !== 'provider_response') {
      throw new Error('The statistics quota source is invalid.');
    }
    const providerId = nullableString(item.providerId);
    const modelId = nullableString(item.modelId);
    if (
      (source === 'chatgpt' && (providerId !== null || modelId !== null)) ||
      (source === 'provider_response' && providerId !== 'openai' && providerId !== 'groq')
    ) {
      throw new Error('The statistics quota provider is invalid.');
    }
    const fetchedAtUnixMs = requiredInteger(item, 'fetchedAtUnixMs');
    if (fetchedAtUnixMs < 0 || fetchedAtUnixMs > 8_640_000_000_000_000) {
      throw new Error('The statistics quota timestamp is invalid.');
    }
    const freshness = item.freshness === 'current' ? 'current' : item.freshness === 'stale' ? 'stale' : 'unknown';
    return {
      source,
      providerId,
      modelId,
      connectionId: source === 'chatgpt' ? requiredString(item, 'connectionId') : nullableString(item.connectionId),
      workspaceId: source === 'chatgpt' ? requiredString(item, 'workspaceId') : nullableString(item.workspaceId),
      connectionName: nullableString(item.connectionName),
      workspaceName: nullableString(item.workspaceName),
      fetchedAtUnixMs,
      freshness,
      ordinaryUsageAllowed: nullableBoolean(item.ordinaryUsageAllowed),
      buckets: parseQuotaBuckets(item.buckets),
    };
  });
}

export function parseLocalModelMeasurements(value: unknown): LocalModelMeasurements {
  const item = objectValue(value, 'local model measurements');
  const localRequests = requiredInteger(item, 'localRequests');
  const measuredRequests = requiredInteger(item, 'measuredRequests');
  const outputTokens = requiredInteger(item, 'outputTokens');
  const generationTimeMs = requiredInteger(item, 'generationTimeMs');
  const outputTokensPerSecond = nullableNumber(item.outputTokensPerSecond);
  if (
    localRequests < 0 ||
    measuredRequests < 0 ||
    measuredRequests > localRequests ||
    outputTokens < 0 ||
    generationTimeMs < 0 ||
    (outputTokensPerSecond !== null && outputTokensPerSecond < 0) ||
    (measuredRequests === 0 && outputTokensPerSecond !== null) ||
    (measuredRequests > 0 && (outputTokens === 0 || generationTimeMs === 0 || outputTokensPerSecond === null))
  ) {
    throw new Error('Local model measurement values are invalid.');
  }
  return { localRequests, measuredRequests, outputTokens, generationTimeMs, outputTokensPerSecond };
}

function parseQuotaBuckets(value: unknown): StatisticsQuotaBucket[] {
  if (!Array.isArray(value)) throw new Error('Statistics quota buckets are invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'quota bucket');
    const limit = nullableInteger(item.limit);
    const remaining = nullableInteger(item.remaining);
    const usedPercent = nullableNumber(item.usedPercent);
    const windowSeconds = nullableInteger(item.windowSeconds);
    const resetAtUnixMs = nullableInteger(item.resetAtUnixMs);
    if (
      (usedPercent !== null && (usedPercent < 0 || usedPercent > 100)) ||
      (limit !== null && limit < 0) ||
      (remaining !== null && remaining < 0) ||
      (windowSeconds !== null && windowSeconds < 0) ||
      (resetAtUnixMs !== null && resetAtUnixMs < 0)
    ) {
      throw new Error('A statistics quota bucket value is invalid.');
    }
    return {
      limitId: requiredString(item, 'limitId'),
      limit,
      remaining,
      usedPercent,
      windowSeconds,
      resetAtUnixMs,
    };
  });
}

export function parseStatisticsDetails(value: JsonObject): {
  requestDetailCount: number | null;
  requests: StatisticsRequest[];
} {
  const requestDetailCount = value.requestDetailCount;
  return {
    requestDetailCount:
      requestDetailCount === null || requestDetailCount === undefined
        ? null
        : requiredInteger(value, 'requestDetailCount'),
    requests: parseRequests(value.requestDetails),
  };
}

export function parseBreakdowns(value: unknown, keyName: string): StatisticsBreakdown[] {
  if (!Array.isArray(value)) throw new Error('Statistics breakdown is invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'breakdown');
    return {
      key: requiredString(item, keyName),
      requests: requiredInteger(item, 'requests'),
      inputTokens: requiredInteger(item, 'inputTokens'),
      outputTokens: requiredInteger(item, 'outputTokens'),
      estimatedCostUsd: requiredNumber(item, 'estimatedCostUsd'),
      estimatedCostRequests: requiredInteger(item, 'estimatedCostRequests'),
      reasoningTokens: requiredInteger(item, 'reasoningTokens'),
      cachedInputTokens: requiredInteger(item, 'cachedInputTokens'),
      cacheWriteTokens: requiredInteger(item, 'cacheWriteTokens'),
      cacheShareInputTokens: requiredInteger(item, 'cacheShareInputTokens'),
      cacheShareCachedTokens: requiredInteger(item, 'cacheShareCachedTokens'),
      cacheShareRequests: requiredInteger(item, 'cacheShareRequests'),
    };
  });
}

export function reportedCacheReadShare(item: StatisticsBreakdown): number | null {
  if (item.cacheShareRequests <= 0 || item.cacheShareInputTokens <= 0) return null;
  if (item.cacheShareCachedTokens < 0 || item.cacheShareCachedTokens > item.cacheShareInputTokens) return null;
  return item.cacheShareCachedTokens / item.cacheShareInputTokens;
}

export function parseTrends(value: unknown, bucketKey: string): StatisticsTrend[] {
  if (!Array.isArray(value)) throw new Error('Statistics trend is invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'trend');
    return {
      bucket: requiredString(item, bucketKey),
      requests: requiredInteger(item, 'requests'),
      inputTokens: requiredInteger(item, 'inputTokens'),
      outputTokens: requiredInteger(item, 'outputTokens'),
      estimatedCostUsd: requiredNumber(item, 'estimatedCostUsd'),
      estimatedCostRequests: requiredInteger(item, 'estimatedCostRequests'),
    };
  });
}

export function parseConversations(value: unknown): StatisticsConversation[] {
  if (!Array.isArray(value)) throw new Error('Statistics conversations are invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'conversation');
    const lastRequestAtUnixMs = requiredInteger(item, 'lastRequestAtUnixMs');
    if (lastRequestAtUnixMs < 0 || lastRequestAtUnixMs > 8_640_000_000_000_000) {
      throw new Error('Statistics conversation timestamp is invalid.');
    }
    return {
      conversationId: requiredString(item, 'conversationId'),
      title: requiredString(item, 'title'),
      requests: requiredInteger(item, 'requests'),
      inputTokens: requiredInteger(item, 'inputTokens'),
      outputTokens: requiredInteger(item, 'outputTokens'),
      estimatedCostUsd: requiredNumber(item, 'estimatedCostUsd'),
      estimatedCostRequests: requiredInteger(item, 'estimatedCostRequests'),
      lastRequestAtUnixMs,
    };
  });
}

export function parseRequests(value: unknown): StatisticsRequest[] {
  if (!Array.isArray(value)) throw new Error('Statistics request details are invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'request');
    const startedAtUnixMs = requiredInteger(item, 'startedAtUnixMs');
    const finishedAtUnixMs = nullableInteger(item.finishedAtUnixMs);
    const firstOutputLatencyMs = nullableInteger(item.firstOutputLatencyMs);
    const providerProcessingMs = nullableInteger(item.providerProcessingMs);
    const providerQueueTimeMs = nullableInteger(item.providerQueueTimeMs);
    const providerInputProcessingMs = nullableInteger(item.providerInputProcessingMs);
    const providerOutputProcessingMs = nullableInteger(item.providerOutputProcessingMs);
    const outputTokens = nullableInteger(item.outputTokens);
    const providerFirstTokenLatencyTotal =
      providerQueueTimeMs !== null && providerInputProcessingMs !== null
        ? providerQueueTimeMs + providerInputProcessingMs
        : null;
    if (finishedAtUnixMs !== null && finishedAtUnixMs < startedAtUnixMs) {
      throw new Error('The statistics request finish time is invalid.');
    }
    if (firstOutputLatencyMs !== null && firstOutputLatencyMs < 0) {
      throw new Error('The first output latency is invalid.');
    }
    if (
      [providerProcessingMs, providerQueueTimeMs, providerInputProcessingMs, providerOutputProcessingMs].some(
        (value) => value !== null && value < 0,
      )
    ) {
      throw new Error('A provider timing value is invalid.');
    }
    return {
      eventKind: requiredString(item, 'eventKind'),
      startedAtUnixMs,
      elapsedTimeMs: finishedAtUnixMs === null ? null : finishedAtUnixMs - startedAtUnixMs,
      firstOutputLatencyMs,
      providerFirstTokenLatencyMs:
        providerFirstTokenLatencyTotal !== null && Number.isSafeInteger(providerFirstTokenLatencyTotal)
          ? providerFirstTokenLatencyTotal
          : null,
      providerProcessingMs,
      providerQueueTimeMs,
      providerInputProcessingMs,
      providerOutputProcessingMs,
      providerOutputTokensPerSecond:
        outputTokens !== null &&
        outputTokens >= 0 &&
        providerOutputProcessingMs !== null &&
        providerOutputProcessingMs > 0
          ? (outputTokens * 1_000) / providerOutputProcessingMs
          : null,
      conversationId: requiredString(item, 'conversationId'),
      conversationTitle: requiredString(item, 'conversationTitle'),
      connectionId: nullableString(item.connectionId),
      workspaceId: nullableString(item.workspaceId),
      projectId: nullableString(item.projectId),
      parentRunId: nullableString(item.parentRunId),
      providerId: nullableString(item.providerId),
      modelId: nullableString(item.modelId),
      reasoningEffort: nullableString(item.reasoningEffort),
      operation: requiredString(item, 'operation'),
      status: requiredString(item, 'status'),
      completionReason: nullableString(item.completionReason),
      fastRequested: nullableBoolean(item.fastRequested),
      inputTokens: nullableInteger(item.inputTokens),
      outputTokens,
      reasoningTokens: nullableInteger(item.reasoningTokens),
      cachedInputTokens: nullableInteger(item.cachedInputTokens),
      cacheWriteTokens: nullableInteger(item.cacheWriteTokens),
      estimatedCostUsd: nullableNumber(item.estimatedCostUsd),
      usageSource: requiredString(item, 'usageSource'),
      runId: nullableString(item.runId),
    };
  });
}

export function parseFilterOptions(value: unknown): { providers: string[]; models: string[] } {
  const options = objectValue(value, 'filter options');
  if (
    !Array.isArray(options.providers) ||
    !options.providers.every(isString) ||
    !Array.isArray(options.models) ||
    !options.models.every(isString)
  ) {
    throw new Error('Statistics filters are invalid.');
  }
  return { providers: options.providers, models: options.models };
}

export function createStatisticsCsv(requests: readonly StatisticsRequest[], t: TFunction, language: string): string {
  const rows = [
    [
      t('statisticsRequestTime'),
      t('statisticsOpenChatElapsedTime'),
      t('statisticsFirstOutputLatency'),
      t('statisticsProviderFirstTokenLatency'),
      t('statisticsProviderProcessingTime'),
      t('statisticsProviderQueueTime'),
      t('statisticsProviderInputProcessingTime'),
      t('statisticsProviderOutputProcessingTime'),
      t('statisticsProviderOutputRate'),
      t('statisticsConversationId'),
      t('statisticsConversationTitle'),
      t('statisticsConnectionId'),
      t('statisticsWorkspaceId'),
      t('statisticsProjectId'),
      t('statisticsParentRunId'),
      t('statisticsProvider'),
      t('statisticsModel'),
      t('statisticsOperation'),
      t('statisticsReasoningEffort'),
      t('statisticsFastModeFilter'),
      t('statisticsStatus'),
      t('statisticsProviderCompletionReason'),
      t('statisticsInputTokens'),
      t('statisticsOutputTokens'),
      t('statisticsReasoningTokens'),
      t('statisticsCachedInputTokens'),
      t('statisticsCacheWriteTokens'),
      t('statisticsEstimatedCost'),
      t('statisticsUsageSource'),
      t('statisticsRunId'),
    ],
  ];
  for (const request of requests) {
    rows.push([
      new Intl.DateTimeFormat(language, { dateStyle: 'short', timeStyle: 'medium' }).format(request.startedAtUnixMs),
      request.elapsedTimeMs === null ? '' : String(request.elapsedTimeMs),
      request.firstOutputLatencyMs === null ? '' : String(request.firstOutputLatencyMs),
      request.providerFirstTokenLatencyMs === null ? '' : String(request.providerFirstTokenLatencyMs),
      request.providerProcessingMs === null ? '' : String(request.providerProcessingMs),
      request.providerQueueTimeMs === null ? '' : String(request.providerQueueTimeMs),
      request.providerInputProcessingMs === null ? '' : String(request.providerInputProcessingMs),
      request.providerOutputProcessingMs === null ? '' : String(request.providerOutputProcessingMs),
      request.providerOutputTokensPerSecond === null ? '' : request.providerOutputTokensPerSecond.toFixed(2),
      request.conversationId,
      request.conversationTitle,
      request.connectionId ?? '',
      request.workspaceId ?? '',
      request.projectId ?? '',
      request.parentRunId ?? '',
      request.providerId === null ? t('statisticsNotReported') : providerLabel(request.providerId, t),
      request.modelId ?? t('statisticsNotReported'),
      operationLabel(request.operation, t),
      request.reasoningEffort === null
        ? t('statisticsNotReported')
        : request.reasoningEffort === 'unspecified'
          ? t('statisticsUnspecified')
          : request.reasoningEffort,
      request.fastRequested === null
        ? t('statisticsNotReported')
        : request.fastRequested
          ? t('statisticsRequested')
          : t('statisticsNotRequested'),
      statusLabel(request, t),
      request.completionReason ?? '',
      request.inputTokens === null ? '' : String(request.inputTokens),
      request.outputTokens === null ? '' : String(request.outputTokens),
      request.reasoningTokens === null ? '' : String(request.reasoningTokens),
      request.cachedInputTokens === null ? '' : String(request.cachedInputTokens),
      request.cacheWriteTokens === null ? '' : String(request.cacheWriteTokens),
      request.estimatedCostUsd === null ? '' : request.estimatedCostUsd.toFixed(9),
      request.usageSource,
      request.runId ?? '',
    ]);
  }
  return `\uFEFF${rows.map((row) => row.map(csvField).join(',')).join('\r\n')}`;
}

export function csvField(value: string): string {
  return `"${value.replaceAll('"', '""')}"`;
}

export function providerLabel(id: string, t: TFunction): string {
  if (id === 'chatgpt') return t('statisticsChatGptOAuth');
  if (id === 'chatgpt_api') return t('statisticsChatGptApi');
  return id;
}

export function operationLabel(operation: string, t: TFunction): string {
  if (operation === 'chat') return t('statisticsOperationChat');
  if (operation === 'tool_follow_up') return t('statisticsOperationToolFollowUp');
  if (operation === 'compaction') return t('statisticsOperationCompaction');
  if (operation === 'title_generation') return t('statisticsOperationTitleGeneration');
  if (operation === 'legacy') return t('statisticsOperationLegacy');
  return operation;
}

export function statusLabel(request: StatisticsRequest, t: TFunction): string {
  if (request.eventKind === 'legacy_output') return t('statisticsStatusLegacy');
  if (request.status === 'completed') return t('statisticsStatusCompleted');
  if (request.status === 'failed') return t('statisticsStatusFailed');
  if (request.status === 'cancelled') return t('statisticsStatusCancelled');
  if (request.status === 'interrupted') return t('statisticsStatusInterrupted');
  if (request.status === 'pending') return t('statisticsStatusPending');
  return t('statisticsNotReported');
}

export function formatNumber(value: number, language: string): string {
  return new Intl.NumberFormat(language).format(value);
}

export function formatCost(value: number, language: string): string {
  return new Intl.NumberFormat(language, { style: 'currency', currency: 'USD', maximumFractionDigits: 4 }).format(
    value,
  );
}

export function formatTrendDate(value: string, language: string, monthly: boolean): string {
  const match = /^(\d{4})-(\d{2})(?:-(\d{2}))?$/u.exec(value);
  if (!match) return value;
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3] ?? 1));
  if (
    date.getFullYear() !== Number(match[1]) ||
    date.getMonth() !== Number(match[2]) - 1 ||
    (match[3] !== undefined && date.getDate() !== Number(match[3]))
  )
    return value;
  return new Intl.DateTimeFormat(language, monthly ? { month: 'short' } : { month: 'numeric', day: 'numeric' }).format(
    date,
  );
}

export function dateInputValue(date: Date): string {
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 10);
}

export function parseDateInput(value: string): Date | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const [yearPart, monthPart, dayPart] = value.split('-');
  if (yearPart === undefined || monthPart === undefined || dayPart === undefined) return null;
  const year = Number(yearPart);
  const month = Number(monthPart);
  const day = Number(dayPart);
  const date = new Date(year, month - 1, day);
  return date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day ? date : null;
}

export function objectValue(value: unknown, field: string): Record<string, unknown> {
  if (isRecord(value)) return value;
  throw new Error(`The statistics ${field} value is invalid.`);
}

export function requiredString(value: Record<string, unknown>, key: string): string {
  const result = value[key];
  if (typeof result !== 'string' || result.length === 0) throw new Error(`The statistics field ${key} is invalid.`);
  return result;
}

export function requiredInteger(value: Record<string, unknown>, key: string): number {
  const result = value[key];
  if (typeof result !== 'number' || !Number.isSafeInteger(result))
    throw new Error(`The statistics field ${key} is invalid.`);
  return result;
}

export function requiredNumber(value: Record<string, unknown>, key: string): number {
  const result = value[key];
  if (typeof result !== 'number' || !Number.isFinite(result))
    throw new Error(`The statistics field ${key} is invalid.`);
  return result;
}

export function nullableInteger(value: unknown): number | null {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isSafeInteger(value))
    throw new Error('A statistics token count is invalid.');
  return value;
}

export function nullableBoolean(value: unknown): boolean | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'boolean') throw new Error('A statistics boolean value is invalid.');
  return value;
}

export function nullableNumber(value: unknown): number | null {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new Error('A statistics cost value is invalid.');
  return value;
}

export function nullableString(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string') throw new Error('A statistics value is invalid.');
  return value;
}

export function isString(value: unknown): value is string {
  return typeof value === 'string';
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
