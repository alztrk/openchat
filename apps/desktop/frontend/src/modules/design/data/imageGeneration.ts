import type { ChatGptAccountQuotaStatus } from '@/modules/providers/data/oauthRateLimits';
import { loadProviderCredentialStatus } from '@/modules/providers/data/providerCredentials';
import { localServiceClient } from '@/platform/local-service';
import type { ServiceOperation } from '@/platform/local-service/LocalServiceClient';

export type ImageQuality = 'auto' | 'low' | 'medium' | 'high' | 'xhigh' | 'max';
export type ImageBackground = 'auto' | 'opaque' | 'transparent';
export type ImageModeration = 'auto' | 'low';
export type ImageOutputFormat = 'png' | 'jpeg' | 'webp';

export interface DesignImageTarget {
  key: string;
  kind: 'oauth' | 'api_key';
  providerId: string;
  providerName: string;
  connectionId: string;
  workspaceId: string | null;
  accountLabel: string | null;
  workspaceLabel: string | null;
  selected: boolean;
}

export interface DesignImageTargetsResult {
  targets: DesignImageTarget[];
  partialFailure: boolean;
}

export interface DesignImageRecord {
  id: string;
  generationId: string;
  providerRoute: 'oauth' | 'api_key' | 'unknown';
  prompt: string;
  model: string;
  size: string | null;
  quality: string | null;
  background: string | null;
  moderation: ImageModeration | null;
  outputFormat: ImageOutputFormat;
  outputCompression: number | null;
  mimeType: string;
  sizeBytes: number;
  createdAt: number;
  isFavorite: boolean;
  tags: string[];
}

export interface ImageLibraryFilters {
  query?: string;
  sort?: 'newest' | 'oldest';
  createdAfter?: number | null;
  createdBefore?: number | null;
  modelId?: string | null;
  providerRoute?: 'oauth' | 'api_key' | null;
  favoritesOnly?: boolean;
  tag?: string | null;
  collectionId?: string | null;
}

