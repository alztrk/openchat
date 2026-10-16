import { useEffect, useState, type FormEvent } from 'react';
import { LoaderCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { CustomProviderSummary } from '@/features/settings/data/providerCredentials';
import { Button, Dialog, Input, Select } from '@/shared/ui';

export interface CustomProviderDraft {
  id: `custom_${string}`;
  name: string;
  baseUrl: string;
  protocol: 'openAi' | 'anthropic';
  apiKey: string;
}

interface CustomProviderDialogProps {
  open: boolean;
  provider: CustomProviderSummary | null;
  saving: boolean;
  externalError: string | null;
  onOpenChange(open: boolean): void;
  onSave(draft: CustomProviderDraft): void;
}

export function CustomProviderDialog({
  open,
  provider,
  saving,
  externalError,
  onOpenChange,
  onSave,
}: CustomProviderDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [protocol, setProtocol] = useState<'openAi' | 'anthropic'>('openAi');
  const [apiKey, setApiKey] = useState('');
  const [errorKey, setErrorKey] = useState<'customProviderInvalid' | 'customProviderUrlInvalid' | 'apiKeyInvalidFormat' | null>(null);

  useEffect(() => {
    if (!open) return;
    setName(provider?.name ?? '');
    setBaseUrl(provider?.baseUrl ?? '');
    setProtocol(provider?.protocol ?? 'openAi');
    setApiKey('');
    setErrorKey(null);
  }, [open, provider]);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const normalizedName = name.trim();
    const normalizedUrl = baseUrl.trim();
    if (normalizedName.length === 0 || normalizedName.length > 80 || /[\x00-\x1F\x7F]/.test(normalizedName)) {
      setErrorKey('customProviderInvalid');
      return;
    }
    if (!isValidBaseUrl(normalizedUrl)) {
      setErrorKey('customProviderUrlInvalid');
      return;
    }
    const normalizedApiKey = apiKey.trim();
    if (normalizedApiKey.length > 4096 || (normalizedApiKey.length > 0 && !/^[\x21-\x7E]+$/.test(normalizedApiKey))) {
      setErrorKey('apiKeyInvalidFormat');
      return;
    }
    onSave({
      id: provider?.id ?? createCustomProviderId(),
      name: normalizedName,
      baseUrl: normalizedUrl,
      protocol,
      apiKey: normalizedApiKey,
    });
  };

  return (
    <Dialog
      onOpenChange={(nextOpen) => { if (!saving) onOpenChange(nextOpen); }}
      open={open}
      title={t(provider ? 'editCustomProvider' : 'addCustomProvider')}
    >
      <form className="grid gap-4" onSubmit={submit}>
        <Input
          autoComplete="organization"
          disabled={saving}
          label={t('customProviderName')}
          maxLength={80}
          onChange={(event) => { setName(event.target.value); setErrorKey(null); }}
          value={name}
        />
        <Select
          disabled={saving}
          label={t('customProviderProtocol')}
          onChange={(event) => { setProtocol(event.target.value === 'anthropic' ? 'anthropic' : 'openAi'); setErrorKey(null); }}
          options={[
            { value: 'openAi', label: t('openAiCompatible') },
            { value: 'anthropic', label: t('anthropicCompatible') },
          ]}
          value={protocol}
        />
        <Input
          autoCapitalize="none"
          autoComplete="url"
          autoCorrect="off"
          disabled={saving}
          label={t('customProviderBaseUrl')}
          maxLength={2048}
          onChange={(event) => { setBaseUrl(event.target.value); setErrorKey(null); }}
          type="url"
          value={baseUrl}
        />
        <Input
          autoComplete="new-password"
          autoCorrect="off"
          disabled={saving}
          label={t('customProviderApiKey')}
          maxLength={4096}
          onChange={(event) => { setApiKey(event.target.value); setErrorKey(null); }}
          placeholder={t(provider ? 'customProviderKeyUnchanged' : 'customProviderApiKeyOptional')}
          type="password"
          value={apiKey}
        />
        {errorKey ? <p className="text-sm text-destructive" role="alert">{t(errorKey)}</p> : externalError ? <p className="text-sm text-destructive" role="alert">{externalError}</p> : null}
        <div className="flex justify-end gap-2">
          <Button disabled={saving} onClick={() => onOpenChange(false)}>{t('cancel')}</Button>
          <Button disabled={saving} type="submit" variant="primary">
            {saving ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : null}
            {t('save')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

function isValidBaseUrl(value: string): boolean {
  if (value.length === 0 || value.length > 2048) return false;
  try {
    const url = new URL(value);
    return (url.protocol === 'http:' || url.protocol === 'https:') &&
      url.hostname.length > 0 &&
      url.username.length === 0 &&
      url.password.length === 0 &&
      url.search.length === 0 &&
      url.hash.length === 0;
  } catch {
    return false;
  }
}

function createCustomProviderId(): `custom_${string}` {
  return `custom_${crypto.randomUUID().replaceAll('-', '')}`;
}
