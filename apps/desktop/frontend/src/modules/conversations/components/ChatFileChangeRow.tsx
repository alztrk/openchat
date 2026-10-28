import { Check, FilePlus2, Info, LoaderCircle, NotebookPen, Trash2, TriangleAlert } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import type { ChatFileChange } from '@/modules/conversations/data/chatFileChanges';

export function FileKindIcon({ kind }: { kind: ChatFileChange['kind'] }) {
  const iconProps = { 'aria-hidden': true as const, className: 'shrink-0 text-secondary-icon', size: 16 };
  if (kind === 'added') return <FilePlus2 {...iconProps} />;
  if (kind === 'deleted') return <Trash2 {...iconProps} />;
  return <NotebookPen {...iconProps} />;
}

export interface FileChangeRowProps {
  change: ChatFileChange;
  isAnyMutating: boolean;
  isSelected: boolean;
  isMutating: boolean;
  onSelect(): void;
  onApprove(): void;
  onRevert(): void;
}

export function FileChangeRow({
  change,
  isAnyMutating,
  isSelected,
  isMutating,
  onSelect,
  onApprove,
  onRevert,
}: FileChangeRowProps) {
  const { t } = useTranslation();
  return (
    <li
      className={`mx-1.5 flex min-h-12 items-center gap-1.5 rounded-control px-2 ${
        isSelected ? 'border border-accent bg-composer' : 'border border-transparent hover:bg-hover'
      }`}
    >
      <Button
        aria-pressed={isSelected}
        className="min-h-0 min-w-0 flex-1 justify-start gap-2 py-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        onClick={onSelect}
        size="small"
        title={change.path}
        type="button"
        variant="ghost"
      >
        <FileKindIcon kind={change.kind} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-xs font-medium text-text">{change.path}</span>
          <span
            className={`block truncate text-xs leading-4 ${
              change.status === 'conflict'
                ? 'text-destructive'
                : change.approved
                  ? 'text-success font-medium'
                  : 'text-secondary-text'
            }`}
          >
            {change.approved
              ? t('fileChangesApproved')
              : change.status === 'active'
                ? t('fileChangesActive')
                : change.status === 'reverted'
                  ? t('fileChangesReverted')
                  : t('fileChangesConflict')}
          </span>
        </span>
        {change.addedLines !== null || change.removedLines !== null ? (
          <span className="flex shrink-0 gap-1 text-xs font-semibold">
            {change.addedLines !== null ? <span className="text-success">+{change.addedLines}</span> : null}
            {change.removedLines !== null ? <span className="text-destructive">-{change.removedLines}</span> : null}
          </span>
        ) : null}
      </Button>
      {change.status === 'active' && !change.approved ? (
        <div className="flex shrink-0 items-center gap-1">
          <Button
            aria-label={`${t('fileChangesApprove')}: ${change.path}`}
            className="h-7 px-2 text-xs text-success hover:text-success"
            disabled={isMutating || isAnyMutating}
            onClick={onApprove}
            size="small"
            title={t('fileChangesApprove')}
            variant="ghost"
          >
            <Check aria-hidden="true" size={14} />
            <span>{t('fileChangesApprove')}</span>
          </Button>
          {change.canRevert ? (
            <Button
              aria-label={`${t('fileChangesRevert')}: ${change.path}`}
              className="h-7 px-2 text-xs"
              disabled={isMutating || isAnyMutating}
              onClick={onRevert}
              size="small"
              variant="ghost"
            >
              {isMutating ? (
                <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={13} />
              ) : (
                t('fileChangesRevert')
              )}
            </Button>
          ) : null}
        </div>
      ) : change.approved ? (
        <span
          className="inline-flex items-center gap-1 rounded-full bg-success/15 px-2 py-0.5 text-caption font-medium text-success"
          title={t('fileChangesApproved')}
        >
          <Check aria-hidden="true" size={12} />
          <span>{t('fileChangesApproved')}</span>
        </span>
      ) : change.status === 'reverted' ? (
        <Check aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
      ) : change.status === 'conflict' ? (
        <span aria-label={t('fileChangesConflict')} role="img" title={t('fileChangesConflict')}>
          <TriangleAlert aria-hidden="true" className="text-destructive" size={16} />
        </span>
      ) : !change.diffAvailable ? (
        <span aria-label={t('fileChangesDiffUnavailable')} role="img" title={t('fileChangesDiffUnavailable')}>
          <Info aria-hidden="true" className="text-secondary-icon" size={16} />
        </span>
      ) : null}
    </li>
  );
}
