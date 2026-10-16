import { useCallback, useEffect, useRef, useState } from 'react';
import { ChartNoAxesCombined, CircleAlert, LoaderCircle, RefreshCw, RotateCcw } from 'lucide-react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import { LocalServiceError } from '@/shared/services/LocalServiceClient';
import { Button, Card, Dialog } from '@/shared/ui';
import { ProviderIcon } from '@/features/chat/components/ProviderIcon';
import { localServiceClient } from '@/features/chat/stores/chatStore';

interface UsageWorkspace {
  id: string;
  displayName: string | null;
}

interface UsageConnection {
  id: string;
  email: string | null;
  displayName: string | null;
  planType: string | null;
  isSelected: boolean;
  workspaces: UsageWorkspace[];
}

interface UsageBucket {
  limitId: string;
  usedPercent: number | null;
  windowSeconds: number | null;
  resetAtUnixMs: number | null;
}

interface ResetCredit {
  id: string;
  resetType: string | null;
  status: string | null;
  grantedAtUnixMs: number | null;
  expiresAtUnixMs: number | null;
  title: string | null;
  description: string | null;
}

interface UsageSnapshot {
  fetchedAtUnixMs: number;
  ordinaryUsageAllowed: boolean | null;
  resetCreditCount: number | null;
  buckets: UsageBucket[];
  resetCredits: ResetCredit[];
}

interface PendingCredit {
  connection: UsageConnection;
  workspace: UsageWorkspace;
  credit: ResetCredit;
}

type LoadState = 'loading' | 'loaded' | 'failed';
type ResetOutcomeKey =
  | 'resetCreditApplied'
  | 'resetCreditAlreadyUsed'
  | 'resetCreditNothingToReset'
  | 'resetCreditNoLongerAvailable'
  | 'resetCreditOutcomeUnknown'
  | 'resetCreditRejected'
  | 'resetCreditSignInRequired';

export interface UsageQuotasSectionProps {
  onNavigateToConnections(): void;
}

