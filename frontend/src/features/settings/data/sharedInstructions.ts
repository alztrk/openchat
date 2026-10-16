const sharedInstructionsStorageKey = 'chat.shared_instructions';
export const maximumSharedInstructionsCharacters = 4096;
export const maximumCustomInstructionsBytes = 16 * 1024;

export class SharedInstructionsStorageError extends Error {
  constructor(readonly code: 'too_many_characters' | 'too_many_bytes') {
    super('The shared instructions exceed the supported size.');
    this.name = 'SharedInstructionsStorageError';
  }
}

export function readSharedInstructions(): string | null {
  return localStorage.getItem(sharedInstructionsStorageKey);
}

export function writeSharedInstructions(value: string): void {
  if (countCharacters(value) > maximumSharedInstructionsCharacters) {
    throw new SharedInstructionsStorageError('too_many_characters');
  }
  if (new TextEncoder().encode(value).byteLength > maximumCustomInstructionsBytes) {
    throw new SharedInstructionsStorageError('too_many_bytes');
  }

  if (value.trim().length === 0) {
    localStorage.removeItem(sharedInstructionsStorageKey);
    return;
  }
  localStorage.setItem(sharedInstructionsStorageKey, value);
}

export function clearSharedInstructions(): void {
  localStorage.removeItem(sharedInstructionsStorageKey);
}

export function countSharedInstructionsCharacters(value: string): number {
  return countCharacters(value);
}

function countCharacters(value: string): number {
  return value.length;
}
