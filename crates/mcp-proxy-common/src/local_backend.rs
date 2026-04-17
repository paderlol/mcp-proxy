//! Platform-specific local secret storage.
//!
//! Users see "Local" as a single option; this module routes to:
//! - **macOS**: OS Keychain (hardware-backed on Apple Silicon)
//! - **Linux / Windows**: AES-256-GCM encrypted file via [`crate::vault::Vault`]
//!   — master password is held in memory as a derived key while the process
//!   runs; the vault must be unlocked with [`unlock_vault`] before any
//!   read/write.
//!
//! Callers never need to know which backend is active. The choice is made by
//! [`default_backend`] at compile time via `cfg(target_os)`.

use crate::session;
use crate::vault::Vault;
use crate::KEYCHAIN_SERVICE;
use std::sync::{Mutex, OnceLock};
use zeroize::Zeroizing;

/// Which concrete backend `Local` resolves to on this platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBackend {
    /// macOS Keychain via `keyring` crate.
    Keychain,
    /// AES-256-GCM encrypted vault file.
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
        LocalBackend::EncryptedFile => "AES-256 encrypted file",
    }
}

/// Short backend identifier for machine-readable status reporting.
pub const fn backend_id() -> &'static str {
    match default_backend() {
        LocalBackend::Keychain => "keychain",
        LocalBackend::EncryptedFile => "encrypted-file",
    }
}

/// Read a secret value from the local backend.
pub async fn get_local(id: &str) -> Result<String, String> {
    match default_backend() {
        LocalBackend::Keychain => get_keychain(id).await,
        LocalBackend::EncryptedFile => {
            with_vault(|v| match v.get(id).map_err(|e| e.to_string())? {
                Some(z) => Ok((*z).clone()),
                None => Err(format!("secret '{id}' not found in vault")),
            })
        }
    }
}

/// Store a secret value in the local backend.
pub fn set_local(id: &str, value: &str) -> Result<(), String> {
    match default_backend() {
        LocalBackend::Keychain => set_keychain(id, value),
        LocalBackend::EncryptedFile => with_vault(|v| v.set(id, value).map_err(|e| e.to_string())),
    }
}

/// Delete a secret from the local backend. Idempotent.
pub fn delete_local(id: &str) -> Result<(), String> {
    match default_backend() {
        LocalBackend::Keychain => delete_keychain(id),
        LocalBackend::EncryptedFile => {
            // If the vault isn't unlocked we treat this as a no-op so the
            // metadata layer can still forget the entry. Deleting an entry we
            // can't read is fine for the idempotent contract.
            if !is_unlocked() {
                return Ok(());
            }
            with_vault(|v| v.delete(id).map_err(|e| e.to_string()))
        }
    }
}

// ---------------------------------------------------------------------------
// EncryptedFile session state
// ---------------------------------------------------------------------------
//
// On non-macOS the vault key lives in memory only while a `Vault` instance
// is stored here. `unlock_vault` populates it; `lock_vault` or process exit
// drops it (`Vault`'s key field is `Zeroizing`, so memory is scrubbed).

fn session_cell() -> &'static Mutex<Option<Vault>> {
    static CELL: OnceLock<Mutex<Option<Vault>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

/// Path where the vault file lives. Reuses the same data-dir override that
/// `servers.json` / `secrets_meta.json` use, so `MCP_PROXY_DATA_DIR` works
/// here too.
pub fn vault_path() -> std::path::PathBuf {
    crate::store::app_data_dir().join("vault.bin")
}

/// `true` if a vault file is present on disk (regardless of lock state).
pub fn vault_exists() -> bool {
    Vault::exists(&vault_path())
}

/// `true` if the vault is currently unlocked in this process. On macOS this
/// is always `true` because the backend is Keychain, which has no per-process
/// lock concept.
pub fn is_unlocked() -> bool {
    if matches!(default_backend(), LocalBackend::Keychain) {
        return true;
    }
    session_cell()
        .lock()
        .ok()
        .map(|g| g.is_some())
        .unwrap_or(false)
}

