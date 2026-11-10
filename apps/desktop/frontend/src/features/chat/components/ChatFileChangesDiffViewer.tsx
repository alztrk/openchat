import { LoaderCircle, Undo2 } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@shared/components';
import type { ChatFileChangeDiff } from '@features/chat/data/chatFileChanges';
import {
  buildSplitHunk,
  type DiffHunk,
  parseUnifiedDiff,
} from './nativeDiffParser';

function DiffNotice({ children }: { children: string }) {
  return (
    <p className="flex min-h-0 flex-1 items-center justify-center px-5 text-center text-xs leading-5 text-secondary-text">
      {children}
    </p>
  );
}

function UnifiedDiffView({ hunks, flush }: { hunks: DiffHunk[]; flush: boolean }) {
  return (
    <div className={`w-full font-mono text-xs ${flush ? 'bg-composer' : 'bg-transparent'}`}>
      {hunks.map((hunk) => (
        <div key={hunk.id} className="border-b border-border/40 last:border-b-0">
          <div className="sticky top-0 z-1 flex items-center px-3 py-1 text-secondary-text bg-surface/90 backdrop-blur-xs border-y border-border/40 select-none font-medium">
            <span className="text-accent/90">{hunk.header}</span>
          </div>
          <div className="divide-y divide-border/20">
            {hunk.lines.map((line, index) => {
              const lineKey = `${hunk.id}-${index}`;
              const isAdd = line.kind === 'addition';
              const isDel = line.kind === 'deletion';
              const isComment = line.kind === 'comment';

              return (
                <div
                  key={lineKey}
                  className={`flex leading-5 hover:bg-hover/30 transition-colors ${
                    isAdd
                      ? 'bg-success/15 text-text'
                      : isDel
                        ? 'bg-destructive/15 text-text'
                        : isComment
                          ? 'bg-surface/50 text-secondary-text italic'
                          : 'text-text'
                  }`}
                >
                  <span className="w-10 shrink-0 select-none text-right pr-2 text-secondary-text/50">
                    {line.oldLineNumber ?? ''}
                  </span>
                  <span className="w-10 shrink-0 select-none text-right pr-2 text-secondary-text/50 border-r border-border/30">
                    {line.newLineNumber ?? ''}
                  </span>
                  <span
                    className={`w-6 shrink-0 select-none text-center font-bold ${
                      isAdd ? 'text-success' : isDel ? 'text-destructive' : 'text-secondary-text/40'
                    }`}
                  >
                    {isAdd ? '+' : isDel ? '-' : ' '}
                  </span>
                  <span className="min-w-0 flex-1 whitespace-pre pr-4 overflow-x-visible select-text">
                    {line.content}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

function SplitDiffView({ hunks, flush }: { hunks: DiffHunk[]; flush: boolean }) {
  const splitHunks = useMemo(() => hunks.map(buildSplitHunk), [hunks]);

  return (
    <div className={`w-full font-mono text-xs ${flush ? 'bg-composer' : 'bg-transparent'}`}>
      {splitHunks.map(({ hunk, rows }) => (
        <div key={hunk.id} className="border-b border-border/40 last:border-b-0">
          <div className="sticky top-0 z-1 flex items-center px-3 py-1 text-secondary-text bg-surface/90 backdrop-blur-xs border-y border-border/40 select-none font-medium">
            <span className="text-accent/90">{hunk.header}</span>
          </div>
          <div className="divide-y divide-border/20">
            {rows.map((row, index) => {
              const rowKey = `${hunk.id}-split-${index}`;
              const left = row.left;
              const right = row.right;

              return (
                <div key={rowKey} className="grid grid-cols-2 divide-x divide-border/40 leading-5">
                  {/* Left Side (Old/Deleted) */}
                  <div
                    className={`flex hover:bg-hover/30 transition-colors ${
                      left.kind === 'deletion'
                        ? 'bg-destructive/15 text-text'
                        : left.kind === 'empty'
                          ? 'bg-surface/20 select-none'
                          : 'text-text'
                    }`}
                  >
                    <span className="w-10 shrink-0 select-none text-right pr-2 text-secondary-text/50 border-r border-border/30">
                      {left.lineNumber ?? ''}
                    </span>
                    <span
                      className={`w-6 shrink-0 select-none text-center font-bold ${
                        left.kind === 'deletion' ? 'text-destructive' : 'text-secondary-text/40'
                      }`}
                    >
                      {left.kind === 'deletion' ? '-' : ' '}
                    </span>
                    <span className="min-w-0 flex-1 whitespace-pre pr-2 overflow-x-visible select-text">
                      {left.content}
                    </span>
                  </div>

                  {/* Right Side (New/Added) */}
                  <div
                    className={`flex hover:bg-hover/30 transition-colors ${
                      right.kind === 'addition'
                        ? 'bg-success/15 text-text'
                        : right.kind === 'empty'
                          ? 'bg-surface/20 select-none'
                          : 'text-text'
                    }`}
                  >
                    <span className="w-10 shrink-0 select-none text-right pr-2 text-secondary-text/50 border-r border-border/30">
                      {right.lineNumber ?? ''}
                    </span>
                    <span
                      className={`w-6 shrink-0 select-none text-center font-bold ${
                        right.kind === 'addition' ? 'text-success' : 'text-secondary-text/40'
                      }`}
                    >
                      {right.kind === 'addition' ? '+' : ' '}
                    </span>
                    <span className="min-w-0 flex-1 whitespace-pre pr-2 overflow-x-visible select-text">
                      {right.content}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

export interface DiffContentProps {
  diff: ChatFileChangeDiff | null;
  isLoading: boolean;
  viewMode: 'unified' | 'split';
  flush?: boolean;
  canRevertHunks: boolean;
  mutatingHunkIndex: number | null;
  onRevertHunk(hunkIndex: number): void;
}

export function DiffContent({
  diff,
  isLoading,
  viewMode,
  flush = false,
  canRevertHunks,
  mutatingHunkIndex,
  onRevertHunk,
}: DiffContentProps) {
  const { t } = useTranslation();
  const hunks = useMemo(() => (diff?.diff ? parseUnifiedDiff(diff.diff) : []), [diff?.diff]);

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
  if (hunks.length === 0) {
    return <DiffNotice>{t('fileChangesEmpty')}</DiffNotice>;
  }

  return (
    <section aria-label={t('fileChangesDiffTitle')} className="openchat-scroll-area min-h-0 flex-1 overflow-auto">
      {!flush && canRevertHunks ? (
        <div className="flex flex-wrap items-center gap-1 border-b border-border/60 px-2 py-1">
          {hunks.map((hunk) => (
            <Button
              aria-label={`${t('fileChangesRejectHunk')} ${hunk.index + 1}`}
              className="min-h-0 shrink-0 gap-1 rounded-control border-border bg-surface px-2 py-0.5 text-caption font-medium text-secondary-text transition-colors hover:bg-hover hover:text-destructive disabled:opacity-50"
              disabled={mutatingHunkIndex !== null}
              key={hunk.id}
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
              <span>{t('fileChangesRejectHunk')} {hunk.index + 1}</span>
            </Button>
          ))}
        </div>
      ) : null}
      {viewMode === 'split' ? (
        <SplitDiffView flush={flush} hunks={hunks} />
      ) : (
        <UnifiedDiffView flush={flush} hunks={hunks} />
      )}
      {diff.truncated ? <p className="px-2 py-2 text-xs text-secondary-text">{t('fileChangesDiffTruncated')}</p> : null}
    </section>
  );
}
