import { CircleAlert, LoaderCircle, LogIn, LogOut, RefreshCw } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@/design-system';
import {
  type CopilotDeviceFlow,
  type CopilotOAuthConnection,
  cancelCopilotOAuth,
  disconnectCopilotOAuth,
  loadCopilotOAuthStatus,
  pollCopilotOAuth,
  startCopilotOAuth,
} from '@/modules/providers/data/copilotOAuth';
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
  | 'copilotOAuthRefreshRequired';

export function CopilotOAuthCard({ serviceReady, onChanged }: CopilotOAuthCardProps) {
  const { t } = useTranslation();
  const [connection, setConnection] = useState<CopilotOAuthConnection | null>(null);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [flow, setFlow] = useState<ActiveFlow | null>(null);
  const [pollPaused, setPollPaused] = useState(false);
  const [error, setError] = useState<CardError | null>(null);
  const flowGenerationRef = useRef(0);
  const isMountedRef = useRef(false);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      flowGenerationRef.current += 1;
    };
  }, []);

  const loadStatus = useCallback(async () => {
    if (isMountedRef.current) setLoading(true);
    try {
      const status = await loadCopilotOAuthStatus();
      if (!isMountedRef.current) return;
      setConnection(status.connection);
      setError(null);
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
                setConnection(status.connection);
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
            setConnection(result.connection);
            setFlow(null);
            setPollPaused(false);
            setError(null);
            try {
              await onChanged();
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
    } catch {
      if (generation === flowGenerationRef.current && isMountedRef.current) setError('copilotOAuthStartFailed');
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
    try {
      const cancelled = await cancelCopilotOAuth(activeFlow.flowId);
      if (!cancelled) {
        const status = await loadCopilotOAuthStatus();
        if (generation !== flowGenerationRef.current || !isMountedRef.current) return;
        setConnection(status.connection);
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
    if (working) return;
    const generation = ++flowGenerationRef.current;
    setWorking(true);
    setError(null);
    try {
      await disconnectCopilotOAuth();
      if (generation !== flowGenerationRef.current || !isMountedRef.current) return;
      setConnection(null);
      await onChanged();
    } catch {
      if (generation !== flowGenerationRef.current || !isMountedRef.current) return;
      setError('copilotOAuthDisconnectFailed');
      await loadStatus();
    } finally {
      if (generation === flowGenerationRef.current && isMountedRef.current) setWorking(false);
    }
  };

  const retryPolling = () => {
    setError(null);
    setPollPaused(false);
    setFlow((current) => (current ? { ...current, retryAfterSeconds: current.intervalSeconds } : null));
  };

  const isReauthRequired = connection?.authStatus === 'reauth_required';

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
          {connection ? (
            <Button disabled={working || flow !== null} onClick={() => void connect()} variant="secondary">
              {working ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <RefreshCw aria-hidden="true" size={16} />
              )}
              {t('copilotOAuthReconnect')}
            </Button>
          ) : (
            <Button
              disabled={!serviceReady || loading || working || flow !== null}
              onClick={() => void connect()}
              variant="primary"
            >
              {working ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <LogIn aria-hidden="true" size={16} />
              )}
              {t('copilotOAuthConnect')}
            </Button>
          )}
          {connection ? (
            <Button disabled={working} onClick={() => void disconnect()} variant="secondary">
              {working ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <LogOut aria-hidden="true" size={16} />
              )}
              {t('copilotOAuthDisconnect')}
            </Button>
          ) : null}
        </div>
      </div>

      {loading ? (
        <div className="border-t border-border px-5 py-4" role="status">
          <p className="text-sm text-secondary-text">{t('copilotOAuthLoading')}</p>
        </div>
      ) : connection ? (
        <div className="flex flex-wrap items-center gap-2 border-t border-border px-5 py-4" role="status">
          <span className="text-sm font-medium">{connection.name ?? connection.login}</span>
          <span className="text-sm text-secondary-text">@{connection.login}</span>
          {isReauthRequired ? (
            <span className="inline-flex items-center gap-1 text-sm text-destructive">
              <CircleAlert aria-hidden="true" size={15} />
              {t('copilotOAuthRefreshRequired')}
            </span>
          ) : (
            <span className="text-sm text-success">{t('copilotOAuthConnected')}</span>
          )}
        </div>
      ) : null}

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
    </Card>
  );
}

function errorCode(error: unknown): string | null {
  if (error instanceof LocalServiceError) return error.code;
  return null;
}
