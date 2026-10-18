import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Switch } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import { localServiceClient } from '@/platform/local-service';

interface ProjectIndexState {
  enabled: boolean;
  status: string;
  indexedFiles: number;
  indexedBytes: number;
}

interface ProjectIndexPanelProps {
  project: ChatProject;
  onClose(): void;
}

export function ProjectIndexPanel({ project, onClose }: ProjectIndexPanelProps) {
  const { t } = useTranslation();
  const [index, setIndex] = useState<ProjectIndexState | null>(null);
  const [isBusy, setIsBusy] = useState(true);
  const [error, setError] = useState<'projectIndexLoadFailed' | 'projectIndexActionFailed' | null>(null);

  useEffect(() => {
    let current = true;
    void loadIndex(project.id)
      .then((loaded) => {
        if (current) setIndex(loaded);
      })
      .catch(() => {
        if (current) setError('projectIndexLoadFailed');
      })
      .finally(() => {
        if (current) setIsBusy(false);
      });
    return () => {
      current = false;
    };
  }, [project.id]);

  const reload = async () => {
    setIsBusy(true);
    setError(null);
    try {
      setIndex(await loadIndex(project.id));
    } catch {
      setError('projectIndexLoadFailed');
    } finally {
      setIsBusy(false);
    }
  };

  const run = async (method: string, extra: Record<string, unknown> = {}) => {
    if (isBusy) return;
    setIsBusy(true);
    setError(null);
    try {
      const response = await localServiceClient.call(method, { projectId: project.id, ...extra });
      setIndex(parseIndex(response.index));
    } catch {
      setError('projectIndexActionFailed');
    } finally {
      setIsBusy(false);
    }
  };

  return (
    <div className="grid gap-4">
      <p className="text-sm text-secondary-text">{t('projectIndexDescription')}</p>
      {isBusy && index === null ? (
        <p aria-live="polite" className="py-8 text-center text-sm text-secondary-text">
          {t('historyLoading')}
        </p>
      ) : null}
      {index ? (
        <div className="grid gap-3 rounded-card border border-border p-3">
          <div className="flex items-center justify-between gap-4 text-sm font-medium text-text">
            <span>{t('projectIndexEnable')}</span>
            <Switch
              aria-label={t('projectIndexEnable')}
              checked={index.enabled}
              disabled={isBusy}
              onCheckedChange={(enabled) => void run('project.index.set_enabled', { enabled })}
            />
          </div>
          <p className="text-sm text-secondary-text">
            {t('projectIndexStatus', { status: index.status, files: index.indexedFiles, bytes: index.indexedBytes })}
          </p>
        </div>
      ) : null}
      {error ? (
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm text-destructive" role="alert">
            {t(error)}
          </p>
          {error === 'projectIndexLoadFailed' ? (
            <Button disabled={isBusy} onClick={() => void reload()} size="small">
              {t('retry')}
            </Button>
          ) : null}
        </div>
      ) : null}
      <div className="flex justify-end gap-2">
        <Button disabled={isBusy} onClick={() => void run('project.index.sync')} size="small" variant="secondary">
          {t('projectIndexSync')}
        </Button>
        <Button disabled={isBusy} onClick={() => void run('project.index.clear')} size="small" variant="secondary">
          {t('projectIndexClear')}
        </Button>
        <Button disabled={isBusy} onClick={onClose}>
          {t('cancel')}
        </Button>
      </div>
    </div>
  );
}

async function loadIndex(projectId: string): Promise<ProjectIndexState> {
  const response = await localServiceClient.call('project.index.get', { projectId });
  return parseIndex(response.index);
}

function parseIndex(value: unknown): ProjectIndexState {
  if (
    !isObject(value) ||
    typeof value.enabled !== 'boolean' ||
    typeof value.status !== 'string' ||
    !isCount(value.indexedFiles) ||
    !isCount(value.indexedBytes)
  ) {
    throw new Error('The project index state is invalid.');
  }
  return {
    enabled: value.enabled,
    status: value.status,
    indexedFiles: value.indexedFiles,
    indexedBytes: value.indexedBytes,
  };
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
