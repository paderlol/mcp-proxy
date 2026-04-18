//! User preferences persisted to a small JSON file in the app data dir.
//!
//! Kept in a separate file from `servers.json` / `secrets_meta.json` so that
//! unrelated code paths don't have to co-serialize. The only preference that
//! exists today is [`Preferences::prefer_local_vault`] — a macOS-only opt-in
//! for the AES-256-GCM vault instead of Keychain.
//!
//! Both the Tauri GUI and the standalone CLI read this file. The CLI needs
//! it because the choice of backend must be consistent between the two —
//! otherwise a secret stored via the GUI in the vault would be invisible to
//! `mcp-proxy run` (which would look in Keychain), or vice versa.

use crate::store::{app_data_dir, load_json, save_json};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Filename for the preferences blob inside [`app_data_dir`].
const PREFERENCES_FILE: &str = "preferences.json";

/// Persisted user preferences. New optional fields can be added without
/// breaking older clients — missing fields fall back to [`Default`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// macOS-only: when true, route "Local" secrets through the AES-256-GCM
    /// encrypted vault instead of the system Keychain. Ignored on other
    /// platforms (they always use the encrypted vault).
    pub prefer_local_vault: bool,
}

/// Full path to the preferences file.
pub fn preferences_path() -> PathBuf {
    app_data_dir().join(PREFERENCES_FILE)
}

/// Load preferences from disk. Missing / corrupted file → defaults (no panic).
pub fn load() -> Preferences {
    load_json::<Preferences>(preferences_path()).unwrap_or_default()
}

/// Persist preferences atomically (via `save_json`). Failures are swallowed
/// (written to logs, not returned) — the caller has no meaningful recovery.
pub fn save(prefs: &Preferences) {
    save_json(preferences_path(), prefs);
}

/// Convenience: update a single field via a closure and persist the result.
pub fn update<F: FnOnce(&mut Preferences)>(f: F) -> Preferences {
    let mut prefs = load();
    f(&mut prefs);
    save(&prefs);
    prefs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{test_env_lock, DATA_DIR_ENV};

    fn with_temp_data_dir<F: FnOnce()>(f: F) {
        let _lock = test_env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let prev = std::env::var(DATA_DIR_ENV).ok();
        unsafe {
            std::env::set_var(DATA_DIR_ENV, tmp.path());
        }
        f();
        unsafe {
            match prev {
                Some(v) => std::env::set_var(DATA_DIR_ENV, v),
                None => std::env::remove_var(DATA_DIR_ENV),
            }
        }
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        with_temp_data_dir(|| {
            let prefs = load();
            assert_eq!(prefs, Preferences::default());
            assert!(!prefs.prefer_local_vault);
        });
    }

    #[test]
    fn save_then_load_round_trips() {
        with_temp_data_dir(|| {
            save(&Preferences {
                prefer_local_vault: true,
            });
            let loaded = load();
            assert!(loaded.prefer_local_vault);
        });
    }

    #[test]
    fn update_closure_mutates_and_persists() {
        with_temp_data_dir(|| {
            let after = update(|p| p.prefer_local_vault = true);
            assert!(after.prefer_local_vault);
            // Reopen from disk to confirm persistence.
            assert!(load().prefer_local_vault);
        });
    }

    #[test]
    fn unknown_fields_are_ignored_and_defaults_fill_missing() {
        with_temp_data_dir(|| {
            // Simulate a file written by a newer version with a field we
            // don't know about, and missing `prefer_local_vault`.
            std::fs::write(preferences_path(), r#"{"future_setting": "wibble"}"#).unwrap();
            let loaded = load();
            assert_eq!(loaded, Preferences::default());
        });
    }

    #[test]
    fn corrupted_file_falls_back_to_default_without_panic() {
        with_temp_data_dir(|| {
            std::fs::write(preferences_path(), "not json at all").unwrap();
            let loaded = load();
            assert_eq!(loaded, Preferences::default());
        });
    }
}
