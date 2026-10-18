import { Eye, EyeOff, KeyRound, LoaderCircle, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog, Input } from '@/design-system';
import type { McpServerRecord } from '@/modules/model-library/data/mcpCatalog';
import type { GlobalMcpSecretStatus, GlobalMcpSecretUpdate } from '@/modules/model-library/data/mcpStorage';

interface McpEnvModalProps {
  open: boolean;
  server: McpServerRecord | null;
  busy: boolean;
  onOpenChange(open: boolean): void;
  onSaved(): void;
  loadStatus(serverId: string, environmentNames: readonly string[]): Promise<GlobalMcpSecretStatus[]>;
  save(serverId: string, updates: readonly GlobalMcpSecretUpdate[]): Promise<boolean>;
}

type ModalErrorCode = 'mcpCredentialStatusFailed' | 'mcpCredentialSaveFailed';

export function McpEnvModal({ open, server, busy, loadStatus, onOpenChange, onSaved, save }: McpEnvModalProps) {
  const { t } = useTranslation();
  const environmentNames = useMemo(
    () => (server ? [...server.requiredEnvKeys, ...server.optionalEnvKeys] : []),
    [server],
  );
  const [statuses, setStatuses] = useState<Record<string, boolean>>({});
  const [values, setValues] = useState<Record<string, string>>({});
  const [visibleNames, setVisibleNames] = useState<string[]>([]);
  const [clearNames, setClearNames] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [errorCode, setErrorCode] = useState<ModalErrorCode | null>(null);

  useEffect(() => {
    if (!open || !server) return;
    let active = true;
    setIsLoading(true);
    setErrorCode(null);
    setStatuses({});
    setValues({});
    setVisibleNames([]);
    setClearNames([]);
    void loadStatus(server.id, environmentNames)
      .then((items) => {
        if (!active) return;
        setStatuses(Object.fromEntries(items.map((item) => [item.environmentName, item.isSaved])));
        setIsLoading(false);
      })
      .catch(() => {
        if (!active) return;
        setErrorCode('mcpCredentialStatusFailed');
        setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [environmentNames, loadStatus, open, server]);

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      setValues({});
      setVisibleNames([]);
      setClearNames([]);
    }
    onOpenChange(nextOpen);
  };

  const saveValues = async () => {
    if (!server || isLoading || isSaving || busy) return;
    const updates = environmentNames.flatMap((environmentName): GlobalMcpSecretUpdate[] => {
      const value = values[environmentName]?.trim() ?? '';
      if (clearNames.includes(environmentName)) return [{ environmentName, value: '' }];
      return value ? [{ environmentName, value }] : [];
    });
    setIsSaving(true);
    setErrorCode(null);
    try {
      const succeeded = await save(server.id, updates);
      if (!succeeded) {
        setErrorCode('mcpCredentialSaveFailed');
        return;
      }
      setStatuses((current) => {
        const next = { ...current };
        for (const update of updates) next[update.environmentName] = update.value.length > 0;
        return next;
      });
      setValues({});
      setClearNames([]);
      onSaved();
      handleOpenChange(false);
    } finally {
      setIsSaving(false);
    }
  };

  const toggleClear = (name: string) => {
    setClearNames((current) => (current.includes(name) ? current.filter((item) => item !== name) : [...current, name]));
    setValues((current) => ({ ...current, [name]: '' }));
  };

  return (
    <Dialog
      className="w-dialog-provider"
      onOpenChange={handleOpenChange}
      open={open && server !== null}
      title={
        <span className="flex items-center gap-2">
          <KeyRound aria-hidden="true" className="shrink-0 text-secondary-icon" size={18} />
          <span>{server ? `${server.name} · ${t('mcpEnvDialogTitle')}` : t('mcpEnvDialogTitle')}</span>
        </span>
      }
    >
      <p className="mb-4 text-sm leading-5 text-secondary-text">{t('mcpEnvDialogDescription')}</p>
      {isLoading ? (
        <p aria-live="polite" className="flex items-center gap-2 py-6 text-secondary-text" role="status">
          <LoaderCircle aria-hidden="true" className="animate-spin" size={17} />
          {t('modelsLoading')}
        </p>
      ) : null}
      {!isLoading && environmentNames.length === 0 ? (
        <p className="py-4 text-sm text-secondary-text">{t('connectorCatalogNoCredentialVariables')}</p>
      ) : null}
      {!isLoading && environmentNames.length > 0 ? (
        <div className="grid max-h-mcp-environment-list gap-4 overflow-y-auto pr-1">
          {environmentNames.map((name) => {
            const required = server?.requiredEnvKeys.includes(name) ?? false;
            const cleared = clearNames.includes(name);
            const visible = visibleNames.includes(name);
            return (
              <div className="grid gap-2" key={name}>
                <label className="text-sm font-medium text-text" htmlFor={`mcp-env-${server?.id}-${name}`}>
                  {name}
                  {required ? ' *' : ''}
                </label>
                <div className="flex items-center gap-2">
                  <Input
                    autoComplete="off"
                    aria-required={required}
                    disabled={busy || isSaving || cleared}
                    id={`mcp-env-${server?.id}-${name}`}
                    maxLength={2500}
                    onChange={(event) => {
                      setValues((current) => ({ ...current, [name]: event.target.value }));
                      if (event.target.value) setClearNames((current) => current.filter((item) => item !== name));
                    }}
                    placeholder={statuses[name] ? t('mcpEnvKeepSavedHint') : ''}
                    type={visible ? 'text' : 'password'}
                    value={values[name] ?? ''}
                  />
                  <Button
                    aria-label={t(visible ? 'hideApiKey' : 'showApiKey')}
                    disabled={busy || isSaving}
                    onClick={() =>
                      setVisibleNames((current) =>
                        current.includes(name) ? current.filter((item) => item !== name) : [...current, name],
                      )
                    }
                    size="icon"
                    variant="ghost"
                  >
                    {visible ? <EyeOff aria-hidden="true" size={17} /> : <Eye aria-hidden="true" size={17} />}
                  </Button>
                </div>
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <p className="text-xs text-secondary-text" role="status">
                    {t(cleared ? 'mcpEnvNotSavedLabel' : statuses[name] ? 'mcpEnvSavedLabel' : 'mcpEnvNotSavedLabel')}
                  </p>
                  {statuses[name] ? (
                    <Button
                      aria-pressed={cleared}
                      disabled={busy || isSaving}
                      onClick={() => toggleClear(name)}
                      size="small"
                      variant={cleared ? 'destructive' : 'ghost'}
                    >
                      <Trash2 aria-hidden="true" size={14} />
                      {t(cleared ? 'cancel' : 'mcpEnvClearAction')}
                    </Button>
                  ) : null}
                </div>
              </div>
            );
          })}
        </div>
      ) : null}
      {errorCode ? (
        <p className="mt-3 text-sm text-destructive" role="alert">
          {t(errorCode)}
        </p>
      ) : null}
      <div className="mt-5 flex flex-wrap justify-end gap-2">
        <Button disabled={isSaving} onClick={() => handleOpenChange(false)}>
          {t('cancel')}
        </Button>
        <Button
          disabled={busy || isLoading || isSaving || environmentNames.length === 0}
          onClick={() => void saveValues()}
          variant="primary"
        >
          {isSaving ? <LoaderCircle aria-hidden="true" className="animate-spin" size={15} /> : null}
          {t('save')}
        </Button>
      </div>
    </Dialog>
  );
}
