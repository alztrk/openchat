import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const sourcePath = path.resolve('src/modules/settings/components/statisticsData.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { createStatisticsCsv, parseLocalModelMeasurements, parseQuotaHistory, parseRequests, reportedCacheReadShare } =
  await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

const t = (key) => key;

test('reported cache-read share requires provider-reported input and cache coverage', () => {
  const base = {
    key: 'provider',
    requests: 4,
    inputTokens: 600,
    outputTokens: 0,
    reasoningTokens: 0,
    cachedInputTokens: 100,
    cacheWriteTokens: 0,
    estimatedCostUsd: 0,
    estimatedCostRequests: 0,
    cacheShareInputTokens: 500,
    cacheShareCachedTokens: 100,
    cacheShareRequests: 2,
  };

  assert.equal(reportedCacheReadShare(base), 0.2);
  assert.equal(reportedCacheReadShare({ ...base, cacheShareRequests: 0 }), null);
  assert.equal(reportedCacheReadShare({ ...base, cacheShareRequests: -1 }), null);
  assert.equal(reportedCacheReadShare({ ...base, cacheShareInputTokens: 0 }), null);
  assert.equal(reportedCacheReadShare({ ...base, cacheShareCachedTokens: 501 }), null);
});

test('local model measurements retain only the service aggregate and validate its coverage', () => {
  assert.deepEqual(
    parseLocalModelMeasurements({
      localRequests: 8,
      measuredRequests: 3,
      outputTokens: 120,
      generationTimeMs: 6_000,
      outputTokensPerSecond: 20,
    }),
    {
      localRequests: 8,
      measuredRequests: 3,
      outputTokens: 120,
      generationTimeMs: 6_000,
      outputTokensPerSecond: 20,
    },
  );
  assert.throws(
    () =>
      parseLocalModelMeasurements({
        localRequests: 1,
        measuredRequests: 2,
        outputTokens: 1,
        generationTimeMs: 1,
        outputTokensPerSecond: 1_000,
      }),
    /invalid/u,
  );
  assert.throws(
    () =>
      parseLocalModelMeasurements({
        localRequests: 1,
        measuredRequests: 1,
        outputTokens: 1,
        generationTimeMs: 1,
        outputTokensPerSecond: null,
      }),
    /invalid/u,
  );
});

test('quota history keeps freshness and unavailable provider fields explicit', () => {
  const [snapshot] = parseQuotaHistory([
    {
      source: 'chatgpt',
      providerId: null,
      modelId: null,
      connectionId: 'connection-1',
      workspaceId: 'workspace-1',
      connectionName: null,
      workspaceName: 'Workspace',
      fetchedAtUnixMs: 1_700_000_000_000,
      freshness: 'stale',
      ordinaryUsageAllowed: null,
      buckets: [
        {
          limitId: 'primary',
          limit: null,
          remaining: null,
          usedPercent: 75.5,
          windowSeconds: 3600,
          resetAtUnixMs: null,
        },
      ],
    },
    {
      source: 'chatgpt',
      providerId: null,
      modelId: null,
      connectionId: 'connection-2',
      workspaceId: 'workspace-2',
      connectionName: 'Account',
      workspaceName: null,
      fetchedAtUnixMs: 1_700_000_000_001,
      freshness: 'future-state',
      ordinaryUsageAllowed: false,
      buckets: [],
    },
  ]);

  assert.equal(snapshot.freshness, 'stale');
  assert.equal(snapshot.ordinaryUsageAllowed, null);
  assert.equal(snapshot.buckets[0].usedPercent, 75.5);
  assert.equal(snapshot.buckets[0].resetAtUnixMs, null);
  assert.equal(parseQuotaHistory([{ ...snapshot, freshness: 'future-state' }])[0].freshness, 'unknown');
  assert.throws(
    () => parseQuotaHistory([{ ...snapshot, buckets: [{ ...snapshot.buckets[0], usedPercent: 125 }] }]),
    /invalid/u,
  );
});

test('provider response limits preserve reported counts and reset times', () => {
  const [snapshot] = parseQuotaHistory([
    {
      source: 'provider_response',
      providerId: 'openai',
      modelId: 'gpt-model',
      connectionId: null,
      workspaceId: null,
      connectionName: null,
      workspaceName: null,
      fetchedAtUnixMs: 1_700_000_000_000,
      freshness: 'stale',
      ordinaryUsageAllowed: null,
      buckets: [
        {
          limitId: 'requests',
          limit: 60,
          remaining: 58,
          usedPercent: (100 / 60) * 2,
          windowSeconds: null,
          resetAtUnixMs: 1_700_000_001_000,
        },
      ],
    },
  ]);

  assert.equal(snapshot.source, 'provider_response');
  assert.equal(snapshot.providerId, 'openai');
  assert.equal(snapshot.modelId, 'gpt-model');
  assert.equal(snapshot.buckets[0].limit, 60);
  assert.equal(snapshot.buckets[0].remaining, 58);
  assert.equal(snapshot.buckets[0].resetAtUnixMs, 1_700_000_001_000);
  assert.throws(() => parseQuotaHistory([{ ...snapshot, providerId: 'custom_openai' }]), /invalid/u);
});

test('request statistics preserve nullable first-output latency in CSV export', () => {
  const [request] = parseRequests([
    {
      eventKind: 'request',
      startedAtUnixMs: 10,
      finishedAtUnixMs: 250,
      firstOutputLatencyMs: 80,
      providerProcessingMs: 190,
      providerQueueTimeMs: 10,
      providerInputProcessingMs: 20,
      providerOutputProcessingMs: 200,
      conversationId: 'conversation-1',
      conversationTitle: 'Conversation',
      connectionId: 'connection-1',
      workspaceId: 'workspace-1',
      projectId: 'project-1',
      parentRunId: 'parent-run-1',
      providerId: 'openai',
      modelId: 'gpt',
      reasoningEffort: null,
      operation: 'chat',
      status: 'completed',
      completionReason: 'tool_calls',
      fastRequested: false,
      inputTokens: 20,
      outputTokens: 4,
      reasoningTokens: null,
      cachedInputTokens: null,
      cacheWriteTokens: null,
      estimatedCostUsd: null,
      usageSource: 'provider_reported',
      runId: null,
    },
  ]);

  assert.equal(request.firstOutputLatencyMs, 80);
  assert.equal(request.providerFirstTokenLatencyMs, 30);
  assert.equal(request.providerProcessingMs, 190);
  assert.equal(request.providerQueueTimeMs, 10);
  assert.equal(request.providerInputProcessingMs, 20);
  assert.equal(request.providerOutputProcessingMs, 200);
  assert.equal(request.providerOutputTokensPerSecond, 20);
  assert.equal(request.completionReason, 'tool_calls');
  assert.equal(request.connectionId, 'connection-1');
  assert.equal(request.workspaceId, 'workspace-1');
  assert.equal(request.projectId, 'project-1');
  assert.equal(request.parentRunId, 'parent-run-1');
  const csv = createStatisticsCsv([request], t, 'en-US');
  const lines = csv.trim().split('\r\n');
  assert.ok(lines[0].includes('statisticsFirstOutputLatency'));
  assert.ok(lines[0].includes('statisticsProviderFirstTokenLatency'));
  assert.ok(lines[0].includes('statisticsProviderProcessingTime'));
  assert.ok(lines[0].includes('statisticsProviderQueueTime'));
  assert.ok(lines[0].includes('statisticsProviderInputProcessingTime'));
  assert.ok(lines[0].includes('statisticsProviderOutputProcessingTime'));
  assert.ok(lines[0].includes('statisticsProviderOutputRate'));
  assert.ok(lines[0].includes('statisticsProviderCompletionReason'));
  assert.ok(lines[0].includes('statisticsConnectionId'));
  assert.ok(lines[1].includes('"80"'));
  assert.ok(lines[1].includes('"30"'));
  assert.ok(lines[1].includes('"190"'));
  assert.ok(lines[1].includes('"10"'));
  assert.ok(lines[1].includes('"20"'));
  assert.ok(lines[1].includes('"200"'));
  assert.ok(lines[1].includes('"20.00"'));
  assert.ok(lines[1].includes('"connection-1"'));
  assert.ok(lines[1].includes('"workspace-1"'));
  assert.ok(lines[1].includes('"project-1"'));
  assert.ok(lines[1].includes('"parent-run-1"'));
  assert.ok(lines[1].includes('"tool_calls"'));
});

test('request statistics accept unavailable first-output latency without inventing a value', () => {
  const [request] = parseRequests([
    {
      eventKind: 'request',
      startedAtUnixMs: 10,
      finishedAtUnixMs: 10,
      firstOutputLatencyMs: null,
      providerProcessingMs: null,
      providerQueueTimeMs: null,
      providerInputProcessingMs: null,
      providerOutputProcessingMs: null,
      conversationId: 'conversation-1',
      conversationTitle: 'Conversation',
      connectionId: null,
      workspaceId: null,
      projectId: null,
      parentRunId: null,
      providerId: 'openai',
      modelId: 'gpt',
      reasoningEffort: null,
      operation: 'chat',
      status: 'interrupted',
      completionReason: null,
      fastRequested: false,
      inputTokens: null,
      outputTokens: null,
      reasoningTokens: null,
      cachedInputTokens: null,
      cacheWriteTokens: null,
      estimatedCostUsd: null,
      usageSource: 'unavailable',
      runId: null,
    },
  ]);

  assert.equal(request.firstOutputLatencyMs, null);
  assert.equal(request.providerFirstTokenLatencyMs, null);
  assert.equal(request.providerProcessingMs, null);
  assert.equal(request.providerQueueTimeMs, null);
  assert.equal(request.providerInputProcessingMs, null);
  assert.equal(request.providerOutputProcessingMs, null);
  assert.equal(request.providerOutputTokensPerSecond, null);
  assert.equal(request.completionReason, null);
});
