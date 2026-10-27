import {
  Archive,
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  Clock3,
  Folder,
  FolderOpen,
  FolderPlus,
  Menu,
  MessageSquare,
  Pencil,
  Pin,
  Plus,
  RefreshCw,
  SlidersHorizontal,
  SquarePen,
  Trash2,
  X,
} from 'lucide-react';
import {
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';

import {
  Button,
  Dialog,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
  Input,
} from '@/design-system';
import type { ChatConversation, ChatProject } from '@/modules/conversations/data/chatStorage';
import {
  snapshotAssistantProfile,
  type AssistantAgentSnapshot,
} from '@/modules/settings/data/assistantProfiles';
import { useAssistantProfilesStore } from '@/modules/settings/stores/assistantProfilesStore';
import {
  type CustomSidebarFolder,
  clampSidebarWidth,
  readCollapsedSidebarSections,
  readCustomSidebarFolders,
  readSidebarManualOrder,
  readSidebarSortMode,
  readSidebarViewMode,
  readSidebarWidth,
  readLegacyHiddenConversationIds,
  SIDEBAR_DEFAULT_WIDTH,
  SIDEBAR_MAX_WIDTH,
  SIDEBAR_MIN_WIDTH,
  type SidebarSectionId,
  type SidebarSortMode,
  type SidebarViewMode,
  sortSidebarConversations,
  writeCollapsedSidebarSections,
  writeCustomSidebarFolders,
  writeSidebarManualOrder,
  writeSidebarSortMode,
  writeSidebarViewMode,
  writeSidebarWidth,
} from '@/modules/conversations/data/sidebarPreferences';
import { useConversationSidebarActions } from '@/modules/conversations/hooks/useConversationSidebarActions';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { CreateProjectDialog, ProjectOptionsDialog, ProjectSidebarSection } from '@/modules/projects/components';
import { discardProjectFolderSelection, selectProjectFolder } from '@/platform/tauri';
import { ConversationSidebarEditDialog } from './ConversationSidebarEditDialog';
import { ConversationSidebarRow } from './ConversationSidebarRow';
import { ConversationSidebarSection } from './ConversationSidebarSection';

const SIDEBAR_PAGE_SIZE = 50;
const SIDEBAR_COMPACT_WIDTH = 256;

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
  onNewAgentChat?(agent: AssistantAgentSnapshot): void;
  onManageAgents?(): void;
  onStartTemporaryChat?(): void;
}

