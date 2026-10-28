import { create } from 'zustand';
import type {
  ImageBackground,
  ImageModeration,
  ImageOutputFormat,
  ImageQuality,
} from '@/modules/design/data/imageGeneration';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export const DESIGN_PANEL_DEFAULT_WIDTH = 320;
export const DESIGN_PANEL_MIN_WIDTH = 220;
export const DESIGN_PANEL_MAX_WIDTH = 420;

const designPanelWidthKey = 'design.control_panel_width';
const designSettingsKey = 'design.generation_preferences';

export interface DesignGenerationSettings {
  size: string;
  quality: ImageQuality;
  background: ImageBackground;
  customBackgroundMode: 'none' | 'solid' | 'gradient';
  backgroundColor: string;
  gradientEndColor: string;
  gradientAngle: number;
  moderation: ImageModeration;
  outputFormat: ImageOutputFormat;
  outputCompression: number;
  count: number;
  customWidth: string;
  customHeight: string;
}

export interface DesignGenerationPreset {
  id: string;
  name: string;
  settings: DesignGenerationSettings;
}

export interface DesignPreferences {
  rememberLastGeneration: boolean;
  defaultGenerationSettings: DesignGenerationSettings;
  lastGenerationSettings: DesignGenerationSettings | null;
  presets: DesignGenerationPreset[];
  defaultPresetId: string | null;
  gallerySort: 'newest' | 'oldest';
  favoritesOnly: boolean;
  defaultCollectionId: string | null;
}

interface DesignPreferencesState {
  preferences: DesignPreferences;
  loadState: 'idle' | 'ready' | 'error';
  saveState: 'idle' | 'saving' | 'saved' | 'error';
  initialize(): void;
  retry(): void;
  updatePreferences(patch: Partial<DesignPreferences>): Promise<boolean>;
  savePreset(name: string): Promise<boolean>;
  deletePreset(presetId: string): Promise<boolean>;
  reset(): Promise<boolean>;
}

const defaultGenerationSettings: DesignGenerationSettings = {
  size: '1024x1024',
  quality: 'auto',
  background: 'auto',
  customBackgroundMode: 'none',
  backgroundColor: '#E8DCCB',
  gradientEndColor: '#8FA7C7',
  gradientAngle: 135,
  moderation: 'auto',
  outputFormat: 'png',
  outputCompression: 90,
  count: 1,
  customWidth: '1536',
  customHeight: '1024',
};

export const defaultDesignPreferences: DesignPreferences = {
  rememberLastGeneration: true,
  defaultGenerationSettings,
  lastGenerationSettings: null,
  presets: [],
  defaultPresetId: null,
  gallerySort: 'newest',
  favoritesOnly: false,
  defaultCollectionId: null,
};

export function readDesignPreferences(): DesignPreferences {
  const encoded = unifiedStorage.getItem(designSettingsKey);
  if (encoded === null) return defaultDesignPreferences;
  const value: unknown = JSON.parse(encoded);
  if (!isRecord(value)) throw new Error('Saved design preferences are invalid.');
  const gallerySort = value.gallerySort ?? defaultDesignPreferences.gallerySort;
  const rememberLastGeneration =
    value.rememberLastGeneration === undefined
      ? defaultDesignPreferences.rememberLastGeneration
      : requireBoolean(value.rememberLastGeneration);
  const favoritesOnly =
    value.favoritesOnly === undefined ? defaultDesignPreferences.favoritesOnly : requireBoolean(value.favoritesOnly);
  const preferences: DesignPreferences = {
    rememberLastGeneration,
    defaultGenerationSettings: parseGenerationSettings(value.defaultGenerationSettings ?? defaultGenerationSettings),
    lastGenerationSettings:
      value.lastGenerationSettings === null || value.lastGenerationSettings === undefined
        ? null
        : parseGenerationSettings(value.lastGenerationSettings),
    presets: parsePresets(value.presets ?? []),
    defaultPresetId: optionalIdentifier(value.defaultPresetId),
    gallerySort: gallerySort === 'newest' || gallerySort === 'oldest' ? gallerySort : invalidDesignPreference(),
    favoritesOnly,
    defaultCollectionId: optionalIdentifier(value.defaultCollectionId),
  };
  validateDesignPreferences(preferences);
  return preferences;
}

