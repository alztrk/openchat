import { BellOff, BellRing, LoaderCircle, Play, RefreshCw, Send } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Switch } from '@/design-system';
import type en from '@/localization/locales/en.json';
import {
  type NotificationPreferences,
  openWindowsNotificationSettings,
  readNotificationPreferences,
  readWindowsNotificationStatus,
  sendTestNotification,
  type WindowsNotificationStatus,
  writeNotificationPreference,
} from '@/modules/notifications/data/notificationPreferences';
import { playTestChime } from '@/shared/utils/audioFeedback';

type LoadState = 'loading' | 'loaded' | 'failed';
type TranslationKey = keyof typeof en;

export function NotificationSettingsSection() {
  const { t } = useTranslation();
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [preferences, setPreferences] = useState<NotificationPreferences>({
    responses: true,
    questions: true,
    responsePreview: true,
    quickReply: true,
    soundEnabled: true,
    soundMuted: false,
    soundVolume: 0.7,
    failureSoundEnabled: true,
  });
  const [isSaving, setIsSaving] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const [status, setStatus] = useState<WindowsNotificationStatus | null>(null);
  const [isLoadingStatus, setIsLoadingStatus] = useState(true);
  const [statusFailed, setStatusFailed] = useState(false);
  const [isOpeningSettings, setIsOpeningSettings] = useState(false);
  const [settingsOpenFailed, setSettingsOpenFailed] = useState(false);
  const [isSendingTest, setIsSendingTest] = useState(false);
  const [testResult, setTestResult] = useState<'requested' | 'failed' | null>(null);
  const [isTestingChime, setIsTestingChime] = useState(false);
  const statusGeneration = useRef(0);

  const loadPreferences = useCallback(() => {
    setLoadState('loading');
    try {
      setPreferences(readNotificationPreferences());
      setLoadState('loaded');
    } catch {
      setLoadState('failed');
    }
  }, []);

  const loadDeliveryStatus = useCallback(async () => {
    const generation = ++statusGeneration.current;
    setIsLoadingStatus(true);
    setStatusFailed(false);
    try {
      const nextStatus = await readWindowsNotificationStatus();
      if (generation !== statusGeneration.current) return;
      setStatus(nextStatus);
      setStatusFailed(nextStatus === 'unavailable');
    } catch {
      if (generation !== statusGeneration.current) return;
      setStatus('unavailable');
      setStatusFailed(true);
    } finally {
      if (generation === statusGeneration.current) setIsLoadingStatus(false);
    }
  }, []);

  useEffect(() => {
    loadPreferences();
  }, [loadPreferences]);
  useEffect(() => {
    void loadDeliveryStatus();
    const onWindowFocus = () => {
      void loadDeliveryStatus();
    };
    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') void loadDeliveryStatus();
    };
    window.addEventListener('focus', onWindowFocus);
    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      statusGeneration.current += 1;
      window.removeEventListener('focus', onWindowFocus);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, [loadDeliveryStatus]);

  const save = <K extends keyof NotificationPreferences>(key: K, value: NotificationPreferences[K]) => {
    if (isSaving || loadState !== 'loaded') return;
    const updated = { ...preferences, [key]: value };
    setIsSaving(true);
    setSaveFailed(false);
    try {
      writeNotificationPreference(key, value);
      setPreferences(updated);
    } catch {
      setSaveFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  const openSettings = async () => {
    if (isOpeningSettings) return;
    setIsOpeningSettings(true);
    setSettingsOpenFailed(false);
    try {
      await openWindowsNotificationSettings();
    } catch {
      setSettingsOpenFailed(true);
    } finally {
      setIsOpeningSettings(false);
    }
  };

  const sendTest = async () => {
    if (isSendingTest) return;
    setIsSendingTest(true);
    setTestResult(null);
    try {
      const requested = await sendTestNotification(t('notificationTestTitle'), t('notificationTestBody'));
      setTestResult(requested ? 'requested' : 'failed');
    } catch {
      setTestResult('failed');
    } finally {
      setIsSendingTest(false);
    }
  };

  const testChime = async () => {
    if (isTestingChime) return;
    setIsTestingChime(true);
    try {
      await playTestChime(preferences.soundVolume);
    } finally {
      setIsTestingChime(false);
    }
  };

  const statusText = status === null ? t('notificationDeliveryUnavailable') : t(statusLabelKey(status));
  const isBlocked = status !== null && status !== 'enabled' && status !== 'notSupported' && status !== 'unavailable';

  return (
    <section aria-labelledby="settings-page-heading" className="grid content-start gap-5">
      {loadState === 'loading' ? (
        <div aria-live="polite" className="flex min-h-24 items-center justify-center">
          <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-icon" size={22} />
        </div>
      ) : loadState === 'failed' ? (
        <div className="grid justify-items-start gap-3">
          <p className="text-sm text-destructive" role="alert">
            {t('notificationSettingsUnavailable')}
          </p>
          <Button onClick={loadPreferences} size="small">
            <RefreshCw aria-hidden="true" size={15} />
            {t('retry')}
          </Button>
        </div>
      ) : (
        <div className="grid gap-1">
          <div aria-live="polite" className="flex items-start gap-3 rounded-control bg-composer p-3">
            {status === 'enabled' ? (
              <BellRing aria-hidden="true" className="mt-0.5 shrink-0 text-success" size={18} />
            ) : (
              <BellOff aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={18} />
            )}
            <div className="grid min-w-0 gap-1">
              <p className="text-sm">{isLoadingStatus ? t('notificationDeliveryChecking') : statusText}</p>
              {isBlocked || statusFailed ? (
                <div className="flex flex-wrap gap-2">
                  <Button
                    disabled={isOpeningSettings || isLoadingStatus}
                    onClick={() => void openSettings()}
                    size="small"
                    variant="ghost"
                  >
                    {t('openNotificationSettings')}
                  </Button>
                  {statusFailed ? (
                    <Button
                      disabled={isLoadingStatus}
                      onClick={() => void loadDeliveryStatus()}
                      size="small"
                      variant="ghost"
                    >
                      <RefreshCw aria-hidden="true" size={15} />
                      {t('retry')}
                    </Button>
                  ) : null}
                </div>
              ) : null}
              {settingsOpenFailed ? (
                <p className="text-xs text-destructive" role="alert">
                  {t('notificationSettingsUnavailable')}
                </p>
              ) : null}
              {status === 'enabled' ? (
                <div className="grid justify-items-start gap-1">
                  <Button disabled={isSendingTest} onClick={() => void sendTest()} size="small" variant="ghost">
                    {isSendingTest ? (
                      <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                    ) : (
                      <Send aria-hidden="true" size={15} />
                    )}
                    {t('sendTestNotification')}
                  </Button>
                  {testResult ? (
                    <p
                      className={`text-xs ${testResult === 'failed' ? 'text-destructive' : 'text-secondary-text'}`}
                      role={testResult === 'failed' ? 'alert' : 'status'}
                    >
                      {t(testResult === 'requested' ? 'notificationTestRequested' : 'notificationTestFailed')}
                    </p>
                  ) : null}
                </div>
              ) : null}
            </div>
          </div>

          <PreferenceRow
            description={t('notificationResponsesDescription')}
            disabled={isSaving}
            label={t('notificationResponses')}
            checked={preferences.responses}
            onCheckedChange={(value) => save('responses', value)}
          />
          <PreferenceRow
            description={t('notificationQuestionsDescription')}
            disabled={isSaving}
            label={t('notificationQuestions')}
            checked={preferences.questions}
            onCheckedChange={(value) => save('questions', value)}
          />
          <PreferenceRow
            description={t('notificationResponsePreviewDescription')}
            disabled={isSaving || !preferences.responses}
            label={t('notificationResponsePreview')}
            checked={preferences.responsePreview}
            onCheckedChange={(value) => save('responsePreview', value)}
          />
          <PreferenceRow
            description={t('notificationQuickReplyDescription')}
            disabled={isSaving || !preferences.responses}
            label={t('notificationQuickReply')}
            checked={preferences.quickReply}
            onCheckedChange={(value) => save('quickReply', value)}
          />

          <PreferenceRow
            description={t('notificationSoundEnabledDescription')}
            disabled={isSaving}
            label={t('notificationSoundEnabled')}
            checked={preferences.soundEnabled}
            onCheckedChange={(value) => save('soundEnabled', value)}
          />
          <PreferenceRow
            description={t('notificationFailureSoundEnabledDescription')}
            disabled={isSaving}
            label={t('notificationFailureSoundEnabled')}
            checked={preferences.failureSoundEnabled}
            onCheckedChange={(value) => save('failureSoundEnabled', value)}
          />
          <PreferenceRow
            description={t('notificationSoundMutedDescription')}
            disabled={isSaving}
            label={t('notificationSoundMuted')}
            checked={preferences.soundMuted}
            onCheckedChange={(value) => save('soundMuted', value)}
          />
          <VolumeRow
            description={t('notificationSoundVolumeDescription')}
            disabled={
              isSaving ||
              preferences.soundMuted ||
              (!preferences.soundEnabled && !preferences.failureSoundEnabled)
            }
            isTesting={isTestingChime}
            label={t('notificationSoundVolume')}
            onTest={() => void testChime()}
            onValueChange={(value) => save('soundVolume', value)}
            testLabel={t('notificationTestChime')}
            value={preferences.soundVolume}
          />

          {saveFailed ? (
            <p className="text-sm text-destructive" role="alert">
              {t('notificationSettingsSaveFailed')}
            </p>
          ) : null}
        </div>
      )}
    </section>
  );
}

function PreferenceRow({
  checked,
  description,
  disabled,
  label,
  onCheckedChange,
}: {
  checked: boolean;
  description: string;
  disabled: boolean;
  label: string;
  onCheckedChange(value: boolean): void;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border py-3 last:border-b-0">
      <div className="grid min-w-0 gap-1">
        <span className="font-medium">{label}</span>
        <span className="text-sm text-secondary-text">{description}</span>
      </div>
      <Switch aria-label={label} checked={checked} disabled={disabled} onCheckedChange={onCheckedChange} />
    </div>
  );
}

function VolumeRow({
  disabled,
  label,
  description,
  value,
  onValueChange,
  onTest,
  isTesting,
  testLabel,
}: {
  disabled: boolean;
  label: string;
  description: string;
  value: number;
  onValueChange(value: number): void;
  onTest(): void;
  isTesting: boolean;
  testLabel: string;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border py-3 last:border-b-0">
      <div className="grid min-w-0 gap-1">
        <span className="font-medium">{label}</span>
        <span className="text-sm text-secondary-text">{description}</span>
      </div>
      <div className="flex items-center gap-3">
        <input
          aria-label={label}
          aria-valuemax={100}
          aria-valuemin={0}
          aria-valuenow={Math.round(value * 100)}
          className="h-1.5 w-28 cursor-pointer accent-accent disabled:cursor-not-allowed disabled:opacity-50"
          disabled={disabled}
          max="1"
          min="0"
          step="0.05"
          type="range"
          value={value}
          onChange={(event) => {
            const next = Number.parseFloat(event.target.value);
            if (Number.isFinite(next)) {
              onValueChange(Math.max(0, Math.min(1, next)));
            }
          }}
        />
        <span className="min-w-10 text-right text-xs tabular-nums text-secondary-text">
          {Math.round(value * 100)}%
        </span>
        <Button disabled={disabled || isTesting} onClick={onTest} size="small" variant="ghost">
          {isTesting ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={14} />
          ) : (
            <Play aria-hidden="true" size={14} />
          )}
          {testLabel}
        </Button>
      </div>
    </div>
  );
}

function statusLabelKey(status: WindowsNotificationStatus): TranslationKey {
  switch (status) {
    case 'enabled':
      return 'notificationDeliveryEnabled';
    case 'disabledForApplication':
      return 'notificationDeliveryDisabledForApp';
    case 'disabledForUser':
      return 'notificationDeliveryDisabledForUser';
    case 'disabledByGroupPolicy':
      return 'notificationDeliveryDisabledByPolicy';
    case 'disabledByManifest':
      return 'notificationDeliveryDisabledByManifest';
    case 'notSupported':
      return 'notificationDeliveryNotSupported';
    case 'unavailable':
      return 'notificationDeliveryUnavailable';
  }
}
