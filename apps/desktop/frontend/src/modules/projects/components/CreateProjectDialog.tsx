import { Folder, FolderOpen, LoaderCircle } from 'lucide-react';
import { type FormEvent, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Input } from '@/design-system';
import { discardProjectFolderSelection, selectProjectFolder } from '@/platform/tauri';

/** Props for collecting a project name and native folder selection. */
interface CreateProjectDialogProps {
  open: boolean;
  onOpenChange(open: boolean): void;
  onCreate(name: string, folderSelectionId: string): Promise<void>;
}

function getFolderBaseName(folderPath: string): string {
  const trimmed = folderPath.replace(/[\\/]+$/, '');
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] ?? folderPath;
}

/** Creates projects without allowing the renderer to choose the persisted folder path. */
export function CreateProjectDialog({ open: isOpen, onOpenChange, onCreate }: CreateProjectDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [folderSelectionId, setFolderSelectionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [isSelectingFolder, setIsSelectingFolder] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    setName('');
    setFolderPath(null);
    setFolderSelectionId(null);
    setError(null);
  }, [isOpen]);

  const selectFolder = async () => {
    if (isSelectingFolder || isBusy) return;
    setIsSelectingFolder(true);
    setError(null);
    try {
      const selected = await selectProjectFolder({
        title: t('chooseProjectFolder'),
        previousSelectionId: folderSelectionId ?? undefined,
      });
      if (selected) {
        setFolderPath(selected.folderPath);
        setFolderSelectionId(selected.selectionId);
        setName((current) => {
          if (current.trim().length > 0) return current;
          return getFolderBaseName(selected.folderPath);
        });
      }
    } catch {
      setError(t('projectFolderSelectionFailed'));
    } finally {
      setIsSelectingFolder(false);
    }
  };

  const createProject = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const normalizedName = name.trim();
    if (!normalizedName) {
      setError(t('projectNameRequired'));
      return;
    }
    if (!folderSelectionId) {
      setError(t('projectFolderNotSelected'));
      return;
    }
    setIsBusy(true);
    setError(null);
    try {
      await onCreate(normalizedName, folderSelectionId);
      setName('');
      setFolderPath(null);
      setFolderSelectionId(null);
      onOpenChange(false);
    } catch {
      if (folderSelectionId) {
        try {
          await discardProjectFolderSelection(folderSelectionId);
        } catch {
          setFolderPath(null);
          setFolderSelectionId(null);
          setError(t('projectFolderSelectionFailed'));
          return;
        }
      }
      setFolderPath(null);
      setFolderSelectionId(null);
      setError(t('projectCreateFailed'));
    } finally {
      setIsBusy(false);
    }
  };

  const closeDialog = async () => {
    if (isBusy || isSelectingFolder) return;
    if (folderSelectionId) {
      try {
        await discardProjectFolderSelection(folderSelectionId);
      } catch {
        setError(t('projectFolderSelectionFailed'));
        return;
      }
    }
    setFolderPath(null);
    setFolderSelectionId(null);
    onOpenChange(false);
  };

  return (
    <Dialog
      onOpenChange={(nextOpen) => {
        if (nextOpen) onOpenChange(true);
        else void closeDialog();
      }}
      open={isOpen}
      title={t('createProject')}
    >
      <form className="grid gap-4" onSubmit={(event) => void createProject(event)}>
        <Input
          autoFocus
          disabled={isBusy || isSelectingFolder}
          label={t('projectName')}
          maxLength={512}
          onChange={(event) => {
            setName(event.target.value);
            if (error) setError(null);
          }}
          placeholder={t('projectNamePlaceholder')}
          value={name}
        />

        <div className="grid gap-2">
          <span className="text-sm font-medium text-text">{t('projectFolder')}</span>
          <button
            className={`group flex min-h-12 w-full items-center justify-between gap-3 rounded-button border px-3 py-2.5 text-left transition-all duration-quick focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50 ${
              folderPath
                ? 'border-accent/40 bg-accent/5 hover:border-accent/60 hover:bg-accent/10'
                : 'border-control-border bg-surface hover:border-accent/40 hover:bg-hover/60'
            }`}
            disabled={isBusy || isSelectingFolder}
            onClick={() => void selectFolder()}
            title={folderPath ?? t('chooseProjectFolder')}
            type="button"
          >
            <div className="flex min-w-0 flex-1 items-center gap-2.5">
              <span
                className={`flex size-8 shrink-0 items-center justify-center rounded-control transition-colors ${
                  folderPath ? 'bg-accent/15 text-accent' : 'bg-hover text-secondary-icon group-hover:text-text'
                }`}
              >
                {isSelectingFolder ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={16} />
                ) : folderPath ? (
                  <FolderOpen aria-hidden="true" size={16} />
                ) : (
                  <Folder aria-hidden="true" size={16} />
                )}
              </span>
              <div className="grid min-w-0 flex-1 gap-0.5">
                {folderPath ? (
                  <>
                    <span className="truncate text-sm font-medium text-text">{getFolderBaseName(folderPath)}</span>
                    <span className="truncate font-code text-xs text-secondary-text">{folderPath}</span>
                  </>
                ) : (
                  <span className="truncate text-sm text-secondary-text group-hover:text-text">
                    {t('chooseProjectFolder')}
                  </span>
                )}
              </div>
            </div>
            <span className="shrink-0 rounded-control border border-border bg-raised px-2.5 py-1 text-xs font-medium text-text shadow-2xs transition-colors group-hover:border-control-border">
              {folderPath ? t('changeProjectFolder') : t('chooseProjectFolder')}
            </span>
          </button>
        </div>

        {error ? (
          <p
            className="rounded-control border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs font-medium text-destructive"
            role="alert"
          >
            {error}
          </p>
        ) : null}

        <div className="mt-1 flex items-center justify-end gap-2 border-t border-border pt-4">
          <Button disabled={isBusy || isSelectingFolder} onClick={() => void closeDialog()} type="button">
            {t('cancel')}
          </Button>
          <Button disabled={isBusy || isSelectingFolder} type="submit" variant="primary">
            {t('createProjectSubmit')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
