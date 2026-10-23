import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Switch } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';

interface ProjectIndexState {
  enabled: boolean;
  status: string;
  indexedFiles: number;
  indexedBytes: number;
  errorDetails: string | null;
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
  const [errorDetails, setErrorDetails] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    void loadIndex(project.id)
      .then((loaded) => {
        if (current) setIndex(loaded);
      })
      .catch((cause: unknown) => {
        if (current) {
          setError('projectIndexLoadFailed');
          setErrorDetails(readErrorDetails(cause));
        }
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
    setErrorDetails(null);
    try {
      setIndex(await loadIndex(project.id));
    } catch (cause: unknown) {
      setError('projectIndexLoadFailed');
      setErrorDetails(readErrorDetails(cause));
    } finally {
      setIsBusy(false);
    }
  };

  const run = async (method: string, extra: Record<string, unknown> = {}) => {
    if (isBusy) return;
    setIsBusy(true);
    setError(null);
    setErrorDetails(null);
    try {
      const response = await localServiceClient.call(method, { projectId: project.id, ...extra });
      setIndex(parseIndex(response.index));
    } catch (cause: unknown) {
      setError('projectIndexActionFailed');
      setErrorDetails(readErrorDetails(cause));
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
          {index.status === 'stale' ? (
            <div className="grid gap-1 text-sm text-destructive" role="alert">
              <p>{t('projectIndexStale')}</p>
              {index.errorDetails ? (
                <p className="break-words text-xs">
                  {t('projectIndexCause')}: {index.errorDetails}
                </p>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
      {error ? (
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm text-destructive" role="alert">
            {t(error)}
            {errorDetails ? (
              <span className="mt-1 block break-words text-xs">
                {t('projectIndexCause')}: {errorDetails}
              </span>
            ) : null}
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
    !isCount(value.indexedBytes) ||
    (value.errorDetails !== null && typeof value.errorDetails !== 'string')
  ) {
    throw new Error('The project index state is invalid.');
  }
  return {
    enabled: value.enabled,
    status: value.status,
    indexedFiles: value.indexedFiles,
    indexedBytes: value.indexedBytes,
    errorDetails: value.errorDetails === null ? null : value.errorDetails.slice(0, 2048),
  };
}

function readErrorDetails(cause: unknown): string | null {
  if (cause instanceof LocalServiceError) return cause.details?.trim().slice(0, 2048) || null;
  if (cause instanceof Error) return cause.message.trim().slice(0, 2048) || null;
  return null;
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
