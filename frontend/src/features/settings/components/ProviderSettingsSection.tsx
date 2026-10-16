import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import {
  Circle,
  CircleAlert,
  CircleCheck,
  CircleUserRound,
  KeyRound,
  ListFilter,
  LoaderCircle,
  LogIn,
  MessagesSquare,
  RefreshCw,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Select } from '@/shared/ui';
import { LocalServiceError, type ServiceOperation } from '@/shared/services/LocalServiceClient';
import type { ChatModel } from '@/features/chat/data/chatModels';
import { ProviderIcon } from '@/features/chat/components/ProviderIcon';
import { localServiceClient, useChatStore } from '@/features/chat/stores/chatStore';
import { formatModelRouteKey, modelRouteKey } from '@/features/settings/data/modelPreferences';
import {
  readProviderVerificationRecord,
  type ProviderVerificationRecord,
} from '@/features/settings/data/providerVerification';
import {
  readProviderRequestOutcomeRecord,
  type ProviderRequestOutcomeRecord,
} from '@/features/settings/data/providerRequestOutcome';
import {
  readTitleGenerationPreference,
  titlePreferenceForModel,
  type TitleGenerationMode,
  type TitleGenerationPreference,
  writeTitleGenerationPreference,
} from '@/features/settings/data/titleGenerationPreferences';
import { ApiKeyProvidersSection } from './ApiKeyProvidersSection';
import {
  ProviderVerificationStatus,
  providerVerificationStateForOutcome,
} from './ProviderVerificationStatus';

interface AccountWorkspace {
  id: string;
  displayName: string | null;
  planType: string | null;
  isSelected: boolean;
}

interface AccountConnection {
  id: string;
  authStatus: string;
  displayName: string | null;
  email: string | null;
  planType: string | null;
  isSelected: boolean;
  workspaces: AccountWorkspace[];
}

type ConnectionRequest = 'idle' | 'loading' | 'signingIn' | 'error';
type VerificationState = 'notChecked' | 'checking' | 'verified' | 'empty' | 'stale' | 'authenticationFailed' | 'rateLimited' | 'networkFailed' | 'failed';
type ConnectionErrorCode =
  | 'oauthConnectionsLoadFailed'
  | 'oauthSignInFailed'
  | 'connectionSelectionFailed'
  | 'workspaceSelectionFailed'
  | 'connectionRemoveFailed';

