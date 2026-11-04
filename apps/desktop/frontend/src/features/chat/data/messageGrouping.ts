export interface MessageGroupingCandidate {
  id: string;
  role: string;
}

export interface ResponseGroup<T extends MessageGroupingCandidate> {
  id: string;
  messages: [T, ...T[]];
}

export function groupResponses<T extends MessageGroupingCandidate>(messages: readonly T[]): ResponseGroup<T>[] {
  const groups: ResponseGroup<T>[] = [];

  for (const message of messages) {
    if (!message) continue;
    const previous = groups[groups.length - 1];
    const previousMessage = previous?.messages[previous.messages.length - 1];

    if (previous && message.role === 'assistant' && previousMessage?.role === 'assistant') {
      previous.messages.push(message);
      continue;
    }

    groups.push({ id: message.id, messages: [message] });
  }

  return groups;
}

export function startsNewDay(previous: number | null, current: number | null): boolean {
  if (previous === null || current === null) return previous !== current;
  if (previous === current) return false;

  const previousDate = new Date(previous);
  const currentDate = new Date(current);
  return (
    previousDate.getFullYear() !== currentDate.getFullYear() ||
    previousDate.getMonth() !== currentDate.getMonth() ||
    previousDate.getDate() !== currentDate.getDate()
  );
}
