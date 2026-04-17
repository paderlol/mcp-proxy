//! `mcp-proxy` CLI — wraps MCP server invocations with transparent secret injection.
//!
//! Invoked by AI clients (Claude Desktop, Codex, Cursor, etc.) as configured in
//! their respective config files:
//!   { "command": "mcp-proxy", "args": ["run", "<server-id>"] }
//!
//! Reads server + secret metadata from the same JSON store as the Tauri desktop app,
//! resolves secrets from their backends (Keychain / 1Password / EncryptedFile),
//! spawns the real MCP server with env vars injected, and uses inherited stdio
//! so MCP protocol traffic flows transparently.

mod docker;

use clap::{Parser, Subcommand};
use mcp_proxy_common::audit::{append_audit_log, AuditLogEntry, AuditStatus};
use mcp_proxy_common::models::{McpServerConfig, RunMode, SecretMeta};
use mcp_proxy_common::secret_resolver::resolve_secret;
use mcp_proxy_common::store::{
    app_data_dir, load_json, save_json, secrets_meta_path, servers_path,
};
use std::collections::HashMap;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "mcp-proxy")]
#[command(
    about = "MCP Proxy — wrap MCP servers with secure secret injection",
    long_about = "A CLI wrapper that resolves secrets from local storage (Keychain on \
                  macOS, encrypted vault on other platforms) or from 1Password, injects \
                  them as environment variables, and transparently proxies stdio to the \
                  real MCP server process."
)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an MCP server with secrets resolved and injected as env vars
    Run {
        /// Server ID as configured in the MCP Proxy desktop app
        server_id: String,
    },
    /// List all configured MCP servers
    List,
}

fn main() {
    // Keep logging off stdout — stdio is reserved for MCP protocol traffic
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run { server_id } => run_server(&server_id),
        Commands::List => list_servers(),
    };

    if let Err(e) = result {
        eprintln!("mcp-proxy: {e}");
        std::process::exit(1);
    }
}

