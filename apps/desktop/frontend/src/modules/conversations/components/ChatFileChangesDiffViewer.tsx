import { LoaderCircle, Undo2 } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import type { ChatFileChangeDiff } from '@/modules/conversations/data/chatFileChanges';

export interface ParsedDiffHunk {
  index: number;
  header: string;
  lines: string[];
}

export function parseUnifiedDiffHunks(rawDiff: string): { fileHeaderLines: string[]; hunks: ParsedDiffHunk[] } {
  const allLines = rawDiff.split('\n');
  const fileHeaderLines: string[] = [];
  const hunks: ParsedDiffHunk[] = [];
  let currentHunk: ParsedDiffHunk | null = null;

  for (const line of allLines) {
    if (line.startsWith('@@')) {
      currentHunk = {
        index: hunks.length,
        header: line,
        lines: [],
      };
      hunks.push(currentHunk);
    } else if (currentHunk) {
      currentHunk.lines.push(line);
    } else {
      fileHeaderLines.push(line);
    }
  }
  return { fileHeaderLines, hunks };
}

export function keyedDiffLines(lines: readonly string[], prefix: string): { key: string; text: string }[] {
  const occurrences = new Map<string, number>();
  return lines.map((text) => {
    const occurrence = (occurrences.get(text) ?? 0) + 1;
    occurrences.set(text, occurrence);
    return { key: `${prefix}-${occurrence}-${text}`, text };
  });
}

export function diffLineClass(line: string): string {
  if (line.startsWith('+') && !line.startsWith('+++')) return 'bg-success/10 text-success';
  if (line.startsWith('-') && !line.startsWith('---')) return 'bg-destructive/10 text-destructive';
  return 'text-secondary-text';
}

export function DiffNotice({ children }: { children: string }) {
  return (
    <p className="flex min-h-0 flex-1 items-center justify-center px-5 text-center text-xs leading-5 text-secondary-text">
      {children}
    </p>
  );
}

export function SplitHunkView({ hunkIndex, lines }: { hunkIndex: number; lines: string[] }) {
  const leftLines: string[] = [];
  const rightLines: string[] = [];
  for (const line of lines) {
    if (line.startsWith('-')) {
      leftLines.push(line);
    } else if (line.startsWith('+')) {
      rightLines.push(line);
    } else {
      while (leftLines.length < rightLines.length) leftLines.push('');
      while (rightLines.length < leftLines.length) rightLines.push('');
      leftLines.push(line);
      rightLines.push(line);
    }
  }
  while (leftLines.length < rightLines.length) leftLines.push('');
  while (rightLines.length < leftLines.length) rightLines.push('');

  return (
    <div className="grid grid-cols-2 divide-x divide-border/60 overflow-x-auto font-code text-xs leading-5">
      <pre className="overflow-x-auto whitespace-pre px-2 py-1">
        {keyedDiffLines(leftLines, `left-${hunkIndex}`).map((line) => (
          <span className={`block select-text ${diffLineClass(line.text)}`} key={line.key}>
            {line.text || ' '}
          </span>
        ))}
      </pre>
      <pre className="overflow-x-auto whitespace-pre px-2 py-1">
        {keyedDiffLines(rightLines, `right-${hunkIndex}`).map((line) => (
          <span className={`block select-text ${diffLineClass(line.text)}`} key={line.key}>
            {line.text || ' '}
          </span>
        ))}
      </pre>
    </div>
  );
}

export interface DiffContentProps {
  diff: ChatFileChangeDiff | null;
  isLoading: boolean;
  viewMode: 'unified' | 'split';
  canRevertHunks: boolean;
  mutatingHunkIndex: number | null;
  onRevertHunk(hunkIndex: number): void;
}

export function DiffContent({
  diff,
  isLoading,
  viewMode,
  canRevertHunks,
  mutatingHunkIndex,
  onRevertHunk,
}: DiffContentProps) {
  const { t } = useTranslation();
  const parsed = useMemo(() => (diff?.diff ? parseUnifiedDiffHunks(diff.diff) : null), [diff?.diff]);

  if (isLoading) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center" role="status">
        <LoaderCircle aria-hidden="true" className="animate-spin text-accent motion-reduce:animate-none" size={18} />
      </div>
    );
  }
  if (!diff) return <div className="min-h-0 flex-1" />;
  if (diff.isBinary) return <DiffNotice>{t('fileChangesBinary')}</DiffNotice>;
  if (!diff.available) return <DiffNotice>{t('fileChangesDiffUnavailable')}</DiffNotice>;
  if (!parsed || parsed.hunks.length === 0) {
    return <DiffNotice>{t('fileChangesEmpty')}</DiffNotice>;
  }

  return (
    <section
      aria-label={t('fileChangesDiffTitle')}
      className="openchat-scroll-area min-h-0 flex-1 overflow-auto px-2 py-1.5"
    >
      <div className="flex flex-col gap-2">
        {parsed.hunks.map((hunk) => (
          <div className="overflow-hidden rounded-control border border-border/80 bg-composer" key={hunk.index}>
            <div className="flex items-center justify-between gap-2 border-b border-border/60 bg-raised px-2.5 py-1 text-caption">
              <span className="truncate font-code text-secondary-text">{hunk.header}</span>
              {canRevertHunks ? (
                <Button
                  className="min-h-0 shrink-0 gap-1 rounded-control border-border bg-surface px-2 py-0.5 text-caption font-medium text-secondary-text transition-colors hover:bg-hover hover:text-destructive disabled:opacity-50"
                  disabled={mutatingHunkIndex !== null}
                  onClick={() => onRevertHunk(hunk.index)}
                  size="small"
                  type="button"
                  variant="secondary"
                >
                  {mutatingHunkIndex === hunk.index ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" size={11} />
                  ) : (
                    <Undo2 aria-hidden="true" size={11} />
                  )}
                  <span>{t('fileChangesRejectHunk')}</span>
                </Button>
              ) : null}
            </div>
            {viewMode === 'unified' ? (
              <pre className="overflow-x-auto whitespace-pre px-2 py-1 font-code text-xs leading-5">
                {keyedDiffLines(hunk.lines, `hunk-${hunk.index}`).map((line) => (
                  <span className={`block select-text ${diffLineClass(line.text)}`} key={line.key}>
                    {line.text || ' '}
                  </span>
                ))}
              </pre>
            ) : (
              <SplitHunkView hunkIndex={hunk.index} lines={hunk.lines} />
            )}
          </div>
        ))}
      </div>
      {diff.truncated ? <p className="px-2 py-2 text-xs text-secondary-text">{t('fileChangesDiffTruncated')}</p> : null}
    </section>
  );
}
