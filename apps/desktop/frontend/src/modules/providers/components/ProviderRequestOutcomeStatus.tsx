import { useTranslation } from 'react-i18next';

import type {
  ProviderRequestOutcome,
  ProviderRequestOutcomeRecord,
} from '@/modules/providers/data/providerRequestOutcome';

interface ProviderRequestOutcomeStatusProps {
  record: ProviderRequestOutcomeRecord | null;
  readFailed?: boolean;
  saveFailed?: boolean;
}

export function ProviderRequestOutcomeStatus({
  record,
  readFailed = false,
  saveFailed = false,
}: ProviderRequestOutcomeStatusProps) {
  const { i18n, t } = useTranslation();
  if (record === null && !readFailed && !saveFailed) return null;

  return (
    <div className="grid gap-1 text-xs text-secondary-text">
      {record ? (
        <>
          <p>{t('providerLastRequestOutcome', { outcome: t(outcomeTranslationKey(record.outcome)) })}</p>
          <p>
            {t('providerLastRequestAt', {
              dateTime: new Intl.DateTimeFormat(i18n.resolvedLanguage ?? i18n.language, {
                dateStyle: 'medium',
                timeStyle: 'short',
              }).format(record.occurredAtUnixMs),
            })}
          </p>
        </>
      ) : null}
      {readFailed ? (
        <p className="text-destructive" role="alert">
          {t('providerLastRequestReadFailed')}
        </p>
      ) : null}
      {saveFailed ? (
        <p className="text-destructive" role="alert">
          {t('providerLastRequestSaveFailed')}
        </p>
      ) : null}
    </div>
  );
}

type ProviderRequestOutcomeTranslationKey =
  | 'providerRequestCompleted'
  | 'providerRequestWaitingForInput'
  | 'providerRequestStopped'
  | 'providerRequestAuthenticationFailed'
  | 'providerRequestRateLimited'
  | 'providerRequestNetworkFailed'
  | 'providerRequestFailed';

function outcomeTranslationKey(outcome: ProviderRequestOutcome): ProviderRequestOutcomeTranslationKey {
  switch (outcome) {
    case 'completed':
      return 'providerRequestCompleted';
    case 'waitingForInput':
      return 'providerRequestWaitingForInput';
    case 'stopped':
      return 'providerRequestStopped';
    case 'authenticationFailed':
      return 'providerRequestAuthenticationFailed';
    case 'rateLimited':
      return 'providerRequestRateLimited';
    case 'networkFailed':
      return 'providerRequestNetworkFailed';
    case 'failed':
      return 'providerRequestFailed';
  }
}
