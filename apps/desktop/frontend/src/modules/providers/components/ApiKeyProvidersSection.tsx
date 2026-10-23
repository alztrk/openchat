import { LoaderCircle, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '@/design-system';
import {
  type CompatibleProviderId,
  type CustomProviderSummary,
  compatibleProviderIds,
  deleteChatGptApiKey,
  deleteCompatibleProviderApiKey,
  deleteCustomProvider,
  deleteOpenCodeApiKey,
  loadProviderCredentialStatus,
  type ProviderCredentialSnapshot,
  saveChatGptApiKey,
  saveCompatibleProviderApiKey,
  saveCustomProvider as saveCustomProviderRecord,
  saveOpenCodeApiKey,
} from '@/modules/providers/data/providerCredentials';
import {
  clearProviderRequestOutcomeRecord,
  type ProviderRequestRoute,
  readProviderRequestOutcomeRecord,
} from '@/modules/providers/data/providerRequestOutcome';
import {
  clearProviderVerificationRecord,
  type ProviderVerificationRoute,
  readProviderVerificationRecord,
} from '@/modules/providers/data/providerVerification';
import {
  CredentialCard,
  type CredentialFeedback,
  CredentialInput,
  CustomProviderCard,
  FeedbackMessage,
  type ProviderRequestOutcomeView,
  SavedChatGptApiKey,
} from './ApiKeyProviderCards';
import { CustomProviderDialog, type CustomProviderDraft } from './CustomProviderDialog';
import {
  notCheckedVerificationState,
  type ProviderVerificationViewState,
  providerVerificationStateForOutcome,
  requestOutcomeEntries,
  verificationEntries,
} from './ProviderVerificationStatus';

interface ApiKeyProvidersSectionProps {
  mode: 'chatgpt' | 'modelProviders';
  onChanged(): Promise<void>;
  chatGptApiKeyFormOpen?: boolean;
  onChatGptApiKeyFormToggle?(): void;
}

type CompatibleProviderNameKey =
  | 'providerNameGemini'
  | 'providerNameGroq'
  | 'providerNameCerebras'
  | 'providerNameOpenRouter'
  | 'providerNameMistral'
  | 'providerNameDeepSeek'
  | 'providerNameXai'
  | 'providerNamePerplexity'
  | 'providerNameTogether'
  | 'providerNameSambaNova'
  | 'providerNameAnthropic'
  | 'providerNameFireworks'
  | 'providerNameDeepInfra'
  | 'providerNameDashScope'
  | 'providerNameCohere'
  | 'providerNameKilo';

const providerNameKeys: Record<CompatibleProviderId, CompatibleProviderNameKey> = {
  gemini: 'providerNameGemini',
  groq: 'providerNameGroq',
  cerebras: 'providerNameCerebras',
  openrouter: 'providerNameOpenRouter',
  mistral: 'providerNameMistral',
  deepseek: 'providerNameDeepSeek',
  xai: 'providerNameXai',
  perplexity: 'providerNamePerplexity',
  together: 'providerNameTogether',
  sambanova: 'providerNameSambaNova',
  anthropic: 'providerNameAnthropic',
  fireworks: 'providerNameFireworks',
  deepinfra: 'providerNameDeepInfra',
  dashscope: 'providerNameDashScope',
  cohere: 'providerNameCohere',
  kilo: 'providerNameKilo',
};

type CompatibleProviderDescriptionKey =
  | 'geminiApiDescription'
  | 'groqApiDescription'
  | 'cerebrasApiDescription'
  | 'openRouterApiDescription'
  | 'mistralApiDescription'
  | 'deepSeekApiDescription'
  | 'xAiApiDescription'
  | 'perplexityApiDescription'
  | 'togetherApiDescription'
  | 'sambaNovaApiDescription'
  | 'anthropicApiDescription'
  | 'fireworksApiDescription'
  | 'deepInfraApiDescription'
  | 'dashScopeApiDescription'
  | 'cohereApiDescription'
  | 'kiloApiDescription';

const providerDescriptionKeys: Record<CompatibleProviderId, CompatibleProviderDescriptionKey> = {
  gemini: 'geminiApiDescription',
  groq: 'groqApiDescription',
  cerebras: 'cerebrasApiDescription',
  openrouter: 'openRouterApiDescription',
  mistral: 'mistralApiDescription',
  deepseek: 'deepSeekApiDescription',
  xai: 'xAiApiDescription',
  perplexity: 'perplexityApiDescription',
  together: 'togetherApiDescription',
  sambanova: 'sambaNovaApiDescription',
  anthropic: 'anthropicApiDescription',
  fireworks: 'fireworksApiDescription',
  deepinfra: 'deepInfraApiDescription',
  dashscope: 'dashScopeApiDescription',
  cohere: 'cohereApiDescription',
  kilo: 'kiloApiDescription',
};

const providerDocumentationUrls: Record<CompatibleProviderId, string> = {
  gemini: 'https://ai.google.dev/gemini-api/docs',
  groq: 'https://console.groq.com/docs',
  cerebras: 'https://docs.cerebras.ai/',
  openrouter: 'https://openrouter.ai/docs',
  mistral: 'https://docs.mistral.ai/',
  deepseek: 'https://api-docs.deepseek.com/',
  xai: 'https://docs.x.ai/',
  perplexity: 'https://docs.perplexity.ai/',
  together: 'https://docs.together.ai/',
  sambanova: 'https://docs.sambanova.ai/',
  anthropic: 'https://platform.claude.com/docs/en/api/overview',
  fireworks: 'https://docs.fireworks.ai/',
  deepinfra: 'https://docs.deepinfra.com/',
  dashscope: 'https://www.alibabacloud.com/help/en/model-studio/',
  cohere: 'https://docs.cohere.com/',
  kilo: 'https://kilo.ai/docs',
};

type CredentialRoute =
  | { kind: 'chatgpt'; connectionId: string; label: string }
  | { kind: 'compatible'; providerId: CompatibleProviderId; label: string }
  | { kind: 'custom'; provider: CustomProviderSummary; label: string }
  | { kind: 'opencode'; label: string };

type CredentialInputRoute = Exclude<CredentialRoute, { kind: 'custom' }>;
type CredentialVerificationRoute = Exclude<CredentialRoute, { kind: 'custom' }>;

export function ApiKeyProvidersSection({
  chatGptApiKeyFormOpen,
  mode,
  onChanged,
  onChatGptApiKeyFormToggle,
}: ApiKeyProvidersSectionProps) {
  const { t } = useTranslation();
  const chatGptApiAddRoute: CredentialRoute = {
    kind: 'chatgpt',
    connectionId: 'new',
    label: t('providerNameChatGptApi'),
  };
  const [snapshot, setSnapshot] = useState<ProviderCredentialSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadFailed, setLoadFailed] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [openFormKeys, setOpenFormKeys] = useState<Record<string, boolean>>({});
  const [savingKey, setSavingKey] = useState<string | null>(null);
  const [verificationRecords, setVerificationRecords] = useState<Record<string, ProviderVerificationViewState>>({});
  const [verificationResetKeys, setVerificationResetKeys] = useState<Record<string, boolean>>({});
  const [verificationResetFailedKeys, setVerificationResetFailedKeys] = useState<Record<string, boolean>>({});
  const [requestOutcomes, setRequestOutcomes] = useState<Record<string, ProviderRequestOutcomeView>>({});
  const isMountedRef = useRef(true);
  const [removingKey, setRemovingKey] = useState<string | null>(null);
  const [pendingRemoval, setPendingRemoval] = useState<CredentialRoute | null>(null);
  const [editingCustomProvider, setEditingCustomProvider] = useState<CustomProviderSummary | 'new' | null>(null);
  const [savingCustomProvider, setSavingCustomProvider] = useState(false);
  const [customDialogError, setCustomDialogError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<CredentialFeedback | null>(null);

  const reloadStatus = useCallback(async () => {
    setLoading(true);
    try {
      setSnapshot(await loadProviderCredentialStatus());
      setLoadFailed(false);
    } catch {
      setSnapshot(null);
      setLoadFailed(true);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    void reloadStatus();
  }, [reloadStatus]);

  useEffect(() => {
    if (!snapshot) return;
    let cancelled = false;
    const loadVerificationRecords = async () => {
      const entries = verificationEntries(snapshot);
      const next: Record<string, ProviderVerificationViewState> = {};
      for (const [key, route] of entries) {
        if (verificationResetKeys[key]) {
          next[key] = {
            ...notCheckedVerificationState(),
            saveFailed: verificationResetFailedKeys[key] === true,
          };
          continue;
        }
        try {
          const record = readProviderVerificationRecord(route);
          next[key] = {
            state: record ? providerVerificationStateForOutcome(record.outcome) : 'notChecked',
            checkedAtUnixMs: record?.checkedAtUnixMs ?? null,
            recordReadFailed: false,
            saveFailed: false,
          };
        } catch {
          next[key] = {
            state: 'notChecked',
            checkedAtUnixMs: null,
            recordReadFailed: true,
            saveFailed: false,
          };
        }
      }
      const requestNext: Record<string, ProviderRequestOutcomeView> = {};
      for (const [key, route] of requestOutcomeEntries(snapshot)) {
        try {
          requestNext[key] = { record: readProviderRequestOutcomeRecord(route), readFailed: false, saveFailed: false };
        } catch {
          requestNext[key] = { record: null, readFailed: true, saveFailed: false };
        }
      }
      if (!cancelled && isMountedRef.current) {
        setVerificationRecords(next);
        setRequestOutcomes(requestNext);
      }
    };
    void loadVerificationRecords();
    return () => {
      cancelled = true;
    };
  }, [snapshot, verificationResetFailedKeys, verificationResetKeys]);

  const setDraft = (key: string, value: string) => {
    setDrafts((current) => ({ ...current, [key]: value }));
  };

  const clearDraft = (key: string) => {
    setDrafts((current) => {
      if (!(key in current)) return current;
      const next = { ...current };
      delete next[key];
      return next;
    });
  };

  const toggleCredentialForm = (key: string) => {
    if (mode === 'chatgpt' && key === routeKey(chatGptApiAddRoute) && onChatGptApiKeyFormToggle) {
      if (chatGptApiKeyFormOpen) {
        clearDraft(key);
        setFeedback((current) => (current?.key === key ? null : current));
      }
      onChatGptApiKeyFormToggle();
      return;
    }
    const isClosing = openFormKeys[key] === true;
    if (isClosing) {
      clearDraft(key);
      setFeedback((current) => (current?.key === key ? null : current));
    }
    setOpenFormKeys((current) => ({ ...current, [key]: !current[key] }));
  };

  const closeCredentialForm = (key: string) => {
    if (mode === 'chatgpt' && key === routeKey(chatGptApiAddRoute) && onChatGptApiKeyFormToggle) {
      clearDraft(key);
      setFeedback((current) => (current?.key === key ? null : current));
      if (chatGptApiKeyFormOpen) onChatGptApiKeyFormToggle();
      return;
    }
    clearDraft(key);
    setOpenFormKeys((current) => {
      if (!current[key]) return current;
      return { ...current, [key]: false };
    });
  };

  const resetVerification = (key: string, route: CredentialVerificationRoute) => {
    setVerificationResetKeys((current) => ({ ...current, [key]: true }));
    setVerificationRecords((current) => ({ ...current, [key]: notCheckedVerificationState() }));
    setVerificationResetFailedKeys((current) => {
      if (!current[key]) return current;
      const next = { ...current };
      delete next[key];
      return next;
    });
    try {
      clearProviderVerificationRecord(providerVerificationRouteForCredential(route));
    } catch {
      setVerificationResetFailedKeys((current) => ({ ...current, [key]: true }));
      setVerificationRecords((current) => ({
        ...current,
        [key]: { ...notCheckedVerificationState(), saveFailed: true },
      }));
    }
  };

  const clearRequestOutcome = (key: string, route: ProviderRequestRoute) => {
    let saveFailed = false;
    try {
      clearProviderRequestOutcomeRecord(route);
    } catch {
      saveFailed = true;
    }
    setRequestOutcomes((current) => ({ ...current, [key]: { record: null, readFailed: false, saveFailed } }));
  };

  const saveCredential = async (route: CredentialInputRoute, apiKey: string, feedbackKey = routeKey(route)) => {
    const key = feedbackKey;
    if (savingKey !== null || apiKey.trim().length === 0) return;
    const invalidFormatKey = route.kind === 'chatgpt' ? 'openAiApiKeyInvalidFormat' : 'apiKeyInvalidFormat';
    if (!isValidApiKey(route.kind === 'compatible' ? route.providerId : route.kind, apiKey)) {
      setFeedback({ key, kind: 'error', text: t(invalidFormatKey) });
      return;
    }
    setSavingKey(key);
    setFeedback(null);
    try {
      if (route.kind === 'chatgpt') {
        await saveChatGptApiKey(route.connectionId, apiKey);
      } else if (route.kind === 'compatible') {
        await saveCompatibleProviderApiKey(route.providerId, apiKey);
      } else {
        await saveOpenCodeApiKey(apiKey);
      }
      setDraft(key, '');
      closeCredentialForm(key);
      resetVerification(key, route);
      clearRequestOutcome(key, providerRequestRouteForCredential(route));
      await reloadStatus();
      await onChanged();
      setFeedback({
        key,
        kind: 'success',
        text: t(route.kind === 'chatgpt' ? 'openAiApiKeySaved' : 'apiKeySaved'),
      });
    } catch (error) {
      const code = readErrorCode(error);
      setFeedback({
        key,
        kind: 'error',
        text:
          code === 'provider_credential_duplicate'
            ? t('apiKeyDuplicate')
            : code === 'provider_credential_invalid'
              ? t(invalidFormatKey)
              : t('apiKeyStorageSaveFailed'),
      });
    } finally {
      setSavingKey(null);
    }
  };

  const confirmRemoval = async () => {
    if (!pendingRemoval || removingKey !== null) return;
    const route = pendingRemoval;
    const key = routeKey(route);
    setRemovingKey(key);
    setFeedback(null);
    try {
      if (route.kind === 'chatgpt') {
        await deleteChatGptApiKey(route.connectionId);
      } else if (route.kind === 'compatible') {
        await deleteCompatibleProviderApiKey(route.providerId);
      } else if (route.kind === 'custom') {
        await deleteCustomProvider(route.provider.id);
      } else {
        await deleteOpenCodeApiKey();
      }
      setPendingRemoval(null);
      if (route.kind !== 'custom') resetVerification(key, route);
      clearRequestOutcome(key, providerRequestRouteForCredential(route));
      await reloadStatus();
      await onChanged();
      setFeedback({ key, kind: 'success', text: t('connectionRemoveSucceeded') });
    } catch {
      setFeedback({ key, kind: 'error', text: t('apiKeyStorageRemoveFailed') });
    } finally {
      setRemovingKey(null);
    }
  };

  const saveCustomProvider = async (draft: CustomProviderDraft) => {
    const key = `custom:${draft.id}`;
    setSavingCustomProvider(true);
    setCustomDialogError(null);
    setFeedback(null);
    try {
      await saveCustomProviderRecord(draft);
      setEditingCustomProvider(null);
      clearRequestOutcome(key, { providerId: draft.id, connectionId: draft.id, workspaceId: null });
      await reloadStatus();
      await onChanged();
      setFeedback({ key, kind: 'success', text: t('apiKeySaved') });
    } catch (error) {
      const code = readErrorCode(error);
      setCustomDialogError(
        t(
          code === 'custom_provider_key_required_for_endpoint_change'
            ? 'customProviderKeyRequiredForEndpointChange'
            : code === 'custom_provider_http_approval_required'
              ? 'customProviderHttpApprovalRequired'
              : code === 'custom_provider_http_approval_denied'
                ? 'customProviderHttpApprovalDenied'
                : code === 'provider_credential_invalid'
                  ? 'customProviderInvalid'
                  : 'apiKeyStorageSaveFailed',
        ),
      );
    } finally {
      setSavingCustomProvider(false);
    }
  };

  const isChatGptApiFormOpen =
    mode === 'chatgpt' && onChatGptApiKeyFormToggle
      ? chatGptApiKeyFormOpen === true
      : openFormKeys[routeKey(chatGptApiAddRoute)] === true;
  const openCodeRoute: CredentialRoute = { kind: 'opencode', label: t('openCodeConsole') };
  const openCodeConfigured = snapshot?.openCodeConfigured === true;
  const openCodeKeySuffix = snapshot?.openCodeKeySuffix ?? null;
  const apiKeyLoadingState = loading ? (
    <div className="px-5 py-5">
      <div aria-label={t('modelsLoading')} className="h-0.5 animate-pulse rounded-full bg-accent" role="status" />
    </div>
  ) : null;
  const loadFailureState = loadFailed ? (
    <div
      className={`flex flex-wrap items-center justify-between gap-3 px-5 py-4 ${mode === 'chatgpt' ? '' : 'rounded-card border border-destructive bg-surface'}`}
      role="alert"
    >
      <p className="text-sm text-destructive">{t('apiKeyStorageUnavailable')}</p>
      <Button disabled={loading} onClick={() => void reloadStatus()} size="small">
        {loading ? (
          <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
        ) : (
          <RefreshCw aria-hidden="true" size={16} />
        )}
        {t('retry')}
      </Button>
    </div>
  ) : null;

  const chatGptApiContent =
    snapshot && !loading ? (
      <>
        {snapshot.chatgptApiKeys.length > 0 ? (
          <div className="grid divide-y divide-border">
            {snapshot.chatgptApiKeys.map((entry) => {
              const route: CredentialRoute = {
                kind: 'chatgpt',
                connectionId: entry.id,
                label: t('providerNameChatGptApi'),
              };
              return (
                <SavedChatGptApiKey
                  entry={entry}
                  feedback={feedback?.key === routeKey(route) ? feedback : null}
                  key={entry.id}
                  onRemove={() => setPendingRemoval(route)}
                  removing={removingKey === routeKey(route)}
                  requestOutcome={requestOutcomes[routeKey(route)]}
                  verification={verificationRecords[routeKey(route)]}
                />
              );
            })}
          </div>
        ) : null}
        {isChatGptApiFormOpen ? (
          <div className={`grid gap-2 px-3 py-3 ${snapshot.chatgptApiKeys.length > 0 ? 'border-t border-border' : ''}`}>
            <CredentialInput
              draft={drafts[routeKey(chatGptApiAddRoute)] ?? ''}
              inputLabel={t('openAiApiKeyInputLabel')}
              label={t('providerNameChatGptApi')}
              loading={false}
              onChange={(value) => setDraft(routeKey(chatGptApiAddRoute), value)}
              onSave={() =>
                void saveCredential(
                  { ...chatGptApiAddRoute, connectionId: createCredentialId() },
                  drafts[routeKey(chatGptApiAddRoute)] ?? '',
                  routeKey(chatGptApiAddRoute),
                )
              }
              onCancel={() => toggleCredentialForm(routeKey(chatGptApiAddRoute))}
              saving={savingKey === routeKey(chatGptApiAddRoute)}
              saveLabel={t('apiKeyAddAction')}
            />
          </div>
        ) : null}
        {feedback?.key === routeKey(chatGptApiAddRoute) ? (
          <div className="px-3 pb-3">
            <FeedbackMessage feedback={feedback} />
          </div>
        ) : null}
      </>
    ) : null;

  const modelProviderCards =
    snapshot && !loading ? (
      <div className="grid gap-8">
        <div className="grid min-w-0 gap-4 lg:grid-cols-2">
          <CredentialCard
            configured={openCodeConfigured}
            description={t('openCodeConsoleDescription')}
            documentationUrl="https://opencode.ai/zen"
            draft={drafts.opencode ?? ''}
            feedback={feedback?.key === routeKey(openCodeRoute) ? feedback : null}
            formOpen={openFormKeys[routeKey(openCodeRoute)] === true}
            inputLabel={t('openCodeApiKey')}
            keySuffix={openCodeKeySuffix}
            label={t('openCodeConsole')}
            loading={false}
            onChange={(value) => setDraft('opencode', value)}
            onRemove={() => setPendingRemoval(openCodeRoute)}
            onSave={() => void saveCredential(openCodeRoute, drafts.opencode ?? '')}
            onToggleForm={() => toggleCredentialForm(routeKey(openCodeRoute))}
            providerId="opencode"
            removing={removingKey === routeKey(openCodeRoute)}
            saving={savingKey === routeKey(openCodeRoute)}
            statusText={
              openCodeConfigured
                ? openCodeKeySuffix
                  ? t('openCodeKeySaved', { suffix: openCodeKeySuffix })
                  : t('providerKeySaved')
                : t('openCodeNoKey')
            }
            requestOutcome={requestOutcomes[routeKey(openCodeRoute)]}
            verification={verificationRecords[routeKey(openCodeRoute)]}
          />

          {compatibleProviderIds.map((providerId) => {
            const credential = snapshot.compatibleProviders.find((item) => item.providerId === providerId);
            const route: CredentialRoute = { kind: 'compatible', providerId, label: t(providerNameKeys[providerId]) };
            const key = routeKey(route);
            return (
              <CredentialCard
                configured={credential?.isConfigured === true}
                description={t(providerDescriptionKeys[providerId])}
                documentationUrl={providerDocumentationUrls[providerId]}
                draft={drafts[key] ?? ''}
                feedback={feedback?.key === key ? feedback : null}
                formOpen={openFormKeys[key] === true}
                inputLabel={t('providerApiKey')}
                key={providerId}
                keySuffix={credential?.keySuffix ?? null}
                label={route.label}
                loading={false}
                onChange={(value) => setDraft(key, value)}
                onRemove={() => setPendingRemoval(route)}
                onSave={() => void saveCredential(route, drafts[key] ?? '')}
                onToggleForm={() => toggleCredentialForm(key)}
                providerId={providerId}
                removing={removingKey === key}
                saving={savingKey === key}
                statusText={
                  credential?.isConfigured
                    ? credential.keySuffix
                      ? t('providerKeySavedSuffix', { suffix: credential.keySuffix })
                      : t('providerKeySaved')
                    : t('providerNoKey')
                }
                requestOutcome={requestOutcomes[key]}
                verification={verificationRecords[key]}
                showGeminiNotice={providerId === 'gemini'}
              />
            );
          })}
        </div>

        <div className="grid min-w-0 gap-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h3 className="font-semibold">{t('customProvidersHeading')}</h3>
              <p className="mt-1 text-sm text-secondary-text">{t('customProvidersDescription')}</p>
            </div>
            <Button
              disabled={savingCustomProvider}
              onClick={() => {
                setCustomDialogError(null);
                setEditingCustomProvider('new');
              }}
              size="small"
              variant="secondary"
            >
              <Plus aria-hidden="true" size={15} />
              {t('addCustomProvider')}
            </Button>
          </div>
          {snapshot.customProviders.length === 0 ? (
            <p className="text-sm text-secondary-text">{t('customProvidersEmpty')}</p>
          ) : null}
          {snapshot.customProviders.map((provider) => {
            const route: CredentialRoute = { kind: 'custom', provider, label: provider.name };
            const key = routeKey(route);
            return (
              <CustomProviderCard
                feedback={feedback?.key === key ? feedback : null}
                key={provider.id}
                onEdit={() => {
                  setCustomDialogError(null);
                  setEditingCustomProvider(provider);
                }}
                onRemove={() => setPendingRemoval(route)}
                provider={provider}
                removing={removingKey === key}
              />
            );
          })}
        </div>
      </div>
    ) : null;
  const showChatGptApiBody =
    loading || loadFailed || (snapshot !== null && snapshot.chatgptApiKeys.length > 0) || isChatGptApiFormOpen;

  return (
    <>
      {mode === 'chatgpt' ? (
        showChatGptApiBody ? (
          <div className="grid border-t border-border">
            {loadFailureState}
            {loading ? apiKeyLoadingState : chatGptApiContent}
          </div>
        ) : null
      ) : (
        <div className="grid gap-4">
          {loadFailureState}
          {loading ? apiKeyLoadingState : modelProviderCards}
        </div>
      )}

      <Dialog
        onOpenChange={(open) => {
          if (!open && removingKey === null) setPendingRemoval(null);
        }}
        open={pendingRemoval !== null}
        title={t('apiKeyRemoveDialogTitle')}
      >
        <p className="text-sm leading-6 text-secondary-text">
          {t('apiKeyRemoveDialogBody', { provider: pendingRemoval?.label ?? '' })}
        </p>
        <div className="mt-6 flex justify-end gap-2">
          <Button disabled={removingKey !== null} onClick={() => setPendingRemoval(null)}>
            {t('cancel')}
          </Button>
          <Button disabled={removingKey !== null} onClick={() => void confirmRemoval()} variant="destructive">
            {removingKey !== null ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            ) : (
              <Trash2 aria-hidden="true" size={16} />
            )}
            {t('apiKeyRemoveAction')}
          </Button>
        </div>
      </Dialog>

      {mode === 'modelProviders' ? (
        <CustomProviderDialog
          onOpenChange={(open) => {
            if (!open && !savingCustomProvider) {
              setEditingCustomProvider(null);
              setCustomDialogError(null);
            }
          }}
          onSave={(draft) => void saveCustomProvider(draft)}
          externalError={customDialogError}
          open={editingCustomProvider !== null}
          provider={editingCustomProvider === 'new' ? null : editingCustomProvider}
          saving={savingCustomProvider}
        />
      ) : null}
    </>
  );
}

function routeKey(route: CredentialRoute): string {
  if (route.kind === 'chatgpt')
    return route.connectionId === 'new' ? 'chatgpt_api:add' : `chatgpt_api:${route.connectionId}`;
  if (route.kind === 'compatible') return `compatible:${route.providerId}`;
  if (route.kind === 'custom') return `custom:${route.provider.id}`;
  return 'opencode';
}

function providerVerificationRouteForCredential(route: CredentialVerificationRoute): ProviderVerificationRoute {
  if (route.kind === 'chatgpt') {
    return { providerId: 'chatgpt_api', connectionId: route.connectionId, workspaceId: null };
  }
  if (route.kind === 'compatible') {
    return { providerId: route.providerId, connectionId: route.providerId, workspaceId: null };
  }
  return { providerId: 'opencode', connectionId: 'opencode', workspaceId: null };
}

function providerRequestRouteForCredential(route: CredentialRoute): ProviderRequestRoute {
  if (route.kind === 'custom') {
    return { providerId: route.provider.id, connectionId: route.provider.id, workspaceId: null };
  }
  return providerVerificationRouteForCredential(route);
}

function isValidApiKey(providerId: string, input: string): boolean {
  const value = input.trim();
  if (value.length === 0 || value.length > 4096 || !/^[\x21-\x7E]+$/.test(value)) return false;
  if (providerId === 'chatgpt' || providerId === 'chatgpt_api') return value.startsWith('sk-');
  if (providerId === 'gemini') return value.startsWith('AIza') || value.startsWith('AQ.');
  if (providerId === 'groq') return value.startsWith('gsk_');
  if (providerId === 'openrouter') return value.startsWith('sk-or-v1-');
  return true;
}

function readErrorCode(error: unknown): string | null {
  if (isObject(error) && typeof error.code === 'string') return error.code;
  if (typeof error !== 'string') return null;
  try {
    const parsed: unknown = JSON.parse(error);
    return isObject(parsed) && typeof parsed.code === 'string' ? parsed.code : null;
  } catch {
    return null;
  }
}

function createCredentialId(): string {
  return crypto.randomUUID().replaceAll('-', '');
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
