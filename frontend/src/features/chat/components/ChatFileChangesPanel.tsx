import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Check,
  FileDiff,
  FilePlus2,
  Info,
  LoaderCircle,
  NotebookPen,
  TriangleAlert,
  Trash2,
  X,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/shared/ui';
import { LocalServiceError } from '@/shared/services/LocalServiceClient';
import {
  listConversationFileChanges,
  readConversationFileChangeDiff,
  revertConversationFileChange,
  type ChatFileChange,
  type ChatFileChangeDiff,
} from '@/features/chat/data/chatFileChanges';

export interface ChatFileChangesPanelProps {
  conversationId: string;
  refreshRevision: string;
  onClose(): void;
  onChangesUpdated(conversationId: string, changes: ChatFileChange[]): void;
}

type FileChangeError = 'load' | 'diff' | 'conflict' | 'revert';

export function ChatFileChangesPanel({
  conversationId,
  refreshRevision,
  onChangesUpdated,
  onClose,
}: ChatFileChangesPanelProps) {
  const { t } = useTranslation();
  const [changes, setChanges] = useState<ChatFileChange[]>([]);
  const [selectedChangeId, setSelectedChangeId] = useState<string | null>(null);
  const [selectedDiff, setSelectedDiff] = useState<ChatFileChangeDiff | null>(null);
  const [error, setError] = useState<FileChangeError | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [revertingChangeId, setRevertingChangeId] = useState<string | null>(null);
  const [diffRevision, setDiffRevision] = useState(0);
  const selectedChangeIdRef = useRef<string | null>(null);
  const changesRequestRef = useRef(0);

  useEffect(() => {
    selectedChangeIdRef.current = selectedChangeId;
  }, [selectedChangeId]);

  const loadChanges = useCallback(async (selectChangeId?: string, keepError = false) => {
    const request = ++changesRequestRef.current;
    const previousSelection = selectedChangeIdRef.current;
    setIsLoading(true);
    if (!keepError) setError(null);
    setSelectedDiff(null);
    setIsLoadingDiff(false);
    if (selectChangeId !== undefined) setSelectedChangeId(selectChangeId);

    try {
      const updated = await listConversationFileChanges(conversationId);
      if (request !== changesRequestRef.current) return;
      const selection = selectChangeId ??
        (updated.some((change) => change.id === previousSelection) ? previousSelection : firstChangeId(updated));
      setChanges(updated);
      setSelectedChangeId(selection);
      onChangesUpdated(conversationId, updated);
    } catch {
      if (request !== changesRequestRef.current) return;
      setError('load');
    } finally {
      if (request === changesRequestRef.current) setIsLoading(false);
    }
  }, [conversationId, onChangesUpdated]);

  useEffect(() => {
    void loadChanges();
    return () => { changesRequestRef.current += 1; };
  }, [loadChanges, refreshRevision]);

  useEffect(() => {
    if (!selectedChangeId) {
      setSelectedDiff(null);
      setIsLoadingDiff(false);
      return;
    }
    let current = true;
    setSelectedDiff(null);
    setIsLoadingDiff(true);
    setError((currentError) => currentError === 'conflict' ? currentError : null);
    void readConversationFileChangeDiff(conversationId, selectedChangeId).then((diff) => {
      if (!current) return;
      setSelectedDiff(diff);
      setIsLoadingDiff(false);
    }).catch(() => {
      if (!current) return;
      setIsLoadingDiff(false);
      setError('diff');
    });
    return () => { current = false; };
  }, [conversationId, diffRevision, refreshRevision, selectedChangeId]);

  const revertChange = async (change: ChatFileChange) => {
    if (!change.canRevert || revertingChangeId !== null) return;
    setRevertingChangeId(change.id);
    setError(null);
    try {
      const updated = await revertConversationFileChange(conversationId, change.id);
      setChanges(updated);
      onChangesUpdated(conversationId, updated);
      setDiffRevision((revision) => revision + 1);
    } catch (failure) {
      const isConflict = failure instanceof LocalServiceError && failure.code === 'file_change_conflict';
      setError(isConflict ? 'conflict' : 'revert');
      if (isConflict) void loadChanges(change.id, true);
    } finally {
      setRevertingChangeId(null);
    }
  };

  const activeChanges = changes.filter((change) => change.status === 'active');
  const added = activeChanges.reduce((total, change) => total + (change.addedLines ?? 0), 0);
  const removed = activeChanges.reduce((total, change) => total + (change.removedLines ?? 0), 0);
  const errorKey = error === 'load'
    ? 'fileChangesLoadFailed'
    : error === 'diff'
      ? 'fileChangesDiffFailed'
      : error === 'conflict'
        ? 'fileChangesConflict'
        : error === 'revert'
          ? 'fileChangesRevertFailed'
          : null;

  return (
    <aside aria-label={t('fileChangesTitle')} className="chat-file-changes-panel min-h-0 min-w-0 shrink-0 border-border bg-surface">
      <header className="flex min-h-14 shrink-0 items-center gap-2 border-b border-border px-4">
        <FileDiff aria-hidden="true" className="shrink-0 text-accent" size={17} />
        <h2 className="min-w-0 flex-1 truncate text-sm font-semibold text-text">{t('fileChangesTitle')}</h2>
        <Button aria-label={t('close')} className="size-11" onClick={onClose} size="icon" variant="ghost">
          <X aria-hidden="true" size={17} />
        </Button>
      </header>
      <div className="flex h-10 shrink-0 items-center gap-3 border-b border-border px-3 text-xs">
        <span className="min-w-0 flex-1 truncate text-text">{t('fileChangesSummary', { count: activeChanges.length })}</span>
        <span className="shrink-0 font-semibold text-success">+{added}</span>
        <span className="shrink-0 font-semibold text-destructive">-{removed}</span>
      </div>
      {errorKey ? (
        <p className="mx-3 mt-2 rounded-control bg-destructive/10 px-2.5 py-2 text-xs text-destructive" role="alert">{t(errorKey)}</p>
      ) : null}
      {isLoading ? (
        <div className="flex min-h-0 flex-1 items-center justify-center" aria-label={t('messageHistoryLoading')} role="status">
          <LoaderCircle aria-hidden="true" className="animate-spin text-accent motion-reduce:animate-none" size={20} />
        </div>
      ) : changes.length === 0 ? (
        <p className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-xs leading-5 text-secondary-text">{t('fileChangesEmpty')}</p>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <ul aria-label={t('fileChangesTitle')} className="chat-file-changes-list min-h-0 shrink-0 overflow-y-auto py-1" role="list">
            {changes.map((change) => (
              <FileChangeRow
                change={change}
                isAnyReverting={revertingChangeId !== null}
                isReverting={revertingChangeId === change.id}
                isSelected={selectedChangeId === change.id}
                key={change.id}
                onRevert={() => void revertChange(change)}
                onSelect={() => { setError(null); setSelectedChangeId(change.id); }}
              />
            ))}
          </ul>
          <section aria-label={selectedDiff?.path ?? t('fileChangesDiffTitle')} className="chat-file-changes-diff flex min-h-0 flex-1 flex-col border-t border-border">
            <h3 className="h-10 shrink-0 truncate px-3 py-2.5 text-xs font-semibold text-text">
              {selectedDiff?.path ?? selectedChangePath(changes, selectedChangeId) ?? t('fileChangesDiffTitle')}
            </h3>
            <DiffContent diff={selectedDiff} isLoading={isLoadingDiff} />
          </section>
        </div>
      )}
    </aside>
  );
}

