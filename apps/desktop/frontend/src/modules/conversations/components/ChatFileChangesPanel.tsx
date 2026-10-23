import {
  Check,
  CheckCheck,
  Columns2,
  FileCode2,
  FileDiff,
  FilePlus2,
  GitFork,
  Globe,
  Info,
  LoaderCircle,
  MessageSquare,
  NotebookPen,
  Plus,
  Rows3,
  Trash2,
  TriangleAlert,
  Undo2,
  X,
} from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/design-system';
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
import { readOpenConversationTabs, writeOpenConversationTabs } from '@/modules/conversations/data/sidebarPreferences';
import { LocalServiceError } from '@/platform/local-service';
import { EmbeddedBrowserPanel } from './EmbeddedBrowserPanel';
import { detectFileLanguage, highlightFileLines } from './message/FileSyntaxHighlight';
import { SubagentsPanel } from './SubagentsPanel';

export type RightWorkspaceTab = 'changes' | 'browser' | 'subagents';

type BuiltinWorkspaceTab = {
  id: string;
  kind: 'builtin';
  tab: RightWorkspaceTab;
  conversationId: string | null;
};
type FileWorkspaceTab = {
  id: string;
  kind: 'file';
  conversationId: string | null;
  fileName: string;
  content: string;
  language: string | null;
  changeId: string | null;
};
type ConversationWorkspaceTab = { id: string; kind: 'conversation'; conversationId: string };
type WorkspaceTab = BuiltinWorkspaceTab | FileWorkspaceTab | ConversationWorkspaceTab;

export type WorkspacePanelRequest =
  | { requestId: number; kind: 'builtin'; tab: RightWorkspaceTab; conversationId: string | null }
  | { requestId: number; kind: 'conversation'; conversationId: string };

export interface WorkspaceFileSelection {
  path: string;
  content?: string;
  changeId?: string;
}

export interface WorkspaceFileRequest extends WorkspaceFileSelection {
  requestId: number;
  conversationId?: string | null;
}

