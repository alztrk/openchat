import { Check, Eye, EyeOff, Info, KeyRound, LoaderCircle, Network, Pencil, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input } from '@/design-system';
import type {
  ChatGptApiKeySummary,
  CompatibleProviderId,
  CustomProviderSummary,
} from '@/modules/providers/data/providerCredentials';
import type { ProviderRequestOutcomeRecord } from '@/modules/providers/data/providerRequestOutcome';
import { ProviderIcon } from './ProviderIcon';
import { ProviderVerificationStatus, type ProviderVerificationViewState } from './ProviderVerificationStatus';

export interface CredentialFeedback {
  key: string;
  kind: 'success' | 'error';
  text: string;
}

export interface ProviderRequestOutcomeView {
  record: ProviderRequestOutcomeRecord | null;
  readFailed: boolean;
  saveFailed: boolean;
}

export interface CredentialCardProps {
  configured: boolean;
  documentationUrl: string;
  draft: string;
  feedback: CredentialFeedback | null;
  formOpen: boolean;
  inputLabel: string;
  keySuffix: string | null;
  label: string;
  loading: boolean;
  onChange(value: string): void;
  onRemove(): void;
  onSave(): void;
  onToggleForm(): void;
  providerId: CompatibleProviderId | 'chatgpt' | 'opencode';
  requestOutcome?: ProviderRequestOutcomeView;
  removing: boolean;
  saving: boolean;
  showGeminiNotice?: boolean;
  statusText: string;
  verification?: ProviderVerificationViewState;
}

export function CredentialCard({
  configured,
  documentationUrl,
  draft,
  feedback,
  formOpen,
  inputLabel,
  keySuffix,
  label,
  loading,
  onChange,
  onRemove,
  onSave,
  onToggleForm,
  providerId,
  requestOutcome,
  removing,
  saving,
  showGeminiNotice = false,
  statusText,
  verification,
}: CredentialCardProps) {
  const { t } = useTranslation();
  const documentationHost = new URL(documentationUrl).hostname;
  return (
    <div className="grid min-w-0 gap-3 p-3 sm:flex sm:items-center sm:justify-between sm:gap-5 sm:p-4">
      <div className="flex min-w-0 items-center gap-3 sm:flex-1">
        <ProviderIcon className="shrink-0" providerId={providerId} />
        <div className="grid min-w-0 gap-1">
          <h3 className="truncate text-sm font-medium">{label}</h3>
          <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
            <CredentialStatus configured={configured} keySuffix={keySuffix} text={statusText} />
            <a
              aria-label={`${label}: ${documentationHost}`}
              className="truncate text-xs text-secondary-text underline underline-offset-2 hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              href={documentationUrl}
              rel="noreferrer"
              target="_blank"
            >
              {documentationHost}
            </a>
          </div>
        </div>
      </div>
      <div className="flex flex-wrap gap-2 sm:shrink-0 sm:justify-end">
        <Button disabled={loading || saving} onClick={onToggleForm} size="small" variant="secondary">
          {configured ? <Pencil aria-hidden="true" size={15} /> : <Plus aria-hidden="true" size={15} />}
          {configured ? t('edit') : t('add')}
        </Button>
        {configured ? (
          <Button disabled={removing || loading || saving} onClick={onRemove} size="small" variant="ghost">
            {removing ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
            ) : (
              <Trash2 aria-hidden="true" size={15} />
            )}
            {t('apiKeyRemoveAction')}
          </Button>
        ) : null}
      </div>
      {showGeminiNotice && formOpen ? (
        <div className="flex items-start gap-2 text-xs leading-5 text-secondary-text sm:basis-full">
          <Info aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={16} />
          <p>{t('geminiUnpaidDataNotice')}</p>
        </div>
      ) : null}
      {configured ? (
        <div className="sm:basis-full">
          <ProviderVerificationStatus requestOutcome={requestOutcome} value={verification} />
        </div>
      ) : null}
      {formOpen ? (
        <div className="sm:basis-full">
          <CredentialInput
            draft={draft}
            inputLabel={inputLabel}
            label={label}
            loading={loading}
            onChange={onChange}
            onSave={onSave}
            onCancel={onToggleForm}
            saving={saving}
            saveLabel={t(configured ? 'apiKeyReplaceAction' : 'apiKeySaveAction')}
          />
        </div>
      ) : null}
      {feedback ? (
        <div className="sm:basis-full">
          <FeedbackMessage feedback={feedback} />
        </div>
      ) : null}
    </div>
  );
}

interface CredentialInputProps {
  draft: string;
  inputLabel?: string;
  label: string;
  loading: boolean;
  onChange(value: string): void;
  onSave(): void;
  onCancel(): void;
  saving: boolean;
  saveLabel: string;
}

