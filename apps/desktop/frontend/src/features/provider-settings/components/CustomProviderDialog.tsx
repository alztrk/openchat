/** @file Collects custom-provider settings and validates them before native storage. */
import { LoaderCircle } from 'lucide-react';
import { type FormEvent, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog, Input, Select, Switch } from '@shared/components';
import type { CustomProviderSummary } from '@features/provider-settings/data';
import { containsAsciiControlCharacters } from '@shared/lib/utils/textValidation';

/** Values submitted by the custom-provider editor for native validation and storage. */
export interface CustomProviderDraft {
  id: `custom_${string}`;
  name: string;
  baseUrl: string;
  protocol: 'openAi' | 'anthropic';
  allowInsecureHttp: boolean;
  reasoningEfforts: string[];
  enableThinking: boolean;
  apiKey: string;
}

/** Controlled state and callbacks supplied by the owner of the custom-provider dialog. */
interface CustomProviderDialogProps {
  /** Whether the dialog is currently visible. */
  open: boolean;
  /** Existing provider to edit, or null when creating a provider. */
  provider: CustomProviderSummary | null;
  /** Prevents edits and closing while the owner saves the draft. */
  saving: boolean;
  /** Safe error message returned by the owner after a failed save. */
  externalError: string | null;
  /** Notifies the owner when the dialog requests an open-state change. */
  onOpenChange(open: boolean): void;
  /** Gives a locally validated draft to the owner for native persistence. */
  onSave(draft: CustomProviderDraft): void;
}

type CustomProviderValidationKey =
  | 'customProviderInvalid'
  | 'customProviderUrlInvalid'
  | 'customProviderHttpIpOrLoopbackRequired'
  | 'customProviderHttpApprovalRequired'
  | 'customProviderReasoningLevelRequired'
  | 'apiKeyInvalidFormat';

type CustomProviderValidationField = 'name' | 'baseUrl' | 'httpApproval' | 'reasoningEfforts' | 'apiKey';

