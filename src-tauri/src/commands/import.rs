//! Import existing MCP server configs from other AI clients (Claude Desktop,
//! Codex, Cursor, VS Code, Windsurf, Claude Code).
//!
//! Two-phase flow:
//! 1. `discover_client_servers` — scan all known client config paths, return
//!    a unified list the UI can render for user selection.
//! 2. `import_servers` — take the user's decisions (which env vars to promote
//!    to the vault vs keep as plaintext) and create the matching
//!    `McpServerConfig` rows + `SecretMeta` entries.

use crate::store::AppState;
use mcp_proxy_common::client_read::{discover_all, DiscoveredServer};
use mcp_proxy_common::models::{EnvMapping, McpServerConfig, SecretMeta, SecretSource, Transport};
use mcp_proxy_common::secret_resolver::store_secret_local;
use serde::{Deserialize, Serialize};
use tauri::State;

#[tauri::command]
pub async fn discover_client_servers() -> Result<Vec<DiscoveredServer>, String> {
    Ok(discover_all())
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum EnvDecision {
    Secret {
        env_var_name: String,
        /// Suggested id for the new secret (e.g. `github_token`).
        secret_id: String,
        /// Human-readable label stored on the secret metadata row.
        label: String,
        /// The plaintext value from the discovered config. Written into the
        /// local vault / Keychain and not persisted in `servers.json`.
        value: String,
    },
    Plaintext {
        env_var_name: String,
        value: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct ImportSelection {
    pub discovered: DiscoveredServer,
    pub env_decisions: Vec<EnvDecision>,
    pub trusted: bool,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub server_id: String,
    pub server_name: String,
    pub created_secret_ids: Vec<String>,
}

#[tauri::command]
pub async fn import_servers(
    selections: Vec<ImportSelection>,
    state: State<'_, AppState>,
) -> Result<Vec<ImportResult>, String> {
    let mut results = Vec::with_capacity(selections.len());

    for sel in selections {
        let discovered = sel.discovered;

        // Write any new secrets into the local vault + secrets_meta.
        let mut created_secret_ids = Vec::new();
        let mut mappings = Vec::new();
        for decision in sel.env_decisions {
            match decision {
                EnvDecision::Secret {
                    env_var_name,
                    secret_id,
                    label,
                    value,
                } => {
                    store_secret_local(&secret_id, &value)?;
                    {
                        let mut metas = state.secrets_meta.lock().map_err(|e| e.to_string())?;
                        if !metas.iter().any(|m| m.id == secret_id) {
                            metas.push(SecretMeta::new(
                                secret_id.clone(),
                                label,
                                SecretSource::Local,
                            ));
                            state.save_secrets_meta(&metas);
                        }
                    }
                    created_secret_ids.push(secret_id.clone());
                    mappings.push(EnvMapping::new_secret(env_var_name, secret_id));
                }
                EnvDecision::Plaintext {
                    env_var_name,
                    value,
                } => {
                    mappings.push(EnvMapping::new_plaintext(env_var_name, value));
                }
            }
        }

        let transport = match discovered.transport.as_str() {
            "sse" => Transport::Sse {
                port: 3000,
                path: "/sse".into(),
            },
            _ => Transport::Stdio,
        };
        let mut config = McpServerConfig::new(
            discovered.name.clone(),
            discovered.command,
            discovered.args,
            transport,
        );
        config.env_mappings = mappings;
        config.trusted = sel.trusted;

        {
            let mut servers = state.servers.lock().map_err(|e| e.to_string())?;
            servers.push(config.clone());
            state.save_servers(&servers);
        }

        results.push(ImportResult {
            server_id: config.id,
            server_name: config.name,
            created_secret_ids,
        });
    }

    Ok(results)
}
