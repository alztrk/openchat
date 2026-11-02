import { LocalServiceError } from '@shared/lib/platform/local-service';

export function retryDelayFromServiceError(error: unknown): number | null {
  if (!(error instanceof LocalServiceError)) return null;
  return error.retryAfterMs;
}
