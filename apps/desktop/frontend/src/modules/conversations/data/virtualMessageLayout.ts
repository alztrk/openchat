/** Counts explicit and wrapped lines without allocating a substring for every source line. */
export function estimateWrappedLineCount(content: string, charsPerLine: number): number {
  let lineCount = 0;
  let lineStart = 0;

  while (true) {
    const lineBreak = content.indexOf('\n', lineStart);
    const lineEnd = lineBreak < 0 ? content.length : lineBreak;
    lineCount += Math.max(1, Math.ceil((lineEnd - lineStart) / charsPerLine));

    if (lineBreak < 0) return lineCount;
    lineStart = lineBreak + 1;
  }
}
