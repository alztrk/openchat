import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';

const storageKey = 'openchat:recent_searches';

export const recentSearchLimits = {
  maximumCount: 5,
  maximumLength: 256,
} as const;

export function readRecentSearches(): string[] {
  const encoded = unifiedStorage.getItem(storageKey);
  if (encoded === null) return [];

  let value: unknown;
  try {
    value = JSON.parse(encoded);
  } catch {
    throw new Error('Recent search data is invalid.');
  }
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) {
    throw new Error('Recent search data is invalid.');
  }
  return normalizeRecentSearches(value);
}

export async function writeRecentSearches(terms: string[]): Promise<void> {
  const normalized = normalizeRecentSearches(terms);
  if (normalized.length === 0) {
    await unifiedStorage.removePersistedItem(storageKey);
    return;
  }
  await unifiedStorage.setPersistedItem(storageKey, JSON.stringify(normalized));
}

export function addRecentSearch(terms: string[], term: string): string[] {
  const trimmed = term.trim();
  if (!trimmed || trimmed.length > recentSearchLimits.maximumLength) return terms;
  return normalizeRecentSearches([
    trimmed,
    ...terms.filter((existing) => existing.toLocaleLowerCase('en-US') !== trimmed.toLocaleLowerCase('en-US')),
  ]);
}

export function removeRecentSearch(terms: string[], term: string): string[] {
  return terms.filter((existing) => existing !== term);
}

function normalizeRecentSearches(value: unknown[]): string[] {
  const terms: string[] = [];
  for (const item of value) {
    if (typeof item !== 'string') continue;
    const trimmed = item.trim();
    if (!trimmed || trimmed.length > recentSearchLimits.maximumLength) continue;
    if (terms.some((existing) => existing.toLocaleLowerCase('en-US') === trimmed.toLocaleLowerCase('en-US'))) continue;
    terms.push(trimmed);
    if (terms.length >= recentSearchLimits.maximumCount) break;
  }
  return terms;
}
