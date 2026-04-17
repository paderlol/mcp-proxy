use crate::store::AppState;
use mcp_proxy_common::models::{EnvMapping, McpServerConfig, RunMode, Transport};
use tauri::State;

#[tauri::command]
pub async fn list_servers(state: State<'_, AppState>) -> Result<Vec<McpServerConfig>, String> {
    let servers = state.servers.lock().map_err(|e| e.to_string())?;
    Ok(servers.clone())
}

#[tauri::command]
pub async fn get_server(id: String, state: State<'_, AppState>) -> Result<McpServerConfig, String> {
    let servers = state.servers.lock().map_err(|e| e.to_string())?;
    servers
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| format!("Server '{id}' not found"))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_server(
    name: String,
    command: String,
    args: Vec<String>,
    transport_type: String,
    sse_port: Option<u16>,
    sse_path: Option<String>,
    run_mode_type: Option<String>,
    docker_image: Option<String>,
    env_mappings: Option<Vec<EnvMapping>>,
    trusted: Option<bool>,
    state: State<'_, AppState>,
) -> Result<McpServerConfig, String> {
    let transport = match transport_type.as_str() {
        "stdio" => Transport::Stdio,
        "sse" => Transport::Sse {
            port: sse_port.unwrap_or(3000),
            path: sse_path.unwrap_or_else(|| "/sse".to_string()),
        },
        _ => return Err("Invalid transport type".to_string()),
    };

    let mut config = McpServerConfig::new(name, command, args, transport);

    if run_mode_type.as_deref() == Some("docker") {
        config.run_mode = RunMode::DockerSandbox {
            image: docker_image,
            extra_args: Vec::new(),
        };
    }

    if let Some(mappings) = env_mappings {
        config.env_mappings = mappings;
    }

    if let Some(trusted) = trusted {
        config.trusted = trusted;
    }

    let mut servers = state.servers.lock().map_err(|e| e.to_string())?;
    servers.push(config.clone());
    state.save_servers(&servers);
    Ok(config)
}

#[tauri::command]
pub async fn update_server(
    server: McpServerConfig,
    state: State<'_, AppState>,
) -> Result<McpServerConfig, String> {
    let mut servers = state.servers.lock().map_err(|e| e.to_string())?;
    if let Some(existing) = servers.iter_mut().find(|s| s.id == server.id) {
        // `first_launched_at` is written only by the CLI on launch; the
        // frontend never sends it back. Preserve the stored value across
        // GUI updates so a re-save doesn't wipe the launch history.
        let preserved_first_launched_at = existing.first_launched_at;
        *existing = server.clone();
        existing.first_launched_at = preserved_first_launched_at;
        let merged = existing.clone();
        state.save_servers(&servers);
        Ok(merged)
    } else {
        Err(format!("Server '{}' not found", server.id))
    }
}

#[tauri::command]
pub async fn delete_server(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut servers = state.servers.lock().map_err(|e| e.to_string())?;
    let before = servers.len();
    servers.retain(|s| s.id != id);
    if servers.len() == before {
        return Err(format!("Server '{id}' not found"));
    }
    state.save_servers(&servers);
    Ok(())
}
