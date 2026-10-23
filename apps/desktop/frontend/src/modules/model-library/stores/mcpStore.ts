import { create } from 'zustand';
import { createMcpCatalog, type McpServerRecord } from '@/modules/model-library/data/mcpCatalog';
import {
  type GlobalMcpSecretStatus,
  type GlobalMcpSecretUpdate,
  getGlobalMcpSecretStatus,
  loadGlobalMcpServers,
  McpStorageClientError,
  probeGlobalMcpServer,
  removeGlobalMcpServer,
  saveGlobalMcpSecrets,
  saveGlobalMcpServer,
  setGlobalMcpEnabled,
  mcpStorageErrorDetails,
} from '@/modules/model-library/data/mcpStorage';

type McpLoadState = 'idle' | 'loading' | 'ready' | 'error';
type McpErrorCode =
  | 'mcpCatalogLoadFailed'
  | 'mcpConfigActionFailed'
  | 'mcpCredentialStatusFailed'
  | 'mcpCredentialSaveFailed'
  | 'mcpRequiredCredentialsMissing'
  | 'mcpDiscoverFailed';

interface McpState {
  servers: McpServerRecord[];
  loadState: McpLoadState;
  invalidConfigCount: number;
  busyServerIds: string[];
  errorCode: McpErrorCode | null;
  errorDetails: string | null;
  initialize(): Promise<void>;
  install(serverId: string, localizedDescription: string): Promise<boolean>;
  setEnabled(serverId: string, enabled: boolean): Promise<boolean>;
  remove(serverId: string): Promise<boolean>;
  loadSecretStatus(serverId: string, environmentNames: readonly string[]): Promise<GlobalMcpSecretStatus[]>;
  saveSecrets(serverId: string, updates: readonly GlobalMcpSecretUpdate[]): Promise<boolean>;
  probe(serverId: string): Promise<boolean>;
}

export const useMcpStore = create<McpState>((set, get) => ({
  servers: createMcpCatalog(),
  loadState: 'idle',
  invalidConfigCount: 0,
  busyServerIds: [],
  errorCode: null,
  errorDetails: null,

  async initialize() {
    if (get().loadState === 'loading') return;
    set({ loadState: 'loading', errorCode: null, errorDetails: null });
    try {
      const loaded = await loadGlobalMcpServers();
      set({
        servers: mergeServers(loaded.servers),
        invalidConfigCount: loaded.invalidConfigCount,
        loadState: 'ready',
      });
    } catch (error) {
      set({ loadState: 'error', errorCode: 'mcpCatalogLoadFailed', errorDetails: mcpStorageErrorDetails(error) });
    }
  },

  async install(serverId, localizedDescription) {
    const server = get().servers.find((item) => item.id === serverId);
    if (!server || server.installed || get().busyServerIds.includes(serverId)) return false;
    markBusy(set, serverId, true);
    set({ errorCode: null, errorDetails: null });
    try {
      const saved = await saveGlobalMcpServer({ ...server, description: localizedDescription });
      updateServer(set, saved);
      return true;
    } catch (error) {
      set({ errorCode: 'mcpConfigActionFailed', errorDetails: mcpStorageErrorDetails(error) });
      return false;
    } finally {
      markBusy(set, serverId, false);
    }
  },

  async setEnabled(serverId, enabled) {
    if (get().busyServerIds.includes(serverId)) return false;
    markBusy(set, serverId, true);
    set({ errorCode: null, errorDetails: null });
    try {
      const updated = await setGlobalMcpEnabled(serverId, enabled);
      updateServer(set, updated);
      return true;
    } catch (error) {
      set({ errorCode: 'mcpConfigActionFailed', errorDetails: mcpStorageErrorDetails(error) });
      return false;
    } finally {
      markBusy(set, serverId, false);
    }
  },

  async remove(serverId) {
    if (get().busyServerIds.includes(serverId)) return false;
    markBusy(set, serverId, true);
    set({ errorCode: null, errorDetails: null });
    try {
      await removeGlobalMcpServer(serverId);
      const current = get().servers.find((server) => server.id === serverId);
      if (!current) return true;
      const catalogServer = createMcpCatalog().find((server) => server.id === serverId);
      if (!catalogServer) {
        set((state) => ({ servers: state.servers.filter((server) => server.id !== serverId) }));
        return true;
      }
      updateServer(set, {
        ...current,
        installed: false,
        enabled: false,
        installDir: '',
        discoveredTools: catalogServer.discoveredTools,
      });
      return true;
    } catch (error) {
      set({ errorCode: 'mcpConfigActionFailed', errorDetails: mcpStorageErrorDetails(error) });
      return false;
    } finally {
      markBusy(set, serverId, false);
    }
  },

  async loadSecretStatus(serverId, environmentNames) {
    try {
      return await getGlobalMcpSecretStatus(serverId, environmentNames);
    } catch (error) {
      set({ errorCode: 'mcpCredentialStatusFailed', errorDetails: mcpStorageErrorDetails(error) });
      throw error;
    }
  },

  async saveSecrets(serverId, updates) {
    if (get().busyServerIds.includes(serverId)) return false;
    markBusy(set, serverId, true);
    set({ errorCode: null, errorDetails: null });
    try {
      await saveGlobalMcpSecrets(serverId, updates);
      return true;
    } catch (error) {
      set({ errorCode: 'mcpCredentialSaveFailed', errorDetails: mcpStorageErrorDetails(error) });
      throw error;
    } finally {
      markBusy(set, serverId, false);
    }
  },

  async probe(serverId) {
    if (get().busyServerIds.includes(serverId)) return false;
    markBusy(set, serverId, true);
    set({ errorCode: null, errorDetails: null });
    try {
      updateServer(set, await probeGlobalMcpServer(serverId));
      return true;
    } catch (error) {
      set({
        errorCode:
          error instanceof McpStorageClientError && error.code === 'mcp_required_credentials_missing'
            ? 'mcpRequiredCredentialsMissing'
            : 'mcpDiscoverFailed',
        errorDetails: mcpStorageErrorDetails(error),
      });
      return false;
    } finally {
      markBusy(set, serverId, false);
    }
  },
}));

function mergeServers(stored: readonly McpServerRecord[]): McpServerRecord[] {
  const storedById = new Map(stored.map((server) => [server.id, server]));
  const catalog = createMcpCatalog().map((server) => {
    const saved = storedById.get(server.id);
    if (!saved) return server;
    storedById.delete(server.id);
    return {
      ...server,
      ...saved,
      category: server.category,
      description: saved.description,
      descriptionKey: server.descriptionKey,
    };
  });
  return [...catalog, ...storedById.values()];
}

function updateServer(
  set: (partial: Partial<McpState> | ((state: McpState) => Partial<McpState>)) => void,
  updated: McpServerRecord,
): void {
  set((state) => ({
    servers: state.servers.map((server) => {
      if (server.id !== updated.id) return server;
      return {
        ...server,
        ...updated,
        descriptionKey: server.descriptionKey,
        category: server.category,
        description: server.descriptionKey ? server.description : updated.description,
      };
    }),
  }));
}

function markBusy(
  set: (partial: Partial<McpState> | ((state: McpState) => Partial<McpState>)) => void,
  serverId: string,
  busy: boolean,
): void {
  set((state) => ({
    busyServerIds: busy
      ? [...new Set([...state.busyServerIds, serverId])]
      : state.busyServerIds.filter((id) => id !== serverId),
  }));
}
