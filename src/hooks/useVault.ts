import { create } from "zustand";
import type { VaultStatus } from "../lib/types";
import * as api from "../lib/tauri";

interface VaultStore {
  status: VaultStatus | null;
  busy: boolean;
  error: string | null;

  /** Fetch the current vault status from the backend. */
  refresh: () => Promise<void>;
  /** Unlock the vault (or create it on first use). */
  unlock: (password: string) => Promise<void>;
  /** Zero the in-memory derived key. */
  lock: () => Promise<void>;
}

export const useVault = create<VaultStore>((set, get) => ({
  status: null,
  busy: false,
  error: null,

  refresh: async () => {
    try {
      const status = await api.vaultStatus();
      set({ status, error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  unlock: async (password) => {
    set({ busy: true, error: null });
    try {
      await api.unlockVault(password);
      await get().refresh();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    } finally {
      set({ busy: false });
    }
  },

  lock: async () => {
    set({ busy: true, error: null });
    try {
      await api.lockVault();
      await get().refresh();
    } finally {
      set({ busy: false });
    }
  },
}));
