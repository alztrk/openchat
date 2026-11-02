import { LoaderCircle, Undo2 } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { FileDiffOptions } from '@pierre/diffs/react';
import { PatchDiff } from '@pierre/diffs/react';
import { Button } from '@shared/components';
import type { ChatFileChangeDiff } from '@features/chat/data/chatFileChanges';

const toolPatchOptions: FileDiffOptions<undefined, undefined> = {
  theme: { dark: 'pierre-dark', light: 'pierre-light' },
  themeType: 'system',
  diffStyle: 'unified',
  diffIndicators: 'bars',
  disableBackground: false,
  disableFileHeader: true,
  hunkSeparators: 'simple',
  unsafeCSS: `:host {
    --diffs-dark-bg: rgb(var(--composer));
    --diffs-light-bg: rgb(var(--composer));
    --diffs-bg: rgb(var(--composer));
    --diffs-bg-context-override: rgb(var(--composer));
    --diffs-bg-separator-override: rgb(var(--surface));
    --diffs-addition-color-override: rgb(var(--success));
    --diffs-deletion-color-override: rgb(var(--destructive));
  }`,
};
const panelPatchOptions: FileDiffOptions<undefined, undefined> = {
  ...toolPatchOptions,
  unsafeCSS: `:host {
    --diffs-dark-bg: transparent;
    --diffs-light-bg: transparent;
    --diffs-bg: transparent;
    --diffs-bg-context-override: transparent;
    --diffs-bg-separator-override: transparent;
    --diffs-addition-color-override: rgb(var(--success));
    --diffs-deletion-color-override: rgb(var(--destructive));
  }`,
};
function DiffNotice({ children }: { children: string }) {
  return (
    <p className="flex min-h-0 flex-1 items-center justify-center px-5 text-center text-xs leading-5 text-secondary-text">
      {children}
    </p>
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
  const hunkCount = useMemo(() => diff?.diff.match(/^@@/gm)?.length ?? 0, [diff?.diff]);

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
  if (hunkCount === 0) {
    return <DiffNotice>{t('fileChangesEmpty')}</DiffNotice>;
  }

  return (
    <section aria-label={t('fileChangesDiffTitle')} className="openchat-scroll-area min-h-0 flex-1 overflow-auto">
      {!flush && canRevertHunks ? (
        <div className="flex flex-wrap items-center gap-1 border-b border-border/60 px-2 py-1">
          {Array.from({ length: hunkCount }, (_, hunkIndex) => (
            <Button
              aria-label={`${t('fileChangesRejectHunk')} ${hunkIndex + 1}`}
              className="min-h-0 shrink-0 gap-1 rounded-control border-border bg-surface px-2 py-0.5 text-caption font-medium text-secondary-text transition-colors hover:bg-hover hover:text-destructive disabled:opacity-50"
              disabled={mutatingHunkIndex !== null}
              key={hunkIndex}
              onClick={() => onRevertHunk(hunkIndex)}
              size="small"
              type="button"
              variant="secondary"
            >
              {mutatingHunkIndex === hunkIndex ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={11} />
              ) : (
                <Undo2 aria-hidden="true" size={11} />
              )}
              <span>{t('fileChangesRejectHunk')}</span>
            </Button>
          ))}
        </div>
      ) : null}
      <PatchDiff
        className="tool-patch-diff"
        options={{ ...(flush ? toolPatchOptions : panelPatchOptions), diffStyle: viewMode }}
        patch={diff.diff}
      />
      {diff.truncated ? <p className="px-2 py-2 text-xs text-secondary-text">{t('fileChangesDiffTruncated')}</p> : null}
    </section>
  );
}
