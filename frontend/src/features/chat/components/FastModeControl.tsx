import { Zap } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/shared/ui';

interface FastModeControlProps {
  available: boolean;
  enabled: boolean;
  loading: boolean;
  saving: boolean;
  onToggle(): void;
}

export function FastModeControl({ available, enabled, loading, saving, onToggle }: FastModeControlProps) {
  const { t } = useTranslation();
  const tooltip = loading
    ? t('chatGptFastModeLoadingTooltip')
    : !available
      ? t('chatGptFastModeUnavailableTooltip')
      : enabled
        ? t('chatGptFastModeEnabledTooltip')
        : t('chatGptFastModeDisabledTooltip');
  return (
    <Button
      aria-label={tooltip}
      aria-pressed={enabled && available}
      className={enabled && available ? 'size-10 bg-accent/15 text-accent-icon' : 'size-10'}
      disabled={!available || loading || saving}
      onClick={onToggle}
      size="icon"
      title={tooltip}
      type="button"
      variant="ghost"
    >
      {saving ? <span aria-hidden="true" className="size-4 animate-pulse rounded-full bg-current motion-reduce:animate-none" /> : <Zap aria-hidden="true" size={18} />}
    </Button>
  );
}
