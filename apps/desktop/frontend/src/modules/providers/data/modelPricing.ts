import type { JsonObject, LocalServiceClient } from '@/platform/local-service';
import { isObject } from '@/shared/utils/json';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type ModelPricingState = 'current' | 'stale' | 'unavailable';

export interface ModelPricingSnapshot {
  state: ModelPricingState;
  fetchedAtUnixMs: number | null;
  model: ModelPricingDetails | null;
}

export interface CachedModelPricing {
  cachedAtUnixMs: number;
  snapshot: ModelPricingSnapshot;
}

const modelPricingCacheStorageKey = 'openchat.modelPricing.v1';
const modelPricingCacheAgeMs = 6 * 60 * 60 * 1000;
const failedPricingRetryDelayMs = 10 * 60 * 1000;

export interface ModelPricingDetails {
  providerId: string;
  providerName: string | null;
  modelId: string;
  modelName: string | null;
  contextWindow: number | null;
  maximumOutput: number | null;
  inputModalities: string[];
  outputModalities: string[];
  supportsTools: boolean | null;
  pricing: ModelTokenPricing | null;
}

export interface ModelTokenPricing {
  input: number;
  output: number;
  reasoning: number | null;
  cacheRead: number | null;
  cacheWrite: number | null;
  tiers: ModelTokenPricingTier[];
}

export interface ModelTokenPricingTier {
  contextWindow: number;
  input: number;
  output: number;
  reasoning: number | null;
  cacheRead: number | null;
  cacheWrite: number | null;
}

export interface ModelPricingRequest {
  providerId: string;
  modelId: string;
}

export async function loadModelPricingBatch(
  client: LocalServiceClient,
  models: readonly ModelPricingRequest[],
): Promise<ReadonlyMap<string, ModelPricingSnapshot>> {
  const response = await client.call('models.pricing.get_many', {
    models: models.map(({ providerId, modelId }) => ({ providerId, modelId })),
  });
  if (!Array.isArray(response.models) || response.models.length !== models.length) {
    throw new Error('The model pricing response is invalid.');
  }
  const state = response.state;
  if (state !== 'current' && state !== 'stale' && state !== 'unavailable') {
    throw new Error('The model pricing state is invalid.');
  }
  const fetchedAtUnixMs = parseFetchedAt(response.fetchedAtUnixMs);
  const snapshots = new Map<string, ModelPricingSnapshot>();
  response.models.forEach((raw, index) => {
    if (!isObject(raw)) throw new Error('A model pricing item is invalid.');
    const request = models[index];
    if (!request || raw.providerId !== request.providerId || raw.modelId !== request.modelId) {
      throw new Error('The model pricing response did not match its request.');
    }
    snapshots.set(
      modelPricingKey(request.providerId, request.modelId),
      parseModelPricingSnapshot({
        state,
        fetchedAtUnixMs,
        model: raw.model,
      }),
    );
  });
  return snapshots;
}

export function modelPricingKey(providerId: string, modelId: string): string {
  return JSON.stringify([providerId, modelId]);
}

export function readModelPricingCache(): Map<string, CachedModelPricing> {
  const encoded = unifiedStorage.getItem(modelPricingCacheStorageKey);
  if (encoded === null) return new Map();
  let parsed: unknown;
  try {
    parsed = JSON.parse(encoded);
  } catch (cause) {
    throw new Error('The model pricing cache is invalid.', { cause });
  }
  if (!isObject(parsed)) throw new Error('The model pricing cache is invalid.');
  const cache = new Map<string, CachedModelPricing>();
  for (const [key, rawEntry] of Object.entries(parsed)) {
    if (!isObject(rawEntry) || !isSafeInteger(rawEntry.cachedAtUnixMs)) {
      throw new Error('A model pricing cache entry is invalid.');
    }
    if (!isObject(rawEntry.snapshot)) throw new Error('A model pricing cache snapshot is invalid.');
    cache.set(key, {
      cachedAtUnixMs: rawEntry.cachedAtUnixMs,
      snapshot: parseModelPricingSnapshot(rawEntry.snapshot),
    });
  }
  return cache;
}

export function writeModelPricingCache(cache: ReadonlyMap<string, CachedModelPricing>): void {
  unifiedStorage.setItem(modelPricingCacheStorageKey, JSON.stringify(Object.fromEntries(cache)));
}

