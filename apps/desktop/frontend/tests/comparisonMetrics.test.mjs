import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(path.resolve('src/features/chat/data/comparisonMetricsCore.ts'), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { calculateModelComparisonMetrics } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('comparison metrics aggregate only request records for the selected conversation', () => {
  const metrics = calculateModelComparisonMetrics('comparison-a', [
    {
      conversationId: 'comparison-a',
      eventKind: 'request',
      operation: 'chat',
      startedAtUnixMs: 200,
      firstOutputLatencyMs: 700,
      inputTokens: 40,
      outputTokens: 20,
      providerOutputProcessingMs: 1_000,
      estimatedCostUsd: 0.002,
    },
    {
      conversationId: 'comparison-a',
      eventKind: 'request',
      operation: 'tool_follow_up',
      startedAtUnixMs: 100,
      firstOutputLatencyMs: 350,
      inputTokens: 60,
      outputTokens: 30,
      providerOutputProcessingMs: 2_000,
      estimatedCostUsd: 0.003,
    },
    {
      conversationId: 'comparison-a',
      eventKind: 'legacy_output',
      operation: 'chat',
      startedAtUnixMs: 50,
      firstOutputLatencyMs: null,
      inputTokens: 100,
      outputTokens: 100,
      providerOutputProcessingMs: 1,
      estimatedCostUsd: 100,
    },
    {
      conversationId: 'comparison-b',
      eventKind: 'request',
      operation: 'chat',
      startedAtUnixMs: 75,
      firstOutputLatencyMs: 10,
      inputTokens: 100,
      outputTokens: 100,
      providerOutputProcessingMs: 1,
      estimatedCostUsd: 100,
    },
  ]);

  assert.deepEqual(metrics, {
    firstOutputLatencyMs: 350,
    inputTokens: 100,
    outputTokens: 50,
    outputTokensPerSecond: 50 / 3,
    estimatedCostUsd: 0.005,
  });
});

test('comparison metrics leave unknown totals unavailable instead of inventing values', () => {
  const metrics = calculateModelComparisonMetrics('comparison-a', [
    {
      conversationId: 'comparison-a',
      eventKind: 'request',
      operation: 'chat',
      startedAtUnixMs: 100,
      firstOutputLatencyMs: null,
      inputTokens: null,
      outputTokens: 12,
      providerOutputProcessingMs: null,
      estimatedCostUsd: null,
    },
  ]);

  assert.deepEqual(metrics, {
    firstOutputLatencyMs: null,
    inputTokens: null,
    outputTokens: 12,
    outputTokensPerSecond: null,
    estimatedCostUsd: null,
  });
});