export const useDesignPreferencesStore = create<DesignPreferencesState>((set, get) => ({
  preferences: defaultDesignPreferences,
  loadState: 'idle',
  saveState: 'idle',

  initialize() {
    if (get().loadState === 'ready') return;
    try {
      set({ preferences: readDesignPreferences(), loadState: 'ready', saveState: 'idle' });
    } catch {
      set({ loadState: 'error', saveState: 'idle' });
    }
  },

  retry() {
    set({ loadState: 'idle' });
    get().initialize();
  },

  async updatePreferences(patch) {
    if (get().loadState !== 'ready') return false;
    const preferences = { ...get().preferences, ...patch };
    return persistPreferences(preferences, set);
  },

  async savePreset(name) {
    const current = get();
    const trimmedName = name.trim();
    if (
      current.loadState !== 'ready' ||
      trimmedName.length === 0 ||
      trimmedName.length > 48 ||
      current.preferences.presets.length >= 20 ||
      current.preferences.presets.some(
        (preset) => preset.name.toLocaleLowerCase('en-US') === trimmedName.toLocaleLowerCase('en-US'),
      )
    ) {
      return false;
    }
    const id = globalThis.crypto.randomUUID();
    return persistPreferences(
      {
        ...current.preferences,
        presets: [
          ...current.preferences.presets,
          { id, name: trimmedName, settings: current.preferences.defaultGenerationSettings },
        ],
      },
      set,
    );
  },

  async deletePreset(presetId) {
    const current = get();
    if (current.loadState !== 'ready') return false;
    const preferences = {
      ...current.preferences,
      presets: current.preferences.presets.filter((preset) => preset.id !== presetId),
      defaultPresetId: current.preferences.defaultPresetId === presetId ? null : current.preferences.defaultPresetId,
    };
    return persistPreferences(preferences, set);
  },

  async reset() {
    return persistPreferences(defaultDesignPreferences, set);
  },
}));

export function resolveDesignGenerationSettings(preferences: DesignPreferences): DesignGenerationSettings {
  if (preferences.rememberLastGeneration && preferences.lastGenerationSettings !== null) {
    return preferences.lastGenerationSettings;
  }
  const preset = preferences.presets.find((item) => item.id === preferences.defaultPresetId);
  return preset?.settings ?? preferences.defaultGenerationSettings;
}

export function clampDesignPanelWidth(width: number): number {
  if (!Number.isFinite(width)) return DESIGN_PANEL_DEFAULT_WIDTH;
  return Math.round(Math.min(DESIGN_PANEL_MAX_WIDTH, Math.max(DESIGN_PANEL_MIN_WIDTH, width)));
}

export function readDesignPanelWidth(): number {
  const encoded = unifiedStorage.getItem(designPanelWidthKey);
  if (encoded === null) return DESIGN_PANEL_DEFAULT_WIDTH;
  return clampDesignPanelWidth(Number(encoded));
}

export function writeDesignPanelWidth(width: number): Promise<void> {
  return unifiedStorage.setPersistedItem(designPanelWidthKey, String(clampDesignPanelWidth(width)));
}

async function persistPreferences(
  preferences: DesignPreferences,
  set: (partial: Partial<DesignPreferencesState>) => void,
): Promise<boolean> {
  set({ saveState: 'saving' });
  try {
    validateDesignPreferences(preferences);
    await unifiedStorage.setPersistedItem(designSettingsKey, JSON.stringify(preferences));
    set({ preferences, loadState: 'ready', saveState: 'saved' });
    return true;
  } catch {
    set({ saveState: 'error' });
    return false;
  }
}

function parsePresets(value: unknown): DesignGenerationPreset[] {
  if (!Array.isArray(value) || value.length > 20) return invalidDesignPreference();
  const presets = value.map((item) => {
    if (
      !isRecord(item) ||
      !isValidIdentifier(item.id) ||
      typeof item.name !== 'string' ||
      item.name.trim().length === 0 ||
      item.name.length > 48
    ) {
      return invalidDesignPreference();
    }
    return {
      id: item.id,
      name: item.name,
      settings: parseGenerationSettings(item.settings),
    };
  });
  const identifiers = new Set(presets.map((preset) => preset.id));
  const names = new Set(presets.map((preset) => preset.name.toLocaleLowerCase('en-US')));
  if (identifiers.size !== presets.length || names.size !== presets.length) return invalidDesignPreference();
  return presets;
}

