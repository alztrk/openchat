import { isObject } from '@shared/lib/utils/json';
import { localServiceClient } from '@shared/lib/platform/local-service';
import {
  calculateModelComparisonMetrics,
  type ComparisonRequestMetric,
  type ModelComparisonMetrics,
} from './comparisonMetricsCore';

export * from './comparisonMetricsCore';

const maximumComparisonMetricDetails = 1_000;

export async function loadModelComparisonMetrics({
  conversationId,
  providerId,
  modelId,
  fromUnixMs,
}: {
  conversationId: string;
  providerId: string;
  modelId: string;
  fromUnixMs: number;
}): Promise<ModelComparisonMetrics> {
  const response = await localServiceClient.call('statistics.details', {
    fromUnixMs,
    toUnixMs: Date.now() + 1,
    providerId,
    modelId,
    offset: 0,
    limit: maximumComparisonMetricDetails,
    includeTotal: true,
  });
  const requestDetailCount = response.requestDetailCount;
  if (
    !Array.isArray(response.requestDetails) ||
    typeof requestDetailCount !== 'number' ||
    !Number.isSafeInteger(requestDetailCount) ||
    requestDetailCount < 0
  ) {
    throw new Error('The model comparison usage response is invalid.');
  }
  if (requestDetailCount > response.requestDetails.length) {
    throw new Error('The model comparison usage response exceeds its detail limit.');
  }
  const details = response.requestDetails.map(parseComparisonRequestMetric);
  return calculateModelComparisonMetrics(
    conversationId,
    details.filter((metric) => metric.conversationId === conversationId),
  );
}

function parseComparisonRequestMetric(value: unknown): ComparisonRequestMetric {
  if (!isObject(value)) throw new Error('A model comparison usage item is invalid.');
  return {
    conversationId: requiredString(value, 'conversationId'),
    eventKind: requiredString(value, 'eventKind'),
    operation: requiredString(value, 'operation'),
    startedAtUnixMs: requiredNonNegativeInteger(value, 'startedAtUnixMs'),
    firstOutputLatencyMs: nullableNonNegativeInteger(value, 'firstOutputLatencyMs'),
    inputTokens: nullableNonNegativeInteger(value, 'inputTokens'),
    outputTokens: nullableNonNegativeInteger(value, 'outputTokens'),
    providerOutputProcessingMs: nullableNonNegativeInteger(value, 'providerOutputProcessingMs'),
    estimatedCostUsd: nullableNonNegativeNumber(value, 'estimatedCostUsd'),
  };
}

function requiredString(value: Record<string, unknown>, key: string): string {
  const result = value[key];
  if (typeof result !== 'string') throw new Error(`The model comparison usage field ${key} is invalid.`);
  return result;
}

function requiredNonNegativeInteger(value: Record<string, unknown>, key: string): number {
  const result = value[key];
  if (typeof result !== 'number' || !Number.isSafeInteger(result) || result < 0) {
    throw new Error(`The model comparison usage field ${key} is invalid.`);
  }
  return result;
}

function nullableNonNegativeInteger(value: Record<string, unknown>, key: string): number | null {
  const result = value[key];
  if (result === null) return null;
  if (typeof result !== 'number' || !Number.isSafeInteger(result) || result < 0) {
    throw new Error(`The model comparison usage field ${key} is invalid.`);
  }
  return result;
}

function nullableNonNegativeNumber(value: Record<string, unknown>, key: string): number | null {
  const result = value[key];
  if (result === null) return null;
  if (typeof result !== 'number' || !Number.isFinite(result) || result < 0) {
    throw new Error(`The model comparison usage field ${key} is invalid.`);
  }
  return result;
}
