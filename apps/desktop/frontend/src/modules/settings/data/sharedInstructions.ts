import { unifiedStorage } from '@/platform/storage/unifiedStorage';

const sharedInstructionsStorageKey = 'chat.shared_instructions';
export const maximumSharedInstructionsCharacters = 20_000;
export const maximumCustomInstructionsBytes = 80 * 1024;

export class SharedInstructionsStorageError extends Error {
  constructor(readonly code: 'too_many_characters' | 'too_many_bytes') {
    super('The shared instructions exceed the supported size.');
    this.name = 'SharedInstructionsStorageError';
  }
}

export function readSharedInstructions(): string | null {
  return unifiedStorage.getItem(sharedInstructionsStorageKey);
}

export function writeSharedInstructions(value: string): void {
  if (countCharacters(value) > maximumSharedInstructionsCharacters) {
    throw new SharedInstructionsStorageError('too_many_characters');
  }
  if (new TextEncoder().encode(value).byteLength > maximumCustomInstructionsBytes) {
    throw new SharedInstructionsStorageError('too_many_bytes');
  }

  if (value.trim().length === 0) {
    unifiedStorage.removeItem(sharedInstructionsStorageKey);
    return;
  }
  unifiedStorage.setItem(sharedInstructionsStorageKey, value);
}

export function clearSharedInstructions(): void {
  unifiedStorage.removeItem(sharedInstructionsStorageKey);
}

export function countSharedInstructionsCharacters(value: string): number {
  return countCharacters(value);
}

function countCharacters(value: string): number {
  return value.length;
}
