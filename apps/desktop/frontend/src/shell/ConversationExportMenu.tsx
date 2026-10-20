import { Download, FileCode, FileJson, FileText } from 'lucide-react';
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import { Tooltip } from '@/design-system/Tooltip';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';
import {
  type ExportFormat,
  exportConversationToFile,
} from '@/modules/conversations/data/conversationExport';
import { conversationMarkdownLabels } from '@/modules/conversations/data/conversationMarkdownLabels';

export interface ConversationExportMenuProps {
  conversation: ChatConversation;
  disabled?: boolean;
}

export function ConversationExportMenu({ conversation, disabled = false }: ConversationExportMenuProps) {
  const { t } = useTranslation();
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
    const menuHeight = menu?.offsetHeight ?? 128;
    const menuWidth = menu?.offsetWidth ?? 240;
    const placement = bounds.bottom + menuHeight + 8 <= window.innerHeight ? 'below' : 'above';
    const top = placement === 'below' ? bounds.bottom + 4 : Math.max(8, bounds.top - menuHeight - 4);
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
      const labels = conversationMarkdownLabels(conversation, t);
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
    const currentIndex = items.indexOf(document.activeElement as HTMLElement);

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
          className="size-9"
          disabled={disabled || exporting}
          onClick={() => setMenuOpen((open) => !open)}
          size="icon"
          variant="ghost"
        >
          <Download aria-hidden="true" size={17} />
        </Button>
      </Tooltip>

      {menuOpen && menuPosition !== null
        ? createPortal(
            <div
              aria-label={t('exportConversation')}
              className="openchat-popover-enter fixed z-50 grid min-w-64 gap-0.5 rounded-card border border-border bg-raised p-1 shadow-lg"
              onKeyDown={handleMenuKeyDown}
              ref={menuRef}
              role="menu"
              style={{ top: menuPosition.top, left: menuPosition.left }}
            >
              <button
                className="flex min-h-9 items-center gap-2 rounded-button px-2.5 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                onClick={() => void handleExport('md')}
                role="menuitem"
                type="button"
              >
                <FileText aria-hidden="true" size={15} />
                <span>{t('exportFormatMarkdown')}</span>
              </button>
              <button
                className="flex min-h-9 items-center gap-2 rounded-button px-2.5 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                onClick={() => void handleExport('html')}
                role="menuitem"
                type="button"
              >
                <FileCode aria-hidden="true" size={15} />
                <span>{t('exportFormatHtml')}</span>
              </button>
              <button
                className="flex min-h-9 items-center gap-2 rounded-button px-2.5 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                onClick={() => void handleExport('json')}
                role="menuitem"
                type="button"
              >
                <FileJson aria-hidden="true" size={15} />
                <span>{t('exportFormatJson')}</span>
              </button>
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}
