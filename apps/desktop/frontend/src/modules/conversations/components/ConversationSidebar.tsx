import { Archive, CheckSquare, Clock3, Folder, MessagesSquare, Pin, Plus, RefreshCw, Search, X } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Select } from '@/design-system';
import { Tooltip } from '@/design-system/Tooltip';
import type { ChatConversation, ChatProject } from '@/modules/conversations/data/chatStorage';
import {
  readCollapsedSidebarSections,
  type SidebarSectionId,
  writeCollapsedSidebarSections,
} from '@/modules/conversations/data/sidebarPreferences';
import { useConversationSidebarActions } from '@/modules/conversations/hooks/useConversationSidebarActions';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { CreateProjectDialog, ProjectOptionsDialog, ProjectSidebarSection } from '@/modules/projects/components';
import { discardProjectFolderSelection, selectProjectFolder } from '@/platform/tauri';
import { ConversationSidebarEditDialog } from './ConversationSidebarEditDialog';
import { ConversationSidebarRow } from './ConversationSidebarRow';
import { ConversationSidebarSection } from './ConversationSidebarSection';

export type RecencyBucketKey = 'today' | 'yesterday' | 'previous7Days' | 'previous30Days' | 'older';

export interface RecencyBucket {
  id: RecencyBucketKey;
  labelKey: RecencyBucketKey;
  conversations: ChatConversation[];
}

export function groupConversationsByRecency(
  conversations: readonly ChatConversation[],
  now = Date.now(),
): RecencyBucket[] {
  const nowDate = new Date(now);
  const todayStart = new Date(nowDate.getFullYear(), nowDate.getMonth(), nowDate.getDate()).getTime();
  const yesterdayStart = todayStart - 86_400_000;
  const sevenDaysAgoStart = todayStart - 6 * 86_400_000;
  const thirtyDaysAgoStart = todayStart - 29 * 86_400_000;

  const today: ChatConversation[] = [];
  const yesterday: ChatConversation[] = [];
  const previous7Days: ChatConversation[] = [];
  const previous30Days: ChatConversation[] = [];
  const older: ChatConversation[] = [];

  const sorted = [...conversations].sort((a, b) => b.updatedAt - a.updatedAt);

  for (const item of sorted) {
    if (item.updatedAt >= todayStart) {
      today.push(item);
    } else if (item.updatedAt >= yesterdayStart) {
      yesterday.push(item);
    } else if (item.updatedAt >= sevenDaysAgoStart) {
      previous7Days.push(item);
    } else if (item.updatedAt >= thirtyDaysAgoStart) {
      previous30Days.push(item);
    } else {
      older.push(item);
    }
  }

  const buckets: RecencyBucket[] = [];
  if (today.length > 0) buckets.push({ id: 'today', labelKey: 'today', conversations: today });
  if (yesterday.length > 0) buckets.push({ id: 'yesterday', labelKey: 'yesterday', conversations: yesterday });
  if (previous7Days.length > 0) {
    buckets.push({ id: 'previous7Days', labelKey: 'previous7Days', conversations: previous7Days });
  }
  if (previous30Days.length > 0) {
    buckets.push({ id: 'previous30Days', labelKey: 'previous30Days', conversations: previous30Days });
  }
  if (older.length > 0) buckets.push({ id: 'older', labelKey: 'older', conversations: older });

  return buckets;
}

export interface ConversationSidebarProps {
  isDrawer?: boolean;
  isModal?: boolean;
  sectionFilter?: 'all' | 'chats' | 'projects';
  onCloseMobile?(): void;
  onRequestTemporaryExit(action: () => void | Promise<void>): void;
  onNewChat?(): void;
  onStartTemporaryChat?(): void;
  onOpenSearch?(): void;
}