fn run_server(server_id: &str) -> Result<(), String> {
    // 0. On non-macOS platforms the "Local" secret backend is an encrypted
    //    vault that needs unlocking before any read. AI clients launching us
    //    can't prompt for a password, so we try two sources in order:
    //
    //      1. A session file written by the GUI at unlock time (preferred —
    //         no /proc env-var leak, lifetime bound to GUI session).
    //      2. `MCP_PROXY_MASTER_PASSWORD` env var (fallback for headless
    //         setups or users who haven't launched the GUI this session).
    //
    //    On macOS this block is compiled out — Keychain just works.
    #[cfg(not(target_os = "macos"))]
    {
        use mcp_proxy_common::local_backend;
        if local_backend::vault_exists() && !local_backend::is_unlocked() {
            // Session file fast path.
            match local_backend::unlock_from_session() {
                Ok(true) => { /* loaded from session */ }
                Ok(false) => {
                    // No session on disk — fall back to env var.
                    match std::env::var("MCP_PROXY_MASTER_PASSWORD") {
                        Ok(pw) => local_backend::unlock_vault(&pw)?,
                        Err(_) => {
                            return Err(
                                "Local vault is locked. Unlock the vault in the MCP Proxy \
                                 desktop app, or set MCP_PROXY_MASTER_PASSWORD before invoking \
                                 mcp-proxy."
                                    .to_string(),
                            );
                        }
                    }
                }
                Err(_e) => {
                    // Stale session (password rotated, vault missing, etc.).
                    // Fall through to env var.
                    tracing::warn!("stale session file; falling back to env var");
                    match std::env::var("MCP_PROXY_MASTER_PASSWORD") {
                        Ok(pw) => local_backend::unlock_vault(&pw)?,
                        Err(_) => {
                            return Err(
                                "Local vault session expired. Unlock the vault again in the \
                                 MCP Proxy desktop app, or set MCP_PROXY_MASTER_PASSWORD."
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
    }

    // 1. Load server configs
    let servers: Vec<McpServerConfig> = load_json(servers_path()).ok_or_else(|| {
        format!(
            "No servers.json found at {}. \
             Add a server via the MCP Proxy desktop app first.",
            servers_path().display()
        )
    })?;

    let config = servers
        .iter()
        .find(|s| s.id == server_id)
        .cloned()
        .ok_or_else(|| format!("Server '{server_id}' not found in servers.json"))?;

    if !config.enabled {
        return Err(format!("Server '{server_id}' is disabled"));
    }

    // Trust gate: untrusted servers must be reviewed and marked Trusted in the
    // desktop app before an AI client is allowed to launch them. Enforced here
    // (before secret resolution) so untrusted configs never trigger a secret
    // read or audit entry.
    if !config.trusted {
        return Err(format!(
            "Server '{}' is not trusted. Review and mark it as Trusted in the \
             MCP Proxy desktop app before launching it from an AI client.",
            config.name
        ));
    }

    // 2. Load secret metadata
    let secret_metas: Vec<SecretMeta> = load_json(secrets_meta_path()).unwrap_or_default();

    // 3. Resolve env vars from secrets (uses tokio for async resolvers)
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

    let mut env_vars: HashMap<String, String> = HashMap::new();
    for mapping in &config.env_mappings {
        let meta = secret_metas
            .iter()
            .find(|m| m.id == mapping.secret_ref)
            .ok_or_else(|| {
                format!(
                    "Secret '{}' referenced by env var '{}' not found",
                    mapping.secret_ref, mapping.env_var_name
                )
            })?;

        tracing::debug!(
            "Resolving secret '{}' for env var '{}'",
            meta.id,
            mapping.env_var_name
        );

        let resolved = runtime.block_on(resolve_secret(&meta.id, &meta.source));
        let source_name = match &meta.source {
            mcp_proxy_common::models::SecretSource::Local => "Local",
            mcp_proxy_common::models::SecretSource::OnePassword { .. } => "OnePassword",
        };
        let status = match &resolved {
            Ok(_) => AuditStatus::Success,
            Err(err) => AuditStatus::Error(err.clone()),
        };
        if let Err(err) = append_audit_log(&AuditLogEntry {
            timestamp: chrono::Utc::now(),
            server_id: config.id.clone(),
            secret_id: meta.id.clone(),
            source: source_name.to_string(),
            status,
        }) {
            tracing::warn!("failed to append audit log: {err}");
        }
        let value = resolved?;
        env_vars.insert(mapping.env_var_name.clone(), value);
    }

    tracing::info!(
        "Launching server '{}' (command: {}, {} env vars)",
        config.name,
        config.command,
        env_vars.len()
    );

    // Record the first successful launch timestamp. Observational only —
    // failures here are logged but must not block the launch.
    if config.first_launched_at.is_none() {
        if let Err(err) = record_first_launch(&config.id) {
            tracing::warn!("failed to record first_launched_at: {err}");
        }
    }

    // 4. Dispatch based on run mode
    match &config.run_mode {
        RunMode::Local => spawn_local(&config, env_vars),
        RunMode::DockerSandbox { image, extra_args } => {
            let image = image.as_deref().ok_or_else(|| {
                "Docker sandbox requires a base image — edit the server config and set one \
                 (e.g., `node:20-alpine` for npx-based servers)."
                    .to_string()
            })?;
            let build_root = app_data_dir().join("docker-build");
            docker::run_sandbox(docker::SandboxConfig {
                server_id: &config.id,
                image,
                command: &config.command,
                args: &config.args,
                env_vars: &env_vars,
                extra_args,
                build_root: &build_root,
            })
        }
    }
}

/// Re-read `servers.json`, stamp `first_launched_at` on the matching entry if
/// still unset, and write back. Idempotent: returns Ok without writing if the
/// field is already populated (e.g., a concurrent run beat us to it).
fn record_first_launch(server_id: &str) -> Result<(), String> {
    let path = servers_path();
    let mut servers: Vec<McpServerConfig> = load_json(&path)
        .ok_or_else(|| "servers.json vanished between load and write".to_string())?;

    let mut changed = false;
    for s in &mut servers {
        if s.id == server_id && s.first_launched_at.is_none() {
            s.first_launched_at = Some(chrono::Utc::now());
            changed = true;
            break;
        }
    }

    if changed {
        save_json(&path, &servers);
    }
    Ok(())
}

fn spawn_local(config: &McpServerConfig, env_vars: HashMap<String, String>) -> Result<(), String> {
    // Inherit stdio: AI client's stdin/stdout IS our stdin/stdout,
    // and the child inherits them directly. MCP protocol traffic flows through
    // without any manual piping on our side.
    let mut child = Command::new(&config.command)
        .args(&config.args)
        .envs(&env_vars)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {e}", config.command))?;

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for child process: {e}"))?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }

    Ok(())
}

fn list_servers() -> Result<(), String> {
    let servers: Vec<McpServerConfig> = load_json(servers_path()).unwrap_or_default();

    if servers.is_empty() {
        println!("No MCP servers configured.");
        println!("Add servers via the MCP Proxy desktop app, then they'll appear here.");
        return Ok(());
    }

    println!("Configured MCP servers:\n");
    for s in &servers {
        let status = if s.enabled { "enabled" } else { "disabled" };
        let mode = match &s.run_mode {
            RunMode::Local => "local",
            RunMode::DockerSandbox { .. } => "docker",
        };
        println!("  {}  ({}, {})", s.id, status, mode);
        println!("    name:    {}", s.name);
        println!("    command: {} {}", s.command, s.args.join(" "));
        if !s.env_mappings.is_empty() {
            println!("    env vars:");
            for m in &s.env_mappings {
                println!("      {} → secret:{}", m.env_var_name, m.secret_ref);
            }
        }
        println!();
    }
    Ok(())
}
