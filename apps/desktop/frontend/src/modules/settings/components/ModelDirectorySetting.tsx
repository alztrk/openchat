import { FolderOpen, Save } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Input } from '@/design-system';
import { useModelDirectoryStore } from '@/modules/settings/stores/modelDirectoryStore';
import { localServiceClient } from '@/platform/local-service';
import { open } from '@/platform/tauri';

type LoadState = 'loading' | 'ready' | 'error';

export function ModelDirectorySetting() {
  const { t } = useTranslation();
  const customDirectory = useModelDirectoryStore((store) => store.directory);
  const preferenceAvailable = useModelDirectoryStore((store) => store.available);
  const saveDirectory = useModelDirectoryStore((store) => store.setDirectory);
  const resetDirectory = useModelDirectoryStore((store) => store.resetDirectory);
  const [defaultDirectory, setDefaultDirectory] = useState('');
  const [directory, setDirectory] = useState(customDirectory ?? '');
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [errorKey, setErrorKey] = useState<'localModelDirectoryUnavailable' | 'localModelActionError' | null>(null);

  useEffect(() => {
    let mounted = true;
    const loadDefaultDirectory = async () => {
      try {
        const response = await localServiceClient.call('local.engines.list');
        if (!Array.isArray(response.engines)) throw new Error('Local engine catalog is invalid.');
        const engine = response.engines.find((item: unknown) => isObject(item) && item.engineId === 'llama_cpp');
        if (!isObject(engine) || typeof engine.modelDirectory !== 'string') {
          throw new Error('Model folder is unavailable.');
        }
        if (!mounted) return;
        setDefaultDirectory(engine.modelDirectory);
        setDirectory(customDirectory ?? engine.modelDirectory);
        setLoadState('ready');
        setErrorKey(preferenceAvailable ? null : 'localModelActionError');
      } catch {
        if (!mounted) return;
        setLoadState('error');
        setErrorKey('localModelDirectoryUnavailable');
      }
    };
    void loadDefaultDirectory();
    return () => {
      mounted = false;
    };
  }, [customDirectory, preferenceAvailable]);

  const persistDirectory = () => {
    const value = directory.trim();
    const saved = value === defaultDirectory.trim() ? resetDirectory() : saveDirectory(value);
    if (!saved) {
      setErrorKey(value ? 'localModelActionError' : 'localModelDirectoryUnavailable');
      return;
    }
    setErrorKey(null);
  };

  const chooseDirectory = async () => {
    if (loadState !== 'ready') return;
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === 'string') {
        setDirectory(selected);
        setErrorKey(null);
      }
    } catch {
      setErrorKey('localModelActionError');
    }
  };

  return (
    <Card className="grid gap-3 p-5">
      <div>
        <h3 className="font-semibold text-text">{t('localModelDirectoryTitle')}</h3>
        <p className="mt-1 text-sm text-secondary-text">{t('localModelDirectoryDescription')}</p>
      </div>
      <div className="grid gap-2 text-sm font-medium text-text">
        {t('localModelDirectoryTitle')}
        <Input
          aria-label={t('localModelDirectoryTitle')}
          className="font-mono text-xs"
          disabled={loadState !== 'ready'}
          onChange={(event) => setDirectory(event.target.value)}
          value={directory}
        />
      </div>
      <div className="flex flex-wrap gap-2">
        <Button disabled={loadState !== 'ready'} onClick={() => void chooseDirectory()} variant="secondary">
          <FolderOpen aria-hidden="true" size={16} />
          {t('localModelChooseDirectory')}
        </Button>
        <Button disabled={loadState !== 'ready'} onClick={persistDirectory} variant="secondary">
          <Save aria-hidden="true" size={16} />
          {t('save')}
        </Button>
        {customDirectory ? (
          <Button
            disabled={loadState !== 'ready'}
            onClick={() => {
              if (resetDirectory()) {
                setDirectory(defaultDirectory);
                setErrorKey(null);
              } else {
                setErrorKey('localModelActionError');
              }
            }}
            variant="ghost"
          >
            {t('localModelUseDefaultDirectory')}
          </Button>
        ) : null}
      </div>
      {loadState === 'loading' ? (
        <p aria-live="polite" className="text-sm text-secondary-text">
          {t('localModelsLoading')}
        </p>
      ) : null}
      {errorKey ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}
    </Card>
  );
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
