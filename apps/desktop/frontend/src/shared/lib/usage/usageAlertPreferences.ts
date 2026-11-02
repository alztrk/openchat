import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';

const storageKey = 'usage.alert_preferences';
const changedEvent = 'openchat:usage-alert-preferences-changed';

export interface UsageAlertPreferences {
  quotaWarningAtPercent: number;
  monthlyCostWarningAtPercent: number;
  monthlyEstimatedCostLimitUsd: number | null;
}

export const defaultUsageAlertPreferences: UsageAlertPreferences = {
  quotaWarningAtPercent: 80,
  monthlyCostWarningAtPercent: 80,
  monthlyEstimatedCostLimitUsd: null,
};

export function readUsageAlertPreferences(): UsageAlertPreferences {
  const encoded = unifiedStorage.getItem(storageKey);
  if (encoded === null) return defaultUsageAlertPreferences;
  const value: unknown = JSON.parse(encoded);
  if (!isRecord(value)) {
    throw new Error('Saved usage alert preferences are invalid.');
  }
  const quotaWarningAtPercent = value.quotaWarningAtPercent ?? defaultUsageAlertPreferences.quotaWarningAtPercent;
  const monthlyCostWarningAtPercent =
    value.monthlyCostWarningAtPercent ?? defaultUsageAlertPreferences.monthlyCostWarningAtPercent;
  const monthlyEstimatedCostLimitUsd =
    value.monthlyEstimatedCostLimitUsd ?? defaultUsageAlertPreferences.monthlyEstimatedCostLimitUsd;
  if (
    !isValidPercent(quotaWarningAtPercent) ||
    !isValidPercent(monthlyCostWarningAtPercent) ||
    !(
      monthlyEstimatedCostLimitUsd === null ||
      (typeof monthlyEstimatedCostLimitUsd === 'number' &&
        Number.isFinite(monthlyEstimatedCostLimitUsd) &&
        monthlyEstimatedCostLimitUsd > 0 &&
        monthlyEstimatedCostLimitUsd <= 100_000)
    )
  ) {
    throw new Error('Saved usage alert preferences are invalid.');
  }
  return { quotaWarningAtPercent, monthlyCostWarningAtPercent, monthlyEstimatedCostLimitUsd };
}

export async function writeUsageAlertPreferences(preferences: UsageAlertPreferences): Promise<void> {
  if (
    !isValidPercent(preferences.quotaWarningAtPercent) ||
    !isValidPercent(preferences.monthlyCostWarningAtPercent) ||
    !(preferences.monthlyEstimatedCostLimitUsd === null || isValidCostLimit(preferences.monthlyEstimatedCostLimitUsd))
  ) {
    throw new Error('Usage alert preferences are invalid.');
  }
  await unifiedStorage.setPersistedItem(storageKey, JSON.stringify(preferences));
  if (typeof window !== 'undefined') window.dispatchEvent(new Event(changedEvent));
}

export function subscribeToUsageAlertPreferences(onChange: () => void): () => void {
  if (typeof window === 'undefined') return () => undefined;
  window.addEventListener(changedEvent, onChange);
  return () => window.removeEventListener(changedEvent, onChange);
}

function isValidPercent(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 1 && value <= 100;
}

function isValidCostLimit(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 && value <= 100_000;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
