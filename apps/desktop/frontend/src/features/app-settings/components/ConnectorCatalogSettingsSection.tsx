import type { TFunction } from 'i18next';
import { AlertCircle, Download, LoaderCircle, Pencil, Plus, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Input } from '@shared/components';
import { useChatStore } from '@features/chat/state';
import { whenUnifiedStorageReady } from '@shared/lib/platform/storage/unifiedStorage';
import { type ProjectMcpServer, parseProjectMcpCatalog } from '@entities/project/projectMcp';
import {
  loadProjectToolPermissionRules,
  saveProjectToolPermissionRules,
} from '@entities/project/projectPermissionSettings';
import {
  type ConnectorCatalogEntry,
  type ConnectorCatalogTrust,
  type ConnectorExecutablePackage,
  readConnectorCatalog,
  writeConnectorCatalog,
} from '@features/app-settings/data/connectorCatalog';
import type { ConnectorManifest } from '@shared/lib/preferences/connectorManifest';
import { localServiceClient } from '@shared/lib/platform/local-service';
import { ConnectorCatalogSourcesPanel } from './ConnectorCatalogSourcesPanel';
import { ConnectorTemplateForm } from './ConnectorTemplateForm';
import { SettingsSectionSkeleton } from './SettingsSectionSkeleton';

type LoadState = 'loading' | 'ready' | 'failed';
type SaveOutcome = 'saved' | 'duplicate' | 'failed';

class DuplicateProjectConnectorError extends Error {
  constructor() {
    super('The connector is already installed in this project.');
    this.name = 'DuplicateProjectConnectorError';
  }
}

class ConnectorPackageCleanupError extends Error {
  constructor() {
    super('The newly downloaded connector package could not be removed after installation failed.');
    this.name = 'ConnectorPackageCleanupError';
  }
}

