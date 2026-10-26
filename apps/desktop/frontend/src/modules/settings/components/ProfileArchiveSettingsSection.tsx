import { CircleCheckBig, History, LoaderCircle, LockKeyhole, Shield, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Dialog } from '@/design-system';
import {
  buildArchiveFileName,
  exportProfileArchive,
  type ProfileArchiveResult,
  prepareProfileRestore,
  savePendingArchiveExport,
} from '@/modules/settings/data/localDataOperations';
import { LocalServiceError } from '@/platform/local-service';
import { getCurrentWindow, open } from '@/platform/tauri';
import { ArchivePassphraseDialog } from './ArchivePassphraseDialog';

type ErrorKey =
  | 'profileArchiveUnavailable'
  | 'profileArchivePickerFailed'
  | 'profileArchiveInvalidFile'
  | 'profileArchiveInvalidResponse'
  | 'profileArchiveExportFailed'
  | 'profileArchiveRestoreFailed'
  | 'profileArchiveInvalidArchive'
  | 'profileArchivePassphraseInvalid'
  | 'profileArchiveNotFound'
  | 'profileArchiveConflict'
  | 'profileArchiveBusy'
  | 'profileArchiveStorageFailed'
  | 'profileArchiveLimitExceeded'
  | 'profileArchiveSchemaUnsupported'
  | 'profileArchiveTakingLong'
  | 'profileArchiveOperationFailed'
  | 'profileArchiveCloseFailed';

