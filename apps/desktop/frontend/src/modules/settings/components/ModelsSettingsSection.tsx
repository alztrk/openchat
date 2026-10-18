import { Eye, EyeOff, LoaderCircle, RefreshCw, Star, StarOff } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Input } from '@/design-system';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { ProviderIcon } from '@/modules/providers/components/ProviderIcon';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { formatModelRouteKey, modelRouteKey } from '@/modules/providers/data/modelPreferences';

type FeedbackKey =
  | 'defaultModelUpdated'
  | 'defaultModelCleared'
  | 'modelHidden'
  | 'modelUnhidden'
  | 'defaultModelPreferenceClearFailed'
  | 'hiddenModelPreferencesClearFailed'
  | 'modelPreferencesSaveFailed'
  | 'chatHistoryUnavailable';

type TranslationKey = keyof typeof import('@/localization/locales/en.json');

const providerTranslationKeys: Record<string, TranslationKey> = {
  opencode: 'openCodeProvider',
  copilot: 'providerNameCopilot',
  chatgpt: 'chatGptProvider',
  chatgpt_api: 'chatGptProvider',
  gemini: 'geminiProvider',
  groq: 'groqProvider',
  cerebras: 'cerebrasProvider',
  openrouter: 'openRouterProvider',
  mistral: 'mistralProvider',
  deepseek: 'deepSeekProvider',
  xai: 'xAiProvider',
  perplexity: 'perplexityProvider',
  together: 'togetherProvider',
  sambanova: 'sambaNovaProvider',
  anthropic: 'anthropicProvider',
  fireworks: 'fireworksProvider',
  deepinfra: 'deepInfraProvider',
  dashscope: 'dashScopeProvider',
  cohere: 'cohereProvider',
  llama_cpp: 'localEngines',
};

