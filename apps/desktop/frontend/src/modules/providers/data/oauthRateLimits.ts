import { useSyncExternalStore } from 'react';
import type { ChatModel } from './chatModels';
import { modelRouteKey } from './modelPreferences';
import { loadCopilotUsage } from './copilotOAuth';
import { localServiceClient, LocalServiceError } from '@/platform/local-service';

export type OAuthRateLimitReason = 'quotaExceeded' | 'temporarilyRateLimited';

export interface OAuthRateLimitState {
  reason: OAuthRateLimitReason;
  resetAtUnixMs: number | null;
}

const states = new Map<string, OAuthRateLimitState>();
const listeners = new Set<() => void>();

export function useOAuthRateLimits(): ReadonlyMap<string, OAuthRateLimitState> {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export function rateLimitForModel(
  model: ChatModel,
  rateLimits: ReadonlyMap<string, OAuthRateLimitState>,
): OAuthRateLimitState | null {
  const state = rateLimits.get(modelRouteKey(model));
  if (state?.resetAtUnixMs !== null && state?.resetAtUnixMs !== undefined && state.resetAtUnixMs <= Date.now()) {
    clearOAuthRateLimit(model);
    return null;
  }
  return state ?? null;
}

export async function recordOAuthRateLimit(model: ChatModel, models: readonly ChatModel[], error: unknown): Promise<void> {
  if (model.providerId !== 'chatgpt' && model.providerId !== 'copilot') return;
  const retryAt = retryAtFromError(error);
  setModelsRateLimit(
    matchingOAuthModels(model, models),
    { reason: 'temporarilyRateLimited', resetAtUnixMs: retryAt },
  );

  try {
    if (model.providerId === 'chatgpt') {
      const response = await localServiceClient.call('chatgpt.usage.get', {
        connectionId: model.connectionId,
        workspaceId: model.workspaceId,
      });
      const snapshot = parseChatGptUsage(response);
      if (snapshot.exhausted) {
        setModelsRateLimit(matchingOAuthModels(model, models), {
          reason: 'quotaExceeded',
          resetAtUnixMs: snapshot.resetAtUnixMs,
        });
      }
    } else {
      const snapshot = await loadCopilotUsage();
      const depleted = snapshot.quotas.filter(
        (quota) =>
          (quota.kind === 'chat' || quota.kind === 'premiumInteractions') &&
          quota.remaining === 0 &&
          quota.unlimited !== true,
      );
      if (depleted.length > 0) {
        setModelsRateLimit(matchingOAuthModels(model, models), {
          reason: 'quotaExceeded',
          resetAtUnixMs: latestReset(depleted.map((quota) => quota.resetAtUnixMs)),
        });
      }
    }
  } catch (cause) {
    globalThis.reportError(new Error('OAuth rate limit status could not be refreshed.', { cause }));
  }
}

export async function refreshOAuthRateLimits(models: readonly ChatModel[]): Promise<void> {
  const routes = uniqueChatGptRoutes(models);
  await Promise.all([
    ...routes.map(async (route) => {
      try {
        const response = await localServiceClient.call('chatgpt.usage.get', route);
        const snapshot = parseChatGptUsage(response);
        const scopedModels = models.filter(
          (model) =>
            model.providerId === 'chatgpt' &&
            model.connectionId === route.connectionId &&
            model.workspaceId === route.workspaceId,
        );
        updateQuotaState(scopedModels, snapshot.exhausted, snapshot.resetAtUnixMs);
      } catch (cause) {
        globalThis.reportError(new Error('ChatGPT OAuth quota status could not be loaded.', { cause }));
      }
    }),
    ...(models.some((model) => model.providerId === 'copilot')
      ? [
          loadCopilotUsage()
            .then((snapshot) => {
              const depleted = snapshot.quotas.filter(
                (quota) =>
                  (quota.kind === 'chat' || quota.kind === 'premiumInteractions') &&
                  quota.remaining === 0 &&
                  quota.unlimited !== true,
              );
              updateQuotaState(
                models.filter((model) => model.providerId === 'copilot'),
                depleted.length > 0,
                latestReset(depleted.map((quota) => quota.resetAtUnixMs)),
              );
            })
            .catch((cause: unknown) => {
              globalThis.reportError(new Error('Copilot OAuth quota status could not be loaded.', { cause }));
            }),
        ]
      : []),
  ]);
}

export function clearOAuthRateLimit(model: ChatModel): void {
  if (states.delete(modelRouteKey(model))) emitChange();
}

function updateQuotaState(models: readonly ChatModel[], exhausted: boolean, resetAtUnixMs: number | null): void {
  let changed = false;
  for (const model of models) {
    const key = modelRouteKey(model);
    if (exhausted) {
      const current = states.get(key);
      if (current?.reason !== 'quotaExceeded' || current.resetAtUnixMs !== resetAtUnixMs) {
        states.set(key, { reason: 'quotaExceeded', resetAtUnixMs });
        changed = true;
      }
    } else if (states.get(key)?.reason === 'quotaExceeded') {
      states.delete(key);
      changed = true;
    }
  }
  if (changed) emitChange();
}

function matchingOAuthModels(model: ChatModel, models: readonly ChatModel[]): ChatModel[] {
  if (model.providerId === 'chatgpt') {
    return models.filter(
      (candidate) =>
        candidate.providerId === 'chatgpt' &&
        candidate.connectionId === model.connectionId &&
        candidate.workspaceId === model.workspaceId,
    );
  }
  return models.filter(
    (candidate) =>
      candidate.providerId === 'copilot' &&
      candidate.connectionId === model.connectionId &&
      candidate.modelId === model.modelId,
  );
}

function uniqueChatGptRoutes(models: readonly ChatModel[]): Array<{ connectionId: string; workspaceId: string }> {
  const routes = new Map<string, { connectionId: string; workspaceId: string }>();
  for (const model of models) {
    if (model.providerId !== 'chatgpt' || !model.connectionId || !model.workspaceId) continue;
    routes.set(`${model.connectionId}:${model.workspaceId}`, {
      connectionId: model.connectionId,
      workspaceId: model.workspaceId,
    });
  }
  return [...routes.values()];
}

function parseChatGptUsage(value: Record<string, unknown>): {
  exhausted: boolean;
  resetAtUnixMs: number | null;
} {
  const buckets = Array.isArray(value.buckets) ? value.buckets : [];
  const exhaustedBuckets = buckets.filter(
    (bucket): bucket is Record<string, unknown> =>
      typeof bucket === 'object' &&
      bucket !== null &&
      typeof bucket.usedPercent === 'number' &&
      bucket.usedPercent >= 100,
  );
  const exhausted = value.ordinaryUsageAllowed === false || exhaustedBuckets.length > 0;
  return {
    exhausted,
    resetAtUnixMs: latestReset(
      exhaustedBuckets.map((bucket) =>
        typeof bucket.resetAtUnixMs === 'number' && Number.isSafeInteger(bucket.resetAtUnixMs)
          ? bucket.resetAtUnixMs
          : null,
      ),
    ),
  };
}

function latestReset(values: readonly (number | null)[]): number | null {
  const timestamps = values.filter((value): value is number => value !== null && value > 0);
  return timestamps.length > 0 ? Math.max(...timestamps) : null;
}

function retryAtFromError(error: unknown): number | null {
  if (!(error instanceof LocalServiceError) || error.details === null) return null;
  const match = /retry-after-seconds=(\d{1,8})/.exec(error.details);
  if (!match) return null;
  const seconds = Number(match[1]);
  return Number.isSafeInteger(seconds) && seconds > 0 ? Date.now() + seconds * 1000 : null;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot(): ReadonlyMap<string, OAuthRateLimitState> {
  return states;
}

function setModelsRateLimit(models: readonly ChatModel[], state: OAuthRateLimitState): void {
  let changed = false;
  for (const model of models) {
    const key = modelRouteKey(model);
    const current = states.get(key);
    if (current?.reason !== state.reason || current.resetAtUnixMs !== state.resetAtUnixMs) {
      states.set(key, state);
      changed = true;
    }
  }
  if (changed) emitChange();
}

function emitChange(): void {
  for (const listener of listeners) listener();
}
