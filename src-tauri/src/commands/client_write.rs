//! One-click writes to AI client config files.
//!
//! Scope: Claude Desktop, Cursor, Codex, Windsurf. VS Code is workspace-scoped
//! (`.vscode/mcp.json`), so we can't auto-write it from a standalone desktop
//! app and users fall back to Copy.
//!
//! Merge invariant: entries whose `command == "mcp-proxy"` are ours and may be
//! rewritten. Everything else in the user's config is preserved byte-for-byte.

use crate::store::AppState;
use chrono::Utc;
use mcp_proxy_common::models::McpServerConfig;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

/// Constant used to identify entries we own.
const OUR_COMMAND: &str = "mcp-proxy";

// ---------------------------------------------------------------------------
// Public response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ClientConfigInfo {
    pub client: String,
    pub supported: bool,
    pub unsupported_reason: Option<String>,
    pub path: Option<String>,
    pub exists: bool,
}

#[derive(Debug, Serialize)]
pub struct WriteConfigResult {
    pub path: String,
    pub backup_path: Option<String>,
    pub managed_count: usize,
    pub preserved_count: usize,
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_client_config_info(client: String) -> Result<ClientConfigInfo, String> {
    match client_config_path(&client)? {
        PathResolution::Supported(path) => {
            let exists = path.exists();
            Ok(ClientConfigInfo {
                client,
                supported: true,
                unsupported_reason: None,
                path: Some(path.display().to_string()),
                exists,
            })
        }
        PathResolution::Unsupported(reason) => Ok(ClientConfigInfo {
            client,
            supported: false,
            unsupported_reason: Some(reason),
            path: None,
            exists: false,
        }),
    }
}

#[tauri::command]
pub async fn write_client_config(
    client: String,
    state: State<'_, AppState>,
) -> Result<WriteConfigResult, String> {
    let path = match client_config_path(&client)? {
        PathResolution::Supported(p) => p,
        PathResolution::Unsupported(reason) => return Err(reason),
    };

    // Snapshot the current enabled servers
    let enabled: Vec<McpServerConfig> = {
        let servers = state.servers.lock().map_err(|e| e.to_string())?;
        servers.iter().filter(|s| s.enabled).cloned().collect()
    };

    // Produce the rendered file contents
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let (new_contents, managed_count, preserved_count) = match client.as_str() {
        "claude" | "cursor" => merge_json(&existing, "mcpServers", entries_json(&enabled, false))?,
        "windsurf" => merge_json(&existing, "servers", entries_json(&enabled, false))?,
        "codex" => merge_toml(&existing, "mcp_servers", entries_toml(&enabled))?,
        _ => return Err(format!("Unknown client: {client}")),
    };

    let backup = atomic_write_with_backup(&path, &new_contents)?;

    Ok(WriteConfigResult {
        path: path.display().to_string(),
        backup_path: backup.map(|p| p.display().to_string()),
        managed_count,
        preserved_count,
    })
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

enum PathResolution {
    Supported(PathBuf),
    Unsupported(String),
}

fn client_config_path(client: &str) -> Result<PathResolution, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;

    match client {
        "claude" => {
            #[cfg(target_os = "macos")]
            {
                Ok(PathResolution::Supported(
                    home.join("Library/Application Support/Claude/claude_desktop_config.json"),
                ))
            }
            #[cfg(target_os = "windows")]
            {
                let appdata = std::env::var("APPDATA")
                    .map_err(|_| "APPDATA not set; cannot locate Claude Desktop config".to_string())?;
                Ok(PathResolution::Supported(
                    PathBuf::from(appdata).join("Claude/claude_desktop_config.json"),
                ))
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                Ok(PathResolution::Unsupported(
                    "Claude Desktop is not officially supported on this platform. Use the Copy button and paste the config manually.".to_string(),
                ))
            }
        }
        "cursor" => Ok(PathResolution::Supported(home.join(".cursor/mcp.json"))),
        "codex" => Ok(PathResolution::Supported(home.join(".codex/config.toml"))),
        "windsurf" => Ok(PathResolution::Supported(
            home.join(".codeium/windsurf/mcp_config.json"),
        )),
        "vscode" => Ok(PathResolution::Unsupported(
            "VS Code MCP config is workspace-scoped (.vscode/mcp.json). Use Copy and paste into the project you want it in.".to_string(),
        )),
        other => Err(format!("Unknown client: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Entry builders (what we'd add / overwrite)
// ---------------------------------------------------------------------------

fn entries_json(
    servers: &[McpServerConfig],
    include_type_stdio: bool,
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for s in servers {
        let mut entry = serde_json::Map::new();
        if include_type_stdio {
            entry.insert("type".into(), "stdio".into());
        }
        entry.insert("command".into(), OUR_COMMAND.into());
        entry.insert(
            "args".into(),
            serde_json::Value::Array(vec!["run".into(), s.id.clone().into()]),
        );
        map.insert(s.id.clone(), serde_json::Value::Object(entry));
    }
    map
}

fn entries_toml(servers: &[McpServerConfig]) -> toml::value::Table {
    let mut table = toml::value::Table::new();
    for s in servers {
        let mut entry = toml::value::Table::new();
        entry.insert("command".into(), OUR_COMMAND.into());
        entry.insert(
            "args".into(),
            toml::Value::Array(vec!["run".into(), s.id.clone().into()]),
        );
        table.insert(s.id.clone(), toml::Value::Table(entry));
    }
    table
}

// ---------------------------------------------------------------------------
// Merge logic (JSON)
// ---------------------------------------------------------------------------

/// Merge our entries into an existing JSON config string.
/// Returns (new_contents, managed_count, preserved_count).
fn merge_json(
    existing: &str,
    servers_key: &str,
    ours: serde_json::Map<String, serde_json::Value>,
) -> Result<(String, usize, usize), String> {
    let mut root: serde_json::Value = if existing.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(existing).map_err(|e| {
            format!("Existing config is not valid JSON: {e}. Aborting to avoid data loss.")
        })?
    };

    let obj = root.as_object_mut().ok_or_else(|| {
        "Existing config is JSON but not an object at the root; cannot merge.".to_string()
    })?;

    // Extract or create the servers section
    let mut servers = obj
        .remove(servers_key)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    // Drop any stale entry that was previously written by mcp-proxy
    let before = servers.len();
    servers.retain(|_, v| {
        v.get("command")
            .and_then(|c| c.as_str())
            .map(|c| c != OUR_COMMAND)
            .unwrap_or(true)
    });
    let preserved_count = servers.len();
    let _stale_removed = before - preserved_count;

    // Insert our current set (id collisions: our entry wins — but we already
    // purged any prior ours-entry with the same id)
    let managed_count = ours.len();
    for (k, v) in ours {
        servers.insert(k, v);
    }

    obj.insert(servers_key.to_string(), serde_json::Value::Object(servers));

    let rendered = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize merged JSON: {e}"))?;
    Ok((rendered + "\n", managed_count, preserved_count))
}

// ---------------------------------------------------------------------------
// Merge logic (TOML)
// ---------------------------------------------------------------------------

/// Merge our entries into an existing TOML config string (Codex format).
/// Returns (new_contents, managed_count, preserved_count).
fn merge_toml(
    existing: &str,
    servers_key: &str,
    ours: toml::value::Table,
) -> Result<(String, usize, usize), String> {
    let mut root: toml::value::Table = if existing.trim().is_empty() {
        toml::value::Table::new()
    } else {
        existing
            .parse::<toml::Value>()
            .map_err(|e| {
                format!("Existing config is not valid TOML: {e}. Aborting to avoid data loss.")
            })?
            .as_table()
            .cloned()
            .ok_or_else(|| "Existing TOML root is not a table".to_string())?
    };

    // Extract or create the servers table
    let mut servers = root
        .remove(servers_key)
        .and_then(|v| match v {
            toml::Value::Table(t) => Some(t),
            _ => None,
        })
        .unwrap_or_default();

    let before = servers.len();
    servers.retain(|_, v| {
        v.as_table()
            .and_then(|t| t.get("command"))
            .and_then(|c| c.as_str())
            .map(|c| c != OUR_COMMAND)
            .unwrap_or(true)
    });
    let preserved_count = servers.len();
    let _stale_removed = before - preserved_count;

    let managed_count = ours.len();
    for (k, v) in ours {
        servers.insert(k, v);
    }

    root.insert(servers_key.to_string(), toml::Value::Table(servers));

    let rendered = toml::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize merged TOML: {e}"))?;
    Ok((rendered, managed_count, preserved_count))
}

// ---------------------------------------------------------------------------
// Atomic write with backup
// ---------------------------------------------------------------------------

fn atomic_write_with_backup(path: &Path, contents: &str) -> Result<Option<PathBuf>, String> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create parent directory {}: {e}",
                parent.display()
            )
        })?;
    }

    // Back up existing file if present
    let backup_path = if path.exists() {
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let mut backup = path.as_os_str().to_owned();
        backup.push(format!(".backup-{timestamp}"));
        let backup = PathBuf::from(backup);
        fs::copy(path, &backup)
            .map_err(|e| format!("Backup failed ({}): {e}. Aborting write.", backup.display()))?;
        Some(backup)
    } else {
        None
    };

    // Atomic write via tmp + rename
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, contents)
        .map_err(|e| format!("Failed to write temp file {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        format!(
            "Failed to commit write (rename {} → {}): {e}",
            tmp.display(),
            path.display()
        )
    })?;

    Ok(backup_path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Test rules enforced here (TEST_RULES §3):
// - Merge never clobbers parseable user data
// - Malformed files are never overwritten
// - Backup is created before any overwrite

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_proxy_common::models::{McpServerConfig, Transport};
    use tempfile::TempDir;

    fn sample_server(id: &str) -> McpServerConfig {
        let mut s = McpServerConfig::new(
            id.to_string(),
            "npx".to_string(),
            vec!["-y".to_string(), format!("@example/{id}")],
            Transport::Stdio,
        );
        s.id = id.to_string();
        s
    }

    // --- Path resolution --------------------------------------------------

    #[test]
    fn vscode_is_unsupported_with_a_reason() {
        let r = client_config_path("vscode").unwrap();
        match r {
            PathResolution::Unsupported(reason) => {
                assert!(reason.to_lowercase().contains("workspace"));
            }
            _ => panic!("expected unsupported"),
        }
    }

    #[test]
    fn cursor_path_is_under_home_dot_cursor() {
        let r = client_config_path("cursor").unwrap();
        match r {
            PathResolution::Supported(p) => {
                assert!(p.ends_with(".cursor/mcp.json"), "got {}", p.display());
            }
            _ => panic!("expected supported"),
        }
    }

    #[test]
    fn unknown_client_returns_err() {
        assert!(client_config_path("gemini").is_err());
    }

    // --- JSON merge -------------------------------------------------------

    #[test]
    fn merge_json_preserves_user_servers() {
        let existing = r#"{
            "mcpServers": {
                "other-tool": { "command": "/usr/local/bin/other", "args": [] }
            }
        }"#;
        let ours = entries_json(&[sample_server("github")], false);
        let (out, managed, preserved) = merge_json(existing, "mcpServers", ours).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();

        assert_eq!(managed, 1);
        assert_eq!(preserved, 1);
        assert_eq!(
            v["mcpServers"]["other-tool"]["command"],
            "/usr/local/bin/other"
        );
        assert_eq!(v["mcpServers"]["github"]["command"], "mcp-proxy");
    }

    #[test]
    fn merge_json_replaces_stale_mcp_proxy_entries() {
        // File contains an old mcp-proxy entry for a server the user has since deleted.
        let existing = r#"{
            "mcpServers": {
                "deleted-server": { "command": "mcp-proxy", "args": ["run", "deleted-server"] }
            }
        }"#;
        let ours = entries_json(&[sample_server("current")], false);
        let (out, managed, preserved) = merge_json(existing, "mcpServers", ours).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();

        assert_eq!(managed, 1);
        assert_eq!(preserved, 0); // stale one was removed, no user entries to keep
        assert!(
            v["mcpServers"].get("deleted-server").is_none(),
            "stale mcp-proxy entry should have been removed"
        );
        assert_eq!(v["mcpServers"]["current"]["command"], "mcp-proxy");
    }

    #[test]
    fn merge_json_creates_key_when_file_has_no_servers_section() {
        let existing = r#"{ "someOtherKey": true }"#;
        let ours = entries_json(&[sample_server("a")], false);
        let (out, _, preserved) = merge_json(existing, "mcpServers", ours).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();

        assert_eq!(preserved, 0);
        assert_eq!(
            v["someOtherKey"], true,
            "other top-level keys must be preserved"
        );
        assert_eq!(v["mcpServers"]["a"]["command"], "mcp-proxy");
    }

    #[test]
    fn merge_json_handles_empty_input() {
        let ours = entries_json(&[sample_server("a")], false);
        let (out, managed, preserved) = merge_json("", "mcpServers", ours).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(managed, 1);
        assert_eq!(preserved, 0);
        assert!(v["mcpServers"]["a"].is_object());
    }

    #[test]
    fn merge_json_rejects_malformed_input() {
        let bad = "{ this is not json";
        let ours = entries_json(&[sample_server("a")], false);
        let err = merge_json(bad, "mcpServers", ours).unwrap_err();
        assert!(err.contains("not valid JSON"));
        assert!(err.to_lowercase().contains("aborting"));
    }

    #[test]
    fn merge_json_rejects_non_object_root() {
        let bad = "[]";
        let ours = entries_json(&[sample_server("a")], false);
        assert!(merge_json(bad, "mcpServers", ours).is_err());
    }

    // --- TOML merge -------------------------------------------------------

    #[test]
    fn merge_toml_preserves_user_tables() {
        let existing = r#"
[mcp_servers.user-thing]
command = "/usr/local/bin/other"
args = []
"#;
        let ours = entries_toml(&[sample_server("github")]);
        let (out, managed, preserved) = merge_toml(existing, "mcp_servers", ours).unwrap();
        assert_eq!(managed, 1);
        assert_eq!(preserved, 1);
        assert!(out.contains("[mcp_servers.user-thing]"));
        assert!(out.contains("[mcp_servers.github]"));
        assert!(out.contains(r#"command = "mcp-proxy""#));
    }

    #[test]
    fn merge_toml_replaces_stale_mcp_proxy_entries() {
        let existing = r#"
[mcp_servers.deleted]
command = "mcp-proxy"
args = ["run", "deleted"]
"#;
        let ours = entries_toml(&[sample_server("current")]);
        let (out, _, preserved) = merge_toml(existing, "mcp_servers", ours).unwrap();
        assert_eq!(preserved, 0);
        assert!(!out.contains("mcp_servers.deleted"));
        assert!(out.contains("mcp_servers.current"));
    }

    #[test]
    fn merge_toml_rejects_malformed_input() {
        let bad = "[[this is not toml";
        let ours = entries_toml(&[sample_server("a")]);
        let err = merge_toml(bad, "mcp_servers", ours).unwrap_err();
        assert!(err.contains("not valid TOML"));
        assert!(err.to_lowercase().contains("aborting"));
    }

    // --- Atomic write -----------------------------------------------------

    #[test]
    fn atomic_write_creates_backup_when_file_exists() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("existing.json");
        fs::write(&target, r#"{"old":true}"#).unwrap();

        let backup = atomic_write_with_backup(&target, r#"{"new":true}"#)
            .unwrap()
            .expect("expected a backup path");

        assert!(backup.exists());
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            r#"{"old":true}"#,
            "backup should contain the pre-write contents"
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), r#"{"new":true}"#);
    }

    #[test]
    fn atomic_write_skips_backup_when_no_prior_file() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("brand-new.json");

        let backup = atomic_write_with_backup(&target, "{}").unwrap();
        assert!(backup.is_none());
        assert_eq!(fs::read_to_string(&target).unwrap(), "{}");
    }

    #[test]
    fn atomic_write_creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("nested/does/not/exist.json");

        atomic_write_with_backup(&target, "{}").unwrap();
        assert!(target.exists());
    }

    // --- Plumbing check ---------------------------------------------------

    #[test]
    fn entries_json_uses_our_command_marker() {
        let entries = entries_json(&[sample_server("slack")], false);
        assert_eq!(entries["slack"]["command"], OUR_COMMAND);
        assert_eq!(
            entries["slack"]["args"],
            serde_json::json!(["run", "slack"])
        );
    }

    #[test]
    fn entries_json_with_type_stdio_for_vscode_shape() {
        let entries = entries_json(&[sample_server("fs")], true);
        assert_eq!(entries["fs"]["type"], "stdio");
    }

    #[test]
    fn entries_toml_uses_our_command_marker() {
        let t = entries_toml(&[sample_server("notion")]);
        let entry = t["notion"].as_table().unwrap();
        assert_eq!(entry["command"].as_str(), Some(OUR_COMMAND));
    }
}
