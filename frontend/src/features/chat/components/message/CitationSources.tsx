import { Info } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

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
      <div className="mt-3 flex flex-wrap gap-2" aria-label={t('toolSourceDetails')}>
        {sources.map((source) => (
          <button
            aria-haspopup="dialog"
            aria-label={t('toolCitationSource', { sourceId: source.id })}
            className="inline-flex max-w-full items-center gap-1 rounded-control border border-border bg-surface px-2 py-1 text-xs text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            key={source.id}
            onClick={() => selectSource(source)}
            title={source.title}
            type="button"
          >
            <Info aria-hidden="true" size={13} />
            <span>{t('toolCitationSource', { sourceId: source.id })}</span>
          </button>
        ))}
      </div>
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
