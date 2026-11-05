import { type FormEvent, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Input } from '@shared/components';

export function ArchivePassphraseDialog({
  confirming,
  errorMessage,
  errorMessageField = 'form',
  onInputChange,
  onCancel,
  onSubmit,
  open,
  title,
  working,
}: {
  confirming: boolean;
  errorMessage: string | null;
  errorMessageField?: 'passphrase' | 'form';
  onInputChange(): void;
  onCancel(): void;
  onSubmit(passphrase: string): Promise<boolean>;
  open: boolean;
  title: string;
  working: boolean;
}) {
  const { t } = useTranslation();
  const [passphrase, setPassphrase] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [validationKey, setValidationKey] = useState<
    | 'conversationArchivePassphraseTooShort'
    | 'conversationArchivePassphraseTooLong'
    | 'conversationArchivePassphraseMismatch'
    | null
  >(null);
  const passphraseInputRef = useRef<HTMLInputElement>(null);
  const confirmationInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) return;
    setPassphrase('');
    setConfirmation('');
    setValidationKey(null);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    if (validationKey === 'conversationArchivePassphraseMismatch') {
      confirmationInputRef.current?.focus();
    } else if (validationKey || (errorMessageField === 'passphrase' && errorMessage)) {
      passphraseInputRef.current?.focus();
    }
  }, [errorMessage, errorMessageField, open, validationKey]);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (Array.from(passphrase).length < 12) {
      setValidationKey('conversationArchivePassphraseTooShort');
      passphraseInputRef.current?.focus();
      return;
    }
    if (new TextEncoder().encode(passphrase).byteLength > 512 || passphrase.includes('\0')) {
      setValidationKey('conversationArchivePassphraseTooLong');
      passphraseInputRef.current?.focus();
      return;
    }
    if (confirming && confirmation !== passphrase) {
      setValidationKey('conversationArchivePassphraseMismatch');
      confirmationInputRef.current?.focus();
      return;
    }
    setValidationKey(null);
    if (await onSubmit(passphrase)) {
      setPassphrase('');
      setConfirmation('');
    }
  };

  const passphraseError =
    validationKey === 'conversationArchivePassphraseTooShort' ||
    validationKey === 'conversationArchivePassphraseTooLong'
      ? t(validationKey)
      : errorMessageField === 'passphrase'
        ? (errorMessage ?? undefined)
        : undefined;
  const confirmationError =
    validationKey === 'conversationArchivePassphraseMismatch' ? t(validationKey) : undefined;

  return (
    <Dialog
      onOpenChange={(isOpen) => {
        if (!isOpen) onCancel();
      }}
      open={open}
      title={title}
    >
      <form className="grid gap-4" onSubmit={(event) => void submit(event)}>
        <div className="grid gap-3">
          <Input
            autoComplete="new-password"
            autoFocus
            error={passphraseError}
            label={t('conversationArchivePassphrase')}
            maxLength={512}
            onChange={(event) => {
              setPassphrase(event.target.value);
              setValidationKey(null);
              onInputChange();
            }}
            ref={passphraseInputRef}
            type="password"
            value={passphrase}
          />
          {confirming ? (
            <Input
              autoComplete="new-password"
              error={confirmationError}
              label={t('conversationArchiveConfirmPassphrase')}
              maxLength={512}
              onChange={(event) => {
                setConfirmation(event.target.value);
                setValidationKey(null);
                onInputChange();
              }}
              ref={confirmationInputRef}
              type="password"
              value={confirmation}
            />
          ) : null}
        </div>
        {confirming ? (
          <p className="text-sm text-secondary-text">{t('conversationArchivePassphraseRecovery')}</p>
        ) : null}
        {errorMessage && errorMessageField === 'form' ? (
          <p className="text-sm text-destructive" role="alert">
            {errorMessage}
          </p>
        ) : null}
        <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
          <Button disabled={working} onClick={onCancel} type="button">
            {t('cancel')}
          </Button>
          <Button disabled={working} type="submit">
            {t('continueLabel')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
