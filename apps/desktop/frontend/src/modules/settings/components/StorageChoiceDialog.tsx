import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';

export function StorageChoiceDialog({
  active,
  onCancel,
  onChoose,
  path,
  targetDirectory,
}: {
  active: boolean;
  onCancel(): void;
  onChoose(action: 'move' | 'copy' | 'keep'): void;
  path: string;
  targetDirectory: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid gap-4">
      <p className="break-all font-mono text-xs text-secondary-text">{path}</p>
      <p className="text-sm text-secondary-text">{t('localModelStorageChoiceTarget', { folder: targetDirectory })}</p>
      <div className="grid gap-2">
        <Button disabled={active} onClick={() => onChoose('move')} variant="secondary">
          {t('localModelMoveToFolder')}
        </Button>
        <Button disabled={active} onClick={() => onChoose('copy')} variant="secondary">
          {t('localModelCopyToFolder')}
        </Button>
        <Button disabled={active} onClick={() => onChoose('keep')} variant="ghost">
          {t('localModelKeepInPlace')}
        </Button>
      </div>
      <Button disabled={active} onClick={onCancel} variant="ghost">
        {t('cancel')}
      </Button>
    </div>
  );
}
