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
    /// macOS-only: wrap Local-mode child processes with `sandbox-exec` +
    /// generated `.sb` profile. No-op on Linux/Windows (CLI ignores). Opt-in
    /// only — defaults to `false` for backward compatibility.
    #[serde(default)]
    pub sandbox_local: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_launched_at: Option<DateTime<Utc>>,
    /// Persist JSON-RPC traffic + session rows for this server's runs into
    /// `invocations.db`. Defaults to true; toggleable per-server for privacy.
    #[serde(default = "default_true")]
    pub log_invocations: bool,
}

fn default_true() -> bool {
    true
}

/// Docker-style short identifier: the first hyphen-delimited segment of a
/// UUID (8 hex chars for a v4 UUID like `5a4dfc7a-6ea7-...`). If `id` has no
/// hyphen (tests or legacy ids), returns the input unchanged.
pub fn short_id(id: &str) -> &str {
    id.split('-').next().unwrap_or(id)
}

/// Docker-style 12-char hex identifier: strip dashes from a UUID, take the
/// first 12 hex chars. Matches how `docker ps` displays container ids.
/// For non-UUID legacy ids (no dashes, short) returns the id unchanged so
/// tests and hand-set ids keep working.
pub fn hex_id(id: &str) -> String {
    let stripped: String = id.chars().filter(|c| *c != '-').collect();
    if stripped.len() >= 12 && id.contains('-') {
        stripped[..12].to_string()
    } else {
        id.to_string()
    }
}

/// Slugify a server name for use as a config map key.
///
/// Result is constrained to `[a-z0-9_-]+` so it's valid as a TOML bare key
/// (Codex) and unsurprising as a JSON key in the other clients. Non-alnum
/// runs collapse to a single `-`; leading/trailing `-` are trimmed. Returns
/// an empty string if the name contains no alnum characters — callers
/// should fall back to `short_id` in that case.
pub fn slug_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if ch == '_' {
            out.push('_');
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

/// True if `s` looks like a v4 UUID (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`,
/// 36 chars, hex + hyphens at the canonical positions). Used to reject
/// UUID-as-name when building config keys: if a server's `name` was
/// previously imported from a client config where the key was the full
/// UUID (e.g. re-imported from an old mcp-proxy-generated
/// `claude_desktop_config.json`), the name is meaningless and the key
/// should fall back to `short_id` rather than preserving the UUID.
fn looks_like_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        let is_dash_pos = matches!(i, 8 | 13 | 18 | 23);
        let ok = if is_dash_pos {
            *b == b'-'
        } else {
            b.is_ascii_hexdigit()
        };
        if !ok {
            return false;
        }
    }
    true
}

/// Build a list of stable, unique, human-readable config keys for a set of
/// servers. Order matches the input. Uses the slugified name when it's
/// unique; falls back to `{slug}-{short_id}` on collision or `{short_id}`
/// when the name has no slug-safe characters or is itself a UUID.
pub fn config_keys(servers: &[&McpServerConfig]) -> Vec<String> {
    let slugs: Vec<String> = servers
        .iter()
        .map(|s| {
            if looks_like_uuid(&s.name) {
                String::new()
            } else {
                slug_name(&s.name)
            }
        })
        .collect();
    let mut counts = std::collections::HashMap::<&str, usize>::new();
    for slug in &slugs {
        if !slug.is_empty() {
            *counts.entry(slug.as_str()).or_insert(0) += 1;
        }
    }
    servers
        .iter()
        .zip(slugs.iter())
        .map(|(s, slug)| {
            if slug.is_empty() {
                s.short_id().to_string()
            } else if counts.get(slug.as_str()).copied().unwrap_or(0) > 1 {
                format!("{}-{}", slug, s.short_id())
            } else {
                slug.clone()
            }
        })
        .collect()
}

impl McpServerConfig {
    pub fn short_id(&self) -> &str {
        short_id(&self.id)
    }

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
            sandbox_local: false,
            created_at: now,
            updated_at: now,
            first_launched_at: None,
            log_invocations: true,
        }
    }
}

