import { useTranslation } from 'react-i18next';

import {
  type ProviderVerificationOutcome,
} from '@/features/settings/data/providerVerification';
import type { ProviderCredentialSnapshot } from '@/features/settings/data/providerCredentials';
import type { ProviderRequestOutcomeRecord, ProviderRequestRoute } from '@/features/settings/data/providerRequestOutcome';
import { ProviderRequestOutcomeStatus } from './ProviderRequestOutcomeStatus';

export type ProviderVerificationState =
  | 'notChecked'
  | 'checking'
  | 'verified'
  | 'empty'
  | 'stale'
  | 'authenticationFailed'
  | 'rateLimited'
  | 'networkFailed'
  | 'failed';

export type CompletedProviderVerificationState = Exclude<ProviderVerificationState, 'notChecked' | 'checking'>;

export interface ProviderVerificationViewState {
  state: ProviderVerificationState;
  checkedAtUnixMs: number | null;
  recordReadFailed: boolean;
  saveFailed: boolean;
}

export function verificationEntries(snapshot: ProviderCredentialSnapshot): Array<[string, { providerId: string; connectionId: string; workspaceId: null }]> {
  const entries: Array<[string, { providerId: string; connectionId: string; workspaceId: null }]> = [];
  for (const entry of snapshot.chatgptApiKeys) {
    entries.push([`chatgpt_api:${entry.id}`, { providerId: 'chatgpt_api', connectionId: entry.id, workspaceId: null }]);
  }
  for (const entry of snapshot.compatibleProviders) {
    if (!entry.isConfigured) continue;
    entries.push([`compatible:${entry.providerId}`, { providerId: entry.providerId, connectionId: entry.providerId, workspaceId: null }]);
  }
  if (snapshot.openCodeConfigured) {
    entries.push(['opencode', { providerId: 'opencode', connectionId: 'opencode', workspaceId: null }]);
  }
  return entries;
}

export function notCheckedVerificationState(): ProviderVerificationViewState {
  return { state: 'notChecked', checkedAtUnixMs: null, recordReadFailed: false, saveFailed: false };
}

export function requestOutcomeEntries(snapshot: ProviderCredentialSnapshot): Array<[string, ProviderRequestRoute]> {
  const entries: Array<[string, ProviderRequestRoute]> = [];
  for (const entry of snapshot.chatgptApiKeys) {
    entries.push([`chatgpt_api:${entry.id}`, { providerId: 'chatgpt_api', connectionId: entry.id, workspaceId: null }]);
  }
  for (const entry of snapshot.compatibleProviders) {
    if (!entry.isConfigured) continue;
    entries.push([`compatible:${entry.providerId}`, { providerId: entry.providerId, connectionId: entry.providerId, workspaceId: null }]);
  }
  if (snapshot.openCodeConfigured) {
    entries.push(['opencode', { providerId: 'opencode', connectionId: 'opencode', workspaceId: null }]);
  }
  return entries;
}

interface ProviderVerificationStatusProps {
  requestOutcome?: {
    record: ProviderRequestOutcomeRecord | null;
    readFailed: boolean;
    saveFailed: boolean;
  };
  showQuotaUnavailable?: boolean;
  value: ProviderVerificationViewState | undefined;
}

export function ProviderVerificationStatus({
  requestOutcome,
  showQuotaUnavailable = true,
  value,
}: ProviderVerificationStatusProps) {
  const { i18n, t } = useTranslation();
  const state = value?.state ?? 'notChecked';
  const checkedAt = value?.checkedAtUnixMs ?? null;
  const messageKey = verificationMessageKey(state);
  const colorClass = verificationColorClass(state);

  return (
    <div className="grid gap-2 pt-2">
      <p aria-live="polite" className={`text-sm ${colorClass}`}>
        {t(messageKey)}
      </p>
      {checkedAt !== null ? (
        <p className="text-xs text-secondary-text">
          {t('providerConnectionLastChecked', {
            dateTime: new Intl.DateTimeFormat(i18n.resolvedLanguage ?? i18n.language, {
              dateStyle: 'medium',
              timeStyle: 'short',
            }).format(checkedAt),
          })}
        </p>
      ) : null}
      <ProviderRequestOutcomeStatus
        readFailed={requestOutcome?.readFailed}
        record={requestOutcome?.record ?? null}
        saveFailed={requestOutcome?.saveFailed}
      />
      {value?.saveFailed ? (
        <p className="text-xs text-destructive" role="alert">{t('providerVerificationResultNotSaved')}</p>
      ) : null}
      {value?.recordReadFailed ? (
        <p className="text-xs text-destructive" role="alert">{t('providerVerificationRecordUnreadable')}</p>
      ) : null}
      {showQuotaUnavailable ? <p className="text-xs text-secondary-text">{t('providerQuotaUnavailable')}</p> : null}
    </div>
  );
}

export function providerVerificationStateForOutcome(outcome: ProviderVerificationOutcome): CompletedProviderVerificationState {
  switch (outcome) {
    case 'verified': return 'verified';
    case 'emptyCatalog': return 'empty';
    case 'staleCatalog': return 'stale';
    case 'authenticationFailed': return 'authenticationFailed';
    case 'rateLimited': return 'rateLimited';
    case 'networkFailed': return 'networkFailed';
    case 'failed': return 'failed';
  }
}

function verificationMessageKey(state: ProviderVerificationState):
  | 'providerConnectionNotVerified'
  | 'verifyingProviderConnection'
  | 'providerConnectionVerified'
  | 'providerModelCatalogEmpty'
  | 'providerVerificationStaleCatalog'
  | 'providerVerificationAuthenticationFailed'
  | 'providerVerificationRateLimited'
  | 'providerVerificationNetworkFailed'
  | 'providerConnectionVerificationFailed' {
  switch (state) {
    case 'notChecked': return 'providerConnectionNotVerified';
    case 'checking': return 'verifyingProviderConnection';
    case 'verified': return 'providerConnectionVerified';
    case 'empty': return 'providerModelCatalogEmpty';
    case 'stale': return 'providerVerificationStaleCatalog';
    case 'authenticationFailed': return 'providerVerificationAuthenticationFailed';
    case 'rateLimited': return 'providerVerificationRateLimited';
    case 'networkFailed': return 'providerVerificationNetworkFailed';
    case 'failed': return 'providerConnectionVerificationFailed';
  }
}

function verificationColorClass(state: ProviderVerificationState): string {
  switch (state) {
    case 'verified': return 'text-success';
    case 'stale': return 'text-warning';
    case 'authenticationFailed':
    case 'rateLimited':
    case 'networkFailed':
    case 'failed': return 'text-destructive';
    default: return 'text-secondary-text';
  }
}
