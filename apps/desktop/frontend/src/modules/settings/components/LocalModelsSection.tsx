import { FolderOpen, LoaderCircle, Square, Trash2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Card } from '@/design-system';
import type { BusyAction, LocalModel } from '@/modules/settings/data/localEngineCatalog';

export function LocalModelsSection({
  addLabel,
  activeAction,
  engineId,
  models,
  onAddModel,
  onCancelStart,
  onRemove,
  onStart,
  onStop,
  runningModelIds,
  startingModelId,
  startingModelIsCancelling,
}: {
  addLabel: 'localModelAddFile' | 'localModelAddFolder';
  activeAction: BusyAction | null;
  engineId: string;
  models: LocalModel[];
  onAddModel(): void;
  onCancelStart(): void;
  onRemove(model: LocalModel): void;
  onStart(modelId: string): void;
  onStop(modelId: string): void;
  runningModelIds: string[];
  startingModelId: string | null;
  startingModelIsCancelling: boolean;
}) {
  const { t } = useTranslation();
  return (
    <section aria-labelledby={`local-models-heading-${engineId}`} className="grid gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h3 className="text-base font-semibold" id={`local-models-heading-${engineId}`}>
          {t('localModels')}
        </h3>
        <div className="flex flex-wrap gap-2">
          <Button disabled={activeAction !== null} onClick={onAddModel} size="small" variant="secondary">
            <FolderOpen aria-hidden="true" size={16} />
            {t(addLabel)}
          </Button>
        </div>
      </div>
      {models.length === 0 ? (
        <p className="rounded-card border border-border bg-surface p-5 text-sm text-secondary-text">
          {t('localModelsEmpty')}
        </p>
      ) : null}
      <ul className="grid gap-2">
        {models.map((model) => {
          const running = runningModelIds.includes(model.id);
          const busy = activeAction !== null;
          const starting = startingModelId === model.id;
          return (
            <li key={model.id}>
              <Card className="flex min-w-0 flex-wrap items-start justify-between gap-4 p-4">
                <div className="min-w-0 flex-1">
                  <p className="break-words font-medium text-text">{model.displayName}</p>
                  <p className="mt-1 break-all font-mono text-xs text-secondary-text">{model.path}</p>
                  {!model.pathExists ? <p className="mt-2 text-sm text-warning">{t('localModelPathMissing')}</p> : null}
                  {model.pathExists && !model.isAvailable ? (
                    <p className="mt-2 text-sm text-warning">{t('localModelEngineNotReady')}</p>
                  ) : null}
                </div>
                <div className="flex shrink-0 flex-wrap gap-2">
                  {model.pathExists && model.isAvailable ? (
                    starting ? (
                      <Button
                        aria-label={t('cancel')}
                        disabled={startingModelIsCancelling}
                        onClick={onCancelStart}
                        size="small"
                        variant="secondary"
                      >
                        {startingModelIsCancelling ? (
                          <LoaderCircle aria-hidden="true" className="animate-spin" size={14} />
                        ) : (
                          <X aria-hidden="true" size={14} />
                        )}
                        {t('cancel')}
                      </Button>
                    ) : (
                      <Button
                        disabled={busy}
                        onClick={() => (running ? onStop(model.id) : onStart(model.id))}
                        size="small"
                        variant="secondary"
                      >
                        {running ? <Square aria-hidden="true" size={14} /> : null}
                        {t(running ? 'localEngineStopModel' : 'localEngineStartModel')}
                      </Button>
                    )
                  ) : null}
                  <Button
                    aria-label={`${t('localModelRemove')}: ${model.displayName}`}
                    disabled={busy}
                    onClick={() => onRemove(model)}
                    size="small"
                    variant="ghost"
                  >
                    <Trash2 aria-hidden="true" size={16} />
                    {t('localModelRemove')}
                  </Button>
                </div>
              </Card>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
