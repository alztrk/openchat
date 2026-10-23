import { type JsonObject, LocalServiceError, localServiceClient } from '@/platform/local-service';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { listProjects } from './chatStorage';

const savedSearchesKey = 'openchat.savedHistorySearches';
const maximumSavedSearches = 20;
const maximumTimestamp = 8_640_000_000_000_000;

export type HistoryResultType = 'all' | 'chats' | 'projects' | 'messages' | 'conversations';
export type HistorySortOrder = 'relevance' | 'newest' | 'oldest';

export interface HistorySearchFilters {
  providerId: string | null;
  modelId: string | null;
  projectId: string | null;
  isArchived: boolean | null;
  resultType: HistoryResultType;
  sortOrder: HistorySortOrder;
}

export interface HistorySearchFilterOptions {
  providerIds: string[];
  modelIds: string[];
}

export interface HistorySearchResult {
  kind: 'conversation' | 'message' | 'project';
  conversationId: string;
  conversationTitle: string;
  isConversationTitleMatch: boolean;
  messageId: string | null;
  role: 'user' | 'assistant' | null;
  excerpt: string;
  createdAtUnixMs: number;
  projectId: string | null;
}

export interface SavedHistorySearch {
  name: string;
  query: string;
  filters: HistorySearchFilters;
  fromUnixMs: number | null;
  throughUnixMs: number | null;
}

export class HistorySearchInputError extends Error {
  constructor(readonly reason: 'tooShort' | 'tooLong' | 'invalidDateRange') {
    super('History search input is invalid.');
    this.name = 'HistorySearchInputError';
  }
}

export class SavedHistorySearchError extends Error {
  constructor(readonly reason: 'loadFailed' | 'saveFailed' | 'limitReached') {
    super('Saved history searches could not be updated.');
    this.name = 'SavedHistorySearchError';
  }
}

export const emptyHistorySearchFilters: HistorySearchFilters = {
  providerId: null,
  modelId: null,
  projectId: null,
  isArchived: null,
  resultType: 'all',
  sortOrder: 'relevance',
};

export async function loadHistorySearchFilterOptions(): Promise<HistorySearchFilterOptions> {
  const response = await localServiceClient.call('chat.history.search_filters', {}, 15_000);
  if (
    !Array.isArray(response.providerIds) ||
    !Array.isArray(response.modelIds) ||
    response.providerIds.length > 100 ||
    response.modelIds.length > 200
  ) {
    throw new LocalServiceError('invalid_service_response', 'The history search filters are invalid.');
  }
  return {
    providerIds: parseStringOptions(response.providerIds, 256),
    modelIds: parseStringOptions(response.modelIds, 256),
  };
}

export async function searchHistory(
  query: string,
  filters: HistorySearchFilters,
  fromUnixMs: number | null,
  throughUnixMs: number | null,
): Promise<HistorySearchResult[]> {
  const normalizedQuery = query.trim();
  const queryLength = Array.from(normalizedQuery).length;
  if (queryLength < 2) throw new HistorySearchInputError('tooShort');
  if (queryLength > 512) throw new HistorySearchInputError('tooLong');
  if (fromUnixMs !== null && throughUnixMs !== null && fromUnixMs >= throughUnixMs) {
    throw new HistorySearchInputError('invalidDateRange');
  }

  const nativeResults = isLocalOnlyResultType(filters.resultType)
    ? []
    : await searchNativeHistory(normalizedQuery, filters, fromUnixMs, throughUnixMs);
  const localResults = await searchLocalLibrary(normalizedQuery, filters);
  const seenMessageKeys = new Set(
    nativeResults
      .filter((result) => result.messageId !== null)
      .map((result) => `${result.conversationId}:${result.messageId}`),
  );
  const deduplicatedLocalResults = localResults.filter((result) => {
    if (result.messageId === null) return true;
    const key = `${result.conversationId}:${result.messageId}`;
    if (seenMessageKeys.has(key)) return false;
    seenMessageKeys.add(key);
    return true;
  });
  const merged = [...nativeResults, ...deduplicatedLocalResults];
  if (filters.sortOrder === 'newest') merged.sort((left, right) => right.createdAtUnixMs - left.createdAtUnixMs);
  if (filters.sortOrder === 'oldest') merged.sort((left, right) => left.createdAtUnixMs - right.createdAtUnixMs);
  return merged.slice(0, 30);
}

