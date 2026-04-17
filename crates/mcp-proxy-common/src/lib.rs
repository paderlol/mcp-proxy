pub mod local_backend;
pub mod models;
pub mod secret_resolver;
pub mod session;
pub mod store;
pub mod vault;

/// App identifier — must match `identifier` in src-tauri/tauri.conf.json
pub const APP_IDENTIFIER: &str = "com.mcp-proxy.app";

/// Keychain service name (scoped under APP_IDENTIFIER namespace)
pub const KEYCHAIN_SERVICE: &str = "com.mcp-proxy";
