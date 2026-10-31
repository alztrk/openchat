/** Creates stable, occurrence-aware keys for lists whose records do not expose domain identifiers. */
export function withStableKeys<T>(
  items: readonly T[],
  identityOf: (item: T) => string,
  prefix: string,
): Array<{ item: T; key: string }> {
  const occurrences = new Map<string, number>();
  return items.map((item) => {
    const identity = identityOf(item);
    const baseKey = `${prefix}:${hashText(identity)}:${identity.length}`;
    const occurrence = occurrences.get(baseKey) ?? 0;
    occurrences.set(baseKey, occurrence + 1);
    return { item, key: `${baseKey}:${occurrence}` };
  });
}

function hashText(value: string): string {
  let first = 2166136261;
  let second = 0x9e3779b9;
  for (const character of value) {
    const codePoint = character.codePointAt(0);
    if (codePoint === undefined) continue;
    first = Math.imul(first ^ codePoint, 16777619);
    second = Math.imul(second ^ codePoint, 2246822519);
  }
  return `${(first >>> 0).toString(36)}-${(second >>> 0).toString(36)}`;
}
