import { useEffect, useRef } from 'react';
import { LoaderCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/shared/ui';
import type { ChatModel } from '@/features/chat/data/chatModels';
import { ProviderIcon } from './ProviderIcon';

interface ChatSlashModelMenuProps {
  models: readonly ChatModel[];
  selectedIndex: number;
  isLoading: boolean;
  canSelect: boolean;
  providerLabel(model: ChatModel): string;
  onSelect(model: ChatModel): void;
}

export function ChatSlashModelMenu({
  canSelect,
  isLoading,
  models,
  onSelect,
  providerLabel,
  selectedIndex,
}: ChatSlashModelMenuProps) {
  const { t } = useTranslation();
  const listRef = useRef<HTMLDivElement>(null);
  const groups = new Map<string, { id: string; label: string; models: ChatModel[] }>();
  for (const model of models) {
    const current = groups.get(model.providerId);
    if (current) {
      current.models.push(model);
    } else {
      groups.set(model.providerId, { id: model.providerId, label: providerLabel(model), models: [model] });
    }
  }
  const orderedGroups = [...groups.values()].sort((left, right) => left.label.localeCompare(right.label));

  useEffect(() => {
    if (models.length === 0) return;
    listRef.current?.querySelector<HTMLElement>(`[data-model-index="${selectedIndex}"]`)
      ?.scrollIntoView({ block: 'nearest' });
  }, [models.length, selectedIndex]);

  return (
    <div
      aria-label={t('connectedProvidersModels')}
      className="chat-slash-model-menu absolute bottom-full left-0 z-30 mb-2 w-full overflow-y-auto rounded-card border border-border bg-raised p-1 shadow-lg"
      id="chat-slash-model-list"
      role="listbox"
      ref={listRef}
    >
      {isLoading && models.length === 0 ? (
        <div aria-live="polite" className="flex min-h-16 items-center justify-center text-secondary-icon">
          <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={18} />
          <span className="sr-only">{t('modelsLoading')}</span>
        </div>
      ) : models.length === 0 ? (
        <p className="px-3 py-3 text-sm text-secondary-text">{t('noModelsAvailable')}</p>
      ) : (
        <div className="grid gap-1">
          {orderedGroups.map((group) => (
            <div className="grid gap-1" key={group.id}>
              <p className="px-3 pb-1 pt-2 text-xs font-semibold text-secondary-text">{group.label}</p>
              {group.models.map((model) => {
                const modelIndex = models.findIndex((candidate) => candidate.key === model.key);
                const selected = modelIndex === selectedIndex;
                return (
                  <Button
                    aria-selected={selected}
                    className="h-auto min-h-11 w-full justify-start px-3 py-2 text-left"
                    data-model-index={modelIndex}
                    disabled={!canSelect}
                    id={`chat-slash-model-${encodeURIComponent(model.key)}`}
                    key={model.key}
                    onClick={() => onSelect(model)}
                    onMouseDown={(event) => event.preventDefault()}
                    role="option"
                    size="small"
                    variant={selected ? 'secondary' : 'ghost'}
                  >
                    <ProviderIcon className="shrink-0" providerId={model.providerId} size={16} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium text-text">{model.displayName}</span>
                      {model.sourceLabel ? <span className="block truncate text-xs font-normal text-secondary-text">{model.sourceLabel}</span> : null}
                    </span>
                  </Button>
                );
              })}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
