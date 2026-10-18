import { create } from 'zustand';
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
  setDirectoryFor(engineId: string, directory: string): boolean;
  resetDirectoryFor(engineId: string): boolean;
  setDirectory(directory: string): boolean;
  resetDirectory(): boolean;
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
  setDirectoryFor: (engineId, directory) => {
    if (!isValidEngineId(engineId)) return false;
    const value = directory.trim();
    if (value.length > 4096 || containsAsciiControlCharacters(value)) return false;
    try {
      if (value.length === 0) localStorage.removeItem(preferenceKey(engineId));
      else localStorage.setItem(preferenceKey(engineId), value);
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
  resetDirectoryFor: (engineId) => {
    if (!isValidEngineId(engineId)) return false;
    try {
      localStorage.removeItem(preferenceKey(engineId));
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
  setDirectory: (directory) => get().setDirectoryFor(llamaCppEngineId, directory),
  resetDirectory: () => get().resetDirectoryFor(llamaCppEngineId),
}));

function preferenceKey(engineId: string): string {
  return `${preferencePrefix}${engineId}`;
}

function readDirectory(engineId: string): StoredDirectory {
  try {
    const value = localStorage.getItem(preferenceKey(engineId))?.trim();
    return { directory: value ? value : null, available: true };
  } catch {
    return { directory: null, available: false };
  }
}

function isValidEngineId(value: string): boolean {
  return value.length > 0 && value.length <= 128 && /^[A-Za-z0-9._-]+$/u.test(value);
}