/**
 * Edits or creates a custom provider with client-side validation and an explicit HTTP warning.
 *
 * The parent owns persistence and save state; this dialog keeps the entered API key in component
 * state and passes it to the parent only when the user submits a valid draft.
 */
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
  const [errorKey, setErrorKey] = useState<CustomProviderValidationKey | null>(null);
  const [allowInsecureHttp, setAllowInsecureHttp] = useState(false);
  const [reasoningEffortsText, setReasoningEffortsText] = useState('');
  const [enableThinking, setEnableThinking] = useState(false);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const reasoningEffortsInputRef = useRef<HTMLInputElement>(null);
  const baseUrlInputRef = useRef<HTMLInputElement>(null);
  const apiKeyInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) {
      setApiKey('');
      return;
    }
    setName(provider?.name ?? '');
    setBaseUrl(provider?.baseUrl ?? '');
    setProtocol(provider?.protocol ?? 'openAi');
    setAllowInsecureHttp(provider?.allowInsecureHttp ?? false);
    setReasoningEffortsText(provider?.reasoningEfforts.join(', ') ?? '');
    setEnableThinking(provider?.enableThinking ?? false);
    setApiKey('');
    setErrorKey(null);
  }, [open, provider]);

  const showValidationError = (key: CustomProviderValidationKey, field: CustomProviderValidationField) => {
    setErrorKey(key);
    switch (field) {
      case 'name':
        nameInputRef.current?.focus();
        break;
      case 'baseUrl':
        baseUrlInputRef.current?.focus();
        break;
      case 'httpApproval':
        document.getElementById('custom-provider-http-approval')?.focus();
        break;
      case 'reasoningEfforts':
        reasoningEffortsInputRef.current?.focus();
        break;
      case 'apiKey':
        apiKeyInputRef.current?.focus();
        break;
    }
  };

  /** Validates the entered values and submits a normalized draft to the owner. */
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const normalizedName = name.trim();
    const normalizedUrl = baseUrl.trim();
    if (normalizedName.length === 0 || normalizedName.length > 80 || containsAsciiControlCharacters(normalizedName)) {
      showValidationError('customProviderInvalid', 'name');
      return;
    }
    if (!isValidBaseUrl(normalizedUrl)) {
      showValidationError('customProviderUrlInvalid', 'baseUrl');
      return;
    }
    if (isHttpUrl(normalizedUrl) && !isIpOrLoopbackHttpUrl(normalizedUrl)) {
      showValidationError('customProviderHttpIpOrLoopbackRequired', 'baseUrl');
      return;
    }
    if (isHttpUrl(normalizedUrl) && !allowInsecureHttp) {
      showValidationError('customProviderHttpApprovalRequired', 'httpApproval');
      return;
    }
    const normalizedApiKey = apiKey.trim();
    if (normalizedApiKey.length > 4096 || (normalizedApiKey.length > 0 && !/^[\x21-\x7E]+$/.test(normalizedApiKey))) {
      showValidationError('apiKeyInvalidFormat', 'apiKey');
      return;
    }
    const reasoningEfforts = [
      ...new Set(
        reasoningEffortsText
          .split(',')
          .map((value) => value.trim())
          .filter(Boolean),
      ),
    ];
    if (
      protocol === 'openAi' &&
      (reasoningEfforts.length > 16 || reasoningEfforts.some((value) => !/^[a-zA-Z0-9_-]{1,64}$/.test(value)))
    ) {
      showValidationError('customProviderReasoningLevelRequired', 'reasoningEfforts');
      return;
    }
    onSave({
      id: provider?.id ?? createCustomProviderId(),
      name: normalizedName,
      baseUrl: normalizedUrl,
      protocol,
      allowInsecureHttp: isHttpUrl(normalizedUrl) && allowInsecureHttp,
      reasoningEfforts: protocol === 'openAi' ? reasoningEfforts : [],
      enableThinking: protocol === 'openAi' && enableThinking,
      apiKey: normalizedApiKey,
    });
  };

  const nameError = errorKey === 'customProviderInvalid' ? t(errorKey) : undefined;
  const baseUrlError =
    errorKey === 'customProviderUrlInvalid' || errorKey === 'customProviderHttpIpOrLoopbackRequired'
      ? t(errorKey)
      : undefined;
  const httpApprovalError = errorKey === 'customProviderHttpApprovalRequired' ? t(errorKey) : undefined;
  const reasoningEffortsError =
    errorKey === 'customProviderReasoningLevelRequired' ? t(errorKey) : undefined;
  const apiKeyError = errorKey === 'apiKeyInvalidFormat' ? t(errorKey) : undefined;

  return (
    <Dialog
      onOpenChange={(nextOpen) => {
        if (saving) return;
        if (!nextOpen) setApiKey('');
        onOpenChange(nextOpen);
      }}
      open={open}
      title={t(provider ? 'editCustomProvider' : 'addCustomProvider')}
    >
      <form className="grid gap-4" onSubmit={submit}>
        <Input
          autoComplete="organization"
          disabled={saving}
          error={nameError}
          label={t('customProviderName')}
          maxLength={80}
          onChange={(event) => {
            setName(event.target.value);
            setErrorKey(null);
          }}
          ref={nameInputRef}
          value={name}
        />
        <Select
          disabled={saving}
          label={t('customProviderProtocol')}
          onChange={(event) => {
            const nextProtocol = event.target.value === 'anthropic' ? 'anthropic' : 'openAi';
            setProtocol(nextProtocol);
            if (nextProtocol === 'anthropic') {
              setReasoningEffortsText('');
              setEnableThinking(false);
            }
            setErrorKey(null);
          }}
          options={[
            { value: 'openAi', label: t('openAiCompatible') },
            { value: 'anthropic', label: t('anthropicCompatible') },
          ]}
          value={protocol}
        />
        {protocol === 'openAi' ? (
          <div className="grid gap-3 rounded-card border border-border bg-surface px-3 py-3">
            <div className="grid gap-1">
              <h3 className="text-sm font-semibold text-text">{t('customProviderReasoningMode')}</h3>
              <p className="text-sm text-secondary-text">{t('customProviderReasoningDescription')}</p>
            </div>
            <Input
              disabled={saving}
              error={reasoningEffortsError}
              label={t('customProviderReasoningEffort')}
              maxLength={1100}
              onChange={(event) => {
                setReasoningEffortsText(event.target.value);
                setErrorKey(null);
              }}
              placeholder={t('customProviderReasoningEffortsPlaceholder')}
              ref={reasoningEffortsInputRef}
              value={reasoningEffortsText}
            />
            <div className="flex items-center justify-between gap-3">
              <span className="text-sm text-text" id="custom-provider-thinking-label">
                {t('customProviderReasoningTemplate')}
              </span>
              <Switch
                aria-labelledby="custom-provider-thinking-label"
                checked={enableThinking}
                disabled={saving}
                onCheckedChange={setEnableThinking}
              />
            </div>
          </div>
        ) : null}
        <Input
          autoCapitalize="none"
          autoComplete="url"
          autoCorrect="off"
          disabled={saving}
          error={baseUrlError}
          label={t('customProviderBaseUrl')}
          maxLength={2048}
          onChange={(event) => {
            setBaseUrl(event.target.value);
            setAllowInsecureHttp(false);
            setErrorKey(null);
          }}
          ref={baseUrlInputRef}
          type="url"
          value={baseUrl}
        />
        {isHttpUrl(baseUrl) ? (
          <div className="-mt-3 grid gap-2">
            <p className="text-sm text-warning" role="status">
              {t('customProviderHttpWarning')}
            </p>
            <div className="flex items-center justify-between gap-3">
              <span className="text-sm text-text" id="custom-provider-http-approval-label">
                {t('customProviderHttpApproval')}
              </span>
              <Switch
                aria-labelledby="custom-provider-http-approval-label"
                aria-describedby={httpApprovalError ? 'custom-provider-http-approval-error' : undefined}
                aria-invalid={httpApprovalError ? true : undefined}
                checked={allowInsecureHttp}
                disabled={saving}
                id="custom-provider-http-approval"
                onCheckedChange={(checked) => {
                  setAllowInsecureHttp(checked);
                  setErrorKey(null);
                }}
              />
            </div>
            {httpApprovalError ? (
              <p className="text-sm text-destructive" id="custom-provider-http-approval-error" role="alert">
                {httpApprovalError}
              </p>
            ) : null}
          </div>
        ) : null}
        <Input
          autoComplete="new-password"
          autoCorrect="off"
          disabled={saving}
          error={apiKeyError}
          label={t('customProviderApiKey')}
          maxLength={4096}
          onChange={(event) => {
            setApiKey(event.target.value);
            setErrorKey(null);
          }}
          placeholder={t(provider ? 'customProviderKeyUnchanged' : 'customProviderApiKeyOptional')}
          ref={apiKeyInputRef}
          type="password"
          value={apiKey}
        />
        {errorKey === null && externalError ? (
          <p className="text-sm text-destructive" role="alert">
            {externalError}
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button disabled={saving} onClick={() => onOpenChange(false)}>
            {t('cancel')}
          </Button>
          <Button disabled={saving} type="submit" variant="primary">
            {saving ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : null}
            {t('save')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

/** Accepts an HTTP(S) API URL without credentials, query parameters, or a fragment. */
function isValidBaseUrl(value: string): boolean {
  if (value.length === 0 || value.length > 2048) return false;
  try {
    const url = new URL(value);
    return (
      (url.protocol === 'http:' || url.protocol === 'https:') &&
      url.hostname.length > 0 &&
      url.username.length === 0 &&
      url.password.length === 0 &&
      url.search.length === 0 &&
      url.hash.length === 0
    );
  } catch {
    return false;
  }
}

/** Determines whether a draft URL needs the cleartext HTTP warning. */
function isHttpUrl(value: string): boolean {
  try {
    return new URL(value.trim()).protocol === 'http:';
  } catch {
    return false;
  }
}

/** Restricts unencrypted HTTP providers to loopback endpoints and direct IP addresses. */
function isIpOrLoopbackHttpUrl(value: string): boolean {
  try {
    const hostname = new URL(value.trim()).hostname.toLowerCase();
    return hostname === 'localhost' || hostname === '[::1]' || isIpv4Literal(hostname);
  } catch {
    return false;
  }
}

/** Matches a normalized dotted-decimal IPv4 address literal. */
function isIpv4Literal(hostname: string): boolean {
  return /^(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}$/.test(hostname);
}

/** Creates a provider identifier using the platform cryptographic UUID source. */
function createCustomProviderId(): `custom_${string}` {
  return `custom_${crypto.randomUUID().replaceAll('-', '')}`;
}
