import type { CopilotQuotaSnapshot } from '@features/provider-settings/data';

/** Keeps premium quota details and finite chat/completion allowances visible. */
export function isCopilotQuotaVisible(quota: CopilotQuotaSnapshot): boolean {
  return (
    quota.hasQuota !== false &&
    (quota.kind === 'premiumInteractions' ||
      ((quota.kind === 'chat' || quota.kind === 'completions') &&
        quota.unlimited !== true &&
        quota.entitlement !== null &&
        quota.entitlement >= 0))
  );
}

/** Uses the server reset first, with a monthly fallback only for premium usage. */
export function copilotQuotaResetAt(quota: CopilotQuotaSnapshot, now = new Date()): number | null {
  if (quota.resetAtUnixMs !== null && quota.resetAtUnixMs > 0) return quota.resetAtUnixMs;
  return quota.kind === 'premiumInteractions'
    ? new Date(now.getFullYear(), now.getMonth() + 1, 1).getTime()
    : null;
}