export function ConversationSidebar({
  isDrawer = false,
  isModal = false,
  sectionFilter = 'all',
  onCloseMobile,
  onRequestTemporaryExit,
  onNewChat,
  onNewAgentChat,
  onManageAgents,
  onStartTemporaryChat,
}: ConversationSidebarProps) {
  const { t } = useTranslation();
  const [createProjectOpen, setCreateProjectOpen] = useState(false);
  const [projectOptionsTarget, setProjectOptionsTarget] = useState<ChatProject | null>(null);
  const [editTarget, setEditTarget] = useState<{ conversation: ChatConversation; mode: 'rename' } | null>(
    null,
  );
  const [deleteTarget, setDeleteTarget] = useState<ChatConversation | null>(null);
  const [sidebarPreferences] = useState(readCollapsedSidebarSections);
  const [collapsedSections, setCollapsedSections] = useState<ReadonlySet<SidebarSectionId>>(
    () => sidebarPreferences.collapsedSections,
  );
  const [sidebarWidth, setSidebarWidth] = useState(readSidebarWidth);
  const [isResizing, setIsResizing] = useState(false);
  const [sortMode, setSortMode] = useState<SidebarSortMode>(readSidebarSortMode);
  const [viewMode, setViewMode] = useState<SidebarViewMode>(readSidebarViewMode);
  const [manualOrder, setManualOrder] = useState<string[]>(readSidebarManualOrder);
  const [customFolders, setCustomFolders] = useState<CustomSidebarFolder[]>(readCustomSidebarFolders);
  const [collapsedFolderIds, setCollapsedFolderIds] = useState<ReadonlySet<string>>(() => new Set());
  const [folderDropTargetId, setFolderDropTargetId] = useState<string | null>(null);
  const [isCreatingFolder, setIsCreatingFolder] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');
  const [editingFolderId, setEditingFolderId] = useState<string | null>(null);
  const [editingFolderName, setEditingFolderName] = useState('');
  const [legacyHiddenConversationIds] = useState(readLegacyHiddenConversationIds);
  const [visibleChatLimit, setVisibleChatLimit] = useState(SIDEBAR_PAGE_SIZE);

  const drawerRef = useRef<HTMLElement | null>(null);
  const scrollAreaRef = useRef<HTMLDivElement | null>(null);
  const newFolderInputRef = useRef<HTMLInputElement | null>(null);
  const [isCompactSidebar, setIsCompactSidebar] = useState(() => sidebarWidth <= SIDEBAR_COMPACT_WIDTH);

  useEffect(() => {
    const sidebar = drawerRef.current;
    if (!sidebar) return;

    const updateCompactLayout = () => {
      const width = sidebar.clientWidth;
      setIsCompactSidebar(width <= SIDEBAR_COMPACT_WIDTH);
    };

    updateCompactLayout();
    const observer = new ResizeObserver(updateCompactLayout);
    observer.observe(sidebar);
    return () => observer.disconnect();
  }, []);

  const conversations = useChatStore((state) => state.conversations);
  const projects = useChatStore((state) => state.projects);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const serviceErrorMessage = useChatStore((state) => state.errorMessage);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const sendingConversationId = useChatStore((state) => state.sendingConversationId);
  const compactingConversationId = useChatStore((state) => state.compactingConversationId);
  const selectConversation = useChatStore((state) => state.selectConversation);
  const startNewChat = useChatStore((state) => state.startNewChat);
  const startNewChatWithAgent = useChatStore((state) => state.startNewChatWithAgent);
  const startTemporaryChat = useChatStore((state) => state.startTemporaryChat);
  const temporaryChat = useChatStore((state) => state.temporaryChat);
  const refreshProjects = useChatStore((state) => state.refreshProjects);
  const agentProfiles = useAssistantProfilesStore((state) => state.profiles);
  const agentProfilesLoadState = useAssistantProfilesStore((state) => state.loadState);
  const initializeAgentProfiles = useAssistantProfilesStore((state) => state.initialize);
  const retryAgentProfiles = useAssistantProfilesStore((state) => state.retry);

  const {
    archiveConversation,
    createProject,
    deleteConversation,
    duplicateConversation,
    exportConversation,
    feedback,
    isBusy,
    moveConversation,
    saveTitle,
    showFeedback,
    togglePinned,
  } = useConversationSidebarActions();

  const isConversationAiWorking = useCallback(
    (conversationId: string) =>
      sendingConversationId === conversationId || compactingConversationId === conversationId,
    [compactingConversationId, sendingConversationId],
  );

  useEffect(() => {
    if (sidebarPreferences.error) globalThis.reportError(sidebarPreferences.error);
  }, [sidebarPreferences.error]);

  useEffect(() => initializeAgentProfiles(), [initializeAgentProfiles]);

  useEffect(() => {
    const section = drawerRef.current;
    const shell = section?.closest<HTMLElement>('.conversation-sidebar-shell');
    const px = `${sidebarWidth}px`;
    section?.style.setProperty('--conversation-sidebar-custom-width', px);
    if (shell) {
      shell.style.setProperty('--conversation-sidebar-custom-width', px);
      shell.dataset.resizing = isResizing ? 'true' : 'false';
    }
  }, [isResizing, sidebarWidth]);

  useEffect(() => {
    if (!isCreatingFolder) return;
    const frame = window.requestAnimationFrame(() => newFolderInputRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, [isCreatingFolder]);

  const folderIdByConversationId = useMemo(() => {
    const map = new Map<string, string>();
    for (const folder of customFolders) {
      for (const conversationId of folder.conversationIds) {
        map.set(conversationId, folder.id);
      }
    }
    return map;
  }, [customFolders]);

  const visibleConversations = useMemo(
    () => conversations.filter((conversation) => !legacyHiddenConversationIds.has(conversation.id)),
    [conversations, legacyHiddenConversationIds],
  );

  const activeConversations = useMemo(
    () => visibleConversations.filter((conversation) => !conversation.isArchived),
    [visibleConversations],
  );
  const archivedConversations = useMemo(
    () => visibleConversations.filter((conversation) => conversation.isArchived),
    [visibleConversations],
  );
  const pinnedConversations = useMemo(
    () =>
      sortSidebarConversations(
        activeConversations.filter((conversation) => conversation.isPinned),
        sortMode,
        manualOrder,
      ),
    [activeConversations, manualOrder, sortMode],
  );
  const unassignedConversations = useMemo(
    () =>
      sortSidebarConversations(
        activeConversations.filter(
          (conversation) =>
            conversation.projectId === null && !conversation.isPinned && !folderIdByConversationId.has(conversation.id),
        ),
        sortMode,
        manualOrder,
      ),
    [activeConversations, folderIdByConversationId, manualOrder, sortMode],
  );
  const visibleProjectConversations = useMemo(
    () =>
      sortSidebarConversations(
        activeConversations.filter((conversation) => conversation.projectId !== null && !conversation.isPinned),
        sortMode,
        manualOrder,
      ),
    [activeConversations, manualOrder, sortMode],
  );

  const visiblePinned = pinnedConversations;
  const visibleChats = useMemo(
    () => unassignedConversations.slice(0, visibleChatLimit),
    [unassignedConversations, visibleChatLimit],
  );
  const hasMoreChats = unassignedConversations.length > visibleChatLimit;
  const visibleArchived = archivedConversations;
  const filteredProjectConversations = visibleProjectConversations;
  const filteredProjects = projects;

  const recencyBuckets = useMemo(
    () => (sortMode === 'updatedAt' ? groupConversationsByRecency(visibleChats) : []),
    [sortMode, visibleChats],
  );

  const confirmPermanentDelete = async () => {
    if (!deleteTarget) return;
    if (await deleteConversation(deleteTarget)) {
      setDeleteTarget(null);
    }
  };

  const handleMoveConversationToFolder = (conversation: ChatConversation, targetFolderId: string | null) => {
    const nextFolders = customFolders.map((folder) => {
      const withoutConversation = folder.conversationIds.filter((id) => id !== conversation.id);
      if (folder.id === targetFolderId) {
        return { ...folder, conversationIds: [conversation.id, ...withoutConversation] };
      }
      return withoutConversation.length === folder.conversationIds.length
        ? folder
        : { ...folder, conversationIds: withoutConversation };
    });
    setCustomFolders(nextFolders);
    writeCustomSidebarFolders(nextFolders);
  };

  const handleCreateCustomFolder = (event?: FormEvent<HTMLFormElement>) => {
    event?.preventDefault();
    const trimmed = newFolderName.trim();
    if (!trimmed) {
      setIsCreatingFolder(false);
      setNewFolderName('');
      return;
    }
    const created: CustomSidebarFolder = {
      id: `folder_${crypto.randomUUID().replaceAll('-', '')}`,
      name: trimmed.slice(0, 80),
      conversationIds: [],
      createdAt: Date.now(),
    };
    const next = [...customFolders, created];
    setCustomFolders(next);
    writeCustomSidebarFolders(next);
    setNewFolderName('');
    setIsCreatingFolder(false);
  };

  const handleRenameCustomFolder = (folderId: string) => {
    const trimmed = editingFolderName.trim();
    setEditingFolderId(null);
    if (!trimmed) return;
    const next = customFolders.map((folder) =>
      folder.id === folderId ? { ...folder, name: trimmed.slice(0, 80) } : folder,
    );
    setCustomFolders(next);
    writeCustomSidebarFolders(next);
  };

  const handleDeleteCustomFolder = (folderId: string) => {
    const next = customFolders.filter((folder) => folder.id !== folderId);
    setCustomFolders(next);
    writeCustomSidebarFolders(next);
  };

  const toggleFolderExpanded = (folderId: string) => {
    setCollapsedFolderIds((current) => {
      const next = new Set(current);
      if (next.has(folderId)) next.delete(folderId);
      else next.add(folderId);
      return next;
    });
  };

  const handleReorderConversation = (draggedConversationId: string, targetConversationId: string) => {
    if (draggedConversationId === targetConversationId) return;
    const baseIds = conversations.map((c) => c.id);
    const orderedIds = [
      ...manualOrder.filter((id) => baseIds.includes(id)),
      ...baseIds.filter((id) => !manualOrder.includes(id)),
    ];
    const withoutDragged = orderedIds.filter((id) => id !== draggedConversationId);
    const targetIdx = withoutDragged.indexOf(targetConversationId);
    if (targetIdx < 0) return;
    withoutDragged.splice(targetIdx, 0, draggedConversationId);
    setManualOrder(withoutDragged);
    writeSidebarManualOrder(withoutDragged);
    if (sortMode !== 'manual') {
      setSortMode('manual');
      writeSidebarSortMode('manual');
    }
  };

  const dropConversationToProject = (conversationId: string, projectId: string) => {
    const conversation = activeConversations.find((item) => item.id === conversationId);
    if (conversation) {
      handleMoveConversationToFolder(conversation, null);
      if (conversation.projectId !== projectId) {
        void moveConversation(conversation, projectId);
      }
    }
  };

  const dropConversationToFolder = (conversationId: string, folderId: string) => {
    const conversation = activeConversations.find((item) => item.id === conversationId);
    if (!conversation) return;
    if (conversation.projectId !== null || conversation.isPinned) {
      void moveConversation(conversation, null);
    }
    handleMoveConversationToFolder(conversation, folderId);
  };

  const dropConversationToChats = (conversationId: string) => {
    const conversation = activeConversations.find((item) => item.id === conversationId);
    if (!conversation) return;
    if (folderIdByConversationId.has(conversation.id)) {
      handleMoveConversationToFolder(conversation, null);
    }
    if (conversation.projectId !== null || conversation.isPinned) {
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

  const handleDuplicateConversation = (conversation: ChatConversation) => {
    onRequestTemporaryExit(async () => {
      const duplicated = await duplicateConversation(conversation);
      if (duplicated) {
        await selectConversation(duplicated.id);
        onCloseMobile?.();
      }
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

  const handleNewChatWithAgent = (agent: (typeof agentProfiles)[number]) => {
    const snapshot = snapshotAssistantProfile(agent);
    if (onNewAgentChat) {
      onNewAgentChat(snapshot);
      return;
    }
    onRequestTemporaryExit(() => {
      startNewChatWithAgent(snapshot);
      onCloseMobile?.();
    });
  };

  const handleManageAgents = () => {
    if (onManageAgents) {
      onManageAgents();
      return;
    }
    onCloseMobile?.();
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

  const handleResizePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (isDrawer || event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = sidebarWidth;
    setIsResizing(true);

    const handlePointerMove = (moveEvent: PointerEvent) => {
      const nextWidth = clampSidebarWidth(startWidth + (moveEvent.clientX - startX));
      setSidebarWidth(nextWidth);
    };

    const handlePointerUp = (upEvent: PointerEvent) => {
      const finalWidth = clampSidebarWidth(startWidth + (upEvent.clientX - startX));
      setSidebarWidth(finalWidth);
      setIsResizing(false);
      writeSidebarWidth(finalWidth);
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', handlePointerUp);
      window.removeEventListener('pointercancel', handlePointerUp);
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', handlePointerUp);
    window.addEventListener('pointercancel', handlePointerUp);
  };

  const handleResizeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      event.preventDefault();
      const delta = event.key === 'ArrowRight' ? 12 : -12;
      const next = clampSidebarWidth(sidebarWidth + delta);
      setSidebarWidth(next);
      writeSidebarWidth(next);
    } else if (event.key === 'Home') {
      event.preventDefault();
      setSidebarWidth(SIDEBAR_MIN_WIDTH);
      writeSidebarWidth(SIDEBAR_MIN_WIDTH);
    } else if (event.key === 'End') {
      event.preventDefault();
      setSidebarWidth(SIDEBAR_MAX_WIDTH);
      writeSidebarWidth(SIDEBAR_MAX_WIDTH);
    }
  };

  const handleScrollAreaKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    const activeElement = document.activeElement;
    if (!(activeElement instanceof HTMLElement) || !activeElement.hasAttribute('data-sidebar-nav-item')) return;
    const container = scrollAreaRef.current;
    if (!container) return;
    const navItems = Array.from(
      container.querySelectorAll<HTMLElement>('[data-sidebar-nav-item="true"]:not(:disabled)'),
    ).filter((el) => el.getClientRects().length > 0);
    if (navItems.length === 0) return;
    const currentIndex = navItems.indexOf(activeElement);
    if (currentIndex < 0) return;
    event.preventDefault();
    const nextIndex =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? navItems.length - 1
          : event.key === 'ArrowDown'
            ? (currentIndex + 1) % navItems.length
            : (currentIndex <= 0 ? navItems.length : currentIndex) - 1;
    navItems[nextIndex]?.focus();
  };

  const handleScrollAreaScroll = () => {
    const container = scrollAreaRef.current;
    if (!container || !hasMoreChats) return;
    const remaining = container.scrollHeight - container.scrollTop - container.clientHeight;
    if (remaining < 180) {
      setVisibleChatLimit((current) => Math.min(unassignedConversations.length, current + SIDEBAR_PAGE_SIZE));
    }
  };

  const renderRows = (
    rows: readonly ChatConversation[],
    options: {
      archived: boolean;
      currentProjectId: string | null;
      currentFolderId?: string | null;
      allowMove: boolean;
    },
  ) =>
    rows.map((conversation) => (
      <ConversationSidebarRow
        active={activeConversationId === conversation.id}
        allowMove={options.allowMove}
        archived={options.archived}
        conversation={conversation}
        currentFolderId={options.currentFolderId ?? folderIdByConversationId.get(conversation.id) ?? null}
        currentProjectId={conversation.projectId ?? options.currentProjectId}
        customFolders={customFolders}
        disabled={isBusy}
        isAiWorking={isConversationAiWorking(conversation.id)}
        key={conversation.id}
        onArchive={() => void archiveConversation(conversation)}
        onDelete={() => setDeleteTarget(conversation)}
        onDuplicate={() => handleDuplicateConversation(conversation)}
        onExport={(format) => void exportConversation(conversation, format)}
        onInlineRename={(title) => saveTitle(conversation.id, title)}
        onMoveToChats={() => {
          handleMoveConversationToFolder(conversation, null);
          void moveConversation(conversation, null);
        }}
        onMoveToFolder={(folderId) => handleMoveConversationToFolder(conversation, folderId)}
        onMoveToProject={(projectId) => {
          handleMoveConversationToFolder(conversation, null);
          void moveConversation(conversation, projectId);
        }}
        onRename={() => setEditTarget({ conversation, mode: 'rename' })}
        onReorderConversation={handleReorderConversation}
        onSelect={() => requestSelectConversation(conversation.id)}
        onTogglePinned={() => void togglePinned(conversation)}
        projects={projects}
        viewMode={viewMode}
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

  const showPinned = (sectionFilter === 'all' || sectionFilter === 'chats') && sectionIsVisible(visiblePinned, false);
  const showProjects = sectionFilter === 'all' || sectionFilter === 'projects';
  const showFolders =
    (sectionFilter === 'all' || sectionFilter === 'chats') && (customFolders.length > 0 || isCreatingFolder);
  const showChats = (sectionFilter === 'all' || sectionFilter === 'chats') && sectionIsVisible(visibleChats, true);
  const showArchived =
    (sectionFilter === 'all' || sectionFilter === 'chats') && sectionIsVisible(visibleArchived, false);
  const agentMenuItems = (
    <>
      {agentProfilesLoadState === 'idle' || agentProfilesLoadState === 'loading' ? (
        <p aria-live="polite" className="px-2 py-2 text-xs text-secondary-text" role="status">
          {t('modelsLoading')}
        </p>
      ) : agentProfilesLoadState === 'error' ? (
        <div className="grid gap-1 px-2 py-2" role="alert">
          <p className="text-xs text-destructive">{t('assistantProfilesLoadFailed')}</p>
          <DropdownMenuItem asChild onSelect={retryAgentProfiles}>
            <Button className="w-full justify-start" size="small" type="button" variant="ghost">
              {t('retry')}
            </Button>
          </DropdownMenuItem>
        </div>
      ) : agentProfiles.length > 0 ? (
        agentProfiles.map((agent) => (
          <DropdownMenuItem
            asChild
            className="w-full min-w-0 gap-2"
            disabled={isBusy}
            key={agent.id}
            onSelect={() => handleNewChatWithAgent(agent)}
            textValue={agent.name}
          >
            <Button className="w-full justify-start" disabled={isBusy} size="small" type="button" variant="ghost">
              <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
              <span className="min-w-0 flex-1 truncate text-left">{agent.name}</span>
            </Button>
          </DropdownMenuItem>
        ))
      ) : (
        <p className="px-2 py-2 text-xs text-secondary-text" role="status">
          {t('assistantProfilesEmpty')}
        </p>
      )}
      <DropdownMenuSeparator />
      <DropdownMenuItem asChild className="w-full min-w-0 gap-2" onSelect={handleManageAgents}>
        <Button className="w-full justify-start" size="small" type="button" variant="ghost">
          <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
          <span>{t('assistantProfileManage')}</span>
        </Button>
      </DropdownMenuItem>
    </>
  );
  const viewAndSortMenuItems = (
    <>
      <DropdownMenuLabel>{t('sidebarViewModeLabel')}</DropdownMenuLabel>
      {(['compact', 'detailed'] as const).map((mode) => (
        <DropdownMenuItem
          aria-checked={viewMode === mode}
          className="justify-between"
          key={mode}
          onSelect={() => {
            setViewMode(mode);
            writeSidebarViewMode(mode);
          }}
          role="menuitemradio"
        >
          <span>{t(mode === 'compact' ? 'sidebarViewCompact' : 'sidebarViewDetailed')}</span>
          {viewMode === mode ? <Check aria-hidden="true" className="text-accent" size={13} /> : null}
        </DropdownMenuItem>
      ))}
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t('sidebarSortByLabel')}</DropdownMenuLabel>
      {(
        [
          { id: 'updatedAt', labelKey: 'sidebarSortUpdatedAt' },
          { id: 'createdAt', labelKey: 'sidebarSortCreatedAt' },
          { id: 'alphabetical', labelKey: 'sidebarSortAlphabetical' },
          { id: 'manual', labelKey: 'sidebarSortManual' },
        ] as const
      ).map((option) => (
        <DropdownMenuItem
          aria-checked={sortMode === option.id}
          className="justify-between"
          key={option.id}
          onSelect={() => {
            setSortMode(option.id);
            writeSidebarSortMode(option.id);
          }}
          role="menuitemradio"
        >
          <span>{t(option.labelKey)}</span>
          {sortMode === option.id ? <Check aria-hidden="true" className="text-accent" size={13} /> : null}
        </DropdownMenuItem>
      ))}
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={() => setIsCreatingFolder(true)}>
        <Folder aria-hidden="true" size={14} />
        <span>{t('createCustomFolder')}</span>
      </DropdownMenuItem>
    </>
  );

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
        <div className="conversation-sidebar-toolbar sticky top-0 z-10 flex shrink-0 items-center gap-1.5 bg-navigation/80 px-3 pt-3 pb-2 backdrop-blur-md">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                aria-label={t('moreOptions')}
                className="conversation-sidebar-minimal-menu-trigger size-9 shrink-0 border-border bg-surface/80 px-0 text-secondary-icon shadow-2xs backdrop-blur-sm transition-all duration-quick hover:border-control-border hover:bg-raised/90 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                size="icon"
                title={t('moreOptions')}
                type="button"
                variant="secondary"
              >
                <Menu aria-hidden="true" size={16} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" aria-label={t('moreOptions')} className="min-w-56">
              <DropdownMenuItem onSelect={handleNewChat}>
                <SquarePen aria-hidden="true" size={14} />
                <span>{t('newChat')}</span>
              </DropdownMenuItem>
              <DropdownMenuSub>
                <DropdownMenuSubTrigger className="justify-between" disabled={isBusy}>
                  <span className="flex min-w-0 items-center gap-3">
                    <Bot aria-hidden="true" size={14} />
                    <span className="truncate">{t('newChatWithAgent')}</span>
                  </span>
                  <ChevronRight aria-hidden="true" size={14} />
                </DropdownMenuSubTrigger>
                <DropdownMenuSubContent aria-label={t('newChatWithAgent')} className="min-w-60">
                  <DropdownMenuLabel>{t('newChatWithAgent')}</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  {agentMenuItems}
                </DropdownMenuSubContent>
              </DropdownMenuSub>
              <DropdownMenuSeparator />
              <DropdownMenuItem disabled={isBusy} onSelect={() => setCreateProjectOpen(true)}>
                <FolderPlus aria-hidden="true" size={14} />
                <span>{t('createProject')}</span>
              </DropdownMenuItem>
              <DropdownMenuItem
                aria-checked={Boolean(temporaryChat)}
                className="justify-between"
                onSelect={handleStartTemporaryChat}
                role="menuitemcheckbox"
              >
                <span className="flex min-w-0 items-center gap-3">
                  <Clock3 aria-hidden="true" size={14} />
                  <span className="truncate">
                    {t(temporaryChat ? 'temporaryChatEndButtonTitle' : 'temporaryChatStartButtonTitle')}
                  </span>
                </span>
                {temporaryChat ? <Check aria-hidden="true" className="text-accent" size={13} /> : null}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuSub>
                <DropdownMenuSubTrigger className="justify-between">
                  <span className="flex min-w-0 items-center gap-3">
                    <SlidersHorizontal aria-hidden="true" size={14} />
                    <span className="truncate">{t('sidebarViewAndSortOptions')}</span>
                  </span>
                  <ChevronRight aria-hidden="true" size={14} />
                </DropdownMenuSubTrigger>
                <DropdownMenuSubContent aria-label={t('sidebarViewAndSortOptions')} className="min-w-52">
                  {viewAndSortMenuItems}
                </DropdownMenuSubContent>
              </DropdownMenuSub>
            </DropdownMenuContent>
          </DropdownMenu>
          <div className="conversation-sidebar-new-chat-group flex h-9 min-w-0 flex-1 overflow-hidden rounded-button border border-border bg-surface/80 shadow-2xs backdrop-blur-sm">
            <Button
              aria-label={t('newChat')}
              className="conversation-sidebar-new-chat-main group min-h-0 h-full min-w-0 flex-1 justify-between rounded-none border-0 bg-transparent px-3 text-left text-settings-description font-medium text-text shadow-none transition-colors duration-quick hover:bg-raised/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-focus"
              onClick={handleNewChat}
              size="small"
              title={t('newChatButtonTitle')}
              type="button"
              variant="ghost"
            >
              <span className="flex min-w-0 items-center gap-2">
                <SquarePen
                  aria-hidden="true"
                  className="shrink-0 text-secondary-icon transition-colors group-hover:text-text"
                  size={15}
                />
                <span className="conversation-sidebar-new-chat-label truncate">{t('newChat')}</span>
              </span>
              <Plus
                aria-hidden="true"
                className="conversation-sidebar-new-chat-plus shrink-0 text-secondary-icon/70 transition-colors group-hover:text-text"
                size={14}
              />
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  aria-label={t('newChatWithAgent')}
                  className="conversation-sidebar-agent-button h-full w-9 shrink-0 rounded-none border-0 border-l border-l-border bg-transparent px-0 text-secondary-icon transition-colors duration-quick hover:bg-raised/90 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-focus data-[state=open]:bg-raised data-[state=open]:text-text disabled:opacity-45"
                  disabled={isBusy}
                  size="icon"
                  title={t('newChatWithAgent')}
                  type="button"
                  variant="ghost"
                >
                  <Bot aria-hidden="true" size={15} />
                  <ChevronDown aria-hidden="true" className="conversation-sidebar-agent-chevron" size={12} />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" aria-label={t('newChatWithAgent')} className="min-w-60">
                <DropdownMenuLabel>{t('newChatWithAgent')}</DropdownMenuLabel>
                <DropdownMenuSeparator />
                {agentMenuItems}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
          <Button
            aria-label={t('createProject')}
            className="conversation-sidebar-toolbar-button size-9 shrink-0 border-border bg-surface/80 px-0 text-secondary-icon shadow-2xs backdrop-blur-sm transition-all duration-quick hover:border-control-border hover:bg-raised/90 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-45"
            disabled={isBusy}
            onClick={() => setCreateProjectOpen(true)}
            size="icon"
            title={t('createProjectButtonTitle')}
            type="button"
            variant="secondary"
          >
            <FolderPlus aria-hidden="true" size={15} />
          </Button>
          <Button
            aria-label={temporaryChat ? t('temporaryChatEnd') : t('temporaryChatStart')}
            aria-pressed={Boolean(temporaryChat)}
            className={`conversation-sidebar-toolbar-button size-9 shrink-0 border px-0 backdrop-blur-sm transition-all duration-quick focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
              temporaryChat
                ? 'border-accent/40 bg-accent/15 text-accent hover:bg-accent/25'
                : 'border-border bg-surface/80 text-secondary-icon shadow-2xs hover:border-control-border hover:bg-raised/90 hover:text-text'
            }`}
            onClick={handleStartTemporaryChat}
            size="icon"
            title={temporaryChat ? t('temporaryChatEndButtonTitle') : t('temporaryChatStartButtonTitle')}
            type="button"
            variant="ghost"
          >
            <Clock3 aria-hidden="true" size={15} />
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                aria-label={t('sidebarViewAndSortOptions')}
                className="conversation-sidebar-toolbar-button size-9 shrink-0 border-border bg-surface/80 px-0 text-secondary-icon shadow-2xs backdrop-blur-sm transition-all duration-quick hover:border-control-border hover:bg-raised/90 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus data-[state=open]:border-control-border data-[state=open]:bg-raised data-[state=open]:text-text"
                size="icon"
                title={t('sidebarViewAndSortOptions')}
                type="button"
                variant="secondary"
              >
                <SlidersHorizontal aria-hidden="true" size={15} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent aria-label={t('sidebarViewAndSortOptions')} className="min-w-52">
              {viewAndSortMenuItems}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        <section
          aria-label={t('statisticsConversations')}
          className="openchat-scroll-area conversation-sidebar-scroll-area min-w-0 flex-1 px-2.5 pb-4 pt-1"
          onKeyDown={handleScrollAreaKeyDown}
          onScroll={handleScrollAreaScroll}
          ref={scrollAreaRef}
        >
          <div className="grid gap-4">
            {feedback ? (
              <p
                className={`rounded-button border border-border bg-surface px-3 py-2 text-xs ${feedback.success ? 'text-success' : 'text-destructive'}`}
                role={feedback.success ? 'status' : 'alert'}
              >
                {t(feedback.key)}
              </p>
            ) : null}
            {serviceStatus === 'error' ? (
              <div className="grid gap-1 rounded-button border border-border bg-surface px-3 py-2">
                <div className="flex items-center justify-between gap-2">
                  <p className="text-xs text-destructive" role="alert">
                    {t('chatHistoryUnavailable')}
                  </p>
                  <Button
                    aria-label={t('retry')}
                    className="size-7 shrink-0 rounded-control"
                    onClick={() => void useChatStore.getState().initialize()}
                    size="icon"
                    variant="ghost"
                  >
                    <RefreshCw aria-hidden="true" size={14} />
                  </Button>
                </div>
                {serviceErrorMessage ? (
                  <p className="break-words text-xs text-secondary-text">
                    {t('chatErrorDetails')}: {serviceErrorMessage}
                  </p>
                ) : null}
              </div>
            ) : null}
            {serviceStatus === 'idle' || serviceStatus === 'loading' ? (
              <p aria-live="polite" className="px-3 py-2 text-xs text-secondary-text">
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
                    compactSidebar={isCompactSidebar}
                    conversations={filteredProjectConversations}
                    customFolders={customFolders}
                    isBusy={isBusy}
                    isConversationAiWorking={isConversationAiWorking}
                    onArchive={(conversation) => void archiveConversation(conversation)}
                    onCreateProjectChat={(projectId) =>
                      onRequestTemporaryExit(() => {
                        startNewChat(projectId);
                        onCloseMobile?.();
                      })
                    }
                    onDelete={(conversation) => setDeleteTarget(conversation)}
                    onDropConversationToProject={dropConversationToProject}
                    onDuplicate={handleDuplicateConversation}
                    onExport={(conversation, format) => void exportConversation(conversation, format)}
                    onInlineRename={(conversation, title) => saveTitle(conversation.id, title)}
                    onMoveToChats={(conversation) => void moveConversation(conversation, null)}
                    onMoveToFolder={(conversation, folderId) => handleMoveConversationToFolder(conversation, folderId)}
                    onMoveToProject={(conversation, projectId) => void moveConversation(conversation, projectId)}
                    onOpenOptions={setProjectOptionsTarget}
                    onRename={(conversation) => setEditTarget({ conversation, mode: 'rename' })}
                    onRetry={() => void refreshProjects().catch(() => showFeedback('projectLoadFailed'))}
                    onSelectConversation={(id) => requestSelectConversation(id)}
                    onToggleCollapsed={() => toggleSection('projects')}
                    onTogglePinned={(conversation) => void togglePinned(conversation)}
                    projects={filteredProjects}
                    status={projectsStatus}
                    viewMode={viewMode}
                  />
                ) : null}

                {showFolders ? (
                  <ConversationSidebarSection
                    action={
                      <Button
                        aria-label={t('createCustomFolder')}
                        className="size-6 rounded-control text-secondary-icon hover:bg-hover hover:text-text"
                        onClick={() => setIsCreatingFolder(true)}
                        size="icon"
                        title={t('createCustomFolder')}
                        variant="ghost"
                      >
                        <Plus aria-hidden="true" size={13} />
                      </Button>
                    }
                    collapsed={collapsedSections.has('folders')}
                    icon={Folder}
                    onToggle={() => toggleSection('folders')}
                    title={t('customFolders')}
                  >
                    {isCreatingFolder ? (
                      <li className="px-1 py-0.5">
                        <form className="flex items-center gap-1" onSubmit={handleCreateCustomFolder}>
                          <Input
                            aria-label={t('customFolderNamePlaceholder')}
                            className="h-7 min-h-0 w-auto min-w-0 flex-1 rounded-control border-accent/60 bg-surface px-2 text-xs text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                            density="compact"
                            fieldLayout="none"
                            maxLength={80}
                            onBlur={() => handleCreateCustomFolder()}
                            onChange={(event) => setNewFolderName(event.target.value)}
                            onKeyDown={(event) => {
                              if (event.key === 'Escape') {
                                event.preventDefault();
                                setIsCreatingFolder(false);
                                setNewFolderName('');
                              }
                            }}
                            placeholder={t('customFolderNamePlaceholder')}
                            ref={newFolderInputRef}
                            value={newFolderName}
                          />
                          <Button
                            aria-label={t('save')}
                            className="size-6 rounded-control p-0 text-accent hover:bg-hover"
                            onMouseDown={(event) => event.preventDefault()}
                            size="icon"
                            type="submit"
                            variant="ghost"
                          >
                            <Check aria-hidden="true" size={13} />
                          </Button>
                          <Button
                            aria-label={t('cancel')}
                            className="size-6 rounded-control p-0 text-secondary-icon hover:bg-hover hover:text-text"
                            onClick={() => {
                              setIsCreatingFolder(false);
                              setNewFolderName('');
                            }}
                            onMouseDown={(event) => event.preventDefault()}
                            size="icon"
                            type="button"
                            variant="ghost"
                          >
                            <X aria-hidden="true" size={13} />
                          </Button>
                        </form>
                      </li>
                    ) : null}
                    {customFolders.map((folder) => {
                      const folderConversations = sortSidebarConversations(
                        activeConversations.filter(
                          (conversation) =>
                            !conversation.isPinned &&
                            conversation.projectId === null &&
                            folder.conversationIds.includes(conversation.id),
                        ),
                        sortMode,
                        manualOrder,
                      );
                      const isExpanded = !collapsedFolderIds.has(folder.id);
                      const FolderIcon = isExpanded && folderConversations.length > 0 ? FolderOpen : Folder;
                      const isSelected = folderConversations.some((c) => c.id === activeConversationId);
                      return (
                        <li
                          className={`group/folder-item grid gap-0.5 rounded-button transition-colors duration-quick ${
                            folderDropTargetId === folder.id ? 'bg-accent/5 ring-1 ring-accent/60' : ''
                          }`}
                          key={folder.id}
                          onDragLeave={(event) => {
                            if (
                              event.relatedTarget instanceof Node &&
                              event.currentTarget.contains(event.relatedTarget)
                            )
                              return;
                            setFolderDropTargetId(null);
                          }}
                          onDragOver={(event) => {
                            if (!event.dataTransfer.types.includes('application/x-openchat-conversation')) return;
                            event.preventDefault();
                            event.dataTransfer.dropEffect = 'move';
                            setFolderDropTargetId(folder.id);
                          }}
                          onDrop={(event) => {
                            const conversationId = event.dataTransfer.getData('application/x-openchat-conversation');
                            if (!conversationId) return;
                            event.preventDefault();
                            setFolderDropTargetId(null);
                            dropConversationToFolder(conversationId, folder.id);
                          }}
                        >
                          {editingFolderId === folder.id ? (
                            <form
                              className="flex min-h-9 items-center gap-1 px-2 py-1"
                              onSubmit={(event) => {
                                event.preventDefault();
                                handleRenameCustomFolder(folder.id);
                              }}
                            >
                              <Input
                                aria-label={t('customFolderNamePlaceholder')}
                                className="h-7 min-h-0 w-auto min-w-0 flex-1 rounded-control border-accent/60 bg-surface px-2 text-xs text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                                density="compact"
                                fieldLayout="none"
                                maxLength={80}
                                onBlur={() => handleRenameCustomFolder(folder.id)}
                                onChange={(event) => setEditingFolderName(event.target.value)}
                                onKeyDown={(event) => {
                                  if (event.key === 'Escape') {
                                    event.preventDefault();
                                    setEditingFolderId(null);
                                  }
                                }}
                                value={editingFolderName}
                              />
                            </form>
                          ) : (
                            <div
                              className={`relative flex min-h-9 items-center rounded-button transition-colors duration-quick ${
                                isSelected
                                  ? 'bg-selected/65 font-medium text-text'
                                  : 'text-text/85 hover:bg-hover/85 hover:text-text focus-within:bg-hover/85 focus-within:text-text'
                              }`}
                            >
                              <Button
                                aria-expanded={folderConversations.length > 0 ? isExpanded : undefined}
                                className="min-h-9 min-w-0 flex-1 justify-start gap-2.5 rounded-button px-3 py-1.5 text-left transition-all duration-quick group-focus-within/folder-item:pr-14 group-hover/folder-item:pr-14 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                                data-sidebar-nav-item="true"
                                onClick={() => toggleFolderExpanded(folder.id)}
                                size="small"
                                type="button"
                                variant="ghost"
                              >
                                <FolderIcon
                                  aria-hidden="true"
                                  className={`shrink-0 transition-colors ${isSelected ? 'text-accent' : 'text-secondary-icon'}`}
                                  size={15}
                                />
                                <span className="sidebar-project-title min-w-0 flex-1 truncate font-medium">
                                  {folder.name}
                                </span>
                                {folderConversations.length > 0 ? (
                                  <span className="shrink-0 text-xs text-secondary-text/70 transition-opacity duration-quick group-focus-within/folder-item:opacity-0 group-hover/folder-item:opacity-0">
                                    {folderConversations.length}
                                  </span>
                                ) : null}
                              </Button>
                              <div className="pointer-events-none absolute inset-y-0 right-1.5 flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity duration-quick group-focus-within/folder-item:pointer-events-auto group-focus-within/folder-item:opacity-100 group-hover/folder-item:pointer-events-auto group-hover/folder-item:opacity-100">
                                <Button
                                  aria-label={t('renameCustomFolder')}
                                  className="size-7 rounded-control text-secondary-icon hover:bg-surface/80 hover:text-text"
                                  onClick={() => {
                                    setEditingFolderName(folder.name);
                                    setEditingFolderId(folder.id);
                                  }}
                                  size="icon"
                                  title={t('renameCustomFolder')}
                                  variant="ghost"
                                >
                                  <Pencil aria-hidden="true" size={13} />
                                </Button>
                                <Button
                                  aria-label={t('deleteCustomFolder')}
                                  className="size-7 rounded-control text-secondary-icon hover:bg-surface/80 hover:text-destructive"
                                  onClick={() => handleDeleteCustomFolder(folder.id)}
                                  size="icon"
                                  title={t('deleteCustomFolder')}
                                  variant="ghost"
                                >
                                  <Trash2 aria-hidden="true" size={13} />
                                </Button>
                              </div>
                            </div>
                          )}
                          {folderConversations.length > 0 && isExpanded ? (
                            <ul className="ml-4 grid gap-0.5 border-l border-border pl-2 py-0.5">
                              {renderRows(folderConversations, {
                                archived: false,
                                currentProjectId: null,
                                currentFolderId: folder.id,
                                allowMove: true,
                              })}
                            </ul>
                          ) : null}
                        </li>
                      );
                    })}
                  </ConversationSidebarSection>
                ) : null}

                {showChats ? (
                  <ConversationSidebarSection
                    childrenClassName="grid gap-3"
                    collapsed={collapsedSections.has('chats')}
                    icon={MessageSquare}
                    onDropConversation={dropConversationToChats}
                    onToggle={() => toggleSection('chats')}
                    title={t('chats')}
                  >
                    <li className="grid gap-3">
                      {visibleChats.length === 0 ? (
                        <div className="px-3 py-2 text-xs text-secondary-text">{t('noChatsTitle')}</div>
                      ) : recencyBuckets.length > 0 ? (
                        <ul className="grid gap-3.5">
                          {recencyBuckets.map((bucket) => (
                            <li className="grid gap-0.5" key={bucket.id}>
                              <div className="px-3 pb-1 text-xs font-medium text-secondary-text/70 select-none">
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
                          ))}
                        </ul>
                      ) : (
                        <ul className="grid gap-0.5">
                          {renderRows(visibleChats, {
                            archived: false,
                            currentProjectId: null,
                            allowMove: true,
                          })}
                        </ul>
                      )}
                      {hasMoreChats ? (
                        <Button
                          className="mx-2 min-h-9 justify-start rounded-button border-border bg-surface/60 px-3 py-1.5 text-xs font-medium text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                          onClick={() =>
                            setVisibleChatLimit((current) =>
                              Math.min(unassignedConversations.length, current + SIDEBAR_PAGE_SIZE),
                            )
                          }
                          size="small"
                          type="button"
                          variant="secondary"
                        >
                          {t('loadMoreChats')}
                        </Button>
                      ) : null}
                    </li>
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
        </section>

        {!isDrawer ? (
          // biome-ignore lint/a11y/useSemanticElements: Custom interactive resize separator for the sidebar width.
          <div
            aria-label={t('resizeSidebar')}
            aria-orientation="vertical"
            aria-valuemax={SIDEBAR_MAX_WIDTH}
            aria-valuemin={SIDEBAR_MIN_WIDTH}
            aria-valuenow={sidebarWidth}
            className="group/resizer absolute inset-y-0 right-0 z-20 w-1.5 cursor-col-resize select-none focus-visible:outline-none"
            onDoubleClick={() => {
              setSidebarWidth(SIDEBAR_DEFAULT_WIDTH);
              writeSidebarWidth(SIDEBAR_DEFAULT_WIDTH);
            }}
            onKeyDown={handleResizeKeyDown}
            onPointerDown={handleResizePointerDown}
            role="separator"
            tabIndex={0}
            title={t('resizeSidebar')}
          >
            <span
              className={`pointer-events-none absolute inset-y-0 right-0 w-0.5 transition-colors duration-quick ${
                isResizing
                  ? 'bg-accent'
                  : 'bg-transparent group-hover/resizer:bg-accent/50 group-focus-visible/resizer:bg-accent'
              }`}
            />
          </div>
        ) : null}

        <CreateProjectDialog onCreate={createProject} onOpenChange={setCreateProjectOpen} open={createProjectOpen} />
        <ProjectOptionsDialog
          onOpenChange={(open) => {
            if (!open) setProjectOptionsTarget(null);
          }}
          onUseWorktree={async (path, branch) => {
            if (!projectOptionsTarget) return;
            onRequestTemporaryExit(async () => {
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
              <div className="flex justify-end gap-2 border-t border-border pt-4">
                <Button disabled={isBusy} onClick={() => setDeleteTarget(null)} variant="ghost">
                  {t('cancel')}
                </Button>
                <Button disabled={isBusy} onClick={() => void confirmPermanentDelete()} variant="destructive">
                  {t('deleteConversationPermanently')}
                </Button>
              </div>
            </div>
          ) : null}
        </Dialog>
      </section>
    </>
  );
}

function getFocusableElements(root: HTMLElement): HTMLElement[] {
  const selector =
    'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';
  return Array.from(root.querySelectorAll<HTMLElement>(selector)).filter(
    (element) => element.getClientRects().length > 0,
  );
}

function normalizeProjectPath(path: string): string {
  return path.trim().replaceAll('/', '\\').replace(/\\+$/, '').toLowerCase();
}
