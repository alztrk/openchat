import { FileCode, FileJson, FileText, Share } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  Tooltip,
} from '@/design-system';
import type { ChatConversation } from '@/modules/conversations/data/chatStorage';
import type { ExportFormat } from '@/modules/conversations/data/conversationExport';
import { conversationMarkdownLabels } from '@/modules/conversations/data/conversationMarkdownLabels';

export interface ConversationExportMenuProps {
  conversation: ChatConversation;
  disabled?: boolean;
}

export function ConversationExportMenu({ conversation, disabled = false }: ConversationExportMenuProps) {
  const { t, i18n } = useTranslation();
  const [exporting, setExporting] = useState(false);

  const handleExport = async (format: ExportFormat) => {
    setExporting(true);
    try {
      const labels = conversationMarkdownLabels(conversation, t, i18n.language);
      const { exportConversationToFile } = await import('@/modules/conversations/data/conversationExport');
      await exportConversationToFile(conversation, format, labels, t('exportConversation'));
    } finally {
      setExporting(false);
    }
  };

  return (
    <DropdownMenu>
      <Tooltip label={t('exportConversation')}>
        <DropdownMenuTrigger asChild>
          <Button
            aria-label={t('exportConversation')}
            className="size-8 rounded-control text-secondary-icon hover:text-text"
            disabled={disabled || exporting}
            size="icon"
            variant="ghost"
          >
            <Share aria-hidden="true" size={15} />
          </Button>
        </DropdownMenuTrigger>
      </Tooltip>

      <DropdownMenuContent aria-label={t('exportConversation')} className="w-80">
        <DropdownMenuLabel>{t('exportConversation')}</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={() => void handleExport('md')}>
          <span
            aria-hidden="true"
            className="grid size-9 shrink-0 place-items-center rounded-control bg-accent/10 text-accent"
          >
            <FileText size={17} />
          </span>
          <span className="min-w-0 flex-1">
            <span className="flex items-center justify-between gap-1.5">
              <span className="truncate text-sm font-medium text-text">{t('exportFormatMarkdownTitle')}</span>
              <span className="shrink-0 rounded bg-surface px-1.5 py-0.5 font-mono text-caption text-secondary-text">
                .md
              </span>
            </span>
            <span className="block truncate text-xs text-secondary-text">{t('exportFormatMarkdownHint')}</span>
          </span>
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => void handleExport('html')}>
          <span
            aria-hidden="true"
            className="grid size-9 shrink-0 place-items-center rounded-control bg-info/10 text-info"
          >
            <FileCode size={17} />
          </span>
          <span className="min-w-0 flex-1">
            <span className="flex items-center justify-between gap-1.5">
              <span className="truncate text-sm font-medium text-text">{t('exportFormatHtmlTitle')}</span>
              <span className="shrink-0 rounded bg-surface px-1.5 py-0.5 font-mono text-caption text-secondary-text">
                .html
              </span>
            </span>
            <span className="block truncate text-xs text-secondary-text">{t('exportFormatHtmlHint')}</span>
          </span>
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => void handleExport('json')}>
          <span
            aria-hidden="true"
            className="grid size-9 shrink-0 place-items-center rounded-control bg-warning/10 text-warning"
          >
            <FileJson size={17} />
          </span>
          <span className="min-w-0 flex-1">
            <span className="flex items-center justify-between gap-1.5">
              <span className="truncate text-sm font-medium text-text">{t('exportFormatJsonTitle')}</span>
              <span className="shrink-0 rounded bg-surface px-1.5 py-0.5 font-mono text-caption text-secondary-text">
                .json
              </span>
            </span>
            <span className="block truncate text-xs text-secondary-text">{t('exportFormatJsonHint')}</span>
          </span>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
