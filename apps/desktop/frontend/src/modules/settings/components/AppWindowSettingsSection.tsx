import { FolderOpen } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Select, Switch } from '@/design-system';
import { SettingsPreferenceRow } from '@/modules/settings/components/SettingsPreferenceRow';
import { SettingsSaveFeedback, type SettingsSaveState } from '@/modules/settings/components/SettingsState';
import {
  type CloseWindowBehavior,
  readCloseWindowBehavior,
  readLaunchOnStartup,
  readStartupSurfacePreference,
  type StartupSurfacePreference,
  writeCloseWindowBehavior,
  writeLaunchOnStartup,
  writeStartupSurfacePreference,
} from '@/modules/settings/data/appPreferences';
import { localServiceClient } from '@/platform/local-service';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { isTauri, open } from '@/platform/tauri';

const defaultWorkspaceSettingKey = 'openchat.defaultWorkspacePath';

export function AppWindowSettingsSection() {
  const { t } = useTranslation();
  const [defaultWorkspacePath, setDefaultWorkspacePath] = useState(
    () => unifiedStorage.getItem(defaultWorkspaceSettingKey) ?? '',
  );
  const [workspaceSaveState, setWorkspaceSaveState] = useState<SettingsSaveState>('idle');
  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
  const [choosingFolder, setChoosingFolder] = useState(false);
  const [startupSurface, setStartupSurface] = useState<StartupSurfacePreference>(readStartupSurfacePreference);
  const [closeBehavior, setCloseBehavior] = useState<CloseWindowBehavior>(readCloseWindowBehavior);
  const [launchOnStartup, setLaunchOnStartup] = useState(readLaunchOnStartup);
  const [appSaveState, setAppSaveState] = useState<SettingsSaveState>('idle');

  const saveDefaultWorkspace = async (value: string) => {
    setWorkspaceSaveState('saving');
    setWorkspaceError(null);
    try {
      const trimmed = value.trim();
      if (!trimmed) {
        await unifiedStorage.removePersistedItem(defaultWorkspaceSettingKey);
        setDefaultWorkspacePath('');
        setWorkspaceSaveState('saved');
        return;
      }

      const validated = await localServiceClient.call('storage.settings.validate_workspace', { path: trimmed });
      const canonical = typeof validated.path === 'string' ? validated.path : '';
      if (!canonical) throw new Error('The workspace path is invalid.');
      await unifiedStorage.setPersistedItem(defaultWorkspaceSettingKey, canonical);
      setDefaultWorkspacePath(canonical);
      setWorkspaceSaveState('saved');
    } catch {
      setWorkspaceSaveState('error');
      setWorkspaceError(t('defaultWorkspaceSaveFailed'));
    }
  };

  const chooseDefaultWorkspace = async () => {
    setChoosingFolder(true);
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === 'string') {
        await saveDefaultWorkspace(selected);
      }
    } catch {
      setWorkspaceSaveState('error');
      setWorkspaceError(t('defaultWorkspaceSaveFailed'));
    } finally {
      setChoosingFolder(false);
    }
  };

  const changeStartupSurface = async (value: string) => {
    if (value !== 'last' && value !== 'chats' && value !== 'models' && value !== 'design' && value !== 'settings') {
      return;
    }
    setAppSaveState('saving');
    try {
      await writeStartupSurfacePreference(value);
      setStartupSurface(value);
      setAppSaveState('saved');
    } catch {
      setAppSaveState('error');
    }
  };

  const changeCloseBehavior = async (value: string) => {
    if (value !== 'exit' && value !== 'minimize') return;
    setAppSaveState('saving');
    try {
      await writeCloseWindowBehavior(value);
      setCloseBehavior(value);
      setAppSaveState('saved');
    } catch {
      setAppSaveState('error');
    }
  };

  const changeLaunchOnStartup = async (enabled: boolean) => {
    setAppSaveState('saving');
    try {
      await writeLaunchOnStartup(enabled);
      setLaunchOnStartup(enabled);
      setAppSaveState('saved');
    } catch {
      setAppSaveState('error');
    }
  };

  const appControlsDisabled = appSaveState === 'saving';

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      <div className="grid gap-0 rounded-card border border-border bg-raised">
        <SettingsPreferenceRow
          description={t('defaultWorkspaceDescription')}
          id="setting-default-workspace"
          label={t('defaultWorkspace')}
        >
          {(descriptionId) => (
            <div className="grid min-w-0 gap-2">
              <Button
                aria-describedby={descriptionId}
                className="min-w-0 w-full justify-start gap-3 rounded-control border-control-border bg-surface px-3 text-left"
                loading={choosingFolder}
                loadingContent={t('defaultWorkspaceChooseFolder')}
                onClick={() => void chooseDefaultWorkspace()}
                title={defaultWorkspacePath || undefined}
                variant="secondary"
              >
                <FolderOpen aria-hidden="true" className="shrink-0 text-secondary-text" size={16} />
                <span
                  className={
                    defaultWorkspacePath
                      ? 'min-w-0 flex-1 truncate font-mono text-xs text-text'
                      : 'min-w-0 flex-1 truncate text-sm text-secondary-text'
                  }
                >
                  {defaultWorkspacePath || t('defaultWorkspacePlaceholder')}
                </span>
                <span className="shrink-0 text-xs font-medium text-accent">{t('defaultWorkspaceChooseFolder')}</span>
              </Button>
              {defaultWorkspacePath ? (
                <Button
                  disabled={workspaceSaveState === 'saving' || choosingFolder}
                  loading={workspaceSaveState === 'saving'}
                  loadingContent={t('settingsSaving')}
                  onClick={() => void saveDefaultWorkspace('')}
                  size="small"
                  variant="ghost"
                >
                  {t('defaultWorkspaceClear')}
                </Button>
              ) : null}
              <SettingsSaveFeedback errorMessage={workspaceError ?? undefined} state={workspaceSaveState} />
            </div>
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('startupSurfaceDescription')}
          id="setting-startup-surface"
          label={t('startupSurface')}
        >
          {(descriptionId) => (
            <Select
              aria-describedby={descriptionId}
              disabled={appControlsDisabled}
              label={t('startupSurface')}
              onChange={(event) => void changeStartupSurface(event.target.value)}
              options={[
                { value: 'last', label: t('startupSurfaceLast') },
                { value: 'chats', label: t('chats') },
                { value: 'models', label: t('modelLibrary') },
                { value: 'design', label: t('design') },
                { value: 'security', label: t('securityScansTitle') },
                { value: 'settings', label: t('settings') },
              ]}
              value={startupSurface}
              visuallyHiddenLabel
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('windowCloseBehaviorDescription')}
          id="setting-window-close-behavior"
          label={t('windowCloseBehavior')}
        >
          {(descriptionId) => (
            <Select
              aria-describedby={descriptionId}
              disabled={appControlsDisabled}
              label={t('windowCloseBehavior')}
              onChange={(event) => void changeCloseBehavior(event.target.value)}
              options={[
                { value: 'exit', label: t('windowCloseExit') },
                { value: 'minimize', label: t('windowCloseMinimize') },
              ]}
              value={closeBehavior}
              visuallyHiddenLabel
            />
          )}
        </SettingsPreferenceRow>
        <SettingsPreferenceRow
          description={t('launchOnStartupDescription')}
          id="setting-launch-on-startup"
          label={t('launchOnStartup')}
        >
          {(descriptionId) => (
            <Switch
              aria-label={t('launchOnStartup')}
              aria-describedby={descriptionId}
              checked={launchOnStartup}
              disabled={!isTauri() || appControlsDisabled}
              onCheckedChange={(enabled) => void changeLaunchOnStartup(enabled)}
            />
          )}
        </SettingsPreferenceRow>
      </div>
      <SettingsSaveFeedback state={appSaveState} />
    </section>
  );
}
