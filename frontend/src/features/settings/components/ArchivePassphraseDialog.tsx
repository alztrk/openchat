import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Input } from '@/shared/ui';

export function ArchivePassphraseDialog({
  confirming,
  errorMessage,
  onCancel,
  onSubmit,
  open,
  title,
  working,
}: {
  confirming: boolean;
  errorMessage: string | null;
  onCancel(): void;
  onSubmit(passphrase: string): Promise<boolean>;
  open: boolean;
  title: string;
  working: boolean;
}) {
  const { t } = useTranslation();
  const [passphrase, setPassphrase] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [validationKey, setValidationKey] = useState<'conversationArchivePassphraseTooShort' | 'conversationArchivePassphraseTooLong' | 'conversationArchivePassphraseMismatch' | null>(null);

  useEffect(() => {
    if (open) return;
    setPassphrase('');
    setConfirmation('');
    setValidationKey(null);
  }, [open]);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (Array.from(passphrase).length < 12) {
      setValidationKey('conversationArchivePassphraseTooShort');
      return;
    }
    if (new TextEncoder().encode(passphrase).byteLength > 512 || passphrase.includes('\0')) {
      setValidationKey('conversationArchivePassphraseTooLong');
      return;
    }
    if (confirming && confirmation !== passphrase) {
      setValidationKey('conversationArchivePassphraseMismatch');
      return;
    }
    setValidationKey(null);
    if (await onSubmit(passphrase)) {
      setPassphrase('');
      setConfirmation('');
    }
  };

  return (
    <Dialog onOpenChange={(isOpen) => { if (!isOpen) onCancel(); }} open={open} title={title}>
      <form className="grid gap-4" onSubmit={(event) => void submit(event)}>
        <div className="grid gap-3">
          <Input autoComplete="new-password" autoFocus label={t('conversationArchivePassphrase')} maxLength={512} onChange={(event) => { setPassphrase(event.target.value); setValidationKey(null); }} type="password" value={passphrase} />
          {confirming ? <Input autoComplete="new-password" label={t('conversationArchiveConfirmPassphrase')} maxLength={512} onChange={(event) => { setConfirmation(event.target.value); setValidationKey(null); }} type="password" value={confirmation} /> : null}
        </div>
        {confirming ? <p className="text-sm text-secondary-text">{t('conversationArchivePassphraseRecovery')}</p> : null}
        {validationKey || errorMessage ? <p className="text-sm text-destructive" role="alert">{validationKey ? t(validationKey) : errorMessage}</p> : null}
        <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
          <Button disabled={working} onClick={onCancel} type="button">{t('cancel')}</Button>
          <Button disabled={working} type="submit">{t('continueLabel')}</Button>
        </div>
      </form>
    </Dialog>
  );
}
