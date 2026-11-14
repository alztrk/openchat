import { localServiceClient } from '@shared/lib/platform/local-service';

export interface SharedDesignTextStyle {
  id: string;
  name: string;
  color: string;
  fontSize: number;
  fontWeight: number;
  revision: number;
  updatedAtUnixMs: number;
}

export async function listSharedDesignTextStyles(): Promise<SharedDesignTextStyle[]> {
  const response = await localServiceClient.call('design.styles.list', { limit: 2_000 });
  if (!Array.isArray(response.items)) throw new Error('The shared Design style list is invalid.');
  return response.items.map(parseSharedStyle);
}

export async function saveSharedDesignTextStyle(
  style: Pick<SharedDesignTextStyle, 'id' | 'name' | 'color' | 'fontSize' | 'fontWeight'>,
  expectedRevision: number | null,
): Promise<number> {
  const response = await localServiceClient.call('design.styles.save', {
    ...style,
    expectedRevision,
  });
  if (response.saved !== true || !Number.isSafeInteger(response.revision) || typeof response.revision !== 'number' || response.revision < 1) {
    throw new Error('The shared Design style save response is invalid.');
  }
  return response.revision;
}

export async function deleteSharedDesignTextStyle(id: string, expectedRevision: number): Promise<void> {
  const response = await localServiceClient.call('design.styles.delete', { id, expectedRevision });
  if (response.deleted !== true) throw new Error('The shared Design style no longer exists.');
}

function parseSharedStyle(value: unknown): SharedDesignTextStyle {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128 ||
    typeof value.name !== 'string' || value.name.trim().length === 0 || Array.from(value.name).length > 128 ||
    typeof value.color !== 'string' || !/^#[\da-f]{6}$/i.test(value.color) ||
    typeof value.fontSize !== 'number' || !Number.isFinite(value.fontSize) || value.fontSize < 1 || value.fontSize > 4096 ||
    typeof value.fontWeight !== 'number' || !Number.isInteger(value.fontWeight) || value.fontWeight < 100 || value.fontWeight > 900 ||
    typeof value.revision !== 'number' || !Number.isSafeInteger(value.revision) || value.revision < 1 ||
    typeof value.updatedAtUnixMs !== 'number' || !Number.isSafeInteger(value.updatedAtUnixMs) || value.updatedAtUnixMs < 0) {
    throw new Error('A shared Design style is invalid.');
  }
  return {
    id: value.id,
    name: value.name,
    color: value.color,
    fontSize: value.fontSize,
    fontWeight: value.fontWeight,
    revision: value.revision,
    updatedAtUnixMs: value.updatedAtUnixMs,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