export function ProfileArchiveSettingsSection({ enabled }: { enabled: boolean }) {
  const { t } = useTranslation();
  const [isWorking, setIsWorking] = useState(false);
  const [restorePath, setRestorePath] = useState<string | null>(null);
  const [restorePassphrase, setRestorePassphrase] = useState<string | null>(null);
  const [passphraseTarget, setPassphraseTarget] = useState<'export' | 'restore' | null>(null);
  const [restoreReady, setRestoreReady] = useState<ProfileArchiveResult | null>(null);
  const [errorKey, setErrorKey] = useState<ErrorKey | null>(null);
  const [feedback, setFeedback] = useState<ProfileArchiveResult | null>(null);

  const reportError = (error: unknown, action: 'export' | 'restore') => {
    if (error instanceof LocalServiceError) {
      setErrorKey(
        profileArchiveErrorKey(error.code) ??
          (error.code === 'invalid_service_response'
            ? 'profileArchiveInvalidResponse'
            : action === 'export'
              ? 'profileArchiveExportFailed'
              : 'profileArchiveRestoreFailed'),
      );
    } else {
      setErrorKey(action === 'export' ? 'profileArchiveExportFailed' : 'profileArchiveRestoreFailed');
    }
  };

  const startExport = () => {
    setErrorKey(null);
    setFeedback(null);
    setPassphraseTarget('export');
  };

  const startRestore = async () => {
    setIsWorking(true);
    setErrorKey(null);
    setFeedback(null);
    try {
      let selected: string | string[] | null;
      try {
        selected = await open({
          multiple: false,
          directory: false,
          filters: [{ name: 'OpenChat profile backup', extensions: ['openchatprofilebackup'] }],
          title: t('profileArchiveChooseFile'),
        });
      } catch {
        setErrorKey('profileArchivePickerFailed');
        return;
      }
      if (selected === null) return;
      if (typeof selected !== 'string' || !selected.trim()) {
        setErrorKey('profileArchiveInvalidFile');
        return;
      }
      setRestorePath(selected);
      setPassphraseTarget('restore');
    } finally {
      setIsWorking(false);
    }
  };

  const submitPassphrase = async (passphrase: string): Promise<boolean> => {
    setErrorKey(null);
    setIsWorking(true);
    try {
      if (passphraseTarget === 'export') {
        const result = await exportProfileArchive(passphrase);
        const saved = await savePendingArchiveExport(
          'profile',
          result.exportId,
          buildArchiveFileName('openchatprofilebackup'),
          t('profileArchiveSaveTitle'),
        );
        setPassphraseTarget(null);
        if (!saved) return true;
        setFeedback(result);
        return true;
      }
      if (passphraseTarget === 'restore' && restorePath) {
        setRestorePassphrase(passphrase);
        setPassphraseTarget(null);
        return true;
      }
      return false;
    } catch (error) {
      reportError(error, passphraseTarget ?? 'restore');
      return false;
    } finally {
      setIsWorking(false);
    }
  };

  const confirmRestore = async () => {
    const path = restorePath;
    const passphrase = restorePassphrase;
    if (!path || passphrase === null || isWorking) return;
    setIsWorking(true);
    setErrorKey(null);
    try {
      const result = await prepareProfileRestore(path, passphrase);
      if (!result.requiresRestart) {
        setErrorKey('profileArchiveInvalidResponse');
        return;
      }
      setRestoreReady(result);
      setRestorePath(null);
      setRestorePassphrase(null);
    } catch (error) {
      reportError(error, 'restore');
    } finally {
      setIsWorking(false);
    }
  };

  const closeForRestore = async () => {
    try {
      await getCurrentWindow().close();
    } catch {
      setErrorKey('profileArchiveCloseFailed');
    }
  };

  return (
    <Card className="grid gap-4 bg-composer sm:p-5" padding="default">
      <div className="flex items-start gap-3">
        <Shield aria-hidden="true" className="mt-0.5 shrink-0 text-accent" size={19} />
        <div className="grid gap-1">
          <h3 className="font-semibold">{t('profileArchiveTitle')}</h3>
          <p className="text-sm text-secondary-text">{t('profileArchiveDescription')}</p>
          <p className="text-sm text-secondary-text">{t('profileArchiveIncludesNotice')}</p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button disabled={!enabled || isWorking} onClick={() => void startExport()}>
          <LockKeyhole aria-hidden="true" size={16} />
          {t('profileArchiveExport')}
        </Button>
        <Button disabled={!enabled || isWorking} onClick={() => void startRestore()}>
          <History aria-hidden="true" size={16} />
          {t('profileArchiveRestore')}
        </Button>
      </div>

      {!enabled ? (
        <p className="text-sm text-warning" role="status">
          {t('profileArchiveUnavailable')}
        </p>
      ) : null}
      {isWorking ? (
        <>
          <div
            aria-label={t('profileArchiveProcessing')}
            className="h-1 overflow-hidden rounded-full bg-selected"
            role="progressbar"
          >
            <div className="h-full w-1/3 animate-pulse rounded-full bg-accent" />
          </div>
          <p aria-live="polite" className="text-sm text-secondary-text">
            {t('profileArchiveProcessing')}
          </p>
        </>
      ) : null}
      {errorKey ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}
      {feedback ? (
        <p aria-live="polite" className="text-sm text-success">
          {t('profileArchiveExportSuccess', {
            conversationCount: new Intl.NumberFormat().format(feedback.conversationCount),
            messageCount: new Intl.NumberFormat().format(feedback.messageCount),
            attachmentCount: new Intl.NumberFormat().format(feedback.attachmentCount),
          })}
        </p>
      ) : null}

      <ArchivePassphraseDialog
        confirming={passphraseTarget === 'export'}
        errorMessage={errorKey && passphraseTarget ? t(errorKey) : null}
        onCancel={() => {
          setPassphraseTarget(null);
          setRestorePath(null);
          setRestorePassphrase(null);
          setErrorKey(null);
        }}
        onSubmit={submitPassphrase}
        open={passphraseTarget !== null}
        title={t('conversationArchivePassphraseTitle')}
        working={isWorking}
      />

      <Dialog
        onOpenChange={() => undefined}
        open={restorePath !== null && restorePassphrase !== null}
        title={t('profileArchiveRestoreConfirmTitle')}
      >
        <div className="grid gap-5">
          <div className="flex items-start gap-3">
            <TriangleAlert aria-hidden="true" className="mt-0.5 shrink-0 text-destructive" size={20} />
            <p className="text-sm text-secondary-text">{t('profileArchiveRestoreConfirmBody')}</p>
          </div>
          <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
            <Button
              disabled={isWorking}
              onClick={() => {
                setRestorePath(null);
                setRestorePassphrase(null);
              }}
              type="button"
            >
              {t('cancel')}
            </Button>
            <Button disabled={isWorking} onClick={() => void confirmRestore()} variant="destructive">
              {isWorking ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <History aria-hidden="true" size={16} />
              )}
              {t('profileArchiveRestoreConfirmButton')}
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog onOpenChange={() => undefined} open={restoreReady !== null} title={t('profileArchiveRestartTitle')}>
        {restoreReady ? (
          <div className="grid gap-5">
            <div className="flex items-start gap-3">
              <CircleCheckBig aria-hidden="true" className="mt-0.5 shrink-0 text-success" size={20} />
              <p className="text-sm text-secondary-text">
                {t('profileArchiveRestoreReady', {
                  conversationCount: new Intl.NumberFormat().format(restoreReady.conversationCount),
                  messageCount: new Intl.NumberFormat().format(restoreReady.messageCount),
                  attachmentCount: new Intl.NumberFormat().format(restoreReady.attachmentCount),
                })}
              </p>
            </div>
            <Button className="justify-self-end" onClick={() => void closeForRestore()} variant="destructive">
              {t('profileArchiveCloseApp')}
            </Button>
            {errorKey === 'profileArchiveCloseFailed' ? (
              <p className="text-sm text-destructive" role="alert">
                {t(errorKey)}
              </p>
            ) : null}
          </div>
        ) : null}
      </Dialog>
    </Card>
  );
}

function profileArchiveErrorKey(code: string): ErrorKey | null {
  switch (code) {
    case 'archive_export_busy':
      return 'profileArchiveBusy';
    case 'archive_export_conflict':
      return 'profileArchiveConflict';
    case 'archive_export_invalid':
      return 'profileArchiveInvalidResponse';
    case 'archive_export_storage_failed':
    case 'archive_export_unavailable':
      return 'profileArchiveStorageFailed';
    case 'profile_archive_invalid':
      return 'profileArchiveInvalidArchive';
    case 'profile_archive_passphrase_invalid':
      return 'profileArchivePassphraseInvalid';
    case 'profile_archive_not_found':
      return 'profileArchiveNotFound';
    case 'profile_archive_conflict':
      return 'profileArchiveConflict';
    case 'profile_archive_busy':
      return 'profileArchiveBusy';
    case 'profile_archive_storage_failed':
      return 'profileArchiveStorageFailed';
    case 'profile_archive_limit_exceeded':
      return 'profileArchiveLimitExceeded';
    case 'profile_archive_schema_unsupported':
      return 'profileArchiveSchemaUnsupported';
    case 'service_timeout':
      return 'profileArchiveTakingLong';
    default:
      return null;
  }
}
