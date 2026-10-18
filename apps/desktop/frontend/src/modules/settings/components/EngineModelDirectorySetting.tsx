import { FolderOpen, LoaderCircle, Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';

export function EngineModelDirectorySetting({
  available,
  directory,
  isBusy,
  isCustom,
  onChoose,
  onScan,
  onUseDefault,
  scanning,
}: {
  available: boolean;
  directory: string;
  isBusy: boolean;
  isCustom: boolean;
  onChoose(): void;
  onScan(): void;
  onUseDefault(): void;
  scanning: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid gap-3 rounded-control border border-border bg-surface p-4">
      <div>
        <h4 className="font-semibold text-text">{t('localModelDirectoryTitle')}</h4>
        <p className="mt-1 text-sm text-secondary-text">{t('localModelDirectoryDescription')}</p>
      </div>
      <p className={`break-all font-mono text-xs ${available ? 'text-secondary-text' : 'text-warning'}`}>{directory}</p>
      {!available ? (
        <p className="text-sm text-warning" role="status">
          {t('localModelDirectoryUnavailable')}
        </p>
      ) : null}
      <div className="flex flex-wrap gap-2">
        <Button disabled={isBusy} onClick={onChoose} size="small" variant="secondary">
          <FolderOpen aria-hidden="true" size={16} />
          {t('localModelChooseDirectory')}
        </Button>
        <Button disabled={isBusy} onClick={onScan} size="small" variant="ghost">
          {scanning ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          ) : (
            <Search aria-hidden="true" size={16} />
          )}
          {t(scanning ? 'localModelScanningDirectory' : 'localModelScanDirectory')}
        </Button>
        {isCustom ? (
          <Button disabled={isBusy} onClick={onUseDefault} size="small" variant="ghost">
            {t('localModelUseDefaultDirectory')}
          </Button>
        ) : null}
      </div>
    </div>
  );
}
