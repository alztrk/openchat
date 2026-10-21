import {
  Check,
  CheckCheck,
  Columns2,
  FileDiff,
  FilePlus2,
  Globe,
  Info,
  LoaderCircle,
  NotebookPen,
  Rows3,
  Trash2,
  TriangleAlert,
  Undo2,
  X,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import {
  approveAllConversationFileChanges,
  approveConversationFileChange,
  type ChatFileChange,
  type ChatFileChangeDiff,
  listConversationFileChanges,
  readConversationFileChangeDiff,
  revertAllConversationFileChanges,
  revertConversationFileChange,
  revertConversationFileChangeHunk,
} from '@/modules/conversations/data/chatFileChanges';
import { LocalServiceError } from '@/platform/local-service';
import { EmbeddedBrowserPanel } from './EmbeddedBrowserPanel';

export type RightWorkspaceTab = 'changes' | 'browser';

export interface ChatFileChangesPanelProps {
  conversationId: string | null;
  refreshRevision?: string;
  initialTab?: RightWorkspaceTab;
  initialBrowserUrl?: string;
  onClose(): void;
  onChangesUpdated?(conversationId: string, changes: ChatFileChange[]): void;
}

type FileChangeError = 'load' | 'diff' | 'conflict' | 'revert';

interface ParsedDiffHunk {
  index: number;
  header: string;
  lines: string[];
}

function parseUnifiedDiffHunks(rawDiff: string): { fileHeaderLines: string[]; hunks: ParsedDiffHunk[] } {
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

export function ChatFileChangesPanel({
  conversationId,
  refreshRevision,
  initialTab = 'changes',
  initialBrowserUrl,
  onChangesUpdated,
  onClose,
}: ChatFileChangesPanelProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<RightWorkspaceTab>(initialTab);
  const [diffViewMode, setDiffViewMode] = useState<'unified' | 'split'>('unified');
  const [changes, setChanges] = useState<ChatFileChange[]>([]);
  const [selectedChangeId, setSelectedChangeId] = useState<string | null>(null);
  const [selectedDiff, setSelectedDiff] = useState<ChatFileChangeDiff | null>(null);
  const [error, setError] = useState<FileChangeError | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [mutatingChangeId, setMutatingChangeId] = useState<string | null>(null);
  const [mutatingHunkIndex, setMutatingHunkIndex] = useState<number | null>(null);
  const [diffRevision, setDiffRevision] = useState(0);
  const selectedChangeIdRef = useRef<string | null>(null);
  const changesRequestRef = useRef(0);

  useEffect(() => {
    setActiveTab(initialTab);
  }, [initialTab]);

  useEffect(() => {
    selectedChangeIdRef.current = selectedChangeId;
  }, [selectedChangeId]);

  const loadChanges = useCallback(
    async (selectChangeId?: string, keepError = false) => {
      const request = ++changesRequestRef.current;
      const previousSelection = selectedChangeIdRef.current;
      setIsLoading(true);
      if (!keepError) setError(null);
      setSelectedDiff(null);
      setIsLoadingDiff(false);
      if (selectChangeId !== undefined) setSelectedChangeId(selectChangeId);

      if (!conversationId) {
        setChanges([]);
        setSelectedChangeId(null);
        setIsLoading(false);
        return;
      }

      try {
        const updated = await listConversationFileChanges(conversationId);
        if (request !== changesRequestRef.current) return;
        const selection =
          selectChangeId ??
          (updated.some((change) => change.id === previousSelection) ? previousSelection : firstChangeId(updated));
        setChanges(updated);
        setSelectedChangeId(selection);
        onChangesUpdated?.(conversationId, updated);
      } catch {
        if (request !== changesRequestRef.current) return;
        setError('load');
      } finally {
        if (request === changesRequestRef.current) setIsLoading(false);
      }
    },
    [conversationId, onChangesUpdated],
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: The revision is an external signal to reload the file-change list.
  useEffect(() => {
    void loadChanges();
    return () => {
      changesRequestRef.current += 1;
    };
  }, [loadChanges, refreshRevision]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Both revisions invalidate the selected diff after related file changes.
  useEffect(() => {
    if (!conversationId || !selectedChangeId) {
      setSelectedDiff(null);
      setIsLoadingDiff(false);
      return;
    }
    let current = true;
    setSelectedDiff(null);
    setIsLoadingDiff(true);
    setError((currentError) => (currentError === 'conflict' ? currentError : null));
    void readConversationFileChangeDiff(conversationId, selectedChangeId)
      .then((diff) => {
        if (!current) return;
        setSelectedDiff(diff);
        setIsLoadingDiff(false);
      })
      .catch(() => {
        if (!current) return;
        setIsLoadingDiff(false);
        setError('diff');
      });
    return () => {
      current = false;
    };
  }, [conversationId, diffRevision, refreshRevision, selectedChangeId]);

  const revertChange = async (change: ChatFileChange) => {
    if (!conversationId || !change.canRevert || mutatingChangeId !== null) return;
    setMutatingChangeId(change.id);
    setError(null);
    try {
      const updated = await revertConversationFileChange(conversationId, change.id);
      setChanges(updated);
      onChangesUpdated?.(conversationId, updated);
      setDiffRevision((revision) => revision + 1);
    } catch (failure) {
      const isConflict = failure instanceof LocalServiceError && failure.code === 'file_change_conflict';
      setError(isConflict ? 'conflict' : 'revert');
      if (isConflict) void loadChanges(change.id, true);
    } finally {
      setMutatingChangeId(null);
    }
  };

  const approveChange = async (change: ChatFileChange) => {
    if (!conversationId || change.status !== 'active' || change.approved || mutatingChangeId !== null) return;
    setMutatingChangeId(change.id);
    setError(null);
    try {
      const updated = await approveConversationFileChange(conversationId, change.id);
      setChanges(updated);
      onChangesUpdated?.(conversationId, updated);
      setDiffRevision((revision) => revision + 1);
    } catch (failure) {
      const isConflict = failure instanceof LocalServiceError && failure.code === 'file_change_conflict';
      setError(isConflict ? 'conflict' : 'revert');
      if (isConflict) void loadChanges(change.id, true);
    } finally {
      setMutatingChangeId(null);
    }
  };

  const handleApproveAll = async () => {
    if (!conversationId || mutatingChangeId !== null) return;
    setMutatingChangeId('__all__');
    setError(null);
    try {
      const updated = await approveAllConversationFileChanges(conversationId);
      setChanges(updated);
      onChangesUpdated?.(conversationId, updated);
      setDiffRevision((revision) => revision + 1);
    } catch {
      setError('revert');
    } finally {
      setMutatingChangeId(null);
    }
  };

  const handleRevertAll = async () => {
    if (!conversationId || mutatingChangeId !== null) return;
    setMutatingChangeId('__all__');
    setError(null);
    try {
      const updated = await revertAllConversationFileChanges(conversationId);
      setChanges(updated);
      onChangesUpdated?.(conversationId, updated);
      setDiffRevision((revision) => revision + 1);
    } catch {
      setError('revert');
    } finally {
      setMutatingChangeId(null);
    }
  };

  const handleRevertHunk = async (hunkIndex: number) => {
    if (!conversationId || !selectedChangeId || mutatingChangeId !== null || mutatingHunkIndex !== null) return;
    setMutatingHunkIndex(hunkIndex);
    setError(null);
    try {
      const updated = await revertConversationFileChangeHunk(conversationId, selectedChangeId, hunkIndex);
      setChanges(updated);
      onChangesUpdated?.(conversationId, updated);
      setDiffRevision((revision) => revision + 1);
    } catch (failure) {
      const isConflict = failure instanceof LocalServiceError && failure.code === 'file_change_conflict';
      setError(isConflict ? 'conflict' : 'revert');
      if (isConflict) void loadChanges(selectedChangeId, true);
    } finally {
      setMutatingHunkIndex(null);
    }
  };

  const activeChanges = changes.filter((change) => change.status === 'active');
  const pendingApprovalChanges = activeChanges.filter((change) => !change.approved);
  const selectedChange = useMemo(
    () => changes.find((change) => change.id === selectedChangeId) ?? null,
    [changes, selectedChangeId],
  );
  const added = activeChanges.reduce((total, change) => total + (change.addedLines ?? 0), 0);
  const removed = activeChanges.reduce((total, change) => total + (change.removedLines ?? 0), 0);
  const errorKey =
    error === 'load'
      ? 'fileChangesLoadFailed'
      : error === 'diff'
        ? 'fileChangesDiffFailed'
        : error === 'conflict'
          ? 'fileChangesConflict'
          : error === 'revert'
            ? 'fileChangesRevertFailed'
            : null;

  return (
    <aside
      aria-label={activeTab === 'browser' ? t('browserPanelTitle') : t('fileChangesTitle')}
      className="chat-file-changes-panel flex h-full min-h-0 min-w-0 shrink-0 flex-col border-border bg-surface"
    >
      <header className="flex h-12 shrink-0 items-center justify-between gap-2 border-b border-border px-3">
        <div className="flex min-w-0 items-center gap-1 rounded-control bg-raised p-1" role="tablist">
          <button
            aria-selected={activeTab === 'changes'}
            className={`inline-flex items-center gap-1.5 rounded-control px-2.5 py-1 text-xs font-semibold transition-colors ${
              activeTab === 'changes' ? 'bg-surface text-text shadow-xs' : 'text-secondary-text hover:text-text'
            }`}
            onClick={() => setActiveTab('changes')}
            role="tab"
            type="button"
          >
            <FileDiff aria-hidden="true" className="text-accent" size={14} />
            <span>{t('fileChangesTitle')}</span>
            {pendingApprovalChanges.length > 0 ? (
              <span className="rounded-full bg-accent/15 px-1.5 py-0.2 text-micro font-bold text-accent">
                {pendingApprovalChanges.length}
              </span>
            ) : null}
          </button>
          <button
            aria-selected={activeTab === 'browser'}
            className={`inline-flex items-center gap-1.5 rounded-control px-2.5 py-1 text-xs font-semibold transition-colors ${
              activeTab === 'browser' ? 'bg-surface text-text shadow-xs' : 'text-secondary-text hover:text-text'
            }`}
            onClick={() => setActiveTab('browser')}
            role="tab"
            type="button"
          >
            <Globe aria-hidden="true" className="text-accent" size={14} />
            <span>{t('browserPanelTitle')}</span>
          </button>
        </div>
        <Button aria-label={t('close')} className="size-9" onClick={onClose} size="icon" variant="ghost">
          <X aria-hidden="true" size={16} />
        </Button>
      </header>

      {activeTab === 'browser' ? (
        <EmbeddedBrowserPanel initialUrl={initialBrowserUrl} />
      ) : (
        <>
          <div className="flex min-h-10 shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border px-3 py-1.5 text-xs">
            <div className="flex min-w-0 items-center gap-2">
              <span className="truncate text-text">{t('fileChangesSummary', { count: activeChanges.length })}</span>
              <span className="shrink-0 font-semibold text-success">+{added}</span>
              <span className="shrink-0 font-semibold text-destructive">-{removed}</span>
            </div>
            {pendingApprovalChanges.length > 0 ? (
              <div className="flex items-center gap-1">
                <Button
                  className="h-7 gap-1 px-2 text-caption"
                  disabled={mutatingChangeId !== null}
                  onClick={() => void handleApproveAll()}
                  size="small"
                  variant="secondary"
                >
                  <CheckCheck aria-hidden="true" size={13} />
                  <span>{t('fileChangesApproveAll')}</span>
                </Button>
                <Button
                  className="h-7 gap-1 px-2 text-caption"
                  disabled={mutatingChangeId !== null}
                  onClick={() => void handleRevertAll()}
                  size="small"
                  variant="ghost"
                >
                  <Undo2 aria-hidden="true" size={13} />
                  <span>{t('fileChangesRejectAll')}</span>
                </Button>
              </div>
            ) : null}
          </div>
          {errorKey ? (
            <p
              className="mx-3 mt-2 rounded-control bg-destructive/10 px-2.5 py-2 text-xs text-destructive"
              role="alert"
            >
              {t(errorKey)}
            </p>
          ) : null}
          {isLoading ? (
            <div
              aria-label={t('messageHistoryLoading')}
              className="flex min-h-0 flex-1 items-center justify-center"
              role="status"
            >
              <LoaderCircle
                aria-hidden="true"
                className="animate-spin text-accent motion-reduce:animate-none"
                size={20}
              />
            </div>
          ) : changes.length === 0 ? (
            <p className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-xs leading-5 text-secondary-text">
              {t('fileChangesEmpty')}
            </p>
          ) : (
            <div className="flex min-h-0 flex-1 flex-col">
              <ul
                aria-label={t('fileChangesTitle')}
                className="chat-file-changes-list min-h-0 shrink-0 overflow-y-auto py-1"
              >
                {changes.map((change) => (
                  <FileChangeRow
                    change={change}
                    isAnyMutating={mutatingChangeId !== null}
                    isMutating={mutatingChangeId === change.id}
                    isSelected={selectedChangeId === change.id}
                    key={change.id}
                    onApprove={() => void approveChange(change)}
                    onRevert={() => void revertChange(change)}
                    onSelect={() => {
                      setError(null);
                      setSelectedChangeId(change.id);
                    }}
                  />
                ))}
              </ul>
              <section
                aria-label={selectedDiff?.path ?? t('fileChangesDiffTitle')}
                className="chat-file-changes-diff flex min-h-0 flex-1 flex-col border-t border-border"
              >
                <div className="flex h-10 shrink-0 items-center justify-between gap-2 border-b border-border/60 px-3">
                  <h3 className="min-w-0 flex-1 truncate text-xs font-semibold text-text">
                    {selectedDiff?.path ?? selectedChangePath(changes, selectedChangeId) ?? t('fileChangesDiffTitle')}
                  </h3>
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      aria-pressed={diffViewMode === 'unified'}
                      className={`inline-flex items-center gap-1 rounded-control px-2 py-0.5 text-caption font-medium transition-colors ${
                        diffViewMode === 'unified'
                          ? 'bg-accent text-white'
                          : 'bg-raised text-secondary-text hover:text-text'
                      }`}
                      onClick={() => setDiffViewMode('unified')}
                      type="button"
                    >
                      <Rows3 aria-hidden="true" size={12} />
                      <span>{t('fileChangesViewUnified')}</span>
                    </button>
                    <button
                      aria-pressed={diffViewMode === 'split'}
                      className={`inline-flex items-center gap-1 rounded-control px-2 py-0.5 text-caption font-medium transition-colors ${
                        diffViewMode === 'split'
                          ? 'bg-accent text-white'
                          : 'bg-raised text-secondary-text hover:text-text'
                      }`}
                      onClick={() => setDiffViewMode('split')}
                      type="button"
                    >
                      <Columns2 aria-hidden="true" size={12} />
                      <span>{t('fileChangesViewSplit')}</span>
                    </button>
                  </div>
                </div>
                <DiffContent
                  canRevertHunks={
                    selectedChange !== null &&
                    selectedChange.status === 'active' &&
                    !selectedChange.approved &&
                    selectedChange.canRevert &&
                    selectedChange.kind === 'modified'
                  }
                  diff={selectedDiff}
                  isLoading={isLoadingDiff}
                  mutatingHunkIndex={mutatingHunkIndex}
                  onRevertHunk={(hunkIndex) => void handleRevertHunk(hunkIndex)}
                  viewMode={diffViewMode}
                />
              </section>
            </div>
          )}
        </>
      )}
    </aside>
  );
}

function keyedDiffLines(lines: readonly string[], prefix: string): { key: string; text: string }[] {
  const occurrences = new Map<string, number>();
  return lines.map((text) => {
    const occurrence = (occurrences.get(text) ?? 0) + 1;
    occurrences.set(text, occurrence);
    return { key: `${prefix}-${occurrence}-${text}`, text };
  });
}

function DiffContent({
  diff,
  isLoading,
  viewMode,
  canRevertHunks,
  mutatingHunkIndex,
  onRevertHunk,
}: {
  diff: ChatFileChangeDiff | null;
  isLoading: boolean;
  viewMode: 'unified' | 'split';
  canRevertHunks: boolean;
  mutatingHunkIndex: number | null;
  onRevertHunk(hunkIndex: number): void;
}) {
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
                <button
                  className="inline-flex shrink-0 items-center gap-1 rounded-control border border-border bg-surface px-2 py-0.5 text-caption font-medium text-secondary-text transition-colors hover:bg-hover hover:text-destructive disabled:opacity-50"
                  disabled={mutatingHunkIndex !== null}
                  onClick={() => onRevertHunk(hunk.index)}
                  type="button"
                >
                  {mutatingHunkIndex === hunk.index ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" size={11} />
                  ) : (
                    <Undo2 aria-hidden="true" size={11} />
                  )}
                  <span>{t('fileChangesRejectHunk')}</span>
                </button>
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

function SplitHunkView({ hunkIndex, lines }: { hunkIndex: number; lines: string[] }) {
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

function DiffNotice({ children }: { children: string }) {
  return (
    <p className="flex min-h-0 flex-1 items-center justify-center px-5 text-center text-xs leading-5 text-secondary-text">
      {children}
    </p>
  );
}

function FileChangeRow({
  change,
  isAnyMutating,
  isSelected,
  isMutating,
  onSelect,
  onApprove,
  onRevert,
}: {
  change: ChatFileChange;
  isAnyMutating: boolean;
  isSelected: boolean;
  isMutating: boolean;
  onSelect(): void;
  onApprove(): void;
  onRevert(): void;
}) {
  const { t } = useTranslation();
  return (
    <li
      className={`mx-1.5 flex min-h-12 items-center gap-1.5 rounded-control px-2 ${
        isSelected ? 'border border-accent bg-composer' : 'border border-transparent hover:bg-hover'
      }`}
    >
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
      </button>
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

function FileKindIcon({ kind }: { kind: ChatFileChange['kind'] }) {
  const iconProps = { 'aria-hidden': true as const, className: 'shrink-0 text-secondary-icon', size: 16 };
  if (kind === 'added') return <FilePlus2 {...iconProps} />;
  if (kind === 'deleted') return <Trash2 {...iconProps} />;
  return <NotebookPen {...iconProps} />;
}

function firstChangeId(changes: ChatFileChange[]): string | null {
  return (
    changes.find((change) => change.status === 'active' && !change.approved)?.id ??
    changes.find((change) => change.status === 'active')?.id ??
    changes[0]?.id ??
    null
  );
}

function selectedChangePath(changes: ChatFileChange[], selectedId: string | null): string | null {
  return changes.find((change) => change.id === selectedId)?.path ?? null;
}

function diffLineClass(line: string): string {
  if (line.startsWith('+') && !line.startsWith('+++')) return 'bg-success/10 text-success';
  if (line.startsWith('-') && !line.startsWith('---')) return 'bg-destructive/10 text-destructive';
  return 'text-secondary-text';
}
