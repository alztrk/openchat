import { FolderOpen, Link, LoaderCircle, Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, Card } from '@shared/components';
import type { ExternalServer } from '@features/app-settings/data/localEngineCatalog';

interface LlamaRuntimeSettingProps {
  executableActive: boolean;
  executableAvailable: boolean;
  executablePath: string | null;
  externalActive: boolean;
  externalServers: ExternalServer[];
  showExternalCheck: boolean;
  onCheckExternalServers(): void;
  onChooseExecutable(): void;
  onClearExecutable(): void;
  onDisconnectExternalServer(server: ExternalServer): void;
}

export function LlamaRuntimeSetting({
  executableActive,
  executableAvailable,
  executablePath,
  externalActive,
  externalServers,
  showExternalCheck,
  onCheckExternalServers,
  onChooseExecutable,
  onClearExecutable,
  onDisconnectExternalServer,
}: LlamaRuntimeSettingProps) {
  const { t } = useTranslation();

  return (
    <Card className="grid gap-3 bg-surface" padding="compact">
      <section className="grid gap-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h4 className="font-semibold text-text">{t('localEngineExecutable')}</h4>
          <div className="flex flex-wrap gap-1.5">
            <Button
              disabled={executableActive}
              onClick={onChooseExecutable}
              size="small"
              variant={!executableAvailable ? 'primary' : 'secondary'}
            >
              {executableActive ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <FolderOpen aria-hidden="true" size={16} />
              )}
              {t('localEngineExecutableChoose')}
            </Button>
            {executablePath ? (
              <Button disabled={executableActive} onClick={onClearExecutable} size="small" variant="ghost">
                {t('localEngineExecutableClear')}
              </Button>
            ) : null}
          </div>
        </div>
        <p className={`break-all font-mono text-xs ${executableAvailable ? 'text-secondary-text' : 'text-warning'}`}>
          {executablePath ?? t('localEngineExecutableNotConfigured')}
        </p>
        {!executableAvailable && executablePath ? (
          <p className="text-sm text-warning" role="status">
            {t('localEngineExecutableMissing')}
          </p>
        ) : null}
      </section>
      {showExternalCheck || externalServers.length > 0 ? (
        <section className="grid gap-2 rounded-control border border-border bg-raised p-2.5">
          {showExternalCheck ? (
            <div className="flex justify-end">
              <Button disabled={externalActive} onClick={onCheckExternalServers} size="small" variant="secondary">
                {externalActive ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
                ) : (
                  <Search aria-hidden="true" size={16} />
                )}
                {t('localEngineExternalServerCheck')}
              </Button>
            </div>
          ) : null}
          {externalServers.length > 0 ? (
            <ul className="grid gap-2">
              {externalServers.map((server) => (
                <li
                  className="flex flex-wrap items-center justify-between gap-3 rounded-control border border-border bg-surface px-3 py-2"
                  key={`${server.processId}:${server.port}`}
                >
                  <p className="flex min-w-0 items-center gap-2 text-sm text-secondary-text">
                    <Link aria-hidden="true" className="shrink-0 text-accent-icon" size={15} />
                    <span>
                      {t('localEngineExternalServerConnected', { port: server.port, count: server.modelIds.length })}
                    </span>
                  </p>
                  <Button
                    disabled={externalActive}
                    onClick={() => onDisconnectExternalServer(server)}
                    size="small"
                    variant="ghost"
                  >
                    {t('localEngineExternalServerDisconnect')}
                  </Button>
                </li>
              ))}
            </ul>
          ) : null}
        </section>
      ) : null}
    </Card>
  );
}
