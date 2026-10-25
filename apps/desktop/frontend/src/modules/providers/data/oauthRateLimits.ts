import { useSyncExternalStore } from 'react';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';
import type { ChatModel } from './chatModels';
import { loadCopilotUsage } from './copilotOAuth';
import { modelRouteKey } from './modelPreferences';

export type OAuthRateLimitReason = 'quotaExceeded' | 'temporarilyRateLimited';

export interface OAuthRateLimitState {
  reason: OAuthRateLimitReason;
  resetAtUnixMs: number | null;
}

export type ChatGptAccountQuotaState = 'available' | 'exhausted' | 'unknown' | 'unavailable' | 'not_applicable';

export interface ChatGptAccountQuotaStatus {
  state: ChatGptAccountQuotaState;
  usedPercent: number | null;
  resetAtUnixMs: number | null;
  reason?: string;
}

const states = new Map<string, OAuthRateLimitState>();
const listeners = new Set<() => void>();
const resetTimers = new Map<string, number>();
let snapshot: ReadonlyMap<string, OAuthRateLimitState> = new Map();

export function useOAuthRateLimits(): ReadonlyMap<string, OAuthRateLimitState> {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export function rateLimitForModel(
  model: ChatModel,
  rateLimits: ReadonlyMap<string, OAuthRateLimitState>,
): OAuthRateLimitState | null {
  const state = rateLimits.get(modelRouteKey(model));
  if (state?.resetAtUnixMs !== null && state?.resetAtUnixMs !== undefined && state.resetAtUnixMs <= Date.now()) {
    return null;
  }
  return state ?? null;
}

export async function refreshChatGptAccountQuota(
  connectionId: string,
  workspaceId: string,
): Promise<ChatGptAccountQuotaStatus> {
  try {
    const snapshot = await localServiceClient.call('chatgpt.usage.get', { connectionId, workspaceId });
    return accountQuotaFromUsage(snapshot);
  } catch (error) {
    return {
      state: 'unavailable',
      usedPercent: null,
      resetAtUnixMs: null,
      reason: error instanceof LocalServiceError ? error.code : 'usage_refresh_failed',
    };
  }
}

export async function recordOAuthRateLimit(
  model: ChatModel,
  models: readonly ChatModel[],
  error: unknown,
): Promise<void> {
  if (model.providerId !== 'chatgpt' && model.providerId !== 'copilot') return;
  const retryAt = retryAtFromError(error);
  setModelsRateLimit(model.providerId === 'chatgpt' ? matchingOAuthModels(model, models) : [model], {
    reason: 'temporarilyRateLimited',
    resetAtUnixMs: retryAt,
  });

  try {
    if (model.providerId === 'chatgpt') {
      if (!model.connectionId || !model.workspaceId) return;
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
          quota.unlimited !== true &&
          quota.hasQuota !== false,
      );
      const chatQuotaDepleted = depleted.some((quota) => quota.kind === 'chat');
      const premiumQuotaDepleted = depleted.some((quota) => quota.kind === 'premiumInteractions');
      if (chatQuotaDepleted || premiumQuotaDepleted) {
        setModelsRateLimit(chatQuotaDepleted ? matchingOAuthModels(model, models) : [model], {
          reason: 'quotaExceeded',
          resetAtUnixMs: latestReset(
            depleted
              .filter((quota) => quota.kind === (chatQuotaDepleted ? 'chat' : 'premiumInteractions'))
              .map((quota) => quota.resetAtUnixMs),
          ),
        });
      }
    }
  } catch (cause) {
    globalThis.reportError(new Error('OAuth rate limit status could not be refreshed.', { cause }));
  }
}

export async function refreshOAuthRateLimits(models: readonly ChatModel[]): Promise<void> {
  pruneExpiredStates();
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
                  quota.unlimited !== true &&
                  quota.hasQuota !== false,
              );
              const copilotModels = models.filter((model) => model.providerId === 'copilot');
              const chatQuotaDepleted = depleted.some((quota) => quota.kind === 'chat');
              const premiumQuotaDepleted = depleted.some((quota) => quota.kind === 'premiumInteractions');
              if (chatQuotaDepleted) {
                updateQuotaState(
                  copilotModels,
                  true,
                  latestReset(depleted.filter((quota) => quota.kind === 'chat').map((quota) => quota.resetAtUnixMs)),
                );
              } else if (premiumQuotaDepleted) {
                const previouslyRateLimitedModels = copilotModels.filter(
                  (model) => states.get(modelRouteKey(model))?.reason === 'quotaExceeded',
                );
                updateQuotaState(
                  previouslyRateLimitedModels,
                  true,
                  latestReset(
                    depleted
                      .filter((quota) => quota.kind === 'premiumInteractions')
                      .map((quota) => quota.resetAtUnixMs),
                  ),
                );
                updateQuotaState(
                  copilotModels.filter((model) => !previouslyRateLimitedModels.includes(model)),
                  false,
                  null,
                );
              } else {
                updateQuotaState(copilotModels, false, null);
              }
            })
            .catch((cause: unknown) => {
              globalThis.reportError(new Error('Copilot OAuth quota status could not be loaded.', { cause }));
            }),
        ]
      : []),
  ]);
}

