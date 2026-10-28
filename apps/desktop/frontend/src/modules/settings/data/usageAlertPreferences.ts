import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const storageKey = 'usage.alert_preferences';

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
    typeof quotaWarningAtPercent !== 'number' ||
    !Number.isInteger(quotaWarningAtPercent) ||
    quotaWarningAtPercent < 1 ||
    quotaWarningAtPercent > 100 ||
    typeof monthlyCostWarningAtPercent !== 'number' ||
    !Number.isInteger(monthlyCostWarningAtPercent) ||
    monthlyCostWarningAtPercent < 1 ||
    monthlyCostWarningAtPercent > 100 ||
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

export function writeUsageAlertPreferences(preferences: UsageAlertPreferences): Promise<void> {
  if (
    !Number.isInteger(preferences.quotaWarningAtPercent) ||
    preferences.quotaWarningAtPercent < 1 ||
    preferences.quotaWarningAtPercent > 100 ||
    !Number.isInteger(preferences.monthlyCostWarningAtPercent) ||
    preferences.monthlyCostWarningAtPercent < 1 ||
    preferences.monthlyCostWarningAtPercent > 100 ||
    !(
      preferences.monthlyEstimatedCostLimitUsd === null ||
      (Number.isFinite(preferences.monthlyEstimatedCostLimitUsd) &&
        preferences.monthlyEstimatedCostLimitUsd > 0 &&
        preferences.monthlyEstimatedCostLimitUsd <= 100_000)
    )
  ) {
    throw new Error('Usage alert preferences are invalid.');
  }
  return unifiedStorage.setPersistedItem(storageKey, JSON.stringify(preferences));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
