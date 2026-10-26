import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Textarea } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import { localServiceClient } from '@/platform/local-service';

const maximumInstructionBytes = 16 * 1024;

interface ProjectInstructionsPanelProps {
  project: ChatProject;
  onClose(): void;
}

export function ProjectInstructionsPanel({ project, onClose }: ProjectInstructionsPanelProps) {
  const { t } = useTranslation();
  const [instructions, setInstructions] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const byteCount = new TextEncoder().encode(instructions).length;

  useEffect(() => {
    let current = true;
    setIsLoading(true);
    setLoadFailed(false);
    void localServiceClient
      .call('project.instructions.get', { projectId: project.id })
      .then((response) => {
        const value = response.instructions;
        if (typeof value !== 'string') throw new Error('The project instructions were invalid.');
        if (current) setInstructions(value);
      })
      .catch(() => {
        if (current) setLoadFailed(true);
      })
      .finally(() => {
        if (current) setIsLoading(false);
      });
    return () => {
      current = false;
    };
  }, [project.id]);

  const reload = async () => {
    setIsLoading(true);
    setLoadFailed(false);
    try {
      const response = await localServiceClient.call('project.instructions.get', { projectId: project.id });
      if (typeof response.instructions !== 'string') throw new Error('The project instructions were invalid.');
      setInstructions(response.instructions);
    } catch {
      setLoadFailed(true);
    } finally {
      setIsLoading(false);
    }
  };

  const save = async () => {
    if (isSaving || loadFailed || byteCount > maximumInstructionBytes) return;
    setIsSaving(true);
    setSaveFailed(false);
    try {
      await localServiceClient.call('project.instructions.save', {
        projectId: project.id,
        instructions,
      });
      onClose();
    } catch {
      setSaveFailed(true);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="grid gap-3">
      <p className="text-sm text-secondary-text">{t('projectInstructionsDescription')}</p>
      {isLoading ? (
        <p aria-live="polite" className="py-8 text-center text-sm text-secondary-text">
          {t('historyLoading')}
        </p>
      ) : (
        <>
          <label className="grid gap-2 text-sm font-medium text-text">
            {t('projectInstructionsTitle')}
            <Textarea
              aria-label={t('projectInstructionsTitle')}
              className="min-h-40 resize-y rounded-control border border-border bg-raised p-3 font-ui text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
              fieldLayout="none"
              disabled={isSaving || loadFailed}
              onChange={(event) => setInstructions(event.target.value)}
              placeholder={t('projectInstructionsHint')}
              value={instructions}
            />
          </label>
          <p className="text-right text-xs text-secondary-text">
            {t('projectInstructionsSize', { bytes: byteCount, maxBytes: maximumInstructionBytes })}
          </p>
          {loadFailed ? (
            <div className="flex items-center justify-between gap-3">
              <p className="text-sm text-destructive" role="alert">
                {t('projectInstructionsLoadFailed')}
              </p>
              <Button disabled={isLoading} onClick={() => void reload()} size="small">
                {t('retry')}
              </Button>
            </div>
          ) : null}
          {saveFailed ? (
            <p className="text-sm text-destructive" role="alert">
              {t('projectInstructionsSaveFailed')}
            </p>
          ) : null}
        </>
      )}
      <div className="flex justify-end gap-2">
        <Button disabled={isSaving} onClick={onClose}>
          {t('cancel')}
        </Button>
        <Button
          disabled={isLoading || isSaving || loadFailed || byteCount > maximumInstructionBytes}
          onClick={() => void save()}
          variant="primary"
        >
          {isSaving ? t('saving') : t('projectInstructionsSave')}
        </Button>
      </div>
    </div>
  );
}
