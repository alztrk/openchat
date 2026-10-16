import { useEffect, useMemo, useState } from 'react';
import { LoaderCircle, Search, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { McpCategory, McpCategoryKey, McpServerRecord } from '@/features/library/data/mcpCatalog';
import { useMcpStore } from '@/features/library/stores/mcpStore';
import { Button, Input } from '@/shared/ui';
import { McpCard } from './McpCard';
import { McpEnvModal } from './McpEnvModal';

const categories: readonly ('all' | McpCategory)[] = [
  'all', 'developer', 'database', 'webSearch', 'devOpsCloud', 'productivity', 'memoryReasoning', 'finance',
];

export function McpCatalogTab() {
  const { i18n, t } = useTranslation();
  const servers = useMcpStore((state) => state.servers);
  const loadState = useMcpStore((state) => state.loadState);
  const invalidConfigCount = useMcpStore((state) => state.invalidConfigCount);
  const busyServerIds = useMcpStore((state) => state.busyServerIds);
  const errorCode = useMcpStore((state) => state.errorCode);
  const initialize = useMcpStore((state) => state.initialize);
  const install = useMcpStore((state) => state.install);
  const setEnabled = useMcpStore((state) => state.setEnabled);
  const remove = useMcpStore((state) => state.remove);
  const probe = useMcpStore((state) => state.probe);
  const loadSecretStatus = useMcpStore((state) => state.loadSecretStatus);
  const saveSecrets = useMcpStore((state) => state.saveSecrets);
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState<'all' | McpCategory>('all');
  const [environmentServerId, setEnvironmentServerId] = useState<string | null>(null);
  const [noticeKey, setNoticeKey] = useState<'mcpEnabledSuccess' | 'mcpDisabledSuccess' | 'mcpDiscoverSuccess' | 'mcpEnvSaveSuccess' | null>(null);

  useEffect(() => { void initialize(); }, [initialize]);

  useEffect(() => {
    if (!noticeKey) return;
    const timeout = window.setTimeout(() => setNoticeKey(null), 4000);
    return () => window.clearTimeout(timeout);
  }, [noticeKey]);

  const categoryLabel = (value: string) => {
    const normalized = value.toLocaleLowerCase('en-US').replace(/[^a-z]/g, '');
    const categoryKey = categoryKeyFor(normalized);
    return categoryKey ? t(categoryKey) : value;
  };

  const visibleServers = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase(i18n.language);
    return servers.filter((server) => {
      const matchesCategory = category === 'all' || categoryKeyFor(server.category.toLocaleLowerCase('en-US').replace(/[^a-z]/g, '')) === categoryKeyFor(category);
      if (!matchesCategory) return false;
      if (!normalizedQuery) return true;
      const description = server.descriptionKey ? t(server.descriptionKey) : server.description;
      return [server.name, description, server.category, ...server.discoveredTools]
        .some((value) => value.toLocaleLowerCase(i18n.language).includes(normalizedQuery));
    });
  }, [category, i18n.language, query, servers, t]);

  const environmentServer = servers.find((server) => server.id === environmentServerId) ?? null;

  const installServer = async (server: McpServerRecord) => {
    const description = server.descriptionKey ? t(server.descriptionKey) : server.description;
    const installed = await install(server.id, description);
    if (installed) {
      setNoticeKey('mcpEnabledSuccess');
      if (server.requiredEnvKeys.length > 0) {
        setEnvironmentServerId(server.id);
      }
    }
    return installed;
  };

  const toggleServer = async (serverId: string, enabled: boolean) => {
    const changed = await setEnabled(serverId, enabled);
    if (changed) setNoticeKey(enabled ? 'mcpEnabledSuccess' : 'mcpDisabledSuccess');
    return changed;
  };

  const probeServer = async (serverId: string) => {
    const discovered = await probe(serverId);
    if (discovered) setNoticeKey('mcpDiscoverSuccess');
    return discovered;
  };

  const removeServer = async (serverId: string) => {
    const removed = await remove(serverId);
    if (removed) setNoticeKey('mcpDisabledSuccess');
    return removed;
  };

  return (
    <div className="mcp-catalog-page min-h-full min-w-0 px-5 py-5 sm:px-8 sm:py-6">
      <div className="mx-auto grid w-full max-w-screen-2xl gap-5">
        <div className="mcp-catalog-toolbar flex w-full flex-wrap items-center gap-3 rounded-card border border-border bg-surface p-3">
          <div className="mcp-catalog-search min-w-0 flex-1">
            <div className="relative">
              <Search aria-hidden="true" className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-secondary-icon" size={16} />
              <Input
                aria-label={t('mcpSearchHint')}
                className="pl-9"
                id="mcp-catalog-search"
                onChange={(event) => setQuery(event.target.value.trim())}
                placeholder={t('mcpSearchHint')}
                value={query}
              />
            </div>
          </div>
          {query ? (
            <Button aria-label={t('settingsSearchClear')} onClick={() => setQuery('')} size="icon" variant="ghost">
              <X aria-hidden="true" size={17} />
            </Button>
          ) : null}
        </div>

        <nav aria-label={t('mcpCategoryFilter')} className="mcp-category-filter flex gap-2 overflow-x-auto pb-1">
          {categories.map((item) => (
            <Button
              aria-pressed={category === item}
              className={category === item
                ? 'shrink-0 border border-accent/40 bg-selected text-accent-icon hover:bg-selected'
                : 'shrink-0 border border-border bg-surface text-secondary-text hover:bg-hover'}
              key={item}
              onClick={() => setCategory(item)}
              size="small"
              variant="ghost"
            >
              {t(item === 'all' ? 'mcpCategoryAll' : categoryKeyFor(item) ?? 'mcpCategoryAll')}
            </Button>
          ))}
        </nav>

        {loadState === 'loading' ? (
          <div aria-label={t('settingsSectionLoading')} className="flex justify-center p-12" role="status">
            <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-icon" size={20} />
          </div>
        ) : null}
        {loadState === 'error' ? (
          <div className="rounded-card border border-destructive/40 bg-surface p-5" role="alert">
            <p className="text-sm text-destructive">{t(errorCode ?? 'mcpCatalogLoadFailed')}</p>
            <Button className="mt-3" onClick={() => void initialize()} size="small">{t('retry')}</Button>
          </div>
        ) : null}
        {loadState === 'ready' && invalidConfigCount > 0 ? (
          <p className="rounded-control border border-warning/40 bg-surface px-3 py-2 text-sm text-warning" role="status">
            {t('mcpInvalidConfigCount')}
          </p>
        ) : null}
        {noticeKey ? <p className="rounded-control border border-success/40 bg-surface px-3 py-2 text-sm text-success" role="status">{t(noticeKey)}</p> : null}
        {loadState === 'ready' && errorCode && errorCode !== 'mcpCatalogLoadFailed' ? (
          <p className="text-sm text-destructive" role="alert">{t(errorCode)}</p>
        ) : null}
        {loadState === 'ready' && visibleServers.length === 0 ? (
          <p className="p-8 text-center text-secondary-text">
            {t(query || category !== 'all' ? 'mcpNoSearchResults' : 'mcpNoToolsFound')}
          </p>
        ) : null}
        {loadState === 'ready' && visibleServers.length > 0 ? (
          <ul className="mcp-server-list grid gap-3">
            {visibleServers.map((server) => (
              <li key={server.id}>
                <McpCard
                  busy={busyServerIds.includes(server.id)}
                  categoryLabel={categoryLabel(server.category)}
                  description={server.descriptionKey ? t(server.descriptionKey) : server.description}
                  onConfigure={() => setEnvironmentServerId(server.id)}
                  onInstall={() => installServer(server)}
                  onProbe={() => probeServer(server.id)}
                  onRemove={() => removeServer(server.id)}
                  onToggle={(enabled) => toggleServer(server.id, enabled)}
                  server={server}
                />
              </li>
            ))}
          </ul>
        ) : null}

        {environmentServer ? (
          <McpEnvModal
            busy={busyServerIds.includes(environmentServer.id)}
            loadStatus={loadSecretStatus}
            onOpenChange={(open) => { if (!open) setEnvironmentServerId(null); }}
            onSaved={() => setNoticeKey('mcpEnvSaveSuccess')}
            open={environmentServerId !== null}
            save={saveSecrets}
            server={environmentServer}
          />
        ) : null}
      </div>
    </div>
  );
}

function categoryKeyFor(normalizedCategory: string): McpCategoryKey | null {
  switch (normalizedCategory) {
    case 'developer': return 'mcpCategoryDeveloper';
    case 'database': return 'mcpCategoryDatabase';
    case 'websearch': return 'mcpCategoryWebSearch';
    case 'devopscloud': return 'mcpCategoryDevOpsCloud';
    case 'productivity': return 'mcpCategoryProductivity';
    case 'memoryreasoning': return 'mcpCategoryMemoryReasoning';
    case 'finance': return 'mcpCategoryFinance';
    default: return null;
  }
}
