/**
 * Detect whether the frontend is running on macOS.
 *
 * Used to gate macOS-only UI (e.g. the Local-mode `sandbox-exec` toggle on the
 * server form). Runtime-only — the check reads `navigator.platform` with a
 * `userAgent` fallback because `navigator.platform` is officially deprecated,
 * and some desktop webview builds quietly stub it to an empty string.
 *
 * In Playwright tests the `navigator` can be mocked via `addInitScript` to
 * exercise both branches; see `tests/e2e/servers.spec.ts`.
 */
export function isMacosPlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  const platform = navigator.platform ?? "";
  if (platform) {
    if (platform.toLowerCase().startsWith("mac")) return true;
    // iPad/iPhone are not macOS; fall through to userAgent for the common
    // Electron/Tauri case where platform is "" on some Linux builds.
    if (platform.toLowerCase().includes("iphone")) return false;
    if (platform.toLowerCase().includes("ipad")) return false;
  }
  const ua = navigator.userAgent ?? "";
  return /Mac OS X|macOS|Macintosh/i.test(ua);
}

/**
 * Hook wrapper. Returns a stable boolean — safe to read during render.
 * Memo-free: the platform never changes inside a single session, so recomputing
 * is cheap.
 */
export function useIsMacos(): boolean {
  return isMacosPlatform();
}
