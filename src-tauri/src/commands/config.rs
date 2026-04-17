use crate::store::AppState;
use tauri::State;

#[tauri::command]
pub async fn generate_config(
    client: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let servers = state.servers.lock().map_err(|e| e.to_string())?;
    let enabled: Vec<_> = servers.iter().filter(|s| s.enabled).collect();

    if enabled.is_empty() {
        return Ok(match client.as_str() {
            "codex" => "# No MCP servers configured yet\n# Add servers in MCP Proxy, then regenerate this config\n".to_string(),
            _ => "{}".to_string(),
        });
    }

    match client.as_str() {
        "claude" | "cursor" => generate_claude_cursor(&enabled),
        "codex" => generate_codex(&enabled),
        "vscode" => generate_vscode(&enabled),
        "windsurf" => generate_windsurf(&enabled),
        _ => Err(format!("Unknown client: {client}")),
    }
}

fn generate_claude_cursor(
    servers: &[&mcp_proxy_common::models::McpServerConfig],
) -> Result<String, String> {
    let mut map = serde_json::Map::new();
    for s in servers {
        let mut entry = serde_json::Map::new();
        entry.insert(
            "command".to_string(),
            serde_json::Value::String("mcp-proxy".to_string()),
        );
        entry.insert(
            "args".to_string(),
            serde_json::json!(["run", &s.id]).as_array().unwrap().clone().into(),
        );
        map.insert(s.id.clone(), serde_json::Value::Object(entry));
    }

    let config = serde_json::json!({ "mcpServers": map });
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

fn generate_codex(
    servers: &[&mcp_proxy_common::models::McpServerConfig],
) -> Result<String, String> {
    let mut toml = String::new();
    for s in servers {
        toml.push_str(&format!("[mcp_servers.{}]\n", s.id));
        toml.push_str("command = \"mcp-proxy\"\n");
        toml.push_str(&format!("args = [\"run\", \"{}\"]\n\n", s.id));
    }
    Ok(toml)
}

fn generate_vscode(
    servers: &[&mcp_proxy_common::models::McpServerConfig],
) -> Result<String, String> {
    let mut map = serde_json::Map::new();
    for s in servers {
        let mut entry = serde_json::Map::new();
        entry.insert(
            "type".to_string(),
            serde_json::Value::String("stdio".to_string()),
        );
        entry.insert(
            "command".to_string(),
            serde_json::Value::String("mcp-proxy".to_string()),
        );
        entry.insert(
            "args".to_string(),
            serde_json::json!(["run", &s.id]).as_array().unwrap().clone().into(),
        );
        map.insert(s.id.clone(), serde_json::Value::Object(entry));
    }

    let config = serde_json::json!({ "servers": map });
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

fn generate_windsurf(
    servers: &[&mcp_proxy_common::models::McpServerConfig],
) -> Result<String, String> {
    let mut map = serde_json::Map::new();
    for s in servers {
        let mut entry = serde_json::Map::new();
        entry.insert(
            "command".to_string(),
            serde_json::Value::String("mcp-proxy".to_string()),
        );
        entry.insert(
            "args".to_string(),
            serde_json::json!(["run", &s.id]).as_array().unwrap().clone().into(),
        );
        map.insert(s.id.clone(), serde_json::Value::Object(entry));
    }

    let config = serde_json::json!({ "servers": map });
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Every supported AI client has a slightly different config shape. These tests
// pin down the exact format so a regression in the generator is caught instead
// of silently breaking real clients.
//
// References (as of Apr 2026):
// - Claude Desktop / Cursor: JSON, root key "mcpServers", object of id → config
// - Codex: TOML, "[mcp_servers.id]" tables
// - VS Code: JSON, root key "servers", config entry requires "type": "stdio"
// - Windsurf: JSON, root key "servers" (NOT "mcpServers")

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_proxy_common::models::{EnvMapping, McpServerConfig, RunMode, Transport};

    fn sample_server(id: &str) -> McpServerConfig {
        let mut config = McpServerConfig::new(
            id.to_string(),
            "npx".to_string(),
            vec!["-y".to_string(), format!("@example/{id}")],
            Transport::Stdio,
        );
        config.id = id.to_string(); // fix id instead of UUID for stable tests
        config.run_mode = RunMode::Local;
        config.env_mappings = vec![EnvMapping {
            env_var_name: "TOKEN".to_string(),
            secret_ref: "tok".to_string(),
        }];
        config
    }

    fn as_json(s: &str) -> serde_json::Value {
        serde_json::from_str(s).expect("valid JSON")
    }

    #[test]
    fn claude_cursor_wraps_in_mcp_servers_key() {
        let srv = sample_server("github");
        let out = generate_claude_cursor(&[&srv]).unwrap();
        let v = as_json(&out);
        assert_eq!(v["mcpServers"]["github"]["command"], "mcp-proxy");
        assert_eq!(
            v["mcpServers"]["github"]["args"],
            serde_json::json!(["run", "github"])
        );
        // Crucially: never emit real env vars / secret values here.
        assert!(v["mcpServers"]["github"].get("env").is_none());
    }

    #[test]
    fn codex_uses_toml_table_syntax() {
        let srv = sample_server("slack");
        let out = generate_codex(&[&srv]).unwrap();
        assert!(out.contains("[mcp_servers.slack]"));
        assert!(out.contains(r#"command = "mcp-proxy""#));
        assert!(out.contains(r#"args = ["run", "slack"]"#));
    }

    #[test]
    fn vscode_requires_type_stdio() {
        let srv = sample_server("fs");
        let out = generate_vscode(&[&srv]).unwrap();
        let v = as_json(&out);
        // VS Code uses "servers" key (NOT "mcpServers")
        assert!(v.get("mcpServers").is_none());
        assert_eq!(v["servers"]["fs"]["type"], "stdio");
        assert_eq!(v["servers"]["fs"]["command"], "mcp-proxy");
    }

    #[test]
    fn windsurf_uses_servers_key_without_type() {
        let srv = sample_server("notion");
        let out = generate_windsurf(&[&srv]).unwrap();
        let v = as_json(&out);
        // Windsurf: "servers" key, but unlike VS Code no "type": "stdio" field
        assert_eq!(v["servers"]["notion"]["command"], "mcp-proxy");
        assert!(v["servers"]["notion"].get("type").is_none());
        assert!(v.get("mcpServers").is_none());
    }

    #[test]
    fn generators_emit_no_secret_values() {
        // Regression guard: secrets should NEVER leak into generated configs,
        // even if the server has env_mappings that reference real secret IDs.
        // The whole point is that mcp-proxy resolves them at runtime.
        let srv = sample_server("github");
        let secret_ref = &srv.env_mappings[0].secret_ref;

        for out in [
            generate_claude_cursor(&[&srv]).unwrap(),
            generate_codex(&[&srv]).unwrap(),
            generate_vscode(&[&srv]).unwrap(),
            generate_windsurf(&[&srv]).unwrap(),
        ] {
            assert!(
                !out.contains(secret_ref),
                "generator leaked secret_ref '{secret_ref}' into: {out}"
            );
            assert!(
                !out.to_lowercase().contains("token = "),
                "generator appears to have embedded an env var value: {out}"
            );
        }
    }

    #[test]
    fn multiple_servers_round_trip() {
        let a = sample_server("alpha");
        let b = sample_server("beta");
        let out = generate_claude_cursor(&[&a, &b]).unwrap();
        let v = as_json(&out);
        assert!(v["mcpServers"]["alpha"].is_object());
        assert!(v["mcpServers"]["beta"].is_object());
    }
}
