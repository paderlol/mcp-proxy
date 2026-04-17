import { useEffect, useRef } from "react";
import { useVault } from "./useVault";

/**
 * Automatically lock the vault after `timeoutMs` of no user interaction.
 *
 * Only active when:
 * - the vault backend is `encrypted-file` (Keychain has no process-scoped lock)
 * - the vault is currently unlocked
 * - `timeoutMs` is a positive number (pass 0 to disable)
 *
 * Mount this once near the app root so it sees interaction events regardless
 * of which page is open.
 */
export function useIdleLock(timeoutMs: number) {
  const { status, lock } = useVault();
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    // Only arm when applicable.
    if (
      !status ||
      status.backend !== "encrypted-file" ||
      !status.unlocked ||
      timeoutMs <= 0
    ) {
      return;
    }

    const resetTimer = () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
      }
      timerRef.current = window.setTimeout(() => {
        // Fire-and-forget; lock() refreshes status, which will
        // cause this effect to tear down via the status.unlocked guard above.
        void lock();
      }, timeoutMs);
    };

    // Interaction signals fired on window: any of these resets the idle timer.
    const windowEvents: (keyof WindowEventMap)[] = [
      "mousemove",
      "mousedown",
      "keydown",
      "wheel",
      "touchstart",
    ];
    for (const ev of windowEvents) {
      window.addEventListener(ev, resetTimer, { passive: true });
    }
    // `visibilitychange` is a document event, not a window one. Including it
    // so returning to the tab also resets the timer.
    document.addEventListener("visibilitychange", resetTimer, { passive: true });
    resetTimer();

    return () => {
      for (const ev of windowEvents) {
        window.removeEventListener(ev, resetTimer);
      }
      document.removeEventListener("visibilitychange", resetTimer);
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [status, timeoutMs, lock]);
}
