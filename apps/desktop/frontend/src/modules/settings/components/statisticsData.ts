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
  providerReportedCostUsd: number;
  modelsDevCatalogCostUsd: number;
}

export interface StatisticsBreakdown {
  key: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  cachedInputTokens: number;
  cacheWriteTokens: number;
  providerReportedCostUsd: number;
  costReportedRequests: number;
  modelsDevCatalogCostUsd: number;
  modelsDevCatalogCostRequests: number;
}

export interface StatisticsTrend {
  bucket: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
}

export interface StatisticsConversation {
  conversationId: string;
  title: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  modelsDevCatalogCostUsd: number;
  modelsDevCatalogCostRequests: number;
  lastRequestAtUnixMs: number;
}

export interface StatisticsRequest {
  eventKind: string;
  startedAtUnixMs: number;
  conversationTitle: string;
  providerId: string | null;
  modelId: string | null;
  reasoningEffort: string | null;
  operation: string;
  status: string;
  fastRequested: boolean | null;
  inputTokens: number | null;
  outputTokens: number | null;
  reasoningTokens: number | null;
  cachedInputTokens: number | null;
  cacheWriteTokens: number | null;
  providerReportedCostUsd: number | null;
  modelsDevCatalogCostUsd: number | null;
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
  requestDetailCount: number;
  requests: StatisticsRequest[];
  filterOptions: { providers: string[]; models: string[] };
}

export type LoadState = 'loading' | 'loaded' | 'failed';

export function statisticsQuery({
  fromDate,
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
      providerReportedCostUsd: requiredNumber(summary, 'providerReportedCostUsd'),
      modelsDevCatalogCostUsd: requiredNumber(summary, 'modelsDevCatalogCostUsd'),
    },
    providers: parseBreakdowns(value.providerBreakdown, 'providerId'),
    models: parseBreakdowns(value.modelBreakdown, 'modelId'),
    dailyTrend: parseTrends(value.dailyTrend, 'day'),
    monthlyTrend: parseTrends(value.monthlyTrend, 'month'),
    conversations: parseConversations(value.conversationRanking),
    requestDetailCount: requiredInteger(value, 'requestDetailCount'),
    requests: parseRequests(value.requestDetails),
    filterOptions: parseFilterOptions(value.filterOptions),
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
      reasoningTokens: requiredInteger(item, 'reasoningTokens'),
      cachedInputTokens: requiredInteger(item, 'cachedInputTokens'),
      cacheWriteTokens: requiredInteger(item, 'cacheWriteTokens'),
      providerReportedCostUsd: requiredNumber(item, 'providerReportedCostUsd'),
      costReportedRequests: requiredInteger(item, 'costReportedRequests'),
      modelsDevCatalogCostUsd: requiredNumber(item, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: requiredInteger(item, 'modelsDevCatalogCostRequests'),
    };
  });
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
      modelsDevCatalogCostUsd: requiredNumber(item, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: requiredInteger(item, 'modelsDevCatalogCostRequests'),
      lastRequestAtUnixMs,
    };
  });
}

export function parseRequests(value: unknown): StatisticsRequest[] {
  if (!Array.isArray(value)) throw new Error('Statistics request details are invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'request');
    return {
      eventKind: requiredString(item, 'eventKind'),
      startedAtUnixMs: requiredInteger(item, 'startedAtUnixMs'),
      conversationTitle: requiredString(item, 'conversationTitle'),
      providerId: nullableString(item.providerId),
      modelId: nullableString(item.modelId),
      reasoningEffort: nullableString(item.reasoningEffort),
      operation: requiredString(item, 'operation'),
      status: requiredString(item, 'status'),
      fastRequested: nullableBoolean(item.fastRequested),
      inputTokens: nullableInteger(item.inputTokens),
      outputTokens: nullableInteger(item.outputTokens),
      reasoningTokens: nullableInteger(item.reasoningTokens),
      cachedInputTokens: nullableInteger(item.cachedInputTokens),
      cacheWriteTokens: nullableInteger(item.cacheWriteTokens),
      providerReportedCostUsd: nullableNumber(item.providerReportedCostUsd),
      modelsDevCatalogCostUsd: nullableNumber(item.modelsDevCatalogCostUsd),
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
      t('statisticsConversationTitle'),
      t('statisticsProvider'),
      t('statisticsModel'),
      t('statisticsOperation'),
      t('statisticsReasoningEffort'),
      t('statisticsFastModeFilter'),
      t('statisticsStatus'),
      t('statisticsInputTokens'),
      t('statisticsOutputTokens'),
      t('statisticsReasoningTokens'),
      t('statisticsCachedInputTokens'),
      t('statisticsCacheWriteTokens'),
      t('statisticsProviderReportedCost'),
      t('statisticsModelsDevCatalogCost'),
      t('statisticsUsageSource'),
      t('statisticsRunId'),
    ],
  ];
  for (const request of requests) {
    rows.push([
      new Intl.DateTimeFormat(language, { dateStyle: 'short', timeStyle: 'medium' }).format(request.startedAtUnixMs),
      request.conversationTitle,
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
      request.inputTokens === null ? '' : String(request.inputTokens),
      request.outputTokens === null ? '' : String(request.outputTokens),
      request.reasoningTokens === null ? '' : String(request.reasoningTokens),
      request.cachedInputTokens === null ? '' : String(request.cachedInputTokens),
      request.cacheWriteTokens === null ? '' : String(request.cacheWriteTokens),
      request.providerReportedCostUsd === null ? '' : request.providerReportedCostUsd.toFixed(9),
      request.modelsDevCatalogCostUsd === null ? '' : request.modelsDevCatalogCostUsd.toFixed(9),
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
