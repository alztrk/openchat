import { useTranslation } from 'react-i18next';

import { Button, Card } from '@/design-system';

export type SettingsSaveState = 'idle' | 'saving' | 'saved' | 'error';

export function SettingsSaveFeedback({ errorMessage, state }: { errorMessage?: string; state: SettingsSaveState }) {
  const { t } = useTranslation();
  if (state === 'idle') return null;

  if (state === 'error') {
    return (
      <p className="text-sm text-destructive" role="alert">
        {errorMessage ?? t('settingsSaveFailed')}
      </p>
    );
  }

  return (
    <p aria-live="polite" className="text-sm text-secondary-text" role="status">
      {t(state === 'saving' ? 'settingsSaving' : 'settingsSaved')}
    </p>
  );
}

export function SettingsLoadError({
  message,
  onReset,
  onRetry,
  resetLabel,
  resetLoading = false,
}: {
  message: string;
  onReset?: () => void;
  onRetry: () => void;
  resetLabel?: string;
  resetLoading?: boolean;
}) {
  const { t } = useTranslation();

  return (
    <Card className="grid gap-3" padding="default" role="alert">
      <p className="text-sm text-destructive">{message}</p>
      <div className="flex flex-wrap gap-2">
        <Button onClick={onRetry} size="small" variant="secondary">
          {t('retry')}
        </Button>
        {onReset && resetLabel ? (
          <Button loading={resetLoading} onClick={onReset} size="small" variant="ghost">
            {resetLabel}
          </Button>
        ) : null}
      </div>
    </Card>
  );
}
