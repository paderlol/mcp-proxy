use crate::store::AppState;
use mcp_proxy_common::secret_resolver::resolve_secret;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub server_id: String,
    pub running: bool,
    pub pid: Option<u32>,
}

#[tauri::command]
pub async fn start_proxy(
    server_id: String,
    state: State<'_, AppState>,
) -> Result<ProxyStatus, String> {
    // 1. Snapshot server config + secret metas (drop locks before await)
    let (config, secret_metas) = {
        let servers = state.servers.lock().map_err(|e| e.to_string())?;
        let config = servers
            .iter()
            .find(|s| s.id == server_id)
            .cloned()
            .ok_or_else(|| format!("Server '{server_id}' not found"))?;
        let metas = state.secrets_meta.lock().map_err(|e| e.to_string())?;
        (config, metas.clone())
    };

    // 2. Resolve env vars from secrets
    let mut env_vars = std::collections::HashMap::new();
    for mapping in &config.env_mappings {
        let meta = secret_metas
            .iter()
            .find(|m| m.id == mapping.secret_ref)
            .ok_or_else(|| format!("Secret '{}' not found", mapping.secret_ref))?;

        let value = resolve_secret(&meta.id, &meta.source).await?;
        env_vars.insert(mapping.env_var_name.clone(), value);
    }

    // 3. Spawn child process
    let child = Command::new(&config.command)
        .args(&config.args)
        .envs(&env_vars)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {e}", config.command))?;

    let pid = child.id();

    // 4. Store in running proxies
    {
        let mut proxies = state.running_proxies.lock().map_err(|e| e.to_string())?;
        proxies.insert(server_id.clone(), child);
    }

    Ok(ProxyStatus {
        server_id,
        running: true,
        pid,
    })
}

#[tauri::command]
pub async fn stop_proxy(
    server_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut child = {
        let mut proxies = state.running_proxies.lock().map_err(|e| e.to_string())?;
        proxies
            .remove(&server_id)
            .ok_or_else(|| format!("No running proxy for server '{server_id}'"))?
    };
    child
        .kill()
        .await
        .map_err(|e| format!("Failed to kill proxy: {e}"))
}

#[tauri::command]
pub async fn get_proxy_status(
    server_id: String,
    state: State<'_, AppState>,
) -> Result<ProxyStatus, String> {
    let mut proxies = state.running_proxies.lock().map_err(|e| e.to_string())?;

    if let Some(child) = proxies.get_mut(&server_id) {
        match child.try_wait() {
            Ok(Some(_)) => {
                proxies.remove(&server_id);
                Ok(ProxyStatus {
                    server_id,
                    running: false,
                    pid: None,
                })
            }
            Ok(None) => Ok(ProxyStatus {
                server_id,
                running: true,
                pid: child.id(),
            }),
            Err(e) => Err(format!("Failed to check process status: {e}")),
        }
    } else {
        Ok(ProxyStatus {
            server_id,
            running: false,
            pid: None,
        })
    }
}