/// How an env var value is produced for the child MCP server process.
///
/// - `Secret`: resolved at runtime from the secret backend (vault / Keychain /
///   1Password). The stored field is the secret id.
/// - `Plaintext`: literal value baked into the server config. Used by the
///   Import-from-clients flow when the user opts to keep an existing plaintext
///   value instead of promoting it to the vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EnvValue {
    Secret { secret_ref: String },
    Plaintext { value: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvMapping {
    pub env_var_name: String,
    pub value: EnvValue,
    /// Legacy: keep old `secret_ref` key on serialized output so downgraded
    /// binaries can still read the config. Mirrors `value` when it is a
    /// `Secret`; empty for `Plaintext`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub secret_ref: String,
}

impl EnvMapping {
    pub fn new_secret(env_var_name: String, secret_ref: String) -> Self {
        Self {
            env_var_name,
            value: EnvValue::Secret {
                secret_ref: secret_ref.clone(),
            },
            secret_ref,
        }
    }

    pub fn new_plaintext(env_var_name: String, value: String) -> Self {
        Self {
            env_var_name,
            value: EnvValue::Plaintext { value },
            secret_ref: String::new(),
        }
    }
}

// Custom deserializer so old configs (`{"env_var_name": "X", "secret_ref": "id"}`)
// still deserialize into the new `EnvValue::Secret` shape.
impl<'de> Deserialize<'de> for EnvMapping {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            env_var_name: String,
            #[serde(default)]
            value: Option<EnvValue>,
            #[serde(default)]
            secret_ref: Option<String>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let (value, mirror) = match (raw.value, raw.secret_ref) {
            (Some(EnvValue::Secret { secret_ref }), _) => {
                let v = EnvValue::Secret {
                    secret_ref: secret_ref.clone(),
                };
                (v, secret_ref)
            }
            (Some(EnvValue::Plaintext { value }), _) => {
                (EnvValue::Plaintext { value }, String::new())
            }
            (None, Some(secret_ref)) => {
                let v = EnvValue::Secret {
                    secret_ref: secret_ref.clone(),
                };
                (v, secret_ref)
            }
            (None, None) => {
                return Err(serde::de::Error::custom(
                    "EnvMapping requires either `value` or legacy `secret_ref`",
                ));
            }
        };
        Ok(EnvMapping {
            env_var_name: raw.env_var_name,
            value,
            secret_ref: mirror,
        })
    }
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
        assert!(config.first_launched_at.is_none());
    }

    /// Old `servers.json` files written before `first_launched_at` existed
    /// must deserialize cleanly with the field absent.
    #[test]
    fn mcp_server_config_round_trips_without_first_launched_at() {
        let legacy = r#"{
            "id": "srv-1",
            "name": "legacy",
            "command": "npx",
            "args": [],
            "transport": {"type": "Stdio"},
            "env_mappings": [],
            "run_mode": {"type": "Local"},
            "enabled": true,
            "trusted": false,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;
        let parsed: McpServerConfig = serde_json::from_str(legacy).unwrap();
        assert!(parsed.first_launched_at.is_none());
        // Re-serializing must not emit the field (skip_serializing_if).
        let json = serde_json::to_string(&parsed).unwrap();
        assert!(!json.contains("first_launched_at"));
    }

    /// `sandbox_local` is the macOS-only opt-in hardening flag. Legacy configs
    /// written before the field existed must still deserialize, and the default
    /// must be `false`.
    #[test]
    fn mcp_server_config_defaults_sandbox_local_false() {
        let legacy = r#"{
            "id": "srv-1",
            "name": "legacy",
            "command": "npx",
            "args": [],
            "transport": {"type": "Stdio"},
            "env_mappings": [],
            "run_mode": {"type": "Local"},
            "enabled": true,
            "trusted": false,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;
        let parsed: McpServerConfig = serde_json::from_str(legacy).unwrap();
        assert!(!parsed.sandbox_local);
    }

    #[test]
    fn mcp_server_config_round_trips_with_sandbox_local() {
        let mut config =
            McpServerConfig::new("t".to_string(), "npx".to_string(), vec![], Transport::Stdio);
        config.sandbox_local = true;
        let json = serde_json::to_string(&config).unwrap();
        let back: McpServerConfig = serde_json::from_str(&json).unwrap();
        assert!(back.sandbox_local);
    }

    #[test]
    fn env_mapping_legacy_schema_deserializes() {
        // Old `servers.json` rows used `{"env_var_name": "...", "secret_ref": "..."}`
        // with no tagged `value` field. Must still parse into the new shape.
        let legacy = r#"{"env_var_name":"GITHUB_TOKEN","secret_ref":"sec-1"}"#;
        let parsed: EnvMapping = serde_json::from_str(legacy).unwrap();
        assert_eq!(parsed.env_var_name, "GITHUB_TOKEN");
        match parsed.value {
            EnvValue::Secret { secret_ref } => assert_eq!(secret_ref, "sec-1"),
            _ => panic!("expected Secret"),
        }
        assert_eq!(parsed.secret_ref, "sec-1");
    }

    #[test]
    fn env_mapping_plaintext_round_trips() {
        let m = EnvMapping::new_plaintext("FOO".to_string(), "bar".to_string());
        let json = serde_json::to_string(&m).unwrap();
        let back: EnvMapping = serde_json::from_str(&json).unwrap();
        match back.value {
            EnvValue::Plaintext { value } => assert_eq!(value, "bar"),
            _ => panic!("expected Plaintext"),
        }
    }

    #[test]
    fn env_mapping_secret_round_trips_with_mirror() {
        let m = EnvMapping::new_secret("FOO".to_string(), "sec-9".to_string());
        let json = serde_json::to_string(&m).unwrap();
        // Legacy mirror field present so downgraded binaries still resolve it.
        assert!(json.contains("\"secret_ref\":\"sec-9\""));
        let back: EnvMapping = serde_json::from_str(&json).unwrap();
        assert!(matches!(back.value, EnvValue::Secret { .. }));
    }

    #[test]
    fn mcp_server_config_log_invocations_defaults_true() {
        let legacy = r#"{
            "id": "srv-1",
            "name": "legacy",
            "command": "npx",
            "args": [],
            "transport": {"type": "Stdio"},
            "env_mappings": [],
            "run_mode": {"type": "Local"},
            "enabled": true,
            "trusted": false,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;
        let parsed: McpServerConfig = serde_json::from_str(legacy).unwrap();
        assert!(parsed.log_invocations);
    }

    #[test]
    fn mcp_server_config_round_trips_with_first_launched_at() {
        let mut config =
            McpServerConfig::new("t".to_string(), "npx".to_string(), vec![], Transport::Stdio);
        let ts: DateTime<Utc> = "2026-04-17T10:00:00Z".parse().unwrap();
        config.first_launched_at = Some(ts);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("first_launched_at"));
        let back: McpServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.first_launched_at, Some(ts));
    }
}
