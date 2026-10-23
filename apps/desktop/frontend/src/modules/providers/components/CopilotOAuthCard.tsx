import { CircleAlert, CircleCheck, CircleUserRound, LoaderCircle, LogIn, RefreshCw, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog } from '@/design-system';
import {
  type CopilotDeviceFlow,
  type CopilotOAuthConnection,
  cancelCopilotOAuth,
  disconnectCopilotOAuth,
  loadCopilotOAuthStatus,
  loadCopilotUsage,
  pollCopilotOAuth,
  selectCopilotOAuthConnection,
  startCopilotOAuth,
} from '@/modules/providers/data/copilotOAuth';
import {
  getCachedCopilotPlanType,
  refreshOAuthPlans,
  subscribeToOAuthPlanUpdates,
} from '@/modules/providers/data/oauthPlanMonitor';
import { LocalServiceError } from '@/platform/local-service';
import { ProviderIcon } from './ProviderIcon';

interface CopilotOAuthCardProps {
  serviceReady: boolean;
  onChanged(): Promise<void>;
}

interface ActiveFlow extends CopilotDeviceFlow {
  retryAfterSeconds: number;
}

type CardError =
  | 'copilotOAuthLoadFailed'
  | 'copilotOAuthStartFailed'
  | 'copilotOAuthPollFailed'
  | 'copilotOAuthExpired'
  | 'copilotOAuthDenied'
  | 'copilotOAuthCancelFailed'
  | 'copilotOAuthDisconnectFailed'
  | 'copilotOAuthRefreshRequired'
  | 'copilotOAuthSelectionFailed';

const copilotErrorKeys: Record<string, CardError> = {
  oauth_access_denied: 'copilotOAuthDenied',
  oauth_authorization_rejected: 'copilotOAuthDenied',
  oauth_flow_expired: 'copilotOAuthExpired',
  oauth_refresh_unavailable: 'copilotOAuthRefreshRequired',
};

