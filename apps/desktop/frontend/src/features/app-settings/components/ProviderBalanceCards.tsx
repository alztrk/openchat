import { CircleAlert } from 'lucide-react';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, ProviderIcon } from '@shared/components';
import {
  formatBalanceAmount,
  loadProviderBalances,
  type ProviderBalanceSnapshot,
  type ProviderQuotaWindow,
  providerBalanceAmountFields,
} from '@features/provider-settings/data';
import { serviceErrorCategoryTranslationKey } from '@shared/lib/platform/local-service';

interface ProviderBalanceCardsProps {
  refreshSequence: number;
  renderWindow(window: ProviderQuotaWindow, label: string): ReactNode;
}

const providerNameKeys = {
  openrouter: 'providerNameOpenRouter',
  deepseek: 'providerNameDeepSeek',
  opencode: 'providerNameOpenCodeGo',
} as const;

const windowLabelKeys = {
  key: 'usageQuotasKey',
  credits: 'usageQuotasCredits',
  rolling: 'usageFiveHour',
  weekly: 'usageWeekly',
  monthly: 'usageMonthly',
} as const;

const balanceLabelKeys = {
  key: 'usageQuotasKey',
  credits: 'usageQuotasCredits',
  balance: 'usageQuotasBalance',
  daily: 'usageDaily',
  weekly: 'usageWeekly',
  monthly: 'usageMonthly',
} as const;

const amountLabelKeys = {
  total: 'usageQuotasTotal',
  granted: 'usageQuotasGranted',
  toppedUp: 'usageQuotasToppedUp',
  limit: 'usageQuotasLimit',
  remaining: 'usageQuotasRemaining',
  used: 'statisticsUsed',
} as const;

export function ProviderBalanceCards({ refreshSequence, renderWindow }: ProviderBalanceCardsProps) {
  const { i18n, t } = useTranslation();
  const [snapshots, setSnapshots] = useState<ProviderBalanceSnapshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [failed, setFailed] = useState(false);
  const latestRequest = useRef(0);
  const snapshotsRef = useRef<ProviderBalanceSnapshot[]>([]);

  const load = useCallback(async () => {
    const request = ++latestRequest.current;
    const hasSnapshots = snapshotsRef.current.length > 0;
    const isCurrent = () => latestRequest.current === request;
    setLoading(!hasSnapshots);
    setRefreshing(hasSnapshots);
    try {
      const loaded = await loadProviderBalances();
      if (!isCurrent()) return;
      snapshotsRef.current = loaded;
      setSnapshots(loaded);
      setFailed(false);
    } catch {
      if (!isCurrent()) return;
      setFailed(true);
    } finally {
      if (isCurrent()) {
        setLoading(false);
        setRefreshing(false);
      }
    }
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: refreshSequence invalidates the balances after an explicit refresh.
  useEffect(() => {
    void load();
    return () => {
      latestRequest.current += 1;
    };
  }, [load, refreshSequence]);

  if (snapshots.length === 0 && !failed) return null;

  const locale = i18n.resolvedLanguage ?? i18n.language;

  return (
    <>
      {snapshots.map((snapshot) => (
        <Card className="grid gap-2" key={snapshot.providerId} padding="compact">
          <header className="flex min-w-0 items-center gap-3">
            <ProviderIcon providerId={snapshot.providerId} size={22} />
            <h3 className="truncate font-semibold">{t(providerNameKeys[snapshot.providerId])}</h3>
          </header>
          {snapshot.errorCategory !== null ? (
            <p className="text-sm text-warning" role="status">
              {t(serviceErrorCategoryTranslationKey(snapshot.errorCategory))}
            </p>
          ) : null}
          {snapshot.reasonCode === 'insufficient_balance' ? (
            <p className="text-sm text-warning" role="status">
              {t('usageQuotasInsufficientBalance')}
            </p>
          ) : null}
          {snapshot.windows.map((window) => (
            <div key={window.label}>{renderWindow(window, t(windowLabelKeys[window.label]))}</div>
          ))}
          {snapshot.balances.map((balance) => (
            <section className="grid gap-1.5" key={`${balance.label}:${balance.currency}`}>
              <h4 className="text-sm font-medium">{t(balanceLabelKeys[balance.label])}</h4>
              <dl className="grid gap-1 text-sm">
                {providerBalanceAmountFields.map((field) => {
                  const amount = balance[field];
                  if (amount === null) return null;
                  return (
                    <div className="flex items-center justify-between gap-2" key={field}>
                      <dt className="text-secondary-text">{t(amountLabelKeys[field])}</dt>
                      <dd className="tabular-nums">{formatBalanceAmount(amount, balance.currency, locale)}</dd>
                    </div>
                  );
                })}
              </dl>
            </section>
          ))}
        </Card>
      ))}
      {refreshing ? (
        <div
          aria-label={t('usageLoading')}
          className="col-span-full h-0.5 animate-pulse rounded-full bg-accent"
          role="status"
        />
      ) : null}
      {failed && snapshots.length > 0 ? (
        <p className="col-span-full text-sm text-warning" role="status">
          {t('usageQuotasLoadFailed')}
        </p>
      ) : null}
      {failed && snapshots.length === 0 ? (
        <Card className="flex flex-wrap items-center justify-between gap-3" padding="compact" role="alert">
          <div className="flex items-center gap-2 text-sm text-destructive">
            <CircleAlert aria-hidden="true" size={18} />
            <span>{t('usageQuotasLoadFailed')}</span>
          </div>
          <Button disabled={loading} onClick={() => void load()} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </Card>
      ) : null}
    </>
  );
}
