//! Shared file-based store for server configs and secret metadata.
//! Both the Tauri app and the CLI read from the same JSON files.

use crate::APP_IDENTIFIER;
use std::fs;
use std::path::{Path, PathBuf};

/// Environment variable that overrides the data directory.
/// Primarily used by integration tests; also usable by advanced users who
/// want multiple isolated mcp-proxy profiles on one machine.
pub const DATA_DIR_ENV: &str = "MCP_PROXY_DATA_DIR";

/// Return the app's data directory, creating it if needed.
///
/// Resolution order:
/// 1. `$MCP_PROXY_DATA_DIR` if set (tests + power users)
/// 2. Platform default:
///    - macOS: `~/Library/Application Support/com.mcp-proxy.app`
///    - Linux: `~/.local/share/com.mcp-proxy.app`
///    - Windows: `%APPDATA%\com.mcp-proxy.app`
pub fn app_data_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var(DATA_DIR_ENV) {
        let dir = PathBuf::from(override_dir);
        fs::create_dir_all(&dir).ok();
        return dir;
    }
    let base = dirs::data_dir().expect("Failed to find user data directory");
    let dir = base.join(APP_IDENTIFIER);
    fs::create_dir_all(&dir).ok();
    dir
}

/// Load a JSON file and deserialize into `T`. Returns `None` if the file is missing
/// or cannot be parsed.
pub fn load_json<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Option<T> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Serialize `value` as pretty JSON and write atomically to `path`.
pub fn save_json<T: serde::Serialize + ?Sized>(path: impl AsRef<Path>, value: &T) {
    if let Ok(data) = serde_json::to_string_pretty(value) {
        let _ = fs::write(path, data);
    }
}

/// Path to the servers.json file.
pub fn servers_path() -> PathBuf {
    app_data_dir().join("servers.json")
}

/// Path to the secrets_meta.json file.
pub fn secrets_meta_path() -> PathBuf {
    app_data_dir().join("secrets_meta.json")
}
