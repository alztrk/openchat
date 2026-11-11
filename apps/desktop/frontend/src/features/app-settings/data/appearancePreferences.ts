import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';

export type ConversationWidthPreference = 'narrow' | 'normal' | 'wide';
export type ConversationTextSizePreference = 'small' | 'normal' | 'large' | '150' | '175' | '200';
export type AppFontPreference = 'sourceSans3' | 'manrope' | 'segoeUi' | 'georgia';
export type AppScalePreference = 'small' | 'normal' | 'large' | '125' | '150' | '175' | '200';
export type AppDensityPreference = 'compact' | 'comfortable';

export const CONVERSATION_TEXT_SIZE_VALUES: readonly ConversationTextSizePreference[] = [
  'small',
  'normal',
  'large',
  '150',
  '175',
  '200',
];

export const APP_SCALE_VALUES: readonly AppScalePreference[] = [
  'small',
  'normal',
  'large',
  '125',
  '150',
  '175',
  '200',
];

export function isConversationTextSizePreference(value: string | null): value is ConversationTextSizePreference {
  return (
    value === 'small' ||
    value === 'normal' ||
    value === 'large' ||
    value === '150' ||
    value === '175' ||
    value === '200'
  );
}

export function isAppScalePreference(value: string | null): value is AppScalePreference {
  return (
    value === 'small' ||
    value === 'normal' ||
    value === 'large' ||
    value === '125' ||
    value === '150' ||
    value === '175' ||
    value === '200'
  );
}

export function isLargeConversationTextSize(value: ConversationTextSizePreference): boolean {
  return value === 'large' || value === '150' || value === '175' || value === '200';
}

export interface AppearancePreferences {
  conversationWidth: ConversationWidthPreference;
  conversationTextSize: ConversationTextSizePreference;
  appFont: AppFontPreference;
  appScale: AppScalePreference;
  appDensity: AppDensityPreference;
}

export interface AppearanceCssValues {
  conversationWidth: string;
  textScale: string;
  fontFamily: string;
  appScale: string;
  appSpacing: string;
}

export function readAppearancePreferences(): AppearancePreferences {
  const width = unifiedStorage.getItem('appearance.conversation_width');
  const textSize = unifiedStorage.getItem('appearance.conversation_text_size');
  const font = unifiedStorage.getItem('appearance.conversation_font');
  const appScale = unifiedStorage.getItem('appearance.app_scale');
  const appDensity = unifiedStorage.getItem('appearance.app_density');
  return {
    conversationWidth: width === 'narrow' || width === 'wide' ? width : 'normal',
    conversationTextSize: isConversationTextSizePreference(textSize) ? textSize : 'normal',
    appFont: font === 'manrope' || font === 'segoeUi' || font === 'georgia' ? font : 'sourceSans3',
    appScale: isAppScalePreference(appScale) ? appScale : 'normal',
    appDensity: appDensity === 'compact' ? 'compact' : 'comfortable',
  };
}

export function writeConversationWidthPreference(value: ConversationWidthPreference): Promise<void> {
  return unifiedStorage.setPersistedItem('appearance.conversation_width', value);
}

export function writeConversationTextSizePreference(value: ConversationTextSizePreference): Promise<void> {
  return unifiedStorage.setPersistedItem('appearance.conversation_text_size', value);
}

export function writeAppFontPreference(value: AppFontPreference): Promise<void> {
  return unifiedStorage.setPersistedItem('appearance.conversation_font', value);
}

export function writeAppScalePreference(value: AppScalePreference): Promise<void> {
  return unifiedStorage.setPersistedItem('appearance.app_scale', value);
}

export function writeAppDensityPreference(value: AppDensityPreference): Promise<void> {
  return unifiedStorage.setPersistedItem('appearance.app_density', value);
}

export function getAppearanceCssValues(preferences: AppearancePreferences): AppearanceCssValues {
  const width =
    preferences.conversationWidth === 'narrow'
      ? '760px'
      : preferences.conversationWidth === 'wide'
        ? '1120px'
        : '920px';
  const textScale =
    preferences.conversationTextSize === 'small'
      ? '0.85'
      : preferences.conversationTextSize === 'large'
        ? '1.25'
        : preferences.conversationTextSize === '150'
          ? '1.5'
          : preferences.conversationTextSize === '175'
            ? '1.75'
            : preferences.conversationTextSize === '200'
              ? '2'
              : '1';
  const appScale =
    preferences.appScale === 'small'
      ? '90%'
      : preferences.appScale === 'large'
        ? '110%'
        : preferences.appScale === '125'
          ? '125%'
          : preferences.appScale === '150'
            ? '150%'
            : preferences.appScale === '175'
              ? '175%'
              : preferences.appScale === '200'
                ? '200%'
                : '100%';
  const appSpacing = preferences.appDensity === 'compact' ? 'calc(0.25rem * 0.9)' : '0.25rem';
  const fontFamily =
    preferences.appFont === 'sourceSans3'
      ? "'Source Sans 3', sans-serif"
      : preferences.appFont === 'segoeUi'
        ? "'Segoe UI', sans-serif"
        : preferences.appFont === 'georgia'
          ? 'Georgia, serif'
          : 'Manrope, sans-serif';
  return { conversationWidth: width, textScale, fontFamily, appScale, appSpacing };
}
