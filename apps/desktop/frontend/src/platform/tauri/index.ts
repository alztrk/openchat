import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export { getVersion } from '@tauri-apps/api/app';
export { invoke, isTauri } from '@tauri-apps/api/core';
export { listen } from '@tauri-apps/api/event';
export { join } from '@tauri-apps/api/path';
export { getCurrentWindow } from '@tauri-apps/api/window';
export { open } from '@tauri-apps/plugin-dialog';

/** A native folder choice and the one-time token that authorizes project creation. */
export interface ProjectFolderSelection {
  selectionId: string;
  folderPath: string;
}

function isProjectFolderSelection(value: unknown): value is ProjectFolderSelection {
  return (
    typeof value === 'object' &&
    value !== null &&
    'selectionId' in value &&
    typeof value.selectionId === 'string' &&
    'folderPath' in value &&
    typeof value.folderPath === 'string'
  );
}

/** Opens the native directory picker and returns a token for the selected folder. */
export async function selectProjectFolder(input: {
  title: string;
  initialPath?: string;
  previousSelectionId?: string;
}): Promise<ProjectFolderSelection | null> {
  const result: unknown = await tauriInvoke('select_project_folder', {
    title: input.title,
    initialPath: input.initialPath ?? null,
    previousSelectionId: input.previousSelectionId ?? null,
  });
  if (result === null) return null;
  if (!isProjectFolderSelection(result)) {
    throw new TypeError('The native project folder command returned an invalid result.');
  }
  return result;
}

/** Releases a folder token when the project creation flow ends without using it. */
export async function discardProjectFolderSelection(selectionId: string): Promise<void> {
  await tauriInvoke('discard_project_folder_selection', { selectionId });
}

export async function savePendingArchiveExport(
  kind: 'conversation' | 'profile',
  exportId: string,
  fileName: string,
  title: string,
): Promise<boolean> {
  const result: unknown = await tauriInvoke('save_pending_archive_export', {
    kind,
    exportId,
    fileName,
    title,
  });
  if (typeof result !== 'boolean') {
    throw new TypeError('The native archive save command returned an invalid result.');
  }
  return result;
}
