import { useTranslation } from 'react-i18next';

import { Button, Dialog } from '@/design-system';

export interface TemporaryChatExitDialogProps {
  open: boolean;
  isBusy: boolean;
  canSave: boolean;
  errorMessage: string | null;
  onOpenChange(open: boolean): void;
  onSave(): void;
  onDiscard(): void;
}

export function TemporaryChatExitDialog({
  canSave,
  errorMessage,
  isBusy,
  onDiscard,
  onOpenChange,
  onSave,
  open,
}: TemporaryChatExitDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog onOpenChange={onOpenChange} open={open} title={t('temporaryChatExitTitle')}>
      <div className="grid gap-5">
        <p className="text-sm text-secondary-text">{t('temporaryChatExitBody')}</p>
        {errorMessage ? (
          <p className="text-sm text-destructive" role="alert">
            {errorMessage}
          </p>
        ) : null}
        <div className="flex flex-wrap justify-end gap-2">
          <Button disabled={isBusy} onClick={() => onOpenChange(false)}>
            {t('temporaryChatCancel')}
          </Button>
          <Button disabled={isBusy} onClick={onDiscard} variant="destructive">
            {t('temporaryChatDiscard')}
          </Button>
          <Button disabled={isBusy || !canSave} onClick={onSave} variant="primary">
            {t('temporaryChatSave')}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
