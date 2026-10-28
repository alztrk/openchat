import { Download, FileDown, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '@/design-system';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { historyStorageStatusForInitialization } from '@/modules/settings/data/historyStorageStatus';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';
import { getVersion, invoke } from '@/platform/tauri';

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
      setErrorKey(
        error instanceof LocalServiceError && error.code === 'invalid_service_response'
          ? 'supportDiagnosticsInvalidResponse'
          : 'supportDiagnosticsLoadFailed',
      );
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
      if (!bundle) {
        setErrorKey('supportDiagnosticsInvalidResponse');
        return;
      }
      const saved: unknown = await invoke('export_support_diagnostics', {
        title: t('supportDiagnosticsPreviewTitle'),
        diagnostics: bundle,
      });
      if (typeof saved !== 'boolean') {
        setErrorKey('supportDiagnosticsInvalidResponse');
        return;
      }
      if (!saved) return;
      setFeedback(true);
    } catch {
      setErrorKey('supportDiagnosticsExportFailed');
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <section className="grid gap-4">
      <div className="grid gap-3 diagnostics:grid-cols-diagnostics-actions diagnostics:items-start">
        <div>
          <h3 className="font-semibold">{t('supportDiagnosticsTitle')}</h3>
          <p className="mt-1 text-sm text-secondary-text">{t('supportDiagnosticsDescription')}</p>
        </div>
        <Button
          className="w-full"
          loading={isPreparing || isExporting}
          loadingContent={t(isPreparing ? 'supportDiagnosticsPreparing' : 'saving')}
          onClick={() => void preparePreview()}
        >
          <FileDown aria-hidden="true" size={16} />
          {t('reviewSupportDiagnostics')}
        </Button>
      </div>
      {errorKey ? (
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-sm text-destructive" role="alert">
            {t(errorKey)}
          </p>
          {errorKey !== 'supportDiagnosticsExportFailed' ? (
            <Button
              disabled={isPreparing || isExporting}
              onClick={() => void preparePreview()}
              size="small"
              variant="secondary"
            >
              <RefreshCw aria-hidden="true" size={15} />
              {t('retry')}
            </Button>
          ) : null}
        </div>
      ) : null}
      <p className="text-xs text-secondary-text">{t('supportDiagnosticsPrivacyNote')}</p>
      {feedback ? (
        <p aria-live="polite" className="text-sm text-success">
          {t('supportDiagnosticsSaved')}
        </p>
      ) : null}

      <Dialog
        onOpenChange={(isOpen) => {
          if (!isOpen) {
            setPreview(null);
            setPreviewBundle(null);
          }
        }}
        open={preview !== null}
        title={t('supportDiagnosticsPreviewTitle')}
      >
        {preview !== null ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">{t('supportDiagnosticsPreviewDescription')}</p>
            <pre className="max-h-diagnostics-preview select-text overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-3 font-mono text-xs text-text">
              {preview}
            </pre>
            <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
              <Button
                disabled={isExporting}
                onClick={() => {
                  setPreview(null);
                  setPreviewBundle(null);
                }}
                variant="ghost"
              >
                {t('cancel')}
              </Button>
              <Button
                loading={isExporting}
                loadingContent={t('saving')}
                onClick={() => void exportPreview()}
                variant="primary"
              >
                <Download aria-hidden="true" size={16} />
                {t('supportDiagnosticsExportAction')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
    </section>
  );
}
