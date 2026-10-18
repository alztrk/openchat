import { LoaderCircle, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Card } from '@/design-system';
import type { InstallProgress } from '@/modules/settings/data/localEngineCatalog';
import { formatBytes, installPhaseKey } from '@/modules/settings/data/localEngineCatalog';

export function InstallProgressInline({
  assetProgress,
  locale,
  progress,
}: {
  assetProgress: Record<string, InstallProgress>;
  locale: string;
  progress: InstallProgress | null;
}) {
  const { t } = useTranslation();
  const fraction =
    progress && progress.totalBytes > 0 ? Math.min(progress.downloadedBytes, progress.totalBytes) : undefined;
  const assets = Object.values(assetProgress);
  return (
    <div className="grid gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
        <span className="font-semibold text-text">
          {progress ? t(installPhaseKey(progress.phase)) : t('localEngineInstalling')}
        </span>
        {progress ? <span className="truncate text-secondary-text">{progress.assetName}</span> : null}
      </div>
      <progress
        aria-label={t('localEngineInstallProgress')}
        className="h-2 w-full accent-accent"
        max={progress?.totalBytes ?? 1}
        value={fraction ?? 0}
      />
      {progress ? (
        <p className="text-xs text-secondary-text">
          {formatBytes(progress.downloadedBytes, locale)} / {formatBytes(progress.totalBytes, locale)}
        </p>
      ) : null}
      {assets.length > 1 ? (
        <ul className="grid gap-1">
          {assets.map((asset) => (
            <li className="flex items-center gap-2 text-xs text-secondary-text" key={asset.assetName}>
              <span className="min-w-0 flex-1 truncate">{asset.assetName}</span>
              <progress
                className="h-1.5 w-28 accent-accent"
                max={asset.totalBytes || 1}
                value={asset.totalBytes > 0 ? Math.min(asset.downloadedBytes, asset.totalBytes) : 0}
              />
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

export function InstallProgressCard({
  assetProgress,
  cancelling,
  locale,
  onCancel,
  progress,
}: {
  assetProgress: Record<string, InstallProgress>;
  cancelling: boolean;
  locale: string;
  onCancel(): void;
  progress: InstallProgress | null;
}) {
  const { t } = useTranslation();
  return (
    <Card aria-live="polite" className="grid gap-3 p-4">
      <InstallProgressInline assetProgress={assetProgress} locale={locale} progress={progress} />
      <div className="flex justify-end">
        <Button disabled={cancelling} onClick={onCancel} size="small" variant="secondary">
          {cancelling ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
          ) : (
            <X aria-hidden="true" size={16} />
          )}
          {t(cancelling ? 'localEngineCancellingInstall' : 'localEngineCancelInstall')}
        </Button>
      </div>
    </Card>
  );
}
