import type { LucideIcon } from 'lucide-react';
import {
  Archive,
  ArchiveRestore,
  Bookmark,
  BookmarkCheck,
  Download,
  FolderInput,
  MessageCircle,
  MoreHorizontal,
  Pencil,
  Pin,
  PinOff,
  Tags,
  Trash2,
} from 'lucide-react';
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import type { ChatConversation, ChatProject, ExportFormat } from '@/modules/conversations/data/chatStorage';
import { useTransitionPresence } from '@/shared/hooks/useTransitionPresence';

interface ConversationSidebarRowProps {
  conversation: ChatConversation;
  projects: readonly ChatProject[];
  active: boolean;
  archived: boolean;
  currentProjectId: string | null;
  selectionMode: boolean;
  selectedForBatch: boolean;
  allowSelection: boolean;
  allowMove: boolean;
  disabled: boolean;
  onSelect(): void;
  onToggleSelection(): void;
  onTogglePinned(): void;
  onToggleBookmark(): void;
  onEditTags(): void;
  onRename(): void;
  onExport(format?: ExportFormat): void;
  onArchive(): void;
  onDelete(): void;
  onMoveToProject(projectId: string): void;
  onMoveToChats(): void;
}

export function ConversationSidebarRow({
  conversation,
  projects,
  active,
  archived,
  currentProjectId,
  selectionMode,
  selectedForBatch,
  allowSelection,
  allowMove,
  disabled,
  onSelect,
  onToggleSelection,
  onTogglePinned,
  onToggleBookmark,
  onEditTags,
  onRename,
  onExport,
  onArchive,
  onDelete,
  onMoveToProject,
  onMoveToChats,
}: ConversationSidebarRowProps) {
  const { t } = useTranslation();
  const availableProjects = projects.filter((project) => project.id !== currentProjectId);

  const [menuOpen, setMenuOpen] = useState(false);
  const menuIsPresent = useTransitionPresence(menuOpen);
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number; placement: 'above' | 'below' } | null>(
    null,
  );
  const rowRef = useRef<HTMLLIElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const menuTrigger = useCallback(
    () => rowRef.current?.querySelector<HTMLButtonElement>('[data-conversation-menu-trigger]') ?? null,
    [],
  );

  useEffect(() => {
    if (!menuOpen) return;

    const updatePosition = () => {
      const trigger = menuTrigger();
      if (!trigger) return;
      const bounds = trigger.getBoundingClientRect();
      const menu = menuRef.current;
      const menuHeight = menu?.offsetHeight ?? Math.min(window.innerHeight * 0.7, 384);
      const menuWidth = menu?.offsetWidth ?? 192;
      const placement = bounds.bottom + menuHeight + 8 <= window.innerHeight ? 'below' : 'above';
      const top = placement === 'below' ? bounds.bottom + 4 : Math.max(8, bounds.top - menuHeight - 4);
      const left = Math.max(8, Math.min(window.innerWidth - menuWidth - 8, bounds.right - menuWidth));
      setMenuPosition((current) =>
        current !== null && current.top === top && current.left === left && current.placement === placement
          ? current
          : { top, left, placement },
      );
    };

    const closeOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (menuTrigger()?.contains(target) || menuRef.current?.contains(target)) return;
      setMenuOpen(false);
    };

    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      setMenuOpen(false);
      menuTrigger()?.focus();
    };

    updatePosition();
    const focusFrame = window.requestAnimationFrame(() => {
      updatePosition();
      const firstEnabledItem = Array.from(
        menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [],
      ).find((item) => !item.matches(':disabled'));
      firstEnabledItem?.focus();
    });
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    document.addEventListener('keydown', closeOnEscape);
    return () => {
      window.cancelAnimationFrame(focusFrame);
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
      document.removeEventListener('pointerdown', closeOnOutsidePointer);
      document.removeEventListener('keydown', closeOnEscape);
    };
  }, [menuOpen, menuTrigger]);

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

  return (
    <li
      className="group relative flex min-h-11 items-center rounded-button hover:bg-hover focus-within:bg-hover"
      draggable={!disabled && allowMove && !archived && !selectionMode}
      ref={rowRef}
      onDragStart={(event) => {
        event.dataTransfer.setData('application/x-openchat-conversation', conversation.id);
        event.dataTransfer.effectAllowed = 'move';
      }}
    >
      {selectionMode && allowSelection ? (
        <input
          aria-label={t('selectConversation', { title: conversation.title })}
          checked={selectedForBatch}
          className="ml-2 size-4 shrink-0 accent-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          disabled={disabled}
          onChange={onToggleSelection}
          type="checkbox"
        />
      ) : null}
      <Button
        aria-current={!selectionMode && active ? 'page' : undefined}
        aria-pressed={selectionMode && allowSelection ? selectedForBatch : undefined}
        className={`min-h-11 min-w-0 flex-1 justify-start gap-2 rounded-button px-2.5 text-left text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${!selectionMode && active ? 'bg-selected text-text' : 'text-secondary-text hover:text-text'}`}
        disabled={disabled}
        onClick={selectionMode && allowSelection ? onToggleSelection : onSelect}
        size="small"
        variant="ghost"
      >
        <span className="min-w-0 flex-1 truncate">{conversation.title}</span>
        {conversation.tags.length > 0 ? (
          <span className="shrink-0" title={conversation.tags.join(', ')}>
            <Tags aria-hidden="true" className="text-secondary-icon" size={13} />
          </span>
        ) : null}
        {conversation.isBookmarked ? (
          <BookmarkCheck aria-label={t('conversationBookmarked')} className="shrink-0 text-accent" size={13} />
        ) : null}
      </Button>
      {!selectionMode && !archived ? (
        <Button
          aria-label={conversation.isPinned ? t('unpinConversation') : t('pinConversation')}
          className="conversation-sidebar-pin-action pointer-events-none size-11 shrink-0 opacity-0 transition-opacity group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100"
          disabled={disabled}
          onClick={onTogglePinned}
          size="icon"
          title={conversation.isPinned ? t('unpinConversation') : t('pinConversation')}
          variant="ghost"
        >
          {conversation.isPinned ? <Pin aria-hidden="true" size={15} /> : <PinOff aria-hidden="true" size={15} />}
        </Button>
      ) : null}
      {!selectionMode ? (
        <>
          <Button
            aria-expanded={menuOpen}
            aria-haspopup="menu"
            aria-label={t('moreOptions')}
            className="conversation-sidebar-overflow-action pointer-events-none mr-1 size-11 shrink-0 opacity-0 transition-opacity group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100"
            data-conversation-menu-trigger="true"
            onClick={() => setMenuOpen((current) => !current)}
            size="icon"
            title={t('moreOptions')}
            variant="ghost"
          >
            <MoreHorizontal aria-hidden="true" size={16} />
          </Button>
          {menuIsPresent && menuPosition !== null
            ? createPortal(
                <div
                  aria-hidden={!menuOpen}
                  aria-label={t('moreOptions')}
                  className={`openchat-popover-enter ${menuPosition.placement === 'above' ? 'openchat-popover-enter--bottom-right' : 'openchat-popover-enter--right'} fixed z-50 grid max-h-conversation-menu min-w-48 max-w-message-menu gap-0.5 overflow-y-auto rounded-card border border-border bg-raised p-1 shadow-lg`}
                  data-state={menuOpen ? 'open' : 'closed'}
                  inert={!menuOpen}
                  onKeyDown={moveMenuFocus}
                  ref={menuRef}
                  role="menu"
                  style={{ top: menuPosition.top, left: menuPosition.left }}
                >
                  <MenuAction
                    disabled={disabled}
                    icon={conversation.isBookmarked ? BookmarkCheck : Bookmark}
                    label={conversation.isBookmarked ? t('removeConversationBookmark') : t('bookmarkConversation')}
                    onClick={() => runAction(onToggleBookmark)}
                  />
                  <MenuAction
                    disabled={disabled}
                    icon={Tags}
                    label={t('editConversationTags')}
                    onClick={() => runAction(onEditTags)}
                  />
                  <MenuAction
                    disabled={disabled}
                    icon={Pencil}
                    label={t('renameConversation')}
                    onClick={() => runAction(onRename)}
                  />
                  <details className="openchat-native-disclosure group/export relative">
                    <summary
                      className="flex min-h-9 cursor-pointer list-none items-center gap-2 rounded-button px-2.5 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus [&::-webkit-details-marker]:hidden"
                      role="menuitem"
                      tabIndex={0}
                    >
                      <Download aria-hidden="true" size={15} />
                      <span>{t('exportConversation')}</span>
                    </summary>
                    <div className="grid max-h-40 gap-0.5 overflow-y-auto border-l border-border pl-2">
                      <button
                        className="min-h-8 truncate rounded-button px-2 text-left text-xs text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                        disabled={disabled}
                        onClick={() => runAction(() => onExport('md'))}
                        role="menuitem"
                        type="button"
                      >
                        {t('exportFormatMarkdown')}
                      </button>
                      <button
                        className="min-h-8 truncate rounded-button px-2 text-left text-xs text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                        disabled={disabled}
                        onClick={() => runAction(() => onExport('html'))}
                        role="menuitem"
                        type="button"
                      >
                        {t('exportFormatHtml')}
                      </button>
                      <button
                        className="min-h-8 truncate rounded-button px-2 text-left text-xs text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                        disabled={disabled}
                        onClick={() => runAction(() => onExport('json'))}
                        role="menuitem"
                        type="button"
                      >
                        {t('exportFormatJson')}
                      </button>
                    </div>
                  </details>
                  {allowMove && currentProjectId ? (
                    <MenuAction
                      disabled={disabled}
                      icon={MessageCircle}
                      label={t('moveToChats')}
                      onClick={() => runAction(onMoveToChats)}
                    />
                  ) : null}
                  {allowMove && availableProjects.length > 0 ? (
                    <details className="openchat-native-disclosure group/move relative">
                      <summary
                        className="flex min-h-9 cursor-pointer list-none items-center gap-2 rounded-button px-2.5 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus [&::-webkit-details-marker]:hidden"
                        role="menuitem"
                        tabIndex={0}
                      >
                        <FolderInput aria-hidden="true" size={15} />
                        <span>{t('moveConversationToProject')}</span>
                      </summary>
                      <div className="grid max-h-40 gap-0.5 overflow-y-auto border-l border-border pl-2">
                        {availableProjects.map((project) => (
                          <button
                            className="min-h-8 truncate rounded-button px-2 text-left text-xs text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
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
        </>
      ) : null}
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
      className={`flex min-h-9 items-center gap-2 rounded-button px-2.5 text-left text-sm hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-45 ${destructive ? 'text-destructive' : 'text-secondary-text hover:text-text'}`}
      disabled={disabled}
      onClick={onClick}
      role="menuitem"
      type="button"
    >
      <Icon aria-hidden="true" size={15} />
      {label}
    </button>
  );
}
