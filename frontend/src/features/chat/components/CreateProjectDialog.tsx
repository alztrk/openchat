import { useEffect, useState, type FormEvent } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Folder, FolderOpen, LoaderCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Input } from '@/shared/ui';

interface CreateProjectDialogProps {
  open: boolean;
  onOpenChange(open: boolean): void;
  onCreate(name: string, folderPath: string): Promise<void>;
}

export function CreateProjectDialog({ open: isOpen, onOpenChange, onCreate }: CreateProjectDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [isSelectingFolder, setIsSelectingFolder] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    setName('');
    setFolderPath(null);
    setError(null);
  }, [isOpen]);

  const selectFolder = async () => {
    if (isSelectingFolder || isBusy) return;
    setIsSelectingFolder(true);
    setError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('chooseProjectFolder'),
      });
      if (typeof selected === 'string' && selected.trim()) {
        setFolderPath(selected.trim());
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
    if (!folderPath) {
      setError(t('projectFolderNotSelected'));
      return;
    }
    setIsBusy(true);
    setError(null);
    try {
      await onCreate(normalizedName, folderPath);
      setName('');
      setFolderPath(null);
      onOpenChange(false);
    } catch {
      setError(t('projectCreateFailed'));
    } finally {
      setIsBusy(false);
    }
  };

  return (
    <Dialog
      onOpenChange={onOpenChange}
      open={isOpen}
      title={t('createProject')}
    >
      <form className="grid gap-5" onSubmit={(event) => void createProject(event)}>
        <Input
          autoFocus
          disabled={isBusy || isSelectingFolder}
          label={t('projectName')}
          maxLength={512}
          onChange={(event) => setName(event.target.value)}
          value={name}
        />
        <div className="grid gap-2">
          <div>
            <p className="text-sm font-medium text-text">{t('projectFolder')}</p>
            <p className="mt-1 text-sm text-secondary-text">{t('projectFolderDescription')}</p>
          </div>
          <div className="flex min-h-12 items-center gap-3 rounded-control border border-border bg-composer px-3 py-2">
            <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={18} />
            <span className="min-w-0 flex-1 break-all text-sm text-text" title={folderPath ?? undefined}>
              {folderPath ?? t('projectFolderNotSelected')}
            </span>
          </div>
          <Button disabled={isBusy || isSelectingFolder} onClick={() => void selectFolder()} type="button">
            {isSelectingFolder
              ? <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={16} />
              : <FolderOpen aria-hidden="true" size={16} />}
            {t('chooseProjectFolder')}
          </Button>
        </div>
        {error ? <p className="text-sm text-destructive" role="alert">{error}</p> : null}
        <div className="flex justify-end gap-2">
          <Button disabled={isBusy || isSelectingFolder} onClick={() => onOpenChange(false)} type="button">
            {t('cancel')}
          </Button>
          <Button disabled={isBusy || isSelectingFolder} type="submit" variant="primary">
            {t('createProject')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
