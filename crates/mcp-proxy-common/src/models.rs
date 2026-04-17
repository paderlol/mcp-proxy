use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub transport: Transport,
    pub env_mappings: Vec<EnvMapping>,
    pub run_mode: RunMode,
    pub enabled: bool,
    pub trusted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl McpServerConfig {
    pub fn new(name: String, command: String, args: Vec<String>, transport: Transport) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            command,
            args,
            transport,
            env_mappings: Vec::new(),
            run_mode: RunMode::Local,
            enabled: true,
            trusted: false,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvMapping {
    pub env_var_name: String,
    pub secret_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMeta {
    pub id: String,
    pub label: String,
    pub source: SecretSource,
    pub server_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SecretMeta {
    pub fn new(id: String, label: String, source: SecretSource) -> Self {
        let now = Utc::now();
        Self {
            id,
            label,
            source,
            server_ids: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Where a secret's value is stored/fetched from.
///
/// Only two conceptual choices are exposed to users:
/// - `Local`: the app stores the secret on this device (platform auto-selects
///   Keychain on macOS, AES-256 encrypted file elsewhere). See `local_backend`.
/// - `OnePassword`: a reference to an external 1Password secret, fetched on demand.
///
/// The `#[serde(alias = ...)]` attributes accept pre-refactor tags so existing
/// `secrets_meta.json` files keep working: old `"Keychain"` and `"EncryptedFile"`
/// both map to `Local`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SecretSource {
    /// Stored locally by the app. Backend auto-selected per platform.
    #[serde(alias = "Keychain", alias = "EncryptedFile")]
    Local,
    /// Reference to a secret in 1Password, fetched via `op read <reference>`.
    OnePassword {
        /// 1Password secret reference, e.g. "op://vault/item/field"
        reference: String,
    },
}

/// How to run the MCP server process.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RunMode {
    /// Spawn directly as a local child process. Fast, no isolation.
    Local,
    /// Run inside a Docker container for filesystem/network isolation.
    /// Secrets are injected via a one-time token + ephemeral localhost API.
    DockerSandbox {
        /// Docker image to use. If empty, auto-built from command + args.
        image: Option<String>,
        /// Extra docker run flags (e.g., network restrictions, volume mounts).
        extra_args: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Transport {
    Stdio,
    Sse { port: u16, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerStatus {
    Stopped,
    Running,
    Error(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// These tests enforce the contracts described in TEST_RULES.md §3.1:
// any serde change to these types must round-trip and preserve aliases.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_source_local_round_trips() {
        let source = SecretSource::Local;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, r#"{"type":"Local"}"#);
        let back: SecretSource = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, SecretSource::Local));
    }

    #[test]
    fn secret_source_onepassword_round_trips() {
        let source = SecretSource::OnePassword {
            reference: "op://vault/item/field".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        let back: SecretSource = serde_json::from_str(&json).unwrap();
        match back {
            SecretSource::OnePassword { reference } => {
                assert_eq!(reference, "op://vault/item/field");
            }
            _ => panic!("wrong variant after round-trip"),
        }
    }

    /// Legacy `secrets_meta.json` files created before the Local/OnePassword
    /// refactor used `"type": "Keychain"`. The serde alias on `SecretSource::Local`
    /// must keep those files readable.
    #[test]
    fn secret_source_accepts_legacy_keychain_tag() {
        let legacy = r#"{"type":"Keychain"}"#;
        let parsed: SecretSource = serde_json::from_str(legacy).unwrap();
        assert!(matches!(parsed, SecretSource::Local));
    }

    /// Legacy `"EncryptedFile"` tag — same contract.
    #[test]
    fn secret_source_accepts_legacy_encrypted_file_tag() {
        let legacy = r#"{"type":"EncryptedFile"}"#;
        let parsed: SecretSource = serde_json::from_str(legacy).unwrap();
        assert!(matches!(parsed, SecretSource::Local));
    }

    #[test]
    fn mcp_server_config_new_defaults() {
        let config = McpServerConfig::new(
            "test".to_string(),
            "npx".to_string(),
            vec!["-y".to_string(), "@example/server".to_string()],
            Transport::Stdio,
        );
        assert_eq!(config.name, "test");
        assert!(config.enabled);
        assert!(!config.trusted);
        assert!(config.env_mappings.is_empty());
        assert!(matches!(config.run_mode, RunMode::Local));
        assert!(matches!(config.transport, Transport::Stdio));
    }
}