function DiffContent({ diff, isLoading }: { diff: ChatFileChangeDiff | null; isLoading: boolean }) {
  const { t } = useTranslation();
  if (isLoading) {
    return <div className="flex min-h-0 flex-1 items-center justify-center" role="status"><LoaderCircle aria-hidden="true" className="animate-spin text-accent motion-reduce:animate-none" size={18} /></div>;
  }
  if (!diff) return <div className="min-h-0 flex-1" />;
  if (diff.isBinary) return <DiffNotice>{t('fileChangesBinary')}</DiffNotice>;
  if (!diff.available) return <DiffNotice>{t('fileChangesDiffUnavailable')}</DiffNotice>;

  const lines = diff.diff.split('\n');
  return (
    <div aria-label={t('fileChangesDiffTitle')} className="openchat-scroll-area min-h-0 flex-1 overflow-auto px-2 py-1">
      <pre className="whitespace-pre font-code text-xs leading-5">
        {lines.map((line, index) => (
          <span className={`block select-text ${diffLineClass(line)}`} key={`${index}-${line.slice(0, 24)}`}>{line || ' '}</span>
        ))}
      </pre>
      {diff.truncated ? <p className="px-2 py-2 text-xs text-secondary-text">{t('fileChangesDiffTruncated')}</p> : null}
    </div>
  );
}

