import { LocalServiceError, type JsonObject } from '@/shared/services/LocalServiceClient';

export interface HubModel {
  repoId: string;
  downloads: number;
  likes: number;
  tags: string[];
  pipelineTag: string | null;
  libraryName: string | null;
  license: string | null;
  gated: boolean;
  private: boolean;
}

export interface HubFile {
  path: string;
  kind: string;
  sizeBytes: number | null;
}

export interface HubDownloadGroup {
  id: string;
  displayName: string;
  files: HubFile[];
  totalBytes: number | null;
  canDownload: boolean;
}

export interface HubDetails {
  repoId: string;
  revision: string;
  gated: boolean;
  private: boolean;
  license: string | null;
  files: HubFile[];
  downloadGroups: HubDownloadGroup[];
  readmeStatus: ReadmeStatus;
  readmeMarkdown: string | null;
  readmeErrorCode: string | null;
}

export type ReadmeStatus = 'available' | 'missing' | 'access_denied' | 'too_large' | 'unavailable';
export type ModelSort = 'downloads' | 'likes' | 'recently_updated';
export type LoadState = 'idle' | 'loading' | 'ready' | 'error';
export type SearchErrorCode =
  | 'localModelDirectoryUnavailable'
  | 'modelSearchFailed'
  | 'modelSearchRateLimited'
  | 'modelSearchTimedOut'
  | 'modelSearchUnavailable'
  | 'modelSearchInvalidResponse'
  | 'modelDownloadAccessNeeded';

export const componentKinds = new Set(['model', 'vision', 'mtp', 'auxiliary']);

const readmeStatuses: readonly ReadmeStatus[] = [
  'available',
  'missing',
  'access_denied',
  'too_large',
  'unavailable',
];

export function parseHubModel(value: unknown): HubModel {
  if (!isObject(value) || !isNonEmptyString(value.repoId)) {
    throw new HubResponseError();
  }
  return {
    repoId: value.repoId,
    downloads: parseNonNegativeIntegerOrZero(value.downloads),
    likes: parseNonNegativeIntegerOrZero(value.likes),
    tags: parseStringList(value.tags),
    pipelineTag: typeof value.pipelineTag === 'string' ? value.pipelineTag : null,
    libraryName: typeof value.libraryName === 'string' ? value.libraryName : null,
    license: typeof value.license === 'string' ? value.license : null,
    gated: value.gated === true,
    private: value.private === true,
  };
}

export function parseHubDetails(value: JsonObject): HubDetails {
  if (
    !isNonEmptyString(value.repoId) ||
    !isNonEmptyString(value.revision) ||
    !Array.isArray(value.files) ||
    !Array.isArray(value.downloadGroups) ||
    !isReadmeStatus(value.readmeStatus)
  ) {
    throw new HubResponseError();
  }
  const readme = value.readme;
  const readmeErrorCode = value.readmeErrorCode;
  if ((value.readmeStatus === 'available') !== (typeof readme === 'string')) {
    throw new HubResponseError();
  }
  if (readmeErrorCode !== undefined && readmeErrorCode !== null && typeof readmeErrorCode !== 'string') {
    throw new HubResponseError();
  }
  return {
    repoId: value.repoId,
    revision: value.revision,
    gated: value.gated === true,
    private: value.private === true,
    license: typeof value.license === 'string' ? value.license : null,
    files: value.files.map(parseHubFile),
    downloadGroups: value.downloadGroups.map(parseHubDownloadGroup),
    readmeStatus: value.readmeStatus,
    readmeMarkdown: typeof readme === 'string' ? readme : null,
    readmeErrorCode: typeof readmeErrorCode === 'string' ? readmeErrorCode : null,
  };
}

export function searchErrorFor(error: unknown): SearchErrorCode {
  if (error instanceof LocalServiceError) {
    if (error.code === 'local_model_directory_unavailable') return 'localModelDirectoryUnavailable';
    if (error.code === 'hugging_face_rate_limited') return 'modelSearchRateLimited';
    if (error.code === 'hugging_face_unavailable') return 'modelSearchUnavailable';
    if (error.code === 'hugging_face_access_denied') return 'modelDownloadAccessNeeded';
    if (error.code === 'service_timeout') return 'modelSearchTimedOut';
    if (error.code === 'hugging_face_response_invalid') {
      return 'modelSearchInvalidResponse';
    }
    return 'modelSearchFailed';
  }
  return 'modelSearchFailed';
}

