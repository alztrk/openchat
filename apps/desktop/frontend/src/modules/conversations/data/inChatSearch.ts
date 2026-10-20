import type { ChatMessage } from '@/modules/conversations/data/chatStorage';

export type InChatFilterType = 'all' | 'code' | 'error' | 'user';

export interface InChatMessageMatch {
  messageId: string;
  matchIndex: number;
  occurrenceIndexInMessage: number;
  start: number;
  end: number;
  matchedText: string;
}

const CODE_BLOCK_PATTERN = /```|~~~|`[^`]+`/;
const ERROR_KEYWORD_PATTERN = /\b(error|failed|failure|exception|crash|rejected|blocker)\b/i;

/** Checks whether a message contains code blocks, inline code, or tool execution content. */
export function isCodeBlockMessage(message: ChatMessage): boolean {
  if (CODE_BLOCK_PATTERN.test(message.content)) return true;
  if (message.toolActivities && message.toolActivities.length > 2) {
    return true;
  }
  return false;
}

/** Checks whether a message represents or mentions an error, failure, or blocker. */
export function isErrorMessage(message: ChatMessage): boolean {
  if (message.status === 'failed' || message.failureCode !== null) return true;
  return ERROR_KEYWORD_PATTERN.test(message.content);
}

/** Determines whether a message matches the active quick filter category. */
export function matchesInChatFilter(message: ChatMessage, filter: InChatFilterType): boolean {
  switch (filter) {
    case 'all':
      return true;
    case 'code':
      return isCodeBlockMessage(message);
    case 'error':
      return isErrorMessage(message);
    case 'user':
      return message.role === 'user';
    default:
      return true;
  }
}

/**
 * Searches across rendered conversation messages matching the keyword and active filter.
 *
 * When the query is empty but a specific filter is chosen, each matching message is returned
 * once to allow cycling between filtered messages.
 */
export function findInChatMatches(
  messages: readonly ChatMessage[],
  query: string,
  filter: InChatFilterType = 'all',
): InChatMessageMatch[] {
  const normalizedQuery = query.trim().toLowerCase();
  const matches: InChatMessageMatch[] = [];
  let globalMatchIndex = 0;

  for (const message of messages) {
    if (!matchesInChatFilter(message, filter)) continue;

    if (!normalizedQuery) {
      if (filter !== 'all') {
        matches.push({
          messageId: message.id,
          matchIndex: globalMatchIndex++,
          occurrenceIndexInMessage: 0,
          start: 0,
          end: 0,
          matchedText: '',
        });
      }
      continue;
    }

    const content = message.content ?? '';
    const lowerContent = content.toLowerCase();
    let searchStartIndex = 0;
    let occurrenceInMessage = 0;

    while (searchStartIndex < lowerContent.length) {
      const matchIndex = lowerContent.indexOf(normalizedQuery, searchStartIndex);
      if (matchIndex === -1) break;

      matches.push({
        messageId: message.id,
        matchIndex: globalMatchIndex++,
        occurrenceIndexInMessage: occurrenceInMessage++,
        start: matchIndex,
        end: matchIndex + normalizedQuery.length,
        matchedText: content.slice(matchIndex, matchIndex + normalizedQuery.length),
      });

      searchStartIndex = matchIndex + Math.max(1, normalizedQuery.length);
    }

    // If message content didn't match directly, check failureCode or toolActivities fallback
    if (occurrenceInMessage === 0) {
      const fallbackText = `${message.failureCode ?? ''} ${message.toolActivities ?? ''}`.toLowerCase();
      if (fallbackText.includes(normalizedQuery)) {
        matches.push({
          messageId: message.id,
          matchIndex: globalMatchIndex++,
          occurrenceIndexInMessage: 0,
          start: 0,
          end: 0,
          matchedText: '',
        });
      }
    }
  }

  return matches;
}