export function ProviderSettingsSection() {
  const { t } = useTranslation();
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelError = useChatStore((state) => state.modelError);
  const models = useChatStore((state) => state.models);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const [request, setRequest] = useState<ConnectionRequest>('loading');
  const [oauthOperation, setOauthOperation] = useState<ServiceOperation | null>(null);
  const [isCancellingOAuth, setIsCancellingOAuth] = useState(false);
  const [connections, setConnections] = useState<AccountConnection[]>([]);
  const isMountedRef = useRef(true);
  const oauthOperationRef = useRef<ServiceOperation | null>(null);
  const [errorCode, setErrorCode] = useState<ConnectionErrorCode | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [removeTarget, setRemoveTarget] = useState<AccountConnection | null>(null);
  const [removingId, setRemovingId] = useState<string | null>(null);
  const [verificationState, setVerificationState] = useState<VerificationState>('notChecked');
  const [verificationCheckedAt, setVerificationCheckedAt] = useState<number | null>(null);
  const [verificationRecordReadFailed, setVerificationRecordReadFailed] = useState(false);
  const [verificationSaveFailed, setVerificationSaveFailed] = useState(false);
  const [lastRequestOutcome, setLastRequestOutcome] = useState<ProviderRequestOutcomeRecord | null>(null);
  const [lastRequestReadFailed, setLastRequestReadFailed] = useState(false);
  const [notice, setNotice] = useState<'connectionRemoveSucceeded' | null>(null);
  const [titlePreference, setTitlePreference] = useState<TitleGenerationPreference>({ mode: 'conversation', model: null });
  const [titlePreferenceLoadFailed, setTitlePreferenceLoadFailed] = useState(false);
  const [titleError, setTitleError] = useState<'titlePreferenceSaveFailed' | null>(null);
  const [titleSaving, setTitleSaving] = useState(false);
  const [chatGptApiKeyFormOpen, setChatGptApiKeyFormOpen] = useState(false);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      const operation = oauthOperationRef.current;
      oauthOperationRef.current = null;
      if (operation) void cancelOAuthAfterUnmount(operation);
    };
  }, []);

  const loadConnections = useCallback(async () => {
    setRequest('loading');
    try {
      const response = await localServiceClient.call('chatgpt.connections.list');
      if (!Array.isArray(response.connections)) throw new Error('Invalid account response.');
      const items = response.connections.flatMap(parseConnection);
      if (!isMountedRef.current) return;
      const selectedConnection = items.find((connection) => connection.isSelected);
      const selectedWorkspace = selectedConnection?.workspaces.find((workspace) => workspace.isSelected);
      let verificationRecord: ProviderVerificationRecord | null = null;
      let verificationReadFailed = false;
      let requestRecord: ProviderRequestOutcomeRecord | null = null;
      let requestReadFailed = false;
      if (selectedConnection && selectedWorkspace) {
        const route = {
          providerId: 'chatgpt_oauth',
          connectionId: selectedConnection.id,
          workspaceId: selectedWorkspace.id,
        };
        try {
          verificationRecord = readProviderVerificationRecord(route);
        } catch {
          verificationReadFailed = true;
        }
        try {
          requestRecord = readProviderRequestOutcomeRecord(route);
        } catch {
          requestReadFailed = true;
        }
      }
      setConnections(items);
      setSelectedId(selectedConnection?.id ?? null);
      setVerificationState(verificationRecord ? providerVerificationStateForOutcome(verificationRecord.outcome) : 'notChecked');
      setVerificationCheckedAt(verificationRecord?.checkedAtUnixMs ?? null);
      setVerificationRecordReadFailed(verificationReadFailed);
      setVerificationSaveFailed(false);
      setLastRequestOutcome(requestRecord);
      setLastRequestReadFailed(requestReadFailed);
      setErrorCode(null);
      setRequest('idle');
    } catch {
      if (!isMountedRef.current) return;
      setErrorCode('oauthConnectionsLoadFailed');
      setRequest('error');
    }
  }, []);

  useEffect(() => {
    if (serviceStatus === 'ready') {
      void loadConnections();
      return;
    }
    if (serviceStatus === 'error') {
      setErrorCode('oauthConnectionsLoadFailed');
      setRequest('error');
    }
  }, [loadConnections, serviceStatus]);

  const loadTitlePreference = useCallback(() => {
    try {
      setTitlePreference(readTitleGenerationPreference());
      setTitlePreferenceLoadFailed(false);
    } catch {
      setTitlePreferenceLoadFailed(true);
    }
  }, []);

  useEffect(() => {
    loadTitlePreference();
  }, [loadTitlePreference]);

  const signIn = async () => {
    if (request === 'signingIn') return;
    setRequest('signingIn');
    setErrorCode(null);
    setNotice(null);
    let operation: ServiceOperation | null = null;
    try {
      operation = await localServiceClient.startOperation('chatgpt.oauth.start', {});
      if (!isMountedRef.current) {
        await cancelOAuthAfterUnmount(operation);
        return;
      }
      oauthOperationRef.current = operation;
      setOauthOperation(operation);
      await operation.result;
      if (oauthOperationRef.current === operation) oauthOperationRef.current = null;
      if (!isMountedRef.current) return;
      setOauthOperation(null);
      await loadConnections();
      if (!isMountedRef.current) return;
      await refreshModels();
    } catch (error) {
      if (!isMountedRef.current) return;
      if (!(error instanceof LocalServiceError) || error.code !== 'oauth_cancelled') {
        setErrorCode('oauthSignInFailed');
      }
      setRequest('idle');
    } finally {
      if (oauthOperationRef.current === operation) oauthOperationRef.current = null;
      if (isMountedRef.current) {
        setOauthOperation(null);
        setIsCancellingOAuth(false);
      }
    }
  };

  const cancelOAuth = async () => {
    const operation = oauthOperation;
    if (!operation || isCancellingOAuth) return;
    setIsCancellingOAuth(true);
    try {
      await operation.cancel();
    } catch {
      setErrorCode('oauthSignInFailed');
    } finally {
      setIsCancellingOAuth(false);
    }
  };

  const selectConnection = async (connectionId: string) => {
    setVerificationState('notChecked');
    setVerificationCheckedAt(null);
    setVerificationRecordReadFailed(false);
    setVerificationSaveFailed(false);
    setLastRequestOutcome(null);
    setLastRequestReadFailed(false);
    setErrorCode(null);
    setNotice(null);
    try {
      await localServiceClient.call('chatgpt.connections.select', { connectionId });
      await loadConnections();
      await refreshModels();
    } catch {
      setErrorCode('connectionSelectionFailed');
    }
  };

  const selectWorkspace = async (connectionId: string, workspaceId: string) => {
    setVerificationState('notChecked');
    setVerificationCheckedAt(null);
    setVerificationRecordReadFailed(false);
    setVerificationSaveFailed(false);
    setLastRequestOutcome(null);
    setLastRequestReadFailed(false);
    setErrorCode(null);
    setNotice(null);
    try {
      await localServiceClient.call('chatgpt.workspaces.select', { connectionId, workspaceId });
      await loadConnections();
      await refreshModels();
    } catch {
      setErrorCode('workspaceSelectionFailed');
    }
  };

  const removeConnection = async () => {
    const target = removeTarget;
    if (!target || removingId !== null) return;
    setRemovingId(target.id);
    setErrorCode(null);
    setNotice(null);
    try {
      const response = await localServiceClient.call('chatgpt.connections.delete', {
        connectionId: target.id,
      });
      if (response.removed !== true) throw new Error('The connection was not removed.');
      setRemoveTarget(null);
      setNotice('connectionRemoveSucceeded');
      await loadConnections();
      await refreshModels();
    } catch {
      setErrorCode('connectionRemoveFailed');
    } finally {
      setRemovingId(null);
    }
  };

  const saveTitlePreference = (preference: TitleGenerationPreference) => {
    setTitleSaving(true);
    try {
      writeTitleGenerationPreference(preference);
      setTitlePreference(preference);
      setTitlePreferenceLoadFailed(false);
      setTitleError(null);
    } catch {
      setTitleError('titlePreferenceSaveFailed');
    } finally {
      setTitleSaving(false);
    }
  };

  useEffect(() => {
    if (titlePreference.mode !== 'lightweight') return;
    const lightweightModel = findLightweightModel(models);
    const nextModel = lightweightModel ? titlePreferenceForModel(lightweightModel) : null;
    const currentRoute = titlePreference.model ? formatModelRouteKey(titlePreference.model) : null;
    const nextRoute = nextModel ? formatModelRouteKey(nextModel) : null;
    if (currentRoute === nextRoute) return;
    const nextPreference: TitleGenerationPreference = { mode: 'lightweight', model: nextModel };
    try {
      writeTitleGenerationPreference(nextPreference);
      setTitlePreference(nextPreference);
      setTitlePreferenceLoadFailed(false);
      setTitleError(null);
    } catch {
      setTitleError('titlePreferenceSaveFailed');
    }
  }, [models, titlePreference.mode, titlePreference.model]);

  const selectedConnection = connections.find((connection) => connection.id === selectedId);
  const selectedWorkspace = selectedConnection?.workspaces.find((workspace) => workspace.isSelected);

  return (
    <section aria-labelledby="settings-connections-heading" className="grid gap-6">
      <div>
        <h2 className="text-xl font-semibold leading-7" id="settings-connections-heading">{t('settingsAccountConnectionsHeading')}</h2>
      </div>

      <Card className="overflow-hidden">
        <div className="flex flex-wrap items-start justify-between gap-4 p-5">
          <div className="flex min-w-0 items-start gap-3">
            <ProviderIcon className="mt-0.5" providerId="chatgpt" size={28} />
            <div className="min-w-0">
              <h3 className="font-semibold">{t('chatGptProvider')}</h3>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => setChatGptApiKeyFormOpen((current) => !current)} variant="secondary">
              <KeyRound aria-hidden="true" size={16} />
              {t('apiKey')}
            </Button>
            <Button disabled={request === 'signingIn'} onClick={() => void signIn()} variant="primary">
              {request === 'signingIn' ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : <LogIn aria-hidden="true" size={16} />}
              {t(request === 'signingIn' ? 'oauthSigningIn' : 'oauth')}
            </Button>
          </div>
        </div>

        {request === 'signingIn' ? (
          <div className="mx-5 mt-4 flex flex-wrap items-center justify-between gap-3 rounded-control border border-border bg-raised px-3 py-2">
            <p aria-live="polite" className="text-sm text-secondary-text">{t('oauthBrowserWaiting')}</p>
            <Button disabled={oauthOperation === null || isCancellingOAuth} onClick={() => void cancelOAuth()} size="small" variant="secondary">
              {isCancellingOAuth ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : null}
              {t('cancel')}
            </Button>
          </div>
        ) : null}

        {request === 'loading' ? (
          <div className="border-t border-border px-5 py-5">
            <div aria-label={t('modelsLoading')} className="h-0.5 animate-pulse rounded-full bg-accent" />
          </div>
        ) : request === 'error' ? (
          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-5 py-4">
            <p className="text-sm text-secondary-text">{t('oauthConnectionsLoadFailed')}</p>
            <Button onClick={() => void loadConnections()} size="small" variant="secondary">
              <RefreshCw aria-hidden="true" size={15} />
              {t('retry')}
            </Button>
          </div>
        ) : connections.length > 0 ? (
          <ul className="grid gap-3 border-t border-border px-5 py-3">
            {connections.map((connection) => (
              <ConnectionRow
                connection={connection}
                isRemoving={removingId === connection.id}
                isSigningIn={request === 'signingIn'}
                key={connection.id}
                onRemove={() => setRemoveTarget(connection)}
                onSelect={() => void selectConnection(connection.id)}
                onWorkspaceChanged={(workspaceId) => void selectWorkspace(connection.id, workspaceId)}
                selected={connection.id === selectedId}
              />
            ))}
          </ul>
        ) : (
          <p className="border-t border-border px-5 py-5 text-sm text-secondary-text">{t('noChatGptConnections')}</p>
        )}

        {request !== 'loading' && request !== 'error' && selectedConnection && selectedWorkspace ? (
          <div className="px-5 pb-3">
            <ProviderVerificationStatus
              requestOutcome={{
                record: lastRequestOutcome,
                readFailed: lastRequestReadFailed,
                saveFailed: false,
              }}
              showQuotaUnavailable={false}
              value={{
                checkedAtUnixMs: verificationCheckedAt,
                recordReadFailed: verificationRecordReadFailed,
                saveFailed: verificationSaveFailed,
                state: verificationState,
              }}
            />
          </div>
        ) : null}

        {errorCode && request !== 'error' ? <p className="px-5 pb-4 pt-1 text-sm text-destructive" role="alert">{t(errorCode)}</p> : null}
        {notice ? <p className="px-5 pb-4 pt-1 text-sm text-success" role="status">{t(notice)}</p> : null}

        <ApiKeyProvidersSection
          chatGptApiKeyFormOpen={chatGptApiKeyFormOpen}
          mode="chatgpt"
          onChanged={refreshModels}
          onChatGptApiKeyFormToggle={() => setChatGptApiKeyFormOpen((current) => !current)}
        />
      </Card>

      <section aria-labelledby="title-generation-heading" className="grid gap-3">
        <div>
          <h3 className="text-base font-semibold" id="title-generation-heading">{t('titleGenerationTarget')}</h3>
          <p className="mt-1 text-sm text-secondary-text">{t('titleGenerationTargetDescription')}</p>
        </div>
        <TitleGenerationSettings
          modelStatus={modelStatus}
          modelError={modelError}
          models={models}
          onChange={saveTitlePreference}
          onPreferenceRetry={loadTitlePreference}
          onRetry={() => void refreshModels(true)}
          error={titleError}
          preferenceLoadFailed={titlePreferenceLoadFailed}
          preference={titlePreference}
          saving={titleSaving}
        />
      </section>

      <section aria-labelledby="settings-model-providers-heading" className="grid gap-4">
        <h3 className="text-base font-semibold" id="settings-model-providers-heading">{t('settingsModelProvidersHeading')}</h3>
        <ApiKeyProvidersSection mode="modelProviders" onChanged={refreshModels} />
      </section>

      <Dialog
        onOpenChange={(open) => { if (!open && removingId === null) setRemoveTarget(null); }}
        open={removeTarget !== null}
        title={t('removeChatGptConnection')}
      >
        {removeTarget ? (
          <div className="grid gap-4">
            <p className="text-sm text-secondary-text">
              {t('confirmRemoveConnection', { name: removeTarget.displayName ?? removeTarget.email ?? removeTarget.id })}
            </p>
            <div className="flex justify-end gap-2">
              <Button disabled={removingId !== null} onClick={() => setRemoveTarget(null)}>{t('cancel')}</Button>
              <Button disabled={removingId !== null} onClick={() => void removeConnection()} variant="destructive">
                {removingId !== null ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : <Trash2 aria-hidden="true" size={16} />}
                {t('removeConnectionAction')}
              </Button>
            </div>
          </div>
        ) : null}
      </Dialog>
    </section>
  );
}