export function UsageQuotasSection({ onNavigateToConnections }: UsageQuotasSectionProps) {
  const { i18n, t } = useTranslation();
  const [connections, setConnections] = useState<UsageConnection[]>([]);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [isRefreshingAll, setIsRefreshingAll] = useState(false);
  const [snapshots, setSnapshots] = useState<Record<string, UsageSnapshot>>({});
  const [loadingKeys, setLoadingKeys] = useState<Set<string>>(() => new Set());
  const [errors, setErrors] = useState<Record<string, 'usageLoadFailed'>>({});
  const [refreshRequiredKeys, setRefreshRequiredKeys] = useState<Set<string>>(() => new Set());
  const [outcomes, setOutcomes] = useState<Record<string, ResetOutcomeKey>>({});
  const [pendingCredit, setPendingCredit] = useState<PendingCredit | null>(null);
  const [redeemingCreditId, setRedeemingCreditId] = useState<string | null>(null);
  const isMounted = useRef(false);
  const connectionLoadVersion = useRef(0);
  const workspaceLoadVersions = useRef(new Map<string, number>());

  const loadWorkspaceUsage = useCallback(async (
    connection: UsageConnection,
    workspace: UsageWorkspace,
    userInitiated = false,
  ) => {
    const key = workspaceKey(connection.id, workspace.id);
    const version = (workspaceLoadVersions.current.get(key) ?? 0) + 1;
    workspaceLoadVersions.current.set(key, version);
    setLoadingKeys((current) => new Set(current).add(key));
    setErrors((current) => {
      const next = { ...current };
      delete next[key];
      return next;
    });

    const requestIsCurrent = () =>
      isMounted.current && workspaceLoadVersions.current.get(key) === version;

    try {
      const response = await localServiceClient.call('chatgpt.usage.get', {
        connectionId: connection.id,
        workspaceId: workspace.id,
      });
      const snapshot = parseUsageSnapshot(response);
      if (!requestIsCurrent()) return;
      setSnapshots((current) => ({ ...current, [key]: snapshot }));
      if (userInitiated) {
        setRefreshRequiredKeys((current) => {
          const next = new Set(current);
          next.delete(key);
          return next;
        });
      }
    } catch {
      if (!requestIsCurrent()) return;
      setErrors((current) => ({ ...current, [key]: 'usageLoadFailed' }));
    } finally {
      if (requestIsCurrent()) {
        setLoadingKeys((current) => {
          const next = new Set(current);
          next.delete(key);
          return next;
        });
      }
    }
  }, []);

  const loadAll = useCallback(async (forceRefresh = false) => {
    const version = ++connectionLoadVersion.current;
    setLoadState('loading');
    setIsRefreshingAll(forceRefresh);
    try {
      const response = await localServiceClient.call('chatgpt.connections.list');
      const loadedConnections = parseConnections(response.connections);
      if (!isMounted.current || connectionLoadVersion.current !== version) return;
      setConnections(loadedConnections);
      setLoadState('loaded');
      await Promise.all(loadedConnections.flatMap((connection) =>
        connection.workspaces.map((workspace) => loadWorkspaceUsage(connection, workspace, forceRefresh)),
      ));
    } catch {
      if (isMounted.current && connectionLoadVersion.current === version) {
        setLoadState('failed');
      }
    } finally {
      if (isMounted.current && connectionLoadVersion.current === version) {
        setIsRefreshingAll(false);
      }
    }
  }, [loadWorkspaceUsage]);

  useEffect(() => {
    isMounted.current = true;
    void loadAll();
    return () => {
      isMounted.current = false;
      connectionLoadVersion.current += 1;
      workspaceLoadVersions.current.clear();
    };
  }, [loadAll]);

  const redeemCredit = async () => {
    const pending = pendingCredit;
    if (!pending || redeemingCreditId !== null) return;

    const key = workspaceKey(pending.connection.id, pending.workspace.id);
    setRedeemingCreditId(pending.credit.id);
    let outcome: ResetOutcomeKey = 'resetCreditOutcomeUnknown';
    let requiresRefresh = true;
    try {
      const response = await localServiceClient.call('chatgpt.reset_credits.consume', {
        connectionId: pending.connection.id,
        workspaceId: pending.workspace.id,
        creditId: pending.credit.id,
      });
      switch (response.outcome) {
        case 'reset':
          outcome = 'resetCreditApplied';
          requiresRefresh = false;
          break;
        case 'alreadyRedeemed':
          outcome = 'resetCreditAlreadyUsed';
          break;
        case 'nothingToReset':
          outcome = 'resetCreditNothingToReset';
          requiresRefresh = false;
          break;
        case 'noCredit':
          outcome = 'resetCreditNoLongerAvailable';
          break;
        default:
          outcome = 'resetCreditOutcomeUnknown';
      }
    } catch (error) {
      if (error instanceof LocalServiceError) {
        switch (error.code) {
          case 'authentication_required':
            outcome = 'resetCreditSignInRequired';
            requiresRefresh = false;
            break;
          case 'permission_denied':
            outcome = 'resetCreditRejected';
            requiresRefresh = false;
            break;
          case 'reset_credit_unavailable':
          case 'workspace_not_found':
            outcome = 'resetCreditNoLongerAvailable';
            break;
          default:
            outcome = 'resetCreditOutcomeUnknown';
        }
      }
    } finally {
      setPendingCredit(null);
      setRedeemingCreditId(null);
      setOutcomes((current) => ({ ...current, [key]: outcome }));
      setRefreshRequiredKeys((current) => {
        const next = new Set(current);
        if (requiresRefresh) next.add(key);
        else next.delete(key);
        return next;
      });
      await loadWorkspaceUsage(pending.connection, pending.workspace, true);
    }
  };

  const locale = i18n.resolvedLanguage ?? i18n.language;
  const refreshWorkspace = (connection: UsageConnection) => {
    void Promise.all(connection.workspaces.map((workspace) =>
      loadWorkspaceUsage(connection, workspace, true),
    ));
  };

  return (
    <section aria-labelledby="usage-quotas-heading" className="grid gap-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-xl font-semibold leading-7" id="usage-quotas-heading">{t('usageQuotas')}</h2>
          <p className="mt-1 text-sm text-secondary-text">{t('usageQuotasDescription')}</p>
        </div>
        <Button
          disabled={isRefreshingAll || loadState === 'loading'}
          onClick={() => void loadAll(true)}
          variant="secondary"
        >
          {isRefreshingAll ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : <RefreshCw aria-hidden="true" size={16} />}
          {t('refreshAll')}
        </Button>
      </div>

      {loadState === 'loading' && connections.length === 0 ? (
        <div aria-live="polite" className="flex min-h-24 items-center justify-center">
          <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-icon" size={22} />
        </div>
      ) : loadState === 'failed' ? (
        <Card className="flex flex-wrap items-center justify-between gap-3 p-5" role="alert">
          <CircleAlert aria-hidden="true" className="shrink-0 text-destructive" size={22} />
          <p className="text-sm text-destructive">{t('usageQuotasLoadFailed')}</p>
          <Button onClick={() => void loadAll(true)} size="small" variant="secondary">{t('retry')}</Button>
        </Card>
      ) : connections.length === 0 ? (
        <Card className="grid justify-items-center gap-3 p-6 text-center">
          <ChartNoAxesCombined aria-hidden="true" className="text-secondary-icon" size={36} />
          <div className="grid gap-1">
            <h3 className="font-semibold">{t('noChatGptAccountsForQuota')}</h3>
            <p className="max-w-md text-sm text-secondary-text">{t('noChatGptAccountsForQuotaDescription')}</p>
          </div>
          <Button onClick={onNavigateToConnections} size="small" variant="secondary">{t('goToConnections')}</Button>
        </Card>
      ) : (
        <div className="grid gap-4">
          {connections.map((connection) => {
            const accountName = connection.email ?? connection.displayName ?? connection.id;
            return (
              <Card
                className={`grid gap-4 p-5 ${connection.isSelected ? 'border-accent' : ''}`}
                key={connection.id}
              >
                <header className="flex min-w-0 items-start gap-3">
                  <ProviderIcon className="mt-1" providerId="chatgpt" size={22} />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="truncate font-semibold">{accountName}</h3>
                      {connection.isSelected ? (
                        <span className="rounded-sm bg-selected px-2 py-0.5 text-xs font-semibold text-accent">
                          {t('activeAccountBadge')}
                        </span>
                      ) : null}
                    </div>
                    <p className="mt-1 text-sm text-secondary-text">
                      {t('accountUsage', { plan: connection.planType ?? t('planUnavailable') })}
                    </p>
                  </div>
                  <Button
                    aria-label={t('refreshUsage')}
                    disabled={connection.workspaces.some((workspace) => loadingKeys.has(workspaceKey(connection.id, workspace.id)))}
                    onClick={() => refreshWorkspace(connection)}
                    size="icon"
                    variant="ghost"
                  >
                    <RefreshCw aria-hidden="true" size={16} />
                  </Button>
                </header>

                <div className="grid gap-4 border-t border-border pt-4">
                  {connection.workspaces.map((workspace, index) => (
                    <WorkspaceUsage
                      connection={connection}
                      errors={errors}
                      index={index}
                      key={workspace.id}
                      locale={locale}
                      loadingKeys={loadingKeys}
                      onOpenCredit={setPendingCredit}
                      onRetry={() => void loadWorkspaceUsage(connection, workspace, true)}
                      outcomes={outcomes}
                      refreshRequiredKeys={refreshRequiredKeys}
                      snapshot={snapshots[workspaceKey(connection.id, workspace.id)] ?? null}
                      t={t}
                      workspace={workspace}
                    />
                  ))}
                </div>
              </Card>
            );
          })}
        </div>
      )}

      <Dialog
        onOpenChange={(open) => {
          if (!open && redeemingCreditId === null) setPendingCredit(null);
        }}
        open={pendingCredit !== null}
        title={t('confirmResetCreditTitle')}
      >
        <div className="grid gap-5">
          <p className="text-sm text-secondary-text">{t('confirmResetCreditMessage')}</p>
          <div className="flex flex-wrap justify-end gap-2">
            <Button disabled={redeemingCreditId !== null} onClick={() => setPendingCredit(null)} variant="ghost">
              {t('cancel')}
            </Button>
            <Button disabled={redeemingCreditId !== null} onClick={() => void redeemCredit()} variant="primary">
              {redeemingCreditId === pendingCredit?.credit.id
                ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                : <RotateCcw aria-hidden="true" size={16} />}
              {t(redeemingCreditId === pendingCredit?.credit.id ? 'resetCreditRedeeming' : 'confirmResetCreditAction')}
            </Button>
          </div>
        </div>
      </Dialog>
    </section>
  );
}

