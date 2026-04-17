//! Tauri commands for the Local secret vault.
//!
//! On macOS the backend is Keychain and the vault concept is effectively a
//! no-op (always unlocked, always "exists" in the sense that Keychain is
//! always available). On Linux/Windows these commands drive the real
//! AES-256-GCM vault via [`mcp_proxy_common::local_backend`].

use mcp_proxy_common::local_backend;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Clone, Debug, Serialize)]
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
    Ok(crate::vault_events::current_status())
}

#[tauri::command]
pub async fn unlock_vault(app: AppHandle, password: String) -> Result<(), String> {
    local_backend::unlock_vault(&password)?;
    crate::vault_events::emit(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn lock_vault(app: AppHandle) -> Result<(), String> {
    local_backend::lock_vault();
    crate::vault_events::emit(&app).map_err(|e| e.to_string())
}

/// Rotate the master password. Requires the vault to currently be unlocked.
#[tauri::command]
pub async fn change_vault_password(app: AppHandle, new_password: String) -> Result<(), String> {
    local_backend::change_password(&new_password)?;
    crate::vault_events::emit(&app).map_err(|e| e.to_string())
}

/// Delete the vault file — wipes all Local secrets. Caller MUST confirm with
/// the user before invoking; this command itself asks no questions.
#[tauri::command]
pub async fn reset_vault(app: AppHandle) -> Result<(), String> {
    local_backend::reset_vault()?;
    crate::vault_events::emit(&app).map_err(|e| e.to_string())
}
