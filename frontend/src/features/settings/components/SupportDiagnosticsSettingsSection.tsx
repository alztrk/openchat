import { useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { Download, FileDown, LoaderCircle, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { LocalServiceError } from '@/shared/services/LocalServiceClient';
import { Button, Dialog } from '@/shared/ui';
import { localServiceClient, useChatStore } from '@/features/chat/stores/chatStore';
import { historyStorageStatusForInitialization } from '@/features/settings/data/historyStorageStatus';

interface SupportDiagnostics {
  formatVersion: 1;
  application: 'OpenChat';
  generatedAtUtc: string;
  runtime: {
    platform: 'windows';
    appVersion: string;
    runtime: 'Tauri';
  };
  storage: {
    historyStatus: 'loading' | 'available' | 'unavailable' | 'corrupt' | 'backupUnavailable';
    serviceStatus: 'ready';
    databaseSchemaVersion: number;
  };
}

type ErrorKey = 'supportDiagnosticsLoadFailed' | 'supportDiagnosticsInvalidResponse' | 'supportDiagnosticsExportFailed';

export function SupportDiagnosticsSettingsSection() {
  const { t } = useTranslation();
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const storageInitializationErrorCode = useChatStore((state) => state.storageInitializationErrorCode);
  const [isPreparing, setIsPreparing] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [previewBundle, setPreviewBundle] = useState<SupportDiagnostics | null>(null);
  const [errorKey, setErrorKey] = useState<ErrorKey | null>(null);
  const [feedback, setFeedback] = useState(false);

  const preparePreview = async () => {
    setIsPreparing(true);
    setErrorKey(null);
    setFeedback(false);
    try {
      const health = await localServiceClient.readHealthForDiagnostics();
      if (
        health.status !== 'ready' ||
        typeof health.schema_version !== 'number' ||
        !Number.isSafeInteger(health.schema_version) ||
        health.schema_version < 1 ||
        health.schema_version > 100
      ) {
        setErrorKey('supportDiagnosticsInvalidResponse');
        return;
      }

      const bundle: SupportDiagnostics = {
        formatVersion: 1,
        application: 'OpenChat',
        generatedAtUtc: new Date().toISOString(),
        runtime: {
          platform: 'windows',
          appVersion: await getVersion(),
          runtime: 'Tauri',
        },
        storage: {
          historyStatus: historyStorageStatusForInitialization(serviceStatus, storageInitializationErrorCode),
          serviceStatus: 'ready',
          databaseSchemaVersion: health.schema_version,
        },
      };
      setPreviewBundle(bundle);
      setPreview(JSON.stringify(bundle, null, 2));
    } catch (error) {
      setErrorKey(error instanceof LocalServiceError && error.code === 'invalid_service_response'
        ? 'supportDiagnosticsInvalidResponse'
        : 'supportDiagnosticsLoadFailed');
    } finally {
      setIsPreparing(false);
    }
  };

  const exportPreview = async () => {
    if (preview === null || isExporting) return;
    const bundle = previewBundle;
    setPreview(null);
    setPreviewBundle(null);
    setIsExporting(true);
    setErrorKey(null);
    try {
      let path: string | null;
      try {
        path = await save({
          defaultPath: 'openchat-support-diagnostics.json',
          filters: [{ name: 'JSON', extensions: ['json'] }],
          title: t('supportDiagnosticsPreviewTitle'),
        });
      } catch {
        setErrorKey('supportDiagnosticsExportFailed');
        return;
      }
      if (path === null) return;
      if (!bundle) {
        setErrorKey('supportDiagnosticsInvalidResponse');
        return;
      }
      await invoke('export_support_diagnostics', { path, diagnostics: bundle });
      setFeedback(true);
    } catch {
      setErrorKey('supportDiagnosticsExportFailed');
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <section className="grid gap-4">
      <div className="grid gap-3 min-[680px]:grid-cols-[minmax(0,1fr)_220px] min-[680px]:items-start">
        <div>
          <h3 className="font-semibold">{t('supportDiagnosticsTitle')}</h3>
          <p className="mt-1 text-sm text-secondary-text">{t('supportDiagnosticsDescription')}</p>
        </div>
        <Button className="w-full" disabled={isPreparing || isExporting} onClick={() => void preparePreview()}>
          {isPreparing || isExporting
            ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            : <FileDown aria-hidden="true" size={16} />}
          {t(isPreparing ? 'supportDiagnosticsPreparing' : isExporting ? 'saving' : 'reviewSupportDiagnostics')}
        </Button>
      </div>
      {errorKey ? (
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-sm text-destructive" role="alert">{t(errorKey)}</p>
          {errorKey !== 'supportDiagnosticsExportFailed' ? (
            <Button disabled={isPreparing || isExporting} onClick={() => void preparePreview()} size="small" variant="secondary">
              <RefreshCw aria-hidden="true" size={15} />
              {t('retry')}
            </Button>
          ) : null}
        </div>
      ) : null}
      <p className="text-xs text-secondary-text">{t('supportDiagnosticsPrivacyNote')}</p>
      {feedback ? <p aria-live="polite" className="text-sm text-success">{t('supportDiagnosticsSaved')}</p> : null}

      <Dialog onOpenChange={(isOpen) => { if (!isOpen) { setPreview(null); setPreviewBundle(null); } }} open={preview !== null} title={t('supportDiagnosticsPreviewTitle')}>
        {preview !== null ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">{t('supportDiagnosticsPreviewDescription')}</p>
            <pre className="max-h-[360px] select-text overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-3 font-mono text-xs text-text">{preview}</pre>
            <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
              <Button disabled={isExporting} onClick={() => { setPreview(null); setPreviewBundle(null); }} variant="ghost">{t('cancel')}</Button>
              <Button disabled={isExporting} onClick={() => void exportPreview()} variant="primary">
                {isExporting ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : <Download aria-hidden="true" size={16} />}
                {t(isExporting ? 'saving' : 'supportDiagnosticsExportAction')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
    </section>
  );
}