interface WorkspaceUsageProps {
  connection: UsageConnection;
  errors: Record<string, 'usageLoadFailed'>;
  index: number;
  locale: string;
  loadingKeys: Set<string>;
  onOpenCredit(credit: PendingCredit): void;
  onRetry(): void;
  outcomes: Record<string, ResetOutcomeKey>;
  refreshRequiredKeys: Set<string>;
  snapshot: UsageSnapshot | null;
  t: TFunction;
  workspace: UsageWorkspace;
}

function WorkspaceUsage({
  connection,
  errors,
  index,
  locale,
  loadingKeys,
  onOpenCredit,
  onRetry,
  outcomes,
  refreshRequiredKeys,
  snapshot,
  t,
  workspace,
}: WorkspaceUsageProps) {
  const key = workspaceKey(connection.id, workspace.id);
  const isLoading = loadingKeys.has(key);
  const error = errors[key];

  return (
    <section aria-label={connection.workspaces.length > 1
      ? workspace.displayName?.trim() || t('workspaceNumbered', { number: index + 1 })
      : t('workspace')}
    >
      {connection.workspaces.length > 1 ? (
        <h4 className="mb-2 text-sm font-medium text-secondary-text">
          {workspace.displayName?.trim()
            ? t('workspaceQuotaLabel', { name: workspace.displayName.trim() })
            : t('workspaceNumbered', { number: index + 1 })}
        </h4>
      ) : null}

      {isLoading && snapshot === null ? (
        <div aria-live="polite" className="flex items-center justify-center py-4">
          <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-icon" size={20} />
        </div>
      ) : error && snapshot === null ? (
        <div className="flex flex-wrap items-center justify-between gap-3 py-2" role="alert">
          <p className="text-sm text-destructive">{t(error)}</p>
          <Button onClick={onRetry} size="small" variant="ghost">{t('retry')}</Button>
        </div>
      ) : snapshot === null ? (
        <p className="py-2 text-sm text-secondary-text">{t('ordinaryUsageUnknown')}</p>
      ) : (
        <div className="grid gap-3">
          {isLoading ? <div aria-label={t('usageLoading')} className="h-0.5 animate-pulse rounded-full bg-accent" /> : null}
          <div className="flex flex-wrap items-center justify-between gap-2 text-sm">
            <UsagePermissionStatus value={snapshot.ordinaryUsageAllowed} t={t} />
            <span className="text-xs text-secondary-text">
              {t('usageUpdatedAt', { time: formatTimestamp(snapshot.fetchedAtUnixMs, locale) })}
            </span>
          </div>
          {snapshot.buckets.map((bucket, bucketIndex) => (
            <UsageBucketView bucket={bucket} key={`${bucket.limitId}:${bucketIndex}`} locale={locale} t={t} />
          ))}
          {snapshot.resetCredits.length > 0 || snapshot.resetCreditCount !== null ? (
            <ResetCreditsView
              connection={connection}
              onOpenCredit={onOpenCredit}
              outcomes={outcomes}
              refreshRequired={refreshRequiredKeys.has(key)}
              snapshot={snapshot}
              t={t}
              workspace={workspace}
            />
          ) : null}
          {error ? <p className="text-sm text-warning" role="status">{t(error)}</p> : null}
        </div>
      )}
    </section>
  );
}

