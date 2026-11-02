import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';

export const SETTINGS_SIDEBAR_DEFAULT_WIDTH = 272;
export const SETTINGS_SIDEBAR_MIN_WIDTH = 220;
export const SETTINGS_SIDEBAR_MAX_WIDTH = 420;

const settingsSidebarWidthKey = 'settings.sidebar_width';

export function clampSettingsSidebarWidth(width: number): number {
  if (!Number.isFinite(width)) return SETTINGS_SIDEBAR_DEFAULT_WIDTH;
  return Math.round(Math.min(SETTINGS_SIDEBAR_MAX_WIDTH, Math.max(SETTINGS_SIDEBAR_MIN_WIDTH, width)));
}

export function readSettingsSidebarWidth(): number {
  const encoded = unifiedStorage.getItem(settingsSidebarWidthKey);
  if (encoded === null) return SETTINGS_SIDEBAR_DEFAULT_WIDTH;
  return clampSettingsSidebarWidth(Number(encoded));
}

export function writeSettingsSidebarWidth(width: number): void {
  unifiedStorage.setItem(settingsSidebarWidthKey, String(clampSettingsSidebarWidth(width)));
}
