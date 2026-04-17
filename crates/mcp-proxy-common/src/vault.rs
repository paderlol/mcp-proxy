//! AES-256-GCM encrypted secret vault.
//!
//! Platform-agnostic implementation used by `local_backend.rs` on non-macOS
//! platforms (macOS uses Keychain instead).
//!
//! # Threat model
//!
//! The vault protects **at-rest confidentiality** of secrets against:
//! - An attacker with read access to `vault.bin` (stolen laptop, leaked backup)
//! - Accidental exposure of the vault file (git commit, cloud sync)
//!
//! It does **not** protect against:
//! - An attacker who can read process memory while the vault is unlocked
//! - An attacker who can capture the master password (keylogger, shoulder surf)
//! - An attacker who can read `/proc/PID/environ` when the CLI runs with
//!   `MCP_PROXY_MASTER_PASSWORD` set
//!
//! # File format
//!
//! ```text
//!   offset  size  field
//!     0      4    magic        b"MPVL"
//!     4      1    version      0x01
//!     5      16   salt         random per vault (for Argon2id)
//!    21      12   nonce        random per write (for AES-GCM)
//!    33      N    ciphertext   AES-256-GCM(plaintext = JSON entries); last
//!                              16 bytes are the auth tag.
//! ```
//!
//! # Crypto
//!
//! - Key derivation: Argon2id, OWASP "interactive" params
//!   (memory = 19456 KiB, iterations = 2, parallelism = 1) → 32 bytes
//! - Cipher: AES-256-GCM with a fresh 96-bit nonce per write
//! - The GCM auth tag protects against tampering; wrong password or flipped
//!   byte anywhere in the header or ciphertext produces [`VaultError::WrongPasswordOrCorrupted`].

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"MPVL";
const VERSION: u8 = 0x01;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN; // 33

/// OWASP-recommended Argon2id params for interactive use.
/// <https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html>
const ARGON2_MEM_KIB: u32 = 19_456;
const ARGON2_ITERS: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;

/// All failure modes a caller needs to distinguish.
#[derive(Debug)]
pub enum VaultError {
    /// File IO error (missing parent dir, permission denied, etc.).
    Io(String),
    /// Magic bytes don't match "MPVL" — probably not a vault file.
    BadMagic,
    /// Version byte is unknown — the file was written by a newer mcp-proxy.
    UnsupportedVersion(u8),
    /// File is shorter than the minimum header — truncated or not a vault.
    Truncated,
    /// AES-GCM authentication failed: either the password is wrong or the
    /// ciphertext / header has been tampered with. Indistinguishable by design.
    WrongPasswordOrCorrupted,
    /// Plaintext decoded but is not the expected JSON shape.
    CorruptedJson(String),
    /// `create` called on a path that already has a file.
    AlreadyExists(PathBuf),
    /// Argon2 KDF itself failed (very rare — bad params).
    Kdf(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::Io(e) => write!(f, "vault I/O error: {e}"),
            VaultError::BadMagic => write!(f, "vault file has unexpected magic bytes"),
            VaultError::UnsupportedVersion(v) => {
                write!(f, "vault version {v} is not supported by this build")
            }
            VaultError::Truncated => write!(f, "vault file is truncated"),
            VaultError::WrongPasswordOrCorrupted => {
                write!(f, "wrong master password, or vault file has been modified")
            }
            VaultError::CorruptedJson(e) => write!(f, "vault plaintext is not valid JSON: {e}"),
            VaultError::AlreadyExists(p) => {
                write!(f, "vault file already exists at {}", p.display())
            }
            VaultError::Kdf(e) => write!(f, "argon2 key derivation failed: {e}"),
        }
    }
}

impl std::error::Error for VaultError {}

impl From<VaultError> for String {
    fn from(e: VaultError) -> String {
        e.to_string()
    }
}

/// Plaintext shape — `BTreeMap` for deterministic serialization order.
#[derive(Serialize, Deserialize, Default)]
struct VaultContents {
    entries: BTreeMap<String, String>,
}

/// An open vault. Holding this struct means the derived key is in memory.
/// `Zeroizing` on the key ensures memory is scrubbed on drop.
pub struct Vault {
    path: PathBuf,
    salt: [u8; SALT_LEN],
    derived_key: Zeroizing<[u8; 32]>,
}

// Manual Debug that deliberately avoids printing the derived key or salt —
// we don't want either ending up in tracing output, panics, or test logs.
impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("path", &self.path)
            .field("salt", &"<redacted>")
            .field("derived_key", &"<redacted>")
            .finish()
    }
}