function UsagePermissionStatus({
  value,
  t,
}: {
  value: boolean | null;
  t: WorkspaceUsageProps['t'];
}) {
  const label = value === true
    ? t('ordinaryUsageAvailable')
    : value === false
      ? t('ordinaryUsageUnavailable')
      : t('ordinaryUsageUnknown');
  const color = value === true
    ? 'text-success'
    : value === false
      ? 'text-destructive'
      : 'text-secondary-text';
  return <span className={`font-medium ${color}`}>{label}</span>;
}

function UsageBucketView({
  bucket,
  locale,
  t,
}: {
  bucket: UsageBucket;
  locale: string;
  t: WorkspaceUsageProps['t'];
}) {
  const label = bucketLabel(bucket, t);
  const percent = bucket.usedPercent;
  const displayedPercent = percent === null ? null : Math.round(percent);
  const meterValue = percent === null ? null : Math.min(100, Math.max(0, percent));

  return (
    <div className="grid gap-1.5">
      <div className="flex flex-wrap items-center justify-between gap-2 text-sm">
        <span className="font-medium">{label}</span>
        <span className="text-secondary-text">
          {displayedPercent === null
            ? t('unavailableValue')
            : t('usageUsedPercent', { percent: displayedPercent })}
        </span>
      </div>
      <div
        aria-label={label}
        aria-valuemax={100}
        aria-valuemin={0}
        aria-valuenow={meterValue ?? undefined}
        aria-valuetext={displayedPercent === null
          ? t('unavailableValue')
          : t('usageUsedPercent', { percent: displayedPercent })}
        className="h-1.5 overflow-hidden rounded-full bg-selected"
        role="meter"
      >
        <div
          className={meterValue === null
            ? 'h-full w-1/3 animate-pulse rounded-full bg-accent'
            : 'h-full rounded-full bg-accent'}
          style={meterValue === null ? undefined : { width: `${meterValue}%` }}
        />
      </div>
      {bucket.resetAtUnixMs !== null ? (
        <span className="text-xs text-secondary-text">
          {t('quotaResetsAt', { time: formatTimestamp(bucket.resetAtUnixMs, locale) })}
        </span>
      ) : null}
    </div>
  );
}

