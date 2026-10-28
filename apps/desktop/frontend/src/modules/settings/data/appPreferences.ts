import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { invoke, isTauri } from '@/platform/tauri';

export type AppSurface = 'chats' | 'design' | 'models' | 'security' | 'settings';
export type StartupSurfacePreference = 'last' | AppSurface;
export type CloseWindowBehavior = 'exit' | 'minimize';

const startupSurfaceKey = 'app.startup_surface';
const lastSurfaceKey = 'app.last_surface';
const closeWindowBehaviorKey = 'app.close_window_behavior';
const launchOnStartupKey = 'app.launch_on_startup';

export function readStartupSurfacePreference(): StartupSurfacePreference {
  const value = unifiedStorage.getItem(startupSurfaceKey);
  return value === 'last' || isAppSurface(value) ? value : 'last';
}

export function readInitialSurface(): AppSurface {
  const preference = readStartupSurfacePreference();
  if (preference !== 'last') return preference;
  const lastSurface = unifiedStorage.getItem(lastSurfaceKey);
  return isAppSurface(lastSurface) ? lastSurface : 'chats';
}

export function writeStartupSurfacePreference(value: StartupSurfacePreference): Promise<void> {
  return unifiedStorage.setPersistedItem(startupSurfaceKey, value);
}

export function writeLastSurface(value: AppSurface): void {
  unifiedStorage.setItem(lastSurfaceKey, value);
}

export function readCloseWindowBehavior(): CloseWindowBehavior {
  return unifiedStorage.getItem(closeWindowBehaviorKey) === 'minimize' ? 'minimize' : 'exit';
}

export async function writeCloseWindowBehavior(value: CloseWindowBehavior): Promise<void> {
  const previous = readCloseWindowBehavior();
  if (isTauri()) await invoke('set_close_window_behavior', { behavior: value });
  try {
    await unifiedStorage.setPersistedItem(closeWindowBehaviorKey, value);
  } catch (cause) {
    if (isTauri()) {
      try {
        await invoke('set_close_window_behavior', { behavior: previous });
      } catch (rollbackCause) {
        throw new Error('The close behavior could not be restored after saving failed.', { cause: rollbackCause });
      }
    }
    throw cause;
  }
}

export function readLaunchOnStartup(): boolean {
  return unifiedStorage.getItem(launchOnStartupKey) === 'true';
}

export async function writeLaunchOnStartup(enabled: boolean): Promise<void> {
  if (!isTauri()) throw new Error('Operating-system startup settings are unavailable.');
  const previous = unifiedStorage.getItem(launchOnStartupKey);
  const previousEnabled = previous === 'true';
  await invoke('set_windows_startup_enabled', { enabled });
  try {
    await unifiedStorage.setPersistedItem(launchOnStartupKey, String(enabled));
  } catch (cause) {
    try {
      await invoke('set_windows_startup_enabled', { enabled: previousEnabled });
    } catch (restoreCause) {
      throw new Error('The operating-system startup setting could not be restored after saving failed.', {
        cause: restoreCause,
      });
    }
    throw cause;
  }
}

function isAppSurface(value: string | null): value is AppSurface {
  return value === 'chats' || value === 'design' || value === 'models' || value === 'security' || value === 'settings';
}
