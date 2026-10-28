import { create } from 'zustand';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { containsAsciiControlCharacters } from '@/shared/utils/textValidation';

const preferencePrefix = 'openchat.localModelDirectory.';
const llamaCppEngineId = 'llama_cpp';

interface StoredDirectory {
  directory: string | null;
  available: boolean;
}

interface ModelDirectoryState extends StoredDirectory {
  directories: Record<string, string | null>;
  directoryAvailability: Record<string, boolean>;
  hydrateDirectoryFor(engineId: string): void;
  setDirectoryFor(engineId: string, directory: string): Promise<boolean>;
  resetDirectoryFor(engineId: string): Promise<boolean>;
  setDirectory(directory: string): Promise<boolean>;
  resetDirectory(): Promise<boolean>;
}

const savedDirectory = readDirectory(llamaCppEngineId);

export const useModelDirectoryStore = create<ModelDirectoryState>((set, get) => ({
  directory: savedDirectory.directory,
  available: savedDirectory.available,
  directories: { [llamaCppEngineId]: savedDirectory.directory },
  directoryAvailability: { [llamaCppEngineId]: savedDirectory.available },
  hydrateDirectoryFor: (engineId) => {
    if (!isValidEngineId(engineId)) return;
    const saved = readDirectory(engineId);
    set((current) => ({
      directories: { ...current.directories, [engineId]: saved.directory },
      directoryAvailability: { ...current.directoryAvailability, [engineId]: saved.available },
      ...(engineId === llamaCppEngineId ? { directory: saved.directory, available: saved.available } : {}),
    }));
  },
  setDirectoryFor: async (engineId, directory) => {
    if (!isValidEngineId(engineId)) return false;
    const value = directory.trim();
    if (value.length > 4096 || containsAsciiControlCharacters(value)) return false;
    try {
      if (value.length === 0) await unifiedStorage.removePersistedItem(preferenceKey(engineId));
      else await unifiedStorage.setPersistedItem(preferenceKey(engineId), value);
      set((current) => ({
        directories: { ...current.directories, [engineId]: value.length > 0 ? value : null },
        directoryAvailability: { ...current.directoryAvailability, [engineId]: true },
        ...(engineId === llamaCppEngineId ? { directory: value.length > 0 ? value : null, available: true } : {}),
      }));
      return true;
    } catch {
      set((current) => ({
        directoryAvailability: { ...current.directoryAvailability, [engineId]: false },
        ...(engineId === llamaCppEngineId ? { available: false } : {}),
      }));
      return false;
    }
  },
  resetDirectoryFor: async (engineId) => {
    if (!isValidEngineId(engineId)) return false;
    try {
      await unifiedStorage.removePersistedItem(preferenceKey(engineId));
      set((current) => ({
        directories: { ...current.directories, [engineId]: null },
        directoryAvailability: { ...current.directoryAvailability, [engineId]: true },
        ...(engineId === llamaCppEngineId ? { directory: null, available: true } : {}),
      }));
      return true;
    } catch {
      set((current) => ({
        directoryAvailability: { ...current.directoryAvailability, [engineId]: false },
        ...(engineId === llamaCppEngineId ? { available: false } : {}),
      }));
      return false;
    }
  },
  setDirectory: async (directory) => get().setDirectoryFor(llamaCppEngineId, directory),
  resetDirectory: async () => get().resetDirectoryFor(llamaCppEngineId),
}));

function preferenceKey(engineId: string): string {
  return `${preferencePrefix}${engineId}`;
}

function readDirectory(engineId: string): StoredDirectory {
  try {
    const value = unifiedStorage.getItem(preferenceKey(engineId))?.trim();
    return { directory: value ? value : null, available: true };
  } catch {
    return { directory: null, available: false };
  }
}

function isValidEngineId(value: string): boolean {
  return value.length > 0 && value.length <= 128 && /^[A-Za-z0-9._-]+$/u.test(value);
}
