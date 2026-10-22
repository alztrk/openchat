import { Check, Download, LoaderCircle, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Card, Dialog, Input } from '@/design-system';
import {
  type ConnectorCatalogEntry,
  type ConnectorCatalogSource,
  type VerifiedConnectorCatalog,
  fetchSignedConnectorCatalog,
  parseConnectorCatalogSource,
  readConnectorSources,
  writeConnectorSources,
} from '@/modules/settings/data/connectorCatalog';

interface ConnectorCatalogSourcesPanelProps {
  existingEntries: readonly ConnectorCatalogEntry[];
  canRefresh: boolean;
  onImport(entry: ConnectorCatalogEntry): Promise<'saved' | 'duplicate' | 'failed'>;
}

interface SourceResult {
  sourceId: string;
  catalog: VerifiedConnectorCatalog;
}

export function ConnectorCatalogSourcesPanel({
  existingEntries,
  canRefresh,
  onImport,
}: ConnectorCatalogSourcesPanelProps) {
  const { t } = useTranslation();
  const [sources, setSources] = useState<ConnectorCatalogSource[]>([]);
  const [loadFailed, setLoadFailed] = useState(false);
  const [sourceError, setSourceError] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [publicKey, setPublicKey] = useState('');
  const [addFailed, setAddFailed] = useState(false);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);
  const [result, setResult] = useState<SourceResult | null>(null);
  const [refreshError, setRefreshError] = useState<
    'connectorSourceFetchError' | 'connectorSourceRollbackError' | 'connectorSourceRevisionConflict' | 'connectorSourceSaveFailed' | null
  >(null);
  const [importingId, setImportingId] = useState<string | null>(null);
  const [importedIds, setImportedIds] = useState<string[]>([]);
  const [importFailed, setImportFailed] = useState(false);

  useEffect(() => {
    try {
      setSources(readConnectorSources());
      setLoadFailed(false);
    } catch {
      setLoadFailed(true);
    }
  }, [loadAttempt]);

  const persistSources = (nextSources: ConnectorCatalogSource[]) => {
    try {
      writeConnectorSources(nextSources);
      setSources(nextSources);
      setSourceError(false);
      return true;
    } catch {
      setSourceError(true);
      return false;
    }
  };

  const addSource = () => {
    setAddFailed(false);
    try {
      const source = parseConnectorCatalogSource({
        id: crypto.randomUUID().replaceAll('-', ''),
        name,
        url,
        publicKeyBase64: publicKey.trim(),
      });
      if (sources.some((current) => current.url === source.url)) {
        setAddFailed(true);
        return;
      }
      if (persistSources([...sources, source])) {
        setShowAddDialog(false);
        setName('');
        setUrl('');
        setPublicKey('');
      }
    } catch {
      setAddFailed(true);
    }
  };

  const removeSource = (source: ConnectorCatalogSource) => {
    if (!persistSources(sources.filter((item) => item.id !== source.id))) return;
    if (result?.sourceId === source.id) setResult(null);
  };

  const refreshSource = async (source: ConnectorCatalogSource) => {
    if (!canRefresh || refreshingId !== null) return;
    setRefreshingId(source.id);
    setRefreshError(null);
    setResult(null);
    setImportedIds([]);
    try {
      const catalog = await fetchSignedConnectorCatalog(source);
      if (source.lastRevision !== null && catalog.revision < source.lastRevision) {
        setRefreshError('connectorSourceRollbackError');
        return;
      }
      if (
        source.lastRevision === catalog.revision &&
        source.lastPayloadHash !== catalog.payloadHash
      ) {
        setRefreshError('connectorSourceRevisionConflict');
        return;
      }
      if (source.lastRevision === null || catalog.revision > source.lastRevision) {
        const nextSources = sources.map((item) =>
          item.id === source.id
            ? { ...item, lastRevision: catalog.revision, lastPayloadHash: catalog.payloadHash }
            : item,
        );
        if (!persistSources(nextSources)) {
          setRefreshError('connectorSourceSaveFailed');
          return;
        }
      }
      setResult({ sourceId: source.id, catalog });
    } catch {
      setRefreshError('connectorSourceFetchError');
    } finally {
      setRefreshingId(null);
    }
  };

  const importEntry = async (entry: ConnectorCatalogEntry) => {
    if (importingId !== null) return;
    setImportingId(entry.id);
    setImportFailed(false);
    try {
      const outcome = await onImport(entry);
      if (outcome === 'saved') setImportedIds((current) => [...current, entry.id]);
      else if (outcome === 'failed') setImportFailed(true);
    } catch {
      setImportFailed(true);
    } finally {
      setImportingId(null);
    }
  };

  return (
    <section aria-labelledby="connector-catalog-sources-heading" className="grid gap-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="font-medium text-text" id="connector-catalog-sources-heading">
            {t('connectorSourcesTitle')}
          </h3>
          <p className="mt-1 text-sm text-secondary-text">{t('connectorSourcesDescription')}</p>
        </div>
        <Button onClick={() => setShowAddDialog(true)} size="small" variant="secondary">
          <Plus aria-hidden="true" size={15} />
          {t('connectorSourceAdd')}
        </Button>
      </div>
      <Card className="flex gap-3 p-4" role="note">
        <p className="text-sm text-secondary-text">{t('connectorSourceIdentityWarning')}</p>
      </Card>

      {loadFailed ? (
        <div className="flex flex-wrap items-center gap-3" role="alert">
          <p className="text-sm text-destructive">{t('connectorSourceLoadError')}</p>
          <Button onClick={() => setLoadAttempt((attempt) => attempt + 1)} size="small" variant="secondary">
            {t('retry')}
          </Button>
        </div>
      ) : sources.length === 0 ? (
        <p className="text-sm text-secondary-text">{t('connectorSourcesEmpty')}</p>
      ) : (
        <div className="grid gap-3">
          {sources.map((source) => (
            <Card className="grid gap-3 p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center" key={source.id}>
              <div className="min-w-0">
                <p className="font-medium text-text">{source.name}</p>
                <p className="mt-1 break-all text-xs text-secondary-text">{source.url}</p>
                {result?.sourceId === source.id ? (
                  <p className="mt-1 text-xs text-secondary-text">
                    {t('connectorSourceRevision', { revision: result.catalog.revision })}
                    {' · '}
                    {t('connectorSourceFingerprint', { fingerprint: result.catalog.keyFingerprint })}
                  </p>
                ) : null}
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  disabled={!canRefresh || refreshingId !== null}
                  onClick={() => void refreshSource(source)}
                  size="small"
                  variant="secondary"
                >
                  {refreshingId === source.id ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                  ) : (
                    <RefreshCw aria-hidden="true" size={15} />
                  )}
                  {t('connectorSourceRefresh')}
                </Button>
                <Button
                  aria-label={`${t('connectorSourceRemove')}: ${source.name}`}
                  onClick={() => removeSource(source)}
                  size="small"
                  variant="ghost"
                >
                  <Trash2 aria-hidden="true" size={15} />
                  {t('connectorSourceRemove')}
                </Button>
              </div>
            </Card>
          ))}
        </div>
      )}
      {sourceError ? <p className="text-sm text-destructive" role="alert">{t('connectorSourceAddFailed')}</p> : null}
      {refreshError ? <p className="text-sm text-destructive" role="alert">{t(refreshError)}</p> : null}

      {result ? (
        <div className="grid gap-3" aria-label={t('connectorSourcesTitle')}>
          <p className="text-sm text-secondary-text">
            {t('connectorSourceVerifiedCount', { count: result.catalog.entries.length })}
          </p>
          {result.catalog.entries.map((entry) => {
            const isAlreadyAdded = existingEntries.some(
              (current) =>
                current.id === entry.id ||
                current.name.toLocaleLowerCase('en-US') === entry.name.toLocaleLowerCase('en-US'),
            );
            const wasImported = importedIds.includes(entry.id);
            return (
              <Card className="flex flex-wrap items-center justify-between gap-3 p-4" key={entry.id}>
                <div className="min-w-0">
                  <p className="font-medium text-text">{entry.name}</p>
                  <p className="mt-1 text-xs text-secondary-text">
                    {entry.publisher} · {entry.version} · {entry.transport}
                  </p>
                </div>
                <Button
                  disabled={isAlreadyAdded || wasImported || importingId !== null}
                  onClick={() => void importEntry(entry)}
                  size="small"
                  variant="secondary"
                >
                  {importingId === entry.id ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" size={15} />
                  ) : wasImported ? (
                    <Check aria-hidden="true" size={15} />
                  ) : (
                    <Download aria-hidden="true" size={15} />
                  )}
                  {wasImported || isAlreadyAdded
                    ? t('connectorSourceAlreadyAdded')
                    : t('connectorSourceImport')}
                </Button>
              </Card>
            );
          })}
          {importFailed ? <p className="text-sm text-destructive" role="alert">{t('connectorSourceAddFailed')}</p> : null}
        </div>
      ) : null}

      <Dialog
        onOpenChange={(open) => {
          if (!open) {
            setShowAddDialog(false);
            setAddFailed(false);
          }
        }}
        open={showAddDialog}
        title={t('connectorSourceAddDialogTitle')}
      >
        <div className="grid gap-4">
          <Input label={t('connectorSourceName')} onChange={(event) => setName(event.target.value)} value={name} />
          <Input
            autoComplete="url"
            label={t('connectorSourceUrl')}
            onChange={(event) => setUrl(event.target.value)}
            value={url}
          />
          <Input
            autoCapitalize="none"
            autoComplete="off"
            label={t('connectorSourcePublicKey')}
            onChange={(event) => setPublicKey(event.target.value)}
            value={publicKey}
          />
          <p className="text-xs text-secondary-text">{t('connectorSourceKeyHelp')}</p>
          {addFailed ? <p className="text-sm text-destructive" role="alert">{t('connectorSourceAddFailed')}</p> : null}
          <div className="flex justify-end gap-2 border-t border-border pt-3">
            <Button onClick={() => setShowAddDialog(false)} variant="ghost">{t('cancel')}</Button>
            <Button onClick={addSource} variant="primary">{t('connectorSourceAdd')}</Button>
          </div>
        </div>
      </Dialog>
    </section>
  );
}
