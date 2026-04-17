import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { VaultStatus } from "../lib/types";
import { useVault } from "./useVault";

const VAULT_STATUS_EVENT = "vault-status-changed";

/**
 * Keeps the frontend's vault badge/UI in sync with backend-side changes.
 *
 * Tauri commands update the store directly through `useVault`, but tray menu
 * actions happen outside React. We subscribe once at the app root and also
 * refresh on visibility/focus as a fallback after the window is re-shown.
 */
export function useVaultStatusSync() {
  const refresh = useVault((state) => state.refresh);
  const setStatus = useVault((state) => state.setStatus);

  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | null = null;

    void listen<VaultStatus>(VAULT_STATUS_EVENT, (event) => {
      if (mounted) {
        setStatus(event.payload);
      }
    }).then((fn) => {
      if (mounted) {
        unlisten = fn;
      } else {
        fn();
      }
    });

    const syncFromBackend = () => {
      void refresh();
    };

    window.addEventListener("focus", syncFromBackend);
    document.addEventListener("visibilitychange", syncFromBackend, {
      passive: true,
    });

    return () => {
      mounted = false;
      unlisten?.();
      window.removeEventListener("focus", syncFromBackend);
      document.removeEventListener("visibilitychange", syncFromBackend);
    };
  }, [refresh, setStatus]);
}
