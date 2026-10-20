import { Archive, Clock3, MessagesSquare, Pin, Plus, RefreshCw } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog } from '@/design-system';
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
}

export function ConversationSidebar({
  isDrawer = false,
  isModal = false,
  sectionFilter = 'all',
  onCloseMobile,
  onRequestTemporaryExit,
  onNewChat,
  onStartTemporaryChat,
}: ConversationSidebarProps) {
  const { t } = useTranslation();
  const [createProjectOpen, setCreateProjectOpen] = useState(false);
  const [projectOptionsTarget, setProjectOptionsTarget] = useState<ChatProject | null>(null);
  const [editTarget, setEditTarget] = useState<{ conversation: ChatConversation; mode: 'rename' | 'tags' } | null>(
    null,
  );
  const [deleteTarget, setDeleteTarget] = useState<ChatConversation | null>(null);
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
  const temporaryChat = useChatStore((state) => state.temporaryChat);
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

  const visiblePinned = pinnedConversations;
  const visibleChats = unassignedConversations;
  const visibleArchived = archivedConversations;
  const filteredProjectConversations = visibleProjectConversations;
  const filteredProjects = visibleProjects;

  const recencyBuckets = useMemo(() => groupConversationsByRecency(visibleChats), [visibleChats]);

  const {
    archiveConversation,
    createProject,
    deleteConversation,
    exportConversation,
    feedback,
    isBusy,
    moveConversation,
    saveTags,
    saveTitle,
    showFeedback,
    toggleBookmark,
    togglePinned,
  } = useConversationSidebarActions();

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
        if (temporaryChat) {
          startNewChat();
        } else {
          startTemporaryChat();
        }
        onCloseMobile?.();
      });
    }
  };

  const renderRows = (
    rows: readonly ChatConversation[],
    options: { archived: boolean; currentProjectId: string | null; allowMove: boolean },
  ) =>
    rows.map((conversation) => (
      <ConversationSidebarRow
        active={activeConversationId === conversation.id}
        allowMove={options.allowMove}
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
        projects={projects}
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
            <Tooltip label={temporaryChat ? t('temporaryChatEnd') : t('temporaryChatStart')}>
              <Button
                aria-label={temporaryChat ? t('temporaryChatEnd') : t('temporaryChatStart')}
                className={`size-8 shrink-0 transition-colors ${
                  temporaryChat
                    ? 'bg-accent/15 text-accent hover:bg-accent/25'
                    : 'text-secondary-icon hover:text-text'
                }`}
                onClick={handleStartTemporaryChat}
                size="icon"
                variant="ghost"
              >
                <Clock3 aria-hidden="true" size={16} />
              </Button>
            </Tooltip>
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
                    projects={filteredProjects}
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
                                allowMove: true,
                              })}
                            </ul>
                          </li>
                        ))
                      : renderRows(visibleChats, {
                          archived: false,
                          currentProjectId: null,
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