export function CredentialInput({
  draft,
  inputLabel,
  label,
  loading,
  onChange,
  onSave,
  onCancel,
  saving,
  saveLabel,
}: CredentialInputProps) {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  return (
    <div className="grid gap-2">
      <div className="relative">
        <Input
          autoComplete="new-password"
          className="pr-12"
          disabled={loading || saving}
          label={inputLabel ?? t('apiKeyInputLabel', { provider: label })}
          onChange={(event) => onChange(event.target.value)}
          placeholder={t('apiKeyInputPlaceholder')}
          type={isVisible ? 'text' : 'password'}
          value={draft}
        />
        <Button
          aria-label={t(isVisible ? 'hideApiKey' : 'showApiKey')}
          className="absolute bottom-0 right-0"
          disabled={loading || saving}
          onClick={() => setIsVisible((current) => !current)}
          size="icon"
          type="button"
          variant="ghost"
        >
          {isVisible ? <EyeOff aria-hidden="true" size={17} /> : <Eye aria-hidden="true" size={17} />}
        </Button>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button disabled={loading || saving} onClick={onCancel} size="small" type="button" variant="secondary">
          {t('cancel')}
        </Button>
        <Button
          disabled={loading || saving || draft.trim().length === 0}
          onClick={onSave}
          size="small"
          variant="primary"
        >
          {saving ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
          ) : (
            <Check aria-hidden="true" size={15} />
          )}
          {saveLabel}
        </Button>
      </div>
    </div>
  );
}

interface SavedChatGptApiKeyProps {
  entry: ChatGptApiKeySummary;
  feedback: CredentialFeedback | null;
  onRemove(): void;
  removing: boolean;
  requestOutcome?: ProviderRequestOutcomeView;
  verification?: ProviderVerificationViewState;
}

export function SavedChatGptApiKey({
  entry,
  feedback,
  onRemove,
  removing,
  requestOutcome,
  verification,
}: SavedChatGptApiKeyProps) {
  const { t } = useTranslation();
  return (
    <div className="grid gap-2 px-3 py-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <KeyRound aria-hidden="true" className="shrink-0 text-secondary-icon" size={18} />
          <p className="truncate text-sm font-medium">
            {entry.keySuffix ? t('apiKeySavedWithSuffix', { suffix: entry.keySuffix }) : t('apiKeyConfigured')}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <span className="text-xs text-secondary-text">{t('apiKey')}</span>
          <Button
            aria-label={t('apiKeyRemoveAction')}
            disabled={removing}
            onClick={onRemove}
            size="icon"
            variant="ghost"
          >
            {removing ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
            ) : (
              <Trash2 aria-hidden="true" size={15} />
            )}
          </Button>
        </div>
      </div>
      <div className="ml-7 mr-2">
        <ProviderVerificationStatus requestOutcome={requestOutcome} value={verification} />
      </div>
      {feedback ? <FeedbackMessage feedback={feedback} /> : null}
    </div>
  );
}

interface CustomProviderCardProps {
  feedback: CredentialFeedback | null;
  onEdit(): void;
  onRemove(): void;
  provider: CustomProviderSummary;
  removing: boolean;
}

export function CustomProviderCard({ feedback, onEdit, onRemove, provider, removing }: CustomProviderCardProps) {
  const { t } = useTranslation();
  return (
    <div className="grid min-w-0 gap-3 rounded-control border border-border bg-raised p-3 sm:flex sm:items-center sm:justify-between">
      <div className="flex min-w-0 items-start gap-3">
        <Network aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={20} />
        <div className="min-w-0">
          <h4 className="truncate font-medium">{provider.name}</h4>
          <p className="mt-1 break-all font-mono text-xs text-secondary-text">
            {t(provider.protocol === 'openAi' ? 'openAiCompatible' : 'anthropicCompatible')} · {provider.baseUrl}
          </p>
          {feedback ? <FeedbackMessage feedback={feedback} /> : null}
        </div>
      </div>
      <div className="flex shrink-0 flex-wrap gap-2">
        <Button
          aria-label={`${t('edit')}: ${provider.name}`}
          disabled={removing}
          onClick={onEdit}
          size="icon"
          variant="ghost"
        >
          <Pencil aria-hidden="true" size={15} />
        </Button>
        <Button
          aria-label={`${t('apiKeyRemoveAction')}: ${provider.name}`}
          disabled={removing}
          onClick={onRemove}
          size="icon"
          variant="ghost"
        >
          {removing ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
          ) : (
            <Trash2 aria-hidden="true" size={15} />
          )}
        </Button>
      </div>
    </div>
  );
}

function CredentialStatus({
  configured,
  keySuffix,
  text,
}: {
  configured: boolean;
  keySuffix: string | null;
  text?: string;
}) {
  const { t } = useTranslation();
  if (!configured) return null;

  const statusLabel = text ?? (keySuffix ? t('apiKeySavedWithSuffix', { suffix: keySuffix }) : t('apiKeyConfigured'));

  return (
    <span
      aria-label={statusLabel}
      className="inline-flex items-center gap-1 text-xs text-success"
      role="status"
      title={statusLabel}
    >
      <Check aria-hidden="true" size={14} />
      {keySuffix ? <span>••{keySuffix}</span> : null}
    </span>
  );
}

export function FeedbackMessage({ feedback }: { feedback: CredentialFeedback }) {
  return (
    <p
      aria-live="polite"
      className={`text-sm ${feedback.kind === 'error' ? 'text-destructive' : 'text-secondary-text'}`}
      role={feedback.kind === 'error' ? 'alert' : 'status'}
    >
      {feedback.text}
    </p>
  );
}
