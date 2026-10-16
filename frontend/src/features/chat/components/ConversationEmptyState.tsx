import { Check, Link2, LoaderCircle, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { BrandMark } from '@/app/BrandMark';
import { Button } from '@/shared/ui';

interface ConversationEmptyStateProps {
  hasAvailableModels: boolean;
  hasConfiguredProvider: boolean;
  isLoading: boolean;
  modelCatalogFailed: boolean;
  modelCatalogStale: boolean;
  onOpenConnections(): void;
  onRetryModels(): void;
  selectedModel: boolean;
  showSetupSteps: boolean;
}

const setupSteps = [
  { id: 'connections', labelKey: 'connections' },
  { id: 'verification', labelKey: 'providerCatalogVerification' },
  { id: 'model', labelKey: 'modelSelection' },
  { id: 'send', labelKey: 'send' },
] as const;

export function ConversationEmptyState({
  hasAvailableModels,
  hasConfiguredProvider,
  isLoading,
  modelCatalogFailed,
  modelCatalogStale,
  onOpenConnections,
  onRetryModels,
  selectedModel,
  showSetupSteps,
}: ConversationEmptyStateProps) {
  const { t } = useTranslation();
  const title = isLoading
    ? t('modelsLoading')
    : !hasAvailableModels
      ? modelCatalogFailed
        ? t('modelsUnavailable')
        : hasConfiguredProvider
          ? t('providerNeedsVerificationTitle')
          : t('noModelConnected')
      : selectedModel
        ? t('emptyChatWelcomeTitle')
        : t('emptyChatSelectModelTitle');
  const description = isLoading
    ? null
    : !hasAvailableModels
      ? modelCatalogFailed
        ? t('modelCatalogUnavailable')
        : hasConfiguredProvider
          ? t('providerNeedsVerificationBody')
          : t('noModelConnectedBody')
      : selectedModel
        ? t('emptyChatWelcomeBody')
        : t('emptyChatSelectModelBody');
  const activeSetupStep = modelCatalogStale
    ? 1
    : selectedModel
      ? 3
      : hasAvailableModels
        ? 2
        : hasConfiguredProvider
          ? 1
          : 0;
  const showConnections = !isLoading && !hasAvailableModels;
  const showRetry = !isLoading && (modelCatalogFailed || modelCatalogStale);

  return (
    <div className="conversation-empty-state flex min-h-0 flex-1 flex-col overflow-y-auto px-4 py-6 sm:px-6 sm:py-8">
      <div className="conversation-empty-state__content mx-auto my-auto w-full max-w-conversation pb-4 text-center">
        {selectedModel && hasAvailableModels && !isLoading ? (
          <div className="conversation-empty-state__brand-mark mb-4">
            <BrandMark size="welcome" />
          </div>
        ) : null}
        {isLoading ? (
          <LoaderCircle aria-hidden="true" className="mx-auto mb-4 animate-spin text-accent" size={24} />
        ) : null}
        <h1 aria-live="polite" className="conversation-empty-state__title mx-auto max-w-empty-title font-semibold text-text">
          {title}
        </h1>
        {description ? (
          <p className="conversation-empty-state__description mx-auto mt-2.5 max-w-empty-description text-body text-secondary-text">
            {description}
          </p>
        ) : null}
        {modelCatalogStale ? (
          <p className="mx-auto mt-2 max-w-empty-description text-body text-warning" role="status">
            {t('providerVerificationStaleCatalog')}
          </p>
        ) : null}
        {showSetupSteps && !isLoading ? (
          <SetupSteps activeStep={activeSetupStep} />
        ) : null}
        {showConnections || showRetry ? (
          <div className="conversation-empty-state__actions flex flex-wrap justify-center gap-2">
            {showConnections ? (
              <Button onClick={onOpenConnections} size="small" variant="primary">
                <Link2 aria-hidden="true" size={16} />
                {t('connections')}
              </Button>
            ) : null}
            {showRetry ? (
              <Button onClick={onRetryModels} size="small" variant="secondary">
                <RefreshCw aria-hidden="true" size={16} />
                {t(modelCatalogStale ? 'refreshModels' : 'retry')}
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function SetupSteps({ activeStep }: { activeStep: number }) {
  const { t } = useTranslation();

  return (
    <ol className="conversation-empty-state__setup-steps mx-auto mt-6 flex max-w-setup-steps flex-wrap justify-center gap-x-6 gap-y-2">
      {setupSteps.map((step, index) => {
        const isCompleted = index < activeStep;
        const isCurrent = index === activeStep;
        return (
          <li
            aria-current={isCurrent ? 'step' : undefined}
            className={`flex min-w-0 items-center gap-2 text-body ${isCurrent ? 'font-semibold text-text' : 'text-secondary-text'}`}
            key={step.id}
          >
            <span className={`flex size-6 shrink-0 items-center justify-center rounded-full border ${isCurrent ? 'border-accent bg-accent text-accent-foreground' : isCompleted ? 'border-accent bg-accent text-accent-foreground' : 'border-border bg-surface text-secondary-text'}`}>
              {isCompleted ? <Check aria-hidden="true" size={14} /> : index + 1}
            </span>
            <span>{t(step.labelKey)}</span>
            {isCompleted ? <span className="sr-only">{t('setupStepCompleted')}</span> : null}
            {isCurrent ? <span className="sr-only">{t('setupStepCurrent')}</span> : null}
          </li>
        );
      })}
    </ol>
  );
}
