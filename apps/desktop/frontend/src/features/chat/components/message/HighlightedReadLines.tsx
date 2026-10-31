import { detectFileLanguage, highlightFileLines, useHighlightingState } from './FileSyntaxHighlight';
import { type ReadCodeLine, ReadCodeLines } from './ReadCodeLines';

export function HighlightedReadLines({ lines, path }: { lines: ReadCodeLine[]; path: string }) {
  // Subscribe to highlighting state changes so we re-render once Shiki loads
  useHighlightingState();

  const highlightedLines = highlightFileLines(
    detectFileLanguage(path),
    lines.map((line) => line.text || ' '),
  );

  return <ReadCodeLines contents={highlightedLines} lines={lines} />;
}
