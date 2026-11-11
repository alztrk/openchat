import type { ChatMessage } from '@entities/conversation/types';

export interface MessageSiblingInfo {
  siblings: ChatMessage[];
  index: number;
  count: number;
}

/**
 * Returns messages along the active branch from root to the active leaf.
 * If activeLeafMessageId is null or not found, falls back to the latest message or the given list.
 */
export function getActiveBranchMessages(
  messages: readonly ChatMessage[],
  activeLeafMessageId?: string | null,
): ChatMessage[] {
  if (messages.length === 0) return [];

  const messageMap = new Map<string, ChatMessage>();
  for (const message of messages) {
    messageMap.set(message.id, message);
  }

  // If active leaf is specified and exists, trace up to root
  const effectiveLeafId =
    activeLeafMessageId && messageMap.has(activeLeafMessageId)
      ? activeLeafMessageId
      : findCandidateLeafId(messages);

  if (!effectiveLeafId || !messageMap.has(effectiveLeafId)) {
    return [...messages];
  }

  const branch: ChatMessage[] = [];
  const visited = new Set<string>();
  let current: ChatMessage | undefined = messageMap.get(effectiveLeafId);

  while (current && !visited.has(current.id)) {
    visited.add(current.id);
    branch.push(current);
    current = current.parentMessageId ? messageMap.get(current.parentMessageId) : undefined;
  }

  branch.reverse();
  return branch;
}

/**
 * Finds sibling messages that share the same parentMessageId and role.
 * Sibling messages are ordered by createdAt ASC, id ASC.
 */
export function getMessageSiblings(
  messages: readonly ChatMessage[],
  messageId: string,
): MessageSiblingInfo {
  const target = messages.find((m) => m.id === messageId);
  if (!target) {
    return { siblings: [], index: 0, count: 0 };
  }

  const targetParent = target.parentMessageId ?? null;
  const siblings = messages.filter(
    (message) => (message.parentMessageId ?? null) === targetParent && message.role === target.role,
  );

  siblings.sort((a, b) => {
    const timeA = a.createdAt ?? 0;
    const timeB = b.createdAt ?? 0;
    if (timeA !== timeB) return timeA - timeB;
    return a.id.localeCompare(b.id);
  });

  const index = siblings.findIndex((message) => message.id === messageId);
  return {
    siblings,
    index: Math.max(0, index),
    count: siblings.length,
  };
}

/**
 * Given a message ID, walks down children greedily towards the newest leaf.
 */
export function findLeafForMessage(
  messages: readonly ChatMessage[],
  targetMessageId: string,
): string {
  const childrenMap = new Map<string, ChatMessage[]>();
  for (const message of messages) {
    if (message.parentMessageId) {
      const existing = childrenMap.get(message.parentMessageId);
      if (existing) {
        existing.push(message);
      } else {
        childrenMap.set(message.parentMessageId, [message]);
      }
    }
  }

  // Sort children by createdAt DESC, id DESC so the newest branch is followed first
  for (const children of childrenMap.values()) {
    children.sort((a, b) => {
      const timeA = a.createdAt ?? 0;
      const timeB = b.createdAt ?? 0;
      if (timeA !== timeB) return timeB - timeA;
      return b.id.localeCompare(a.id);
    });
  }

  let currentId = targetMessageId;
  const visited = new Set<string>();

  while (!visited.has(currentId)) {
    visited.add(currentId);
    const children = childrenMap.get(currentId);
    if (!children || children.length === 0) break;
    const nextChild = children[0];
    if (!nextChild) break;
    currentId = nextChild.id;
  }

  return currentId;
}

function findCandidateLeafId(messages: readonly ChatMessage[]): string | null {
  if (messages.length === 0) return null;

  // Find messages that have no children
  const parentIds = new Set<string>();
  for (const message of messages) {
    if (message.parentMessageId) {
      parentIds.add(message.parentMessageId);
    }
  }

  const leaves = messages.filter((message) => !parentIds.has(message.id));
  if (leaves.length === 0) {
    return messages[messages.length - 1]?.id ?? null;
  }

  leaves.sort((a, b) => {
    const timeA = a.createdAt ?? 0;
    const timeB = b.createdAt ?? 0;
    if (timeA !== timeB) return timeB - timeA;
    return b.id.localeCompare(a.id);
  });

  return leaves[0]?.id ?? null;
}