function ResetCreditsView({
  connection,
  onOpenCredit,
  outcomes,
  refreshRequired,
  snapshot,
  t,
  workspace,
}: {
  connection: UsageConnection;
  onOpenCredit(credit: PendingCredit): void;
  outcomes: Record<string, ResetOutcomeKey>;
  refreshRequired: boolean;
  snapshot: UsageSnapshot;
  t: WorkspaceUsageProps['t'];
  workspace: UsageWorkspace;
}) {
  const key = workspaceKey(connection.id, workspace.id);
  const outcome = outcomes[key];
  return (
    <div className="grid gap-3 rounded-control border border-border bg-selected/40 p-3">
      <div className="flex items-center gap-2">
        <RotateCcw aria-hidden="true" className="shrink-0 text-accent" size={18} />
        <p className="font-semibold">
          {snapshot.resetCreditCount === null
            ? t('resetCreditCountUnavailable')
            : t('resetCreditsAvailable', { count: snapshot.resetCreditCount })}
        </p>
      </div>
      {snapshot.resetCredits.map((credit) => {
        const metadata = creditMetadata(credit, t);
        return (
          <div className="flex flex-col gap-3 border-t border-border pt-3 sm:flex-row sm:items-start sm:justify-between" key={credit.id}>
            <div className="min-w-0">
              <p className="font-medium">{credit.title ?? credit.resetType ?? t('resetCredit')}</p>
              {metadata.length > 0 ? (
                <p className="mt-1 text-xs text-secondary-text">{metadata.join(' · ')}</p>
              ) : null}
            </div>
            {credit.status === 'available' ? (
              <Button
                disabled={refreshRequired}
                onClick={() => onOpenCredit({ connection, workspace, credit })}
                size="small"
                variant="secondary"
              >
                <RotateCcw aria-hidden="true" size={15} />
                {t('useResetCredit')}
              </Button>
            ) : null}
          </div>
        );
      })}
      {snapshot.resetCredits.length === 0 && snapshot.resetCreditCount !== null ? (
        <p className="text-sm text-secondary-text">{t('resetCreditDetailsUnavailable')}</p>
      ) : null}
      {refreshRequired ? <p className="text-sm text-destructive" role="alert">{t('resetCreditRefreshRequired')}</p> : null}
      {outcome ? <p className="text-sm text-secondary-text" role="status">{t(outcome)}</p> : null}
    </div>
  );
}

function creditMetadata(credit: ResetCredit, t: WorkspaceUsageProps['t']): string[] {
  return [
    credit.status === 'available' ? t('resetCreditAvailableStatus') : credit.status,
    credit.grantedAtUnixMs === null ? null : t('creditGrantedAt', { time: formatTimestamp(credit.grantedAtUnixMs) }),
    credit.expiresAtUnixMs === null ? null : t('creditExpiresAt', { time: formatTimestamp(credit.expiresAtUnixMs) }),
  ].filter((value): value is string => typeof value === 'string' && value.length > 0);
}

function bucketLabel(bucket: UsageBucket, t: WorkspaceUsageProps['t']): string {
  switch (bucket.windowSeconds) {
    case 18000:
      return t('usageFiveHour');
    case 604800:
      return t('usageWeekly');
    default:
      if (bucket.windowSeconds !== null && bucket.windowSeconds >= 2419200 && bucket.windowSeconds <= 2678400) {
        return t('usageMonthly');
      }
      return bucket.limitId;
  }
}

function workspaceKey(connectionId: string, workspaceId: string): string {
  return JSON.stringify([connectionId, workspaceId]);
}

