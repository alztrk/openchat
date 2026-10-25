import { Link2, LoaderCircle, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import { BrandMark } from '@/shell/BrandMark';

interface ConversationEmptyStateProps {
  hasAvailableModels: boolean;
  hasConfiguredProvider: boolean;
  isLoading: boolean;
  modelCatalogFailed: boolean;
  modelCatalogStale: boolean;
  chatGptWorkspaceSelectionRequired: boolean;
  onOpenConnections(): void;
  onRetryModels(): void;
  selectedModel: boolean;
}

export function ConversationEmptyState({
  hasAvailableModels,
  hasConfiguredProvider,
  isLoading,
  modelCatalogFailed,
  modelCatalogStale,
  chatGptWorkspaceSelectionRequired,
  onOpenConnections,
  onRetryModels,
  selectedModel,
}: ConversationEmptyStateProps) {
  const { t } = useTranslation();
  const workspaceSelectionRequired = chatGptWorkspaceSelectionRequired && !hasAvailableModels;
  const title = isLoading
    ? t('modelsLoading')
    : workspaceSelectionRequired
      ? t('chatGptWorkspaceSelectionTitle')
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
    : workspaceSelectionRequired
      ? t('chatGptWorkspaceSelectionRequired')
      : !hasAvailableModels
        ? modelCatalogFailed
          ? t('modelCatalogUnavailable')
          : hasConfiguredProvider
            ? t('providerNeedsVerificationBody')
            : t('noModelConnectedBody')
        : selectedModel
          ? t('emptyChatWelcomeBody')
          : t('emptyChatSelectModelBody');
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
        <h1
          aria-live="polite"
          className="conversation-empty-state__title mx-auto max-w-empty-title font-semibold text-text"
        >
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