async function searchNativeHistory(
  query: string,
  filters: HistorySearchFilters,
  fromUnixMs: number | null,
  throughUnixMs: number | null,
): Promise<HistorySearchResult[]> {
  const response = await localServiceClient.call(
    'chat.history.search',
    {
      query,
      fromUnixMs,
      throughUnixMs,
      providerId: filters.providerId,
      modelId: filters.modelId,
      projectId: filters.projectId,
      isArchived: filters.isArchived,
      resultType: nativeResultType(filters.resultType),
      sortOrder: filters.sortOrder,
    },
    8_000,
  );
  if (!Array.isArray(response.results) || response.results.length > 30) {
    throw new LocalServiceError('invalid_service_response', 'The history search results are invalid.');
  }
  return response.results.map(parseHistorySearchResult);
}

async function searchLocalLibrary(
  query: string,
  filters: HistorySearchFilters,
): Promise<HistorySearchResult[]> {
  const normalizedQuery = query.toLocaleLowerCase('en-US');
  const includeProjects =
    filters.resultType === 'all' || filters.resultType === 'projects' || filters.resultType === 'conversations';
  const projects = includeProjects ? await listProjects() : [];
  const results: HistorySearchResult[] = [];
  if (includeProjects) {
    for (const project of projects) {
      if (!project.name.toLocaleLowerCase('en-US').includes(normalizedQuery)) continue;
      if (filters.projectId !== null && filters.projectId !== project.id) continue;
      if (
        filters.providerId !== null ||
        filters.modelId !== null ||
        filters.isArchived !== null
      )
        continue;
      results.push({
        kind: 'project',
        conversationId: '',
        conversationTitle: project.name,
        isConversationTitleMatch: true,
        messageId: null,
        role: null,
        excerpt: project.folderPath,
        createdAtUnixMs: project.updatedAt,
        projectId: project.id,
      });
    }
  }
  return results;
}

export function loadSavedHistorySearches(): SavedHistorySearch[] {
  try {
    const raw = unifiedStorage.getItem(savedSearchesKey);
    if (raw === null) return [];
    const value: unknown = JSON.parse(raw);
    if (!Array.isArray(value) || value.length > maximumSavedSearches) {
      throw new Error('Saved history searches are invalid.');
    }
    return value.map(parseSavedHistorySearch);
  } catch {
    throw new SavedHistorySearchError('loadFailed');
  }
}

export function saveHistorySearch(search: SavedHistorySearch): SavedHistorySearch[] {
  const current = loadSavedHistorySearches();
  const remaining = current.filter(
    (item) => item.name.toLocaleLowerCase('en-US') !== search.name.toLocaleLowerCase('en-US'),
  );
  if (remaining.length >= maximumSavedSearches) {
    throw new SavedHistorySearchError('limitReached');
  }
  return writeSavedHistorySearches([search, ...remaining]);
}

export function deleteHistorySearch(name: string): SavedHistorySearch[] {
  const current = loadSavedHistorySearches();
  return writeSavedHistorySearches(
    current.filter((item) => item.name.toLocaleLowerCase('en-US') !== name.toLocaleLowerCase('en-US')),
  );
}

function writeSavedHistorySearches(searches: SavedHistorySearch[]): SavedHistorySearch[] {
  try {
    unifiedStorage.setItem(savedSearchesKey, JSON.stringify(searches));
    return searches;
  } catch {
    throw new SavedHistorySearchError('saveFailed');
  }
}

function parseStringOptions(value: unknown[], maximumLength: number): string[] {
  if (value.some((item) => typeof item !== 'string' || item.length === 0 || item.length > maximumLength)) {
    throw new LocalServiceError('invalid_service_response', 'A history search option is invalid.');
  }
  return value.filter((item): item is string => typeof item === 'string');
}

function isLocalOnlyResultType(resultType: HistoryResultType): boolean {
  return resultType === 'projects';
}

function nativeResultType(resultType: HistoryResultType): 'all' | 'messages' | 'conversations' {
  switch (resultType) {
    case 'chats':
    case 'all':
      return 'all';
    case 'messages':
      return 'messages';
    case 'conversations':
    case 'projects':
      return 'conversations';
  }
}

