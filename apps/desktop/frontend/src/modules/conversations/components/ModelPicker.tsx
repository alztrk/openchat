import type { TFunction } from 'i18next';
import { Bot, Brain, ChevronDown, LoaderCircle, LogIn, LogOut, Search, Star, Text } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Select } from '@/design-system';
import type { ChatModelFavorite } from '@/modules/conversations/data/chatStorage';
import { favoriteSourceConnection } from '@/modules/conversations/stores/chatStoreUtils';
import { ProviderIcon } from '@/modules/providers/components/ProviderIcon';
import type { ChatModel, ChatModelProvider } from '@/modules/providers/data/chatModels';
import { modelRouteKey } from '@/modules/providers/data/modelPreferences';
import {
  type CachedModelPricing,
  isModelPricingCacheFresh,
  loadModelPricingBatch,
  type ModelPricingSnapshot,
  modelPricingKey,
  readModelPricingCache,
  writeModelPricingCache,
} from '@/modules/providers/data/modelPricing';
import { localServiceClient } from '@/platform/local-service';
import { useTransitionPresence } from '@/shared/hooks/useTransitionPresence';
import { FastModeControl } from './FastModeControl';

export interface ModelPickerProps {
  models: readonly ChatModel[];
  knownModels: readonly ChatModel[];
  modelProviders: readonly ChatModelProvider[];
  selectedModel: ChatModel | null;
  modelFavorites: readonly ChatModelFavorite[];
  hiddenModelKeys: readonly string[];
  canSelectProvider: boolean;
  showFastModeControl: boolean;
  fastModeAvailable: boolean;
  fastModeEnabled: boolean;
  fastModeLoading: boolean;
  fastModeSaving: boolean;
  showReasoningSelector: boolean;
  reasoningDisabled: boolean;
  reasoningEffort: string | null;
  reasoningLevels: readonly string[];
  emptyModelsLabel: string;
  disabled: boolean;
  isOpen: boolean;
  query: string;
  onOpenChange(open: boolean): void;
  onQueryChange(query: string): void;
  onSelect(model: ChatModel): void;
  onSelectFavorite(favorite: ChatModelFavorite): Promise<boolean>;
  onSelectProvider(providerId: string): void;
  onFastModeToggle(): void;
  onSelectReasoningEffort(value: string | null): void;
  onToggleFavorite(model: ChatModel): void;
  onRemoveFavorite(favorite: ChatModelFavorite): void;
}

