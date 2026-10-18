import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const fastModePreferenceKey = 'chatgpt.fast_mode';

export function readChatGptFastMode(): boolean {
  let value: string | null;
  try {
    value = unifiedStorage.getItem(fastModePreferenceKey);
  } catch {
    throw new Error('Fast mode preference could not be read.');
  }
  if (value === null) return false;
  if (value !== 'true' && value !== 'false') {
    throw new Error('Fast mode preference is invalid.');
  }
  return value === 'true';
}

export function writeChatGptFastMode(enabled: boolean): void {
  try {
    unifiedStorage.setItem(fastModePreferenceKey, String(enabled));
  } catch {
    throw new Error('Fast mode preference could not be saved.');
  }
}
