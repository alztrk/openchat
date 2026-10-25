import { unifiedStorage } from '@/platform/storage/unifiedStorage';

export const DESIGN_PANEL_DEFAULT_WIDTH = 320;
export const DESIGN_PANEL_MIN_WIDTH = 220;
export const DESIGN_PANEL_MAX_WIDTH = 420;

const designPanelWidthKey = 'design.control_panel_width';

export function clampDesignPanelWidth(width: number): number {
  if (!Number.isFinite(width)) return DESIGN_PANEL_DEFAULT_WIDTH;
  return Math.round(Math.min(DESIGN_PANEL_MAX_WIDTH, Math.max(DESIGN_PANEL_MIN_WIDTH, width)));
}

export function readDesignPanelWidth(): number {
  const encoded = unifiedStorage.getItem(designPanelWidthKey);
  if (encoded === null) return DESIGN_PANEL_DEFAULT_WIDTH;
  return clampDesignPanelWidth(Number(encoded));
}

export function writeDesignPanelWidth(width: number): void {
  unifiedStorage.setItem(designPanelWidthKey, String(clampDesignPanelWidth(width)));
}