export function ConversationSidebar({
  isDrawer = false,
  isModal = false,
  sectionFilter = 'all',
  onCloseMobile,
  onRequestTemporaryExit,
  onNewChat,
  onStartTemporaryChat,
  onOpenSearch,
}: ConversationSidebarProps) {
  const { t } = useTranslation();
  const [createProjectOpen, setCreateProjectOpen] = useState(false);
  const [projectOptionsTarget, setProjectOptionsTarget] = useState<ChatProject | null>(null);
  const [editTarget, setEditTarget] = useState<{ conversation: ChatConversation; mode: 'rename' | 'tags' } | null>(
    null,
  );
  const [deleteTarget, setDeleteTarget] = useState<ChatConversation | null>(null);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [moveTarget, setMoveTarget] = useState('__chats__');
  const [searchQuery, setSearchQuery] = useState('');
  const [sidebarPreferences] = useState(readCollapsedSidebarSections);
  const [collapsedSections, setCollapsedSections] = useState<ReadonlySet<SidebarSectionId>>(
    () => sidebarPreferences.collapsedSections,
  );
  const [isListScrolled, setIsListScrolled] = useState(false);
  const drawerRef = useRef<HTMLElement | null>(null);

  const conversations = useChatStore((state) => state.conversations);
  const projects = useChatStore((state) => state.projects);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const selectConversation = useChatStore((state) => state.selectConversation);
  const startNewChat = useChatStore((state) => state.startNewChat);
  const startTemporaryChat = useChatStore((state) => state.startTemporaryChat);
  const refreshProjects = useChatStore((state) => state.refreshProjects);

  useEffect(() => {
    if (sidebarPreferences.error) globalThis.reportError(sidebarPreferences.error);
  }, [sidebarPreferences.error]);

  const activeConversations = useMemo(
    () => conversations.filter((conversation) => !conversation.isArchived),
    [conversations],
  );
  const archivedConversations = useMemo(
    () => conversations.filter((conversation) => conversation.isArchived),
    [conversations],
  );
  const pinnedConversations = useMemo(
    () => activeConversations.filter((conversation) => conversation.isPinned),
    [activeConversations],
  );
  const unassignedConversations = useMemo(
    () => activeConversations.filter((conversation) => conversation.projectId === null && !conversation.isPinned),
    [activeConversations],
  );
  const visibleProjects = projects;
  const visibleProjectConversations = useMemo(
    () =>
      activeConversations.filter((conversation) => {
        return conversation.projectId !== null && !conversation.isPinned;
      }),
    [activeConversations],
  );

  const normalizedQuery = searchQuery.trim().toLocaleLowerCase();
  const isSearching = normalizedQuery.length > 0;

  const matchesSearch = (item: ChatConversation) =>
    item.title.toLocaleLowerCase().includes(normalizedQuery) ||
    item.tags.some((tag) => tag.toLocaleLowerCase().includes(normalizedQuery));

  const visiblePinned = useMemo(
    () => (isSearching ? pinnedConversations.filter(matchesSearch) : pinnedConversations),
    [isSearching, pinnedConversations, normalizedQuery],
  );

  const visibleChats = useMemo(
    () => (isSearching ? unassignedConversations.filter(matchesSearch) : unassignedConversations),
    [isSearching, unassignedConversations, normalizedQuery],
  );

  const visibleArchived = useMemo(
    () => (isSearching ? archivedConversations.filter(matchesSearch) : archivedConversations),
    [isSearching, archivedConversations, normalizedQuery],
  );

  const filteredProjectConversations = useMemo(
    () => (isSearching ? visibleProjectConversations.filter(matchesSearch) : visibleProjectConversations),
    [isSearching, visibleProjectConversations, normalizedQuery],
  );

  const filteredProjects = useMemo(() => {
    if (!isSearching) return visibleProjects;
    return visibleProjects.filter(
      (project) =>
        project.name.toLocaleLowerCase().includes(normalizedQuery) ||
        filteredProjectConversations.some((conversation) => conversation.projectId === project.id),
    );
  }, [isSearching, visibleProjects, normalizedQuery, filteredProjectConversations]);

  const recencyBuckets = useMemo(() => groupConversationsByRecency(visibleChats), [visibleChats]);

  const totalSearchResultsCount =
    visiblePinned.length +
    visibleChats.length +
    visibleArchived.length +
    filteredProjectConversations.length;

  const toggleSelection = (conversationId: string) => {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(conversationId)) next.delete(conversationId);
      else next.add(conversationId);
      return next;
    });
  };

  const closeSelection = () => {
    setSelectionMode(false);
    setSelectedIds(new Set());
  };

  const {
    archiveConversation,
    archiveSelected,
    createProject,
    deleteConversation,
    exportConversation,
    feedback,
    isBusy,
    moveConversation,
    moveSelected,
    saveTags,
    saveTitle,
    showFeedback,
    toggleBookmark,
    togglePinned,
  } = useConversationSidebarActions({ closeSelection, selectedIds });

  const confirmDelete = async () => {
    if (!deleteTarget) return;
    if (await deleteConversation(deleteTarget)) setDeleteTarget(null);
  };

  const dropConversationToProject = (conversationId: string, projectId: string) => {
    const conversation = activeConversations.find((item) => item.id === conversationId);
    if (conversation && conversation.projectId !== projectId) {
      void moveConversation(conversation, projectId);
    }
  };

  const dropConversationToChats = (conversationId: string) => {
    const conversation = activeConversations.find((item) => item.id === conversationId);
    if (conversation && (conversation.projectId !== null || conversation.isPinned)) {
      void moveConversation(conversation, null);
    }
  };

  const dropConversationToPinned = (conversationId: string) => {
    const conversation = activeConversations.find((item) => item.id === conversationId);
    if (conversation && !conversation.isPinned) void togglePinned(conversation);
  };

  const requestSelectConversation = (conversationId: string, targetMessageId?: string | null) => {
    onRequestTemporaryExit(async () => {
      await selectConversation(conversationId, targetMessageId);
      onCloseMobile?.();
    });
  };

  const handleNewChat = () => {
    if (onNewChat) {
      onNewChat();
    } else {
      onRequestTemporaryExit(() => {
        startNewChat();
        onCloseMobile?.();
      });
    }
  };

  const handleStartTemporaryChat = () => {
    if (onStartTemporaryChat) {
      onStartTemporaryChat();
    } else {
      onRequestTemporaryExit(() => {
        startTemporaryChat();
        onCloseMobile?.();
      });
    }
  };

  const renderRows = (
    rows: readonly ChatConversation[],
    options: { archived: boolean; currentProjectId: string | null; allowSelection: boolean; allowMove: boolean },
  ) =>
    rows.map((conversation) => (
      <ConversationSidebarRow
        active={activeConversationId === conversation.id}
        allowMove={options.allowMove}
        allowSelection={options.allowSelection}
        archived={options.archived}
        conversation={conversation}
        currentProjectId={conversation.projectId ?? options.currentProjectId}
        disabled={isBusy}
        key={conversation.id}
        onArchive={() => void archiveConversation(conversation)}
        onDelete={() => setDeleteTarget(conversation)}
        onEditTags={() => setEditTarget({ conversation, mode: 'tags' })}
        onExport={(format) => void exportConversation(conversation, format)}
        onMoveToChats={() => void moveConversation(conversation, null)}
        onMoveToProject={(projectId) => void moveConversation(conversation, projectId)}
        onRename={() => setEditTarget({ conversation, mode: 'rename' })}
        onSelect={() => requestSelectConversation(conversation.id)}
        onToggleBookmark={() => void toggleBookmark(conversation)}
        onTogglePinned={() => void togglePinned(conversation)}
        onToggleSelection={() => toggleSelection(conversation.id)}
        projects={projects}
        selectedForBatch={selectedIds.has(conversation.id)}
        selectionMode={selectionMode && options.allowSelection}
      />
    ));

  const toggleSection = (section: SidebarSectionId) => {
    const next = new Set(collapsedSections);
    if (next.has(section)) next.delete(section);
    else next.add(section);
    setCollapsedSections(next);
    try {
      writeCollapsedSidebarSections(next);
    } catch (cause) {
      globalThis.reportError(new Error('Sidebar section preferences could not be saved.', { cause }));
    }
  };

  const sectionIsVisible = (items: readonly ChatConversation[], showEmpty: boolean) => items.length > 0 || showEmpty;

  useEffect(() => {
    if (!isDrawer) return;
    const previouslyFocused =
      document.activeElement instanceof HTMLElement && document.activeElement !== document.body
        ? document.activeElement
        : null;
    const focusFrame = window.requestAnimationFrame(() => {
      const drawer = drawerRef.current;
      if (!drawer) return;
      const closeButton = drawer.querySelector<HTMLElement>('[data-sidebar-close]');
      const firstFocusable = getFocusableElements(drawer)[0];
      (closeButton ?? firstFocusable ?? drawer).focus();
    });
    return () => {
      window.cancelAnimationFrame(focusFrame);
      if (previouslyFocused?.isConnected) previouslyFocused.focus();
    };
  }, [isDrawer]);

  useEffect(() => {
    if (!isDrawer) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      onCloseMobile?.();
    };
    window.addEventListener('keydown', closeOnEscape);
    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [isDrawer, onCloseMobile]);

  useEffect(() => {
    if (!isModal) return;
    const drawer = drawerRef.current;
    if (!drawer) return;
    const keepFocusInside = (event: KeyboardEvent) => {
      if (event.key !== 'Tab') return;
      const focusScope = drawer.querySelector<HTMLElement>('dialog[open]') ?? drawer;
      const focusable = getFocusableElements(focusScope);
      if (focusable.length === 0) {
        event.preventDefault();
        focusScope.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (first === undefined || last === undefined) {
        event.preventDefault();
        focusScope.focus();
        return;
      }
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    drawer.addEventListener('keydown', keepFocusInside);
    return () => drawer.removeEventListener('keydown', keepFocusInside);
  }, [isModal]);

  const showPinned =
    (sectionFilter === 'all' || sectionFilter === 'chats') && sectionIsVisible(visiblePinned, false);
  const showProjects = sectionFilter === 'all' || sectionFilter === 'projects';
  const showChats = (sectionFilter === 'all' || sectionFilter === 'chats') && sectionIsVisible(visibleChats, true);
  const showArchived =
    (sectionFilter === 'all' || sectionFilter === 'chats') && sectionIsVisible(visibleArchived, false);

  return (
    <>
      {/* biome-ignore lint/a11y/useAriaPropsSupportedByRole: aria-modal is present only when this section becomes the modal drawer dialog. */}
      <section
        aria-label={t(sectionFilter === 'projects' ? 'projects' : 'chats')}
        aria-modal={isModal ? true : undefined}
        className={`conversation-sidebar app-content-divider flex h-full shrink-0 flex-col border-y-0 border-r bg-navigation ${isDrawer ? 'absolute inset-y-0 left-0 z-30 shadow-xl' : 'relative'}`}
        id="chat-workspace-panel"
        ref={drawerRef}
        role={isDrawer ? 'dialog' : 'complementary'}
        tabIndex={isDrawer ? -1 : undefined}
      >
        <div
          className={`sticky top-0 z-10 flex min-h-12 items-center justify-between gap-2 bg-navigation px-3 ${isListScrolled ? 'app-content-divider border-b' : ''}`}
        >
          <div className="flex min-w-0 items-center gap-2">
            <MessagesSquare aria-hidden="true" className="shrink-0 text-secondary-icon" size={17} />
            <h2 className="truncate text-sm font-semibold text-text">
              {t(sectionFilter === 'projects' ? 'projects' : 'chats')}
            </h2>
          </div>
          <div className="flex items-center gap-0.5">
            <Tooltip label={t('newChat')}>
              <Button
                aria-label={t('newChat')}
                className="size-8 shrink-0 text-secondary-icon hover:text-text"
                onClick={handleNewChat}
                size="icon"
                variant="ghost"
              >
                <Plus aria-hidden="true" size={16} />
              </Button>
            </Tooltip>
            <Tooltip label={t('temporaryChatStart')}>
              <Button
                aria-label={t('temporaryChatStart')}
                className="size-8 shrink-0 text-secondary-icon hover:text-text"
                onClick={handleStartTemporaryChat}
                size="icon"
                variant="ghost"
              >
                <Clock3 aria-hidden="true" size={16} />
              </Button>
            </Tooltip>
            <Tooltip label={t('moveSelectedChats')}>
              <Button
                aria-label={t('moveSelectedChats')}
                aria-pressed={selectionMode}
                className={`size-8 shrink-0 ${selectionMode ? 'bg-selected text-accent' : 'text-secondary-icon hover:text-text'}`}
                onClick={() => setSelectionMode((prev) => !prev)}
                size="icon"
                variant="ghost"
              >
                <CheckSquare aria-hidden="true" size={16} />
              </Button>
            </Tooltip>
            {onCloseMobile ? (
              <Tooltip label={t('close')}>
                <Button
                  aria-label={t('close')}
                  className="size-8 shrink-0 text-secondary-icon hover:text-text"
                  data-sidebar-close="true"
                  onClick={onCloseMobile}
                  size="icon"
                  variant="ghost"
                >
                  <X aria-hidden="true" size={16} />
                </Button>
              </Tooltip>
            ) : null}
          </div>
        </div>

        <div className="px-3 pt-1 pb-2">
          <div className="relative flex items-center">
            <Search
              aria-hidden="true"
              className="pointer-events-none absolute left-2.5 text-secondary-icon"
              size={14}
            />
            <input
              aria-label={t('searchChats')}
              className="h-8 w-full rounded-button border border-border bg-background pl-8 pr-7 text-xs text-text placeholder:text-secondary-text focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent"
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder={t('searchChats')}
              type="text"
              value={searchQuery}
            />
            {searchQuery ? (
              <button
                aria-label={t('settingsSearchClear')}
                className="absolute right-2 text-secondary-icon hover:text-text focus:outline-none"
                onClick={() => setSearchQuery('')}
                type="button"
              >
                <X aria-hidden="true" size={13} />
              </button>
            ) : null}
          </div>
        </div>

        <div
          className="openchat-scroll-area flex-1 px-2 py-2"
          onScroll={(event) => setIsListScrolled(event.currentTarget.scrollTop > 0)}
        >
          <div className="grid gap-3">
            {feedback ? (
              <p
                className={`px-2 text-xs ${feedback.success ? 'text-success' : 'text-destructive'}`}
                role={feedback.success ? 'status' : 'alert'}
              >
                {t(feedback.key)}
              </p>
            ) : null}
            {serviceStatus === 'error' ? (
              <div className="flex items-center justify-between gap-2 rounded-button px-2 py-1">
                <p className="text-xs text-destructive" role="alert">
                  {t('chatHistoryUnavailable')}
                </p>
                <Button
                  aria-label={t('retry')}
                  className="size-8"
                  onClick={() => void useChatStore.getState().initialize()}
                  size="icon"
                  variant="ghost"
                >
                  <RefreshCw aria-hidden="true" size={15} />
                </Button>
              </div>
            ) : null}
            {serviceStatus === 'idle' || serviceStatus === 'loading' ? (
              <p aria-live="polite" className="px-3 py-3 text-sm text-secondary-text">
                {t('historyLoading')}
              </p>
            ) : null}
            {serviceStatus === 'ready' ? (
              <>
                {selectionMode ? (
                  <div className="grid gap-2 rounded-card border border-border bg-raised p-2">
                    <Select
                      label={t('moveSelectedChats')}
                      onChange={(event) => setMoveTarget(event.target.value)}
                      options={[
                        { value: '__chats__', label: t('moveToChats') },
                        ...projects.map((project) => ({ value: project.id, label: project.name })),
                      ]}
                      value={moveTarget}
                      visuallyHiddenLabel
                    />
                    <div className="flex gap-2">
                      <Button
                        className="min-w-0 flex-1"
                        disabled={selectedIds.size === 0 || isBusy}
                        onClick={() => void moveSelected(moveTarget)}
                        size="small"
                      >
                        <Folder aria-hidden="true" size={14} />
                        {t('moveSelectedChats')}
                      </Button>
                      <Button
                        className="min-w-0 flex-1"
                        disabled={selectedIds.size === 0 || isBusy}
                        onClick={() => void archiveSelected()}
                        size="small"
                      >
                        <Archive aria-hidden="true" size={14} />
                        {t('archiveSelectedConversations')}
                      </Button>
                    </div>
                  </div>
                ) : null}

                {isSearching && totalSearchResultsCount === 0 ? (
                  <div className="grid gap-2 px-3 py-6 text-center">
                    <p className="text-xs text-secondary-text">{t('searchMessagesNoResults')}</p>
                    {onOpenSearch ? (
                      <Button className="mx-auto" onClick={onOpenSearch} size="small" variant="ghost">
                        <Search aria-hidden="true" size={14} />
                        <span>{t('searchChats')}</span>
                      </Button>
                    ) : null}
                  </div>
                ) : null}

                {showPinned ? (
                  <ConversationSidebarSection
                    collapsed={collapsedSections.has('pinned')}
                    dropIndicator={Pin}
                    icon={Pin}
                    onDropConversation={dropConversationToPinned}
                    onToggle={() => toggleSection('pinned')}
                    title={t('pinnedChats')}
                  >
                    {renderRows(visiblePinned, {
                      archived: false,
                      currentProjectId: null,
                      allowSelection: true,
                      allowMove: true,
                    })}
                  </ConversationSidebarSection>
                ) : null}

                {showProjects ? (
                  <ProjectSidebarSection
                    activeConversationId={activeConversationId}
                    collapsed={collapsedSections.has('projects')}
                    conversations={filteredProjectConversations}
                    isBusy={isBusy}
                    onArchive={(conversation) => void archiveConversation(conversation)}
                    onCreateProject={() => setCreateProjectOpen(true)}
                    onCreateProjectChat={(projectId) =>
                      onRequestTemporaryExit(() => {
                        startNewChat(projectId);
                        onCloseMobile?.();
                      })
                    }
                    onDelete={(conversation) => setDeleteTarget(conversation)}
                    onDropConversationToProject={dropConversationToProject}
                    onEditTags={(conversation) => setEditTarget({ conversation, mode: 'tags' })}
                    onExport={(conversation, format) => void exportConversation(conversation, format)}
                    onMoveToChats={(conversation) => void moveConversation(conversation, null)}
                    onMoveToProject={(conversation, projectId) => void moveConversation(conversation, projectId)}
                    onOpenOptions={setProjectOptionsTarget}
                    onRename={(conversation) => setEditTarget({ conversation, mode: 'rename' })}
                    onRetry={() => void refreshProjects().catch(() => showFeedback('projectLoadFailed'))}
                    onSelectConversation={(id) => requestSelectConversation(id)}
                    onToggleBookmark={(conversation) => void toggleBookmark(conversation)}
                    onToggleCollapsed={() => toggleSection('projects')}
                    onTogglePinned={(conversation) => void togglePinned(conversation)}
                    onToggleSelection={toggleSelection}
                    projects={filteredProjects}
                    selectedConversationIds={selectedIds}
                    selectionMode={selectionMode}
                    status={projectsStatus}
                  />
                ) : null}

                {showChats ? (
                  <ConversationSidebarSection
                    collapsed={collapsedSections.has('chats')}
                    dropIndicator={MessagesSquare}
                    emptyMessage={visibleChats.length === 0 ? t('noChatsTitle') : undefined}
                    icon={MessagesSquare}
                    onDropConversation={dropConversationToChats}
                    onToggle={() => toggleSection('chats')}
                    title={t('chats')}
                  >
                    {recencyBuckets.length > 0
                      ? recencyBuckets.map((bucket) => (
                          <li className="grid gap-0.5" key={bucket.id}>
                            <div className="px-2 pt-2 pb-0.5 text-[11px] font-semibold tracking-wider text-secondary-text uppercase">
                              {t(bucket.labelKey)}
                            </div>
                            <ul className="grid gap-0.5">
                              {renderRows(bucket.conversations, {
                                archived: false,
                                currentProjectId: null,
                                allowSelection: true,
                                allowMove: true,
                              })}
                            </ul>
                          </li>
                        ))
                      : renderRows(visibleChats, {
                          archived: false,
                          currentProjectId: null,
                          allowSelection: true,
                          allowMove: true,
                        })}
                  </ConversationSidebarSection>
                ) : null}

                {showArchived ? (
                  <ConversationSidebarSection
                    collapsed={collapsedSections.has('archived')}
                    icon={Archive}
                    onToggle={() => toggleSection('archived')}
                    title={t('archivedChats')}
                  >
                    {renderRows(visibleArchived, {
                      archived: true,
                      currentProjectId: null,
                      allowSelection: false,
                      allowMove: false,
                    })}
                  </ConversationSidebarSection>
                ) : null}
              </>
            ) : null}
          </div>
        </div>

        <CreateProjectDialog onCreate={createProject} onOpenChange={setCreateProjectOpen} open={createProjectOpen} />
        <ProjectOptionsDialog
          onOpenChange={(open) => {
            if (!open) setProjectOptionsTarget(null);
          }}
          onUseWorktree={async (path, branch) => {
            onRequestTemporaryExit(async () => {
              if (!projectOptionsTarget) throw new Error('The selected project is unavailable.');
              const existing = useChatStore
                .getState()
                .projects.find((project) => normalizeProjectPath(project.folderPath) === normalizeProjectPath(path));
              if (existing) {
                startNewChat(existing.id);
                return;
              }
              const folder = await selectProjectFolder({ title: t('chooseProjectFolder'), initialPath: path });
              if (!folder) return;
              try {
                await createProject(`${projectOptionsTarget.name} (${branch})`, folder.selectionId);
              } finally {
                await discardProjectFolderSelection(folder.selectionId);
              }
              const created = useChatStore
                .getState()
                .projects.find(
                  (project) => normalizeProjectPath(project.folderPath) === normalizeProjectPath(folder.folderPath),
                );
              if (!created) throw new Error('The worktree project could not be loaded.');
              startNewChat(created.id);
            });
          }}
          open={projectOptionsTarget !== null}
          project={projectOptionsTarget}
        />
        <ConversationSidebarEditDialog
          conversation={editTarget?.conversation ?? null}
          mode={editTarget?.mode ?? null}
          onOpenChange={(open) => {
            if (!open) setEditTarget(null);
          }}
          onSaveTags={saveTags}
          onSaveTitle={saveTitle}
          open={editTarget !== null}
        />
        <Dialog
          onOpenChange={(open) => {
            if (!open) setDeleteTarget(null);
          }}
          open={deleteTarget !== null}
          title={t('confirmDeleteConversationTitle')}
        >
          {deleteTarget ? (
            <div className="grid gap-5">
              <p className="text-sm text-secondary-text">
                {t('confirmDeleteConversation', { title: deleteTarget.title })}
              </p>
              <div className="flex justify-end gap-2">
                <Button disabled={isBusy} onClick={() => setDeleteTarget(null)}>
                  {t('cancel')}
                </Button>
                <Button disabled={isBusy} onClick={() => void confirmDelete()} variant="destructive">
                  {t('deleteConversation')}
                </Button>
              </div>
            </div>
          ) : null}
        </Dialog>
      </section>
    </>
  );
}

function normalizeProjectPath(path: string): string {
  return path.replace(/[\\/]+$/, '').toLocaleLowerCase('en-US');
}

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return [
    ...container.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ].filter((element) => element.getAttribute('aria-hidden') !== 'true' && element.getClientRects().length > 0);
}
