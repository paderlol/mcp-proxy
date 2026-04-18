use crate::store::AppState;
use tauri::State;

#[tauri::command]
pub async fn generate_config(client: String, state: State<'_, AppState>) -> Result<String, String> {
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

// The config map key is a human-readable slug derived from the server name
// (via `config_keys`), but the `args` passed to `mcp-proxy run` use the
// stable UUID so that renaming a server in the desktop app doesn't break
// existing AI-client configs — the CLI resolves by id.

fn generate_claude_cursor(
    servers: &[&mcp_proxy_common::models::McpServerConfig],
) -> Result<String, String> {
    let keys = mcp_proxy_common::models::config_keys(servers);
    let mut map = serde_json::Map::new();
    for (s, key) in servers.iter().zip(keys) {
        let mut entry = serde_json::Map::new();
        entry.insert(
            "command".to_string(),
            serde_json::Value::String("mcp-proxy".to_string()),
        );
        entry.insert(
            "args".to_string(),
            serde_json::json!(["run", mcp_proxy_common::models::hex_id(&s.id)]),
        );
        map.insert(key, serde_json::Value::Object(entry));
    }

    let config = serde_json::json!({ "mcpServers": map });
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

fn generate_codex(
    servers: &[&mcp_proxy_common::models::McpServerConfig],
) -> Result<String, String> {
    let keys = mcp_proxy_common::models::config_keys(servers);
    let mut toml = String::new();
    for (s, key) in servers.iter().zip(keys) {
        toml.push_str(&format!("[mcp_servers.{}]\n", key));
        toml.push_str("command = \"mcp-proxy\"\n");
        toml.push_str(&format!(
            "args = [\"run\", \"{}\"]\n\n",
            mcp_proxy_common::models::hex_id(&s.id)
        ));
    }
    Ok(toml)
}

fn generate_vscode(
    servers: &[&mcp_proxy_common::models::McpServerConfig],
) -> Result<String, String> {
    let keys = mcp_proxy_common::models::config_keys(servers);
    let mut map = serde_json::Map::new();
    for (s, key) in servers.iter().zip(keys) {
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
            serde_json::json!(["run", mcp_proxy_common::models::hex_id(&s.id)]),
        );
        map.insert(key, serde_json::Value::Object(entry));
    }

    let config = serde_json::json!({ "servers": map });
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

fn generate_windsurf(
    servers: &[&mcp_proxy_common::models::McpServerConfig],
) -> Result<String, String> {
    let keys = mcp_proxy_common::models::config_keys(servers);
    let mut map = serde_json::Map::new();
    for (s, key) in servers.iter().zip(keys) {
        let mut entry = serde_json::Map::new();
        entry.insert(
            "command".to_string(),
            serde_json::Value::String("mcp-proxy".to_string()),
        );
        entry.insert(
            "args".to_string(),
            serde_json::json!(["run", mcp_proxy_common::models::hex_id(&s.id)]),
        );
        map.insert(key, serde_json::Value::Object(entry));
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
        config.env_mappings = vec![EnvMapping::new_secret(
            "TOKEN".to_string(),
            "tok".to_string(),
        )];
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
    fn keys_prefer_slugified_name_over_uuid() {
        // Real servers get UUID v4 ids. The config map key must come from
        // the human-readable name, not the UUID.
        let mut srv = sample_server("placeholder");
        srv.name = "GitHub MCP".to_string();
        srv.id = "5a4dfc7a-6ea7-4a74-995c-4ab599247142".to_string();

        let out = generate_claude_cursor(&[&srv]).unwrap();
        let v = as_json(&out);
        assert!(v["mcpServers"]["github-mcp"].is_object());
        assert!(v["mcpServers"].get(&srv.id).is_none());
        // Map key is the friendly slug; args use the docker-style 12-char
        // hex id (stable under rename since it's derived from the immutable
        // UUID, and short enough to read at a glance).
        let hex = "5a4dfc7a6ea7";
        assert_eq!(
            v["mcpServers"]["github-mcp"]["args"],
            serde_json::json!(["run", hex])
        );
        assert!(!out.contains(&srv.id));

        let codex = generate_codex(&[&srv]).unwrap();
        assert!(codex.contains("[mcp_servers.github-mcp]"));
        assert!(codex.contains(&format!(r#"args = ["run", "{hex}"]"#)));
        assert!(!codex.contains(&srv.id));
    }

    #[test]
    fn duplicate_slugs_get_short_id_suffix() {
        // Two servers with names that slug identically must not collide —
        // keep slug for one and append short-id to the other, or suffix both.
        let mut a = sample_server("a");
        a.name = "GitHub".to_string();
        a.id = "aaaaaaaa-1111-2222-3333-444444444444".to_string();
        let mut b = sample_server("b");
        b.name = "github".to_string();
        b.id = "bbbbbbbb-1111-2222-3333-444444444444".to_string();

        let out = generate_claude_cursor(&[&a, &b]).unwrap();
        let v = as_json(&out);
        let servers = v["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 2);
        assert!(servers.contains_key("github-aaaaaaaa"));
        assert!(servers.contains_key("github-bbbbbbbb"));
    }

    #[test]
    fn uuid_shaped_name_falls_back_to_short_id() {
        // Regression: if a server was imported from a client config whose
        // keys were previously-generated UUIDs (e.g. re-importing an old
        // mcp-proxy-written claude_desktop_config.json), the `name` field
        // ends up holding the UUID. That UUID-as-name must not leak back
        // into the config — treat it as if the name were empty.
        let mut srv = sample_server("x");
        srv.name = "5a4dfc7a-6ea7-4a74-995c-4ab599247142".to_string();
        srv.id = "5a4dfc7a-6ea7-4a74-995c-4ab599247142".to_string();

        let out = generate_claude_cursor(&[&srv]).unwrap();
        let v = as_json(&out);
        let servers = v["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("5a4dfc7a"));
        assert!(!servers.contains_key(&srv.id));
    }

    #[test]
    fn empty_or_symbolic_name_falls_back_to_short_id() {
        let mut srv = sample_server("x");
        srv.name = "***".to_string();
        srv.id = "cafebabe-0000-0000-0000-000000000000".to_string();

        let out = generate_claude_cursor(&[&srv]).unwrap();
        let v = as_json(&out);
        assert!(v["mcpServers"]["cafebabe"].is_object());
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
