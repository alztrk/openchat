import { type FormEvent, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Field, Input, Select, Textarea } from '@shared/components';
import { open } from '@shared/lib/platform/tauri';
import {
  type ConnectorCatalogEntry,
  type ConnectorTransport,
  parseConnectorEntry,
} from '@features/app-settings/data/connectorCatalog';

type SaveOutcome = 'saved' | 'duplicate' | 'failed';

interface ConnectorTemplateFormProps {
  entry: ConnectorCatalogEntry | null;
  onClose(): void;
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

export function ConnectorTemplateForm({ entry, onClose, onSave }: ConnectorTemplateFormProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<ConnectorDraft>(() => createDraft(entry));
  const [errorKey, setErrorKey] = useState<
    | 'connectorCatalogInvalid'
    | 'connectorCatalogDuplicate'
    | 'connectorCatalogSaveFailed'
    | 'connectorCatalogFilePickerFailed'
    | null
  >(null);
  const [isSaving, setIsSaving] = useState(false);
  const [isChoosingProgram, setIsChoosingProgram] = useState(false);

  const update = <K extends keyof ConnectorDraft>(key: K, value: ConnectorDraft[K]) => {
    setDraft((current) => ({ ...current, [key]: value }));
    setErrorKey(null);
  };

  const chooseProgram = async () => {
    if (isChoosingProgram || isSaving) return;
    setIsChoosingProgram(true);
    try {
      const selected = await open({ directory: false, multiple: false, title: t('connectorCatalogChooseProgramTitle') });
      if (typeof selected === 'string') update('program', selected);
    } catch {
      setErrorKey('connectorCatalogFilePickerFailed');
    } finally {
      setIsChoosingProgram(false);
    }
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
    if (outcome === 'saved') onClose();
    if (outcome === 'duplicate') setErrorKey('connectorCatalogDuplicate');
    if (outcome === 'failed') setErrorKey('connectorCatalogSaveFailed');
  };

  return (
    <div className="grid min-w-0 content-start gap-4 border-t border-border pt-4">
      <h3 className="font-medium text-text">{t(entry ? 'connectorCatalogEdit' : 'connectorTemplateNewTitle')}</h3>
      <form className="grid gap-6" onSubmit={(event) => void submit(event)}>
        <section className="grid gap-3">
          <h4 className="text-sm font-semibold text-text">{t('connectorTemplateIdentityGroup')}</h4>
          <div className="grid min-w-0 gap-4 sm:grid-cols-2">
            <Input
              autoComplete="off"
              description={t('connectorCatalogIdDescription')}
              label={t('connectorCatalogId')}
              maxLength={24}
              onChange={(event) => update('id', event.target.value)}
              placeholder={t('connectorCatalogIdPlaceholder')}
              required
              value={draft.id}
            />
            <Input
              autoComplete="off"
              label={t('connectorCatalogName')}
              maxLength={80}
              onChange={(event) => update('name', event.target.value)}
              placeholder={t('connectorCatalogNamePlaceholder')}
              required
              value={draft.name}
            />
            <Input
              autoComplete="off"
              label={t('connectorCatalogPublisher')}
              maxLength={120}
              onChange={(event) => update('publisher', event.target.value)}
              placeholder={t('connectorCatalogPublisherPlaceholder')}
              required
              value={draft.publisher}
            />
            <Input
              autoComplete="off"
              label={t('connectorCatalogVersion')}
              maxLength={64}
              onChange={(event) => update('version', event.target.value)}
              placeholder={t('connectorCatalogVersionPlaceholder')}
              required
              value={draft.version}
            />
            <Input
              autoComplete="off"
              fieldClassName="sm:col-span-2"
              label={t('connectorCatalogSourceUrl')}
              maxLength={2048}
              onChange={(event) => update('sourceUrl', event.target.value)}
              placeholder={t('connectorCatalogSourceUrlPlaceholder')}
              required
              value={draft.sourceUrl}
            />
          </div>
        </section>

        <section className="grid gap-3">
          <h4 className="text-sm font-semibold text-text">{t('connectorTemplateConnectionGroup')}</h4>
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
            <div className="grid min-w-0 gap-4 sm:grid-cols-2">
              <Field
                className="min-w-0 sm:col-span-2"
                description={t('connectorCatalogProgramDescription')}
                label={t('connectorCatalogProgram')}
              >
                {({ id, 'aria-describedby': describedBy, 'aria-invalid': invalid }) => (
                  <div className="connector-program-control-row grid min-w-0 gap-2 sm:items-center">
                    <Input
                      aria-describedby={describedBy}
                      aria-invalid={invalid}
                      autoComplete="off"
                      fieldLayout="none"
                      id={id}
                      maxLength={4096}
                      onChange={(event) => update('program', event.target.value)}
                      placeholder={t('connectorCatalogProgramPlaceholder')}
                      required
                      value={draft.program}
                    />
                    <Button
                      className="w-full sm:w-auto"
                      disabled={isSaving || isChoosingProgram}
                      loading={isChoosingProgram}
                      onClick={() => void chooseProgram()}
                      type="button"
                    >
                      {t('connectorCatalogChooseProgram')}
                    </Button>
                  </div>
                )}
              </Field>
              <MultilineField
                label={t('connectorCatalogArguments')}
                maxLength={4096}
                onChange={(value) => update('arguments', value)}
                placeholder={t('connectorCatalogArgumentsPlaceholder')}
                rows={3}
                value={draft.arguments}
              />
              <MultilineField
                label={t('connectorCatalogEnvironmentVariables')}
                maxLength={128}
                onChange={(value) => update('environmentVariables', value)}
                placeholder={t('connectorCatalogEnvironmentVariablesPlaceholder')}
                rows={3}
                value={draft.environmentVariables}
              />
            </div>
          ) : (
            <div className="grid min-w-0 gap-4 sm:grid-cols-2">
              <Input
                autoComplete="off"
                fieldClassName="sm:col-span-2"
                label={t('connectorCatalogEndpoint')}
                maxLength={2048}
                onChange={(event) => update('endpoint', event.target.value)}
                placeholder={t('connectorCatalogEndpointPlaceholder')}
                required
                value={draft.endpoint}
              />
              <Input
                autoComplete="off"
                label={t('connectorCatalogAuthVariable')}
                maxLength={128}
                onChange={(event) => update('authEnvironmentVariable', event.target.value)}
                placeholder={t('connectorCatalogAuthVariablePlaceholder')}
                value={draft.authEnvironmentVariable}
              />
            </div>
          )}
        </section>

