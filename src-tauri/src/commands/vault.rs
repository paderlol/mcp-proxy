//! Tauri commands for the Local secret vault.
//!
//! On macOS the backend is Keychain and the vault concept is effectively a
//! no-op (always unlocked, always "exists" in the sense that Keychain is
//! always available). On Linux/Windows these commands drive the real
//! AES-256-GCM vault via [`mcp_proxy_common::local_backend`].

use mcp_proxy_common::local_backend;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct VaultStatus {
    /// Which backend Local routes to. `"keychain"` on macOS; `"encrypted-file"` otherwise.
    pub backend: &'static str,
    /// Whether a vault file already exists on disk. Always `true` for Keychain
    /// (there's no per-process file) — callers should only gate UI on
    /// `exists` when `backend == "encrypted-file"`.
    pub exists: bool,
    /// Whether the vault is currently available for reads/writes in this process.
    pub unlocked: bool,
}

#[tauri::command]
pub async fn vault_status() -> Result<VaultStatus, String> {
    Ok(VaultStatus {
        backend: local_backend::backend_id(),
        // Keychain is always "present" from our view; see VaultStatus doc.
        exists: matches!(
            local_backend::default_backend(),
            local_backend::LocalBackend::Keychain
        ) || local_backend::vault_exists(),
        unlocked: local_backend::is_unlocked(),
    })
}

#[tauri::command]
pub async fn unlock_vault(password: String) -> Result<(), String> {
    local_backend::unlock_vault(&password)
}

#[tauri::command]
pub async fn lock_vault() -> Result<(), String> {
    local_backend::lock_vault();
    Ok(())
}
