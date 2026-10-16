import { FileDiff, FilePlus2, Info, NotebookPen, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/shared/ui';
import type { ChatFileChange } from '@/features/chat/data/chatFileChanges';
import type { ToolActivity } from './messageData';

export interface FileChangesSummaryCardProps {
  activities: ToolActivity[];
  latestChanges: ChatFileChange[];
  onViewChanges?: () => void;
}

export function FileChangesSummaryCard({ activities, latestChanges, onViewChanges }: FileChangesSummaryCardProps) {
  const { t } = useTranslation();
  const latestById = new Map(latestChanges.map((change) => [change.id, change]));
  const changesById = new Map<string, ChatFileChange>();
  for (const activity of activities) {
    for (const change of activity.fileChanges) {
      changesById.set(change.id, latestById.get(change.id) ?? change);
    }
  }

  const changes = [...changesById.values()];
  const activeChanges = changes.filter((change) => change.status === 'active');
  const trackingFailed = activities.some((activity) => activity.fileChangesError !== null);
  if (changes.length === 0 && !trackingFailed) return null;

  const hasUnknownCounts = activeChanges.some((change) => change.addedLines === null || change.removedLines === null);
  const added = activeChanges.reduce((total, change) => total + (change.addedLines ?? 0), 0);
  const removed = activeChanges.reduce((total, change) => total + (change.removedLines ?? 0), 0);
  const title = trackingFailed && activeChanges.length === 0
    ? t('fileChangesUnavailableTitle')
    : t('fileChangesSummary', { count: activeChanges.length || changes.length });

  return (
    <section aria-label={t('fileChangesTitle')} className="mt-3 overflow-hidden rounded-card border border-border bg-surface">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 px-3 py-2.5">
        <span className="flex size-8 shrink-0 items-center justify-center rounded-control bg-composer text-accent">
          <FileDiff aria-hidden="true" size={17} />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold text-text">{title}</p>
          {activeChanges.length > 0 ? (
            <p className="text-xs text-secondary-text">
              {hasUnknownCounts ? t('fileChangesSomeCountsUnavailable') : t('fileChangesLineCounts', { added, removed })}
            </p>
          ) : null}
        </div>
        {onViewChanges ? (
          <Button className="min-h-11 shrink-0" onClick={onViewChanges} size="small" variant="ghost">
            {t('fileChangesView')}
          </Button>
        ) : null}
      </div>
      {activeChanges.length > 0 ? (
        <ul className="grid border-t border-border py-1">
          {activeChanges.slice(0, 3).map((change) => (
            <li className="flex min-w-0 items-center gap-2 px-3 py-2 text-xs" key={change.id}>
              <FileKindIcon kind={change.kind} />
              <span className="min-w-0 flex-1 truncate text-text" title={change.path}>{change.path}</span>
              {change.addedLines !== null ? <span className="shrink-0 font-semibold text-success">+{change.addedLines}</span> : null}
              {change.removedLines !== null ? <span className="shrink-0 font-semibold text-destructive">-{change.removedLines}</span> : null}
            </li>
          ))}
          {activeChanges.length > 3 ? (
            <li className="px-3 pb-2 text-xs text-secondary-text">{t('fileChangesMoreFiles', { count: activeChanges.length - 3 })}</li>
          ) : null}
        </ul>
      ) : changes.length > 0 ? (
        <p className="border-t border-border px-3 py-2 text-xs text-secondary-text">
          {changes.some((change) => change.status === 'conflict') ? t('fileChangesConflict') : t('fileChangesReverted')}
        </p>
      ) : null}
      {trackingFailed ? (
        <p className="flex items-start gap-2 border-t border-border px-3 py-2.5 text-xs text-destructive" role="alert">
          <Info aria-hidden="true" className="mt-0.5 shrink-0" size={15} />
          <span>{t('fileChangesTrackingFailed')}</span>
        </p>
      ) : null}
    </section>
  );
}

function FileKindIcon({ kind }: { kind: ChatFileChange['kind'] }) {
  const iconProps = { 'aria-hidden': true as const, className: 'shrink-0 text-secondary-icon', size: 15 };
  if (kind === 'added') return <FilePlus2 {...iconProps} />;
  if (kind === 'deleted') return <Trash2 {...iconProps} />;
  return <NotebookPen {...iconProps} />;
}
