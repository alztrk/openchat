export interface ComparisonRequestMetric {
  conversationId: string;
  eventKind: string;
  operation: string;
  startedAtUnixMs: number;
  firstOutputLatencyMs: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  providerOutputProcessingMs: number | null;
  estimatedCostUsd: number | null;
}

export interface ModelComparisonMetrics {
  firstOutputLatencyMs: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  outputTokensPerSecond: number | null;
  estimatedCostUsd: number | null;
}

export function calculateModelComparisonMetrics(
  conversationId: string,
  requestMetrics: readonly ComparisonRequestMetric[],
): ModelComparisonMetrics {
  const requests = requestMetrics
    .filter(
      (metric) =>
        metric.conversationId === conversationId &&
        metric.eventKind === 'request' &&
        metric.operation !== 'title_generation',
    )
    .sort((left, right) => left.startedAtUnixMs - right.startedAtUnixMs);
  const firstOutputLatencyMs = requests.find((metric) => metric.firstOutputLatencyMs !== null)?.firstOutputLatencyMs ?? null;
  const inputTokens = sumCompleteValues(requests.map((metric) => metric.inputTokens));
  const outputTokens = sumCompleteValues(requests.map((metric) => metric.outputTokens));
  const estimatedCostUsd = sumCompleteValues(requests.map((metric) => metric.estimatedCostUsd));
  const outputProcessingMs = sumCompleteValues(requests.map((metric) => metric.providerOutputProcessingMs));
  const outputTokensPerSecond =
    outputProcessingMs !== null && outputProcessingMs > 0 && outputTokens !== null
      ? (outputTokens * 1_000) / outputProcessingMs
      : null;

  return { firstOutputLatencyMs, inputTokens, outputTokens, outputTokensPerSecond, estimatedCostUsd };
}

function sumCompleteValues(values: readonly (number | null)[]): number | null {
  if (values.length === 0 || values.some((value) => value === null)) return null;
  return values.reduce<number>((sum, value) => sum + (value ?? 0), 0);
}