export function ModelsSettingsSection() {
  const { t } = useTranslation();
  const models = useChatStore((state) => state.models);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelError = useChatStore((state) => state.modelError);
  const modelFavorites = useChatStore((state) => state.modelFavorites);
  const favoriteStatus = useChatStore((state) => state.favoriteStatus);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const defaultModel = useChatStore((state) => state.defaultModel);
  const modelContextLimits = useChatStore((state) => state.modelContextLimits);
  const defaultModelPreferenceInvalid = useChatStore((state) => state.defaultModelPreferenceInvalid);
  const hiddenModelPreferencesInvalid = useChatStore((state) => state.hiddenModelPreferencesInvalid);
  const modelContextLimitsInvalid = useChatStore((state) => state.modelContextLimitsInvalid);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const refreshModelFavorites = useChatStore((state) => state.refreshModelFavorites);
  const setDefaultModel = useChatStore((state) => state.setDefaultModel);
  const clearDefaultModel = useChatStore((state) => state.clearDefaultModel);
  const clearInvalidDefaultModel = useChatStore((state) => state.clearInvalidDefaultModel);
  const toggleHiddenModel = useChatStore((state) => state.toggleHiddenModel);
  const clearHiddenModelPreferences = useChatStore((state) => state.clearHiddenModelPreferences);
  const setModelContextLimit = useChatStore((state) => state.setModelContextLimit);
  const clearModelContextPreferences = useChatStore((state) => state.clearModelContextPreferences);
  const toggleFavoriteModel = useChatStore((state) => state.toggleFavoriteModel);
  const [query, setQuery] = useState('');
  const [contextModel, setContextModel] = useState<ChatModel | null>(null);
  const [contextDraft, setContextDraft] = useState('');
  const [contextError, setContextError] = useState<string | null>(null);
  const [feedbackKey, setFeedbackKey] = useState<FeedbackKey | null>(null);
  const [feedbackModelName, setFeedbackModelName] = useState('');
  const [busyFavoriteKeys, setBusyFavoriteKeys] = useState<Set<string>>(() => new Set());

  const groups = useMemo(() => {
    const search = query.trim().toLocaleLowerCase();
    const grouped = new Map<string, ChatModel[]>();
    for (const model of models) {
      if (
        search &&
        !model.displayName.toLocaleLowerCase().includes(search) &&
        !model.modelId.toLocaleLowerCase().includes(search) &&
        !(model.sourceLabel?.toLocaleLowerCase().includes(search) ?? false)
      ) {
        continue;
      }
      const group = grouped.get(model.providerId) ?? [];
      group.push(model);
      grouped.set(model.providerId, group);
    }
    return [...grouped.entries()];
  }, [models, query]);

  const changeDefault = (model: ChatModel) => {
    setFeedbackModelName(model.displayName);
    setFeedbackKey(setDefaultModel(model) ? 'defaultModelUpdated' : 'modelPreferencesSaveFailed');
  };

  const removeDefault = () => {
    setFeedbackModelName('');
    setFeedbackKey(clearDefaultModel() ? 'defaultModelCleared' : 'defaultModelPreferenceClearFailed');
  };

  const toggleHidden = (model: ChatModel) => {
    const isHidden = hiddenModelKeys.includes(modelRouteKey(model));
    setFeedbackModelName(model.displayName);
    if (!toggleHiddenModel(model)) {
      setFeedbackKey('modelPreferencesSaveFailed');
      return;
    }
    setFeedbackKey(isHidden ? 'modelUnhidden' : 'modelHidden');
  };

  const toggleFavorite = async (model: ChatModel) => {
    const key = modelRouteKey(model);
    setBusyFavoriteKeys((current) => new Set(current).add(key));
    setFeedbackKey(null);
    const saved = await toggleFavoriteModel(model);
    if (!saved) setFeedbackKey('chatHistoryUnavailable');
    setBusyFavoriteKeys((current) => {
      const next = new Set(current);
      next.delete(key);
      return next;
    });
  };

  const openContextLimit = (model: ChatModel) => {
    const savedLimit = modelContextLimits[modelRouteKey(model)];
    setContextDraft(savedLimit === undefined ? '' : String(savedLimit));
    setContextError(null);
    setContextModel(model);
  };

  const saveContextLimit = (limit: number | null) => {
    if (!contextModel) return;
    const saved = setModelContextLimit(modelRouteKey(contextModel), limit);
    if (!saved) {
      setContextError(t('modelPreferencesSaveFailed'));
      return;
    }
    setContextModel(null);
    setFeedbackKey(null);
  };

  const submitContextLimit = () => {
    const value = Number(contextDraft.trim());
    const maximum = contextModel?.contextWindow ?? null;
    if (!/^\d+$/.test(contextDraft.trim()) || !Number.isSafeInteger(value) || value < 2048 || value > 2_000_000) {
      setContextError(t('modelContextInvalid'));
      return;
    }
    if (maximum !== null && value > maximum) {
      setContextError(t('modelContextAboveMaximum'));
      return;
    }
    saveContextLimit(value);
  };

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-6">
      <p className="max-w-3xl text-sm text-secondary-text">{t('modelsDescription')}</p>

      {defaultModelPreferenceInvalid ? (
        <PreferenceNotice
          action={t('clearDefaultModelPreference')}
          message={t('defaultModelPreferenceInvalid')}
          onAction={() =>
            setFeedbackKey(clearInvalidDefaultModel() ? 'defaultModelCleared' : 'defaultModelPreferenceClearFailed')
          }
        />
      ) : null}
      {hiddenModelPreferencesInvalid ? (
        <PreferenceNotice
          action={t('clearHiddenModelPreferences')}
          message={t('hiddenModelPreferencesInvalid')}
          onAction={() => setFeedbackKey(clearHiddenModelPreferences() ? null : 'hiddenModelPreferencesClearFailed')}
        />
      ) : null}
      {modelContextLimitsInvalid ? (
        <PreferenceNotice
          action={t('clearModelContextPreferences')}
          message={t('modelContextPreferencesInvalid')}
          onAction={() => setFeedbackKey(clearModelContextPreferences() ? null : 'modelPreferencesSaveFailed')}
        />
      ) : null}
      {modelError ? (
        <p className="text-sm text-warning" role="status">
          {t('modelCatalogUnavailable')}
        </p>
      ) : null}
      {favoriteStatus === 'error' ? (
        <div
          className="flex flex-wrap items-center justify-between gap-3 rounded-card border border-border bg-surface px-4 py-3"
          role="alert"
        >
          <p className="text-sm text-destructive">{t('chatHistoryUnavailable')}</p>
          <Button onClick={() => void refreshModelFavorites()} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : null}
      {feedbackKey ? (
        <p
          aria-live="polite"
          className={
            feedbackKey === 'modelPreferencesSaveFailed' || feedbackKey === 'chatHistoryUnavailable'
              ? 'text-sm text-destructive'
              : 'text-sm text-success'
          }
          role="status"
        >
          {t(feedbackKey, { model: feedbackModelName })}
        </p>
      ) : null}

      <div className="flex flex-col gap-3 compact:flex-row compact:items-end">
        <div className="min-w-0 compact:flex-1">
          <Input
            aria-label={t('searchModels')}
            label={t('searchModels')}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('modelSearchHint')}
            type="search"
            value={query}
          />
        </div>
        <Button
          aria-label={t('refreshModels')}
          className="compact:shrink-0"
          disabled={modelStatus === 'loading'}
          onClick={() => void refreshModels(true)}
          variant="secondary"
        >
          {modelStatus === 'loading' ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          ) : (
            <RefreshCw aria-hidden="true" size={16} />
          )}
          {t('refreshModels')}
        </Button>
      </div>

      {modelStatus === 'loading' && models.length === 0 ? (
        <p aria-live="polite" className="py-6 text-sm text-secondary-text">
          {t('modelsLoading')}
        </p>
      ) : modelStatus === 'error' ? (
        <Card className="flex flex-wrap items-center justify-between gap-3 p-5" role="alert">
          <p className="text-sm text-destructive">{t('modelCatalogUnavailable')}</p>
          <Button onClick={() => void refreshModels(true)} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </Card>
      ) : groups.length === 0 ? (
        <Card className="p-6 text-center text-sm text-secondary-text">
          {query.trim() ? t('noModelsFound') : t('noConnectedProviders')}
        </Card>
      ) : (
        <div className="grid gap-4">
          {groups.map(([providerId, providerModels]) => (
            <Card className="overflow-hidden p-0" key={providerId}>
              <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
                <div className="flex min-w-0 items-center gap-2.5">
                  <ProviderIcon providerId={providerId} size={20} />
                  <h3 className="truncate font-semibold">
                    {t(providerTranslationKeys[providerId] ?? 'connectedProvidersModels')}
                  </h3>
                </div>
                <span className="text-xs text-secondary-text">{providerModels.length}</span>
              </div>
              <ul className="divide-y divide-border">
                {providerModels.map((model) => {
                  const routeKey = modelRouteKey(model);
                  const hidden = hiddenModelKeys.includes(routeKey);
                  const isDefault = defaultModel !== null && formatModelRouteKey(defaultModel) === routeKey;
                  const isFavorite = modelFavorites.some(
                    (favorite) =>
                      favorite.providerId === model.providerId &&
                      favorite.modelId === model.modelId &&
                      favorite.sourceConnectionId === (model.apiKeyConnectionId ?? null),
                  );
                  const favoriteBusy = busyFavoriteKeys.has(routeKey);
                  const contextLimit = modelContextLimits[routeKey];
                  return (
                    <li className="flex flex-wrap items-start justify-between gap-3 px-4 py-3" key={routeKey}>
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className={`font-medium ${hidden ? 'text-secondary-text line-through' : 'text-text'}`}>
                            {model.displayName}
                          </span>
                          {isDefault ? (
                            <span className="rounded-control border border-accent px-2 py-0.5 text-xs text-accent">
                              {t('defaultModel')}
                            </span>
                          ) : null}
                          {hidden ? (
                            <span className="rounded-control border border-border px-2 py-0.5 text-xs text-secondary-text">
                              {t('hiddenModel')}
                            </span>
                          ) : null}
                          {model.groupId === 'free' ? (
                            <span className="rounded-control border border-border px-2 py-0.5 text-xs text-secondary-text">
                              {t('openCodeFreeModel')}
                            </span>
                          ) : null}
                          {model.sourceLabel ? (
                            <span className="rounded-control border border-border px-2 py-0.5 text-xs text-secondary-text">
                              {model.sourceLabel}
                            </span>
                          ) : null}
                          {model.contextWindow !== null ? (
                            <span className="text-xs text-secondary-text">
                              {t('modelContextWindow', { value: formatTokenCount(model.contextWindow) })}
                            </span>
                          ) : null}
                        </div>
                        <p className="mt-1 break-all font-mono text-xs text-secondary-text">{model.modelId}</p>
                        <Button
                          className="mt-1 min-h-8 px-2 text-xs"
                          onClick={() => openContextLimit(model)}
                          size="small"
                          variant="ghost"
                        >
                          {contextLimit === undefined
                            ? t('modelContextAutomatic')
                            : t('modelContextCustom', { count: formatTokenCount(contextLimit) })}
                        </Button>
                      </div>
                      <div className="flex flex-wrap items-center gap-1">
                        <Button
                          aria-label={t(isFavorite ? 'removeModelFavorite' : 'addModelFavorite')}
                          disabled={favoriteStatus !== 'ready' || favoriteBusy}
                          onClick={() => void toggleFavorite(model)}
                          size="icon"
                          variant="ghost"
                        >
                          {favoriteBusy ? (
                            <LoaderCircle aria-hidden="true" className="animate-spin" size={17} />
                          ) : isFavorite ? (
                            <Star aria-hidden="true" className="text-accent" fill="currentColor" size={17} />
                          ) : (
                            <StarOff aria-hidden="true" size={17} />
                          )}
                        </Button>
                        {isDefault ? (
                          <Button onClick={removeDefault} size="small" variant="secondary">
                            {t('defaultModel')}
                          </Button>
                        ) : (
                          <Button onClick={() => changeDefault(model)} size="small" variant="ghost">
                            {t('setDefaultModel')}
                          </Button>
                        )}
                        <Button
                          aria-label={t(hidden ? 'showModel' : 'hideModel')}
                          onClick={() => toggleHidden(model)}
                          size="icon"
                          variant="ghost"
                        >
                          {hidden ? <Eye aria-hidden="true" size={17} /> : <EyeOff aria-hidden="true" size={17} />}
                        </Button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            </Card>
          ))}
        </div>
      )}

      <Dialog
        open={contextModel !== null}
        onOpenChange={(open) => {
          if (!open) setContextModel(null);
        }}
        title={t('modelContextTitle', { model: contextModel?.displayName ?? '' })}
      >
        <div className="grid gap-4">
          <p className="text-sm text-secondary-text">
            {t('modelContextMaximum', {
              limit:
                contextModel?.contextWindow === null || contextModel?.contextWindow === undefined
                  ? t('contextUsageNoModelLimit')
                  : formatTokenCount(contextModel.contextWindow),
            })}
          </p>
          <Input
            autoFocus
            error={contextError ?? undefined}
            label={t('modelContextCustomLabel')}
            max={2_000_000}
            min={2048}
            onChange={(event) => {
              setContextDraft(event.target.value);
              setContextError(null);
            }}
            placeholder={t('modelContextAutomatic')}
            type="number"
            value={contextDraft}
          />
          <p className="text-sm text-secondary-text">{t('modelContextExplanation')}</p>
          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={() => setContextModel(null)} variant="ghost">
              {t('cancel')}
            </Button>
            <Button onClick={() => saveContextLimit(null)} variant="secondary">
              {t('modelContextAutomatic')}
            </Button>
            <Button onClick={submitContextLimit} variant="primary">
              {t('save')}
            </Button>
          </div>
        </div>
      </Dialog>
    </section>
  );
}

interface PreferenceNoticeProps {
  action: string;
  message: string;
  onAction(): void;
}

function PreferenceNotice({ action, message, onAction }: PreferenceNoticeProps) {
  return (
    <div
      className="flex flex-wrap items-center justify-between gap-3 rounded-card border border-border bg-surface px-4 py-3"
      role="alert"
    >
      <p className="max-w-3xl text-sm text-warning">{message}</p>
      <Button onClick={onAction} size="small" variant="secondary">
        {action}
      </Button>
    </div>
  );
}

function formatTokenCount(value: number): string {
  return value >= 1000 ? `${Math.round(value / 1000)}K` : String(value);
}
