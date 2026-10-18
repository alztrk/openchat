import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export type ConversationWidthPreference = 'narrow' | 'normal' | 'wide';
export type ConversationTextSizePreference = 'small' | 'normal' | 'large';
export type AppFontPreference = 'sourceSans3' | 'manrope' | 'segoeUi' | 'georgia';

export interface AppearancePreferences {
  conversationWidth: ConversationWidthPreference;
  conversationTextSize: ConversationTextSizePreference;
  appFont: AppFontPreference;
}

export interface AppearanceCssValues {
  conversationWidth: string;
  textScale: string;
  fontFamily: string;
}

const defaultAppearancePreferences: AppearancePreferences = {
  conversationWidth: 'normal',
  conversationTextSize: 'normal',
  appFont: 'sourceSans3',
};

export function readAppearancePreferences(): AppearancePreferences {
  try {
    const width = unifiedStorage.getItem('appearance.conversation_width');
    const textSize = unifiedStorage.getItem('appearance.conversation_text_size');
    const font = unifiedStorage.getItem('appearance.conversation_font');
    return {
      conversationWidth: width === 'narrow' || width === 'wide' ? width : 'normal',
      conversationTextSize: textSize === 'small' || textSize === 'large' ? textSize : 'normal',
      appFont: font === 'manrope' || font === 'segoeUi' || font === 'georgia' ? font : 'sourceSans3',
    };
  } catch {
    return defaultAppearancePreferences;
  }
}

export function writeConversationWidthPreference(value: ConversationWidthPreference): void {
  unifiedStorage.setItem('appearance.conversation_width', value);
}

export function writeConversationTextSizePreference(value: ConversationTextSizePreference): void {
  unifiedStorage.setItem('appearance.conversation_text_size', value);
}

export function writeAppFontPreference(value: AppFontPreference): void {
  unifiedStorage.setItem('appearance.conversation_font', value);
}

export function getAppearanceCssValues(preferences: AppearancePreferences): AppearanceCssValues {
  const width =
    preferences.conversationWidth === 'narrow'
      ? '760px'
      : preferences.conversationWidth === 'wide'
        ? '1120px'
        : '920px';
  const textScale =
    preferences.conversationTextSize === 'small' ? '0.85' : preferences.conversationTextSize === 'large' ? '1.25' : '1';
  const fontFamily =
    preferences.appFont === 'sourceSans3'
      ? "'Source Sans 3', sans-serif"
      : preferences.appFont === 'segoeUi'
        ? "'Segoe UI', sans-serif"
        : preferences.appFont === 'georgia'
          ? 'Georgia, serif'
          : 'Manrope, sans-serif';
  return { conversationWidth: width, textScale, fontFamily };
}