export interface ImageGenerationRun {
  id: string;
  sourceKind: 'design' | 'chat';
  providerRoute: 'oauth' | 'api_key';
  connectionId: string | null;
  workspaceId: string | null;
  conversationId: string | null;
  messageId: string | null;
  prompt: string;
  model: string;
  size: string | null;
  quality: string | null;
  background: string | null;
  moderation: ImageModeration | null;
  outputFormat: ImageOutputFormat | null;
  outputCompression: number | null;
  requestedCount: number;
  imageCount: number;
  status: 'preparing' | 'generating' | 'completed' | 'cancelled' | 'failed';
  errorCode: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface ImageCollection {
  id: string;
  name: string;
  createdAt: number;
  updatedAt: number;
}

export interface ImagePromptTemplate {
  id: string;
  name: string;
  prompt: string;
  createdAt: number;
  updatedAt: number;
}

export interface DesignImagePage {
  items: DesignImageRecord[];
  nextOffset: number | null;
}

export interface ImageGenerationOptions {
  prompt: string;
  model: string;
  size: string;
  quality: ImageQuality;
  background: ImageBackground;
  moderation: ImageModeration;
  outputFormat: ImageOutputFormat;
  outputCompression: number;
  count: number;
  target: DesignImageTarget;
}

export interface ImageGenerationResponse {
  images: DesignImageRecord[];
  accountQuota: ChatGptAccountQuotaStatus;
  failedCount: number;
  failureCode: string | null;
}

export async function loadDesignImageTargets(): Promise<DesignImageTargetsResult> {
  const [oauthResult, credentialsResult] = await Promise.allSettled([
    localServiceClient.call('chatgpt.connections.list'),
    loadProviderCredentialStatus(),
  ]);
  if (oauthResult.status === 'rejected' && credentialsResult.status === 'rejected') {
    throw new Error('Image provider connections could not be loaded.');
  }

  const targets: DesignImageTarget[] = [];
  const oauthConnections = oauthResult.status === 'fulfilled' ? readConnections(oauthResult.value) : null;
  if (oauthConnections !== null) {
    for (const connection of oauthConnections) {
      if (!isObject(connection) || connection.authStatus !== 'active' || typeof connection.id !== 'string') continue;
      const accountLabel = firstString(connection.email, connection.displayName);
      if (!Array.isArray(connection.workspaces)) continue;
      for (const workspace of connection.workspaces) {
        if (!isObject(workspace) || typeof workspace.id !== 'string') continue;
        targets.push({
          key: `oauth:${connection.id}:${workspace.id}`,
          kind: 'oauth',
          providerId: 'chatgpt',
          providerName: 'ChatGPT',
          connectionId: connection.id,
          workspaceId: workspace.id,
          accountLabel,
          workspaceLabel: firstString(workspace.displayName),
          selected: connection.isSelected === true && workspace.isSelected === true,
        });
      }
    }
  }
  if (credentialsResult.status === 'fulfilled') {
    for (const credential of credentialsResult.value.chatgptApiKeys) {
      targets.push({
        key: `api:${credential.id}`,
        kind: 'api_key',
        providerId: 'openai_api',
        providerName: 'OpenAI API',
        connectionId: credential.id,
        workspaceId: null,
        accountLabel: credential.keySuffix ? `••••${credential.keySuffix}` : null,
        workspaceLabel: null,
        selected: false,
      });
    }
  }
  return {
    targets,
    partialFailure:
      oauthResult.status === 'rejected' || oauthConnections === null || credentialsResult.status === 'rejected',
  };
}

export function defaultDesignImageTarget(targets: readonly DesignImageTarget[]): string {
  return targets.find((target) => target.selected)?.key ?? targets[0]?.key ?? '';
}

export async function generateDesignImages(
  options: ImageGenerationOptions,
): Promise<Omit<ServiceOperation, 'result'> & { result: Promise<ImageGenerationResponse> }> {
  const target = options.target;
  if (target.kind === 'oauth' && target.workspaceId === null) {
    throw new Error('Choose a ChatGPT workspace before generating an image.');
  }
  const auth =
    target.kind === 'oauth'
      ? { kind: 'oauth', connectionId: target.connectionId, workspaceId: target.workspaceId }
      : { kind: 'api_key', connectionId: target.connectionId };
  const operation = await localServiceClient.startOperation('chatgpt.images.generate', {
    prompt: options.prompt,
    model: options.model,
    size: options.size === 'auto' ? null : options.size,
    quality: options.quality,
    background: options.background,
    count: options.count,
    ...(target.kind === 'api_key' ? { moderation: options.moderation } : {}),
    ...(target.kind === 'api_key'
      ? {
          outputFormat: options.outputFormat,
          outputCompression: options.outputFormat === 'png' ? null : options.outputCompression,
        }
      : {}),
    auth,
  });
  const result = operation.result.then((response): ImageGenerationResponse => {
    const failedCount = response.failedCount;
    const failureCode = response.failureCode;
    if (
      !Array.isArray(response.images) ||
      !response.images.every(isDesignImageRecord) ||
      response.images.length === 0 ||
      typeof failedCount !== 'number' ||
      !Number.isSafeInteger(failedCount) ||
      failedCount < 0 ||
      failedCount > options.count ||
      !(failureCode === null || typeof failureCode === 'string') ||
      response.images.length + failedCount !== options.count ||
      (failedCount === 0 && failureCode !== null) ||
      (failedCount > 0 && failureCode === null)
    ) {
      throw new Error('The image-generation response is invalid.');
    }
    return {
      images: response.images,
      accountQuota: parseAccountQuotaStatus(response.accountQuota),
      failedCount,
      failureCode,
    };
  });
  return { ...operation, result };
}

export async function listDesignImages(
  offset: number,
  limit: number,
  filters: ImageLibraryFilters = {},
): Promise<DesignImagePage> {
  const response = await localServiceClient.call('design.images.list', {
    offset,
    limit,
    ...filters,
    query: filters.query?.trim() || null,
  });
  if (
    !Array.isArray(response.items) ||
    !response.items.every(isDesignImageRecord) ||
    !(response.nextOffset === null || typeof response.nextOffset === 'number')
  ) {
    throw new Error('The Design image library response is invalid.');
  }
  return {
    items: response.items,
    nextOffset: typeof response.nextOffset === 'number' ? response.nextOffset : null,
  };
}

export async function listImageGenerationRuns(limit = 20): Promise<ImageGenerationRun[]> {
  const response = await localServiceClient.call('design.generations.list', { limit });
  if (!Array.isArray(response.items) || !response.items.every(isImageGenerationRun)) {
    throw new Error('The image generation activity response is invalid.');
  }
  return response.items;
}

export async function saveImageAnnotation(input: {
  sourceKind: 'design' | 'chat';
  sourceKey: string;
  isFavorite: boolean;
  tags: string[];
}): Promise<void> {
  await localServiceClient.call('design.images.annotations.set', input);
}

export async function removeImageFromLibrary(sourceKind: 'design' | 'chat', sourceKey: string): Promise<void> {
  await localServiceClient.call('design.images.remove', { sourceKind, sourceKey });
}

export async function clearImageLibrary(): Promise<number> {
  const response = await localServiceClient.call('design.images.clear');
  if (!Number.isSafeInteger(response.removedCount) || typeof response.removedCount !== 'number') {
    throw new Error('The image library removal response is invalid.');
  }
  return response.removedCount;
}

export async function listImageCollections(): Promise<ImageCollection[]> {
  const response = await localServiceClient.call('design.collections.list');
  if (!Array.isArray(response.items) || !response.items.every(isImageCollection)) {
    throw new Error('The image collections response is invalid.');
  }
  return response.items;
}

export async function saveImageCollection(name: string, id?: string): Promise<ImageCollection> {
  const response = await localServiceClient.call('design.collections.save', { name, ...(id ? { id } : {}) });
  if (!isImageCollection(response.collection)) throw new Error('The saved image collection is invalid.');
  return response.collection;
}

export async function deleteImageCollection(id: string): Promise<void> {
  await localServiceClient.call('design.collections.delete', { id });
}

export async function setImageCollectionItem(input: {
  collectionId: string;
  sourceKind: 'design' | 'chat';
  sourceKey: string;
  included: boolean;
}): Promise<void> {
  await localServiceClient.call('design.collections.item.set', input);
}

export async function listImagePromptTemplates(): Promise<ImagePromptTemplate[]> {
  const response = await localServiceClient.call('design.templates.list');
  if (!Array.isArray(response.items) || !response.items.every(isImagePromptTemplate)) {
    throw new Error('The image prompt template response is invalid.');
  }
  return response.items;
}

export async function saveImagePromptTemplate(input: {
  name: string;
  prompt: string;
  id?: string;
}): Promise<ImagePromptTemplate> {
  const response = await localServiceClient.call('design.templates.save', input);
  if (!isImagePromptTemplate(response.template)) throw new Error('The saved image prompt template is invalid.');
  return response.template;
}

export async function deleteImagePromptTemplate(id: string): Promise<void> {
  await localServiceClient.call('design.templates.delete', { id });
}

export async function readDesignImage(
  id: string,
  expectedMimeType: string,
  expectedSizeBytes: number,
): Promise<{ mimeType: string; bytes: Uint8Array }> {
  if (
    !Number.isSafeInteger(expectedSizeBytes) ||
    expectedSizeBytes < 1 ||
    expectedSizeBytes > 32 * 1024 * 1024 ||
    !['image/png', 'image/jpeg', 'image/webp'].includes(expectedMimeType)
  ) {
    throw new Error('The saved Design image metadata is invalid.');
  }
  const chunkSize = 1024 * 1024;
  const bytes = new Uint8Array(expectedSizeBytes);
  let mimeType: string | null = null;
  for (let offset = 0; offset < expectedSizeBytes; offset += chunkSize) {
    const response = await localServiceClient.call('design.images.read', {
      id,
      offset,
      length: Math.min(chunkSize, expectedSizeBytes - offset),
    });
    if (
      typeof response.mimeType !== 'string' ||
      typeof response.dataBase64 !== 'string' ||
      response.totalBytes !== expectedSizeBytes ||
      response.offset !== offset ||
      (mimeType !== null && response.mimeType !== mimeType)
    ) {
      throw new Error('The saved Design image is invalid.');
    }
    mimeType = response.mimeType;
    const decoded = window.atob(response.dataBase64);
    const chunk = new Uint8Array(decoded.length);
    for (let index = 0; index < decoded.length; index += 1) chunk[index] = decoded.charCodeAt(index);
    if (chunk.length !== Math.min(chunkSize, expectedSizeBytes - offset)) {
      throw new Error('The saved Design image is incomplete.');
    }
    bytes.set(chunk, offset);
  }
  if (mimeType === null || mimeType !== expectedMimeType) {
    throw new Error('The saved Design image format changed.');
  }
  return { mimeType, bytes };
}

function parseAccountQuotaStatus(value: unknown): ChatGptAccountQuotaStatus {
  if (!isObject(value)) {
    return { state: 'unknown', usedPercent: null, resetAtUnixMs: null, reason: 'account_quota_invalid' };
  }
  if (
    value.state === 'available' ||
    value.state === 'exhausted' ||
    value.state === 'unknown' ||
    value.state === 'unavailable' ||
    value.state === 'not_applicable'
  ) {
    return {
      state: value.state,
      usedPercent: finiteNumber(value.usedPercent),
      resetAtUnixMs: finiteNumber(value.resetAtUnixMs),
      ...(typeof value.reason === 'string' ? { reason: value.reason } : {}),
    };
  }
  return { state: 'unknown', usedPercent: null, resetAtUnixMs: null, reason: 'account_quota_invalid' };
}

function isDesignImageRecord(value: unknown): value is DesignImageRecord {
  return (
    isObject(value) &&
    typeof value.id === 'string' &&
    typeof value.generationId === 'string' &&
    (value.providerRoute === 'oauth' || value.providerRoute === 'api_key' || value.providerRoute === 'unknown') &&
    typeof value.prompt === 'string' &&
    typeof value.model === 'string' &&
    isNullableString(value.size) &&
    isNullableString(value.quality) &&
    isNullableString(value.background) &&
    (value.moderation === null || value.moderation === 'auto' || value.moderation === 'low') &&
    (value.outputFormat === 'png' || value.outputFormat === 'jpeg' || value.outputFormat === 'webp') &&
    (value.outputCompression === null || typeof value.outputCompression === 'number') &&
    typeof value.mimeType === 'string' &&
    ['image/png', 'image/jpeg', 'image/webp'].includes(value.mimeType) &&
    typeof value.sizeBytes === 'number' &&
    Number.isSafeInteger(value.sizeBytes) &&
    value.sizeBytes > 0 &&
    value.sizeBytes <= 32 * 1024 * 1024 &&
    typeof value.createdAt === 'number' &&
    Number.isSafeInteger(value.createdAt) &&
    value.createdAt >= 0 &&
    typeof value.isFavorite === 'boolean' &&
    Array.isArray(value.tags) &&
    value.tags.length <= 20 &&
    value.tags.every((tag) => typeof tag === 'string' && tag.length > 0 && tag.length <= 48)
  );
}

function isImageGenerationRun(value: unknown): value is ImageGenerationRun {
  return (
    isObject(value) &&
    typeof value.id === 'string' &&
    (value.sourceKind === 'design' || value.sourceKind === 'chat') &&
    (value.providerRoute === 'oauth' || value.providerRoute === 'api_key') &&
    isNullableString(value.connectionId) &&
    isNullableString(value.workspaceId) &&
    isNullableString(value.conversationId) &&
    isNullableString(value.messageId) &&
    typeof value.prompt === 'string' &&
    typeof value.model === 'string' &&
    isNullableString(value.size) &&
    isNullableString(value.quality) &&
    isNullableString(value.background) &&
    (value.moderation === null || value.moderation === 'auto' || value.moderation === 'low') &&
    (value.outputFormat === null ||
      value.outputFormat === 'png' ||
      value.outputFormat === 'jpeg' ||
      value.outputFormat === 'webp') &&
    (value.outputCompression === null ||
      (typeof value.outputCompression === 'number' && Number.isSafeInteger(value.outputCompression))) &&
    Number.isSafeInteger(value.requestedCount) &&
    typeof value.requestedCount === 'number' &&
    Number.isSafeInteger(value.imageCount) &&
    typeof value.imageCount === 'number' &&
    (value.status === 'preparing' ||
      value.status === 'generating' ||
      value.status === 'completed' ||
      value.status === 'cancelled' ||
      value.status === 'failed') &&
    isNullableString(value.errorCode) &&
    Number.isSafeInteger(value.createdAt) &&
    typeof value.createdAt === 'number' &&
    Number.isSafeInteger(value.updatedAt) &&
    typeof value.updatedAt === 'number'
  );
}

function isImageCollection(value: unknown): value is ImageCollection {
  return (
    isObject(value) &&
    typeof value.id === 'string' &&
    typeof value.name === 'string' &&
    Number.isSafeInteger(value.createdAt) &&
    typeof value.createdAt === 'number' &&
    Number.isSafeInteger(value.updatedAt) &&
    typeof value.updatedAt === 'number'
  );
}

function isImagePromptTemplate(value: unknown): value is ImagePromptTemplate {
  return (
    isObject(value) &&
    typeof value.id === 'string' &&
    typeof value.name === 'string' &&
    typeof value.prompt === 'string' &&
    Number.isSafeInteger(value.createdAt) &&
    typeof value.createdAt === 'number' &&
    Number.isSafeInteger(value.updatedAt) &&
    typeof value.updatedAt === 'number'
  );
}

function firstString(...values: unknown[]): string | null {
  return values.find((value): value is string => typeof value === 'string' && value.trim().length > 0) ?? null;
}

function finiteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function readConnections(value: unknown): unknown[] | null {
  if (!isObject(value) || !Array.isArray(value.connections)) return null;
  return value.connections;
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}
