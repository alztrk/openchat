import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ChatModel } from '@entities/model';
import { modelRouteKey } from '@entities/model/modelPreferences';
import { useChatStore, favoriteSourceConnection } from '@features/chat/state';
import { ModelPicker } from './ModelPicker';
import type { ChatModelFavorite } from '@features/chat/data';

interface ModelPickerFieldProps {
  disabled?: boolean;
  emptyOptionLabel?: string;
  emptySelectionLabel?: string;
  models?: readonly ChatModel[];
  onSelect(model: ChatModel): void;
  onSelectEmpty?(): void;
  selectedModel: ChatModel | null;
  triggerAriaDescribedBy?: string;
  triggerAriaLabel: string;
}

export function ModelPickerField({
  disabled = false,
  emptyOptionLabel,
  emptySelectionLabel,
  models: allowedModels,
  onSelect,
  onSelectEmpty,
  selectedModel,
  triggerAriaDescribedBy,
  triggerAriaLabel,
}: ModelPickerFieldProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery] = useState('');
  const allModels = useChatStore((state) => state.models);
  const modelProviders = useChatStore((state) => state.modelProviders);
  const modelFavorites = useChatStore((state) => state.modelFavorites);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const chatGptWorkspaceSelectionRequired = useChatStore((state) => state.chatGptWorkspaceSelectionRequired);
  const toggleFavoriteModel = useChatStore((state) => state.toggleFavoriteModel);
  const removeModelFavorite = useChatStore((state) => state.removeModelFavorite);
  const sourceModels = allowedModels ?? allModels;
  const visibleModels = useMemo(
    () => sourceModels.filter((model) => !hiddenModelKeys.includes(modelRouteKey(model)) || model.key === selectedModel?.key),
    [hiddenModelKeys, selectedModel?.key, sourceModels],
  );
  const providerIds = useMemo(() => new Set(visibleModels.map((model) => model.providerId)), [visibleModels]);
  const visibleProviders = useMemo(
    () => modelProviders.filter((provider) => providerIds.has(provider.providerId)),
    [modelProviders, providerIds],
  );

  const selectFavorite = async (favorite: ChatModelFavorite): Promise<boolean> => {
    const model = visibleModels.find(
      (candidate) =>
        candidate.providerId === favorite.providerId &&
        candidate.modelId === favorite.modelId &&
        favoriteSourceConnection(candidate) === favorite.sourceConnectionId,
    );
    if (!model) return false;
    onSelect(model);
    return true;
  };

  return (
    <ModelPicker
      canSelectProvider
      chatGptWorkspaceSelectionRequired={chatGptWorkspaceSelectionRequired}
      disabled={disabled || visibleModels.length === 0}
      emptyModelsLabel={t('noModelsAvailable')}
      emptyOptionLabel={emptyOptionLabel}
      emptySelectionLabel={emptySelectionLabel}
      fastModeAvailable={false}
      fastModeEnabled={false}
      fastModeLoading={false}
      fastModeSaving={false}
      hiddenModelKeys={hiddenModelKeys}
      isOpen={isOpen}
      knownModels={visibleModels}
      modelFavorites={modelFavorites}
      modelProviders={visibleProviders}
      models={visibleModels}
      onFastModeToggle={() => undefined}
      onOpenChange={setIsOpen}
      onQueryChange={setQuery}
      onRemoveFavorite={(favorite) => { void removeModelFavorite(favorite); }}
      onSelect={(model) => {
        onSelect(model);
        setIsOpen(false);
      }}
      onSelectFavorite={selectFavorite}
      onSelectEmpty={onSelectEmpty}
      onSelectProvider={() => setQuery('')}
      onToggleFavorite={(model) => { void toggleFavoriteModel(model); }}
      query={query}
      selectedModel={selectedModel}
      showFastModeControl={false}
      triggerAriaDescribedBy={triggerAriaDescribedBy}
      triggerAriaLabel={triggerAriaLabel}
    />
  );
}
