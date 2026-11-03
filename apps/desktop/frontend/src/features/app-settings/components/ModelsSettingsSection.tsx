import {
  Brain,
  Check,
  Eye,
  EyeOff,
  LoaderCircle,
  MoreHorizontal,
  RefreshCw,
  SlidersHorizontal,
  Star,
  StarOff,
} from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  Button,
  Card,
  Dialog,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
  Input,
} from '@shared/components';
import { reasoningLevelLabel } from '@entities/model/composerUtils';
import { useChatStore } from '@app/runtime/chatStore';
import { ProviderIcon } from '@shared/components';
import { SettingsSectionSkeleton } from './SettingsSectionSkeleton';
import type { ChatModel } from '@entities/model';
import { formatModelRouteKey, modelRouteKey } from '@entities/model/modelPreferences';

type FeedbackKey =
  | 'defaultModelUpdated'
  | 'defaultModelCleared'
  | 'modelHidden'
  | 'modelUnhidden'
  | 'defaultModelPreferenceClearFailed'
  | 'hiddenModelPreferencesClearFailed'
  | 'modelPreferencesSaveFailed'
  | 'chatHistoryUnavailable';

type TranslationKey = keyof typeof import('@shared/lib/i18n/locales/en.json');

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
  const modelReasoningEfforts = useChatStore((state) => state.modelReasoningEfforts);
  const defaultModelPreferenceInvalid = useChatStore((state) => state.defaultModelPreferenceInvalid);
  const hiddenModelPreferencesInvalid = useChatStore((state) => state.hiddenModelPreferencesInvalid);
  const modelContextLimitsInvalid = useChatStore((state) => state.modelContextLimitsInvalid);
  const modelReasoningEffortsInvalid = useChatStore((state) => state.modelReasoningEffortsInvalid);
  const refreshModels = useChatStore((state) => state.refreshModels);
  const refreshModelFavorites = useChatStore((state) => state.refreshModelFavorites);
  const setDefaultModel = useChatStore((state) => state.setDefaultModel);
  const clearDefaultModel = useChatStore((state) => state.clearDefaultModel);
  const clearInvalidDefaultModel = useChatStore((state) => state.clearInvalidDefaultModel);
  const toggleHiddenModel = useChatStore((state) => state.toggleHiddenModel);
  const clearHiddenModelPreferences = useChatStore((state) => state.clearHiddenModelPreferences);
  const setModelContextLimit = useChatStore((state) => state.setModelContextLimit);
  const clearModelContextPreferences = useChatStore((state) => state.clearModelContextPreferences);
  const setModelReasoningEffort = useChatStore((state) => state.setModelReasoningEffort);
  const clearModelReasoningPreferences = useChatStore((state) => state.clearModelReasoningPreferences);
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
    <section aria-labelledby="settings-page-heading" className="settings-data-page">
      <div className="settings-data-group model-catalog-group">
        <h2 className="settings-data-group-title">{t('settingsModelCatalogTitle')}</h2>
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
      {modelReasoningEffortsInvalid ? (
        <PreferenceNotice
          action={t('clearModelReasoningPreferences')}
          message={t('modelReasoningPreferencesInvalid')}
          onAction={() => setFeedbackKey(clearModelReasoningPreferences() ? null : 'modelPreferencesSaveFailed')}
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

      <div className="model-catalog-toolbar">
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
          className="model-catalog-refresh"
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
        <SettingsSectionSkeleton label={t('modelsLoading')} />
      ) : modelStatus === 'error' ? (
        <Card className="flex flex-wrap items-center justify-between gap-3" padding="comfortable" role="alert">
          <p className="text-sm text-destructive">{t('modelCatalogUnavailable')}</p>
          <Button onClick={() => void refreshModels(true)} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </Card>
      ) : groups.length === 0 ? (
        <Card className="text-center text-sm text-secondary-text" padding="spacious">
          {query.trim() ? t('noModelsFound') : t('noConnectedProviders')}
        </Card>
      ) : (
        <div className="grid gap-4">
          {groups.map(([providerId, providerModels]) => (
            <Card className="model-provider-card overflow-hidden bg-raised" key={providerId}>
              <div className="model-provider-heading">
                <div className="flex min-w-0 items-center gap-2.5">
                  <ProviderIcon providerId={providerId} size={20} />
                  <h3 className="truncate font-semibold">
                    {t(providerTranslationKeys[providerId] ?? 'connectedProvidersModels')}
                  </h3>
                </div>
                <span className="text-xs text-secondary-text">{providerModels.length}</span>
              </div>
              <ul className="grid gap-2 p-3">
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
                  const reasoningEffort = modelReasoningEfforts[routeKey];
                  return (
                    <li className="model-catalog-row rounded-control border border-border bg-surface" key={routeKey}>
                      <div className="model-catalog-details">
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
                        <p className="mt-1 text-xs text-secondary-text">
                          {t('modelContextTitle', { model: model.displayName })}:{' '}
                          {contextLimit === undefined
                            ? t('modelContextAutomatic')
                            : t('modelContextCustom', { count: formatTokenCount(contextLimit) })}
                        </p>
                      </div>
                      <div className="model-catalog-actions">
                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <Button
                              aria-label={t('moreOptions')}
                              size="icon"
                              title={t('moreOptions')}
                              type="button"
                              variant="ghost"
                            >
                              <MoreHorizontal aria-hidden="true" size={18} />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end" aria-label={t('moreOptions')}>
                            <DropdownMenuItem onSelect={() => openContextLimit(model)}>
                              <SlidersHorizontal aria-hidden="true" size={14} />
                              <span>{t('modelContextCustomLabel')}</span>
                            </DropdownMenuItem>
                            {model.supportsReasoning ? (
                              <DropdownMenuSub>
                                <DropdownMenuSubTrigger>
                                  <Brain aria-hidden="true" size={14} />
                                  <span>{t('modelDefaultReasoning')}</span>
                                </DropdownMenuSubTrigger>
                                <DropdownMenuSubContent aria-label={t('modelDefaultReasoning')}>
                                  <DropdownMenuItem
                                    aria-checked={reasoningEffort === undefined}
                                    className="justify-between"
                                    onSelect={() => {
                                      const saved = setModelReasoningEffort(routeKey, null);
                                      setFeedbackKey(saved ? null : 'modelPreferencesSaveFailed');
                                    }}
                                    role="menuitemradio"
                                  >
                                    <span>{t('modelReasoningDefault')}</span>
                                    {reasoningEffort === undefined ? <Check aria-hidden="true" size={14} /> : null}
                                  </DropdownMenuItem>
                                  {model.reasoningLevels.map((level) => (
                                    <DropdownMenuItem
                                      aria-checked={reasoningEffort === level}
                                      className="justify-between"
                                      key={level}
                                      onSelect={() => {
                                        const saved = setModelReasoningEffort(routeKey, level);
                                        setFeedbackKey(saved ? null : 'modelPreferencesSaveFailed');
                                      }}
                                      role="menuitemradio"
                                    >
                                      <span>{reasoningLevelLabel(level, (key) => t(key))}</span>
                                      {reasoningEffort === level ? <Check aria-hidden="true" size={14} /> : null}
                                    </DropdownMenuItem>
                                  ))}
                                </DropdownMenuSubContent>
                              </DropdownMenuSub>
                            ) : null}
                            <DropdownMenuItem
                              aria-checked={isFavorite}
                              disabled={favoriteStatus !== 'ready' || favoriteBusy}
                              onSelect={() => void toggleFavorite(model)}
                              role="menuitemcheckbox"
                            >
                              {favoriteBusy ? (
                                <LoaderCircle aria-hidden="true" className="animate-spin" size={14} />
                              ) : isFavorite ? (
                                <Star aria-hidden="true" className="text-accent" fill="currentColor" size={14} />
                              ) : (
                                <StarOff aria-hidden="true" size={14} />
                              )}
                              <span>{t(isFavorite ? 'removeModelFavorite' : 'addModelFavorite')}</span>
                            </DropdownMenuItem>
                            <DropdownMenuItem onSelect={() => (isDefault ? removeDefault() : changeDefault(model))}>
                              <Star aria-hidden="true" size={14} />
                              <span>{t(isDefault ? 'defaultModel' : 'setDefaultModel')}</span>
                            </DropdownMenuItem>
                            <DropdownMenuItem onSelect={() => toggleHidden(model)}>
                              {hidden ? <Eye aria-hidden="true" size={14} /> : <EyeOff aria-hidden="true" size={14} />}
                              <span>{t(hidden ? 'showModel' : 'hideModel')}</span>
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </div>
                    </li>
                  );
                })}
              </ul>
            </Card>
          ))}
        </div>
      )}
      </div>

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