function DiffNotice({ children }: { children: string }) {
  return <p className="flex min-h-0 flex-1 items-center justify-center px-5 text-center text-xs leading-5 text-secondary-text">{children}</p>;
}

function FileChangeRow({
  change,
  isAnyReverting,
  isSelected,
  isReverting,
  onSelect,
  onRevert,
}: {
  change: ChatFileChange;
  isAnyReverting: boolean;
  isSelected: boolean;
  isReverting: boolean;
  onSelect(): void;
  onRevert(): void;
}) {
  const { t } = useTranslation();
  return (
    <li className={`mx-1.5 flex min-h-12 items-center gap-2 rounded-control px-2 ${isSelected ? 'border border-accent bg-composer' : 'border border-transparent hover:bg-hover'}`}>
      <button
        aria-pressed={isSelected}
        className="flex min-w-0 flex-1 items-center gap-2 py-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        onClick={onSelect}
        title={change.path}
        type="button"
      >
        <FileKindIcon kind={change.kind} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-xs font-medium text-text">{change.path}</span>
          <span className={`block truncate text-xs leading-4 ${change.status === 'conflict' ? 'text-destructive' : 'text-secondary-text'}`}>
            {change.status === 'active' ? t('fileChangesActive') : change.status === 'reverted' ? t('fileChangesReverted') : t('fileChangesConflict')}
          </span>
        </span>
        {change.addedLines !== null || change.removedLines !== null ? (
          <span className="flex shrink-0 gap-1 text-xs font-semibold">
            {change.addedLines !== null ? <span className="text-success">+{change.addedLines}</span> : null}
            {change.removedLines !== null ? <span className="text-destructive">-{change.removedLines}</span> : null}
          </span>
        ) : null}
      </button>
      {change.canRevert ? (
        <Button className="min-h-11 min-w-14 px-2 text-xs" disabled={isReverting || isAnyReverting} onClick={onRevert} size="small" variant="ghost">
          {isReverting ? <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={14} /> : t('fileChangesRevert')}
        </Button>
      ) : change.status === 'reverted' ? (
        <Check aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
      ) : change.status === 'conflict' ? (
        <span aria-label={t('fileChangesConflict')} title={t('fileChangesConflict')}><TriangleAlert aria-hidden="true" className="text-destructive" size={16} /></span>
      ) : !change.diffAvailable ? (
        <span aria-label={t('fileChangesDiffUnavailable')} title={t('fileChangesDiffUnavailable')}><Info aria-hidden="true" className="text-secondary-icon" size={16} /></span>
      ) : null}
    </li>
  );
}

function FileKindIcon({ kind }: { kind: ChatFileChange['kind'] }) {
  const iconProps = { 'aria-hidden': true as const, className: 'shrink-0 text-secondary-icon', size: 16 };
  if (kind === 'added') return <FilePlus2 {...iconProps} />;
  if (kind === 'deleted') return <Trash2 {...iconProps} />;
  return <NotebookPen {...iconProps} />;
}

function firstChangeId(changes: ChatFileChange[]): string | null {
  return changes.find((change) => change.status === 'active')?.id ?? changes[0]?.id ?? null;
}

function selectedChangePath(changes: ChatFileChange[], selectedId: string | null): string | null {
  return changes.find((change) => change.id === selectedId)?.path ?? null;
}

function diffLineClass(line: string): string {
  if (line.startsWith('+') && !line.startsWith('+++')) return 'text-success';
  if (line.startsWith('-') && !line.startsWith('---')) return 'text-destructive';
  return 'text-secondary-text';
}
