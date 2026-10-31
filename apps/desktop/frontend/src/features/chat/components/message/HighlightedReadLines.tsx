import { detectFileLanguage, highlightFileLines } from './FileSyntaxHighlight';
import { type ReadCodeLine, ReadCodeLines } from './ReadCodeLines';

export function HighlightedReadLines({ lines, path }: { lines: ReadCodeLine[]; path: string }) {
  const highlightedLines = highlightFileLines(
    detectFileLanguage(path),
    lines.map((line) => line.text || ' '),
  );

  return <ReadCodeLines contents={highlightedLines} lines={lines} />;
}