        <section className="grid gap-3">
          <h4 className="text-sm font-semibold text-text">{t('connectorTemplateDataGroup')}</h4>
          <div className="grid min-w-0 gap-4 sm:grid-cols-2">
            <MultilineField
              label={t('connectorCatalogRequestedAccess')}
              maxLength={500}
              onChange={(value) => update('requestedAccess', value)}
              placeholder={t('connectorCatalogRequestedAccessPlaceholder')}
              required
              rows={3}
              value={draft.requestedAccess}
            />
            <MultilineField
              label={t('connectorCatalogDataDisclosure')}
              maxLength={500}
              onChange={(value) => update('dataDisclosure', value)}
              placeholder={t('connectorCatalogDataDisclosurePlaceholder')}
              required
              rows={3}
              value={draft.dataDisclosure}
            />
          </div>
        </section>

        {errorKey ? (
          <p className="text-sm text-destructive" role="alert">
            {t(errorKey)}
          </p>
        ) : null}

        <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-4">
          <Button disabled={isSaving} onClick={onClose} type="button">
            {t('cancel')}
          </Button>
          <Button disabled={isSaving} type="submit">
            {t('connectorCatalogSave')}
          </Button>
        </div>
      </form>
    </div>
  );
}

function MultilineField({
  label,
  maxLength,
  onChange,
  placeholder,
  required = false,
  rows = 4,
  value,
}: {
  label: string;
  maxLength: number;
  onChange(value: string): void;
  placeholder: string;
  required?: boolean;
  rows?: number;
  value: string;
}) {
  return (
    <Textarea
      className="bg-raised"
      label={label}
      maxLength={maxLength}
      onChange={(event) => onChange(event.target.value)}
      placeholder={placeholder}
      required={required}
      rows={rows}
      value={value}
    />
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
    ...(currentEntry?.manifest === undefined ? {} : { manifest: currentEntry.manifest }),
  };
}
