import { type FormEvent, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Dialog, Input, Select } from '@/design-system';
import {
  type ConnectorCatalogEntry,
  type ConnectorTransport,
  parseConnectorEntry,
} from '@/modules/settings/data/connectorCatalog';

type SaveOutcome = 'saved' | 'duplicate' | 'failed';

interface ConnectorTemplateDialogProps {
  entry: ConnectorCatalogEntry | null;
  onOpenChange(open: boolean): void;
  onSave(entry: ConnectorCatalogEntry): Promise<SaveOutcome>;
}

interface ConnectorDraft {
  id: string;
  name: string;
  publisher: string;
  version: string;
  sourceUrl: string;
  requestedAccess: string;
  dataDisclosure: string;
  transport: ConnectorTransport;
  program: string;
  arguments: string;
  environmentVariables: string;
  endpoint: string;
  authEnvironmentVariable: string;
}

export function ConnectorTemplateDialog({ entry, onOpenChange, onSave }: ConnectorTemplateDialogProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<ConnectorDraft>(() => createDraft(entry));
  const [errorKey, setErrorKey] = useState<
    'connectorCatalogInvalid' | 'connectorCatalogDuplicate' | 'connectorCatalogSaveFailed' | null
  >(null);
  const [isSaving, setIsSaving] = useState(false);

  const update = <K extends keyof ConnectorDraft>(key: K, value: ConnectorDraft[K]) => {
    setDraft((current) => ({ ...current, [key]: value }));
    setErrorKey(null);
  };

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (isSaving) return;

    let parsed: ConnectorCatalogEntry;
    try {
      parsed = parseConnectorEntry(toEntry(draft, entry));
    } catch {
      setErrorKey('connectorCatalogInvalid');
      return;
    }

    setIsSaving(true);
    const outcome = await onSave(parsed);
    setIsSaving(false);
    if (outcome === 'saved') onOpenChange(false);
    if (outcome === 'duplicate') setErrorKey('connectorCatalogDuplicate');
    if (outcome === 'failed') setErrorKey('connectorCatalogSaveFailed');
  };

  return (
    <Dialog
      className="max-h-dialog-connector overflow-y-auto"
      onOpenChange={onOpenChange}
      open
      title={t(entry ? 'connectorCatalogEdit' : 'connectorCatalogAdd')}
    >
      <form className="grid gap-4" onSubmit={(event) => void submit(event)}>
        <div className="grid gap-4 sm:grid-cols-2">
          <Input
            autoComplete="off"
            label={t('connectorCatalogId')}
            maxLength={24}
            onChange={(event) => update('id', event.target.value)}
            required
            value={draft.id}
          />
          <Input
            autoComplete="off"
            label={t('connectorCatalogName')}
            maxLength={80}
            onChange={(event) => update('name', event.target.value)}
            required
            value={draft.name}
          />
          <Input
            autoComplete="off"
            label={t('connectorCatalogPublisher')}
            maxLength={120}
            onChange={(event) => update('publisher', event.target.value)}
            required
            value={draft.publisher}
          />
          <Input
            autoComplete="off"
            label={t('connectorCatalogVersion')}
            maxLength={64}
            onChange={(event) => update('version', event.target.value)}
            required
            value={draft.version}
          />
          <Input
            autoComplete="off"
            className="sm:col-span-2"
            label={t('connectorCatalogSourceUrl')}
            maxLength={2048}
            onChange={(event) => update('sourceUrl', event.target.value)}
            required
            value={draft.sourceUrl}
          />
          <MultilineField
            label={t('connectorCatalogDataDisclosure')}
            maxLength={500}
            onChange={(value) => update('dataDisclosure', value)}
            required
            value={draft.dataDisclosure}
          />
          <MultilineField
            label={t('connectorCatalogRequestedAccess')}
            maxLength={500}
            onChange={(value) => update('requestedAccess', value)}
            required
            value={draft.requestedAccess}
          />
          <Select
            label={t('connectorCatalogTransport')}
            onChange={(event) =>
              update('transport', event.target.value === 'streamableHttp' ? 'streamableHttp' : 'stdio')
            }
            options={[
              { value: 'stdio', label: t('projectMcpTransportStdio') },
              { value: 'streamableHttp', label: t('projectMcpTransportHttp') },
            ]}
            value={draft.transport}
          />
          {draft.transport === 'stdio' ? (
            <>
              <Input
                autoComplete="off"
                className="sm:col-span-2"
                label={t('connectorCatalogProgram')}
                maxLength={4096}
                onChange={(event) => update('program', event.target.value)}
                required
                value={draft.program}
              />
              <MultilineField
                label={t('connectorCatalogArguments')}
                maxLength={4096}
                onChange={(value) => update('arguments', value)}
                rows={3}
                value={draft.arguments}
              />
              <MultilineField
                label={t('connectorCatalogEnvironmentVariables')}
                maxLength={128}
                onChange={(value) => update('environmentVariables', value)}
                rows={3}
                value={draft.environmentVariables}
              />
            </>
          ) : (
            <>
              <Input
                autoComplete="off"
                className="sm:col-span-2"
                label={t('connectorCatalogEndpoint')}
                maxLength={2048}
                onChange={(event) => update('endpoint', event.target.value)}
                required
                value={draft.endpoint}
              />
              <Input
                autoComplete="off"
                className="sm:col-span-2"
                label={t('connectorCatalogAuthVariable')}
                maxLength={128}
                onChange={(event) => update('authEnvironmentVariable', event.target.value)}
                value={draft.authEnvironmentVariable}
              />
            </>
          )}
        </div>

        {errorKey ? (
          <p className="text-sm text-destructive" role="alert">
            {t(errorKey)}
          </p>
        ) : null}

        <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-4">
          <Button disabled={isSaving} onClick={() => onOpenChange(false)} type="button">
            {t('cancel')}
          </Button>
          <Button disabled={isSaving} type="submit">
            {t('connectorCatalogSave')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

function MultilineField({
  label,
  maxLength,
  onChange,
  required = false,
  rows = 4,
  value,
}: {
  label: string;
  maxLength: number;
  onChange(value: string): void;
  required?: boolean;
  rows?: number;
  value: string;
}) {
  return (
    <label className="grid gap-2 text-sm font-medium text-text">
      {label}
      <textarea
        className="min-h-24 w-full resize-y rounded-control border border-border bg-raised px-3 py-2 text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
        maxLength={maxLength}
        onChange={(event) => onChange(event.target.value)}
        required={required}
        rows={rows}
        value={value}
      />
    </label>
  );
}

function createDraft(entry: ConnectorCatalogEntry | null): ConnectorDraft {
  return {
    id: entry?.id ?? '',
    name: entry?.name ?? '',
    publisher: entry?.publisher ?? '',
    version: entry?.version ?? '',
    sourceUrl: entry?.sourceUrl ?? '',
    requestedAccess: entry?.requestedAccess ?? '',
    dataDisclosure: entry?.dataDisclosure ?? '',
    transport: entry?.transport ?? 'stdio',
    program: entry?.program ?? '',
    arguments: entry?.arguments.join('\n') ?? '',
    environmentVariables: entry?.environmentVariables.join('\n') ?? '',
    endpoint: entry?.endpoint ?? '',
    authEnvironmentVariable: entry?.authEnvironmentVariable ?? '',
  };
}

function toEntry(draft: ConnectorDraft, currentEntry: ConnectorCatalogEntry | null): unknown {
  const lines = (value: string) =>
    value
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
  return {
    id: draft.id,
    name: draft.name,
    publisher: draft.publisher,
    version: draft.version,
    sourceUrl: draft.sourceUrl,
    requestedAccess: draft.requestedAccess,
    dataDisclosure: draft.dataDisclosure,
    transport: draft.transport,
    arguments: draft.transport === 'stdio' ? lines(draft.arguments) : [],
    environmentVariables: draft.transport === 'stdio' ? lines(draft.environmentVariables) : [],
    ...(draft.transport === 'stdio' && draft.program.trim() ? { program: draft.program.trim() } : {}),
    ...(draft.transport === 'streamableHttp' && draft.endpoint.trim() ? { endpoint: draft.endpoint.trim() } : {}),
    ...(draft.transport === 'streamableHttp' && draft.authEnvironmentVariable.trim()
      ? { authEnvironmentVariable: draft.authEnvironmentVariable.trim() }
      : {}),
    ...(currentEntry?.package === undefined
      ? {}
      : { package: currentEntry.package, catalogTrust: currentEntry.catalogTrust }),
  };
}