export interface ChatFileChangesPanelProps {
  conversationId: string | null;
  conversations?: readonly { id: string; title: string }[];
  conversationListReady?: boolean;
  refreshRevision?: string;
  initialTab?: RightWorkspaceTab | null;
  initialBrowserUrl?: string;
  fileRequest?: WorkspaceFileRequest | null;
  panelRequest?: WorkspacePanelRequest | null;
  renderConversation?(conversationId: string): React.ReactNode;
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
  conversations = [],
  conversationListReady = false,
  refreshRevision,
  initialTab = null,
  initialBrowserUrl,
  fileRequest = null,
  panelRequest = null,
  renderConversation,
  onChangesUpdated,
}: ChatFileChangesPanelProps) {
  const { t } = useTranslation();
  const workspaceId = useId();
  const [openTabs, setOpenTabs] = useState<WorkspaceTab[]>(() => [
    { id: 'builtin:changes:global', kind: 'builtin', tab: 'changes', conversationId: null },
    ...readOpenConversationTabs().map((id) => ({
      id: `conversation:${id}`,
      kind: 'conversation' as const,
      conversationId: id,
    })),
  ]);
  const [activeTabId, setActiveTabId] = useState<string>(() => {
    const restoredConversationTabs = readOpenConversationTabs();
    const lastConversationId = restoredConversationTabs.at(-1);
    return lastConversationId ? `conversation:${lastConversationId}` : 'builtin:changes:global';
  });
  const [tabStorageError, setTabStorageError] = useState(false);
  const handledFileRequestId = useRef<number | null>(null);
  const handledPanelRequestId = useRef<number | null>(null);
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
  const activeTab = openTabs.find((tab) => tab.id === activeTabId) ?? null;
  const activeTabConversationId = activeTab && 'conversationId' in activeTab ? activeTab.conversationId : null;
  const panelConversationId = activeTab?.kind === 'conversation' ? null : (activeTabConversationId ?? conversationId);

  const openBuiltinTab = useCallback(
    (tab: RightWorkspaceTab, tabConversationId: string | null = conversationId) => {
      const id = `builtin:${tab}:${tabConversationId ?? 'global'}`;
      const nextTab: BuiltinWorkspaceTab = { id, kind: 'builtin', tab, conversationId: tabConversationId };
      setOpenTabs((current) => (current.some((item) => item.id === id) ? current : [...current, nextTab]));
      setActiveTabId(id);
    },
    [conversationId],
  );

  const openConversationTab = useCallback((targetConversationId: string) => {
    const id = `conversation:${targetConversationId}`;
    const nextTab: ConversationWorkspaceTab = { id, kind: 'conversation', conversationId: targetConversationId };
    setOpenTabs((current) => (current.some((item) => item.id === id) ? current : [...current, nextTab]));
    setActiveTabId(id);
  }, []);

  useEffect(() => {
    if (initialTab) openBuiltinTab(initialTab, conversationId);
  }, [conversationId, initialTab, openBuiltinTab]);

  useEffect(() => {
    if (!activeTabId) return;
    document
      .getElementById(`${workspaceId}-tab-${activeTabId}`)
      ?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [activeTabId, workspaceId]);

  useEffect(() => {
    if (!panelRequest || handledPanelRequestId.current === panelRequest.requestId) return;
    handledPanelRequestId.current = panelRequest.requestId;
    if (panelRequest.kind === 'conversation') {
      openConversationTab(panelRequest.conversationId);
      return;
    }
    openBuiltinTab(panelRequest.tab, panelRequest.conversationId);
  }, [openBuiltinTab, openConversationTab, panelRequest]);

  useEffect(() => {
    const conversationTabs = openTabs.filter((tab): tab is ConversationWorkspaceTab => tab.kind === 'conversation');
    try {
      writeOpenConversationTabs(conversationTabs.map((tab) => tab.conversationId));
      setTabStorageError(false);
    } catch {
      setTabStorageError(true);
    }
  }, [openTabs]);

  useEffect(() => {
    if (!conversationListReady) return;
    const validConversationIds = new Set(conversations.map((item) => item.id));
    const invalidTabs = openTabs.filter(
      (tab): tab is ConversationWorkspaceTab =>
        tab.kind === 'conversation' && !validConversationIds.has(tab.conversationId),
    );
    if (invalidTabs.length === 0) return;
    const invalidTabIds = new Set(invalidTabs.map((tab) => tab.id));
    setOpenTabs((current) => current.filter((tab) => !invalidTabIds.has(tab.id)));
    if (invalidTabIds.has(activeTabId)) setActiveTabId('builtin:changes:global');
  }, [activeTabId, conversationListReady, conversations, openTabs]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Conversation changes must clear the selected diff and its error state.
  useEffect(() => {
    setSelectedChangeId(null);
    setSelectedDiff(null);
    setError(null);
  }, [panelConversationId]);

  useEffect(() => {
    selectedChangeIdRef.current = selectedChangeId;
  }, [selectedChangeId]);

  const loadChanges = useCallback(
    async (selectChangeId?: string, keepError = false, targetConversationId = panelConversationId) => {
      const request = ++changesRequestRef.current;
      const previousSelection = selectedChangeIdRef.current;
      setIsLoading(true);
      if (!keepError) setError(null);
      setSelectedDiff(null);
      setIsLoadingDiff(false);
      if (selectChangeId !== undefined) setSelectedChangeId(selectChangeId);

      if (!targetConversationId) {
        setChanges([]);
        setSelectedChangeId(null);
        setIsLoading(false);
        return;
      }

      try {
        const updated = await listConversationFileChanges(targetConversationId);
        if (request !== changesRequestRef.current) return;
        const selection =
          selectChangeId ??
          (updated.some((change) => change.id === previousSelection) ? previousSelection : firstChangeId(updated));
        setChanges(updated);
        setSelectedChangeId(selection);
        onChangesUpdated?.(targetConversationId, updated);
      } catch {
        if (request !== changesRequestRef.current) return;
        setError('load');
      } finally {
        if (request === changesRequestRef.current) setIsLoading(false);
      }
    },
    [onChangesUpdated, panelConversationId],
  );

  useEffect(() => {
    if (!fileRequest || handledFileRequestId.current === fileRequest.requestId) return;
    if (fileRequest.changeId) {
      handledFileRequestId.current = fileRequest.requestId;
      const requestConversationId = fileRequest.conversationId ?? conversationId;
      openBuiltinTab('changes', requestConversationId);
      void loadChanges(fileRequest.changeId, false, requestConversationId);
      return;
    }
    if (fileRequest.content === undefined || isLoading) return;

    const matchingChange = changes.find(
      (change) => normalizeFilePath(change.path) === normalizeFilePath(fileRequest.path),
    );
    const fileName = fileRequest.path.replaceAll('\\', '/').split('/').at(-1) || fileRequest.path;
    const tabId = `file-${fileRequest.requestId}`;
    handledFileRequestId.current = fileRequest.requestId;
    setOpenTabs((current) => [
      ...current.filter((tab) => tab.id !== tabId),
      {
        id: tabId,
        kind: 'file',
        conversationId: fileRequest.conversationId ?? conversationId,
        fileName,
        content: fileRequest.content ?? '',
        language: detectFileLanguage(fileName),
        changeId: matchingChange?.id ?? null,
      },
    ]);
    setActiveTabId(tabId);
    if (matchingChange) setSelectedChangeId(matchingChange.id);
  }, [changes, conversationId, fileRequest, isLoading, loadChanges, openBuiltinTab]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: The revision is an external signal to reload the file-change list.
  useEffect(() => {
    void loadChanges();
    return () => {
      changesRequestRef.current += 1;
    };
  }, [loadChanges, refreshRevision]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Both revisions invalidate the selected diff after related file changes.
  useEffect(() => {
    if (!panelConversationId || !selectedChangeId) {
      setSelectedDiff(null);
      setIsLoadingDiff(false);
      return;
    }
    let current = true;
    setSelectedDiff(null);
    setIsLoadingDiff(true);
    setError((currentError) => (currentError === 'conflict' ? currentError : null));
    void readConversationFileChangeDiff(panelConversationId, selectedChangeId)
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
  }, [diffRevision, panelConversationId, refreshRevision, selectedChangeId]);

  const revertChange = async (change: ChatFileChange) => {
    if (!panelConversationId || !change.canRevert || mutatingChangeId !== null) return;
    setMutatingChangeId(change.id);
    setError(null);
    try {
      const updated = await revertConversationFileChange(panelConversationId, change.id);
      setChanges(updated);
      onChangesUpdated?.(panelConversationId, updated);
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
    if (!panelConversationId || change.status !== 'active' || change.approved || mutatingChangeId !== null) return;
    setMutatingChangeId(change.id);
    setError(null);
    try {
      const updated = await approveConversationFileChange(panelConversationId, change.id);
      setChanges(updated);
      onChangesUpdated?.(panelConversationId, updated);
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
    if (!panelConversationId || mutatingChangeId !== null) return;
    setMutatingChangeId('__all__');
    setError(null);
    try {
      const updated = await approveAllConversationFileChanges(panelConversationId);
      setChanges(updated);
      onChangesUpdated?.(panelConversationId, updated);
      setDiffRevision((revision) => revision + 1);
    } catch {
      setError('revert');
    } finally {
      setMutatingChangeId(null);
    }
  };

  const handleRevertAll = async () => {
    if (!panelConversationId || mutatingChangeId !== null) return;
    setMutatingChangeId('__all__');
    setError(null);
    try {
      const updated = await revertAllConversationFileChanges(panelConversationId);
      setChanges(updated);
      onChangesUpdated?.(panelConversationId, updated);
      setDiffRevision((revision) => revision + 1);
    } catch {
      setError('revert');
    } finally {
      setMutatingChangeId(null);
    }
  };

  const handleRevertHunk = async (hunkIndex: number) => {
    if (!panelConversationId || !selectedChangeId || mutatingChangeId !== null || mutatingHunkIndex !== null) return;
    setMutatingHunkIndex(hunkIndex);
    setError(null);
    try {
      const updated = await revertConversationFileChangeHunk(panelConversationId, selectedChangeId, hunkIndex);
      setChanges(updated);
      onChangesUpdated?.(panelConversationId, updated);
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
  const closeWorkspaceTab = (tabId: string) => {
    const index = openTabs.findIndex((tab) => tab.id === tabId);
    const remaining = openTabs.filter((tab) => tab.id !== tabId);
    setOpenTabs(remaining);
    if (activeTabId === tabId) {
      setActiveTabId(remaining[Math.max(0, index - 1)]?.id ?? remaining[0]?.id ?? '');
    }
  };
  const pendingApprovalChanges = activeChanges.filter((change) => !change.approved);
  const selectedChange = useMemo(
    () => changes.find((change) => change.id === selectedChangeId) ?? null,
    [changes, selectedChangeId],
  );
  const conversationsById = useMemo(() => new Map(conversations.map((item) => [item.id, item])), [conversations]);
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
      aria-label={t('workspacePanelLabel')}
      className="chat-file-changes-panel flex h-full min-h-0 min-w-0 shrink-0 flex-col border-border bg-background"
    >
      <header className="flex h-12 shrink-0 items-center gap-1 border-b border-border bg-navigation/80 px-2 backdrop-blur-md">
        <div
          aria-label={t('workspaceTabs')}
          className="openchat-scroll-area flex min-w-0 flex-1 items-center gap-1 overflow-x-auto"
          role="tablist"
        >
          {openTabs.map((tab) => {
            const selected = tab.id === activeTabId;
            const label =
              tab.kind === 'file'
                ? tab.fileName
                : tab.kind === 'conversation'
                  ? (conversationsById.get(tab.conversationId)?.title ??
                    t(conversationListReady ? 'conversationUnavailable' : 'messageHistoryLoading'))
                  : t(builtinTabLabelKey(tab.tab));
            return (
              <div
                className={`workspace-tab-item flex h-9 max-w-64 min-w-0 shrink-0 items-center rounded-control px-1 ${selected ? 'bg-raised' : ''}`}
                key={tab.id}
              >
                <button
                  aria-controls={`${workspaceId}-panel`}
                  aria-selected={selected}
                  className={`workspace-tab-select flex h-full min-w-0 items-center gap-2 rounded-control px-2 text-xs font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                    selected ? 'text-text' : 'text-secondary-text hover:text-text'
                  }`}
                  id={`${workspaceId}-tab-${tab.id}`}
                  onClick={() => setActiveTabId(tab.id)}
                  onKeyDown={(event) => {
                    const currentIndex = openTabs.findIndex((openTab) => openTab.id === tab.id);
                    const targetIndex =
                      event.key === 'ArrowRight'
                        ? (currentIndex + 1) % openTabs.length
                        : event.key === 'ArrowLeft'
                          ? (currentIndex - 1 + openTabs.length) % openTabs.length
                          : event.key === 'Home'
                            ? 0
                            : event.key === 'End'
                              ? openTabs.length - 1
                              : null;
                    if (targetIndex === null) return;
                    event.preventDefault();
                    const targetId = openTabs[targetIndex]?.id;
                    if (!targetId) return;
                    setActiveTabId(targetId);
                    window.requestAnimationFrame(() =>
                      document.getElementById(`${workspaceId}-tab-${targetId}`)?.focus(),
                    );
                  }}
                  role="tab"
                  tabIndex={selected ? 0 : -1}
                  type="button"
                >
                  {tab.kind === 'file' ? (
                    <FileCode2 aria-hidden="true" className="shrink-0 text-accent" size={14} />
                  ) : tab.kind === 'conversation' ? (
                    <MessageSquare aria-hidden="true" className="shrink-0 text-accent" size={14} />
                  ) : tab.tab === 'changes' ? (
                    <FileDiff aria-hidden="true" className="shrink-0 text-accent" size={14} />
                  ) : tab.tab === 'browser' ? (
                    <Globe aria-hidden="true" className="shrink-0 text-accent" size={14} />
                  ) : (
                    <GitFork aria-hidden="true" className="shrink-0 text-accent" size={14} />
                  )}
                  <span className="truncate">{label}</span>
                  {tab.kind === 'builtin' &&
                  tab.tab === 'changes' &&
                  tab.conversationId === panelConversationId &&
                  pendingApprovalChanges.length > 0 ? (
                    <span className="rounded-full bg-accent/15 px-1.5 py-0.2 text-micro font-bold text-accent">
                      {pendingApprovalChanges.length}
                    </span>
                  ) : null}
                </button>
                <button
                  aria-label={t('workspaceCloseTab', { name: label })}
                  className="workspace-tab-close grid size-6 shrink-0 place-items-center rounded-control text-secondary-icon hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  onClick={() => closeWorkspaceTab(tab.id)}
                  title={t('workspaceCloseTab', { name: label })}
                  type="button"
                >
                  <X aria-hidden="true" size={12} />
                </button>
              </div>
            );
          })}
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button aria-label={t('workspaceAddTab')} className="size-8 shrink-0" size="icon" variant="ghost">
              <Plus aria-hidden="true" size={16} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" aria-label={t('workspaceAddTab')} className="min-w-48">
            <DropdownMenuItem onSelect={() => openBuiltinTab('changes', activeTabConversationId ?? conversationId)}>
              <FileDiff aria-hidden="true" size={15} />
              <span>{t('fileChangesTitle')}</span>
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => openBuiltinTab('browser', activeTabConversationId ?? conversationId)}>
              <Globe aria-hidden="true" size={15} />
              <span>{t('browserPanelTitle')}</span>
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => openBuiltinTab('subagents', activeTabConversationId ?? conversationId)}>
              <GitFork aria-hidden="true" size={15} />
              <span>{t('subagentsPanelTitle')}</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </header>

      <div
        aria-labelledby={activeTab ? `${workspaceId}-tab-${activeTab.id}` : undefined}
        className="flex min-h-0 flex-1 flex-col bg-background"
        id={`${workspaceId}-panel`}
        role="tabpanel"
      >
        {tabStorageError ? (
          <p className="shrink-0 border-b border-border px-3 py-2 text-xs text-destructive" role="alert">
            {t('workspaceTabsSaveFailed')}
          </p>
        ) : null}
        {!activeTab ? (
          <div className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-xs text-secondary-text">
            {t('workspaceEmpty')}
          </div>
        ) : activeTab.kind === 'file' ? (
          <div className="flex min-h-0 flex-1 flex-col">
            <div className="min-h-0 flex-1">
              <WorkspaceFileView file={activeTab} />
            </div>
            {activeTab.changeId && selectedDiff ? (
              <section
                aria-label={t('fileChangesDiffTitle')}
                className="flex min-h-0 flex-1 flex-col border-t border-border"
              >
                <h3 className="shrink-0 border-b border-border/60 px-3 py-2 text-xs font-semibold text-text">
                  {t('fileChangesDiffTitle')}
                </h3>
                <DiffContent
                  canRevertHunks={
                    selectedChange !== null &&
                    selectedChange.id === activeTab.changeId &&
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
            ) : null}
          </div>
        ) : activeTab.kind === 'conversation' ? (
          renderConversation ? (
            <div className="flex min-h-0 flex-1 flex-col">{renderConversation(activeTab.conversationId)}</div>
          ) : (
            <p
              className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-sm text-destructive"
              role="alert"
            >
              {t('conversationUnavailable')}
            </p>
          )
        ) : activeTab.tab === 'browser' ? (
          <EmbeddedBrowserPanel initialUrl={initialBrowserUrl} />
        ) : activeTab.tab === 'subagents' ? (
          <SubagentsPanel conversationId={panelConversationId} />
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
      </div>
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

function builtinTabLabelKey(
  tabId: RightWorkspaceTab,
): 'fileChangesTitle' | 'browserPanelTitle' | 'subagentsPanelTitle' {
  if (tabId === 'changes') return 'fileChangesTitle';
  if (tabId === 'browser') return 'browserPanelTitle';
  return 'subagentsPanelTitle';
}

function normalizeFilePath(path: string): string {
  return path.replaceAll('\\', '/').replace(/\/+$/, '').toLocaleLowerCase();
}

function WorkspaceFileView({ file }: { file: FileWorkspaceTab }) {
  const { t } = useTranslation();
  const lines = useMemo(
    () => file.content.split(/\r\n|\r|\n/).map((text, index) => ({ number: index + 1, text })),
    [file.content],
  );
  const highlightedLines = useMemo(
    () =>
      highlightFileLines(
        file.language,
        lines.map((line) => line.text),
      ),
    [file.language, lines],
  );

  return (
    <section aria-label={t('workspaceFileContent', { name: file.fileName })} className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-10 shrink-0 items-center gap-2 border-b border-border px-3 text-xs text-secondary-text">
        <FileCode2 aria-hidden="true" className="shrink-0 text-accent" size={14} />
        <span className="truncate" title={file.fileName}>
          {file.fileName}
        </span>
        <span className="ml-auto shrink-0">{t('workspaceFileLineCount', { count: lines.length })}</span>
      </div>
      {file.content.length === 0 ? (
        <p className="flex min-h-0 flex-1 items-center justify-center px-6 text-center text-xs text-secondary-text">
          {t('workspaceFileEmpty')}
        </p>
      ) : (
        <ol
          aria-label={t('workspaceFileContent', { name: file.fileName })}
          className="openchat-scroll-area min-h-0 flex-1 overflow-auto py-2 font-code text-xs leading-5"
        >
          {lines.map((line, index) => (
            <li className="grid grid-cols-tool-lines gap-2 px-3" key={line.number}>
              <span aria-hidden="true" className="select-none text-right text-secondary-text">
                {line.number}
              </span>
              <span className="min-w-0 whitespace-pre-wrap break-words text-text">{highlightedLines[index]}</span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
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
