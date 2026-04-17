//! Shared secret resolution — used by both the Tauri app and the standalone CLI.

use crate::local_backend::{delete_local, get_local, set_local};
use crate::models::SecretSource;

/// Resolve a secret value from its backing store.
///
/// - `Local`: delegates to `local_backend` (Keychain on macOS, encrypted file elsewhere)
/// - `OnePassword`: invokes `op read <reference>` (requires 1Password CLI)
pub async fn resolve_secret(id: &str, source: &SecretSource) -> Result<String, String> {
    match source {
        SecretSource::Local => get_local(id).await,
        SecretSource::OnePassword { reference } => {
            let output = tokio::process::Command::new("op")
                .args(["read", reference])
                .output()
                .await
                .map_err(|e| format!("Failed to run `op` CLI: {e}. Is 1Password CLI installed?"))?;
            if !output.status.success() {
                return Err(format!(
                    "1Password `op read` failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
    }
}

/// Store a secret value in the `Local` backend.
/// Not applicable to `OnePassword` (references are fetched, not written).
pub fn store_secret_local(id: &str, value: &str) -> Result<(), String> {
    set_local(id, value)
}

/// Delete a secret from the `Local` backend. Idempotent.
pub fn delete_secret_local(id: &str) -> Result<(), String> {
    delete_local(id)
}