interface ConnectionRowProps {
  connection: AccountConnection;
  isRemoving: boolean;
  isSigningIn: boolean;
  onRemove(): void;
  onSelect(): void;
  onWorkspaceChanged(workspaceId: string): void;
  selected: boolean;
}

function ConnectionRow({
  connection,
  isRemoving,
  isSigningIn,
  onRemove,
  onSelect,
  onWorkspaceChanged,
  selected,
}: ConnectionRowProps) {
  const { t } = useTranslation();
  const label = connection.email ?? t('accountEmailUnavailable');
  const selectedWorkspace = connection.workspaces.find((workspace) => workspace.isSelected);

  return (
    <li className={`rounded-card border p-3 ${selected ? 'border-accent bg-selected' : 'border-border bg-raised'}`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <CircleUserRound aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={20} />
          <div className="min-w-0">
            <p className="truncate font-medium">{label}</p>
            <p className="mt-1 truncate text-sm text-secondary-text">
              {connection.authStatus === 'active'
                ? t('accountPlan', { plan: connection.planType ?? t('planUnavailable') })
                : t('connectionNeedsSignIn')}
            </p>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {selected ? (
            <span className="flex items-center gap-1 text-sm text-accent"><CircleCheck aria-hidden="true" size={16} />{t('connectionSelected')}</span>
          ) : (
            <Button disabled={isRemoving || isSigningIn} onClick={onSelect} size="small" variant="secondary">{t('useConnection')}</Button>
          )}
          <Button aria-label={t('removeChatGptConnection')} disabled={isRemoving || isSigningIn} onClick={onRemove} size="icon" variant="ghost">
            {isRemoving ? <LoaderCircle aria-hidden="true" className="animate-spin" size={16} /> : <Trash2 aria-hidden="true" size={16} />}
          </Button>
        </div>
      </div>
      {selected && connection.workspaces.length > 1 ? (
        <div className="ml-7 mt-3 max-w-sm">
          <Select
            label={t('selectWorkspace')}
            onChange={(event) => onWorkspaceChanged(event.target.value)}
            options={connection.workspaces.map((workspace) => ({
              value: workspace.id,
              label: workspace.displayName ?? t('workspaceWithoutName'),
            }))}
            visuallyHiddenLabel
            value={selectedWorkspace?.id ?? ''}
          />
        </div>
      ) : selected && connection.workspaces.length === 0 ? (
        <p className="ml-7 mt-2 text-sm text-secondary-text">{t('workspaceUnavailable')}</p>
      ) : null}
    </li>
  );
}

interface TitleGenerationSettingsProps {
  error: 'titlePreferenceSaveFailed' | null;
  modelError: boolean;
  modelStatus: string;
  models: ChatModel[];
  onChange(preference: TitleGenerationPreference): void;
  onPreferenceRetry(): void;
  onRetry(): void;
  preferenceLoadFailed: boolean;
  preference: TitleGenerationPreference;
  saving: boolean;
}

function TitleGenerationSettings({
  error,
  modelError,
  modelStatus,
  models,
  onChange,
  onPreferenceRetry,
  onRetry,
  preferenceLoadFailed,
  preference,
  saving,
}: TitleGenerationSettingsProps) {
  const { t } = useTranslation();
  const providerLabel = (providerId: string): string => {
    switch (providerId) {
      case 'chatgpt':
      case 'chatgpt_api': return t('chatGptProvider');
      case 'opencode': return t('openCodeProvider');
      case 'gemini': return t('geminiProvider');
      case 'groq': return t('groqProvider');
      case 'cerebras': return t('cerebrasProvider');
      case 'openrouter': return t('openRouterProvider');
      case 'mistral': return t('mistralProvider');
      case 'deepseek': return t('deepSeekProvider');
      case 'xai': return t('xAiProvider');
      case 'perplexity': return t('perplexityProvider');
      case 'together': return t('togetherProvider');
      case 'sambanova': return t('sambaNovaProvider');
      case 'anthropic': return t('anthropicProvider');
      case 'fireworks': return t('fireworksProvider');
      case 'deepinfra': return t('deepInfraProvider');
      case 'dashscope': return t('dashScopeProvider');
      case 'cohere': return t('cohereProvider');
      case 'llama_cpp': return 'llama.cpp';
      default:
        if (providerId.startsWith('custom_')) return t('customProvidersHeading');
        return providerId;
    }
  };
  const selectedPreference = preference.model;
  const selectedModel = selectedPreference === null
    ? null
    : models.find((model) => modelRouteKey(model) === formatModelRouteKey(selectedPreference)) ?? null;
  const lightweightModel = findLightweightModel(models);
  const modelOptions: Array<{ value: string; label: string; disabled?: boolean }> = models.map((model) => ({
    value: modelRouteKey(model),
    label: `${model.sourceLabel ?? providerLabel(model.providerId)} · ${model.displayName}`,
  }));
  if (preference.model !== null && selectedModel === null) {
    modelOptions.push({
      value: formatModelRouteKey(preference.model),
      label: preference.model.displayName,
      disabled: true,
    });
  }
  const selectedModelValue = selectedModel
    ? modelRouteKey(selectedModel)
    : preference.model
      ? formatModelRouteKey(preference.model)
      : '';

  const selectMode = (mode: TitleGenerationMode) => {
    if (saving) return;
    if (mode === 'conversation') {
      onChange({ mode, model: null });
      return;
    }
    if (mode === 'lightweight') {
      if (!lightweightModel) return;
      onChange({ mode, model: titlePreferenceForModel(lightweightModel) });
      return;
    }
    const model = selectedModel ?? models[0];
    if (model) onChange({ mode, model: titlePreferenceForModel(model) });
  };

  return (
    <Card className="grid gap-2 p-4">
      {preferenceLoadFailed ? (
        <div className="flex flex-wrap items-center justify-between gap-3 px-3 py-2" role="alert">
          <p className="text-sm text-secondary-text">{t('titlePreferenceLoadFailed')}</p>
          <Button onClick={onPreferenceRetry} size="small" variant="ghost">
            <RefreshCw aria-hidden="true" size={15} />
            {t('retry')}
          </Button>
        </div>
      ) : (modelStatus === 'loading' || modelStatus === 'idle') && models.length === 0 ? (
        <div aria-live="polite" className="flex items-center gap-3 px-3 py-2">
          <LoaderCircle aria-hidden="true" className="animate-spin text-accent-icon" size={16} />
          <p className="text-sm text-secondary-text">{t('modelsLoading')}</p>
        </div>
      ) : modelError && models.length === 0 ? (
        <div className="flex flex-wrap items-center justify-between gap-3 px-3 py-2" role="alert">
          <p className="text-sm text-secondary-text">{t('modelCatalogUnavailable')}</p>
          <Button onClick={onRetry} size="small" variant="ghost">
            <RefreshCw aria-hidden="true" size={15} />
            {t('retry')}
          </Button>
        </div>
      ) : (
        <>
          <TitleModeButton
            description={t('titleConversationModeDescription')}
            disabled={saving}
            icon={<MessagesSquare aria-hidden="true" size={18} />}
            onClick={() => selectMode('conversation')}
            selected={preference.mode === 'conversation'}
            title={t('titleUseConversationModel')}
          />
          <TitleModeButton
            description={t('titleSpecificModelDescription')}
            disabled={saving || models.length === 0}
            icon={<ListFilter aria-hidden="true" size={18} />}
            onClick={() => selectMode('selectedModel')}
            selected={preference.mode === 'selectedModel'}
            title={t('titleUseSpecificModel')}
          />
          <TitleModeButton
            description={t('titleLightweightModeDescription')}
            disabled={saving || lightweightModel === null}
            icon={<Sparkles aria-hidden="true" size={18} />}
            onClick={() => selectMode('lightweight')}
            selected={preference.mode === 'lightweight'}
            title={t('titleUseLightweightModel')}
          />
          {modelError ? <p className="px-3 py-2 text-sm text-warning" role="status">{t('modelCatalogSourcesUnavailable')}</p> : null}
          {preference.mode === 'selectedModel' ? (
            <div className="mt-2 rounded-card border border-border bg-raised p-3">
              <div className="flex min-w-0 items-end gap-2">
                {selectedModel ? (
                  <ProviderIcon
                    className="mb-2 shrink-0"
                    providerId={selectedModel.providerId === 'chatgpt_api' ? 'chatgpt' : selectedModel.providerId}
                    size={16}
                  />
                ) : null}
                <div className="min-w-0 flex-1">
                  <Select
                    disabled={saving || models.length === 0}
                    label={t('titleSelectModelHint')}
                    onChange={(event) => {
                      const model = models.find((item) => modelRouteKey(item) === event.target.value);
                      if (model) onChange({ mode: 'selectedModel', model: titlePreferenceForModel(model) });
                    }}
                    options={modelOptions}
                    value={selectedModelValue}
                  />
                </div>
              </div>
            </div>
          ) : null}
          {preference.mode === 'lightweight' ? (
            <p className={`mt-2 flex items-start gap-2 rounded-card border p-3 text-sm ${lightweightModel ? 'border-success/30 bg-success/10 text-secondary-text' : 'border-warning/30 bg-warning/10 text-secondary-text'}`}>
              {lightweightModel ? <CircleCheck aria-hidden="true" className="mt-0.5 shrink-0 text-success" size={16} /> : <CircleAlert aria-hidden="true" className="mt-0.5 shrink-0 text-warning" size={16} />}
              <span>{lightweightModel ? t('titleLightweightModelSelected', { model: lightweightModel.displayName }) : t('titleLightweightUnavailable')}</span>
            </p>
          ) : null}
          {saving ? <div aria-label={t('saving')} className="mt-2 h-0.5 animate-pulse rounded-full bg-accent" /> : null}
          {error ? <p className="px-3 text-sm text-destructive" role="alert">{t(error)}</p> : null}
        </>
      )}
    </Card>
  );
}

interface TitleModeButtonProps {
  description: string;
  disabled: boolean;
  icon: ReactNode;
  onClick(): void;
  selected: boolean;
  title: string;
}

function TitleModeButton({ description, disabled, icon, onClick, selected, title }: TitleModeButtonProps) {
  return (
    <button
      aria-pressed={selected}
      className={`flex w-full items-start gap-3 rounded-button border px-3 py-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50 ${selected ? 'border-accent bg-selected' : 'border-transparent hover:bg-hover'}`}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      <span className={`mt-0.5 shrink-0 ${selected ? 'text-accent' : 'text-secondary-icon'}`}>{icon}</span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm font-medium">{title}</span>
        <span className="mt-1 block text-xs leading-5 text-secondary-text">{description}</span>
      </span>
      {selected ? <CircleCheck aria-hidden="true" className="mt-0.5 shrink-0 text-accent" size={18} /> : <Circle aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={15} />}
    </button>
  );
}

function findLightweightModel(models: readonly ChatModel[]): ChatModel | null {
  const candidates = models
    .map((model) => ({ model, parameterCount: parameterCountBillions(model) }))
    .filter((candidate): candidate is { model: ChatModel; parameterCount: number } => candidate.parameterCount !== null)
    .sort((left, right) => left.parameterCount - right.parameterCount);
  return candidates[0]?.model ?? null;
}

function parameterCountBillions(model: ChatModel): number | null {
  const pattern = /(?:^|[-_:./\s])(\d+(?:\.\d+)?)b(?:$|[-_:./\s])/iu;
  for (const value of [model.modelId, model.displayName]) {
    const match = pattern.exec(value);
    const count = Number(match?.[1]);
    if (Number.isFinite(count) && count > 0) return count;
  }
  return null;
}

function parseConnection(value: unknown): AccountConnection[] {
  if (!isObject(value) || typeof value.id !== 'string' || typeof value.authStatus !== 'string' || !Array.isArray(value.workspaces)) return [];
  const workspaces = value.workspaces.flatMap(parseWorkspace);
  return [{
    id: value.id,
    authStatus: value.authStatus,
    displayName: optionalString(value.displayName),
    email: optionalString(value.email),
    planType: optionalString(value.planType),
    isSelected: value.isSelected === true,
    workspaces,
  }];
}

function parseWorkspace(value: unknown): AccountWorkspace[] {
  if (!isObject(value) || typeof value.id !== 'string') return [];
  return [{
    id: value.id,
    displayName: optionalString(value.displayName),
    planType: optionalString(value.planType),
    isSelected: value.isSelected === true,
  }];
}

function optionalString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

async function cancelOAuthAfterUnmount(operation: ServiceOperation): Promise<void> {
  try {
    const cancelled = await operation.cancel();
    if (!cancelled) globalThis.reportError(new Error('OpenChat could not cancel a pending OAuth sign-in after leaving settings.'));
  } catch {
    globalThis.reportError(new Error('OpenChat could not cancel a pending OAuth sign-in after leaving settings.'));
  }
}
