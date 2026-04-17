//! Short-lived "vault is unlocked" session file.
//!
//! The GUI holds the 32-byte derived key in memory; the CLI (launched by
//! an AI client) needs that same key to decrypt the vault at MCP-server
//! launch. Before this module existed, the CLI had to read the master
//! password from `MCP_PROXY_MASTER_PASSWORD`, which on Linux leaks via
//! `/proc/<PID>/environ`.
//!
//! Instead, this module persists the **already-derived** key to a
//! user-private file at:
//!
//! - Linux: `$XDG_RUNTIME_DIR/mcp-proxy/session.key` (tmpfs, wiped at logout)
//!   with fallback to `$XDG_STATE_HOME/mcp-proxy/session.key`.
//! - macOS: `~/Library/Application Support/mcp-proxy/session.key`
//!   (though macOS uses Keychain by default; the session file is only
//!   written if a caller explicitly opts into the vault backend).
//! - Windows: `%LOCALAPPDATA%\mcp-proxy\session.key`.
//!
//! On Unix the file is created with `0600` mode so only the owning user
//! can read it. The file is deleted when the vault is locked, the password
//! is rotated, the vault is reset, or the GUI process exits cleanly.
//!
//! Compared with env-var passing this is incrementally better:
//! - No `/proc/<PID>/environ` leak.
//! - The exposed material is the derived key only, not the raw password
//!   (so password re-use across services can't be mined from it).
//! - Lifetime is bounded by the GUI session.
//!
//! It is *not* a defense against a same-UID attacker who can read files
//! owned by the user — that's an inherent limit of any user-space secret
//! caching. Document and accept.

use crate::APP_IDENTIFIER;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"MPSS";
const VERSION: u8 = 0x01;
const SALT_LEN: usize = 16;
const KEY_LEN: usize = 32;
const TS_LEN: usize = 8;
const PAYLOAD_LEN: usize = 4 + 1 + SALT_LEN + KEY_LEN + TS_LEN; // 61

/// Resolve the session-key file path for this platform. Creates the parent
/// directory on demand so callers can always proceed with file operations.
pub fn session_path() -> PathBuf {
    let dir = resolve_parent_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join("session.key")
}

fn resolve_parent_dir() -> PathBuf {
    // On Unix-likes prefer `$XDG_RUNTIME_DIR` (tmpfs, cleaned up at logout).
    #[cfg(unix)]
    {
        if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") {
            if !rt.is_empty() {
                return PathBuf::from(rt).join(APP_IDENTIFIER);
            }
        }
        if let Some(state) = dirs::state_dir() {
            return state.join(APP_IDENTIFIER);
        }
    }
    // Last resort: the same data dir we use for vault.bin.
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_IDENTIFIER)
}

/// Persist the derived key + its vault salt to the session file. The file
/// gets `0600` permissions on Unix.
pub fn write(key: &[u8; KEY_LEN], salt: &[u8; SALT_LEN]) -> Result<(), String> {
    let path = session_path();

    let mut payload = Vec::with_capacity(PAYLOAD_LEN);
    payload.extend_from_slice(MAGIC);
    payload.push(VERSION);
    payload.extend_from_slice(salt);
    payload.extend_from_slice(key);
    payload.extend_from_slice(&now_seconds().to_le_bytes());

    write_user_private_file(&path, &payload)
        .map_err(|e| format!("failed to write session file {}: {e}", path.display()))
}

/// Read the session file, returning `(key, salt)` on success. Returns
/// `None` if the file is absent or malformed — callers should fall back to
/// prompting for a password in that case.
///
/// Does *not* validate the key against any particular vault — the caller
/// decides whether the salt matches the current vault file and whether to
/// use the key at all.
pub fn read() -> Option<(Zeroizing<[u8; KEY_LEN]>, [u8; SALT_LEN])> {
    let raw = fs::read(session_path()).ok()?;
    if raw.len() != PAYLOAD_LEN {
        return None;
    }
    if &raw[..4] != MAGIC || raw[4] != VERSION {
        return None;
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&raw[5..5 + SALT_LEN]);

    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    key.copy_from_slice(&raw[5 + SALT_LEN..5 + SALT_LEN + KEY_LEN]);

    Some((key, salt))
}

/// Delete the session file. Best-effort — ignores "not found".
pub fn delete() {
    let _ = fs::remove_file(session_path());
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(unix)]
fn write_user_private_file(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    // Always rewrite, never append; 0600 so only the owning user can read.
    let mut f = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.flush()?;
    // If the file already existed with looser permissions, tighten them.
    fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_user_private_file(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    // On Windows, ACLs are per-file and NTFS inherits from the parent dir.
    // The parent dir under %LOCALAPPDATA% is already user-private, so a
    // plain write is adequate for MVP. A future improvement could set an
    // explicit DACL to deny access to other users.
    fs::write(path, bytes)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests must not clobber a real user's session file, so they
    /// redirect `XDG_RUNTIME_DIR` to a temp dir for the duration. Uses a
    /// mutex because env vars are process-global.
    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_runtime<F: FnOnce()>(f: F) {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let prev = std::env::var("XDG_RUNTIME_DIR").ok();
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }
        f();
        // Clean up any file the test left behind so parallel tests don't
        // see each other's state.
        let _ = fs::remove_file(session_path());
        unsafe {
            match prev {
                Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
                None => std::env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
    }

    #[test]
    fn roundtrip_write_read_delete() {
        with_temp_runtime(|| {
            let key = [7u8; KEY_LEN];
            let salt = [9u8; SALT_LEN];
            write(&key, &salt).unwrap();
            let (got_key, got_salt) = read().expect("session should exist");
            assert_eq!(*got_key, key);
            assert_eq!(got_salt, salt);
            delete();
            assert!(read().is_none(), "session must be gone after delete");
        });
    }

    #[test]
    fn read_missing_returns_none() {
        with_temp_runtime(|| {
            assert!(read().is_none());
        });
    }

    #[test]
    fn read_short_file_returns_none() {
        with_temp_runtime(|| {
            fs::write(session_path(), b"short").unwrap();
            assert!(read().is_none());
        });
    }

    #[test]
    fn read_bad_magic_returns_none() {
        with_temp_runtime(|| {
            // 61 bytes of the wrong magic
            let mut bytes = vec![0u8; PAYLOAD_LEN];
            bytes[..4].copy_from_slice(b"XXXX");
            fs::write(session_path(), &bytes).unwrap();
            assert!(read().is_none());
        });
    }

    #[test]
    fn read_bad_version_returns_none() {
        with_temp_runtime(|| {
            let mut bytes = vec![0u8; PAYLOAD_LEN];
            bytes[..4].copy_from_slice(MAGIC);
            bytes[4] = 0xff;
            fs::write(session_path(), &bytes).unwrap();
            assert!(read().is_none());
        });
    }

    #[test]
    #[cfg(unix)]
    fn file_is_user_private() {
        use std::os::unix::fs::PermissionsExt;
        with_temp_runtime(|| {
            let key = [0u8; KEY_LEN];
            let salt = [0u8; SALT_LEN];
            write(&key, &salt).unwrap();
            let meta = fs::metadata(session_path()).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "session file must be 0600; got {mode:o}");
        });
    }
}
