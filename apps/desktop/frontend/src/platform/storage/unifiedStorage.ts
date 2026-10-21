import { invoke, isTauri } from '@/platform/tauri';
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

class UnifiedStorage {
  private cache = new Map<string, string>();
  private initialized = false;

  /**
   * Initializes the in-memory cache from the SQLite database.
   * If legacy localStorage entries exist, migrates them to SQLite and clears localStorage.
   */
  async initialize(): Promise<void> {
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

    try {
      // 1. Fetch all settings currently stored in SQLite
      const existingSettings = (await invoke<Record<string, string>>('get_all_app_settings')) ?? {};
      for (const [key, val] of Object.entries(existingSettings)) {
        this.cache.set(key, val);
      }

      // 2. Check if legacy localStorage has data needing migration
      if (typeof window !== 'undefined' && window.localStorage && window.localStorage.length > 0) {
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
          await invoke('migrate_localstorage_to_db', {
            payload: {
              settings: legacySettings,
              facts: legacyFacts,
            },
          });

          // Mirror migrated settings to in-memory cache
          for (const [key, val] of Object.entries(legacySettings)) {
            this.cache.set(key, val);
          }

          // Clear legacy localStorage as requested
          window.localStorage.clear();
        }
      }
    } catch (error) {
      // Degrade gracefully without crashing startup
      globalThis.reportError(new Error('Failed to initialize unified SQLite storage.', { cause: error }));
    }

    this.initialized = true;
  }

  getItem(key: string): string | null {
    return this.cache.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.cache.set(key, value);
    if (isTauri()) {
      invoke('set_app_setting', { key, valueJson: value }).catch((error) => {
        globalThis.reportError(new Error(`Failed to persist setting '${key}' to SQLite.`, { cause: error }));
      });
    }
  }

  removeItem(key: string): void {
    this.cache.delete(key);
    if (isTauri()) {
      invoke('delete_app_setting', { key }).catch((error) => {
        globalThis.reportError(new Error(`Failed to delete setting '${key}' from SQLite.`, { cause: error }));
      });
    }
  }

  clear(): void {
    const keys = [...this.cache.keys()];
    this.cache.clear();
    if (isTauri()) {
      for (const key of keys) {
        invoke('delete_app_setting', { key }).catch((error) => {
          globalThis.reportError(new Error(`Failed to clear setting '${key}' from SQLite.`, { cause: error }));
        });
      }
    }
  }

  getAll(): Record<string, string> {
    return Object.fromEntries(this.cache.entries());
  }
}

export const unifiedStorage = new UnifiedStorage();
