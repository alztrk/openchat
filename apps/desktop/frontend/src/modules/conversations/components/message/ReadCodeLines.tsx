import type { ReactNode } from 'react';

export interface ReadCodeLine {
  number: number;
  text: string;
}

export function ReadCodeLines({
  contents,
  lines,
}: {
  contents: readonly ReactNode[];
  lines: readonly ReadCodeLine[];
}) {
  return (
    <ol className="max-h-64 overflow-auto rounded-control border border-border bg-surface py-1">
      {lines.map((line, index) => (
        <li className="grid grid-cols-tool-lines gap-2 px-2.5 py-0.5 font-code text-xs leading-5" key={line.number}>
          <span className="text-right text-secondary-text">{line.number}</span>
          <span className="whitespace-pre-wrap break-words text-text">{contents[index]}</span>
        </li>
      ))}
    </ol>
  );
}