/// Unlock the vault (create it if missing). On macOS this is a no-op.
///
/// On success, also persists the derived key to a user-private session
/// file so the CLI can unlock without the `MCP_PROXY_MASTER_PASSWORD`
/// env var. The session file is deleted on [`lock_vault`], password
/// rotation, or vault reset.
pub fn unlock_vault(password: &str) -> Result<(), String> {
    if matches!(default_backend(), LocalBackend::Keychain) {
        return Ok(());
    }
    let path = vault_path();
    let vault = if Vault::exists(&path) {
        Vault::open(path, password)
    } else {
        Vault::create(path, password)
    }
    .map_err(|e| e.to_string())?;

    // Persist the derived key so the CLI can find it without the user re-
    // typing or exporting the password as an env var.
    if let Err(e) = session::write(vault.key_bytes(), vault.salt()) {
        tracing::warn!("failed to persist vault session file: {e}");
    }

    let mut guard = session_cell().lock().map_err(|e| e.to_string())?;
    *guard = Some(vault);
    Ok(())
}

/// Try to unlock using the session file written by a previous GUI unlock.
/// Returns `Ok(true)` if a valid session was found and loaded, `Ok(false)`
/// if there is no session file (caller should fall back to password
/// unlock). Returns an error if the session file exists but doesn't match
/// the current vault (stale after password rotation).
///
/// Intended for the CLI happy path: check once at startup before prompting
/// for a password.
pub fn unlock_from_session() -> Result<bool, String> {
    if matches!(default_backend(), LocalBackend::Keychain) {
        return Ok(true); // nothing to do; treat as "already unlocked"
    }
    let Some((key_bytes, session_salt)) = session::read() else {
        return Ok(false);
    };

    let path = vault_path();
    if !Vault::exists(&path) {
        // Session points at a vault that no longer exists — stale.
        session::delete();
        return Ok(false);
    }

    // `open_with_key` verifies the key decrypts the file (via GCM auth).
    let key = Zeroizing::new(*key_bytes);
    let vault = Vault::open_with_key(path, key).map_err(|e| {
        // Session didn't match the current vault — most likely password was
        // rotated elsewhere. Wipe the stale session and ask the caller to
        // fall back to password.
        session::delete();
        e.to_string()
    })?;

    // Double-check the salt recorded in the session still matches the vault
    // on disk (defense in depth against weird edge cases).
    if vault.salt() != &session_salt {
        session::delete();
        return Err("session file does not match current vault (stale).".into());
    }

    let mut guard = session_cell().lock().map_err(|e| e.to_string())?;
    *guard = Some(vault);
    Ok(true)
}

/// Clear the in-memory vault session (zeroizes the derived key) and
/// delete the session file. No-op on macOS.
pub fn lock_vault() {
    if let Ok(mut guard) = session_cell().lock() {
        *guard = None;
    }
    session::delete();
}

/// Rotate the vault's master password. Requires the vault to already be
/// unlocked (session populated) so we know the current password works.
/// On macOS this is a no-op — Keychain uses the OS login password, which
/// users change via System Settings, not through this app.
pub fn change_password(new_password: &str) -> Result<(), String> {
    if matches!(default_backend(), LocalBackend::Keychain) {
        return Err(
            "Password rotation is a no-op on macOS — the Keychain backend \
             inherits your login password. Use System Settings to change it."
                .to_string(),
        );
    }
    let mut guard = session_cell().lock().map_err(|e| e.to_string())?;
    let vault = guard.as_mut().ok_or_else(|| {
        "vault must be unlocked before changing the password — unlock first".to_string()
    })?;
    vault
        .change_password(new_password)
        .map_err(|e| e.to_string())?;

    // Refresh the on-disk session file with the new derived key so the CLI
    // can keep unlocking without a password prompt.
    if let Err(e) = session::write(vault.key_bytes(), vault.salt()) {
        tracing::warn!("failed to refresh vault session file after password rotation: {e}");
    }
    Ok(())
}

