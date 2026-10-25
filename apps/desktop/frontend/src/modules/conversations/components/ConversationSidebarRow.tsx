import type { LucideIcon } from 'lucide-react';
import {
  Archive,
  ArchiveRestore,
  Check,
  Copy,
  Download,
  Folder,
  FolderInput,
  FolderMinus,
  MessageCircle,
  MoreHorizontal,
  PanelRight,
  Pencil,
  Pin,
  PinOff,
  Trash2,
  X,
} from 'lucide-react';
import {
  type DragEvent,
  type FormEvent,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import type { ChatConversation, ChatProject, ExportFormat } from '@/modules/conversations/data/chatStorage';
import type { CustomSidebarFolder, SidebarViewMode } from '@/modules/conversations/data/sidebarPreferences';
import { setSidebarDraggingConversationId } from '@/modules/conversations/data/sidebarPreferences';
import { useTransitionPresence } from '@/shared/hooks/useTransitionPresence';

type MenuAnchor = { kind: 'trigger' } | { kind: 'point'; x: number; y: number };

const CONVERSATION_MENU_OPEN_EVENT = 'openchat-conversation-menu-open';

export interface ConversationSidebarRowProps {
  conversation: ChatConversation;
  projects: readonly ChatProject[];
  customFolders?: readonly CustomSidebarFolder[];
  active: boolean;
  archived: boolean;
  isAiWorking?: boolean;
  viewMode?: SidebarViewMode;
  currentProjectId: string | null;
  currentFolderId?: string | null;
  allowMove: boolean;
  disabled: boolean;
  onSelect(): void;
  onTogglePinned(): void;
  onRename(): void;
  onInlineRename?(title: string): Promise<void> | void;
  onDuplicate?(): void;
  onExport(format?: ExportFormat): void;
  onArchive(): void;
  onDelete(): void;
  onMoveToProject(projectId: string): void;
  onMoveToChats(): void;
  onMoveToFolder?(folderId: string | null): void;
  onReorderConversation?(draggedConversationId: string, targetConversationId: string): void;
}

export function ConversationSidebarRow({
  conversation,
  projects,
  customFolders = [],
  active,
  archived,
  isAiWorking = false,
  viewMode = 'compact',
  currentProjectId,
  currentFolderId = null,
  allowMove,
  disabled,
  onSelect,
  onTogglePinned,
  onRename,
  onInlineRename,
  onDuplicate,
  onExport,
  onArchive,
  onDelete,
  onMoveToProject,
  onMoveToChats,
  onMoveToFolder,
  onReorderConversation,
}: ConversationSidebarRowProps) {
  const { t, i18n } = useTranslation();
  const availableProjects = projects.filter((project) => project.id !== currentProjectId);
  const availableFolders = customFolders.filter((folder) => folder.id !== currentFolderId);

  const menuInstanceId = useId();
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuAnchor, setMenuAnchor] = useState<MenuAnchor>({ kind: 'trigger' });
  const menuIsPresent = useTransitionPresence(menuOpen);
  const [menuPosition, setMenuPosition] = useState<{
    top: number;
    left: number;
    minWidth: number;
    maxWidth: number;
    maxHeight: number;
    placement: 'above' | 'below';
  } | null>(null);
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [draftTitle, setDraftTitle] = useState(conversation.title);
  const [isSavingTitle, setIsSavingTitle] = useState(false);
  const [isReorderDropTarget, setIsReorderDropTarget] = useState(false);

  const rowRef = useRef<HTMLLIElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const inlineInputRef = useRef<HTMLInputElement>(null);

  const formattedUpdatedTime = useMemo(() => {
    try {
      return new Intl.DateTimeFormat(i18n.language, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      }).format(new Date(conversation.updatedAt));
    } catch {
      return '';
    }
  }, [conversation.updatedAt, i18n.language]);

  const menuTrigger = useCallback(
    () => rowRef.current?.querySelector<HTMLButtonElement>('[data-conversation-menu-trigger]') ?? null,
    [],
  );

  const openMenu = (anchor: MenuAnchor) => {
    setMenuAnchor(anchor);
    window.dispatchEvent(new CustomEvent(CONVERSATION_MENU_OPEN_EVENT, { detail: menuInstanceId }));
    setMenuOpen(true);
  };

  useEffect(() => {
    const closeWhenAnotherMenuOpens = (event: Event) => {
      if (!(event instanceof CustomEvent) || typeof event.detail !== 'string') return;
      if (event.detail !== menuInstanceId) setMenuOpen(false);
    };

    window.addEventListener(CONVERSATION_MENU_OPEN_EVENT, closeWhenAnotherMenuOpens);
    return () => window.removeEventListener(CONVERSATION_MENU_OPEN_EVENT, closeWhenAnotherMenuOpens);
  }, [menuInstanceId]);

  const primaryButton = useCallback(
    () => rowRef.current?.querySelector<HTMLElement>('[data-sidebar-nav-item]') ?? null,
    [],
  );

  useEffect(() => {
    if (!isEditingTitle) {
      setDraftTitle(conversation.title);
    }
  }, [conversation.title, isEditingTitle]);

  useEffect(() => {
    if (!isEditingTitle) return;
    const frame = window.requestAnimationFrame(() => {
      inlineInputRef.current?.focus();
      inlineInputRef.current?.select();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [isEditingTitle]);

  const startInlineRename = useCallback(() => {
    if (disabled) return;
    if (!onInlineRename) {
      onRename();
      return;
    }
    setDraftTitle(conversation.title);
    setIsEditingTitle(true);
  }, [conversation.title, disabled, onInlineRename, onRename]);

  const cancelInlineRename = useCallback(() => {
    if (isSavingTitle) return;
    setDraftTitle(conversation.title);
    setIsEditingTitle(false);
    window.requestAnimationFrame(() => primaryButton()?.focus());
  }, [conversation.title, isSavingTitle, primaryButton]);

  const submitInlineRename = async (event?: FormEvent<HTMLFormElement>) => {
    event?.preventDefault();
    if (isSavingTitle || !onInlineRename) return;
    const normalized = draftTitle.trim();
    if (!normalized || normalized === conversation.title) {
      setDraftTitle(conversation.title);
      setIsEditingTitle(false);
      window.requestAnimationFrame(() => primaryButton()?.focus());
      return;
    }
    setIsSavingTitle(true);
    try {
      await onInlineRename(normalized);
      setIsEditingTitle(false);
      window.requestAnimationFrame(() => primaryButton()?.focus());
    } catch {
      inlineInputRef.current?.focus();
    } finally {
      setIsSavingTitle(false);
    }
  };

  useLayoutEffect(() => {
    if (!menuOpen) return;

    const updatePosition = () => {
      const viewportMargin = 8;
      const anchorGap = 4;
      const viewportWidth = document.documentElement.clientWidth;
      const viewportHeight = document.documentElement.clientHeight;
      const menu = menuRef.current;
      const maxWidth = Math.max(0, viewportWidth - viewportMargin * 2);
      const minWidth = Math.min(192, maxWidth);
      const maxMenuHeight = Math.min(viewportHeight * 0.7, 384);
      const contentHeight = Math.min(menu?.scrollHeight || maxMenuHeight, maxMenuHeight);
      const menuWidth = Math.min(menu?.offsetWidth || 208, maxWidth);
      let anchorTop: number;
      let anchorBottom: number;
      let anchorRight: number;

      if (menuAnchor.kind === 'point') {
        anchorTop = menuAnchor.y;
        anchorBottom = menuAnchor.y;
        anchorRight = menuAnchor.x;
      } else {
        const trigger = menuTrigger() ?? rowRef.current;
        if (!trigger) return;
        const bounds = trigger.getBoundingClientRect();
        anchorTop = bounds.top;
        anchorBottom = bounds.bottom;
        anchorRight = bounds.right;
      }

      const availableBelow = Math.max(0, viewportHeight - anchorBottom - anchorGap - viewportMargin);
      const availableAbove = Math.max(0, anchorTop - anchorGap - viewportMargin);
      const placement = availableBelow >= contentHeight || availableBelow >= availableAbove ? 'below' : 'above';
      const availableHeight = placement === 'below' ? availableBelow : availableAbove;
      const maxHeight = Math.min(contentHeight, availableHeight);
      const visibleHeight = Math.min(menu?.offsetHeight || contentHeight, maxHeight);
      const top = placement === 'below' ? anchorBottom + anchorGap : anchorTop - anchorGap - visibleHeight;
      const left = Math.max(
        viewportMargin,
        Math.min(viewportWidth - menuWidth - viewportMargin, anchorRight - menuWidth),
      );
      setMenuPosition((current) =>
        current !== null &&
        current.top === top &&
        current.left === left &&
        current.minWidth === minWidth &&
        current.maxWidth === maxWidth &&
        current.maxHeight === maxHeight &&
        current.placement === placement
          ? current
          : { top, left, minWidth, maxWidth, maxHeight, placement },
      );
    };

    const closeOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (menuRef.current?.contains(target)) return;
      if (event.button === 0 && menuTrigger()?.contains(target)) return;
      setMenuOpen(false);
    };

    const closeOnOutsideContextMenu = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (rowRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setMenuOpen(false);
    };

    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      setMenuOpen(false);
      menuTrigger()?.focus();
    };

    updatePosition();
    const resizeObserver = new ResizeObserver(updatePosition);
    const focusFrame = window.requestAnimationFrame(() => {
      updatePosition();
      if (menuRef.current) resizeObserver.observe(menuRef.current);
      const trigger = menuTrigger();
      if (trigger) resizeObserver.observe(trigger);
      const firstEnabledItem = Array.from(
        menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [],
      ).find((item) => !item.matches(':disabled'));
      firstEnabledItem?.focus();
    });
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    document.addEventListener('contextmenu', closeOnOutsideContextMenu, true);
    document.addEventListener('keydown', closeOnEscape);
    return () => {
      window.cancelAnimationFrame(focusFrame);
      resizeObserver.disconnect();
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
      document.removeEventListener('pointerdown', closeOnOutsidePointer);
      document.removeEventListener('contextmenu', closeOnOutsideContextMenu, true);
      document.removeEventListener('keydown', closeOnEscape);
    };
  }, [menuAnchor, menuOpen, menuTrigger]);

  useEffect(() => {
    if (!menuOpen && !menuIsPresent) setMenuPosition(null);
  }, [menuIsPresent, menuOpen]);

  const runAction = (action: () => void) => {
    setMenuOpen(false);
    menuTrigger()?.focus();
    action();
  };

  const moveMenuFocus = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Tab') {
      event.preventDefault();
      setMenuOpen(false);
      const trigger = menuTrigger();
      const focusableSelector =
        'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';
      const focusableElements = Array.from(document.querySelectorAll<HTMLElement>(focusableSelector)).filter(
        (element) => !menuRef.current?.contains(element) && element.getClientRects().length > 0,
      );
      const triggerIndex = trigger ? focusableElements.indexOf(trigger) : -1;
      const targetIndex = event.shiftKey
        ? triggerIndex > 0
          ? triggerIndex - 1
          : focusableElements.length - 1
        : triggerIndex >= 0 && triggerIndex < focusableElements.length - 1
          ? triggerIndex + 1
          : 0;
      focusableElements[targetIndex]?.focus();
      return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    const menu = menuRef.current;
    if (!menu) return;
    const items = Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]')).filter(
      (item) => !item.matches(':disabled') && (item.closest('details')?.open ?? true),
    );
    if (items.length === 0) return;
    event.preventDefault();
    const activeItem = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const currentIndex = activeItem === null ? -1 : items.indexOf(activeItem);
    const nextIndex =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? items.length - 1
          : event.key === 'ArrowDown'
            ? (currentIndex + 1) % items.length
            : (currentIndex <= 0 ? items.length : currentIndex) - 1;
    items[nextIndex]?.focus();
  };

  const handleContextMenu = (event: ReactMouseEvent<HTMLLIElement>) => {
    if (disabled || isEditingTitle) return;
    event.preventDefault();
    event.stopPropagation();
    openMenu(
      event.clientX === 0 && event.clientY === 0
        ? { kind: 'trigger' }
        : { kind: 'point', x: event.clientX, y: event.clientY },
    );
  };

  const handleRowKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (disabled || isEditingTitle) return;
    if (event.key === 'F2') {
      event.preventDefault();
      event.stopPropagation();
      startInlineRename();
      return;
    }
    if (event.key === 'Delete') {
      event.preventDefault();
      event.stopPropagation();
      onDelete();
      return;
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onSelect();
    }
  };

  const handleDragOverRow = (event: DragEvent<HTMLLIElement>) => {
    if (!onReorderConversation || disabled || archived) return;
    if (!event.dataTransfer.types.includes('application/x-openchat-conversation')) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
    setIsReorderDropTarget(true);
  };

  const handleDropRow = (event: DragEvent<HTMLLIElement>) => {
    if (!onReorderConversation || disabled || archived) return;
    const draggedId = event.dataTransfer.getData('application/x-openchat-conversation');
    if (!draggedId || draggedId === conversation.id) {
      setIsReorderDropTarget(false);
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    setIsReorderDropTarget(false);
    onReorderConversation(draggedId, conversation.id);
  };

  const canDrag = !disabled && !archived && !isEditingTitle;

  const handleStartDrag = (event: DragEvent<HTMLElement>) => {
    event.stopPropagation();
    event.dataTransfer.setData('application/x-openchat-conversation', conversation.id);
    event.dataTransfer.setData('text/plain', conversation.id);
    event.dataTransfer.effectAllowed = 'all';
    setSidebarDraggingConversationId(conversation.id);
    window.dispatchEvent(
      new CustomEvent('openchat-conversation-drag-state', {
        detail: { conversationId: conversation.id, dragging: true },
      }),
    );
  };

  const handleEndDrag = () => {
    setIsReorderDropTarget(false);
    window.setTimeout(() => {
      setSidebarDraggingConversationId(null);
      window.dispatchEvent(
        new CustomEvent('openchat-conversation-drag-state', {
          detail: { conversationId: null, dragging: false },
        }),
      );
    }, 200);
  };

  return (
    <li
      className={`conversation-sidebar-virtual-row group relative flex w-full min-w-0 max-w-full items-center rounded-button transition-colors duration-quick select-none cursor-pointer hover:z-10 focus-within:z-10 ${
        viewMode === 'detailed' ? 'min-h-12' : 'min-h-9'
      } ${isReorderDropTarget ? 'ring-1 ring-accent/70 bg-accent/5' : ''} ${
        active
          ? 'bg-selected text-text shadow-2xs'
          : menuOpen
            ? 'bg-hover text-text'
            : 'text-text/80 hover:bg-hover/85 hover:text-text focus-within:bg-hover/85 focus-within:text-text'
      }`}
      data-view={viewMode}
      draggable={canDrag}
      onContextMenu={handleContextMenu}
      onDragEnd={handleEndDrag}
      onDragLeave={(event) => {
        if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) return;
        setIsReorderDropTarget(false);
      }}
      onDragOver={handleDragOverRow}
      onDragStart={handleStartDrag}
      onDrop={handleDropRow}
      ref={rowRef}
    >
      {isEditingTitle ? (
        <form
          className="flex min-h-9 min-w-0 flex-1 items-center gap-1 px-2 py-1"
          onSubmit={(event) => void submitInlineRename(event)}
        >
          <input
            aria-label={t('conversationTitle')}
            className="h-7 min-w-0 flex-1 rounded-control border border-accent/60 bg-surface px-2 text-settings-description text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            disabled={isSavingTitle}
            maxLength={500}
            onBlur={() => void submitInlineRename()}
            onChange={(event) => setDraftTitle(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                event.preventDefault();
                event.stopPropagation();
                cancelInlineRename();
              }
            }}
            ref={inlineInputRef}
            value={draftTitle}
          />
          <button
            aria-label={t('save')}
            className="flex size-6 shrink-0 items-center justify-center rounded-control text-accent hover:bg-hover"
            disabled={isSavingTitle}
            onMouseDown={(event) => event.preventDefault()}
            type="submit"
          >
            <Check aria-hidden="true" size={13} />
          </button>
          <button
            aria-label={t('cancel')}
            className="flex size-6 shrink-0 items-center justify-center rounded-control text-secondary-icon hover:bg-hover hover:text-text"
            disabled={isSavingTitle}
            onClick={cancelInlineRename}
            onMouseDown={(event) => event.preventDefault()}
            type="button"
          >
            <X aria-hidden="true" size={13} />
          </button>
        </form>
      ) : (
        <>
          {/* biome-ignore lint/a11y/useSemanticElements: The row contains nested action buttons, so it cannot be a native button. */}
          <div
            aria-current={active ? 'page' : undefined}
            className={`min-w-0 flex-1 flex items-center justify-start gap-2 rounded-button bg-transparent px-3 text-left text-settings-description transition-all duration-quick hover:bg-transparent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
              canDrag ? 'cursor-grab active:cursor-grabbing' : ''
            } ${viewMode === 'detailed' ? 'min-h-12 py-1.5' : 'min-h-9 py-1.5'} ${
              active
                ? 'font-medium text-text'
                : 'font-normal text-text/80 group-hover:text-text group-focus-within:text-text'
            } ${
              menuOpen
                ? archived
                  ? 'pr-16'
                  : 'pr-22'
                : archived
                  ? 'group-hover:pr-16 group-focus-within:pr-16'
                  : 'group-hover:pr-22 group-focus-within:pr-22'
            }`}
            data-sidebar-nav-item="true"
            draggable={canDrag}
            onClick={onSelect}
            onDoubleClick={(event) => {
              event.preventDefault();
              startInlineRename();
            }}
            onDragEnd={handleEndDrag}
            onDragStart={handleStartDrag}
            onKeyDown={handleRowKeyDown}
            role="button"
            tabIndex={disabled ? -1 : 0}
            title={conversation.title}
          >
            {isAiWorking ? (
              <span
                aria-label={t('aiWorkingIndicatorLabel')}
                className="relative flex size-4 shrink-0 items-center justify-center text-accent"
                role="status"
              >
                <svg aria-hidden="true" className="openchat-ai-ring-spinner size-3.5" fill="none" viewBox="0 0 16 16">
                  <circle className="opacity-25" cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="2" />
                  <circle className="openchat-ai-ring-arc" cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="2" />
                </svg>
              </span>
            ) : null}
            <span className="min-w-0 flex-1">
              <span className={`block truncate ${isAiWorking ? 'openchat-title-shimmer font-medium' : ''}`}>
                {conversation.title}
              </span>
              {viewMode === 'detailed' ? (
                <span className="mt-0.5 flex items-center gap-1.5 truncate text-caption font-normal text-secondary-text/75">
                  <span>{formattedUpdatedTime}</span>
                </span>
              ) : null}
            </span>
          </div>
          <div
            className={`conversation-sidebar-row-actions absolute right-1.5 inset-y-0 flex items-center gap-0.5 transition-opacity duration-quick ${
              menuOpen
                ? 'pointer-events-auto opacity-100'
                : 'pointer-events-none opacity-0 group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100'
            }`}
            draggable={false}
            role="toolbar"
            onDragStart={(event) => {
              event.preventDefault();
              event.stopPropagation();
            }}
            onMouseDown={(event) => event.stopPropagation()}
            onPointerDown={(event) => event.stopPropagation()}
          >
                {!archived ? (
                  <Button
                    aria-label={conversation.isPinned ? t('unpinConversation') : t('pinConversation')}
                    className="conversation-sidebar-pin-action size-7 shrink-0 rounded-control text-secondary-icon hover:bg-surface/80 hover:text-text"
                    disabled={disabled}
                    onClick={onTogglePinned}
                    size="icon"
                    title={conversation.isPinned ? t('unpinConversationButtonTitle') : t('pinConversationButtonTitle')}
                    variant="ghost"
                  >
                    {conversation.isPinned ? (
                      <Pin aria-hidden="true" size={13.5} />
                    ) : (
                      <PinOff aria-hidden="true" size={13.5} />
                    )}
                  </Button>
                ) : null}
                <Button
                  aria-label={archived ? t('restoreConversation') : t('archiveConversation')}
                  className="size-7 shrink-0 rounded-control text-secondary-icon hover:bg-surface/80 hover:text-text"
                  disabled={disabled}
                  onClick={onArchive}
                  size="icon"
                  title={archived ? t('restoreConversation') : t('archiveConversation')}
                  variant="ghost"
                >
                  {archived ? (
                    <ArchiveRestore aria-hidden="true" size={13.5} />
                  ) : (
                    <Archive aria-hidden="true" size={13.5} />
                  )}
                </Button>
                <Button
                  aria-expanded={menuOpen}
                  aria-haspopup="menu"
                  aria-label={t('moreOptions')}
                  className={`conversation-sidebar-overflow-action size-7 shrink-0 rounded-control hover:bg-surface/80 hover:text-text ${
                    menuOpen ? 'bg-surface/90 text-text' : 'text-secondary-icon'
                  }`}
                  data-conversation-menu-trigger="true"
                  onClick={() => {
                    if (menuOpen) setMenuOpen(false);
                    else openMenu({ kind: 'trigger' });
                  }}
                  size="icon"
                  title={t('conversationMoreOptionsButtonTitle')}
                  variant="ghost"
                >
                  <MoreHorizontal aria-hidden="true" size={14} />
                </Button>
          </div>
        </>
      )}
      {menuOpen || menuIsPresent
        ? createPortal(
            <div
              aria-hidden={!menuOpen}
              aria-label={t('moreOptions')}
              className={`openchat-dropdown-surface openchat-popover-enter ${menuPosition?.placement === 'above' ? 'openchat-popover-enter--bottom-right' : 'openchat-popover-enter--right'} fixed z-50 grid max-h-conversation-menu min-w-48 max-w-message-menu gap-0.5 overflow-y-auto p-1.5`}
              data-state={menuOpen ? 'open' : 'closed'}
              inert={!menuOpen}
              onContextMenu={(event) => event.preventDefault()}
              onKeyDown={moveMenuFocus}
              ref={menuRef}
              role="menu"
              style={{
                top: menuPosition?.top ?? 0,
                left: menuPosition?.left ?? 0,
                minWidth: menuPosition?.minWidth,
                maxWidth: menuPosition?.maxWidth,
                maxHeight: menuPosition?.maxHeight,
                visibility: menuPosition === null ? 'hidden' : 'visible',
              }}
            >
                  {!archived ? (
                    <>
                      <MenuAction
                        disabled={disabled}
                        icon={PanelRight}
                        label={t('openInWorkspacePanel')}
                        onClick={() =>
                          runAction(() => {
                            window.dispatchEvent(
                              new CustomEvent('openchat-open-panel-conversation', {
                                detail: { conversationId: conversation.id },
                              }),
                            );
                          })
                        }
                      />
                      <MenuAction
                        disabled={disabled}
                        icon={conversation.isPinned ? PinOff : Pin}
                        label={conversation.isPinned ? t('unpinConversation') : t('pinConversation')}
                        onClick={() => runAction(onTogglePinned)}
                      />
                    </>
                  ) : null}
                  <MenuAction
                    disabled={disabled}
                    icon={Pencil}
                    label={t('renameConversation')}
                    onClick={() => {
                      setMenuOpen(false);
                      startInlineRename();
                    }}
                  />
                  {onDuplicate ? (
                    <MenuAction
                      disabled={disabled}
                      icon={Copy}
                      label={t('duplicateConversation')}
                      onClick={() => runAction(onDuplicate)}
                    />
                  ) : null}
                  <div aria-hidden="true" className="my-0.5 border-t border-border" />
                  {allowMove && currentProjectId ? (
                    <MenuAction
                      disabled={disabled}
                      icon={MessageCircle}
                      label={t('moveToChats')}
                      onClick={() => runAction(onMoveToChats)}
                    />
                  ) : null}
                  {allowMove && onMoveToFolder && currentFolderId ? (
                    <MenuAction
                      disabled={disabled}
                      icon={FolderMinus}
                      label={t('removeFromFolder')}
                      onClick={() => runAction(() => onMoveToFolder(null))}
                    />
                  ) : null}
                  {allowMove && onMoveToFolder && availableFolders.length > 0 ? (
                    <details className="openchat-native-disclosure group/folder relative">
                      <summary
                        className="flex min-h-8 cursor-pointer list-none items-center gap-2 rounded-control px-2.5 text-left text-xs font-medium text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus [&::-webkit-details-marker]:hidden"
                        role="menuitem"
                        tabIndex={0}
                      >
                        <Folder aria-hidden="true" size={14} />
                        <span>{t('moveConversationToFolder')}</span>
                      </summary>
                      <div className="mt-0.5 grid max-h-40 gap-0.5 overflow-y-auto border-l border-border ml-3.5 pl-2">
                        {availableFolders.map((folder) => (
                          <button
                            className="min-h-7 truncate rounded-control px-2 text-left text-xs text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                            disabled={disabled}
                            key={folder.id}
                            onClick={() => runAction(() => onMoveToFolder(folder.id))}
                            role="menuitem"
                            type="button"
                          >
                            {folder.name}
                          </button>
                        ))}
                      </div>
                    </details>
                  ) : null}
                  {allowMove && availableProjects.length > 0 ? (
                    <details className="openchat-native-disclosure group/move relative">
                      <summary
                        className="flex min-h-8 cursor-pointer list-none items-center gap-2 rounded-control px-2.5 text-left text-xs font-medium text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus [&::-webkit-details-marker]:hidden"
                        role="menuitem"
                        tabIndex={0}
                      >
                        <FolderInput aria-hidden="true" size={14} />
                        <span>{t('moveConversationToProject')}</span>
                      </summary>
                      <div className="mt-0.5 grid max-h-40 gap-0.5 overflow-y-auto border-l border-border ml-3.5 pl-2">
                        {availableProjects.map((project) => (
                          <button
                            className="min-h-7 truncate rounded-control px-2 text-left text-xs text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                            disabled={disabled}
                            key={project.id}
                            onClick={() => runAction(() => onMoveToProject(project.id))}
                            role="menuitem"
                            type="button"
                          >
                            {project.name}
                          </button>
                        ))}
                      </div>
                    </details>
                  ) : null}
                  <details className="openchat-native-disclosure group/export relative">
                    <summary
                      className="flex min-h-8 cursor-pointer list-none items-center gap-2 rounded-control px-2.5 text-left text-xs font-medium text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus [&::-webkit-details-marker]:hidden"
                      role="menuitem"
                      tabIndex={0}
                    >
                      <Download aria-hidden="true" size={14} />
                      <span>{t('exportConversation')}</span>
                    </summary>
                    <div className="mt-0.5 grid max-h-40 gap-0.5 overflow-y-auto border-l border-border ml-3.5 pl-2">
                      <button
                        className="min-h-7 truncate rounded-control px-2 text-left text-xs text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                        disabled={disabled}
                        onClick={() => runAction(() => onExport('md'))}
                        role="menuitem"
                        type="button"
                      >
                        {t('exportFormatMarkdown')}
                      </button>
                      <button
                        className="min-h-7 truncate rounded-control px-2 text-left text-xs text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                        disabled={disabled}
                        onClick={() => runAction(() => onExport('html'))}
                        role="menuitem"
                        type="button"
                      >
                        {t('exportFormatHtml')}
                      </button>
                      <button
                        className="min-h-7 truncate rounded-control px-2 text-left text-xs text-secondary-text transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                        disabled={disabled}
                        onClick={() => runAction(() => onExport('json'))}
                        role="menuitem"
                        type="button"
                      >
                        {t('exportFormatJson')}
                      </button>
                    </div>
                  </details>
                  <div aria-hidden="true" className="my-0.5 border-t border-border" />
                  <MenuAction
                    disabled={disabled}
                    icon={archived ? ArchiveRestore : Archive}
                    label={archived ? t('restoreConversation') : t('archiveConversation')}
                    onClick={() => runAction(onArchive)}
                  />
                  <MenuAction
                    destructive
                    disabled={disabled}
                    icon={Trash2}
                    label={t('deleteConversation')}
                    onClick={() => runAction(onDelete)}
                  />
            </div>,
            document.body,
          )
        : null}
    </li>
  );
}

interface MenuActionProps {
  icon: LucideIcon;
  label: string;
  disabled: boolean;
  destructive?: boolean;
  onClick(): void;
}

function MenuAction({ icon: Icon, label, disabled, destructive = false, onClick }: MenuActionProps) {
  return (
    <button
      className={`flex min-h-8 items-center gap-2 rounded-control px-2.5 text-left text-xs font-medium transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-45 ${destructive ? 'text-destructive' : 'text-secondary-text hover:text-text'}`}
      disabled={disabled}
      onClick={onClick}
      role="menuitem"
      type="button"
    >
      <Icon aria-hidden="true" size={14} />
      {label}
    </button>
  );
}
