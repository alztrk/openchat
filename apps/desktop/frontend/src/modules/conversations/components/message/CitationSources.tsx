import { Info } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { CitationSourceDialog } from './CitationSourceDialog';
import type { CitationSource } from './messageData';

interface CitationSourcesProps {
  onSelect?(source: CitationSource): void;
  sources: readonly CitationSource[];
}

export function CitationSources({ onSelect, sources }: CitationSourcesProps) {
  const { t } = useTranslation();
  const [selectedSource, setSelectedSource] = useState<CitationSource | null>(null);
  const selectSource = (source: CitationSource) => {
    if (onSelect) {
      onSelect(source);
    } else {
      setSelectedSource(source);
    }
  };

  return (
    <>
      <section aria-label={t('toolSourceDetails')} className="mt-3 flex flex-wrap gap-2">
        {sources.map((source) => (
          <Button
            aria-haspopup="dialog"
            aria-label={t('toolCitationSource', { sourceId: source.id })}
            className="max-w-full rounded-control px-2 text-xs text-secondary-text"
            key={source.id}
            onClick={() => selectSource(source)}
            size="small"
            title={source.title}
            type="button"
            variant="secondary"
          >
            <Info aria-hidden="true" size={13} />
            <span>{t('toolCitationSource', { sourceId: source.id })}</span>
          </Button>
        ))}
      </section>
      {onSelect ? null : (
        <CitationSourceDialog
          onOpenChange={(open) => {
            if (!open) setSelectedSource(null);
          }}
          open={selectedSource !== null}
          source={selectedSource}
        />
      )}
    </>
  );
}
