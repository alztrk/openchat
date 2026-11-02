import {
  CheckCheck,
  Columns2,
  FileDiff,
  GitFork,
  Globe,
  LoaderCircle,
  MessageSquare,
  Paperclip,
  Plus,
  Rows3,
  Undo2,
  X,
} from 'lucide-react';
import { Fragment, useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@shared/components';
import {
  approveAllConversationFileChanges,
  approveConversationFileChange,
  type ChatFileChange,
  type ChatFileChangeDiff,
  listConversationFileChanges,
  recordFileChangesLoadFailure,
  readConversationFileChangeDiff,
  revertAllConversationFileChanges,
  revertConversationFileChange,
  revertConversationFileChangeHunk,
} from '@features/chat/data/chatFileChanges';
import { readOpenConversationTabs, writeOpenConversationTabs } from '@features/chat/data/sidebarPreferences';
import { LocalServiceError } from '@shared/lib/platform/local-service';
import { FileChangeRow, FileTypeIcon } from './ChatFileChangeRow';
import { DiffContent } from './ChatFileChangesDiffViewer';
import { EmbeddedBrowserPanel } from './EmbeddedBrowserPanel';
import { detectFileLanguage } from './message/detectFileLanguage';
import { SubagentsPanel } from './SubagentsPanel';
import { ConversationAttachmentsPanel } from './ConversationAttachmentsPanel';
import { type FileWorkspaceTab, WorkspaceFileView } from './WorkspaceFileView';

export type RightWorkspaceTab = 'changes' | 'browser' | 'subagents' | 'attachments';

type BuiltinWorkspaceTab = {
  id: string;
  kind: 'builtin';
  tab: RightWorkspaceTab;
  conversationId: string | null;
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

type FileChangeError = 'load' | 'diff' | 'conflict' | 'approveConflict' | 'applied' | 'revertAll' | 'revert';

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
    { id: 'builtin:changes', kind: 'builtin', tab: 'changes', conversationId },
    ...readOpenConversationTabs().map((id) => ({
      id: `conversation:${id}`,
      kind: 'conversation' as const,
      conversationId: id,
    })),
  ]);
  const [activeTabId, setActiveTabId] = useState<string>(() => {
    const restoredConversationTabs = readOpenConversationTabs();
    const lastConversationId = restoredConversationTabs.at(-1);
    return lastConversationId ? `conversation:${lastConversationId}` : 'builtin:changes';
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
  const diffCacheRef = useRef(new Map<string, ChatFileChangeDiff>());
  const diffCacheRevisionRef = useRef<string | null>(null);
  const selectedChangeIdRef = useRef<string | null>(null);
  const changesRequestRef = useRef(0);
  const activeTabIdRef = useRef(activeTabId);
  activeTabIdRef.current = activeTabId;
  const activeTab = useMemo(() => {
    for (const tab of openTabs) {
      if (tab.id === activeTabId) return tab;
    }
    return null;
  }, [openTabs, activeTabId]);
  const activeTabConversationId = activeTab && 'conversationId' in activeTab ? activeTab.conversationId : null;
  const panelConversationId = activeTab?.kind === 'conversation' ? null : (activeTabConversationId ?? conversationId);

  const openBuiltinTab = useCallback(
    (tab: RightWorkspaceTab, tabConversationId: string | null = conversationId) => {
      const id = tab === 'changes' || tab === 'attachments' ? `builtin:${tab}` : `builtin:${tab}:${tabConversationId ?? 'global'}`;
      const nextTab: BuiltinWorkspaceTab = { id, kind: 'builtin', tab, conversationId: tabConversationId };
      setOpenTabs((current) => {
        if (tab !== 'changes') return current.some((item) => item.id === id) ? current : [...current, nextTab];

        const existingIndex = current.findIndex((item) => item.kind === 'builtin' && item.tab === 'changes');
        if (existingIndex < 0) return [...current, nextTab];

        return current
          .filter((item, index) => item.kind !== 'builtin' || item.tab !== 'changes' || index === existingIndex)
          .map((item) => (item.kind === 'builtin' && item.tab === 'changes' ? nextTab : item));
      });
      setActiveTabId(id);
    },
    [conversationId],
  );

  useEffect(() => {
    setOpenTabs((current) =>
      current.map((tab) =>
        tab.kind === 'builtin' &&
        (tab.tab === 'changes' || tab.tab === 'attachments') &&
        (tab.conversationId !== conversationId || tab.id !== `builtin:${tab.tab}`)
          ? { ...tab, id: `builtin:${tab.tab}`, conversationId }
          : tab,
      ),
    );
  }, [conversationId]);

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
    if (invalidTabIds.has(activeTabId)) setActiveTabId('builtin:changes');
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
      } catch (error: unknown) {
        if (request !== changesRequestRef.current) return;
        await recordFileChangesLoadFailure('panel', error);
        if (request !== changesRequestRef.current) return;
        if (!keepError) setError('load');
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
    const targetConversationId = fileRequest.conversationId ?? conversationId;
    const existingFileTab = openTabs.find(
      (tab) =>
        tab.kind === 'file' &&
        typeof tab.path === 'string' &&
        normalizeFilePath(tab.path) === normalizeFilePath(fileRequest.path),
    );
    const tabId = existingFileTab?.id ?? `file-${fileRequest.requestId}`;
    handledFileRequestId.current = fileRequest.requestId;
    const nextFileTab: FileWorkspaceTab = {
      id: tabId,
      kind: 'file',
      conversationId: targetConversationId,
      path: fileRequest.path,
      fileName,
      content: fileRequest.content,
      language: detectFileLanguage(fileName),
      changeId: matchingChange?.id ?? null,
    };
    setOpenTabs((current) => {
      if (!existingFileTab) return [...current, nextFileTab];
      const existingIndex = current.findIndex((tab) => tab.id === existingFileTab.id);
      if (existingIndex < 0) return [...current, nextFileTab];
      return current.map((tab, index) => (index === existingIndex ? nextFileTab : tab));
    });
    setActiveTabId(tabId);
    if (matchingChange) setSelectedChangeId(matchingChange.id);
  }, [changes, conversationId, fileRequest, isLoading, loadChanges, openBuiltinTab, openTabs]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: The revision is an external signal to reload the file-change list.
  useEffect(() => {
    void loadChanges();
    return () => {
      changesRequestRef.current += 1;
    };
  }, [loadChanges, refreshRevision]);

  useEffect(() => {
    if (!panelConversationId || !selectedChangeId) {
      setSelectedDiff(null);
      setIsLoadingDiff(false);
      return;
    }
    const cacheRevision = JSON.stringify([panelConversationId, refreshRevision, diffRevision]);
    if (diffCacheRevisionRef.current !== cacheRevision) {
      diffCacheRef.current.clear();
      diffCacheRevisionRef.current = cacheRevision;
    }
    let current = true;
    const cacheKey = JSON.stringify([panelConversationId, selectedChangeId]);
    const cachedDiff = diffCacheRef.current.get(cacheKey);
    setSelectedDiff(cachedDiff ?? null);
    setIsLoadingDiff(cachedDiff === undefined);
    setError((currentError) => (currentError === 'conflict' ? currentError : null));
    if (cachedDiff !== undefined) {
      return () => {
        current = false;
      };
    }
    void readConversationFileChangeDiff(panelConversationId, selectedChangeId)
      .then((diff) => {
        diffCacheRef.current.set(cacheKey, diff);
        if (diffCacheRef.current.size > 20) {
          const oldestCacheKey = diffCacheRef.current.keys().next().value;
          if (oldestCacheKey !== undefined) diffCacheRef.current.delete(oldestCacheKey);
        }
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
      const wasApplied =
        failure instanceof LocalServiceError && failure.code === 'file_change_applied_response_unavailable';
      setError(isConflict ? 'conflict' : wasApplied ? 'applied' : 'revert');
      if (isConflict || wasApplied) void loadChanges(change.id, true);
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
      const wasApplied =
        failure instanceof LocalServiceError && failure.code === 'file_change_applied_response_unavailable';
      setError(isConflict ? 'approveConflict' : wasApplied ? 'applied' : 'revert');
      if (isConflict || wasApplied) void loadChanges(change.id, true);
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
      if (updated.some((change) => change.status === 'conflict' && !change.approved)) {
        setError('approveConflict');
      }
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
    } catch (failure) {
      const wasApplied =
        failure instanceof LocalServiceError && failure.code === 'file_change_applied_response_unavailable';
      setError(wasApplied ? 'applied' : 'revertAll');
      void loadChanges(undefined, true);
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
      const wasApplied =
        failure instanceof LocalServiceError && failure.code === 'file_change_applied_response_unavailable';
      setError(isConflict ? 'conflict' : wasApplied ? 'applied' : 'revert');
      if (isConflict || wasApplied) void loadChanges(selectedChangeId, true);
    } finally {
      setMutatingHunkIndex(null);
    }
  };

  const activeChanges = useMemo(() => changes.filter((change) => change.status === 'active'), [changes]);
  const closeWorkspaceTab = (tabId: string) => {
    const index = openTabs.findIndex((tab) => tab.id === tabId);
    const remaining = openTabs.filter((tab) => tab.id !== tabId);
    setOpenTabs(remaining);
    if (activeTabId === tabId) {
      setActiveTabId(remaining[Math.max(0, index - 1)]?.id ?? remaining[0]?.id ?? '');
    }
  };
  const pendingApprovalChanges = useMemo(
    () => activeChanges.filter((change) => !change.approved),
    [activeChanges],
  );
  const selectedChange = useMemo(
    () => {
      for (const change of changes) {
        if (change.id === selectedChangeId) return change;
      }
      return null;
    },
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
          : error === 'approveConflict'
            ? 'fileChangesApproveConflict'
            : error === 'applied'
              ? 'fileChangesAppliedRefresh'
              : error === 'revertAll'
                ? 'fileChangesRevertAllFailed'
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
                <Button
                  aria-controls={`${workspaceId}-panel`}
                  aria-selected={selected}
                  className={`workspace-tab-select min-h-0 h-full min-w-0 justify-start gap-2 rounded-control px-2 text-xs font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
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
                  size="small"
                  tabIndex={selected ? 0 : -1}
                  type="button"
                  variant="ghost"
                >
                  {tab.kind === 'file' ? (
                    <FileTypeIcon path={tab.path} size={14} />
                  ) : tab.kind === 'conversation' ? (
                    <MessageSquare aria-hidden="true" className="shrink-0 text-accent" size={14} />
                  ) : tab.tab === 'changes' ? (
                    <FileDiff aria-hidden="true" className="shrink-0 text-accent" size={14} />
                  ) : tab.tab === 'browser' ? (
                    <Globe aria-hidden="true" className="shrink-0 text-accent" size={14} />
                  ) : tab.tab === 'attachments' ? (
                    <Paperclip aria-hidden="true" className="shrink-0 text-accent" size={14} />
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
                </Button>
                <Button
                  aria-label={t('workspaceCloseTab', { name: label })}
                  className="workspace-tab-close size-6 shrink-0 rounded-control p-0 text-secondary-icon hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  onClick={() => closeWorkspaceTab(tab.id)}
                  size="icon"
                  title={t('workspaceCloseTab', { name: label })}
                  type="button"
                  variant="ghost"
                >
                  <X aria-hidden="true" size={12} />
                </Button>
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
            <DropdownMenuItem onSelect={() => openBuiltinTab('attachments', activeTabConversationId ?? conversationId)}>
              <Paperclip aria-hidden="true" size={15} />
              <span>{t('conversationAttachmentsTitle')}</span>
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
            <div className="flex min-h-0 flex-1 flex-col">
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
        ) : activeTab.tab === 'attachments' ? (
          <ConversationAttachmentsPanel conversationId={panelConversationId} />
        ) : (
          <>
            <div className="flex min-h-10 shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border px-3 py-1.5 text-xs">
              <div className="flex min-w-0 items-center gap-2">
                <span className="truncate text-text">{t('fileChangesSummary', { count: activeChanges.length })}</span>
                <span className="shrink-0 font-semibold text-success">+{added}</span>
                <span className="shrink-0 font-semibold text-destructive">-{removed}</span>
              </div>
              <div className="flex items-center gap-1">
                <Button
                  aria-label={t('fileChangesViewUnified')}
                  aria-pressed={diffViewMode === 'unified'}
                  className={`h-7 w-7 min-h-0 p-0 transition-colors ${
                    diffViewMode === 'unified'
                      ? 'bg-accent text-white'
                      : 'bg-raised text-secondary-text hover:text-text'
                  }`}
                  onClick={() => setDiffViewMode('unified')}
                  size="small"
                  title={t('fileChangesViewUnified')}
                  type="button"
                  variant="ghost"
                >
                  <Rows3 aria-hidden="true" size={14} />
                </Button>
                <Button
                  aria-label={t('fileChangesViewSplit')}
                  aria-pressed={diffViewMode === 'split'}
                  className={`h-7 w-7 min-h-0 p-0 transition-colors ${
                    diffViewMode === 'split'
                      ? 'bg-accent text-white'
                      : 'bg-raised text-secondary-text hover:text-text'
                  }`}
                  onClick={() => setDiffViewMode('split')}
                  size="small"
                  title={t('fileChangesViewSplit')}
                  type="button"
                  variant="ghost"
                >
                  <Columns2 aria-hidden="true" size={14} />
                </Button>
                {pendingApprovalChanges.length > 0 ? (
                  <Fragment>
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
                  </Fragment>
                ) : null}
              </div>
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
                  className="chat-file-changes-list min-h-0 flex-1 overflow-y-auto py-1"
                >
                  {changes.map((change) => (
                    <Fragment key={change.id}>
                      <FileChangeRow
                        change={change}
                        isAnyMutating={mutatingChangeId !== null}
                        isMutating={mutatingChangeId === change.id}
                        isSelected={selectedChangeId === change.id}
                        onApprove={() => void approveChange(change)}
                        onRevert={() => void revertChange(change)}
                        onSelect={() => {
                          setError(null);
                          setSelectedChangeId((current) => (current === change.id ? null : change.id));
                        }}
                      />
                      {selectedChangeId === change.id ? (
                        <li aria-label={selectedDiff?.path ?? change.path} className="chat-file-change-inline-diff">
                          <div className="chat-file-change-inline-diff-content">
                            <DiffContent
                              canRevertHunks={
                                selectedChange !== null &&
                                selectedChange.id === change.id &&
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
                          </div>
                        </li>
                      ) : null}
                    </Fragment>
                  ))}
                </ul>
              </div>
            )}
          </>
        )}
      </div>
    </aside>
  );
}

function builtinTabLabelKey(
  tabId: RightWorkspaceTab,
): 'fileChangesTitle' | 'browserPanelTitle' | 'subagentsPanelTitle' | 'conversationAttachmentsTitle' {
  if (tabId === 'changes') return 'fileChangesTitle';
  if (tabId === 'browser') return 'browserPanelTitle';
  if (tabId === 'attachments') return 'conversationAttachmentsTitle';
  return 'subagentsPanelTitle';
}

function normalizeFilePath(path: string): string {
  return path.replaceAll('\\', '/').replace(/\/+$/, '').toLocaleLowerCase();
}

function firstChangeId(changes: ChatFileChange[]): string | null {
  return (
    changes.find((change) => change.status === 'active' && !change.approved)?.id ??
    changes.find((change) => change.status === 'active')?.id ??
    changes[0]?.id ??
    null
  );
}
