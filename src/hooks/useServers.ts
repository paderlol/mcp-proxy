import { create } from "zustand";
import type { EnvMapping, McpServerConfig } from "../lib/types";
import * as api from "../lib/tauri";

interface ServersStore {
  servers: McpServerConfig[];
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
}

export const useServers = create<ServersStore>((set) => ({
  servers: [],
  loading: false,

  fetchServers: async () => {
    set({ loading: true });
    try {
      const servers = await api.listServers();
      set({ servers });
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
}));