impl Vault {
    /// Returns true if a file exists at `path`. Doesn't validate content.
    pub fn exists(path: &Path) -> bool {
        path.is_file()
    }

    /// Create a brand-new vault file at `path`, encrypted with `password`.
    /// Refuses to overwrite an existing file — callers can detect this via
    /// [`VaultError::AlreadyExists`] and prompt for confirmation if they
    /// really mean to reset.
    pub fn create(path: PathBuf, password: &str) -> Result<Self, VaultError> {
        if path.exists() {
            return Err(VaultError::AlreadyExists(path));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| VaultError::Io(e.to_string()))?;
        }

        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        let derived_key = derive_key(password, &salt)?;

        let me = Vault {
            path,
            salt,
            derived_key,
        };
        me.write_contents(&VaultContents::default())?;
        Ok(me)
    }

    /// Open an existing vault with `password`. Verifies by attempting to
    /// decrypt the current ciphertext — on failure returns
    /// [`VaultError::WrongPasswordOrCorrupted`] without distinguishing
    /// between wrong password and tampered file (that would leak info).
    pub fn open(path: PathBuf, password: &str) -> Result<Self, VaultError> {
        let raw = fs::read(&path).map_err(|e| VaultError::Io(e.to_string()))?;
        let ParsedHeader { salt, .. } = parse_header(&raw)?;
        let derived_key = derive_key(password, &salt)?;

        // Round-trip decrypt to validate the password before returning the
        // vault handle. We discard the plaintext here; subsequent get/set
        // calls re-read the file on demand.
        let _ = decrypt_file(&raw, &derived_key)?;

        Ok(Vault {
            path,
            salt,
            derived_key,
        })
    }

    /// Look up one entry. `Ok(None)` if the key is not present.
    pub fn get(&self, id: &str) -> Result<Option<Zeroizing<String>>, VaultError> {
        let contents = self.read_contents()?;
        Ok(contents.entries.get(id).map(|v| Zeroizing::new(v.clone())))
    }

    /// Insert or replace an entry. Re-encrypts the whole blob with a fresh
    /// nonce and writes atomically (temp file + rename).
    pub fn set(&self, id: &str, value: &str) -> Result<(), VaultError> {
        let mut contents = self.read_contents()?;
        contents.entries.insert(id.to_string(), value.to_string());
        self.write_contents(&contents)
    }

    /// Remove an entry. Idempotent — returning Ok is safe even if `id` was
    /// absent. Still rewrites the file so the removal is durable.
    pub fn delete(&self, id: &str) -> Result<(), VaultError> {
        let mut contents = self.read_contents()?;
        contents.entries.remove(id);
        self.write_contents(&contents)
    }

    /// Re-encrypt the vault with a new master password. Preserves all
    /// existing entries; generates a fresh salt (so the old password's
    /// derived key can never decrypt the new file, even given the plaintext).
    ///
    /// Mutates self in place: after success, this `Vault` handle can
    /// continue serving reads/writes using the new key.
    pub fn change_password(&mut self, new_password: &str) -> Result<(), VaultError> {
        // Read plaintext with the current key first. If this fails we abort —
        // never write a new file we can't round-trip.
        let contents = self.read_contents()?;

        // Fresh salt + derived key so the old key becomes useless.
        let mut new_salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut new_salt);
        let new_key = derive_key(new_password, &new_salt)?;

        // Temporarily swap in the new salt/key and write, so `write_contents`
        // uses them. If the write succeeds, the new state is durable; if it
        // fails, restore the old values so the handle stays consistent with
        // whatever is on disk.
        let old_salt = self.salt;
        let old_key = std::mem::replace(&mut self.derived_key, new_key);
        self.salt = new_salt;

        if let Err(e) = self.write_contents(&contents) {
            // Roll back so this handle still matches the file on disk.
            self.salt = old_salt;
            self.derived_key = old_key;
            return Err(e);
        }
        Ok(())
    }

    // --- internal ----------------------------------------------------------

    fn read_contents(&self) -> Result<VaultContents, VaultError> {
        let raw = fs::read(&self.path).map_err(|e| VaultError::Io(e.to_string()))?;
        // Reuse salt validation only: caller's password already derived the key.
        let header = parse_header(&raw)?;
        if header.salt != self.salt {
            // Someone replaced the file under us with a different vault.
            return Err(VaultError::WrongPasswordOrCorrupted);
        }
        let plaintext = decrypt_file(&raw, &self.derived_key)?;
        serde_json::from_slice::<VaultContents>(&plaintext)
            .map_err(|e| VaultError::CorruptedJson(e.to_string()))
    }

    fn write_contents(&self, contents: &VaultContents) -> Result<(), VaultError> {
        let plaintext =
            serde_json::to_vec(contents).map_err(|e| VaultError::CorruptedJson(e.to_string()))?;

        let mut nonce = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce);

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*self.derived_key));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
            .map_err(|e| VaultError::Kdf(format!("AES-GCM encrypt: {e}")))?;

        let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);

        atomic_write(&self.path, &out).map_err(VaultError::Io)
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn derive_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let params = Params::new(ARGON2_MEM_KIB, ARGON2_ITERS, ARGON2_PARALLELISM, Some(32))
        .map_err(|e| VaultError::Kdf(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut out = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(password.as_bytes(), salt, out.as_mut())
        .map_err(|e| VaultError::Kdf(e.to_string()))?;
    Ok(out)
}

/// Parsed header fields for an encrypted vault file.
struct ParsedHeader<'a> {
    salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
    ciphertext: &'a [u8],
}

