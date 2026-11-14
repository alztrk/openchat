import { Download, LoaderCircle, RefreshCw, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@shared/components';
import { AudioDeviceSettingsSection } from './AudioDeviceSettingsSection';
import { LocalServiceError, localServiceClient, type ServiceOperation } from '@shared/lib/platform/local-service';
import { toast } from '@shared/lib/toast';
import {
  parseVoiceInputInstallProgress,
  parseVoiceInputModelStatus,
  type VoiceInputInstallProgress,
} from '@entities/voice-input/model';

type ModelState = 'loading' | 'missing' | 'ready' | 'installing' | 'error';
const installedModelNames = ['Whisper base', 'Silero VAD 6.2.0'] as const;

export function VoiceInputModelSettingsSection() {
  const { t } = useTranslation();
  const [modelState, setModelState] = useState<ModelState>('loading');
  const [installProgress, setInstallProgress] = useState<VoiceInputInstallProgress | null>(null);
  const [isCancelling, setIsCancelling] = useState(false);
  const [isUninstalling, setIsUninstalling] = useState(false);
  const mountedRef = useRef(true);
  const activeOperationRef = useRef<ServiceOperation | null>(null);
  const installGenerationRef = useRef(0);
  const cancelRequestedGenerationRef = useRef<number | null>(null);

  const loadModelStatus = useCallback(async () => {
    setModelState('loading');
    try {
      const response = await localServiceClient.call('voice.input.model.status');
      const status = parseVoiceInputModelStatus(response);
      if (!mountedRef.current) return;
      setModelState(status.installed ? 'ready' : 'missing');
    } catch {
      if (!mountedRef.current) return;
      setModelState('error');
      toast.error(t('voiceInputModelStatusFailed'));
    }
  }, [t]);

  useEffect(() => {
    mountedRef.current = true;
    void loadModelStatus();
    return () => {
      mountedRef.current = false;
      const operation = activeOperationRef.current;
      if (operation) {
        void operation.cancel().catch((cause: unknown) => {
          globalThis.reportError(new Error('The local voice model installation could not be cancelled.', { cause }));
        });
      }
    };
  }, [loadModelStatus]);

  const installModel = useCallback(async () => {
    if (modelState !== 'missing' && modelState !== 'error') return;
    const generation = installGenerationRef.current + 1;
    installGenerationRef.current = generation;
    cancelRequestedGenerationRef.current = null;
    setModelState('installing');
    setInstallProgress(null);
    setIsCancelling(false);

    let operation: ServiceOperation | null = null;
    try {
      operation = await localServiceClient.startOperation('voice.input.model.install', {}, (event) => {
        if (generation !== installGenerationRef.current) return;
        const progress = parseVoiceInputInstallProgress(event);
        if (progress && mountedRef.current) setInstallProgress(progress);
      });
      if (!mountedRef.current) {
        await operation.cancel();
        return;
      }
      activeOperationRef.current = operation;
      await operation.result;
      const statusResponse = await localServiceClient.call('voice.input.model.status');
      const status = parseVoiceInputModelStatus(statusResponse);
      if (!status.installed) throw new Error('The local voice model installation was not confirmed.');
      if (mountedRef.current) {
        setModelState('ready');
      }
    } catch (cause) {
      const wasCancelled = cancelRequestedGenerationRef.current === generation;
      if (!mountedRef.current) return;
      setModelState('missing');
      if (!wasCancelled && !(cause instanceof LocalServiceError && cause.code === 'operation_cancelled')) {
        toast.error(t('voiceInputModelInstallFailed'));
      }
    } finally {
      if (activeOperationRef.current === operation) activeOperationRef.current = null;
      if (mountedRef.current) {
        setInstallProgress(null);
        setIsCancelling(false);
      }
    }
  }, [modelState, t]);

  const cancelInstall = useCallback(async () => {
    const operation = activeOperationRef.current;
    if (!operation || isCancelling) return;
    const generation = installGenerationRef.current;
    cancelRequestedGenerationRef.current = generation;
    setIsCancelling(true);
    try {
      const cancelled = await operation.cancel();
      if (!cancelled) {
        cancelRequestedGenerationRef.current = null;
        if (mountedRef.current) {
          setIsCancelling(false);
          toast.error(t('voiceInputModelCancelFailed'));
        }
      }
    } catch (cause) {
      cancelRequestedGenerationRef.current = null;
      if (mountedRef.current) {
        setIsCancelling(false);
        toast.error(t('voiceInputModelCancelFailed'));
      }
      globalThis.reportError(new Error('The local voice model installation cancellation failed.', { cause }));
    }
  }, [isCancelling, t]);

  const uninstallModel = useCallback(async () => {
    if (modelState !== 'ready' || isUninstalling) return;
    setIsUninstalling(true);
    try {
      const response = await localServiceClient.call('voice.input.model.uninstall');
      if (response.removed !== true) throw new Error('The local voice models were not removed.');
      if (!mountedRef.current) return;
      setModelState('missing');
      toast.success(t('voiceInputModelUninstalled'));
    } catch (error) {
      if (!mountedRef.current) return;
      toast.error(
        error instanceof LocalServiceError && error.code === 'method_not_found'
          ? t('voiceInputUninstallNeedsRestart')
          : t('voiceInputModelUninstallFailed'),
      );
    } finally {
      if (mountedRef.current) setIsUninstalling(false);
    }
  }, [isUninstalling, modelState, t]);

  const progressValue =
    installProgress && installProgress.totalBytes > 0
      ? Math.min(installProgress.totalDownloadedBytes, installProgress.totalBytes)
      : undefined;

  return (
    <section aria-labelledby="settings-page-heading" className="settings-data-page">
      <AudioDeviceSettingsSection />
      <Card className="grid gap-4 bg-raised" padding="default">
        <h2 className="text-base font-semibold text-text">{t('voiceInputModelSettingsTitle')}</h2>

        {modelState === 'loading' ? (
          <p aria-live="polite" className="flex items-center gap-2 text-sm text-secondary-text" role="status">
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            {t('voiceInputModelLoading')}
          </p>
        ) : null}

        {modelState === 'error' ? (
          <div className="flex flex-wrap items-center gap-3">
            <p className="text-sm text-destructive" role="alert">
              {t('voiceInputModelStatusUnavailable')}
            </p>
            <Button onClick={() => void loadModelStatus()} size="small" variant="secondary">
              <RefreshCw aria-hidden="true" size={15} />
              {t('voiceInputRetryModel')}
            </Button>
          </div>
        ) : null}

        {modelState === 'missing' ? (
          <Button disabled={isUninstalling} onClick={() => void installModel()} variant="primary">
            <Download aria-hidden="true" size={16} />
            {t('voiceInputInstallModel')}
          </Button>
        ) : null}

        {modelState === 'installing' ? (
          <div aria-live="polite" className="flex min-w-0 items-center gap-3" role="status">
            <div className="grid min-w-0 flex-1 gap-2">
              <span className="truncate text-sm text-secondary-text">
                {installProgress
                  ? t('voiceInputInstallFileProgress', {
                      fileName: installProgress.fileName,
                      fileIndex: installProgress.fileIndex,
                      fileCount: installProgress.fileCount,
                    })
                  : t('voiceInputInstallingModel')}
              </span>
              {installProgress ? (
                <progress
                  aria-label={t('voiceInputInstallProgress')}
                  className="h-1.5 w-full accent-accent"
                  max={installProgress.totalBytes || 1}
                  value={progressValue ?? 0}
                />
              ) : null}
            </div>
            <Button
              aria-label={t(isCancelling ? 'voiceInputCancellingInstall' : 'voiceInputCancelInstall')}
              disabled={isCancelling}
              onClick={() => void cancelInstall()}
              size="icon"
              title={t(isCancelling ? 'voiceInputCancellingInstall' : 'voiceInputCancelInstall')}
              variant="ghost"
            >
              {isCancelling ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={17} />
              ) : (
                <X aria-hidden="true" size={17} />
              )}
            </Button>
          </div>
        ) : null}

        {modelState === 'ready' ? (
          <div aria-live="polite" className="flex flex-wrap items-center justify-between gap-3" role="status">
            <div className="grid gap-1">
              {installedModelNames.map((modelName) => (
                <p className="text-sm text-text" key={modelName}>{modelName}</p>
              ))}
            </div>
            <Button
              disabled={isUninstalling}
              loading={isUninstalling}
              onClick={() => void uninstallModel()}
              variant="destructive"
            >
              {!isUninstalling ? <Trash2 aria-hidden="true" size={16} /> : null}
              {t(isUninstalling ? 'voiceInputUninstallingModel' : 'voiceInputUninstallModel')}
            </Button>
          </div>
        ) : null}
      </Card>
    </section>
  );
}
