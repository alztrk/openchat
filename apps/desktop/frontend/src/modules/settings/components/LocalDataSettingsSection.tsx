import { LoaderCircle, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog } from '@/design-system';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { historyStorageStatusForInitialization } from '@/modules/settings/data/historyStorageStatus';
import { ConversationArchiveSettingsSection } from './ConversationArchiveSettingsSection';
import { ProfileArchiveSettingsSection } from './ProfileArchiveSettingsSection';
import { SupportDiagnosticsSettingsSection } from './SupportDiagnosticsSettingsSection';

export function LocalDataSettingsSection() {
  const { t } = useTranslation();
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const storageInitializationErrorCode = useChatStore((state) => state.storageInitializationErrorCode);
  const conversations = useChatStore((state) => state.conversations);
  const isSending = useChatStore((state) => state.isSending);
  const [isConfirmingClear, setIsConfirmingClear] = useState(false);
  const [isClearing, setIsClearing] = useState(false);
  const [clearFailed, setClearFailed] = useState(false);
  const [clearSucceeded, setClearSucceeded] = useState(false);
  const historyStatus = historyStorageStatusForInitialization(serviceStatus, storageInitializationErrorCode);
  const available = historyStatus === 'available';
  const statusKey =
    historyStatus === 'available'
      ? 'historyDeviceStatus'
      : historyStatus === 'loading'
        ? 'historyCheckingStatus'
        : 'historyStorageUnavailableStatus';
  const descriptionKey =
    historyStatus === 'available'
      ? 'historyDeviceDescription'
      : historyStatus === 'loading'
        ? 'historyCheckingDescription'
        : historyStatus === 'corrupt'
          ? 'historyStorageCorruptDescription'
          : historyStatus === 'backupUnavailable'
            ? 'historyStorageBackupFailedDescription'
            : 'historyStorageUnavailableDescription';

  const clearHistory = async () => {
    if (!available || isSending || isClearing) return;
    setIsClearing(true);
    setClearFailed(false);
    setClearSucceeded(false);
    try {
      await useChatStore.getState().clearConversationHistory();
      setIsConfirmingClear(false);
      setClearSucceeded(true);
    } catch {
      setClearFailed(true);
    } finally {
      setIsClearing(false);
    }
  };

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      <div className="@container">
        <div className="appearance-setting-row">
          <div>
            <h3 className="font-medium">{t('conversationHistory')}</h3>
            <p className="mt-1 text-sm text-secondary-text">{t(descriptionKey)}</p>
          </div>
          <span
            className={`mt-1 w-36 justify-self-start rounded-control border px-2 py-1 text-center text-xs ${available ? 'border-success text-success' : 'border-border text-secondary-text'}`}
          >
            {t(statusKey)}
          </span>
        </div>
      </div>

      <ConversationArchiveSettingsSection enabled={available} />
      <ProfileArchiveSettingsSection enabled={available} />
      <SupportDiagnosticsSettingsSection />

      <div className="@container">
        <div className="appearance-setting-row">
          <div>
            <h3 className="font-medium">{t('clearConversationHistory')}</h3>
            <p className="mt-1 text-sm text-secondary-text">{t('clearConversationHistoryDescription')}</p>
          </div>
          <div className="grid justify-items-start gap-2">
            <Button
              className="w-full max-w-44 border-destructive text-destructive hover:bg-destructive/10"
              disabled={!available || conversations.length === 0 || isSending || isClearing}
              onClick={() => {
                setClearFailed(false);
                setClearSucceeded(false);
                setIsConfirmingClear(true);
              }}
              variant="secondary"
            >
              {isClearing ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <Trash2 aria-hidden="true" size={16} />
              )}
              {t(isClearing ? 'clearingHistory' : 'deleteAll')}
            </Button>
            {isSending ? <p className="text-sm text-secondary-text">{t('conversationArchiveBusy')}</p> : null}
            {clearFailed ? (
              <p className="text-sm text-destructive" role="alert">
                {t('clearHistoryFailed')}
              </p>
            ) : null}
            {clearSucceeded ? (
              <p aria-live="polite" className="text-sm text-success">
                {t('clearHistorySucceeded')}
              </p>
            ) : null}
          </div>
        </div>
      </div>

      <Dialog onOpenChange={setIsConfirmingClear} open={isConfirmingClear} title={t('confirmClearHistoryTitle')}>
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">{t('confirmClearHistoryBody')}</p>
          <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
            <Button disabled={isClearing} onClick={() => setIsConfirmingClear(false)}>
              {t('cancel')}
            </Button>
            <Button disabled={isClearing || isSending} onClick={() => void clearHistory()} variant="destructive">
              {isClearing ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <Trash2 aria-hidden="true" size={16} />
              )}
              {t(isClearing ? 'clearingHistory' : 'deleteAll')}
            </Button>
          </div>
        </div>
      </Dialog>
    </section>
  );
}