fn parse_header(raw: &[u8]) -> Result<ParsedHeader<'_>, VaultError> {
    if raw.len() < HEADER_LEN {
        return Err(VaultError::Truncated);
    }
    if &raw[..4] != MAGIC {
        return Err(VaultError::BadMagic);
    }
    let version = raw[4];
    if version != VERSION {
        return Err(VaultError::UnsupportedVersion(version));
    }

    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&raw[5..5 + SALT_LEN]);

    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&raw[5 + SALT_LEN..HEADER_LEN]);

    Ok(ParsedHeader {
        salt,
        nonce,
        ciphertext: &raw[HEADER_LEN..],
    })
}

fn decrypt_file(raw: &[u8], derived_key: &[u8; 32]) -> Result<Vec<u8>, VaultError> {
    let header = parse_header(raw)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(derived_key));
    cipher
        .decrypt(Nonce::from_slice(&header.nonce), header.ciphertext)
        .map_err(|_| VaultError::WrongPasswordOrCorrupted)
}

/// Write bytes to a path atomically by way of temp file + rename. Same
/// technique used in `client_write.rs`.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)
        .map_err(|e| format!("failed to write temp file {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        format!(
            "failed to commit vault write ({} → {}): {e}",
            tmp.display(),
            path.display()
        )
    })
}

