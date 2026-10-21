import { Check, ChevronDown, Code, Copy, FileText, Type } from 'lucide-react';
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import {
  cleanMarkdown,
  extractCodeBlocks,
  hasCodeBlocks,
  markdownToPlainText,
} from '@/modules/conversations/data/messageCopyUtils';

export interface MessageCopyMenuProps {
  content: string;
}

type CopyMode = 'clean-markdown' | 'code-blocks' | 'plain-text';

export function MessageCopyMenu({ content }: MessageCopyMenuProps) {
  const { t } = useTranslation();
  const [copyResult, setCopyResult] = useState<'idle' | 'copied' | 'failed'>('idle');
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number; placement: 'above' | 'below' } | null>(
    null,
  );

  const containerRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const resetTimer = useRef<number | undefined>(undefined);

  const containsCode = hasCodeBlocks(content);

  useEffect(
    () => () => {
      if (resetTimer.current !== undefined) window.clearTimeout(resetTimer.current);
    },
    [],
  );

  const updatePosition = useCallback(() => {
    const trigger = containerRef.current;
    if (!trigger) return;
    const bounds = trigger.getBoundingClientRect();
    const menu = menuRef.current;
    const menuHeight = menu?.offsetHeight ?? 128;
    const menuWidth = menu?.offsetWidth ?? 220;
    const placement = bounds.bottom + menuHeight + 8 <= window.innerHeight ? 'below' : 'above';
    const top = placement === 'below' ? bounds.bottom + 4 : Math.max(8, bounds.top - menuHeight - 4);
    const left = Math.max(8, Math.min(window.innerWidth - menuWidth - 8, bounds.left));
    setMenuPosition({ top, left, placement });
  }, []);

  useEffect(() => {
    if (!menuOpen) return;

    const closeOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (containerRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setMenuOpen(false);
    };

    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      setMenuOpen(false);
      containerRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
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

  const performCopy = async (mode: CopyMode) => {
    setMenuOpen(false);
    let textToCopy = '';
    if (mode === 'clean-markdown') {
      textToCopy = cleanMarkdown(content);
    } else if (mode === 'code-blocks') {
      textToCopy = extractCodeBlocks(content);
    } else if (mode === 'plain-text') {
      textToCopy = markdownToPlainText(content);
    }

    if (!textToCopy && mode === 'code-blocks') {
      return;
    }

    try {
      await navigator.clipboard.writeText(textToCopy);
      setCopyResult('copied');
    } catch {
      setCopyResult('failed');
    }

    if (resetTimer.current !== undefined) window.clearTimeout(resetTimer.current);
    resetTimer.current = window.setTimeout(() => setCopyResult('idle'), 2000);
    containerRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
  };

  const handleMenuKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Tab') {
      event.preventDefault();
      setMenuOpen(false);
      containerRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
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
    <div className="relative inline-flex items-center rounded-button" ref={containerRef}>
      <Button
        aria-label={t(copyResult === 'copied' ? 'messageCopied' : 'copyCleanMarkdown')}
        className="size-8 opacity-70 group-hover:opacity-100"
        onClick={() => void performCopy('clean-markdown')}
        size="icon"
        title={t(copyResult === 'copied' ? 'messageCopied' : 'copyCleanMarkdown')}
        variant="ghost"
      >
        {copyResult === 'copied' ? (
          <Check aria-hidden="true" className="text-success" size={16} />
        ) : (
          <Copy aria-hidden="true" size={16} />
        )}
      </Button>
      <Button
        aria-expanded={menuOpen}
        aria-haspopup="menu"
        aria-label={t('copyOptions')}
        className="-ml-1 h-8 w-4 px-0 opacity-70 group-hover:opacity-100"
        onClick={() => setMenuOpen((open) => !open)}
        size="icon"
        title={t('copyOptions')}
        variant="ghost"
      >
        <ChevronDown aria-hidden="true" size={12} />
      </Button>

      {menuOpen && menuPosition !== null
        ? createPortal(
            <div
              aria-label={t('copyOptions')}
              className="openchat-popover-enter fixed z-50 grid min-w-56 gap-0.5 rounded-card border border-border bg-raised p-1 shadow-lg"
              onKeyDown={handleMenuKeyDown}
              ref={menuRef}
              role="menu"
              style={{ top: menuPosition.top, left: menuPosition.left }}
            >
              <button
                className="flex min-h-9 items-center gap-2 rounded-button px-2.5 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                onClick={() => void performCopy('clean-markdown')}
                role="menuitem"
                type="button"
              >
                <FileText aria-hidden="true" size={15} />
                <span>{t('copyCleanMarkdown')}</span>
              </button>
              <button
                className="flex min-h-9 items-center gap-2 rounded-button px-2.5 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-40"
                disabled={!containsCode}
                onClick={() => void performCopy('code-blocks')}
                role="menuitem"
                title={!containsCode ? t('copyCodeBlocksNone') : undefined}
                type="button"
              >
                <Code aria-hidden="true" size={15} />
                <span>{t('copyCodeBlocksOnly')}</span>
              </button>
              <button
                className="flex min-h-9 items-center gap-2 rounded-button px-2.5 text-left text-sm text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                onClick={() => void performCopy('plain-text')}
                role="menuitem"
                type="button"
              >
                <Type aria-hidden="true" size={15} />
                <span>{t('copyPlainText')}</span>
              </button>
            </div>,
            document.body,
          )
        : null}

      {copyResult === 'failed' ? (
        <span className="sr-only" role="status">
          {t('messageCopyFailed')}
        </span>
      ) : null}
    </div>
  );
}
