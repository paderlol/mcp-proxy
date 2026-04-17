use crate::commands::vault::VaultStatus;
use mcp_proxy_common::local_backend;
use tauri::{AppHandle, Emitter, Runtime};

pub const VAULT_STATUS_EVENT: &str = "vault-status-changed";

pub fn current_status() -> VaultStatus {
    VaultStatus {
        backend: local_backend::backend_id(),
        exists: matches!(
            local_backend::default_backend(),
            local_backend::LocalBackend::Keychain
        ) || local_backend::vault_exists(),
        unlocked: local_backend::is_unlocked(),
    }
}

pub fn emit<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.emit(VAULT_STATUS_EVENT, current_status())
}