// ---------------------------------------------------------------------------
// Tests — runnable on macOS / Linux / Windows since this module has no
// platform-specific code.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn vault_path(tmp: &TempDir) -> PathBuf {
        tmp.path().join("vault.bin")
    }

    #[test]
    fn roundtrip_set_get_delete() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);

        let v = Vault::create(path.clone(), "pw").unwrap();
        v.set("github-pat", "ghp_secret").unwrap();
        v.set("slack-token", "xoxb_secret").unwrap();

        let got = v.get("github-pat").unwrap().unwrap();
        assert_eq!(&*got, "ghp_secret");

        v.delete("github-pat").unwrap();
        assert!(v.get("github-pat").unwrap().is_none());
        assert_eq!(&*v.get("slack-token").unwrap().unwrap(), "xoxb_secret");
    }

    #[test]
    fn open_reads_entries_written_earlier() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);

        {
            let v = Vault::create(path.clone(), "pw").unwrap();
            v.set("k", "v").unwrap();
        }
        // Simulate a fresh process: reopen the file.
        let v2 = Vault::open(path, "pw").unwrap();
        assert_eq!(&*v2.get("k").unwrap().unwrap(), "v");
    }

    #[test]
    fn open_with_wrong_password_fails() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        let v = Vault::create(path.clone(), "right").unwrap();
        v.set("k", "v").unwrap();
        drop(v);

        let err = Vault::open(path, "wrong").unwrap_err();
        assert!(matches!(err, VaultError::WrongPasswordOrCorrupted));
    }

    #[test]
    fn open_with_tampered_ciphertext_fails() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        {
            let v = Vault::create(path.clone(), "pw").unwrap();
            v.set("k", "v").unwrap();
        }
        // Flip a byte in the ciphertext region
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        fs::write(&path, &bytes).unwrap();

        let err = Vault::open(path, "pw").unwrap_err();
        assert!(matches!(err, VaultError::WrongPasswordOrCorrupted));
    }

    #[test]
    fn open_with_tampered_salt_fails() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        {
            Vault::create(path.clone(), "pw").unwrap();
        }
        let mut bytes = fs::read(&path).unwrap();
        bytes[5] ^= 0x01; // first salt byte
        fs::write(&path, &bytes).unwrap();

        let err = Vault::open(path, "pw").unwrap_err();
        assert!(matches!(err, VaultError::WrongPasswordOrCorrupted));
    }

    #[test]
    fn open_with_wrong_magic_fails() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        Vault::create(path.clone(), "pw").unwrap();
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] = b'X';
        fs::write(&path, &bytes).unwrap();

        let err = Vault::open(path, "pw").unwrap_err();
        assert!(matches!(err, VaultError::BadMagic), "got {err:?}");
    }

    #[test]
    fn open_with_unknown_version_fails() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        Vault::create(path.clone(), "pw").unwrap();
        let mut bytes = fs::read(&path).unwrap();
        bytes[4] = 0xFF;
        fs::write(&path, &bytes).unwrap();

        let err = Vault::open(path, "pw").unwrap_err();
        assert!(
            matches!(err, VaultError::UnsupportedVersion(0xFF)),
            "got {err:?}"
        );
    }

    #[test]
    fn open_on_truncated_file_fails() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        Vault::create(path.clone(), "pw").unwrap();
        // Truncate to less than the header length.
        fs::write(&path, b"short").unwrap();

        let err = Vault::open(path, "pw").unwrap_err();
        assert!(matches!(err, VaultError::Truncated));
    }

    #[test]
    fn create_fails_when_file_exists() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        Vault::create(path.clone(), "pw").unwrap();
        let err = Vault::create(path, "pw").unwrap_err();
        assert!(matches!(err, VaultError::AlreadyExists(_)));
    }

    #[test]
    fn set_uses_fresh_nonce_each_write() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        let v = Vault::create(path.clone(), "pw").unwrap();
        v.set("k", "v").unwrap();
        let bytes1 = fs::read(&path).unwrap();
        v.set("k", "v").unwrap();
        let bytes2 = fs::read(&path).unwrap();
        // Same salt, same plaintext → different nonce → different ciphertext bytes.
        assert_ne!(
            &bytes1[5 + SALT_LEN..HEADER_LEN],
            &bytes2[5 + SALT_LEN..HEADER_LEN],
            "nonce must be fresh each write"
        );
    }

    #[test]
    fn delete_nonexistent_entry_is_ok() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        let v = Vault::create(path, "pw").unwrap();
        v.delete("never-existed").unwrap();
        assert!(v.get("never-existed").unwrap().is_none());
    }

    #[test]
    fn multiple_entries_coexist() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        let v = Vault::create(path, "pw").unwrap();
        for i in 0..10 {
            v.set(&format!("id{i}"), &format!("value{i}")).unwrap();
        }
        for i in 0..10 {
            let got = v.get(&format!("id{i}")).unwrap().unwrap();
            assert_eq!(&*got, &format!("value{i}"));
        }
    }

    #[test]
    fn exists_reports_file_presence() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        assert!(!Vault::exists(&path));
        Vault::create(path.clone(), "pw").unwrap();
        assert!(Vault::exists(&path));
    }

    // --- change_password ----------------------------------------------------

    #[test]
    fn change_password_preserves_entries() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        let mut v = Vault::create(path.clone(), "old").unwrap();
        v.set("k1", "v1").unwrap();
        v.set("k2", "v2").unwrap();

        v.change_password("new").unwrap();

        // After rotation, the in-memory handle still works.
        assert_eq!(&*v.get("k1").unwrap().unwrap(), "v1");
        assert_eq!(&*v.get("k2").unwrap().unwrap(), "v2");

        // Reopening with the new password works.
        let v2 = Vault::open(path.clone(), "new").unwrap();
        assert_eq!(&*v2.get("k1").unwrap().unwrap(), "v1");

        // Reopening with the old password fails.
        let err = Vault::open(path, "old").unwrap_err();
        assert!(matches!(err, VaultError::WrongPasswordOrCorrupted));
    }

    #[test]
    fn change_password_rotates_salt() {
        let tmp = TempDir::new().unwrap();
        let path = vault_path(&tmp);
        let mut v = Vault::create(path.clone(), "pw").unwrap();

        let salt_before = fs::read(&path).unwrap()[5..5 + SALT_LEN].to_vec();
        v.change_password("pw2").unwrap();
        let salt_after = fs::read(&path).unwrap()[5..5 + SALT_LEN].to_vec();

        assert_ne!(
            salt_before, salt_after,
            "change_password must rotate the salt so the old key is useless"
        );
    }
}
