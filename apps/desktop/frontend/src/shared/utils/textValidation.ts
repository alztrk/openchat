/** Returns whether a string contains ASCII control characters or DEL. */
export function containsAsciiControlCharacters(value: string): boolean {
  for (const character of value) {
    const codePoint = character.codePointAt(0);
    if (codePoint !== undefined && isAsciiControlCodePoint(codePoint)) return true;
  }
  return false;
}

/** Replaces ASCII control characters and DEL while preserving other Unicode code points. */
export function replaceAsciiControlCharacters(value: string, replacement: string): string {
  let result = '';
  for (const character of value) {
    const codePoint = character.codePointAt(0);
    result += codePoint !== undefined && isAsciiControlCodePoint(codePoint) ? replacement : character;
  }
  return result;
}

function isAsciiControlCodePoint(codePoint: number): boolean {
  return codePoint <= 0x1f || codePoint === 0x7f;
}
