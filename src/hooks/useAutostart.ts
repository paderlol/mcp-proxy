import { useEffect, useState, useCallback } from "react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";

/**
 * Manage the "Launch MCP Proxy at login" setting.
 *
 * Backed by the OS-native autostart mechanism via `tauri-plugin-autostart`:
 * - macOS: LaunchAgent plist in `~/Library/LaunchAgents`
 * - Linux: `~/.config/autostart/*.desktop`
 * - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
 *
 * The hook handles the common failure mode (Tauri runtime absent, e.g.
 * browser preview) by exposing `supported: false` — callers should hide the
 * control in that case rather than show an error.
 */
export function useAutostart() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [supported, setSupported] = useState(true);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setEnabled(await isEnabled());
      setSupported(true);
    } catch {
      setSupported(false);
      setEnabled(null);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const toggle = useCallback(async () => {
    if (enabled === null) return;
    setBusy(true);
    try {
      if (enabled) await disable();
      else await enable();
      await refresh();
    } finally {
      setBusy(false);
    }
  }, [enabled, refresh]);

  return { enabled, supported, busy, toggle };
}