export function clearOAuthRateLimit(model: ChatModel): void {
  removeState(modelRouteKey(model));
}

function updateQuotaState(models: readonly ChatModel[], exhausted: boolean, resetAtUnixMs: number | null): void {
  let changed = false;
  for (const model of models) {
    const key = modelRouteKey(model);
    if (exhausted) {
      const current = states.get(key);
      if (current?.reason !== 'quotaExceeded' || current.resetAtUnixMs !== resetAtUnixMs) {
        states.set(key, { reason: 'quotaExceeded', resetAtUnixMs });
        scheduleExpiry(key, resetAtUnixMs);
        changed = true;
      }
    } else if (states.get(key)?.reason === 'quotaExceeded') {
      removeStateWithoutEmit(key);
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
    (candidate) => candidate.providerId === 'copilot' && candidate.connectionId === model.connectionId,
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
      isRecord(bucket) && typeof bucket.usedPercent === 'number' && bucket.usedPercent >= 100,
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

function accountQuotaFromUsage(value: unknown): ChatGptAccountQuotaStatus {
  if (!isRecord(value)) {
    return { state: 'unknown', usedPercent: null, resetAtUnixMs: null, reason: 'usage_snapshot_invalid' };
  }
  const now = Date.now();
  const accountBuckets = (Array.isArray(value.buckets) ? value.buckets : [])
    .filter(isRecord)
    .filter(
      (bucket) =>
        typeof bucket.limitId === 'string' &&
        bucket.limitId.startsWith('codex:') &&
        (typeof bucket.resetAtUnixMs !== 'number' || bucket.resetAtUnixMs > now),
    );
  const usage = parseChatGptUsage({ ...value, buckets: accountBuckets });
  const usedPercent = accountBuckets.reduce<number | null>((highest, bucket) => {
    if (typeof bucket.usedPercent !== 'number' || !Number.isFinite(bucket.usedPercent)) return highest;
    return highest === null ? bucket.usedPercent : Math.max(highest, bucket.usedPercent);
  }, null);
  const available = value.ordinaryUsageAllowed === true || usedPercent !== null;
  const resetAtUnixMs =
    usage.resetAtUnixMs ??
    (value.ordinaryUsageAllowed === false
      ? latestReset(
          accountBuckets.map((bucket) =>
            typeof bucket.resetAtUnixMs === 'number' && Number.isSafeInteger(bucket.resetAtUnixMs)
              ? bucket.resetAtUnixMs
              : null,
          ),
        )
      : null);
  const state = usage.exhausted ? 'exhausted' : available ? 'available' : 'unknown';
  return {
    state,
    usedPercent,
    resetAtUnixMs,
    ...(state === 'unknown' ? { reason: 'account_quota_not_reported' } : {}),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
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
  return snapshot;
}

function setModelsRateLimit(models: readonly ChatModel[], state: OAuthRateLimitState): void {
  let changed = false;
  for (const model of models) {
    const key = modelRouteKey(model);
    const current = states.get(key);
    if (current?.reason !== state.reason || current.resetAtUnixMs !== state.resetAtUnixMs) {
      states.set(key, state);
      scheduleExpiry(key, state.resetAtUnixMs);
      changed = true;
    }
  }
  if (changed) emitChange();
}

function emitChange(): void {
  snapshot = new Map(states);
  for (const listener of listeners) listener();
}

function pruneExpiredStates(): void {
  const now = Date.now();
  let changed = false;
  for (const [key, state] of states) {
    if (state.resetAtUnixMs !== null && state.resetAtUnixMs <= now) {
      removeStateWithoutEmit(key);
      changed = true;
    }
  }
  if (changed) emitChange();
}

function scheduleExpiry(key: string, resetAtUnixMs: number | null): void {
  const existingTimer = resetTimers.get(key);
  if (existingTimer !== undefined) window.clearTimeout(existingTimer);
  resetTimers.delete(key);
  if (resetAtUnixMs === null) return;
  const remainingMs = resetAtUnixMs - Date.now();
  if (remainingMs <= 0) {
    states.delete(key);
    return;
  }
  const timer = window.setTimeout(
    () => {
      const current = states.get(key);
      if (current?.resetAtUnixMs === null || current?.resetAtUnixMs === undefined) return;
      if (current.resetAtUnixMs <= Date.now()) {
        states.delete(key);
        resetTimers.delete(key);
        emitChange();
      } else {
        scheduleExpiry(key, current.resetAtUnixMs);
      }
    },
    Math.min(remainingMs, 2_147_000_000),
  );
  resetTimers.set(key, timer);
}

function removeState(key: string): void {
  const changed = removeStateWithoutEmit(key);
  if (changed) emitChange();
}

function removeStateWithoutEmit(key: string): boolean {
  const timer = resetTimers.get(key);
  if (timer !== undefined) window.clearTimeout(timer);
  resetTimers.delete(key);
  return states.delete(key);
}
