export interface ParsedSearchQuery {
  cleanQuery: string;
  roleFilter?: 'user' | 'assistant';
  hasCodeOnly?: boolean;
}

/** Normalizes Turkish diacritics, dotted/dotless I, and casing for invariant search matching. */
export function normalizeSearchText(input: string): string {
  let result = '';
  for (const ch of input) {
    switch (ch) {
      case 'İ':
      case 'I':
      case 'ı':
        result += 'i';
        break;
      case 'Ç':
      case 'ç':
        result += 'c';
        break;
      case 'Ş':
      case 'ş':
        result += 's';
        break;
      case 'Ğ':
      case 'ğ':
        result += 'g';
        break;
      case 'Ü':
      case 'ü':
        result += 'u';
        break;
      case 'Ö':
      case 'ö':
        result += 'o';
        break;
      default:
        result += ch.toLowerCase();
        break;
    }
  }
  return result;
}

/** Parses search query operators such as is:user, is:ai, and has:code. */
export function parseSearchQuery(rawQuery: string): ParsedSearchQuery {
  const tokens = rawQuery.trim().split(/\s+/);
  const remainingTokens: string[] = [];
  let roleFilter: 'user' | 'assistant' | undefined;
  let hasCodeOnly: boolean | undefined;

  for (const token of tokens) {
    const lower = token.toLowerCase();
    if (lower === 'is:user' || lower === 'role:user') {
      roleFilter = 'user';
    } else if (
      lower === 'is:ai' ||
      lower === 'is:assistant' ||
      lower === 'role:ai' ||
      lower === 'role:assistant'
    ) {
      roleFilter = 'assistant';
    } else if (lower === 'has:code') {
      hasCodeOnly = true;
    } else if (token) {
      remainingTokens.push(token);
    }
  }

  return {
    cleanQuery: remainingTokens.join(' '),
    roleFilter,
    hasCodeOnly,
  };
}