function parseConnections(value: unknown): UsageConnection[] {
  if (!Array.isArray(value)) throw new Error('The ChatGPT connection list was invalid.');
  return value.map((item): UsageConnection => {
    if (!isObject(item) || !isNonEmptyString(item.id) || typeof item.authStatus !== 'string' || !Array.isArray(item.workspaces)) {
      throw new Error('A ChatGPT connection was invalid.');
    }
    return {
      id: item.id,
      email: optionalString(item.email),
      displayName: optionalString(item.displayName),
      planType: optionalString(item.planType),
      isSelected: item.isSelected === true,
      workspaces: item.workspaces.map((workspace): UsageWorkspace => {
        if (!isObject(workspace) || !isNonEmptyString(workspace.id)) {
          throw new Error('A ChatGPT workspace was invalid.');
        }
        return { id: workspace.id, displayName: optionalString(workspace.displayName) };
      }),
    };
  });
}

function parseUsageSnapshot(value: Record<string, unknown>): UsageSnapshot {
  const ordinaryUsageAllowed = value.ordinaryUsageAllowed;
  const resetCreditCount = value.resetCreditCount;
  if (!isValidTimestamp(value.fetchedAtUnixMs) || !isNonEmptyString(value.freshness) ||
    !isNonEmptyString(value.resetCreditDetailsState) || !Array.isArray(value.buckets) ||
    !Array.isArray(value.resetCredits) ||
    !isNullableBoolean(ordinaryUsageAllowed) || !isNullableSafeInteger(resetCreditCount)) {
    throw new Error('The ChatGPT usage snapshot was invalid.');
  }
  return {
    fetchedAtUnixMs: value.fetchedAtUnixMs,
    ordinaryUsageAllowed: ordinaryUsageAllowed ?? null,
    resetCreditCount: resetCreditCount ?? null,
    buckets: value.buckets.map(parseUsageBucket),
    resetCredits: value.resetCredits.map(parseResetCredit),
  };
}

function parseUsageBucket(value: unknown): UsageBucket {
  if (!isObject(value) || !isNonEmptyString(value.limitId) ||
    !isNullableNumber(value.usedPercent) || !isNullableSafeInteger(value.windowSeconds) ||
    !isNullableTimestamp(value.resetAtUnixMs)) {
    throw new Error('A ChatGPT usage bucket was invalid.');
  }
  return {
    limitId: value.limitId,
    usedPercent: value.usedPercent ?? null,
    windowSeconds: value.windowSeconds ?? null,
    resetAtUnixMs: value.resetAtUnixMs ?? null,
  };
}

function parseResetCredit(value: unknown): ResetCredit {
  if (!isObject(value) || !isNonEmptyString(value.id) ||
    !isNullableString(value.resetType) || !isNullableString(value.status) ||
    !isNullableTimestamp(value.grantedAtUnixMs) || !isNullableTimestamp(value.expiresAtUnixMs) ||
    !isNullableString(value.title) || !isNullableString(value.description)) {
    throw new Error('A ChatGPT reset credit was invalid.');
  }
  return {
    id: value.id,
    resetType: value.resetType ?? null,
    status: value.status ?? null,
    grantedAtUnixMs: value.grantedAtUnixMs ?? null,
    expiresAtUnixMs: value.expiresAtUnixMs ?? null,
    title: value.title ?? null,
    description: value.description ?? null,
  };
}

function formatTimestamp(timestamp: number, locale?: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(timestamp);
}

function isValidTimestamp(value: unknown): value is number {
  return isSafeInteger(value) && Math.abs(value) <= 8.64e15;
}

function isNullableTimestamp(value: unknown): value is number | null | undefined {
  return value === null || value === undefined || isValidTimestamp(value);
}

function isNullableNumber(value: unknown): value is number | null | undefined {
  return value === null || value === undefined || (typeof value === 'number' && Number.isFinite(value));
}

function isNullableSafeInteger(value: unknown): value is number | null | undefined {
  return value === null || value === undefined || isSafeInteger(value);
}

function isNullableBoolean(value: unknown): value is boolean | null | undefined {
  return value === null || value === undefined || typeof value === 'boolean';
}

function isSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value);
}

function optionalString(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string') throw new Error('A ChatGPT response field was invalid.');
  return value;
}

function isNullableString(value: unknown): value is string | null | undefined {
  return value === null || value === undefined || typeof value === 'string';
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