export function CopilotOAuthCard({ serviceReady, onChanged }: CopilotOAuthCardProps) {
  const { t } = useTranslation();
  const [connections, setConnections] = useState<CopilotOAuthConnection[]>([]);
  const [selectedConnectionId, setSelectedConnectionId] = useState<string | null>(null);
  const [removeTarget, setRemoveTarget] = useState<CopilotOAuthConnection | null>(null);
  const [removingId, setRemovingId] = useState<string | null>(null);
  const [planType, setPlanType] = useState<string | null>(() => getCachedCopilotPlanType());
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [flow, setFlow] = useState<ActiveFlow | null>(null);
  const [pollPaused, setPollPaused] = useState(false);
  const [error, setError] = useState<CardError | null>(null);
  const [notice, setNotice] = useState(false);
  const flowGenerationRef = useRef(0);
  const isMountedRef = useRef(false);
  const selectedConnectionIdRef = useRef<string | null>(null);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      flowGenerationRef.current += 1;
    };
  }, []);

  useEffect(() => {
    return subscribeToOAuthPlanUpdates((summary) => {
      if (isMountedRef.current && selectedConnectionIdRef.current === summary.copilotConnectionId) {
        setPlanType(summary.copilotPlanType);
      }
    });
  }, []);

  const loadStatus = useCallback(async () => {
    if (isMountedRef.current) setLoading(true);
    try {
      const status = await loadCopilotOAuthStatus();
      if (!isMountedRef.current) return;
      setConnections(status.connections);
      selectedConnectionIdRef.current = status.selectedConnectionId;
      setSelectedConnectionId(status.selectedConnectionId);
      setError(null);
      if (status.connected && status.connection?.authStatus === 'active') {
        void loadCopilotUsage()
          .then((usage) => {
            if (isMountedRef.current) setPlanType(usage.planType);
          })
          .catch((error) => {
            globalThis.reportError(new Error('Failed to load Copilot plan usage.', { cause: error }));
          });
      } else {
        setPlanType(null);
      }
    } catch {
      if (isMountedRef.current) setError('copilotOAuthLoadFailed');
    } finally {
      if (isMountedRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (serviceReady) void loadStatus();
  }, [loadStatus, serviceReady]);

  useEffect(() => {
    if (!flow || pollPaused) return;
    const generation = flowGenerationRef.current;
    const delay = Math.max(flow.retryAfterSeconds, 1) * 1000;
    const timer = window.setTimeout(() => {
      void (async () => {
        const isCurrentFlow = () => generation === flowGenerationRef.current && isMountedRef.current;
        if (!isCurrentFlow()) return;
        if (Date.now() >= flow.expiresAtUnixMs) {
          setError('copilotOAuthExpired');
          setFlow(null);
          void cancelCopilotOAuth(flow.flowId)
            .then(async (cancelled) => {
              if (cancelled || !isCurrentFlow()) return;
              try {
                const status = await loadCopilotOAuthStatus();
                if (!isCurrentFlow()) return;
                setConnections(status.connections);
                selectedConnectionIdRef.current = status.selectedConnectionId;
                setSelectedConnectionId(status.selectedConnectionId);
                setError(status.connected ? null : 'copilotOAuthExpired');
              } catch {
                if (isCurrentFlow()) setError('copilotOAuthLoadFailed');
              }
            })
            .catch(() => {
              if (isCurrentFlow()) setError('copilotOAuthCancelFailed');
            });
          return;
        }
        try {
          const result = await pollCopilotOAuth(flow.flowId);
          if (!isCurrentFlow()) return;
          if (result.status === 'connected') {
            setFlow(null);
            setPollPaused(false);
            setError(null);
            try {
              const status = await loadCopilotOAuthStatus();
              if (!isCurrentFlow()) return;
              setConnections(status.connections);
              selectedConnectionIdRef.current = status.selectedConnectionId;
              setSelectedConnectionId(status.selectedConnectionId);
              if (status.connected) {
                const usage = await loadCopilotUsage();
                if (isCurrentFlow()) setPlanType(usage.planType);
              } else {
                setPlanType(null);
              }
              await Promise.all([onChanged(), refreshOAuthPlans()]);
            } catch {
              if (isCurrentFlow()) setError('copilotOAuthLoadFailed');
            }
            return;
          }
          setFlow((current) =>
            current?.flowId === flow.flowId
              ? {
                  ...current,
                  intervalSeconds: result.intervalSeconds,
                  retryAfterSeconds: result.retryAfterSeconds,
                }
              : current,
          );
        } catch (cause) {
          if (!isCurrentFlow()) return;
          const code = errorCode(cause);
          if (code === 'oauth_flow_expired') {
            setFlow(null);
            setError('copilotOAuthExpired');
          } else if (code === 'oauth_access_denied') {
            setFlow(null);
            setError('copilotOAuthDenied');
          } else {
            setPollPaused(true);
            setError('copilotOAuthPollFailed');
          }
        }
      })();
    }, delay);
    return () => window.clearTimeout(timer);
  }, [flow, onChanged, pollPaused]);

  const connect = async () => {
    if (working) return;
    const generation = ++flowGenerationRef.current;
    setWorking(true);
    setError(null);
    setNotice(false);
    try {
      const next = await startCopilotOAuth();
      if (generation !== flowGenerationRef.current || !isMountedRef.current) {
        void cancelCopilotOAuth(next.flowId).catch(() => {
          if (generation === flowGenerationRef.current && isMountedRef.current) {
            setError('copilotOAuthCancelFailed');
          }
        });
        return;
      }
      setPollPaused(false);
      setFlow({ ...next, retryAfterSeconds: next.intervalSeconds });
    } catch (cause) {
      if (generation === flowGenerationRef.current && isMountedRef.current) {
        setError(copilotErrorKey(cause, 'copilotOAuthStartFailed'));
      }
    } finally {
      if (generation === flowGenerationRef.current && isMountedRef.current) setWorking(false);
    }
  };

  const cancel = async () => {
    if (!flow || working) return;
    const activeFlow = flow;
    const generation = ++flowGenerationRef.current;
    setWorking(true);
    setFlow(null);
    setPollPaused(false);
    setError(null);
    setNotice(false);
    try {
      const cancelled = await cancelCopilotOAuth(activeFlow.flowId);
      if (!cancelled) {
        const status = await loadCopilotOAuthStatus();
        if (generation !== flowGenerationRef.current || !isMountedRef.current) return;
        setConnections(status.connections);
        selectedConnectionIdRef.current = status.selectedConnectionId;
        setSelectedConnectionId(status.selectedConnectionId);
        setError(status.connected ? null : 'copilotOAuthCancelFailed');
      }
    } catch {
      if (generation === flowGenerationRef.current && isMountedRef.current) {
        setFlow(activeFlow);
        setPollPaused(true);
        setError('copilotOAuthCancelFailed');
      }
    } finally {
      if (generation === flowGenerationRef.current && isMountedRef.current) setWorking(false);
    }
  };

  const disconnect = async () => {
    if (!removeTarget || working) return;
    const target = removeTarget;
    const generation = ++flowGenerationRef.current;
    setRemovingId(target.id);
    setWorking(true);
    setError(null);
    setNotice(false);
    let removed = false;
    try {
      await disconnectCopilotOAuth(target.id);
      removed = true;
    } catch {
      try {
        const status = await loadCopilotOAuthStatus();
        removed = !status.connections.some((connection) => connection.id === target.id);
      } catch {
        removed = false;
      }
    }

    if (generation !== flowGenerationRef.current || !isMountedRef.current) return;
    if (!removed) {
      await loadStatus();
      setError('copilotOAuthDisconnectFailed');
      setWorking(false);
      setRemovingId(null);
      return;
    }

    setRemoveTarget(null);
    setNotice(true);
    setWorking(false);
    setRemovingId(null);
    await loadStatus();
    try {
      await Promise.all([onChanged(), refreshOAuthPlans()]);
    } catch (cause) {
      globalThis.reportError(new Error('Provider state did not refresh after Copilot disconnect.', { cause }));
    }
  };

  const selectConnection = async (connectionId: string) => {
    if (working || flow !== null || connectionId === selectedConnectionId) return;
    setWorking(true);
    setError(null);
    setNotice(false);
    try {
      await selectCopilotOAuthConnection(connectionId);
      await loadStatus();
      await Promise.all([onChanged(), refreshOAuthPlans()]);
    } catch {
      setError('copilotOAuthSelectionFailed');
    } finally {
      if (isMountedRef.current) setWorking(false);
    }
  };

  const retryPolling = () => {
    setError(null);
    setPollPaused(false);
    setFlow((current) => (current ? { ...current, retryAfterSeconds: current.intervalSeconds } : null));
  };

  return (
    <Card className="overflow-hidden">
      <div className="flex flex-wrap items-start justify-between gap-4 p-5">
        <div className="flex min-w-0 items-start gap-3">
          <ProviderIcon className="mt-0.5" providerId="copilot" size={28} />
          <div className="min-w-0">
            <h3 className="font-semibold">{t('providerNameCopilot')}</h3>
            <p className="mt-1 max-w-2xl text-sm text-secondary-text">{t('copilotOAuthDescription')}</p>
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            disabled={!serviceReady || loading || working || flow !== null}
            onClick={() => void connect()}
            variant={connections.length === 0 ? 'primary' : 'secondary'}
          >
            {working ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            ) : (
              <LogIn aria-hidden="true" size={16} />
            )}
            {t(connections.length === 0 ? 'copilotOAuthConnect' : 'copilotOAuthAddAccount')}
          </Button>
        </div>
      </div>

      {loading ? (
        <div className="border-t border-border px-5 py-4" role="status">
          <p className="text-sm text-secondary-text">{t('copilotOAuthLoading')}</p>
        </div>
      ) : connections.length > 0 ? (
        <ul className="grid gap-3 border-t border-border px-5 py-3">
          {connections.map((item) => {
            const selected = item.id === selectedConnectionId;
            const needsSignIn = item.authStatus === 'reauth_required';
            return (
              <li
                className={`rounded-card border p-3 ${selected ? 'border-accent bg-selected' : 'border-border bg-raised'}`}
                key={item.id}
              >
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="flex min-w-0 items-start gap-3">
                    <CircleUserRound aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={20} />
                    <div className="min-w-0">
                      <p className="truncate font-medium">{item.name?.trim() || item.login}</p>
                      <p className="mt-1 truncate text-sm text-secondary-text">
                        @{item.login}
                        {selected && !needsSignIn && planType ? ` · ${t('accountPlan', { plan: planType })}` : null}
                      </p>
                      {needsSignIn ? (
                        <p className="mt-1 flex items-center gap-1 text-sm text-destructive">
                          <CircleAlert aria-hidden="true" size={15} />
                          {t('copilotOAuthRefreshRequired')}
                        </p>
                      ) : null}
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-2">
                    {needsSignIn ? (
                      <Button
                        disabled={working || flow !== null}
                        onClick={() => void connect()}
                        size="small"
                        variant="secondary"
                      >
                        <RefreshCw aria-hidden="true" size={15} />
                        {t('copilotOAuthReconnect')}
                      </Button>
                    ) : selected ? (
                      <span className="flex items-center gap-1 text-sm text-accent">
                        <CircleCheck aria-hidden="true" size={16} />
                        {t('connectionSelected')}
                      </span>
                    ) : (
                      <Button
                        disabled={working || flow !== null}
                        onClick={() => void selectConnection(item.id)}
                        size="small"
                        variant="secondary"
                      >
                        {t('useConnection')}
                      </Button>
                    )}
                    <Button
                      aria-label={t('copilotOAuthRemoveAccountLabel', { login: item.login })}
                      disabled={working || flow !== null}
                      onClick={() => setRemoveTarget(item)}
                      size="icon"
                      variant="ghost"
                    >
                      {removingId === item.id ? (
                        <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                      ) : (
                        <Trash2 aria-hidden="true" size={16} />
                      )}
                    </Button>
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      ) : loading ? null : (
        <p className="border-t border-border px-5 py-5 text-sm text-secondary-text">{t('copilotOAuthNoAccounts')}</p>
      )}

      {flow ? (
        <div className="grid gap-3 border-t border-border px-5 py-4">
          <p className="text-sm text-secondary-text">{t('copilotOAuthDeviceInstructions')}</p>
          <div className="flex flex-wrap items-center gap-3">
            <a
              className="text-sm font-medium text-accent underline underline-offset-2"
              href={flow.verificationUri}
              rel="noreferrer"
              target="_blank"
            >
              {t('copilotOAuthVerificationLink')}
            </a>
            <span className="inline-flex items-center gap-2">
              <span className="sr-only">{t('copilotOAuthDeviceCode')}</span>
              <code className="rounded-control border border-border bg-raised px-3 py-1.5 font-mono text-base font-semibold tracking-wide">
                {flow.userCode}
              </code>
            </span>
            <span aria-live="polite" className="text-sm text-secondary-text">
              {pollPaused ? t('copilotOAuthPaused') : t('copilotOAuthWaiting')}
            </span>
          </div>
          <div className="flex flex-wrap gap-2">
            {pollPaused ? (
              <Button onClick={retryPolling} size="small" variant="secondary">
                <RefreshCw aria-hidden="true" size={15} />
                {t('retry')}
              </Button>
            ) : null}
            <Button disabled={working} onClick={() => void cancel()} size="small" variant="secondary">
              {working ? <LoaderCircle aria-hidden="true" className="animate-spin" size={15} /> : null}
              {t('cancel')}
            </Button>
          </div>
        </div>
      ) : null}

      {error ? (
        <p className="border-t border-border px-5 py-3 text-sm text-destructive" role="alert">
          {t(error)}
        </p>
      ) : null}
      {notice ? (
        <p className="border-t border-border px-5 py-3 text-sm text-success" role="status">
          {t('copilotOAuthConnectionRemoved')}
        </p>
      ) : null}

      <Dialog
        onOpenChange={(open) => {
          if (!open && removingId === null) setRemoveTarget(null);
        }}
        open={removeTarget !== null}
        title={t('copilotOAuthRemoveAccount')}
      >
        {removeTarget ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">
              {t('copilotOAuthRemoveConfirmation', {
                name: removeTarget.name?.trim() || `@${removeTarget.login}`,
              })}
            </p>
            <div className="flex justify-end gap-2">
              <Button disabled={working} onClick={() => setRemoveTarget(null)}>
                {t('cancel')}
              </Button>
              <Button disabled={working} onClick={() => void disconnect()} variant="destructive">
                {working ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                ) : (
                  <Trash2 aria-hidden="true" size={16} />
                )}
                {t('removeConnectionAction')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
    </Card>
  );
}

function errorCode(error: unknown): string | null {
  if (error instanceof LocalServiceError) return error.code;
  return null;
}

function copilotErrorKey(error: unknown, fallback: CardError): CardError {
  if (!(error instanceof LocalServiceError)) return fallback;
  return copilotErrorKeys[error.code] ?? fallback;
}
