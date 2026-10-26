import { Blocks, Download, Info, KeyRound, LoaderCircle, RefreshCw, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Dialog, Switch } from '@/design-system';
import { formatMcpToolLabel, type McpServerRecord } from '@/modules/model-library/data/mcpCatalog';

interface McpCardProps {
  server: McpServerRecord;
  description: string;
  categoryLabel: string;
  busy: boolean;
  onInstall(): Promise<boolean>;
  onToggle(enabled: boolean): Promise<boolean>;
  onConfigure(): void;
  onProbe(): Promise<boolean>;
  onRemove(): Promise<boolean>;
}

export function McpCard({
  busy,
  categoryLabel,
  description,
  onConfigure,
  onInstall,
  onProbe,
  onRemove,
  onToggle,
  server,
}: McpCardProps) {
  const { t } = useTranslation();
  const [confirmRemoval, setConfirmRemoval] = useState(false);
  const environmentKeys = [...server.requiredEnvKeys, ...server.optionalEnvKeys];

  const confirmAndRemove = async () => {
    if (await onRemove()) setConfirmRemoval(false);
  };

  return (
    <Card className="mcp-server-card grid min-w-0 content-start gap-4 sm:p-5" padding="default">
      <div className="mcp-server-card-heading flex min-w-0 flex-wrap items-start gap-3">
        <span
          aria-hidden="true"
          className="flex size-10 shrink-0 items-center justify-center rounded-control border border-border bg-composer text-accent-icon"
        >
          <Blocks size={19} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <h3 className="font-semibold text-text">{server.name}</h3>
            <span className="rounded-control bg-selected px-2 py-0.5 text-xs text-secondary-text">{categoryLabel}</span>
          </div>
          <p className="mcp-server-command mt-2 break-all font-mono text-xs text-secondary-text">
            {server.command} {server.args.join(' ')}
          </p>
          <p className="mt-2 break-words text-sm leading-5 text-secondary-text">{description}</p>
        </div>
        {server.installed ? (
          <div className="flex items-center gap-2">
            <span className="text-xs text-secondary-text">{t(server.enabled ? 'mcpActive' : 'mcpInactive')}</span>
            <Switch
              aria-label={`${server.name}: ${t(server.enabled ? 'mcpActive' : 'mcpInactive')}`}
              checked={server.enabled}
              disabled={busy}
              onCheckedChange={(enabled) => void onToggle(enabled)}
            />
          </div>
        ) : (
          <Button disabled={busy} onClick={() => void onInstall()} size="small" variant="elevated">
            {busy ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
            ) : (
              <Download aria-hidden="true" size={15} />
            )}
            {t('mcpInstall')}
          </Button>
        )}
      </div>

      {server.installed ? (
        <section aria-label={t('mcpDiscoverTools')} className="grid gap-2">
          {server.discoveredTools.length > 0 ? (
            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="text-sm font-medium text-text">
                {t('mcpToolCount', { count: server.discoveredTools.length })}
              </p>
              <Button
                aria-label={t('mcpDiscoverTools')}
                disabled={busy}
                onClick={() => void onProbe()}
                size="small"
                variant="secondary"
              >
                {busy ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                ) : (
                  <RefreshCw aria-hidden="true" size={15} />
                )}
                {t(busy ? 'mcpDiscovering' : 'mcpDiscoverTools')}
              </Button>
            </div>
          ) : null}
          {server.discoveredTools.length > 0 ? (
            <ul className="flex flex-wrap gap-2">
              {server.discoveredTools.map((tool) => (
                <li
                  className="max-w-full break-words rounded-control border border-border bg-composer px-2 py-1 font-mono text-xs text-text"
                  key={`${server.id}-${tool}`}
                >
                  {formatMcpToolLabel(server.id, tool, t)}
                </li>
              ))}
            </ul>
          ) : null}
          {server.discoveredTools.length === 0 ? (
            <div className="flex flex-wrap items-center gap-3 rounded-control border border-border bg-composer/50 px-3 py-2">
              <Info aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
              <p className="min-w-0 flex-1 text-sm text-secondary-text">{t('mcpNoToolsDiscovered')}</p>
              <Button
                aria-label={t('mcpDiscoverTools')}
                disabled={busy}
                onClick={() => void onProbe()}
                size="small"
                variant="secondary"
              >
                {busy ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                ) : (
                  <RefreshCw aria-hidden="true" size={15} />
                )}
                {t(busy ? 'mcpDiscovering' : 'mcpDiscoverTools')}
              </Button>
            </div>
          ) : null}
        </section>
      ) : null}

      <div className="mcp-server-card-actions flex flex-wrap items-center gap-2 border-t border-border pt-3">
        {server.installed ? (
          <>
            {environmentKeys.length > 0 ? (
              <Button disabled={busy} onClick={onConfigure} size="small" variant="secondary">
                <KeyRound aria-hidden="true" size={15} />
                {t('mcpConfigureEnv')}
              </Button>
            ) : null}
            {server.discoveredTools.length > 0 ? (
              <Button disabled={busy} onClick={() => void onProbe()} size="small" variant="secondary">
                {busy ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                ) : (
                  <RefreshCw aria-hidden="true" size={15} />
                )}
                {t(busy ? 'mcpDiscovering' : 'mcpDiscoverTools')}
              </Button>
            ) : null}
            <Button
              className="text-destructive hover:bg-destructive/10 hover:text-destructive"
              disabled={busy}
              onClick={() => setConfirmRemoval(true)}
              size="small"
              variant="ghost"
            >
              <Trash2 aria-hidden="true" size={15} />
              {t('mcpUninstall')}
            </Button>
            <p className="min-w-0 flex-1 break-all text-right font-mono text-xs text-secondary-text">
              {`%LOCALAPPDATA%\\OpenChat\\mcp\\${server.id}.json`}
            </p>
          </>
        ) : null}
      </div>

      <Dialog onOpenChange={setConfirmRemoval} open={confirmRemoval} title={server.name}>
        <p className="text-sm leading-5 text-secondary-text">{t('mcpUninstallConfirm')}</p>
        <div className="mt-5 flex flex-wrap justify-end gap-2">
          <Button disabled={busy} onClick={() => setConfirmRemoval(false)}>
            {t('cancel')}
          </Button>
          <Button disabled={busy} onClick={() => void confirmAndRemove()} variant="destructive">
            {busy ? <LoaderCircle aria-hidden="true" className="animate-spin" size={15} /> : null}
            {t('mcpUninstall')}
          </Button>
        </div>
      </Dialog>
    </Card>
  );
}