export function isModelPricingCacheFresh(entry: CachedModelPricing, now = Date.now()): boolean {
  const age = now - entry.cachedAtUnixMs;
  if (age < 0 || entry.snapshot.model === null || entry.snapshot.model.pricing === null) return false;
  if (entry.snapshot.state !== 'current' || entry.snapshot.fetchedAtUnixMs === null) {
    return age < failedPricingRetryDelayMs;
  }
  return now >= entry.snapshot.fetchedAtUnixMs && now - entry.snapshot.fetchedAtUnixMs < modelPricingCacheAgeMs;
}

function parseModelPricingSnapshot(response: JsonObject): ModelPricingSnapshot {
  const state = response.state;
  if (state !== 'current' && state !== 'stale' && state !== 'unavailable') {
    throw new Error('The model pricing state is invalid.');
  }
  const parsedFetchedAtUnixMs = parseFetchedAt(response.fetchedAtUnixMs);
  const rawModel = response.model;
  let model: ModelPricingDetails | null = null;
  if (rawModel !== null && rawModel !== undefined) {
    if (!isObject(rawModel)) throw new Error('The model pricing details are invalid.');
    model = parseModelPricingDetails(rawModel);
  }
  return {
    state,
    fetchedAtUnixMs: parsedFetchedAtUnixMs,
    model,
  };
}

function parseFetchedAt(value: unknown): number | null {
  if (value === null || value === undefined) return null;
  if (!isSafeInteger(value)) throw new Error('The model pricing timestamp is invalid.');
  return value;
}

function parseModelPricingDetails(value: JsonObject): ModelPricingDetails {
  const providerId = requiredString(value.providerId, 'providerId');
  const modelId = requiredString(value.modelId, 'modelId');
  const rawPricing = value.pricing;
  let pricing: ModelTokenPricing | null = null;
  if (rawPricing !== null && rawPricing !== undefined) {
    if (!isObject(rawPricing)) throw new Error('The model token pricing is invalid.');
    pricing = parseTokenPricing(rawPricing);
  }
  return {
    providerId,
    providerName: optionalString(value.providerName),
    modelId,
    modelName: optionalString(value.modelName),
    contextWindow: positiveIntegerOrNull(value.contextWindow),
    maximumOutput: positiveIntegerOrNull(value.maximumOutput),
    inputModalities: stringList(value.inputModalities),
    outputModalities: stringList(value.outputModalities),
    supportsTools: typeof value.supportsTools === 'boolean' ? value.supportsTools : null,
    pricing,
  };
}

function parseTokenPricing(value: JsonObject): ModelTokenPricing {
  const input = nonNegativeNumber(value.input);
  const output = nonNegativeNumber(value.output);
  if (input === null || output === null) {
    throw new Error('The model token pricing rates are invalid.');
  }
  const tiers = unknownArray(value.tiers)
    .slice(0, 32)
    .flatMap((item) => (isObject(item) ? parsePricingTier(item) : []));
  return {
    input,
    output,
    reasoning: nonNegativeNumber(value.reasoning),
    cacheRead: nonNegativeNumber(value.cacheRead),
    cacheWrite: nonNegativeNumber(value.cacheWrite),
    tiers,
  };
}

function parsePricingTier(value: JsonObject): ModelTokenPricingTier[] {
  const contextWindow = positiveIntegerOrNull(value.context);
  const input = nonNegativeNumber(value.input);
  const output = nonNegativeNumber(value.output);
  if (contextWindow === null || input === null || output === null) return [];
  return [
    {
      contextWindow,
      input,
      output,
      reasoning: nonNegativeNumber(value.reasoning),
      cacheRead: nonNegativeNumber(value.cacheRead),
      cacheWrite: nonNegativeNumber(value.cacheWrite),
    },
  ];
}

function requiredString(value: unknown, field: string): string {
  const normalized = optionalString(value);
  if (normalized === null) throw new Error(`The model pricing ${field} is invalid.`);
  return normalized;
}

function optionalString(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : null;
}

function stringList(value: unknown): string[] {
  return unknownArray(value)
    .filter((item): item is string => typeof item === 'string' && item.length > 0 && item.length <= 64)
    .slice(0, 16);
}

function unknownArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function positiveIntegerOrNull(value: unknown): number | null {
  return isSafeInteger(value) && value > 0 ? value : null;
}

function nonNegativeNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null;
}

function isSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value);
}