function parseHistorySearchResult(value: unknown): HistorySearchResult {
  if (
    !isObject(value) ||
    !isNonEmptyString(value.conversationId) ||
    !isNonEmptyString(value.conversationTitle) ||
    typeof value.isConversationTitleMatch !== 'boolean' ||
    typeof value.excerpt !== 'string' ||
    !isValidTimestamp(value.createdAtUnixMs)
  ) {
    throw new LocalServiceError('invalid_service_response', 'A history search result is invalid.');
  }
  const titleMatch = value.isConversationTitleMatch;
  const messageId = value.messageId;
  const role = value.role;
  if (
    (titleMatch && (messageId !== null || role !== null)) ||
    (!titleMatch && (!isNonEmptyString(messageId) || (role !== 'user' && role !== 'assistant')))
  ) {
    throw new LocalServiceError('invalid_service_response', 'A history search result is invalid.');
  }
  return {
    kind: titleMatch ? 'conversation' : 'message',
    conversationId: value.conversationId,
    conversationTitle: value.conversationTitle,
    isConversationTitleMatch: titleMatch,
    messageId: typeof messageId === 'string' ? messageId : null,
    role: role === 'user' || role === 'assistant' ? role : null,
    excerpt: value.excerpt,
    createdAtUnixMs: value.createdAtUnixMs,
    projectId: null,
  };
}

function parseSavedHistorySearch(value: unknown): SavedHistorySearch {
  if (
    !isObject(value) ||
    !isNonEmptyString(value.name) ||
    value.name.trim() !== value.name ||
    Array.from(value.name).length > 60 ||
    !isNonEmptyString(value.query) ||
    value.query.trim() !== value.query ||
    Array.from(value.query).length < 2 ||
    Array.from(value.query).length > 512 ||
    !isObject(value.filters)
  ) {
    throw new Error('A saved history search is invalid.');
  }
  const filters = parseHistorySearchFilters(value.filters);
  const fromUnixMs = value.fromUnixMs;
  const throughUnixMs = value.throughUnixMs;
  if (
    (fromUnixMs !== null && !isValidTimestamp(fromUnixMs)) ||
    (throughUnixMs !== null && !isValidTimestamp(throughUnixMs)) ||
    (typeof fromUnixMs === 'number' && typeof throughUnixMs === 'number' && fromUnixMs >= throughUnixMs)
  ) {
    throw new Error('A saved history date range is invalid.');
  }
  return {
    name: value.name,
    query: value.query,
    filters,
    fromUnixMs: typeof fromUnixMs === 'number' ? fromUnixMs : null,
    throughUnixMs: typeof throughUnixMs === 'number' ? throughUnixMs : null,
  };
}

function parseHistorySearchFilters(value: JsonObject): HistorySearchFilters {
  const storedResultType = value.resultType ?? 'all';
  // Keep saved searches created before saved responses were removed usable as message searches.
  const resultType = storedResultType === 'savedResponses' ? 'messages' : storedResultType;
  const sortOrder = value.sortOrder ?? 'relevance';
  if (
    !isOptionalString(value.providerId, 256) ||
    !isOptionalString(value.modelId, 256) ||
    !isOptionalString(value.projectId, 256) ||
    (value.isArchived !== null && typeof value.isArchived !== 'boolean') ||
    (resultType !== 'all' &&
      resultType !== 'chats' &&
      resultType !== 'projects' &&
      resultType !== 'messages' &&
      resultType !== 'conversations') ||
    (sortOrder !== 'relevance' && sortOrder !== 'newest' && sortOrder !== 'oldest')
  ) {
    throw new Error('Saved history search filters are invalid.');
  }
  return {
    providerId: typeof value.providerId === 'string' ? value.providerId : null,
    modelId: typeof value.modelId === 'string' ? value.modelId : null,
    projectId: typeof value.projectId === 'string' ? value.projectId : null,
    isArchived: typeof value.isArchived === 'boolean' ? value.isArchived : null,
    resultType,
    sortOrder,
  };
}

function isOptionalString(value: unknown, maximumLength: number): boolean {
  return value === null || (typeof value === 'string' && value.length > 0 && value.length <= maximumLength);
}

function isValidTimestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= maximumTimestamp;
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
