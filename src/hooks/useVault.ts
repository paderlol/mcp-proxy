import { create } from "zustand";
import type { VaultStatus } from "../lib/types";
import * as api from "../lib/tauri";

interface VaultStore {
  status: VaultStatus | null;
  busy: boolean;
  error: string | null;
  setStatus: (status: VaultStatus) => void;

  /** Fetch the current vault status from the backend. */
  refresh: () => Promise<void>;
  /** Unlock the vault (or create it on first use). */
  unlock: (password: string) => Promise<void>;
  /** Zero the in-memory derived key. */
  lock: () => Promise<void>;
  /** Re-encrypt the vault with a new master password (requires unlocked). */
  changePassword: (newPassword: string) => Promise<void>;
  /** Delete the vault file entirely. All Local secrets are lost. */
  reset: () => Promise<void>;
  /**
   * macOS-only: opt in to / out of the encrypted vault instead of Keychain.
   * The caller is responsible for warning the user that secrets do not
   * migrate between backends.
   */
  setPreferLocalVault: (enabled: boolean) => Promise<void>;
}

export const useVault = create<VaultStore>((set, get) => ({
  status: null,
  busy: false,
  error: null,
  setStatus: (status) => set({ status, error: null }),

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

  changePassword: async (newPassword) => {
    set({ busy: true, error: null });
    try {
      await api.changeVaultPassword(newPassword);
      await get().refresh();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    } finally {
      set({ busy: false });
    }
  },

  reset: async () => {
    set({ busy: true, error: null });
    try {
      await api.resetVault();
      await get().refresh();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    } finally {
      set({ busy: false });
    }
  },

  setPreferLocalVault: async (enabled) => {
    set({ busy: true, error: null });
    try {
      await api.setPreferLocalVault(enabled);
      await get().refresh();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    } finally {
      set({ busy: false });
    }
  },
}));
