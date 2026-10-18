import type { TFunction } from 'i18next';
import { AlertCircle, Download, Info, LoaderCircle, Pencil, Plus, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Input } from '@/design-system';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { type ProjectMcpServer, parseProjectMcpCatalog } from '@/modules/projects/data/projectMcp';
import {
  loadProjectToolPermissionRules,
  saveProjectToolPermissionRules,
} from '@/modules/projects/data/projectPermissionSettings';
import {
  type ConnectorCatalogEntry,
  readConnectorCatalog,
  writeConnectorCatalog,
} from '@/modules/settings/data/connectorCatalog';
import { localServiceClient } from '@/platform/local-service';
import { ConnectorTemplateDialog } from './ConnectorTemplateDialog';

type LoadState = 'loading' | 'ready' | 'failed';
type SaveOutcome = 'saved' | 'duplicate' | 'failed';

class DuplicateProjectConnectorError extends Error {
  constructor() {
    super('The connector is already installed in this project.');
    this.name = 'DuplicateProjectConnectorError';
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
    'connectorCatalogDuplicate' | 'connectorCatalogInstallFailed' | null
  >(null);
  const [errorKey, setErrorKey] = useState<'connectorCatalogLoadFailed' | 'connectorCatalogSaveFailed' | null>(null);

  const activeProjectId =
    pendingProjectId ??
    conversations.find((conversation) => conversation.id === activeConversationId)?.projectId ??
    null;

  // biome-ignore lint/correctness/useExhaustiveDependencies: Incrementing loadAttempt is the explicit retry signal for this request.
  useEffect(() => {
    setLoadState('loading');
    setErrorKey(null);
    try {
      setEntries(readConnectorCatalog());
      setLoadState('ready');
    } catch {
      setLoadState('failed');
      setErrorKey('connectorCatalogLoadFailed');
    }
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
        : entries.map((current) => (current.id === editingId ? entry : current));
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
      writeConnectorCatalog(nextEntries);
      setEntries(nextEntries);
      return 'saved';
    } catch {
      setErrorKey('connectorCatalogSaveFailed');
      return 'failed';
    } finally {
      setIsSaving(false);
    }
  };

  const confirmDelete = () => {
    const target = deleteEntry;
    if (!target || isSaving) return;
    setIsSaving(true);
    setErrorKey(null);
    try {
      const nextEntries = entries.filter((entry) => entry.id !== target.id);
      writeConnectorCatalog(nextEntries);
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

    setInstallingId(entry.id);
    setInstallFeedback(null);
    setInstallErrorKey(null);
    setErrorKey(null);
    try {
      const existingResponse = await localServiceClient.call('project.mcp.catalog.get', { projectId });
      const existingCatalog = parseProjectMcpCatalog(existingResponse);
      if (existingCatalog.servers.some((server) => server.id === entry.id)) {
        throw new DuplicateProjectConnectorError();
      }

      const previousRules = await loadProjectToolPermissionRules(projectId);
      const nextRules = {
        ...previousRules,
        [`mcp__${entry.id}__*`]: 'ask' as const,
      };
      await saveProjectToolPermissionRules(projectId, nextRules);

      try {
        const server = projectServerFromEntry(entry);
        const catalog = parseProjectMcpCatalog({
          version: 1,
          servers: [...existingCatalog.servers, server],
        });
        await localServiceClient.call('project.mcp.catalog.save', {
          projectId,
          catalog,
        });
      } catch (saveError) {
        try {
          await saveProjectToolPermissionRules(projectId, previousRules);
        } catch {
          throw new Error('Connector installation rollback failed.', { cause: saveError });
        }
        throw saveError;
      }
      setInstallFeedback('installed');
    } catch (error) {
      if (error instanceof DuplicateProjectConnectorError) {
        setInstallErrorKey('connectorCatalogDuplicate');
      } else {
        setInstallErrorKey('connectorCatalogInstallFailed');
      }
    } finally {
      setInstallingId(null);
    }
  };

  return (
    <section aria-labelledby="settings-page-heading" className="grid gap-5">
      <p className="text-sm text-secondary-text">{t('connectorCatalogIntro')}</p>
      <Card className="flex gap-3 p-4" role="note">
        <Info aria-hidden="true" className="mt-0.5 shrink-0 text-info" size={18} />
        <p className="text-sm text-secondary-text">{t('connectorCatalogSecurityNotice')}</p>
      </Card>
      {activeProjectId === null ? (
        <Card className="flex gap-3 p-4" id="connector-catalog-no-project" role="note">
          <Info aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-text" size={18} />
          <p className="text-sm text-secondary-text">{t('connectorCatalogNoProject')}</p>
        </Card>
      ) : null}

      {installFeedback ? (
        <p aria-live="polite" className="text-sm text-success" role="status">
          {t('connectorCatalogInstalled')}
        </p>
      ) : null}
      {installErrorKey ? (
        <p className="text-sm text-destructive" role="alert">
          {t(installErrorKey)}
        </p>
      ) : null}

      {loadState === 'failed' ? (
        <Card className="grid gap-3 p-5" role="alert">
          <p className="flex items-center gap-2 text-sm text-destructive">
            <AlertCircle aria-hidden="true" size={16} />
            {t(errorKey ?? 'connectorCatalogLoadFailed')}
          </p>
          <Button className="justify-self-start" onClick={() => setLoadAttempt((attempt) => attempt + 1)} size="small">
            {t('retry')}
          </Button>
        </Card>
      ) : (
        <>
          {loadState !== 'loading' && entries.length > 0 ? (
            <Input
              label={t('connectorCatalogSearch')}
              onChange={(event) => setSearch(event.target.value)}
              value={search}
            />
          ) : null}

          {loadState === 'loading' ? (
            <div aria-live="polite" className="flex justify-center py-8" role="status">
              <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-text" size={22} />
            </div>
          ) : entries.length === 0 ? (
            <Card className="p-5">
              <p className="text-sm text-secondary-text">{t('connectorCatalogEmpty')}</p>
            </Card>
          ) : visibleEntries.length === 0 ? (
            <Card className="p-5">
              <p className="text-sm text-secondary-text">{t('connectorCatalogNoResults')}</p>
            </Card>
          ) : (
            <div className="grid gap-3">
              {visibleEntries.map((entry) => (
                <ConnectorEntryCard
                  canInstall={serviceStatus === 'ready' && activeProjectId !== null && installingId === null}
                  entry={entry}
                  hasProject={activeProjectId !== null}
                  installing={installingId === entry.id}
                  key={entry.id}
                  onDelete={() => setDeleteEntry(entry)}
                  onEdit={() => setEditorEntry(entry)}
                  onInstall={() => setInstallEntryTarget(entry)}
                  t={t}
                />
              ))}
            </div>
          )}

          <Button
            className="justify-self-start"
            disabled={loadState !== 'ready' || isSaving}
            onClick={() => setEditorEntry(null)}
          >
            <Plus aria-hidden="true" size={16} />
            {t('connectorCatalogAdd')}
          </Button>
        </>
      )}

      {errorKey === 'connectorCatalogSaveFailed' ? (
        <p className="text-sm text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}

      {editorEntry !== undefined ? (
        <ConnectorTemplateDialog
          entry={editorEntry}
          onOpenChange={(open) => {
            if (!open) setEditorEntry(undefined);
          }}
          onSave={saveEntry}
        />
      ) : null}

      {installEntryTarget !== null ? (
        <ConnectorInstallDialog
          entry={installEntryTarget}
          onConfirm={() => {
            const entry = installEntryTarget;
            setInstallEntryTarget(null);
            void installEntry(entry);
          }}
          onOpenChange={(open) => {
            if (!open) setInstallEntryTarget(null);
          }}
        />
      ) : null}

      <Dialog
        onOpenChange={(open) => {
          if (!open) setDeleteEntry(null);
        }}
        open={deleteEntry !== null}
        title={t('connectorCatalogDelete')}
      >
        <p className="mb-5 text-sm text-secondary-text">{t('connectorCatalogDeleteConfirm')}</p>
        <div className="flex flex-wrap justify-end gap-2">
          <Button disabled={isSaving} onClick={() => setDeleteEntry(null)}>
            {t('cancel')}
          </Button>
          <Button disabled={isSaving} onClick={confirmDelete} variant="destructive">
            {isSaving ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            ) : (
              <Trash2 aria-hidden="true" size={16} />
            )}
            {t('connectorCatalogDelete')}
          </Button>
        </div>
      </Dialog>
    </section>
  );
}

function ConnectorEntryCard({
  canInstall,
  entry,
  hasProject,
  installing,
  onDelete,
  onEdit,
  onInstall,
  t,
}: {
  canInstall: boolean;
  entry: ConnectorCatalogEntry;
  hasProject: boolean;
  onDelete(): void;
  onEdit(): void;
  onInstall(): void;
  installing: boolean;
  t: TFunction;
}) {
  return (
    <Card className="grid gap-4 p-4 sm:p-5">
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

      <Button
        aria-describedby={hasProject ? undefined : 'connector-catalog-no-project'}
        className="justify-self-start"
        disabled={!canInstall || installing}
        onClick={onInstall}
      >
        {installing ? (
          <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
        ) : (
          <Download aria-hidden="true" size={16} />
        )}
        {t('connectorCatalogInstall')}
      </Button>
    </Card>
  );
}

function ConnectorInstallDialog({
  entry,
  onConfirm,
  onOpenChange,
}: {
  entry: ConnectorCatalogEntry;
  onConfirm(): void;
  onOpenChange(open: boolean): void;
}) {
  const { t } = useTranslation();
  const authentication =
    entry.transport === 'streamableHttp'
      ? entry.authEnvironmentVariable === undefined
        ? t('connectorCatalogNoBearerCredential')
        : t('connectorCatalogProjectBearerCredential')
      : entry.environmentVariables.length === 0
        ? t('connectorCatalogNoCredentialVariables')
        : t('connectorCatalogProjectEnvironmentCredentials');

  return (
    <Dialog onOpenChange={onOpenChange} open title={t('connectorCatalogInstall')}>
      <div className="grid gap-3">
        <p className="text-sm text-secondary-text">{t('connectorCatalogInstallPrompt')}</p>
        <dl className="grid gap-2 text-sm">
          <ConnectorDetail label={t('connectorCatalogPublisher')} value={entry.publisher} />
          <ConnectorDetail label={t('connectorCatalogVersion')} value={entry.version} />
          <ConnectorDetail label={t('connectorCatalogSourceUrl')} value={entry.sourceUrl} />
          <ConnectorDetail label={t('projectMcpEndpoint')} value={entry.endpoint ?? entry.program ?? ''} />
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
        <p className="text-sm text-secondary-text">{t('connectorCatalogRemovalImpact')}</p>
        <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
          <Button onClick={() => onOpenChange(false)}>{t('cancel')}</Button>
          <Button onClick={onConfirm} variant="primary">
            {t('connectorCatalogInstall')}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

function projectServerFromEntry(entry: ConnectorCatalogEntry): ProjectMcpServer {
  return {
    id: entry.id,
    enabled: false,
    transport: entry.transport,
    arguments: entry.arguments,
    environmentVariables: entry.environmentVariables,
    ...(entry.program === undefined ? {} : { program: entry.program }),
    ...(entry.endpoint === undefined ? {} : { endpoint: entry.endpoint }),
    ...(entry.authEnvironmentVariable === undefined ? {} : { authEnvironmentVariable: entry.authEnvironmentVariable }),
  };
}

function ConnectorDetail({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <dt className="font-medium text-secondary-text">{label}</dt>
      <dd className="mt-1 break-words text-text">{value}</dd>
    </div>
  );
}
