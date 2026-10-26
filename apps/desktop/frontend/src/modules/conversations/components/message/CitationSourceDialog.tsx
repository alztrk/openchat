import type { TFunction } from 'i18next';
import { ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog } from '@/design-system';

import { type CitationSource, isSafeHttpUrl } from './messageData';

interface CitationSourceDialogProps {
  open: boolean;
  onOpenChange(open: boolean): void;
  source: CitationSource | null;
}

export function CitationSourceDialog({ open, onOpenChange, source }: CitationSourceDialogProps) {
  const { i18n, t } = useTranslation();
  if (source === null) return null;

  const retrievedAt =
    source.retrievedAtUnixMs === null
      ? null
      : new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' }).format(
          source.retrievedAtUnixMs,
        );
  const safeUrl = isSafeHttpUrl(source.url) ? source.url : null;

  return (
    <Dialog onOpenChange={onOpenChange} open={open} size="wide" title={t('toolSourceDetails')}>
      <div className="grid gap-4">
        <div className="grid gap-1">
          <h3 className="break-words text-base font-semibold text-text">{source.title}</h3>
          <div className="flex flex-wrap gap-2 text-xs text-secondary-text">
            <span className="rounded-control bg-composer px-2 py-1">
              {t('toolCitationSource', { sourceId: source.id })}
            </span>
            <span className="rounded-control bg-composer px-2 py-1">{sourceTypeLabel(source.sourceType, t)}</span>
          </div>
        </div>

        {retrievedAt !== null ? (
          <p className="text-sm text-secondary-text">{t('toolSourceRetrievedAt', { time: retrievedAt })}</p>
        ) : null}

        <div className="grid gap-1">
          <span className="text-xs font-medium text-secondary-text">{t('toolUrl')}</span>
          <div className="flex min-w-0 items-start gap-2 rounded-control border border-border bg-surface p-2.5">
            <span className="min-w-0 flex-1 break-all font-code text-xs text-text">{source.url}</span>
            {safeUrl ? (
              <a
                aria-label={t('toolOpenUrl')}
                className="grid size-10 shrink-0 place-items-center rounded-button text-accent-icon hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                href={safeUrl}
                rel="noopener noreferrer"
                target="_blank"
              >
                <ExternalLink aria-hidden="true" size={15} />
              </a>
            ) : null}
          </div>
        </div>

        {source.snippet !== null ? (
          <blockquote className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-composer p-3 text-sm leading-5 text-text">
            {source.snippet}
          </blockquote>
        ) : null}

        <div className="flex justify-end">
          <Button onClick={() => onOpenChange(false)} variant="secondary">
            {t('close')}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

function sourceTypeLabel(sourceType: string, t: TFunction): string {
  switch (sourceType) {
    case 'local_web_search':
      return t('toolLocalWebSource');
    case 'local_read_url':
      return t('toolLocalPageSource');
    case 'provider_native':
      return t('toolProviderSource');
    default:
      return t('toolSourceDetails');
  }
}
