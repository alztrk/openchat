import { Check, ChevronDown, Code, Copy, FileText, Type } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/design-system';
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
  const resetTimer = useRef<number | undefined>(undefined);
  const containsCode = hasCodeBlocks(content);

  useEffect(
    () => () => {
      if (resetTimer.current !== undefined) window.clearTimeout(resetTimer.current);
    },
    [],
  );

  const performCopy = async (mode: CopyMode) => {
    let textToCopy = '';
    if (mode === 'clean-markdown') {
      textToCopy = cleanMarkdown(content);
    } else if (mode === 'code-blocks') {
      textToCopy = extractCodeBlocks(content);
    } else {
      textToCopy = markdownToPlainText(content);
    }

    if (!textToCopy && mode === 'code-blocks') return;

    try {
      await navigator.clipboard.writeText(textToCopy);
      setCopyResult('copied');
    } catch {
      setCopyResult('failed');
    }

    if (resetTimer.current !== undefined) window.clearTimeout(resetTimer.current);
    resetTimer.current = window.setTimeout(() => setCopyResult('idle'), 2000);
  };

  return (
    <div className="relative inline-flex items-center rounded-button">
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
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            aria-label={t('copyOptions')}
            className="-ml-1 h-8 w-4 px-0 opacity-70 group-hover:opacity-100"
            size="icon"
            title={t('copyOptions')}
            variant="ghost"
          >
            <ChevronDown aria-hidden="true" size={12} />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" aria-label={t('copyOptions')} className="min-w-56">
          <DropdownMenuItem onSelect={() => void performCopy('clean-markdown')}>
            <FileText aria-hidden="true" size={15} />
            <span>{t('copyCleanMarkdown')}</span>
          </DropdownMenuItem>
          <DropdownMenuItem
            disabled={!containsCode}
            onSelect={() => void performCopy('code-blocks')}
            title={!containsCode ? t('copyCodeBlocksNone') : undefined}
          >
            <Code aria-hidden="true" size={15} />
            <span>{t('copyCodeBlocksOnly')}</span>
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void performCopy('plain-text')}>
            <Type aria-hidden="true" size={15} />
            <span>{t('copyPlainText')}</span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      {copyResult === 'failed' ? (
        <span className="sr-only" role="status">
          {t('messageCopyFailed')}
        </span>
      ) : null}
    </div>
  );
}
