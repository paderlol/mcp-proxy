import { useEffect, useState } from "react";

const STORAGE_KEY = "mcp-proxy.vault.idle-timeout-ms";

/** Default idle timeout when the user hasn't set one yet: 10 minutes. */
export const DEFAULT_IDLE_TIMEOUT_MS = 10 * 60 * 1000;

/** Predefined choices shown in the Settings dropdown. */
export const IDLE_TIMEOUT_CHOICES: { label: string; ms: number }[] = [
  { label: "Never", ms: 0 },
  { label: "5 minutes", ms: 5 * 60 * 1000 },
  { label: "10 minutes", ms: 10 * 60 * 1000 },
  { label: "30 minutes", ms: 30 * 60 * 1000 },
  { label: "1 hour", ms: 60 * 60 * 1000 },
];

/**
 * Persists the user's preferred idle-lock timeout in `localStorage` so it
 * survives app restarts. Returns `[ms, setMs]` just like `useState`.
 */
export function useVaultIdleTimeout(): [number, (ms: number) => void] {
  const [value, setValue] = useState<number>(() => {
    if (typeof window === "undefined") return DEFAULT_IDLE_TIMEOUT_MS;
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (raw === null) return DEFAULT_IDLE_TIMEOUT_MS;
    const parsed = Number(raw);
    return Number.isFinite(parsed) && parsed >= 0
      ? parsed
      : DEFAULT_IDLE_TIMEOUT_MS;
  });

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, String(value));
  }, [value]);

  return [value, setValue];
}