export function ConnectorCatalogSettingsSection() {
  const { t } = useTranslation();
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const pendingProjectId = useChatStore((state) => state.pendingProjectId);
  const conversations = useChatStore((state) => state.conversations);
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [entries, setEntries] = useState<ConnectorCatalogEntry[]>([]);
  const [search, setSearch] = useState('');
  const [editorEntry, setEditorEntry] = useState<ConnectorCatalogEntry | null | undefined>(undefined);
  const [installEntryTarget, setInstallEntryTarget] = useState<ConnectorCatalogEntry | null>(null);
  const [deleteEntry, setDeleteEntry] = useState<ConnectorCatalogEntry | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [installFeedback, setInstallFeedback] = useState<'installed' | null>(null);
  const [installErrorKey, setInstallErrorKey] = useState<
    | 'connectorCatalogDuplicate'
    | 'connectorCatalogInstallFailed'
    | 'connectorPackageCleanupFailed'
    | 'connectorPackageRollbackCleanupFailed'
    | null
  >(null);
  const [packageCleanupRetry, setPackageCleanupRetry] = useState<{
    id: string;
    package: NonNullable<ProjectMcpServer['package']>;
    errorKey: 'connectorPackageCleanupFailed' | 'connectorPackageRollbackCleanupFailed';
  } | null>(null);
  const [errorKey, setErrorKey] = useState<'connectorCatalogLoadFailed' | 'connectorCatalogSaveFailed' | null>(null);

  const activeProjectId =
    pendingProjectId ??
    conversations.find((conversation) => conversation.id === activeConversationId)?.projectId ??
    null;

  // biome-ignore lint/correctness/useExhaustiveDependencies: Incrementing loadAttempt is the explicit retry signal for this request.
  useEffect(() => {
    setLoadState('loading');
    setErrorKey(null);
    return whenUnifiedStorageReady(() => {
      try {
        setEntries(readConnectorCatalog());
        setLoadState('ready');
      } catch {
        setLoadState('failed');
        setErrorKey('connectorCatalogLoadFailed');
      }
    });
  }, [loadAttempt]);

  const visibleEntries = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    if (!query) return entries;
    return entries.filter((entry) =>
      [entry.name, entry.id, entry.publisher, entry.sourceUrl, entry.dataDisclosure].some((value) =>
        value.toLocaleLowerCase().includes(query),
      ),
    );
  }, [entries, search]);

  const saveEntry = async (entry: ConnectorCatalogEntry): Promise<SaveOutcome> => {
    if (loadState !== 'ready' || isSaving) return 'failed';
    const editingId = editorEntry?.id;
    const nextEntries =
      editingId === undefined
        ? [...entries, entry]
        : entries.map((current) =>
            current.id === editingId
              ? {
                  ...entry,
                  ...(entry.package === undefined && current.package !== undefined
                    ? { package: current.package, catalogTrust: current.catalogTrust }
                    : {}),
                }
              : current,
          );
    const duplicateId = nextEntries.some((current, index) =>
      nextEntries.some(
        (other, otherIndex) =>
          index !== otherIndex &&
          (current.id === other.id ||
            current.name.toLocaleLowerCase('en-US') === other.name.toLocaleLowerCase('en-US')),
      ),
    );
    if (duplicateId) return 'duplicate';

    setIsSaving(true);
    setErrorKey(null);
    try {
      await writeConnectorCatalog(nextEntries);
      setEntries(nextEntries);
      return 'saved';
    } catch {
      setErrorKey('connectorCatalogSaveFailed');
      return 'failed';
    } finally {
      setIsSaving(false);
    }
  };

  const confirmDelete = async () => {
    const target = deleteEntry;
    if (!target || isSaving) return;
    setIsSaving(true);
    setErrorKey(null);
    try {
      const nextEntries = entries.filter((entry) => entry.id !== target.id);
      await writeConnectorCatalog(nextEntries);
      setEntries(nextEntries);
      setDeleteEntry(null);
    } catch {
      setErrorKey('connectorCatalogSaveFailed');
    } finally {
      setIsSaving(false);
    }
  };

  const installEntry = async (entry: ConnectorCatalogEntry) => {
    const projectId = activeProjectId;
    if (serviceStatus !== 'ready' || projectId === null || installingId !== null) return;
    if (entry.package !== undefined && entry.package.authenticodeSignerSha256 === undefined) {
      setInstallErrorKey('connectorCatalogInstallFailed');
      return;
    }

    setInstallingId(entry.id);
    setInstallFeedback(null);
    setInstallErrorKey(null);
    setPackageCleanupRetry(null);
    setErrorKey(null);
    try {
      const existingResponse = await localServiceClient.call('project.mcp.catalog.get', { projectId });
      const existingCatalog = parseProjectMcpCatalog(existingResponse);
      const existingServer = existingCatalog.servers.find((server) => server.id === entry.id);
      const isPackageUpdate =
        existingServer?.package !== undefined &&
        entry.package !== undefined &&
        existingServer.package.version !== entry.version;
      if (existingServer && !isPackageUpdate) {
        throw new DuplicateProjectConnectorError();
      }

      const packageReference =
        entry.package === undefined
          ? undefined
          : expectedPackageReference(entry.version, entry.package, entry.catalogTrust);
      let previousRules: Awaited<ReturnType<typeof loadProjectToolPermissionRules>> | null = null;
      try {
        if (entry.package !== undefined && packageReference !== undefined) {
          await installSignedPackage(entry.id, entry.version, projectId, entry.package, entry.catalogTrust);
        }

        previousRules = await loadProjectToolPermissionRules(projectId);
        const nextRules = { ...previousRules };
        if (entry.package !== undefined) {
          for (const name of Object.keys(nextRules)) {
            if (name.startsWith(`mcp__${entry.id}__`)) delete nextRules[name];
          }
        }
        nextRules[`mcp__${entry.id}__*`] = 'ask';
        await saveProjectToolPermissionRules(projectId, nextRules);
        const server = projectServerFromEntry(entry, packageReference);
        const catalog = parseProjectMcpCatalog({
          version: 1,
          servers: existingServer
            ? existingCatalog.servers.map((item) => (item.id === entry.id ? server : item))
            : [...existingCatalog.servers, server],
        });
        await localServiceClient.call('project.mcp.catalog.save', {
          projectId,
          catalog,
        });
      } catch (saveError) {
        let permissionRollbackFailed = false;
        let packageCleanupFailed = false;
        if (previousRules !== null) {
          try {
            await saveProjectToolPermissionRules(projectId, previousRules);
          } catch {
            permissionRollbackFailed = true;
          }
        }
        try {
          if (packageReference) await removeSignedPackage(projectId, entry.id, packageReference);
        } catch {
          packageCleanupFailed = true;
        }
        if (packageCleanupFailed && packageReference) {
          setPackageCleanupRetry({
            id: entry.id,
            package: packageReference,
            errorKey: 'connectorPackageRollbackCleanupFailed',
          });
          throw new ConnectorPackageCleanupError();
        }
        if (permissionRollbackFailed) {
          throw new Error('Connector installation rollback failed.', { cause: saveError });
        }
        throw saveError;
      }
      if (existingServer?.package && packageReference && !samePackage(existingServer.package, packageReference)) {
        let cleanupFailed = false;
        try {
          await removeSignedPackage(projectId, entry.id, existingServer.package);
        } catch {
          cleanupFailed = true;
          setInstallErrorKey('connectorPackageCleanupFailed');
          setPackageCleanupRetry({
            id: entry.id,
            package: existingServer.package,
            errorKey: 'connectorPackageCleanupFailed',
          });
        }
        if (cleanupFailed) {
          setInstallFeedback(null);
        } else {
          setInstallFeedback('installed');
        }
      } else {
        setInstallFeedback('installed');
      }
    } catch (error) {
      if (error instanceof DuplicateProjectConnectorError) {
        setInstallErrorKey('connectorCatalogDuplicate');
      } else if (error instanceof ConnectorPackageCleanupError) {
        setInstallErrorKey('connectorPackageRollbackCleanupFailed');
      } else {
        setInstallErrorKey('connectorCatalogInstallFailed');
      }
    } finally {
      setInstallingId(null);
    }
  };

  return (
    <section aria-labelledby="settings-page-heading" className="settings-section grid content-start gap-6">
      {installFeedback ? (
        <p aria-live="polite" className="text-sm text-success" role="status">
          {t('connectorCatalogInstalled')}
        </p>
      ) : null}
      {installErrorKey ? (
        <div className="flex flex-wrap items-center gap-3" role="alert">
          <p className="text-sm text-destructive">{t(installErrorKey)}</p>
          {packageCleanupRetry ? (
            <Button
              disabled={installingId !== null || activeProjectId === null}
              onClick={() => {
                const retry = packageCleanupRetry;
                const projectId = activeProjectId;
                if (!retry || !projectId) return;
                setInstallingId(retry.id);
                void removeSignedPackage(projectId, retry.id, retry.package)
                  .then(() => {
                    setPackageCleanupRetry(null);
                    setInstallErrorKey(null);
                  })
                  .catch(() => setInstallErrorKey(retry.errorKey))
                  .finally(() => setInstallingId(null));
              }}
              size="small"
              variant="secondary"
            >
              {t('retry')}
            </Button>
          ) : null}
        </div>
      ) : null}

      <div className="settings-section-group grid">
        <div className="flex flex-wrap items-end justify-between gap-3 px-1">
          <div className="min-w-0">
            <h2 className="text-sm font-semibold text-secondary-text">{t('connectorCatalogTemplatesTitle')}</h2>
            <p className="mt-1 text-sm text-secondary-text">{t('connectorCatalogTemplatesDescription')}</p>
          </div>
          {loadState !== 'failed' && editorEntry === undefined ? (
            <Button
              disabled={loadState !== 'ready' || isSaving}
              onClick={() => {
                setInstallEntryTarget(null);
                setDeleteEntry(null);
                setEditorEntry(null);
              }}
              size="small"
              variant="secondary"
            >
              <Plus aria-hidden="true" size={16} />
              {t('connectorCatalogAdd')}
            </Button>
          ) : null}
        </div>
        <div
          className={`connector-catalog-surface settings-surface-card rounded-card border border-border bg-raised p-4 sm:p-5 ${
            editorEntry !== undefined && entries.length > 0 ? 'connector-catalog-surface--editing' : ''
          }`}
        >
          {loadState === 'failed' ? (
            <div className="grid gap-3" role="alert">
              <p className="flex items-center gap-2 text-sm text-destructive">
                <AlertCircle aria-hidden="true" size={16} />
                {t(errorKey ?? 'connectorCatalogLoadFailed')}
              </p>
              <Button
                className="justify-self-start"
                onClick={() => setLoadAttempt((attempt) => attempt + 1)}
                size="small"
              >
                {t('retry')}
              </Button>
            </div>
          ) : (
            <>
              {editorEntry !== undefined ? (
                <ConnectorTemplateForm
                  entry={editorEntry}
                  key={editorEntry?.id ?? 'new-template'}
                  onClose={() => setEditorEntry(undefined)}
                  onSave={saveEntry}
                />
              ) : null}
              <div className={`connector-catalog-list ${editorEntry !== undefined ? 'connector-catalog-list--editing' : ''}`}>
                {loadState !== 'loading' && entries.length > 0 ? (
                  <Input
                    label={t('connectorCatalogSearch')}
                    onChange={(event) => setSearch(event.target.value)}
                    value={search}
                  />
                ) : null}
                {loadState === 'loading' ? (
                  <SettingsSectionSkeleton label={t('settingsSectionLoading')} />
                ) : entries.length === 0 ? (
                  editorEntry === undefined ? <p className="text-sm text-secondary-text">{t('connectorCatalogEmpty')}</p> : null
                ) : visibleEntries.length === 0 ? (
                <p className="text-sm text-secondary-text">{t('connectorCatalogNoResults')}</p>
                ) : (
                <div className="connector-catalog-entries grid gap-3">
                  {visibleEntries.map((entry) => (
                    <ConnectorEntryCard
                      canInstall={
                        serviceStatus === 'ready' &&
                        activeProjectId !== null &&
                        installingId === null &&
                        (entry.package === undefined || entry.package.authenticodeSignerSha256 !== undefined)
                      }
                      entry={entry}
                      deletePending={deleteEntry?.id === entry.id}
                      installPending={installEntryTarget?.id === entry.id}
                      installing={installingId === entry.id}
                      key={entry.id}
                      onCancelDelete={() => setDeleteEntry(null)}
                      onCancelInstall={() => setInstallEntryTarget(null)}
                      onConfirmDelete={confirmDelete}
                      onConfirmInstall={() => {
                        setInstallEntryTarget(null);
                        void installEntry(entry);
                      }}
                      onDelete={() => {
                        setEditorEntry(undefined);
                        setInstallEntryTarget(null);
                        setDeleteEntry(entry);
                      }}
                      onEdit={() => {
                        setDeleteEntry(null);
                        setInstallEntryTarget(null);
                        setEditorEntry(entry);
                      }}
                      onInstall={() => {
                        setEditorEntry(undefined);
                        setDeleteEntry(null);
                        setInstallEntryTarget(entry);
                      }}
                      saving={isSaving}
                      t={t}
                    />
                  ))}
                </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>

      {errorKey === 'connectorCatalogSaveFailed' ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}

      <ConnectorCatalogSourcesPanel
        canRefresh={serviceStatus === 'ready'}
        existingEntries={entries}
        onImport={saveEntry}
      />

    </section>
  );
}

function ConnectorEntryCard({
  canInstall,
  deletePending,
  entry,
  installPending,
  installing,
  onCancelDelete,
  onCancelInstall,
  onConfirmDelete,
  onConfirmInstall,
  onDelete,
  onEdit,
  onInstall,
  saving,
  t,
}: {
  canInstall: boolean;
  deletePending: boolean;
  entry: ConnectorCatalogEntry;
  installPending: boolean;
  onCancelDelete(): void;
  onCancelInstall(): void;
  onConfirmDelete(): void;
  onConfirmInstall(): void;
  onDelete(): void;
  onEdit(): void;
  onInstall(): void;
  installing: boolean;
  saving: boolean;
  t: TFunction;
}) {
  const missingSignerFingerprint = entry.package !== undefined && entry.package.authenticodeSignerSha256 === undefined;
  const signerNoticeId = `connector-catalog-signer-unavailable-${entry.id}`;
  const describedBy = missingSignerFingerprint ? signerNoticeId : undefined;

  return (
    <Card className="grid gap-4 sm:p-5" padding="default">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="break-words text-base font-semibold">{entry.name}</h3>
          <p className="mt-1 break-all font-mono text-xs text-secondary-text">{entry.id}</p>
        </div>
        <div className="flex gap-1">
          <Button aria-label={t('connectorCatalogEdit')} onClick={onEdit} size="icon" title={t('connectorCatalogEdit')}>
            <Pencil aria-hidden="true" size={16} />
          </Button>
          <Button
            aria-label={t('connectorCatalogDelete')}
            onClick={onDelete}
            size="icon"
            title={t('connectorCatalogDelete')}
            variant="ghost"
          >
            <Trash2 aria-hidden="true" size={16} />
          </Button>
        </div>
      </div>

      <dl className="grid gap-3 text-sm sm:grid-cols-2">
        <ConnectorDetail label={t('connectorCatalogPublisher')} value={entry.publisher} />
        <ConnectorDetail label={t('connectorCatalogVersion')} value={entry.version} />
        <ConnectorDetail label={t('connectorCatalogSourceUrl')} value={entry.sourceUrl} />
        <ConnectorDetail
          label={t('connectorCatalogTransport')}
          value={t(entry.transport === 'stdio' ? 'projectMcpTransportStdio' : 'projectMcpTransportHttp')}
        />
        <ConnectorDetail label={t('connectorCatalogRequestedAccess')} value={entry.requestedAccess} />
        <ConnectorDetail label={t('connectorCatalogDataDisclosure')} value={entry.dataDisclosure} />
      </dl>
      {missingSignerFingerprint ? (
        <p className="text-sm text-secondary-text" id={signerNoticeId}>
          {t('connectorPackageSignerUnavailable')}
        </p>
      ) : null}

      <Button
        aria-describedby={describedBy}
        className="justify-self-start"
        disabled={!canInstall || installing || saving}
        onClick={onInstall}
      >
        {installing ? (
          <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
        ) : (
          <Download aria-hidden="true" size={16} />
        )}
        {t('connectorCatalogInstall')}
      </Button>
      {installPending ? (
        <ConnectorInstallConfirmation entry={entry} onCancel={onCancelInstall} onConfirm={onConfirmInstall} />
      ) : null}
      {deletePending ? (
        <fieldset className="grid gap-3 border-t border-border pt-4">
          <legend className="sr-only">{t('connectorCatalogDelete')}</legend>
          <p className="text-sm text-secondary-text">{t('connectorCatalogDeleteConfirm')}</p>
          <div className="flex flex-wrap justify-end gap-2">
            <Button disabled={saving} onClick={onCancelDelete} variant="ghost">
              {t('cancel')}
            </Button>
            <Button disabled={saving} onClick={onConfirmDelete} variant="destructive">
              {saving ? (
                <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
              ) : (
                <Trash2 aria-hidden="true" size={16} />
              )}
              {t('connectorCatalogDelete')}
            </Button>
          </div>
        </fieldset>
      ) : null}
    </Card>
  );
}

function ConnectorInstallConfirmation({
  entry,
  onCancel,
  onConfirm,
}: {
  entry: ConnectorCatalogEntry;
  onCancel(): void;
  onConfirm(): void;
}) {
  const { i18n, t } = useTranslation();
  const authentication =
    entry.transport === 'streamableHttp'
      ? entry.authEnvironmentVariable === undefined
        ? t('connectorCatalogNoBearerCredential')
        : t('connectorCatalogProjectBearerCredential')
      : entry.environmentVariables.length === 0
        ? t('connectorCatalogNoCredentialVariables')
        : t('connectorCatalogProjectEnvironmentCredentials');

  return (
    <section aria-label={t('connectorCatalogInstall')} className="grid gap-3 border-t border-border pt-4">
      <h4 className="font-medium text-text">{t('connectorCatalogInstall')}</h4>
        <p className="text-sm text-secondary-text">{t('connectorCatalogInstallPrompt')}</p>
        {entry.package ? <p className="text-sm text-secondary-text">{t('connectorPackageDownloadPrompt')}</p> : null}
        <dl className="grid gap-2 text-sm">
          <ConnectorDetail label={t('connectorCatalogPublisher')} value={entry.publisher} />
          <ConnectorDetail label={t('connectorCatalogVersion')} value={entry.version} />
          <ConnectorDetail label={t('connectorCatalogSourceUrl')} value={entry.sourceUrl} />
          <ConnectorDetail
            label={t('projectMcpEndpoint')}
            value={entry.endpoint ?? entry.package?.url ?? entry.program ?? ''}
          />
          <ConnectorDetail label={t('connectorCatalogAuthentication')} value={authentication} />
          {entry.authEnvironmentVariable ? (
            <ConnectorDetail label={t('connectorCatalogAuthVariable')} value={entry.authEnvironmentVariable} />
          ) : null}
          {entry.transport === 'stdio' && entry.environmentVariables.length > 0 ? (
            <ConnectorDetail
              label={t('connectorCatalogEnvironmentVariables')}
              value={entry.environmentVariables.join(', ')}
            />
          ) : null}
          <ConnectorDetail label={t('connectorCatalogRequestedAccess')} value={entry.requestedAccess} />
          <ConnectorDetail label={t('connectorCatalogDataDisclosure')} value={entry.dataDisclosure} />
        </dl>
        {entry.manifest ? <ConnectorManifestDisclosure manifest={entry.manifest} /> : null}
        <p className="text-sm text-secondary-text">{t('connectorCatalogRemovalImpact')}</p>
        {entry.package && entry.catalogTrust ? (
          <>
            <dl className="grid gap-2 text-sm">
              <ConnectorDetail
                label={t('connectorPackageSize')}
                value={`${new Intl.NumberFormat(i18n.resolvedLanguage ?? i18n.language).format(
                  Math.ceil(entry.package.sizeBytes / 1024),
                )} KB`}
              />
              <ConnectorDetail label={t('connectorPackageSha256')} value={entry.package.sha256} />
              {entry.package.authenticodeSignerSha256 !== undefined ? (
                <ConnectorDetail label={t('connectorPackageSigner')} value={entry.package.authenticodeSignerSha256} />
              ) : null}
              <ConnectorDetail label={t('connectorPackageSourceKey')} value={entry.catalogTrust.keyFingerprint} />
            </dl>
            <p className="text-sm text-secondary-text">{t('connectorPackageTrustNotice')}</p>
          </>
        ) : null}
        <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
          <Button onClick={onCancel} variant="ghost">{t('cancel')}</Button>
          <Button onClick={onConfirm} variant="primary">
            {t('connectorCatalogInstall')}
          </Button>
        </div>
    </section>
  );
}

function ConnectorManifestDisclosure({ manifest }: { manifest: ConnectorManifest }) {
  const { t } = useTranslation();
  return (
    <details className="grid gap-3 rounded-control border border-border p-3" open>
      <summary className="cursor-pointer text-sm font-medium text-text">
        {t('connectorManifestHeading', { version: manifest.version })}
      </summary>
      <p className="text-sm text-secondary-text">{t('connectorManifestNotice')}</p>
      <ManifestList
        label={t('connectorManifestTools')}
        values={manifest.tools.map((tool) =>
          tool.description === undefined ? tool.name : `${tool.name}: ${tool.description}`,
        )}
      />
      <ManifestList label={t('connectorManifestPermissions')} values={manifest.permissions} />
      <ManifestList label={t('connectorManifestProviders')} values={manifest.providers} />
      <ManifestList
        label={t('connectorManifestConfiguration')}
        values={manifest.configuration.map((item) => {
          const requirements = [
            item.required ? t('connectorManifestRequired') : t('connectorManifestOptional'),
            ...(item.secret ? [t('connectorManifestSecret')] : []),
          ];
          return `${item.name}: ${item.description} (${requirements.join(', ')})`;
        })}
      />
    </details>
  );
}

function ManifestList({ label, values }: { label: string; values: string[] }) {
  const { t } = useTranslation();
  return (
    <section className="grid gap-1 text-sm">
      <h3 className="font-medium text-text">{label}</h3>
      {values.length === 0 ? (
        <p className="text-secondary-text">{t('connectorManifestNoDeclarations')}</p>
      ) : (
        <ul className="list-inside list-disc text-secondary-text">
          {values.map((value) => (
            <li className="break-words" key={value}>
              {value}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function projectServerFromEntry(
  entry: ConnectorCatalogEntry,
  packageReference?: ProjectMcpServer['package'],
): ProjectMcpServer {
  return {
    id: entry.id,
    enabled: false,
    transport: entry.transport,
    arguments: entry.arguments,
    environmentVariables: entry.environmentVariables,
    ...(entry.package === undefined && entry.program !== undefined ? { program: entry.program } : {}),
    ...(entry.endpoint === undefined ? {} : { endpoint: entry.endpoint }),
    ...(entry.authEnvironmentVariable === undefined ? {} : { authEnvironmentVariable: entry.authEnvironmentVariable }),
    ...(packageReference === undefined ? {} : { package: packageReference }),
    ...(entry.manifest === undefined ? {} : { catalogToolNames: entry.manifest.tools.map((tool) => tool.name) }),
  };
}

async function installSignedPackage(
  id: string,
  version: string,
  projectId: string,
  packageMetadata: ConnectorExecutablePackage,
  trust: ConnectorCatalogTrust | undefined,
): Promise<NonNullable<ProjectMcpServer['package']>> {
  if (trust === undefined) throw new Error('The package has no verified catalog source.');
  const authenticodeSignerSha256 = packageMetadata.authenticodeSignerSha256;
  if (authenticodeSignerSha256 === undefined) throw new Error('The package has no signed signer fingerprint.');
  const response = await localServiceClient.call(
    'connector.package.install_signed',
    {
      projectId,
      sourceUrl: trust.sourceUrl,
      publicKeyBase64: trust.publicKeyBase64,
      id,
      version,
      expectedUrl: packageMetadata.url,
      expectedSha256: packageMetadata.sha256,
      expectedAuthenticodeSignerSha256: authenticodeSignerSha256,
      expectedSizeBytes: packageMetadata.sizeBytes,
    },
    180_000,
  );
  if (
    !isRecord(response) ||
    response.version !== version ||
    response.sha256 !== packageMetadata.sha256 ||
    response.authenticodeSignerSha256 !== authenticodeSignerSha256 ||
    response.sizeBytes !== packageMetadata.sizeBytes ||
    response.sourceKeyFingerprint !== trust.keyFingerprint
  ) {
    throw new Error('The installed package response is invalid.');
  }
  return {
    version,
    sha256: packageMetadata.sha256,
    authenticodeSignerSha256,
    sourceKeyFingerprint: trust.keyFingerprint,
    sizeBytes: packageMetadata.sizeBytes,
  };
}

function expectedPackageReference(
  version: string,
  packageMetadata: ConnectorExecutablePackage,
  trust: ConnectorCatalogTrust | undefined,
): NonNullable<ProjectMcpServer['package']> {
  if (trust === undefined) throw new Error('The package has no verified catalog source.');
  return {
    version,
    sha256: packageMetadata.sha256,
    ...(packageMetadata.authenticodeSignerSha256 === undefined
      ? {}
      : { authenticodeSignerSha256: packageMetadata.authenticodeSignerSha256 }),
    sourceKeyFingerprint: trust.keyFingerprint,
    sizeBytes: packageMetadata.sizeBytes,
  };
}

async function removeSignedPackage(
  projectId: string,
  id: string,
  packageReference: NonNullable<ProjectMcpServer['package']>,
): Promise<void> {
  await localServiceClient.call('connector.package.remove', {
    projectId,
    id,
    package: packageReference,
  });
}

function samePackage(
  first: NonNullable<ProjectMcpServer['package']>,
  second: NonNullable<ProjectMcpServer['package']>,
): boolean {
  return (
    first.version === second.version &&
    first.sha256 === second.sha256 &&
    first.authenticodeSignerSha256 === second.authenticodeSignerSha256 &&
    first.sourceKeyFingerprint === second.sourceKeyFingerprint &&
    first.sizeBytes === second.sizeBytes
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function ConnectorDetail({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <dt className="font-medium text-secondary-text">{label}</dt>
      <dd className="mt-1 break-words text-text">{value}</dd>
    </div>
  );
}
