//! Platform-specific local secret storage.
//!
//! Users see "Local" as a single option; this module routes to:
//! - **macOS**: OS Keychain (hardware-backed on Apple Silicon)
//! - **Linux / Windows**: AES-256-GCM encrypted file (NOT YET IMPLEMENTED)
//!
//! The choice is automatic — users never see `Keychain` vs `EncryptedFile` in the UI.

use crate::KEYCHAIN_SERVICE;

/// Which concrete backend `Local` resolves to on this platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBackend {
    /// macOS Keychain via `keyring` crate.
    Keychain,
    /// AES-256-GCM encrypted vault file (not yet implemented).
    EncryptedFile,
}

/// Returns the backend selected for the current platform.
pub const fn default_backend() -> LocalBackend {
    #[cfg(target_os = "macos")]
    {
        LocalBackend::Keychain
    }
    #[cfg(not(target_os = "macos"))]
    {
        LocalBackend::EncryptedFile
    }
}

/// Human-readable label for the current platform's backend, shown in UI.
pub const fn backend_label() -> &'static str {
    match default_backend() {
        LocalBackend::Keychain => "macOS Keychain",
        LocalBackend::EncryptedFile => "AES-256 Vault (not yet implemented)",
    }
}

/// Read a secret value from the local backend.
pub async fn get_local(id: &str) -> Result<String, String> {
    match default_backend() {
        LocalBackend::Keychain => get_keychain(id).await,
        LocalBackend::EncryptedFile => Err(format!(
            "Local storage not yet implemented on this platform (secret '{id}'). \
             Use 1Password for now — AES-256-GCM vault is tracked in SECURITY_TODO.md."
        )),
    }
}

/// Store a secret value in the local backend.
pub fn set_local(id: &str, value: &str) -> Result<(), String> {
    match default_backend() {
        LocalBackend::Keychain => set_keychain(id, value),
        LocalBackend::EncryptedFile => Err(
            "Local storage not yet implemented on this platform. Use 1Password for now."
                .to_string(),
        ),
    }
}

/// Delete a secret from the local backend. Idempotent.
pub fn delete_local(id: &str) -> Result<(), String> {
    match default_backend() {
        LocalBackend::Keychain => delete_keychain(id),
        // No-op for not-yet-implemented backend; metadata deletion still proceeds.
        LocalBackend::EncryptedFile => Ok(()),
    }
}

// ---- Keychain backend (macOS) ----

async fn get_keychain(id: &str) -> Result<String, String> {
    let id = id.to_string();
    tokio::task::spawn_blocking(move || {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &id)
            .map_err(|e| format!("Keychain error: {e}"))?;
        entry
            .get_password()
            .map_err(|e| format!("Failed to read secret '{id}' from Keychain: {e}"))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

fn set_keychain(id: &str, value: &str) -> Result<(), String> {
    let entry =
        keyring::Entry::new(KEYCHAIN_SERVICE, id).map_err(|e| format!("Keychain error: {e}"))?;
    entry
        .set_password(value)
        .map_err(|e| format!("Failed to store secret '{id}' in Keychain: {e}"))
}

fn delete_keychain(id: &str) -> Result<(), String> {
    let entry =
        keyring::Entry::new(KEYCHAIN_SERVICE, id).map_err(|e| format!("Keychain error: {e}"))?;
    // Idempotent — ignore "not found"
    let _ = entry.delete_credential();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn macos_uses_keychain_backend() {
        assert_eq!(default_backend(), LocalBackend::Keychain);
        assert_eq!(backend_label(), "macOS Keychain");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn non_macos_uses_encrypted_file_backend() {
        assert_eq!(default_backend(), LocalBackend::EncryptedFile);
        assert!(backend_label().contains("Vault"));
    }

    /// On platforms where `EncryptedFile` is still a stub, writes and reads
    /// must return a clear error — never panic, never silently succeed.
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn encrypted_file_backend_returns_not_implemented_error() {
        let err = set_local("some-id", "some-value").unwrap_err();
        assert!(err.to_lowercase().contains("not yet implemented"));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt.block_on(get_local("some-id")).unwrap_err();
        assert!(err.to_lowercase().contains("not yet implemented"));
    }

    /// Delete is idempotent on the stub platform (returns Ok even if the
    /// backend can't really delete anything yet — the metadata layer still
    /// needs to succeed in removing the meta entry).
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn encrypted_file_delete_is_noop_ok() {
        assert!(delete_local("nonexistent").is_ok());
    }
}
