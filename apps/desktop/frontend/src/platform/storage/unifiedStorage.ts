import { localServiceClient } from '@/platform/local-service';
import { isTauri } from '@/platform/tauri';
import { isObject } from '@/shared/utils/json';

interface MigrationPayload {
  settings: Record<string, string>;
  facts: Array<{
    id: string;
    factText: string;
    category?: string;
    sourceConversationId?: string;
    createdAt: number;
    updatedAt: number;
  }>;
}

const skipLegacyLocalStorageImportSetting = 'internal.storage.skipLegacyLocalStorageImport';

class UnifiedStorage {
  private cache = new Map<string, string>();
  private initialized = false;

  /**
   * Initializes the in-memory cache from the SQLite database.
   * If legacy localStorage entries exist, migrates them to SQLite and clears localStorage.
   */
  async initialize(options: { startWithEmptyStorage?: boolean } = {}): Promise<void> {
    if (this.initialized) return;

    if (!isTauri()) {
      try {
        if (typeof window !== 'undefined' && window.localStorage) {
          for (let i = 0; i < window.localStorage.length; i++) {
            const key = window.localStorage.key(i);
            if (key) {
              const val = window.localStorage.getItem(key);
              if (val !== null) this.cache.set(key, val);
            }
          }
        }
      } catch {
        // Fallback for restricted test/node environments
      }
      this.initialized = true;
      return;
    }

    await localServiceClient.initialize({
      startWithEmptyStorage: options.startWithEmptyStorage,
    });
    const initialization = localServiceClient.getStartupSettings();
    if (options.startWithEmptyStorage) {
      await localServiceClient.call('storage.settings.set', {
        key: skipLegacyLocalStorageImportSetting,
        valueJson: '1',
      });
    }

    // Keep the legacy localStorage decision private to this adapter.
    const existingSettings = parseSettingsResponse(initialization);
    if (options.startWithEmptyStorage) {
      existingSettings[skipLegacyLocalStorageImportSetting] = '1';
    }
    const skipLegacyLocalStorageImport = existingSettings[skipLegacyLocalStorageImportSetting] === '1';
    delete existingSettings[skipLegacyLocalStorageImportSetting];
    for (const [key, val] of Object.entries(existingSettings)) {
      this.cache.set(key, val);
    }

    // Preserve old WebView settings when the user explicitly chose an empty storage profile.
    if (
      !skipLegacyLocalStorageImport &&
      typeof window !== 'undefined' &&
      window.localStorage &&
      window.localStorage.length > 0
    ) {
      const legacySettings: Record<string, string> = {};
      const legacyFacts: MigrationPayload['facts'] = [];

      for (let i = 0; i < window.localStorage.length; i++) {
        const key = window.localStorage.key(i);
        if (!key) continue;
        const val = window.localStorage.getItem(key);
        if (val === null) continue;

        legacySettings[key] = val;

        // If this is personal memory facts, extract them for the personal_memory_facts table
        if (key === 'privacy.personal_memory_facts') {
          try {
            const parsed: unknown = JSON.parse(val);
            if (Array.isArray(parsed)) {
              for (const item of parsed) {
                if (isObject(item)) {
                  const id = String(item.id ?? '');
                  const factText = String(item.content ?? '');
                  if (id.length > 0 && factText.length > 0) {
                    legacyFacts.push({
                      id,
                      factText,
                      createdAt: Date.now(),
                      updatedAt: Date.now(),
                    });
                  }
                }
              }
            }
          } catch {
            // Ignore parse error
          }
        }
      }

      if (Object.keys(legacySettings).length > 0) {
        const migration = await localServiceClient.call('storage.settings.migrate_localstorage', {
          settings: legacySettings,
          facts: legacyFacts,
        });
        assertServiceFlag(migration, 'migrated', 'localStorage migration');

        // Mirror migrated settings to in-memory cache
        for (const [key, val] of Object.entries(legacySettings)) {
          this.cache.set(key, val);
        }

        // Clear legacy localStorage as requested
        window.localStorage.clear();
      }
    }

    localServiceClient.clearStartupSettings();
    this.initialized = true;
  }

  getItem(key: string): string | null {
    return this.cache.get(key) ?? null;
  }

  setItem(key: string, value: string, persist = true): void {
    this.cache.set(key, value);
    if (isTauri() && persist) {
      void localServiceClient
        .call('storage.settings.set', { key, valueJson: value })
        .then((response) => assertServiceFlag(response, 'saved', `setting '${key}' persistence`))
        .catch((error) => {
          globalThis.reportError(new Error(`Failed to persist setting '${key}' to local storage.`, { cause: error }));
        });
    }
  }

  removeItem(key: string, persist = true): void {
    this.cache.delete(key);
    if (isTauri() && persist) {
      void localServiceClient
        .call('storage.settings.delete', { key })
        .then((response) => assertServiceBoolean(response, 'deleted', `setting '${key}' deletion`))
        .catch((error) => {
          globalThis.reportError(new Error(`Failed to delete setting '${key}' from local storage.`, { cause: error }));
        });
    }
  }

  clear(): void {
    const keys = [...this.cache.keys()];
    this.cache.clear();
    if (isTauri()) {
      for (const key of keys) {
        void localServiceClient
          .call('storage.settings.delete', { key })
          .then((response) => assertServiceBoolean(response, 'deleted', `setting '${key}' clearing`))
          .catch((error) => {
            globalThis.reportError(new Error(`Failed to clear setting '${key}' from local storage.`, { cause: error }));
          });
      }
    }
  }

  getAll(): Record<string, string> {
    return Object.fromEntries(this.cache.entries());
  }
}

function parseSettingsResponse(value: unknown): Record<string, string> {
  if (!isObject(value) || !isObject(value.settings)) {
    throw new Error('The local service returned invalid application settings.');
  }
  const settings: Record<string, string> = {};
  for (const [key, setting] of Object.entries(value.settings)) {
    if (typeof setting !== 'string') {
      throw new Error(`The local service returned an invalid value for setting '${key}'.`);
    }
    settings[key] = setting;
  }
  return settings;
}

function assertServiceFlag(value: unknown, field: 'migrated' | 'saved', operation: string): void {
  if (!isObject(value) || value[field] !== true) {
    throw new Error(`The local service returned an invalid response for ${operation}.`);
  }
}

function assertServiceBoolean(value: unknown, field: 'deleted', operation: string): void {
  if (!isObject(value) || typeof value[field] !== 'boolean') {
    throw new Error(`The local service returned an invalid response for ${operation}.`);
  }
}

export const unifiedStorage = new UnifiedStorage();
