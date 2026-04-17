import { create } from "zustand";
import type { SecretEntry, SecretSource } from "../lib/types";
import * as api from "../lib/tauri";

interface SecretsStore {
  secrets: SecretEntry[];
  loading: boolean;

  fetchSecrets: () => Promise<void>;
  /** Create or fully update a secret, writing `value` to the backing store. */
  addSecret: (
    id: string,
    label: string,
    value: string,
    source: SecretSource,
  ) => Promise<void>;
  /**
   * Update metadata + optionally the stored value.
   * Pass `value = null` to keep the existing Keychain/vault entry unchanged.
   */
  updateSecret: (
    id: string,
    label: string,
    value: string | null,
    source: SecretSource,
  ) => Promise<void>;
  deleteSecret: (id: string, source: SecretSource) => Promise<void>;
}

export const useSecrets = create<SecretsStore>((set) => ({
  secrets: [],
  loading: false,

  fetchSecrets: async () => {
    set({ loading: true });
    try {
      const secrets = await api.listSecrets();
      set({ secrets });
    } finally {
      set({ loading: false });
    }
  },

  addSecret: async (id, label, value, source) => {
    await api.setSecret(id, label, value, source);
    const secrets = await api.listSecrets();
    set({ secrets });
  },

  updateSecret: async (id, label, value, source) => {
    await api.setSecret(id, label, value, source);
    const secrets = await api.listSecrets();
    set({ secrets });
  },

  deleteSecret: async (id, source) => {
    await api.deleteSecret(id, source);
    set((state) => ({
      secrets: state.secrets.filter((s) => s.id !== id),
    }));
  },
}));
