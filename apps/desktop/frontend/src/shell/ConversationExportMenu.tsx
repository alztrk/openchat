import { FileCode, FileJson, FileText, Share } from 'lucide-react';
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import { Tooltip } from '@/design-system/Tooltip';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';
import { type ExportFormat, exportConversationToFile } from '@/modules/conversations/data/conversationExport';
import { conversationMarkdownLabels } from '@/modules/conversations/data/conversationMarkdownLabels';

export interface ConversationExportMenuProps {
  conversation: ChatConversation;
  disabled?: boolean;
}

export function ConversationExportMenu({ conversation, disabled = false }: ConversationExportMenuProps) {
  const { t, i18n } = useTranslation();
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number; placement: 'above' | 'below' } | null>(
    null,
  );
  const [exporting, setExporting] = useState(false);

  const triggerRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const updatePosition = useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return;
    const bounds = trigger.getBoundingClientRect();
    const menu = menuRef.current;
    const menuHeight = menu?.offsetHeight ?? 230;
    const menuWidth = menu?.offsetWidth ?? 320;
    const placement = bounds.bottom + menuHeight + 8 <= window.innerHeight ? 'below' : 'above';
    const top = placement === 'below' ? bounds.bottom + 6 : Math.max(8, bounds.top - menuHeight - 6);
    const left = Math.max(8, Math.min(window.innerWidth - menuWidth - 8, bounds.right - menuWidth));
    setMenuPosition({ top, left, placement });
  }, []);

  useEffect(() => {
    if (!menuOpen) return;

    const closeOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (triggerRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setMenuOpen(false);
    };

    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      setMenuOpen(false);
      triggerRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
    };

    updatePosition();
    const frameId = window.requestAnimationFrame(() => {
      updatePosition();
      const firstItem = menuRef.current?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)');
      firstItem?.focus();
    });

    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    document.addEventListener('keydown', closeOnEscape);

    return () => {
      window.cancelAnimationFrame(frameId);
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
      document.removeEventListener('pointerdown', closeOnOutsidePointer);
      document.removeEventListener('keydown', closeOnEscape);
    };
  }, [menuOpen, updatePosition]);

  const handleExport = async (format: ExportFormat) => {
    setMenuOpen(false);
    setExporting(true);
    try {
      const labels = conversationMarkdownLabels(conversation, t, i18n.language);
      await exportConversationToFile(conversation, format, labels, t('exportConversation'));
    } finally {
      setExporting(false);
      triggerRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
    }
  };

  const handleMenuKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Tab') {
      event.preventDefault();
      setMenuOpen(false);
      triggerRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
      return;
    }
    const items = Array.from(menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)') ?? []);
    if (items.length === 0) return;
    const activeElement = document.activeElement;
    const currentIndex = activeElement instanceof HTMLElement ? items.indexOf(activeElement) : -1;

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      const nextIndex = currentIndex < items.length - 1 ? currentIndex + 1 : 0;
      items[nextIndex]?.focus();
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      const prevIndex = currentIndex > 0 ? currentIndex - 1 : items.length - 1;
      items[prevIndex]?.focus();
    }
  };

  return (
    <div className="relative inline-flex items-center" ref={triggerRef}>
      <Tooltip label={t('exportConversation')}>
        <Button
          aria-expanded={menuOpen}
          aria-haspopup="menu"
          aria-label={t('exportConversation')}
          className={`size-8 rounded-control ${
            menuOpen ? 'bg-hover text-text' : 'text-secondary-icon hover:bg-hover hover:text-text'
          }`}
          disabled={disabled || exporting}
          onClick={() => setMenuOpen((open) => !open)}
          size="icon"
          variant="ghost"
        >
          <Share aria-hidden="true" size={15} />
        </Button>
      </Tooltip>

      {menuOpen && menuPosition !== null
        ? createPortal(
            <div
              aria-label={t('exportConversation')}
              className="openchat-popover-enter fixed z-50 w-80 min-w-80 rounded-card border border-border/80 bg-raised/95 p-1.5 shadow-xl backdrop-blur-md"
              onKeyDown={handleMenuKeyDown}
              ref={menuRef}
              role="menu"
              style={{ top: menuPosition.top, left: menuPosition.left }}
            >
              <div className="flex items-center justify-between px-2.5 py-1.5 text-xs font-medium text-secondary-text">
                <span>{t('exportConversation')}</span>
                <span className="font-mono text-[11px] text-secondary-text/60">3 format</span>
              </div>
              <div className="my-1 h-px bg-border/60" />

              <button
                className="group flex w-full items-center gap-3 rounded-button px-2.5 py-2 text-left transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                onClick={() => void handleExport('md')}
                role="menuitem"
                type="button"
              >
                <div className="flex size-9 shrink-0 items-center justify-center rounded-control bg-accent/10 text-accent transition-colors group-hover:bg-accent/20">
                  <FileText aria-hidden="true" size={17} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-1.5">
                    <span className="truncate text-sm font-medium text-text">{t('exportFormatMarkdownTitle')}</span>
                    <span className="shrink-0 rounded bg-surface px-1.5 py-0.5 font-mono text-[11px] text-secondary-text">
                      .md
                    </span>
                  </div>
                  <p className="truncate text-xs text-secondary-text">{t('exportFormatMarkdownHint')}</p>
                </div>
              </button>

              <button
                className="group flex w-full items-center gap-3 rounded-button px-2.5 py-2 text-left transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                onClick={() => void handleExport('html')}
                role="menuitem"
                type="button"
              >
                <div className="flex size-9 shrink-0 items-center justify-center rounded-control bg-info/10 text-info transition-colors group-hover:bg-info/20">
                  <FileCode aria-hidden="true" size={17} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-1.5">
                    <span className="truncate text-sm font-medium text-text">{t('exportFormatHtmlTitle')}</span>
                    <span className="shrink-0 rounded bg-surface px-1.5 py-0.5 font-mono text-[11px] text-secondary-text">
                      .html
                    </span>
                  </div>
                  <p className="truncate text-xs text-secondary-text">{t('exportFormatHtmlHint')}</p>
                </div>
              </button>

              <button
                className="group flex w-full items-center gap-3 rounded-button px-2.5 py-2 text-left transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                onClick={() => void handleExport('json')}
                role="menuitem"
                type="button"
              >
                <div className="flex size-9 shrink-0 items-center justify-center rounded-control bg-warning/10 text-warning transition-colors group-hover:bg-warning/20">
                  <FileJson aria-hidden="true" size={17} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-1.5">
                    <span className="truncate text-sm font-medium text-text">{t('exportFormatJsonTitle')}</span>
                    <span className="shrink-0 rounded bg-surface px-1.5 py-0.5 font-mono text-[11px] text-secondary-text">
                      .json
                    </span>
                  </div>
                  <p className="truncate text-xs text-secondary-text">{t('exportFormatJsonHint')}</p>
                </div>
              </button>
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}
