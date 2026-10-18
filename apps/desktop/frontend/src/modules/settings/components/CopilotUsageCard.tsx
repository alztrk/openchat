import type { TFunction } from 'i18next';
import { CircleAlert, LoaderCircle, RefreshCw } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@/design-system';
import { ProviderIcon } from '@/modules/providers/components/ProviderIcon';
import {
  type CopilotOAuthConnection,
  type CopilotQuotaSnapshot,
  type CopilotUsageSnapshot,
  loadCopilotOAuthStatus,
  loadCopilotUsage,
} from '@/modules/providers/data/copilotOAuth';
import { LocalServiceError } from '@/platform/local-service';

interface CopilotUsageCardProps {
  onNavigateToConnections(): void;
  refreshSequence: number;
}

export function CopilotUsageCard({ onNavigateToConnections, refreshSequence }: CopilotUsageCardProps) {
  const { i18n, t } = useTranslation();
  const [connection, setConnection] = useState<CopilotOAuthConnection | null>(null);
  const [snapshot, setSnapshot] = useState<CopilotUsageSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [failed, setFailed] = useState(false);
  const [needsSignIn, setNeedsSignIn] = useState(false);
  const [loadedAt, setLoadedAt] = useState<number | null>(null);
  const latestRequest = useRef(0);
  const snapshotRef = useRef<CopilotUsageSnapshot | null>(null);

  const load = useCallback(async () => {
    const request = ++latestRequest.current;
    const hasSnapshot = snapshotRef.current !== null;
    const isCurrent = () => latestRequest.current === request;
    setLoading(!hasSnapshot);
    setRefreshing(hasSnapshot);
    setFailed(false);
    setNeedsSignIn(false);
    try {
      const status = await loadCopilotOAuthStatus();
      if (!isCurrent()) return;
      if (!status.connected || status.connection === null) {
        setConnection(null);
        setSnapshot(null);
        snapshotRef.current = null;
        setLoadedAt(null);
        return;
      }
      setConnection(status.connection);
      if (status.connection.authStatus !== 'active') {
        setSnapshot(null);
        snapshotRef.current = null;
        setNeedsSignIn(true);
        setLoadedAt(null);
        return;
      }

      const usage = await loadCopilotUsage();
      if (!isCurrent()) return;
      setSnapshot(usage);
      snapshotRef.current = usage;
      setLoadedAt(Date.now());
    } catch (error) {
      if (!isCurrent()) return;
      setFailed(true);
      setNeedsSignIn(isAuthenticationError(error));
    } finally {
      if (isCurrent()) {
        setLoading(false);
        setRefreshing(false);
      }
    }
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: refreshSequence invalidates the quota response after an explicit refresh.
  useEffect(() => {
    void load();
    return () => {
      latestRequest.current += 1;
    };
  }, [load, refreshSequence]);

  if (connection === null && !loading && !failed) return null;

  const locale = i18n.resolvedLanguage ?? i18n.language;
  const accountName = connection?.name?.trim() || connection?.login || t('providerNameCopilot');

  return (
    <Card className="grid gap-4 p-5">
      <header className="flex min-w-0 items-start gap-3">
        <ProviderIcon className="mt-1" providerId="copilot" size={22} />
        <div className="min-w-0 flex-1">
          <h3 className="truncate font-semibold">{accountName}</h3>
          {connection !== null ? (
            <p className="mt-1 text-sm text-secondary-text">
              {t('copilotUsagePlan', { plan: snapshot?.planType ?? t('planUnavailable') })}
            </p>
          ) : null}
        </div>
        <Button
          aria-label={t('copilotUsageRefresh')}
          disabled={loading || refreshing}
          onClick={() => void load()}
          size="icon"
          variant="ghost"
        >
          {refreshing ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          ) : (
            <RefreshCw aria-hidden="true" size={16} />
          )}
        </Button>
      </header>

      {loading ? (
        <div aria-live="polite" className="flex min-h-16 items-center justify-center">
          <LoaderCircle aria-label={t('copilotUsageLoading')} className="animate-spin text-secondary-icon" size={20} />
        </div>
      ) : needsSignIn ? (
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4">
          <p className="text-sm text-secondary-text">{t('copilotUsageNeedsSignIn')}</p>
          <Button onClick={onNavigateToConnections} size="small" variant="secondary">
            {t('copilotUsageReconnect')}
          </Button>
        </div>
      ) : failed && snapshot === null ? (
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4" role="alert">
          <div className="flex items-center gap-2 text-sm text-destructive">
            <CircleAlert aria-hidden="true" size={18} />
            <span>{t('copilotUsageLoadFailed')}</span>
          </div>
          <Button onClick={() => void load()} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : snapshot !== null ? (
        <div className="grid gap-4 border-t border-border pt-4">
          <div className="flex flex-wrap items-center justify-between gap-2 text-sm">
            <div className="grid gap-1">
              <span className="text-secondary-text">
                {t('copilotUsageAccessType', { type: snapshot.accessType ?? t('unavailableValue') })}
              </span>
              {loadedAt !== null ? (
                <span className="text-xs text-secondary-text">
                  {t('usageUpdatedAt', { time: formatTimestamp(loadedAt, locale) })}
                </span>
              ) : null}
            </div>
          </div>
          {snapshot.quotas.length === 0 ? (
            <p className="text-sm text-secondary-text">{t('copilotQuotaUnavailable')}</p>
          ) : (
            <div className="grid gap-4">
              {snapshot.quotas.map((quota) => (
                <CopilotQuotaView key={quota.id} locale={locale} quota={quota} />
              ))}
            </div>
          )}
          {refreshing ? (
            <div
              aria-label={t('copilotUsageLoading')}
              className="h-0.5 animate-pulse rounded-full bg-accent"
              role="status"
            />
          ) : null}
          {failed ? (
            <p className="text-sm text-warning" role="status">
              {t('copilotUsageLoadFailed')}
            </p>
          ) : null}
        </div>
      ) : null}
    </Card>
  );
}

function CopilotQuotaView({ locale, quota }: { locale: string; quota: CopilotQuotaSnapshot }) {
  const { t } = useTranslation();
  const label = quotaLabel(quota, t);
  const remainingPercent = quota.unlimited === true ? null : quota.percentRemaining;
  const meterValue = remainingPercent === null ? null : Math.min(100, Math.max(0, remainingPercent));
  const entitlement = quota.entitlement !== null && quota.entitlement >= 0 ? quota.entitlement : null;
  const numberFormat = new Intl.NumberFormat(locale, { maximumFractionDigits: 2 });

  return (
    <section aria-label={label} className="grid gap-1.5">
      <div className="flex flex-wrap items-center justify-between gap-2 text-sm">
        <span className="font-medium">{label}</span>
        <span className="text-secondary-text">
          {quota.unlimited === true
            ? t('copilotQuotaUnlimited')
            : quota.remaining === null
              ? t('unavailableValue')
              : t('copilotQuotaRemaining', { count: numberFormat.format(quota.remaining) })}
        </span>
      </div>
      {meterValue !== null || entitlement !== null ? (
        <div className="flex items-center justify-between text-xs text-secondary-text">
          {meterValue !== null ? (
            <span>{t('copilotQuotaRemainingPercent', { percent: Math.round(meterValue) })}</span>
          ) : null}
          {entitlement !== null ? (
            <span>{t('copilotQuotaEntitlement', { count: numberFormat.format(entitlement) })}</span>
          ) : null}
        </div>
      ) : null}
      {meterValue !== null ? (
        <>
          {/* biome-ignore lint/a11y/useSemanticElements: This custom meter retains OpenChat's track and fill styling while exposing the meter role. */}
          <div
            aria-label={label}
            aria-valuemax={100}
            aria-valuemin={0}
            aria-valuenow={meterValue}
            aria-valuetext={t('copilotQuotaRemainingPercent', { percent: Math.round(meterValue) })}
            className="h-1.5 overflow-hidden rounded-full bg-selected"
            role="meter"
          >
            <div className="h-full rounded-full bg-accent" style={{ width: `${meterValue}%` }} />
          </div>
        </>
      ) : null}
      {quota.resetAtUnixMs !== null ? (
        <span className="text-xs text-secondary-text">
          {t('copilotQuotaResetsAt', { time: formatTimestamp(quota.resetAtUnixMs, locale) })}
        </span>
      ) : null}
    </section>
  );
}

function quotaLabel(quota: CopilotQuotaSnapshot, t: TFunction): string {
  switch (quota.kind) {
    case 'chat':
      return t('copilotQuotaChat');
    case 'completions':
      return t('copilotQuotaCompletions');
    case 'premiumInteractions':
      return t('copilotQuotaPremiumInteractions');
    case 'other':
      return t('copilotQuotaOther');
  }
}

function formatTimestamp(value: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}

function isAuthenticationError(error: unknown): boolean {
  return (
    error instanceof LocalServiceError &&
    [
      'authentication_required',
      'copilot_authentication_required',
      'oauth_credential_invalid',
      'oauth_tokens_missing',
    ].includes(error.code)
  );
}
