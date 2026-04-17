//! Tauri-specific AppState wrapper around the shared JSON store.

use mcp_proxy_common::models::{McpServerConfig, SecretMeta};
use mcp_proxy_common::store::{
    load_json, save_json, secrets_meta_path, servers_path,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::process::Child;

pub struct AppState {
    pub servers: Mutex<Vec<McpServerConfig>>,
    pub secrets_meta: Mutex<Vec<SecretMeta>>,
    pub running_proxies: Mutex<HashMap<String, Child>>,
    #[allow(dead_code)]
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new() -> Self {
        let servers = load_json(servers_path()).unwrap_or_default();
        let secrets_meta = load_json(secrets_meta_path()).unwrap_or_default();
        Self {
            servers: Mutex::new(servers),
            secrets_meta: Mutex::new(secrets_meta),
            running_proxies: Mutex::new(HashMap::new()),
            data_dir: mcp_proxy_common::store::app_data_dir(),
        }
    }

    pub fn save_servers(&self, servers: &[McpServerConfig]) {
        save_json(servers_path(), servers);
    }

    pub fn save_secrets_meta(&self, metas: &[SecretMeta]) {
        save_json(secrets_meta_path(), metas);
    }
}