export function ModelPicker({
  disabled,
  isOpen,
  canSelectProvider,
  showFastModeControl,
  fastModeAvailable,
  fastModeEnabled,
  fastModeLoading,
  fastModeSaving,
  showReasoningSelector,
  reasoningDisabled,
  reasoningEffort,
  reasoningLevels,
  emptyModelsLabel,
  hiddenModelKeys,
  knownModels,
  modelFavorites,
  modelProviders,
  models,
  onOpenChange,
  onQueryChange,
  onRemoveFavorite,
  onSelectFavorite,
  onFastModeToggle,
  onSelect,
  onSelectReasoningEffort,
  onSelectProvider,
  onToggleFavorite,
  query,
  selectedModel,
}: ModelPickerProps) {
  const { t } = useTranslation();
  const [selectedProviderId, setSelectedProviderId] = useState(() =>
    providerFamily(selectedModel?.providerId ?? 'opencode'),
  );
  const [showFavorites, setShowFavorites] = useState(false);
  const [selectingFavoriteKey, setSelectingFavoriteKey] = useState<string | null>(null);
  const [pricingByModelKey, setPricingByModelKey] = useState<Record<string, PricingLoadState>>({});
  const isPresent = useTransitionPresence(isOpen);
  const rootRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const pricingRequested = useRef(new Set<string>());
  const isMounted = useRef(true);
  useEffect(() => {
    isMounted.current = true;
    return () => {
      isMounted.current = false;
    };
  }, []);
  useEffect(() => {
    if (!isOpen) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && !rootRef.current?.contains(event.target)) onOpenChange(false);
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    return () => document.removeEventListener('pointerdown', closeOnOutsidePointer);
  }, [isOpen, onOpenChange]);
  useEffect(() => {
    if (!isOpen) return;
    const frame = window.requestAnimationFrame(() => searchInputRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, [isOpen]);
  const selectedModelProviderId = selectedModel?.providerId;
  useEffect(() => {
    if (selectedModelProviderId) setSelectedProviderId(providerFamily(selectedModelProviderId));
  }, [selectedModelProviderId]);
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const providerSource = modelProviders.find((provider) => providerFamily(provider.providerId) === selectedProviderId);
  const providerOptions = buildProviderOptions(modelProviders, models);
  const filteredModels = useMemo(
    () =>
      models.filter(
        (model) =>
          providerFamily(model.providerId) === selectedProviderId && matchesModelQuery(model, normalizedQuery, t),
      ),
    [models, normalizedQuery, selectedProviderId, t],
  );
  const groups = groupModels(filteredModels, selectedProviderId, modelFavorites, t);
  const availableFavorites = useMemo(
    () => modelFavorites.filter((favorite) => !isFavoriteHidden(favorite, knownModels, hiddenModelKeys)),
    [hiddenModelKeys, knownModels, modelFavorites],
  );
  const visibleFavorites = useMemo(
    () => availableFavorites.filter((favorite) => matchesFavoriteQuery(favorite, normalizedQuery, modelProviders, t)),
    [availableFavorites, modelProviders, normalizedQuery, t],
  );
  const pricingModels = useMemo(
    () =>
      showFavorites
        ? visibleFavorites.flatMap((favorite) => {
            const model = knownModels.find((candidate) => matchesFavoriteModel(candidate, favorite));
            return model ? [model] : [];
          })
        : filteredModels,
    [filteredModels, knownModels, showFavorites, visibleFavorites],
  );

  useEffect(() => {
    if (!isOpen) return;
    const loadVisiblePricing = async () => {
      let cache: Map<string, CachedModelPricing>;
      try {
        cache = readModelPricingCache();
      } catch (cause) {
        globalThis.reportError(new Error('The model pricing cache could not be read.', { cause }));
        cache = new Map();
      }
      const pendingModels: ChatModel[] = [];
      const cachedStates: Array<[string, PricingLoadState]> = [];
      for (const model of pricingModels) {
        if (model.providerId === 'llama_cpp' || pricingRequested.current.has(model.key)) continue;
        const key = modelPricingKey(model.providerId, model.modelId);
        const cached = cache.get(key);
        if (cached) {
          cachedStates.push([
            model.key,
            {
              status: isModelPricingCacheFresh(cached) ? 'ready' : 'loading',
              snapshot: cached.snapshot,
            },
          ]);
        }
        if (cached && isModelPricingCacheFresh(cached)) {
          pricingRequested.current.add(model.key);
          continue;
        }
        pendingModels.push(model);
        pricingRequested.current.add(model.key);
        if (!cached) cachedStates.push([model.key, { status: 'loading', snapshot: null }]);
      }
      if (cachedStates.length > 0) {
        setPricingByModelKey((current) => ({ ...current, ...Object.fromEntries(cachedStates) }));
      }
      if (pendingModels.length === 0) return;
      try {
        const snapshots = await loadModelPricingBatch(localServiceClient, pendingModels);
        const cachedAtUnixMs = Date.now();
        const updates = pendingModels.map((model) => {
          const snapshot = snapshots.get(modelPricingKey(model.providerId, model.modelId));
          if (!snapshot) throw new Error('A requested model pricing snapshot is missing.');
          cache.set(modelPricingKey(model.providerId, model.modelId), { cachedAtUnixMs, snapshot });
          if (snapshot.model === null || snapshot.model.pricing === null) {
            pricingRequested.current.delete(model.key);
          }
          return [model.key, { status: 'ready' as const, snapshot }] as const;
        });
        try {
          writeModelPricingCache(cache);
        } catch (cause) {
          globalThis.reportError(new Error('The model pricing cache could not be saved.', { cause }));
        }
        if (!isMounted.current) return;
        setPricingByModelKey((current) => ({ ...current, ...Object.fromEntries(updates) }));
      } catch (cause) {
        if (!isMounted.current) return;
        for (const model of pendingModels) pricingRequested.current.delete(model.key);
        globalThis.reportError(new Error('Model pricing could not be loaded.', { cause }));
        setPricingByModelKey((current) => ({
          ...current,
          ...Object.fromEntries(
            pendingModels.map((model) => {
              const cached = cache.get(modelPricingKey(model.providerId, model.modelId));
              return [model.key, { status: 'error' as const, snapshot: cached?.snapshot ?? null }];
            }),
          ),
        }));
      }
    };
    void loadVisiblePricing();
  }, [isOpen, pricingModels]);
  const startFavoriteSelection = async (favorite: ChatModelFavorite) => {
    const key = favoriteKey(favorite);
    setSelectingFavoriteKey(key);
    try {
      const selected = await onSelectFavorite(favorite);
      if (!selected) return;
      setSelectedProviderId(providerFamily(favorite.providerId));
      rootRef.current?.querySelector<HTMLButtonElement>(':scope > button')?.focus();
      onOpenChange(false);
    } finally {
      setSelectingFavoriteKey(null);
    }
  };
  const selectedLabel = selectedModel
    ? selectedModel.displayName
    : models.length > 0
      ? t('modelPickerSelectModel')
      : emptyModelsLabel;
  return (
    <div className="relative min-w-0 max-w-full" ref={rootRef}>
      <Button
        aria-expanded={isOpen}
        aria-haspopup="dialog"
        className="chat-control-trigger max-w-full justify-start"
        disabled={disabled}
        onClick={() => onOpenChange(!isOpen)}
        type="button"
        variant="secondary"
      >
        {selectedModel ? (
          <ProviderIcon className="shrink-0" providerId={selectedModel.providerId} size={16} />
        ) : (
          <Bot aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
        )}
        <span className="max-w-48 truncate text-sm">{selectedLabel}</span>
        <ChevronDown aria-hidden="true" className="shrink-0" size={15} />
      </Button>
      {isPresent ? (
        <div
          aria-hidden={!isOpen}
          aria-label={t('connectedProvidersModels')}
          className="openchat-popover-enter openchat-popover-enter--bottom model-picker-popover absolute bottom-full left-0 z-20 mb-2 flex overflow-hidden rounded-card border border-border bg-raised shadow-lg"
          data-state={isOpen ? 'open' : 'closed'}
          inert={!isOpen}
          role="dialog"
        >
          <nav
            aria-label={t('connectedProvidersModels')}
            className="model-picker-providers openchat-scroll-area grid content-start gap-1 overflow-y-auto border-r border-border bg-navigation p-2"
          >
            <button
              aria-pressed={showFavorites}
              className={`flex min-h-10 min-w-0 items-center gap-2 rounded-control px-2.5 text-left text-sm font-medium ${showFavorites ? 'bg-selected text-text' : 'text-secondary-text hover:bg-hover'}`}
              onClick={() => {
                setShowFavorites(true);
                onQueryChange('');
              }}
              type="button"
            >
              <Star aria-hidden="true" className="shrink-0" size={17} />
              <span className="truncate">{t('favoriteModels')}</span>
            </button>
            {providerOptions.map((provider) => {
              const selected = !showFavorites && provider.providerId === selectedProviderId;
              return (
                <button
                  aria-pressed={selected}
                  className={`flex min-h-10 min-w-0 items-center gap-2 rounded-control px-2.5 text-left text-sm font-medium ${selected ? 'bg-selected text-text' : provider.available ? 'text-secondary-text hover:bg-hover' : 'cursor-not-allowed text-secondary-text opacity-50'}`}
                  disabled={!provider.available || !canSelectProvider}
                  key={provider.providerId}
                  onClick={() => {
                    setShowFavorites(false);
                    setSelectedProviderId(provider.providerId);
                    onQueryChange('');
                    onSelectProvider(provider.providerId);
                  }}
                  type="button"
                >
                  <ProviderIcon className="shrink-0" providerId={provider.providerId} size={17} />
                  <span className="truncate">{provider.name ?? providerNameForId(provider.providerId, t)}</span>
                </button>
              );
            })}
          </nav>
          <section
            aria-label={
              showFavorites ? t('favoriteModels') : (providerSource?.name ?? providerNameForId(selectedProviderId, t))
            }
            className="model-picker-models flex min-w-0 flex-1 flex-col p-3"
          >
            <div className="flex min-w-0 items-center gap-1">
              {showReasoningSelector ? (
                <div className="model-picker-reasoning relative min-w-0 shrink-0">
                  <Brain
                    aria-hidden="true"
                    className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-secondary-icon"
                    size={16}
                  />
                  <Select
                    aria-label={t('reasoning')}
                    className="py-2 pl-8 pr-1 text-xs"
                    disabled={reasoningDisabled}
                    label={t('reasoning')}
                    onChange={(event) => onSelectReasoningEffort(event.currentTarget.value || null)}
                    options={[
                      { value: '', label: t('reasoningDefault') },
                      ...reasoningLevels.map((level) => ({ value: level, label: reasoningLabel(level, t) })),
                    ]}
                    value={reasoningEffort ?? ''}
                    visuallyHiddenLabel
                  />
                </div>
              ) : null}
              <div className="relative min-w-0 flex-1">
                <Search
                  aria-hidden="true"
                  className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-secondary-icon"
                  size={16}
                />
                <input
                  aria-label={t('searchModels')}
                  className="min-h-10 w-full rounded-control border border-control-border bg-surface py-2 pl-9 pr-3 text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  onChange={(event) => onQueryChange(event.currentTarget.value)}
                  placeholder={t('modelSearchHint')}
                  ref={searchInputRef}
                  type="search"
                  value={query}
                />
              </div>
              {!showFavorites && selectedProviderId === 'chatgpt' && showFastModeControl ? (
                <FastModeControl
                  available={fastModeAvailable}
                  enabled={fastModeEnabled}
                  loading={fastModeLoading}
                  saving={fastModeSaving}
                  onToggle={onFastModeToggle}
                />
              ) : null}
            </div>
            <div className="openchat-scroll-area mt-2 min-h-0 flex-1 overflow-y-auto">
              {showFavorites ? (
                visibleFavorites.length > 0 ? (
                  <div className="grid gap-1">
                    {visibleFavorites.map((favorite) => {
                      const model = knownModels.find((candidate) => matchesFavoriteModel(candidate, favorite)) ?? null;
                      const favoriteModelProviderName = model
                        ? providerNameForModel(model, t)
                        : (modelProviders.find(
                            (provider) => providerFamily(provider.providerId) === providerFamily(favorite.providerId),
                          )?.name ?? providerNameForId(favorite.providerId, t));
                      const favoriteIsSelected = model?.key === selectedModel?.key;
                      const selectionKey = favoriteKey(favorite);
                      return model ? (
                        <ModelOption
                          isFavorite
                          isSelected={favoriteIsSelected}
                          key={selectionKey}
                          model={model}
                          onSelect={() => {
                            setSelectedProviderId(providerFamily(model.providerId));
                            onSelect(model);
                            rootRef.current?.querySelector<HTMLButtonElement>(':scope > button')?.focus();
                          }}
                          onRemoveFavorite={() => onRemoveFavorite(favorite)}
                          pricingLoad={pricingByModelKey[model.key]}
                        />
                      ) : (
                        <div
                          className="flex min-w-0 items-center gap-1 rounded-control hover:bg-hover"
                          key={selectionKey}
                        >
                          <button
                            className="flex min-h-10 min-w-0 flex-1 items-center gap-2 px-2 text-left text-sm text-text"
                            disabled={selectingFavoriteKey === selectionKey}
                            onClick={() => void startFavoriteSelection(favorite)}
                            type="button"
                          >
                            {selectingFavoriteKey === selectionKey ? (
                              <LoaderCircle
                                aria-hidden="true"
                                className="shrink-0 animate-spin motion-reduce:animate-none"
                                size={15}
                              />
                            ) : (
                              <ProviderIcon className="shrink-0" providerId={favorite.providerId} size={17} />
                            )}
                            <span className="min-w-0 flex-1 truncate">
                              <span className="block truncate">{favorite.displayName}</span>
                              <span className="block truncate text-xs text-secondary-text">
                                {favoriteModelProviderName}
                              </span>
                            </span>
                          </button>
                          <Button
                            aria-label={t('removeModelFavorite')}
                            className="size-8 shrink-0"
                            onClick={() => onRemoveFavorite(favorite)}
                            size="icon"
                            variant="ghost"
                          >
                            <Star aria-hidden="true" fill="currentColor" size={15} />
                          </Button>
                        </div>
                      );
                    })}
                  </div>
                ) : (
                  <p className="px-2 py-3 text-sm text-secondary-text">
                    {availableFavorites.length === 0 ? t('favoriteModelsEmpty') : t('searchMessagesNoResults')}
                  </p>
                )
              ) : filteredModels.length > 0 ? (
                groups.map((group) => (
                  <div className="grid gap-1" key={group.id}>
                    {group.label ? (
                      <p className="px-2 pb-1 pt-2 text-xs font-semibold text-secondary-text">{group.label}</p>
                    ) : null}
                    {group.models.map((model) => {
                      const isFavorite = modelFavorites.some((favorite) => matchesFavoriteModel(model, favorite));
                      return (
                        <ModelOption
                          isFavorite={isFavorite}
                          isSelected={model.key === selectedModel?.key}
                          key={model.key}
                          model={model}
                          onSelect={() => {
                            onSelect(model);
                            rootRef.current?.querySelector<HTMLButtonElement>(':scope > button')?.focus();
                          }}
                          onRemoveFavorite={() => onToggleFavorite(model)}
                          pricingLoad={pricingByModelKey[model.key]}
                        />
                      );
                    })}
                  </div>
                ))
              ) : (
                <p className="px-2 py-3 text-sm text-secondary-text">
                  {models.some((model) => providerFamily(model.providerId) === selectedProviderId)
                    ? t('searchMessagesNoResults')
                    : t('noModelsAvailable')}
                </p>
              )}
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}

interface ModelOptionProps {
  model: ChatModel;
  isSelected: boolean;
  isFavorite: boolean;
  pricingLoad: PricingLoadState | undefined;
  onSelect(): void;
  onRemoveFavorite(): void;
}

function ModelOption({ isFavorite, isSelected, model, onRemoveFavorite, onSelect, pricingLoad }: ModelOptionProps) {
  const { i18n, t } = useTranslation();
  const pricing = pricingLoad?.snapshot?.model?.pricing;
  const inputPrice = pricing ? `$${formatPriceAmount(pricing.input, i18n.language)}` : '—';
  const outputPrice = pricing ? `$${formatPriceAmount(pricing.output, i18n.language)}` : '—';
  const contextWindow = pricingLoad?.snapshot?.model?.contextWindow ?? model.contextWindow;
  const contextLabel = contextWindow && contextWindow > 0 ? formatModelTokenCount(contextWindow) : '—';
  const isPricingLoading = model.providerId !== 'llama_cpp' && pricingLoad?.snapshot === null;
  return (
    <div className="flex min-w-0 items-center gap-1 rounded-control hover:bg-hover">
      <button
        aria-pressed={isSelected}
        className="flex min-h-14 min-w-0 flex-1 items-center gap-2.5 rounded-control px-2 text-left text-sm text-text"
        onClick={onSelect}
        type="button"
      >
        <ProviderIcon className="shrink-0" providerId={model.providerId} size={17} />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="block truncate">{model.displayName}</span>
          <span
            className="inline-flex min-w-0 items-center gap-1.5 truncate text-xs text-secondary-text"
            role={isPricingLoading ? 'status' : undefined}
            title={pricing ? t('modelDetailsPricingSource') : undefined}
          >
            {isPricingLoading ? (
              <>
                <span className="inline-flex items-center gap-0.5">
                  <LogIn aria-hidden="true" className="text-secondary-icon" size={12} />
                  <span
                    aria-hidden="true"
                    className="h-3 w-8 animate-pulse rounded bg-selected motion-reduce:animate-none"
                  />
                </span>
                <span aria-hidden="true">/</span>
                <span className="inline-flex items-center gap-0.5">
                  <LogOut aria-hidden="true" className="text-secondary-icon" size={12} />
                  <span
                    aria-hidden="true"
                    className="h-3 w-8 animate-pulse rounded bg-selected motion-reduce:animate-none"
                  />
                </span>
              </>
            ) : (
              <>
                <span className="inline-flex items-center gap-0.5">
                  <LogIn aria-hidden="true" className="text-secondary-icon" size={12} />
                  {inputPrice}
                </span>
                <span aria-hidden="true">/</span>
                <span className="inline-flex items-center gap-0.5">
                  <LogOut aria-hidden="true" className="text-secondary-icon" size={12} />
                  {outputPrice}
                </span>
              </>
            )}
            <span aria-hidden="true" className="opacity-50">
              ·
            </span>
            <span className="inline-flex items-center gap-0.5">
              <Text aria-hidden="true" className="text-secondary-icon" size={12} />
              {contextLabel}
            </span>
          </span>
        </span>
      </button>
      <Button
        aria-label={t(isFavorite ? 'removeModelFavorite' : 'addModelFavorite')}
        className="size-8 shrink-0"
        onClick={onRemoveFavorite}
        size="icon"
        variant="ghost"
      >
        <Star aria-hidden="true" fill={isFavorite ? 'currentColor' : 'none'} size={15} />
      </Button>
    </div>
  );
}

interface ProviderOption {
  providerId: string;
  name: string | null;
  available: boolean;
}

const providerOrder = [
  'chatgpt',
  'opencode',
  'kilo',
  'gemini',
  'groq',
  'cerebras',
  'openrouter',
  'mistral',
  'deepseek',
  'xai',
  'perplexity',
  'together',
  'sambanova',
  'anthropic',
  'fireworks',
  'deepinfra',
  'dashscope',
  'cohere',
  'llama_cpp',
];

function buildProviderOptions(sources: readonly ChatModelProvider[], models: readonly ChatModel[]): ProviderOption[] {
  const knownProviders = new Set(providerOrder);
  const sourcesByFamily = new Map<string, ChatModelProvider>();
  for (const source of sources) {
    const family = providerFamily(source.providerId);
    const current = sourcesByFamily.get(family);
    sourcesByFamily.set(family, {
      providerId: family,
      name: source.name ?? current?.name ?? null,
      available: source.available || current?.available === true,
    });
  }
  const customProviderIds = new Set(
    [
      ...sources.map((source) => providerFamily(source.providerId)),
      ...models.map((model) => providerFamily(model.providerId)),
    ].filter((providerId) => !knownProviders.has(providerId)),
  );
  const orderedIds = [...providerOrder, ...[...customProviderIds].sort((left, right) => left.localeCompare(right))];
  return orderedIds.map((providerId) => {
    const source = sourcesByFamily.get(providerId);
    const modelAvailable = models.some((model) => providerFamily(model.providerId) === providerId);
    return {
      providerId,
      name:
        source?.name ??
        models.find((model) => providerFamily(model.providerId) === providerId)?.customProvider?.name ??
        null,
      available: source?.available ?? (providerId === 'opencode' || providerId === 'kilo' || providerId === 'llama_cpp' || modelAvailable),
    };
  });
}

function providerFamily(providerId: string): string {
  return providerId === 'chatgpt_api' ? 'chatgpt' : providerId;
}

function matchesModelQuery(model: ChatModel, normalizedQuery: string, t: (key: TranslationKey) => string): boolean {
  return (
    normalizedQuery.length === 0 ||
    [model.displayName, model.modelId, model.sourceLabel ?? '', providerNameForModel(model, t)].some((value) =>
      value.toLocaleLowerCase().includes(normalizedQuery),
    )
  );
}

function matchesFavoriteModel(model: ChatModel, favorite: ChatModelFavorite): boolean {
  return (
    model.providerId === favorite.providerId &&
    model.modelId === favorite.modelId &&
    favoriteSourceConnection(model) === favorite.sourceConnectionId
  );
}

function isFavoriteHidden(
  favorite: ChatModelFavorite,
  models: readonly ChatModel[],
  hiddenModelKeys: readonly string[],
): boolean {
  const model = models.find((candidate) => matchesFavoriteModel(candidate, favorite));
  if (model) return hiddenModelKeys.includes(modelRouteKey(model));
  return hiddenModelKeys.some(
    (key) =>
      key === `${favorite.providerId}:::${favorite.modelId}` ||
      key === `${favorite.providerId}:${favorite.sourceConnectionId ?? ''}::${favorite.modelId}` ||
      key === `${favorite.providerId}:${favorite.modelId}`,
  );
}

function matchesFavoriteQuery(
  favorite: ChatModelFavorite,
  normalizedQuery: string,
  providers: readonly ChatModelProvider[],
  t: (key: TranslationKey) => string,
): boolean {
  return (
    normalizedQuery.length === 0 ||
    [
      favorite.displayName,
      favorite.modelId,
      providers.find((provider) => providerFamily(provider.providerId) === providerFamily(favorite.providerId))?.name ??
        providerNameForId(favorite.providerId, t),
      favorite.sourceConnectionId ?? '',
    ].some((value) => value.toLocaleLowerCase().includes(normalizedQuery))
  );
}

function favoriteKey(favorite: ChatModelFavorite): string {
  return JSON.stringify([favorite.providerId, favorite.sourceConnectionId, favorite.modelId]);
}

function formatPriceAmount(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 6 }).format(value);
}

function formatModelTokenCount(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toLocaleString(undefined, { maximumFractionDigits: 1 })}M`;
  if (value >= 1_000) return `${Math.round(value / 1_000).toLocaleString()}K`;
  return value.toLocaleString();
}

const reasoningTranslationKeys: Partial<Record<string, TranslationKey>> = {
  minimal: 'reasoningMinimal',
  low: 'reasoningLow',
  medium: 'reasoningMedium',
  high: 'reasoningHigh',
  xhigh: 'reasoningExtraHigh',
  extra_high: 'reasoningExtraHigh',
  'extra-high': 'reasoningExtraHigh',
  max: 'reasoningMax',
  maximum: 'reasoningMax',
  ultra: 'reasoningUltra',
};

function reasoningLabel(level: string, t: (key: TranslationKey) => string): string {
  const key = reasoningTranslationKeys[level.toLocaleLowerCase('en')];
  return key ? t(key) : level;
}

function groupModels(
  models: readonly ChatModel[],
  providerId: string,
  favorites: readonly { providerId: string; modelId: string; sourceConnectionId: string | null }[],
  t: TFunction,
): Array<{ id: string; label: string | null; models: ChatModel[] }> {
  const sections = new Map<string, ChatModel[]>();
  for (const model of models) {
    const group = sections.get(model.groupId ?? 'models') ?? [];
    group.push(model);
    sections.set(model.groupId ?? 'models', group);
  }
  const managedServerSections = [...sections.keys()]
    .filter((section) => section.startsWith('managed-llama-server:'))
    .sort(compareLlamaServerSections);
  const externalServerSections = [...sections.keys()]
    .filter((section) => section.startsWith('external-llama-server:'))
    .sort(compareLlamaServerSections);
  const order =
    providerId === 'opencode'
      ? ['free', 'paid', 'models']
      : providerId === 'openrouter' || providerId === 'kilo'
        ? ['free', 'models']
        : providerId === 'chatgpt'
          ? ['api', 'oauth', 'models']
          : providerId === 'llama_cpp'
            ? ['managed', ...managedServerSections, ...externalServerSections, 'models']
            : ['models'];

  return order.flatMap((section) => {
    const modelsInSection = sections.get(section);
    if (!modelsInSection?.length) return [];
    return [
      {
        id: section,
        label: modelSectionLabel(providerId, section, t),
        models: [...modelsInSection].sort((left, right) => {
          const leftFavorite = favorites.some(
            (favorite) =>
              favorite.providerId === left.providerId &&
              favorite.modelId === left.modelId &&
              favorite.sourceConnectionId === favoriteSourceConnection(left),
          );
          const rightFavorite = favorites.some(
            (favorite) =>
              favorite.providerId === right.providerId &&
              favorite.modelId === right.modelId &&
              favorite.sourceConnectionId === favoriteSourceConnection(right),
          );
          if (leftFavorite !== rightFavorite) return leftFavorite ? -1 : 1;
          return left.displayName.localeCompare(right.displayName);
        }),
      },
    ];
  });
}

function modelSectionLabel(providerId: string, section: string, t: TFunction): string | null {
  if (providerId === 'llama_cpp' && section === 'managed') {
    return t('localEngineManagedModelSection');
  }
  if (providerId === 'llama_cpp' && section.startsWith('managed-llama-server:')) {
    const port = Number(section.split(':').at(-1));
    return Number.isInteger(port) ? t('localEngineManagedServerModelSection', { port }) : null;
  }
  if (providerId === 'llama_cpp' && section.startsWith('external-llama-server:')) {
    const port = Number(section.split(':').at(-1));
    return Number.isInteger(port) ? t('localEngineExternalModelSection', { port }) : null;
  }
  const key: Partial<Record<string, TranslationKey>> = {
    api: 'modelSourceApi',
    oauth: 'modelSourceOAuth',
    free: 'openCodeFreeModels',
    paid: 'openCodeApiModels',
  };
  const translationKey = key[section];
  return translationKey ? t(translationKey) : null;
}

function compareLlamaServerSections(left: string, right: string): number {
  const leftPort = Number(left.split(':').at(-1));
  const rightPort = Number(right.split(':').at(-1));
  const portComparison = (Number.isInteger(leftPort) ? leftPort : 0) - (Number.isInteger(rightPort) ? rightPort : 0);
  return portComparison === 0 ? left.localeCompare(right) : portComparison;
}

type TranslationKey = keyof typeof import('@/localization/locales/en.json');

type PricingLoadState = {
  status: 'loading' | 'ready' | 'error';
  snapshot: ModelPricingSnapshot | null;
};

const providerTranslationKeys: Partial<Record<string, TranslationKey>> = {
  opencode: 'openCodeProvider',
  kilo: 'providerNameKilo',
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

function providerNameForModel(model: ChatModel, t: (key: TranslationKey) => string): string {
  if (model.customProvider?.name) return model.customProvider.name;
  const key = providerTranslationKeys[model.providerId];
  return key ? t(key) : model.providerId;
}

function providerNameForId(providerId: string, t: (key: TranslationKey) => string): string {
  const key = providerTranslationKeys[providerId];
  return key ? t(key) : providerId;
}
