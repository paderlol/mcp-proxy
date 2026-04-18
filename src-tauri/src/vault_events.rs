use crate::commands::vault::VaultStatus;
use mcp_proxy_common::{local_backend, preferences};
use tauri::{AppHandle, Emitter, Runtime};

pub const VAULT_STATUS_EVENT: &str = "vault-status-changed";

pub fn current_status() -> VaultStatus {
    let prefer_local_vault = preferences::load().prefer_local_vault;
    let can_switch_backend = cfg!(target_os = "macos");
    VaultStatus {
        backend: local_backend::backend_id(),
        exists: matches!(
            local_backend::default_backend(),
            local_backend::LocalBackend::Keychain
        ) || local_backend::vault_exists(),
        unlocked: local_backend::is_unlocked(),
        prefer_local_vault,
        can_switch_backend,
    }
}

pub fn emit<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.emit(VAULT_STATUS_EVENT, current_status())
}