function parseGenerationSettings(value: unknown): DesignGenerationSettings {
  if (!isRecord(value)) return invalidDesignPreference();
  const settings: DesignGenerationSettings = {
    size: requireString(value.size, 32),
    quality: requireQuality(value.quality),
    background: requireBackground(value.background),
    customBackgroundMode: requireCustomBackgroundMode(value.customBackgroundMode),
    backgroundColor: requireHexColor(value.backgroundColor),
    gradientEndColor: requireHexColor(value.gradientEndColor),
    gradientAngle: requireBoundedInteger(value.gradientAngle, 0, 360),
    moderation: requireModeration(value.moderation),
    outputFormat: requireOutputFormat(value.outputFormat),
    outputCompression: requireBoundedInteger(value.outputCompression, 0, 100),
    count: requireBoundedInteger(value.count, 1, 10),
    customWidth: requireString(value.customWidth, 4),
    customHeight: requireString(value.customHeight, 4),
  };
  if (
    !isImageSize(settings.size) ||
    !isCustomDimension(settings.customWidth) ||
    !isCustomDimension(settings.customHeight)
  ) {
    return invalidDesignPreference();
  }
  return settings;
}

function validateDesignPreferences(value: DesignPreferences): void {
  if (
    typeof value.rememberLastGeneration !== 'boolean' ||
    !(value.lastGenerationSettings === null || isValidSettings(value.lastGenerationSettings)) ||
    !isValidSettings(value.defaultGenerationSettings) ||
    !Array.isArray(value.presets) ||
    value.presets.length > 20 ||
    value.presets.some(
      (preset) =>
        !isValidIdentifier(preset.id) ||
        typeof preset.name !== 'string' ||
        preset.name.trim().length === 0 ||
        preset.name.length > 48 ||
        !isValidSettings(preset.settings),
    ) ||
    !(value.defaultPresetId === null || value.presets.some((preset) => preset.id === value.defaultPresetId)) ||
    (value.gallerySort !== 'newest' && value.gallerySort !== 'oldest') ||
    typeof value.favoritesOnly !== 'boolean' ||
    !(value.defaultCollectionId === null || isValidIdentifier(value.defaultCollectionId))
  ) {
    invalidDesignPreference();
  }
}

function isValidSettings(value: DesignGenerationSettings): boolean {
  try {
    parseGenerationSettings(value);
    return true;
  } catch {
    return false;
  }
}

function requireString(value: unknown, maxLength: number): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maxLength) return invalidDesignPreference();
  return value;
}

function requireBoolean(value: unknown): boolean {
  if (typeof value !== 'boolean') return invalidDesignPreference();
  return value;
}

function requireBoundedInteger(value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < minimum || value > maximum) {
    return invalidDesignPreference();
  }
  return value;
}

function requireQuality(value: unknown): ImageQuality {
  if (
    value === 'auto' ||
    value === 'low' ||
    value === 'medium' ||
    value === 'high' ||
    value === 'xhigh' ||
    value === 'max'
  ) {
    return value;
  }
  return invalidDesignPreference();
}

function requireBackground(value: unknown): ImageBackground {
  if (value === 'auto' || value === 'opaque' || value === 'transparent') return value;
  return invalidDesignPreference();
}

function requireModeration(value: unknown): ImageModeration {
  if (value === 'auto' || value === 'low') return value;
  return invalidDesignPreference();
}

function requireOutputFormat(value: unknown): ImageOutputFormat {
  if (value === 'png' || value === 'jpeg' || value === 'webp') return value;
  return invalidDesignPreference();
}

function requireCustomBackgroundMode(value: unknown): DesignGenerationSettings['customBackgroundMode'] {
  if (value === 'none' || value === 'solid' || value === 'gradient') return value;
  return invalidDesignPreference();
}

function requireHexColor(value: unknown): string {
  if (typeof value !== 'string' || !/^#[0-9a-f]{6}$/i.test(value)) return invalidDesignPreference();
  return value;
}

function isImageSize(value: string): boolean {
  if (value === 'auto' || value === 'custom') return true;
  const match = /^(\d{3,4})x(\d{3,4})$/.exec(value);
  if (!match) return false;
  const width = Number(match[1]);
  const height = Number(match[2]);
  return width >= 256 && width <= 4096 && height >= 256 && height <= 4096;
}

function isCustomDimension(value: string): boolean {
  const number = Number(value);
  return /^\d{3,4}$/.test(value) && number >= 256 && number <= 4096;
}

function optionalIdentifier(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (isValidIdentifier(value)) return value;
  return invalidDesignPreference();
}

function isValidIdentifier(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= 128 && !hasControlCharacters(value);
}

function hasControlCharacters(value: string): boolean {
  for (let index = 0; index < value.length; index++) {
    const code = value.charCodeAt(index);
    if (code < 0x20 || code === 0x7f) return true;
  }
  return false;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function invalidDesignPreference(): never {
  throw new Error('Saved design preferences are invalid.');
}