export function downloadErrorMessageKey(code: string | null):
  | 'modelDownloadAccessNeeded'
  | 'modelRevisionChanged'
  | 'localModelDirectoryUnavailable'
  | 'modelNoCompatibleFiles'
  | 'modelDownloadFailed' {
  switch (code) {
    case 'hugging_face_access_denied': return 'modelDownloadAccessNeeded';
    case 'hugging_face_revision_changed': return 'modelRevisionChanged';
    case 'local_model_directory_unavailable': return 'localModelDirectoryUnavailable';
    case 'hugging_face_model_format_unsupported': return 'modelNoCompatibleFiles';
    default: return 'modelDownloadFailed';
  }
}

export function readmeMessageKey(status: ReadmeStatus):
  | 'modelReadmeMissing'
  | 'modelReadmeAccessDenied'
  | 'modelReadmeTooLarge'
  | 'modelReadmeUnavailable' {
  switch (status) {
    case 'missing': return 'modelReadmeMissing';
    case 'access_denied': return 'modelReadmeAccessDenied';
    case 'too_large': return 'modelReadmeTooLarge';
    case 'unavailable': return 'modelReadmeUnavailable';
    case 'available': return 'modelReadmeUnavailable';
  }
}

export function readmeErrorMessageKey(code: string | null):
  | 'modelSearchRateLimited'
  | 'modelSearchTimedOut'
  | 'modelReadmeUnavailable' {
  switch (code) {
    case 'hugging_face_rate_limited': return 'modelSearchRateLimited';
    case 'service_timeout': return 'modelSearchTimedOut';
    default: return 'modelReadmeUnavailable';
  }
}

export function sortLabelKey(sort: ModelSort):
  | 'modelSortDownloads'
  | 'modelSortLikes'
  | 'modelSortRecentlyUpdated' {
  switch (sort) {
    case 'downloads': return 'modelSortDownloads';
    case 'likes': return 'modelSortLikes';
    case 'recently_updated': return 'modelSortRecentlyUpdated';
  }
}

export function isModelSort(value: string): value is ModelSort {
  return value === 'downloads' || value === 'likes' || value === 'recently_updated';
}

export function publisherName(repoId: string): string | null {
  const separator = repoId.indexOf('/');
  if (separator <= 0) return null;
  return repoId.slice(0, separator);
}

export function modelDisplayName(repoId: string): string {
  const name = repoId.split('/').pop()?.trim() ?? '';
  return name.length > 0 ? name : repoId;
}

export function componentKey(repoId: string, revision: string, path: string): string {
  return repoId + '\n' + revision + '\n' + path;
}

export function formatCompact(value: number, locale: string): string {
  return value.toLocaleString(locale, {
    maximumFractionDigits: 1,
    notation: 'compact',
  });
}

export function formatBytes(bytes: number, locale: string): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1000 && unitIndex < units.length - 1) {
    value /= 1000;
    unitIndex++;
  }
  if (unitIndex === 0) return `${Math.trunc(value).toLocaleString(locale)} ${units[unitIndex]}`;
  return `${value.toLocaleString(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} ${units[unitIndex]}`;
}

export function isActive(status: string): boolean {
  return status === 'starting' || status === 'downloading' || status === 'cancelling';
}

function parseHubFile(value: unknown): HubFile {
  if (
    !isObject(value) ||
    !isNonEmptyString(value.path) ||
    typeof value.kind !== 'string' ||
    !componentKinds.has(value.kind)
  ) {
    throw new HubResponseError();
  }
  const sizeBytes = parseOptionalNonNegativeInteger(value.sizeBytes);
  return { path: value.path, kind: value.kind, sizeBytes };
}

function parseHubDownloadGroup(value: unknown): HubDownloadGroup {
  if (
    !isObject(value) ||
    !isNonEmptyString(value.id) ||
    !isNonEmptyString(value.displayName) ||
    !Array.isArray(value.files)
  ) {
    throw new HubResponseError();
  }
  const totalBytes = parseOptionalNonNegativeInteger(value.totalBytes);
  const files = value.files.map(parseHubFile);
  return {
    id: value.id,
    displayName: value.displayName,
    files,
    totalBytes,
    canDownload: files.length > 0 && totalBytes !== null && files.every((file) => file.sizeBytes !== null),
  };
}

function parseOptionalNonNegativeInteger(value: unknown): number | null {
  if (value === undefined || value === null) return null;
  return isNonNegativeInteger(value) ? value : null;
}

function isReadmeStatus(value: unknown): value is ReadmeStatus {
  return typeof value === 'string' && readmeStatuses.some((status) => status === value);
}

function parseNonNegativeIntegerOrZero(value: unknown): number {
  return isNonNegativeInteger(value) ? value : 0;
}

function parseStringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export class HubResponseError extends Error {
  constructor() {
    super('The model catalog response is invalid.');
  }
}
