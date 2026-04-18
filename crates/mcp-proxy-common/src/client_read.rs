//! Read existing AI-client MCP configs so the user can import servers into
//! the proxy instead of retyping them.
//!
//! Mirrors the write-side paths used by the "Supported AI Clients" table in
//! CLAUDE.md. Each parser produces `DiscoveredServer` rows tagged with the
//! source client; the caller dedupes by `(name, command, args)`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SourceClient {
    ClaudeDesktop,
    ClaudeCode,
    Codex,
    Cursor,
    VsCode,
    Windsurf,
}

impl SourceClient {
    pub fn label(self) -> &'static str {
        match self {
            SourceClient::ClaudeDesktop => "Claude Desktop",
            SourceClient::ClaudeCode => "Claude Code",
            SourceClient::Codex => "Codex",
            SourceClient::Cursor => "Cursor",
            SourceClient::VsCode => "VS Code",
            SourceClient::Windsurf => "Windsurf",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredServer {
    pub source: SourceClient,
    pub source_path: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub transport: String, // "stdio" | "sse" | ...
}

/// Shape shared by Claude Desktop / Cursor / Windsurf config files.
#[derive(Deserialize)]
struct McpServersJson {
    #[serde(default, rename = "mcpServers")]
    mcp_servers_camel: HashMap<String, JsonServerEntry>,
    #[serde(default, rename = "servers")]
    servers_lower: HashMap<String, JsonServerEntry>,
}

#[derive(Deserialize, Default)]
struct JsonServerEntry {
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default, rename = "type")]
    ty: Option<String>,
}

fn home() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Default file paths per client.
pub fn client_path(client: SourceClient) -> Option<PathBuf> {
    let h = home()?;
    Some(match client {
        SourceClient::ClaudeDesktop => h
            .join("Library")
            .join("Application Support")
            .join("Claude")
            .join("claude_desktop_config.json"),
        SourceClient::ClaudeCode => h.join(".claude.json"),
        SourceClient::Codex => h.join(".codex").join("config.toml"),
        SourceClient::Cursor => h.join(".cursor").join("mcp.json"),
        SourceClient::VsCode => h.join(".vscode").join("mcp.json"),
        SourceClient::Windsurf => h.join(".codeium").join("windsurf").join("mcp_config.json"),
    })
}

/// Probe every supported client's default path and return all discovered
/// servers, deduped across clients by `(name, command, args)`.
pub fn discover_all() -> Vec<DiscoveredServer> {
    let mut out = Vec::new();
    for client in [
        SourceClient::ClaudeDesktop,
        SourceClient::ClaudeCode,
        SourceClient::Codex,
        SourceClient::Cursor,
        SourceClient::VsCode,
        SourceClient::Windsurf,
    ] {
        if let Some(path) = client_path(client) {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                let path_str = path.display().to_string();
                out.extend(parse_client(client, &path_str, &contents));
            }
        }
    }
    dedupe(&mut out);
    out
}

pub fn parse_client(client: SourceClient, path: &str, contents: &str) -> Vec<DiscoveredServer> {
    match client {
        SourceClient::Codex => parse_codex_toml(path, contents),
        _ => parse_json_like(client, path, contents),
    }
}

fn parse_json_like(client: SourceClient, path: &str, contents: &str) -> Vec<DiscoveredServer> {
    let parsed: Result<McpServersJson, _> = serde_json::from_str(contents);
    let Ok(top) = parsed else { return Vec::new() };
    let mut out = Vec::new();
    let iter = top.mcp_servers_camel.into_iter().chain(top.servers_lower);
    for (name, entry) in iter {
        if let Some(cmd) = entry.command {
            out.push(DiscoveredServer {
                source: client,
                source_path: path.to_string(),
                name,
                command: cmd,
                args: entry.args,
                env: entry.env,
                transport: entry.ty.unwrap_or_else(|| "stdio".to_string()),
            });
        }
    }
    out
}

fn parse_codex_toml(path: &str, contents: &str) -> Vec<DiscoveredServer> {
    let parsed: Result<toml::Value, _> = toml::from_str(contents);
    let Ok(top) = parsed else { return Vec::new() };
    let Some(section) = top.get("mcp_servers").and_then(|v| v.as_table()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, raw) in section {
        let Some(tbl) = raw.as_table() else { continue };
        let Some(command) = tbl.get("command").and_then(|v| v.as_str()) else {
            continue;
        };
        let args: Vec<String> = tbl
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let env: HashMap<String, String> = tbl
            .get("env")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        out.push(DiscoveredServer {
            source: SourceClient::Codex,
            source_path: path.to_string(),
            name: name.clone(),
            command: command.to_string(),
            args,
            env,
            transport: "stdio".to_string(),
        });
    }
    out
}

fn dedupe(servers: &mut Vec<DiscoveredServer>) {
    let mut seen: std::collections::HashSet<(String, String, Vec<String>)> =
        std::collections::HashSet::new();
    servers.retain(|s| seen.insert((s.name.clone(), s.command.clone(), s.args.clone())));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_desktop_style() {
        let json = r#"{
            "mcpServers": {
                "github": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-github"],
                    "env": {"GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_xxx"}
                }
            }
        }"#;
        let out = parse_client(SourceClient::ClaudeDesktop, "/x", json);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "github");
        assert_eq!(out[0].command, "npx");
        assert_eq!(
            out[0].env.get("GITHUB_PERSONAL_ACCESS_TOKEN"),
            Some(&"ghp_xxx".to_string())
        );
    }

    #[test]
    fn parses_vscode_servers_key() {
        // VS Code uses `servers` not `mcpServers`.
        let json = r#"{
            "servers": {
                "brave": {
                    "type": "stdio",
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-brave-search"],
                    "env": {"BRAVE_API_KEY": "abc"}
                }
            }
        }"#;
        let out = parse_client(SourceClient::VsCode, "/x", json);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "brave");
        assert_eq!(out[0].transport, "stdio");
    }

    #[test]
    fn parses_codex_toml() {
        let tomltxt = r#"
[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
[mcp_servers.github.env]
GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_xxx"
"#;
        let out = parse_client(SourceClient::Codex, "/x", tomltxt);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].command, "npx");
        assert_eq!(
            out[0].env.get("GITHUB_PERSONAL_ACCESS_TOKEN"),
            Some(&"ghp_xxx".to_string())
        );
    }

    #[test]
    fn dedup_across_clients() {
        let mut list = vec![
            DiscoveredServer {
                source: SourceClient::ClaudeDesktop,
                source_path: "/a".into(),
                name: "gh".into(),
                command: "npx".into(),
                args: vec!["-y".into(), "@x/server".into()],
                env: HashMap::new(),
                transport: "stdio".into(),
            },
            DiscoveredServer {
                source: SourceClient::Cursor,
                source_path: "/b".into(),
                name: "gh".into(),
                command: "npx".into(),
                args: vec!["-y".into(), "@x/server".into()],
                env: HashMap::new(),
                transport: "stdio".into(),
            },
        ];
        dedupe(&mut list);
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn malformed_json_yields_empty() {
        let out = parse_client(SourceClient::ClaudeDesktop, "/x", "not json");
        assert!(out.is_empty());
    }
}
