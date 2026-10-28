import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Input, Select, Switch } from '@/design-system';
import { useDesignPreferencesStore } from '@/modules/design/data/designPreferences';
import { type ImageCollection, listImageCollections } from '@/modules/design/data/imageGeneration';
import { SettingsPreferenceRow } from './SettingsPreferenceRow';
import { SettingsLoadError, SettingsSaveFeedback } from './SettingsState';

export function DesignSettingsSection() {
  const { t } = useTranslation();
  const preferences = useDesignPreferencesStore((state) => state.preferences);
  const loadState = useDesignPreferencesStore((state) => state.loadState);
  const saveState = useDesignPreferencesStore((state) => state.saveState);
  const initialize = useDesignPreferencesStore((state) => state.initialize);
  const retry = useDesignPreferencesStore((state) => state.retry);
  const updatePreferences = useDesignPreferencesStore((state) => state.updatePreferences);
  const savePreset = useDesignPreferencesStore((state) => state.savePreset);
  const deletePreset = useDesignPreferencesStore((state) => state.deletePreset);
  const reset = useDesignPreferencesStore((state) => state.reset);
  const [presetName, setPresetName] = useState('');
  const [collections, setCollections] = useState<ImageCollection[]>([]);
  const [collectionsLoadFailed, setCollectionsLoadFailed] = useState(false);
  const savedCompression = preferences.defaultGenerationSettings.outputCompression;
  const [compressionDraft, setCompressionDraft] = useState(String(savedCompression));
  const [compressionInvalid, setCompressionInvalid] = useState(false);

  useEffect(() => {
    initialize();
    let active = true;
    void listImageCollections()
      .then((items) => {
        if (active) setCollections(items);
      })
      .catch(() => {
        if (active) setCollectionsLoadFailed(true);
      });
    return () => {
      active = false;
    };
  }, [initialize]);

  useEffect(() => {
    setCompressionDraft(String(savedCompression));
    setCompressionInvalid(false);
  }, [savedCompression]);

  if (loadState === 'error') {
    return (
      <SettingsLoadError
        message={t('designPreferencesLoadFailed')}
        onRetry={retry}
        onReset={() => void reset()}
        resetLabel={t('designPreferencesReset')}
        resetLoading={saveState === 'saving'}
      />
    );
  }

  const settings = preferences.defaultGenerationSettings;
  const updateSettings = <K extends keyof typeof settings>(key: K, value: (typeof settings)[K]) => {
    void updatePreferences({ defaultGenerationSettings: { ...settings, [key]: value } });
  };
  const commitCompression = () => {
    const trimmed = compressionDraft.trim();
    const value = Number(trimmed);
    if (trimmed.length === 0 || !Number.isInteger(value) || value < 0 || value > 100) {
      setCompressionInvalid(true);
      return;
    }
    void updateSettings('outputCompression', value);
    setCompressionInvalid(false);
  };
  const availablePresets = preferences.presets.map((preset) => ({ value: preset.id, label: preset.name }));
  if (
    preferences.defaultPresetId !== null &&
    !availablePresets.some((preset) => preset.value === preferences.defaultPresetId)
  ) {
    availablePresets.unshift({ value: preferences.defaultPresetId, label: t('designPresetUnavailable') });
  }
  const validDefaultCollectionId = collections.some((collection) => collection.id === preferences.defaultCollectionId)
    ? preferences.defaultCollectionId
    : null;

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      <fieldset className="contents" disabled={saveState === 'saving'}>
        <Card className="divide-y divide-border">
          <SettingsPreferenceRow
            description={t('designRememberLastGenerationDescription')}
            id="setting-design-remember-last-generation"
            label={t('designRememberLastGeneration')}
          >
            {(descriptionId) => (
              <Switch
                aria-label={t('designRememberLastGeneration')}
                aria-describedby={descriptionId}
                checked={preferences.rememberLastGeneration}
                disabled={loadState !== 'ready'}
                onCheckedChange={(value) => updatePreferences({ rememberLastGeneration: value })}
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designDefaultSizeDescription')}
            id="setting-design-size"
            label={t('designDefaultSize')}
          >
            {(descriptionId) => (
              <Select
                aria-describedby={descriptionId}
                label={t('designDefaultSize')}
                onChange={(event) => updateSettings('size', event.target.value)}
                options={[
                  { value: 'auto', label: t('designResolutionAuto') },
                  { value: '1024x1024', label: t('designSizeSquare1k') },
                  { value: '1536x1024', label: t('designSizeLandscape15k') },
                  { value: '1024x1536', label: t('designSizePortrait15k') },
                  { value: '2048x2048', label: t('designSizeSquare2k') },
                  { value: '2048x1152', label: t('designSizeLandscape2k') },
                  { value: '1152x2048', label: t('designSizePortrait2k') },
                  { value: '3840x2160', label: t('designSizeLandscape4k') },
                  { value: '2160x3840', label: t('designSizePortrait4k') },
                ]}
                value={settings.size}
                visuallyHiddenLabel
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designDefaultQualityDescription')}
            id="setting-design-quality"
            label={t('designQuality')}
          >
            {(descriptionId) => (
              <Select
                aria-describedby={descriptionId}
                label={t('designQuality')}
                onChange={(event) => updateSettings('quality', parseQuality(event.target.value))}
                options={[
                  { value: 'auto', label: t('designQualityAuto') },
                  { value: 'low', label: t('designQualityLow') },
                  { value: 'medium', label: t('designQualityMedium') },
                  { value: 'high', label: t('designQualityHigh') },
                  { value: 'xhigh', label: t('designQualityXhigh') },
                  { value: 'max', label: t('designQualityMax') },
                ]}
                value={settings.quality}
                visuallyHiddenLabel
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designDefaultBackgroundDescription')}
            id="setting-design-background"
            label={t('designBackground')}
          >
            {(descriptionId) => (
              <Select
                aria-describedby={descriptionId}
                label={t('designBackground')}
                onChange={(event) => updateSettings('background', parseBackground(event.target.value))}
                options={[
                  { value: 'auto', label: t('designBackgroundAuto') },
                  { value: 'opaque', label: t('designBackgroundOpaque') },
                  { value: 'transparent', label: t('designBackgroundTransparent') },
                ]}
                value={settings.background}
                visuallyHiddenLabel
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designDefaultOutputFormatDescription')}
            id="setting-design-output-format"
            label={t('designOutputFormat')}
          >
            {(descriptionId) => (
              <Select
                aria-describedby={descriptionId}
                label={t('designOutputFormat')}
                onChange={(event) => updateSettings('outputFormat', parseOutputFormat(event.target.value))}
                options={[
                  { value: 'png', label: 'PNG' },
                  { value: 'jpeg', label: 'JPEG' },
                  { value: 'webp', label: 'WebP' },
                ]}
                value={settings.outputFormat}
                visuallyHiddenLabel
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designDefaultCountDescription')}
            id="setting-design-image-count"
            label={t('designImageCount')}
          >
            {(descriptionId) => (
              <Select
                aria-describedby={descriptionId}
                label={t('designImageCount')}
                onChange={(event) => {
                  const count = Number(event.target.value);
                  if (Number.isInteger(count) && count >= 1 && count <= 10) updateSettings('count', count);
                }}
                options={[1, 2, 3, 4].map((count) => ({
                  value: String(count),
                  label: t('designImageCountOption', { count }),
                }))}
                value={String(settings.count)}
                visuallyHiddenLabel
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designDefaultCompressionDescription')}
            id="setting-design-compression"
            label={t('designCompression', { value: settings.outputCompression })}
          >
            {(descriptionId) => (
              <Input
                aria-label={t('designCompression', { value: settings.outputCompression })}
                aria-describedby={descriptionId}
                error={compressionInvalid ? t('designCompressionRangeError') : undefined}
                max={100}
                min={0}
                onBlur={commitCompression}
                onChange={(event) => {
                  setCompressionDraft(event.target.value);
                  setCompressionInvalid(false);
                }}
                type="number"
                value={compressionDraft}
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designDefaultModerationDescription')}
            id="setting-design-moderation"
            label={t('designModeration')}
          >
            {(descriptionId) => (
              <Select
                aria-describedby={descriptionId}
                label={t('designModeration')}
                onChange={(event) => updateSettings('moderation', event.target.value === 'low' ? 'low' : 'auto')}
                options={[
                  { value: 'auto', label: t('designModerationAuto') },
                  { value: 'low', label: t('designModerationLow') },
                ]}
                value={settings.moderation}
                visuallyHiddenLabel
              />
            )}
          </SettingsPreferenceRow>
        </Card>

        <Card className="grid gap-4" padding="default">
          <div className="grid gap-1">
            <h2 className="font-semibold">{t('designPresets')}</h2>
            <p className="text-sm text-secondary-text">{t('designPresetsDescription')}</p>
          </div>
          <div className="grid gap-3 @container">
            <SettingsPreferenceRow
              description={t('designDefaultPresetDescription')}
              id="setting-design-default-preset"
              label={t('designDefaultPreset')}
            >
              {(descriptionId) => (
                <Select
                  aria-describedby={descriptionId}
                  label={t('designDefaultPreset')}
                  onChange={(event) => updatePreferences({ defaultPresetId: event.target.value || null })}
                  options={[{ value: '', label: t('designUseDefaultSettings') }, ...availablePresets]}
                  value={preferences.defaultPresetId ?? ''}
                  visuallyHiddenLabel
                />
              )}
            </SettingsPreferenceRow>
            <div className="flex flex-wrap items-end gap-2">
              <Input
                className="min-w-48 flex-1"
                label={t('designPresetName')}
                maxLength={48}
                onChange={(event) => setPresetName(event.target.value)}
                value={presetName}
              />
              <Button
                disabled={preferences.presets.length >= 20 || presetName.trim().length === 0}
                onClick={() => {
                  void savePreset(presetName).then((saved) => {
                    if (saved) setPresetName('');
                  });
                }}
                size="small"
                variant="secondary"
              >
                {t('designSavePreset')}
              </Button>
            </div>
            {preferences.presets.map((preset) => (
              <div
                className="flex flex-wrap items-center justify-between gap-2 border-t border-border pt-3"
                key={preset.id}
              >
                <div>
                  <h3 className="font-medium">{preset.name}</h3>
                  <p className="text-sm text-secondary-text">
                    {t('designPresetSummary', {
                      size: preset.settings.size,
                      quality: t(qualityLabel(preset.settings.quality)),
                    })}
                  </p>
                </div>
                <Button onClick={() => deletePreset(preset.id)} size="small" variant="ghost">
                  {t('designDeleteAction')}
                </Button>
              </div>
            ))}
          </div>
        </Card>

        <Card className="divide-y divide-border">
          <SettingsPreferenceRow
            description={t('designDefaultSortDescription')}
            id="setting-design-sort"
            label={t('designDefaultSort')}
          >
            {(descriptionId) => (
              <Select
                aria-describedby={descriptionId}
                label={t('designDefaultSort')}
                onChange={(event) => {
                  if (event.target.value === 'newest' || event.target.value === 'oldest') {
                    updatePreferences({ gallerySort: event.target.value });
                  }
                }}
                options={[
                  { value: 'newest', label: t('designSortNewest') },
                  { value: 'oldest', label: t('designSortOldest') },
                ]}
                value={preferences.gallerySort}
                visuallyHiddenLabel
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designFavoritesOnlyDescription')}
            id="setting-design-favorites"
            label={t('designFavoritesOnly')}
          >
            {(descriptionId) => (
              <Switch
                aria-label={t('designFavoritesOnly')}
                aria-describedby={descriptionId}
                checked={preferences.favoritesOnly}
                onCheckedChange={(value) => updatePreferences({ favoritesOnly: value })}
              />
            )}
          </SettingsPreferenceRow>
          <SettingsPreferenceRow
            description={t('designDefaultCollectionDescription')}
            id="setting-design-default-collection"
            label={t('designDefaultCollection')}
          >
            {(descriptionId) =>
              collectionsLoadFailed ? (
                <span className="text-sm text-warning">{t('designCollectionsUnavailable')}</span>
              ) : (
                <Select
                  aria-describedby={descriptionId}
                  label={t('designDefaultCollection')}
                  onChange={(event) => updatePreferences({ defaultCollectionId: event.target.value || null })}
                  options={[
                    { value: '', label: t('designNoDefaultCollection') },
                    ...collections.map((collection) => ({ value: collection.id, label: collection.name })),
                  ]}
                  value={validDefaultCollectionId ?? ''}
                  visuallyHiddenLabel
                />
              )
            }
          </SettingsPreferenceRow>
        </Card>
      </fieldset>
      <SettingsSaveFeedback errorMessage={t('designPreferencesSaveFailed')} state={saveState} />
    </section>
  );
}

function parseQuality(value: string): 'auto' | 'low' | 'medium' | 'high' | 'xhigh' | 'max' {
  if (value === 'low' || value === 'medium' || value === 'high' || value === 'xhigh' || value === 'max') return value;
  return 'auto';
}

function parseBackground(value: string): 'auto' | 'opaque' | 'transparent' {
  if (value === 'opaque' || value === 'transparent') return value;
  return 'auto';
}

function parseOutputFormat(value: string): 'png' | 'jpeg' | 'webp' {
  if (value === 'jpeg' || value === 'webp') return value;
  return 'png';
}

function qualityLabel(
  value: 'auto' | 'low' | 'medium' | 'high' | 'xhigh' | 'max',
):
  | 'designQualityAuto'
  | 'designQualityLow'
  | 'designQualityMedium'
  | 'designQualityHigh'
  | 'designQualityXhigh'
  | 'designQualityMax' {
  switch (value) {
    case 'auto':
      return 'designQualityAuto';
    case 'low':
      return 'designQualityLow';
    case 'medium':
      return 'designQualityMedium';
    case 'high':
      return 'designQualityHigh';
    case 'xhigh':
      return 'designQualityXhigh';
    case 'max':
      return 'designQualityMax';
  }
}
