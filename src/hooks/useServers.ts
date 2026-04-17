import { create } from "zustand";
import type { EnvMapping, McpServerConfig, ProxyStatus } from "../lib/types";
import * as api from "../lib/tauri";

interface ServersStore {
  servers: McpServerConfig[];
  proxyStatuses: Record<string, ProxyStatus>;
  loading: boolean;

  fetchServers: () => Promise<void>;
  addServer: (params: {
    name: string;
    command: string;
    args: string[];
    transportType: string;
    ssePort?: number;
    ssePath?: string;
    runModeType?: string;
    dockerImage?: string;
    envMappings?: EnvMapping[];
  }) => Promise<McpServerConfig>;
  updateServer: (server: McpServerConfig) => Promise<McpServerConfig>;
  deleteServer: (id: string) => Promise<void>;
  startProxy: (serverId: string) => Promise<void>;
  stopProxy: (serverId: string) => Promise<void>;
  refreshProxyStatus: (serverId: string) => Promise<void>;
}

export const useServers = create<ServersStore>((set) => ({
  servers: [],
  proxyStatuses: {},
  loading: false,

  fetchServers: async () => {
    set({ loading: true });
    try {
      const servers = await api.listServers();
      set({ servers });
      // Refresh proxy status for all servers
      for (const s of servers) {
        const status = await api.getProxyStatus(s.id);
        set((state) => ({
          proxyStatuses: { ...state.proxyStatuses, [s.id]: status },
        }));
      }
    } finally {
      set({ loading: false });
    }
  },

  addServer: async (params) => {
    const server = await api.addServer(params);
    set((state) => ({ servers: [...state.servers, server] }));
    return server;
  },

  updateServer: async (server) => {
    const updated = await api.updateServer(server);
    set((state) => ({
      servers: state.servers.map((s) => (s.id === updated.id ? updated : s)),
    }));
    return updated;
  },

  deleteServer: async (id) => {
    await api.deleteServer(id);
    set((state) => ({
      servers: state.servers.filter((s) => s.id !== id),
    }));
  },

  startProxy: async (serverId) => {
    const status = await api.startProxy(serverId);
    set((state) => ({
      proxyStatuses: { ...state.proxyStatuses, [serverId]: status },
    }));
  },

  stopProxy: async (serverId) => {
    await api.stopProxy(serverId);
    set((state) => ({
      proxyStatuses: {
        ...state.proxyStatuses,
        [serverId]: { server_id: serverId, running: false, pid: null },
      },
    }));
  },

  refreshProxyStatus: async (serverId) => {
    const status = await api.getProxyStatus(serverId);
    set((state) => ({
      proxyStatuses: { ...state.proxyStatuses, [serverId]: status },
    }));
  },
}));