/// Delete the vault file on disk. Also zeros the in-memory session. Caller
/// is expected to have confirmed with the user first (data loss).
/// No-op on macOS (nothing to delete).
pub fn reset_vault() -> Result<(), String> {
    if matches!(default_backend(), LocalBackend::Keychain) {
        return Ok(());
    }
    // Drop session first so any subsequent access requires re-unlock.
    lock_vault();
    let path = vault_path();
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("failed to delete vault file {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Run a closure with the unlocked vault. Returns an error if locked.
fn with_vault<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(&Vault) -> Result<T, String>,
{
    let guard = session_cell()
        .lock()
        .map_err(|e| format!("vault session lock poisoned: {e}"))?;
    match guard.as_ref() {
        Some(v) => f(v),
        None => {
            Err("vault is locked — call unlock_vault() or set MCP_PROXY_MASTER_PASSWORD".into())
        }
    }
}

// ---------------------------------------------------------------------------
// Keychain backend (macOS)
// ---------------------------------------------------------------------------

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
    #[cfg(not(target_os = "macos"))]
    use crate::store::DATA_DIR_ENV;
    #[cfg(not(target_os = "macos"))]
    use std::fs;
    #[cfg(not(target_os = "macos"))]
    use std::sync::Mutex;

    #[test]
    #[cfg(target_os = "macos")]
    fn macos_uses_keychain_backend() {
        assert_eq!(default_backend(), LocalBackend::Keychain);
        assert_eq!(backend_id(), "keychain");
        assert_eq!(backend_label(), "macOS Keychain");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn is_unlocked_always_true_on_macos() {
        // On macOS there is no vault state — Keychain unlocking is handled
        // by the OS, so the concept is always "unlocked" from our view.
        assert!(is_unlocked());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn unlock_and_lock_are_noops_on_macos() {
        // These must not error on macOS even though we don't have a vault
        // file — callers can always invoke them unconditionally.
        unlock_vault("ignored").unwrap();
        lock_vault();
        assert!(is_unlocked());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn non_macos_uses_encrypted_file_backend() {
        assert_eq!(default_backend(), LocalBackend::EncryptedFile);
        assert_eq!(backend_id(), "encrypted-file");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn locked_read_fails_with_clear_error() {
        // Ensure sessions start locked in this test's process state.
        lock_vault();
        assert!(!is_unlocked());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt.block_on(get_local("whatever")).unwrap_err();
        assert!(err.to_lowercase().contains("vault is locked"), "got: {err}");
    }

    #[cfg(not(target_os = "macos"))]
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[cfg(not(target_os = "macos"))]
    fn with_temp_profile<F: FnOnce()>(f: F) {
        let _lock = ENV_LOCK.lock().unwrap();
        let data_dir = tempfile::tempdir().unwrap();
        let runtime_dir = tempfile::tempdir().unwrap();
        let prev_data = std::env::var(DATA_DIR_ENV).ok();
        let prev_runtime = std::env::var("XDG_RUNTIME_DIR").ok();

        unsafe {
            std::env::set_var(DATA_DIR_ENV, data_dir.path());
            std::env::set_var("XDG_RUNTIME_DIR", runtime_dir.path());
        }

        lock_vault();
        f();
        lock_vault();

        unsafe {
            match prev_data {
                Some(v) => std::env::set_var(DATA_DIR_ENV, v),
                None => std::env::remove_var(DATA_DIR_ENV),
            }
            match prev_runtime {
                Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
                None => std::env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn reset_vault_deletes_vault_file_and_session() {
        with_temp_profile(|| {
            unlock_vault("pw1").unwrap();
            assert!(
                vault_path().exists(),
                "vault file should exist after unlock"
            );
            assert!(
                session::session_path().exists(),
                "session file should exist after unlock"
            );

            reset_vault().unwrap();

            assert!(!vault_path().exists(), "reset should remove vault.bin");
            assert!(
                !session::session_path().exists(),
                "reset should remove the vault session file"
            );
            assert!(!is_unlocked(), "vault should be locked after reset");
        });
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn change_password_rotates_session_and_invalidates_old_password() {
        with_temp_profile(|| {
            unlock_vault("pw1").unwrap();
            set_local("api-key", "secret").unwrap();
            let session_before = fs::read(session::session_path()).unwrap();

            change_password("pw2").unwrap();

            let session_after = fs::read(session::session_path()).unwrap();
            assert_ne!(
                session_before, session_after,
                "password rotation should refresh the session file"
            );

            lock_vault();
            let old_password_err = unlock_vault("pw1").unwrap_err();
            assert!(
                old_password_err.contains("wrong master password")
                    || old_password_err.contains("modified"),
                "expected old password to stop working, got: {old_password_err}"
            );

            unlock_vault("pw2").unwrap();
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let value = rt.block_on(get_local("api-key")).unwrap();
            assert_eq!(value, "secret");
        });
    }
}
